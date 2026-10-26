use std::fs::OpenOptions;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use devlock_core::daemon::{self, Request, Response};
use devlock_core::sites::{self, SiteKind};
use devlock_core::{hosts, process, services, Config, Paths};

pub fn start(target: &str, update_hosts: bool) -> Result<()> {
    let (paths, config) = load()?;
    if update_hosts {
        if let Err(err) = sync_hosts(&paths, &config, false) {
            println!("peringatan: file hosts tidak diperbarui ({err:#})");
        }
    }
    ensure_daemon(&paths, &config)?;
    let response = request(
        &config,
        Request::Start {
            target: target.into(),
        },
    )?;
    print_response(&response);
    Ok(())
}

pub fn stop(target: Option<&str>) -> Result<()> {
    let (_, config) = load()?;
    let request = match target {
        Some(target) => Request::Stop {
            target: target.into(),
        },
        None => Request::Shutdown,
    };
    match daemon::send(&config, &request)? {
        Some(response) => print_response(&response),
        None => println!("Daemon tidak berjalan."),
    }
    Ok(())
}

pub fn restart(target: &str) -> Result<()> {
    let (paths, config) = load()?;
    ensure_daemon(&paths, &config)?;
    print_response(&request(
        &config,
        Request::Restart {
            target: target.into(),
        },
    )?);
    Ok(())
}

pub fn reload() -> Result<()> {
    let (paths, config) = load()?;
    if let Err(err) = sync_hosts(&paths, &config, false) {
        println!("peringatan: file hosts tidak diperbarui ({err:#})");
    }
    match daemon::send(&config, &Request::Reload)? {
        Some(response) => print_response(&response),
        None => println!("Daemon tidak berjalan; jalankan `devlock start`."),
    }
    Ok(())
}

pub fn status() -> Result<()> {
    let (_, config) = load()?;
    match daemon::send(&config, &Request::Status)? {
        Some(response) => print_response(&response),
        None => println!("Daemon tidak berjalan; jalankan `devlock start`."),
    }
    Ok(())
}

pub fn run_daemon() -> Result<()> {
    let paths = Paths::discover()?;
    tracing::info!(root = %paths.root().display(), "daemon start");
    daemon::serve(paths)
}

/// Tampilkan akhir log service; `follow` terus menampilkan baris baru.
pub fn logs(name: &str, lines: usize, follow: bool) -> Result<()> {
    let (paths, config) = load()?;
    let log = if name == "daemon" {
        paths.logs().join("daemon.log")
    } else {
        let plan = services::plan(&paths, &config)?;
        plan.services
            .iter()
            .find(|s| s.name == name || s.group == name)
            .map(|s| s.log.clone())
            .with_context(|| {
                let names: Vec<_> = plan.services.iter().map(|s| s.group.as_str()).collect();
                format!(
                    "service `{name}` tidak ada (tersedia: daemon, {})",
                    names.join(", ")
                )
            })?
    };
    print_tail(&log, lines, follow)
}

fn print_tail(log: &Path, lines: usize, follow: bool) -> Result<()> {
    let mut file =
        std::fs::File::open(log).with_context(|| format!("log {} belum ada", log.display()))?;
    let mut text = String::new();
    file.read_to_string(&mut text)?;
    let all: Vec<&str> = text.lines().collect();
    for line in &all[all.len().saturating_sub(lines)..] {
        println!("{line}");
    }
    if !follow {
        return Ok(());
    }
    let mut pos = file.stream_position()?;
    loop {
        std::thread::sleep(Duration::from_millis(500));
        let len = file.metadata()?.len();
        if len < pos {
            pos = 0;
        }
        if len > pos {
            file.seek(SeekFrom::Start(pos))?;
            let mut chunk = String::new();
            file.read_to_string(&mut chunk)?;
            print!("{chunk}");
            pos = len;
        }
    }
}

pub fn site_list() -> Result<()> {
    let (paths, config) = load()?;
    let sites = sites::discover(&paths, &config)?;
    if sites.is_empty() {
        println!("Belum ada project di {}", config.www_dir(&paths).display());
        return Ok(());
    }
    for site in &sites {
        let kind = match &site.kind {
            SiteKind::Php { line } => format!("php {line}"),
            SiteKind::Proxy { port } => format!("proxy :{port}"),
            SiteKind::Static => "static".into(),
        };
        let db = services::site_database(&config, site)
            .map(|db| db.name())
            .unwrap_or_else(|| "-".into());
        println!(
            "http://{:<28} {:<12} db {:<14} {}",
            site.domain(),
            kind,
            db,
            site.docroot.display()
        );
        for alias in &site.domains[1..] {
            println!("  alias http://{alias}");
        }
        if let Some(warning) = &site.warning {
            println!("  ! {warning}");
        }
    }
    Ok(())
}

pub fn hosts(elevated: bool) -> Result<()> {
    let (paths, config) = load()?;
    sync_hosts(&paths, &config, elevated)
}

