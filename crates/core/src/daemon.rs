//! Daemon DevLock: mengawasi service (start/stop/restart otomatis) dan menerima perintah
//! dari CLI (dan nanti GUI) lewat JSON per baris di `127.0.0.1:<daemon.port>`.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::paths::Paths;
use crate::process::{self, Process};
use crate::services::{self, ServiceSpec};

/// Service yang mati sebelum berjalan selama ini dihitung gagal start.
const MIN_HEALTHY: Duration = Duration::from_secs(5);
const MAX_FAILURES: u32 = 5;
const STOP_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "lowercase")]
pub enum Request {
    Status,
    Start {
        target: String,
    },
    Stop {
        target: String,
    },
    Restart {
        target: String,
    },
    /// Baca ulang konfigurasi & folder www, terapkan perubahan.
    Reload,
    Shutdown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub ok: bool,
    pub message: String,
    pub services: Vec<ServiceStatus>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceStatus {
    pub name: String,
    pub group: String,
    pub state: String,
    pub pid: Option<u32>,
    pub port: Option<u16>,
    pub uptime_secs: Option<u64>,
    pub restarts: u32,
    pub error: Option<String>,
    pub log: String,
}

fn backoff(failures: u32) -> Duration {
    Duration::from_secs(1 << failures.min(4))
}

struct Managed {
    spec: ServiceSpec,
    process: Option<Process>,
    wanted: bool,
    started_at: Option<Instant>,
    next_retry: Option<Instant>,
    restarts: u32,
    failures: u32,
    error: Option<String>,
}

impl Managed {
    fn new(spec: ServiceSpec) -> Self {
        Self {
            spec,
            process: None,
            wanted: false,
            started_at: None,
            next_retry: None,
            restarts: 0,
            failures: 0,
            error: None,
        }
    }

    fn start(&mut self) {
        self.error = None;
        self.next_retry = None;
        if let Err(err) = self.try_start() {
            self.error = Some(format!("{err:#}"));
            self.wanted = false;
        }
    }

    fn try_start(&mut self) -> Result<()> {
        if self.process.is_some() {
            return Ok(());
        }
        if let (Some(init), Some(marker)) = (&self.spec.init, &self.spec.init_marker) {
            if !marker.exists() {
                if let Some(parent) = marker.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                let ok = process::run_with_timeout(init, &self.spec.log, Duration::from_secs(180))?;
                if !ok || !marker.exists() {
                    bail!("inisialisasi gagal, lihat {}", self.spec.log.display());
                }
            }
        }
        if let Some(port) = self.spec.port {
            if TcpListener::bind(("127.0.0.1", port)).is_err() {
                bail!("port {port} sudah dipakai proses lain");
            }
        }
        let process = Process::spawn(&self.spec.command, &self.spec.log)?;
        tracing::info!(service = %self.spec.name, pid = process.pid(), "start");
        self.process = Some(process);
        self.started_at = Some(Instant::now());
        Ok(())
    }

    fn stop(&mut self) {
        self.wanted = false;
        self.next_retry = None;
        let Some(mut process) = self.process.take() else {
            return;
        };
        if let Some(stop) = &self.spec.stop {
            let _ = process::run_with_timeout(stop, &self.spec.log, STOP_TIMEOUT);
            let deadline = Instant::now() + STOP_TIMEOUT;
            while Instant::now() < deadline {
                if matches!(process.child.try_wait(), Ok(Some(_))) {
                    break;
                }
                std::thread::sleep(Duration::from_millis(200));
            }
        }
        tracing::info!(service = %self.spec.name, "stop");
        process.kill();
        self.started_at = None;
    }

    /// Cek proses yang keluar sendiri dan jadwalkan restart dengan backoff.
    fn tick(&mut self) {
        if let Some(process) = &mut self.process {
            let Ok(Some(status)) = process.child.try_wait() else {
                return;
            };
            let uptime = self.started_at.map(|t| t.elapsed()).unwrap_or_default();
            tracing::warn!(service = %self.spec.name, ?status, "berhenti");
            if let Some(process) = self.process.take() {
                process.kill();
            }
            self.started_at = None;
            if !self.wanted {
                return;
            }
            self.failures = if uptime < MIN_HEALTHY {
                self.failures + 1
            } else {
                0
            };
            if self.failures >= MAX_FAILURES {
                self.wanted = false;
                self.error = Some(format!(
                    "berhenti {MAX_FAILURES}x berturut-turut, lihat {}",
                    self.spec.log.display()
                ));
                return;
            }
            self.restarts += 1;
            self.next_retry = Some(Instant::now() + backoff(self.failures));
        } else if self.wanted && self.next_retry.is_some_and(|t| Instant::now() >= t) {
            self.next_retry = None;
            if let Err(err) = self.try_start() {
                // Gagal saat restart otomatis: coba lagi dengan backoff sampai batas.
                self.failures += 1;
                self.error = Some(format!("{err:#}"));
                if self.failures >= MAX_FAILURES {
                    self.wanted = false;
                } else {
                    self.next_retry = Some(Instant::now() + backoff(self.failures));
                }
            }
        }
    }