/// Samakan blok DevLock di file hosts dengan daftar site. Minta UAC bila perlu.
fn sync_hosts(paths: &Paths, config: &Config, elevated: bool) -> Result<()> {
    let domains: Vec<String> = sites::discover(paths, config)?
        .into_iter()
        .flat_map(|s| s.domains)
        .collect();
    let file = hosts::hosts_file();
    let current = std::fs::read_to_string(&file)
        .with_context(|| format!("gagal membaca {}", file.display()))?;
    let wanted = hosts::apply(&current, &domains);
    if wanted == current {
        if elevated {
            println!("File hosts sudah sesuai.");
        }
        return Ok(());
    }
    match std::fs::write(&file, &wanted) {
        Ok(()) => {
            println!("File hosts diperbarui ({} domain).", domains.len());
            Ok(())
        }
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied && !elevated => {
            println!("Butuh izin administrator untuk mengubah file hosts (jendela UAC)...");
            run_elevated_hosts(paths)?;
            let after = std::fs::read_to_string(&file)?;
            if after != wanted {
                bail!("file hosts belum berubah (UAC ditolak?)");
            }
            println!("File hosts diperbarui ({} domain).", domains.len());
            Ok(())
        }
        Err(e) => Err(e).with_context(|| format!("gagal menulis {}", file.display())),
    }
}

fn run_elevated_hosts(paths: &Paths) -> Result<()> {
    let exe = std::env::current_exe()?;
    let escape = |p: &Path| p.to_string_lossy().replace('\'', "''");
    // Proses elevated tidak selalu mewarisi environment, jadi root dikirim sebagai argumen
    // (dibungkus kutip ganda agar path berspasi tetap satu argumen).
    let script = format!(
        "Start-Process -FilePath '{exe}' -ArgumentList @('hosts','--elevated','--root','\"{root}\"') -Verb RunAs -Wait -WindowStyle Hidden",
        exe = escape(&exe),
        root = escape(paths.root()),
    );
    let status = Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .status()
        .context("gagal menjalankan powershell")?;
    if !status.success() {
        bail!("elevasi dibatalkan");
    }
    Ok(())
}

fn load() -> Result<(Paths, Config)> {
    let paths = Paths::discover()?;
    let config = Config::load(&paths)?;
    Ok((paths, config))
}

fn request(config: &Config, request: Request) -> Result<Response> {
    daemon::send(config, &request)?.context("daemon tidak merespons")
}

/// Jalankan `devlock daemon` di latar bila belum berjalan.
fn ensure_daemon(paths: &Paths, config: &Config) -> Result<()> {
    if daemon::send(config, &Request::Status)?.is_some() {
        return Ok(());
    }
    paths.ensure_layout()?;
    let log = paths.logs().join("daemon.log");
    let spawn = |flags: u32| -> std::io::Result<std::process::Child> {
        let out = OpenOptions::new().create(true).append(true).open(&log)?;
        let err = out.try_clone()?;
        let mut command = Command::new(std::env::current_exe()?);
        command
            .arg("daemon")
            .env("DEVLOCK_HOME", paths.root())
            .current_dir(paths.root())
            .stdin(Stdio::null())
            .stdout(out)
            .stderr(err);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(flags);
        }
        command.spawn()
    };
    let base =
        process::DETACHED_PROCESS | process::CREATE_NEW_PROCESS_GROUP | process::CREATE_NO_WINDOW;
    // Keluar dari job object terminal (bila diizinkan) agar daemon tidak ikut mati saat terminal ditutup.
    spawn(base | process::CREATE_BREAKAWAY_FROM_JOB)
        .or_else(|_| spawn(base))
        .context("gagal menjalankan daemon")?;

    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(200));
        if daemon::send(config, &Request::Status)?.is_some() {
            return Ok(());
        }
    }
    bail!("daemon tidak merespons; lihat {}", log.display())
}

fn print_response(response: &Response) {
    if !response.message.is_empty() {
        let mark = if response.ok { "" } else { "error: " };
        println!("{mark}{}", response.message);
    }
    for warning in &response.warnings {
        println!("peringatan: {warning}");
    }
    if response.services.is_empty() {
        println!("Belum ada service. Install nginx/php dan buat folder project di www\\.");
        return;
    }
    println!(
        "\n{:<18} {:<11} {:>7} {:>6} {:>9} {:>8}",
        "SERVICE", "STATUS", "PID", "PORT", "UPTIME", "RESTART"
    );
    for s in &response.services {
        println!(
            "{:<18} {:<11} {:>7} {:>6} {:>9} {:>8}",
            s.name,
            s.state,
            s.pid.map_or("-".into(), |p| p.to_string()),
            s.port.map_or("-".into(), |p| p.to_string()),
            s.uptime_secs.map_or("-".into(), format_uptime),
            s.restarts,
        );
        if let Some(error) = &s.error {
            println!("  ! {error}");
        }
    }
}

fn format_uptime(secs: u64) -> String {
    match secs {
        s if s < 60 => format!("{s}s"),
        s if s < 3600 => format!("{}m{}s", s / 60, s % 60),
        s => format!("{}h{}m", s / 3600, (s % 3600) / 60),
    }
}