    fn status(&self) -> ServiceStatus {
        let state = match (&self.process, self.wanted, &self.error) {
            (Some(_), _, _) => "running",
            (None, true, _) => "restarting",
            (None, false, Some(_)) => "failed",
            (None, false, None) => "stopped",
        };
        ServiceStatus {
            name: self.spec.name.clone(),
            group: self.spec.group.clone(),
            state: state.into(),
            pid: self.process.as_ref().map(Process::pid),
            port: self.spec.port,
            uptime_secs: self.started_at.map(|t| t.elapsed().as_secs()),
            restarts: self.restarts,
            error: self.error.clone(),
            log: self.spec.log.display().to_string(),
        }
    }
}

pub struct Supervisor {
    paths: Paths,
    services: BTreeMap<String, Managed>,
    /// Urutan start: php-cgi & database dulu, nginx & dev server setelahnya.
    order: Vec<String>,
    warnings: Vec<String>,
}

fn matches_target(target: &str, spec: &ServiceSpec) -> bool {
    let target = target.trim();
    target == "all"
        || spec.name == target
        || spec.group == target
        || spec.group.starts_with(&format!("{target}@"))
        || (target == "dev" && spec.group.starts_with("dev:"))
        || (matches!(target, "db" | "database")
            && (spec.group.starts_with("mysql@") || spec.group.starts_with("mariadb@")))
}

impl Supervisor {
    pub fn new(paths: Paths) -> Self {
        Self {
            paths,
            services: BTreeMap::new(),
            order: Vec::new(),
            warnings: Vec::new(),
        }
    }

    /// Bangun ulang rencana service; service yang konfigurasinya berubah di-restart.
    pub fn reload(&mut self) -> Result<()> {
        let config = Config::load(&self.paths)?;
        let plan = services::plan(&self.paths, &config)?;
        self.warnings = plan.warnings;

        let mut changed_files = false;
        for (path, content) in &plan.files {
            if std::fs::read_to_string(path).ok().as_deref() != Some(content.as_str()) {
                if let Some(dir) = path.parent() {
                    std::fs::create_dir_all(dir)?;
                }
                std::fs::write(path, content)
                    .with_context(|| format!("gagal menulis {}", path.display()))?;
                changed_files = true;
            }
        }

        let new_names: Vec<String> = plan.services.iter().map(|s| s.name.clone()).collect();
        let removed: Vec<String> = self
            .services
            .keys()
            .filter(|name| !new_names.contains(name))
            .cloned()
            .collect();
        for name in removed {
            if let Some(mut managed) = self.services.remove(&name) {
                managed.stop();
            }
        }

        for spec in plan.services {
            match self.services.get_mut(&spec.name) {
                Some(managed) => {
                    let restart = managed.spec != spec || (changed_files && spec.name == "nginx");
                    let was_wanted = managed.wanted;
                    if restart && managed.process.is_some() {
                        managed.stop();
                        managed.spec = spec;
                        managed.wanted = was_wanted;
                        managed.start();
                    } else {
                        managed.spec = spec;
                    }
                }
                None => {
                    self.services.insert(spec.name.clone(), Managed::new(spec));
                }
            }
        }
        self.order = new_names;
        Ok(())
    }

    pub fn start(&mut self, target: &str) -> Vec<String> {
        let mut started = Vec::new();
        for name in &self.order {
            let Some(managed) = self.services.get_mut(name) else {
                continue;
            };
            if matches_target(target, &managed.spec) {
                managed.wanted = true;
                managed.failures = 0;
                managed.start();
                started.push(name.clone());
            }
        }
        started
    }

    pub fn stop(&mut self, target: &str) -> Vec<String> {
        let mut stopped = Vec::new();
        for name in self.order.iter().rev() {
            let Some(managed) = self.services.get_mut(name) else {
                continue;
            };
            if matches_target(target, &managed.spec) {
                managed.stop();
                managed.error = None;
                stopped.push(name.clone());
            }
        }
        stopped
    }

    pub fn tick(&mut self) {
        for managed in self.services.values_mut() {
            managed.tick();
        }
    }

    pub fn status(&self) -> Vec<ServiceStatus> {
        self.order
            .iter()
            .filter_map(|name| self.services.get(name))
            .map(Managed::status)
            .collect()
    }

    pub fn handle(&mut self, request: Request) -> Response {
        let result: Result<String> = match request {
            Request::Status => Ok(String::new()),
            Request::Reload => self.reload().map(|_| "konfigurasi dimuat ulang".into()),
            Request::Start { target } => self.reload().and_then(|_| {
                let started = self.start(&target);
                if started.is_empty() {
                    bail!("tidak ada service yang cocok dengan `{target}`");
                }
                Ok(format!("start: {}", started.join(", ")))
            }),
            Request::Stop { target } => {
                let stopped = self.stop(&target);
                if stopped.is_empty() {
                    Err(anyhow::anyhow!(
                        "tidak ada service yang cocok dengan `{target}`"
                    ))
                } else {
                    Ok(format!("stop: {}", stopped.join(", ")))
                }
            }
            Request::Restart { target } => {
                self.stop(&target);
                self.reload()
                    .map(|_| format!("restart: {}", self.start(&target).join(", ")))
            }
            Request::Shutdown => {
                self.stop("all");
                Ok("daemon berhenti".into())
            }
        };
        let (ok, message) = match result {
            Ok(message) => (true, message),
            Err(err) => (false, format!("{err:#}")),
        };
        Response {
            ok,
            message,
            services: self.status(),
            warnings: self.warnings.clone(),
        }
    }
}

/// Jalankan daemon (blocking) sampai menerima `Shutdown`.
pub fn serve(paths: Paths) -> Result<()> {
    let config = Config::load(&paths)?;
    let listener = TcpListener::bind(("127.0.0.1", config.daemon.port)).with_context(|| {
        format!(
            "port daemon {} dipakai; daemon sudah berjalan?",
            config.daemon.port
        )
    })?;
    std::fs::create_dir_all(paths.run())?;
    std::fs::write(
        paths.run().join("daemon.pid"),
        std::process::id().to_string(),
    )?;

    let supervisor = Arc::new(Mutex::new(Supervisor::new(paths.clone())));
    if let Err(err) = supervisor.lock().unwrap().reload() {
        tracing::error!("{err:#}");
    }

    let ticker = Arc::clone(&supervisor);
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_millis(500));
        ticker.lock().unwrap().tick();
    });

    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        let shutdown = handle_client(stream, &supervisor).unwrap_or_else(|err| {
            tracing::warn!("koneksi client: {err:#}");
            false
        });
        if shutdown {
            break;
        }
    }
    let _ = std::fs::remove_file(paths.run().join("daemon.pid"));
    Ok(())
}

fn handle_client(stream: TcpStream, supervisor: &Mutex<Supervisor>) -> Result<bool> {
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let request: Request = serde_json::from_str(line.trim()).context("request tidak valid")?;
    let shutdown = matches!(request, Request::Shutdown);
    let response = supervisor.lock().unwrap().handle(request);
    let mut stream = stream;
    serde_json::to_writer(&mut stream, &response)?;
    stream.write_all(b"\n")?;
    Ok(shutdown)
}

/// Kirim satu request ke daemon. `None` bila daemon tidak berjalan.
pub fn send(config: &Config, request: &Request) -> Result<Option<Response>> {
    let Ok(mut stream) = TcpStream::connect_timeout(
        &([127, 0, 0, 1], config.daemon.port).into(),
        Duration::from_millis(500),
    ) else {
        return Ok(None);
    };
    // Start/stop database bisa butuh waktu (inisialisasi data dir).
    stream.set_read_timeout(Some(Duration::from_secs(300)))?;
    serde_json::to_writer(&mut stream, request)?;
    stream.write_all(b"\n")?;
    let mut line = String::new();
    BufReader::new(stream).read_line(&mut line)?;
    let response = serde_json::from_str(line.trim()).context("respons daemon tidak valid")?;
    Ok(Some(response))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::Command;

    fn spec(name: &str, group: &str) -> ServiceSpec {
        ServiceSpec {
            name: name.into(),
            group: group.into(),
            command: Command {
                program: "x".into(),
                args: vec![],
                cwd: ".".into(),
                env: vec![],
            },
            stop: None,
            init: None,
            init_marker: None,
            port: None,
            log: "x.log".into(),
        }
    }

    #[test]
    fn target_matching() {
        let php = spec("php@8.3#0", "php@8.3");
        assert!(matches_target("all", &php));
        assert!(matches_target("php", &php));
        assert!(matches_target("php@8.3", &php));
        assert!(!matches_target("php@8.2", &php));
        let db = spec("mysql@8.4", "mysql@8.4");
        assert!(matches_target("db", &db));
        assert!(matches_target("mysql", &db));
        assert!(!matches_target("nginx", &db));
        assert!(matches_target("dev", &spec("dev:dash", "dev:dash")));
    }

    #[test]
    fn request_wire_format() {
        let json = serde_json::to_string(&Request::Start {
            target: "all".into(),
        })
        .unwrap();
        assert_eq!(json, r#"{"cmd":"start","target":"all"}"#);
    }
}
