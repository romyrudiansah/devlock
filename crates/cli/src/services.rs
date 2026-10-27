use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result};
use devlock_core::daemon::{self, Request, Response};
use devlock_core::launcher::{self, HostsSync};
use devlock_core::sites::{self, SiteKind};
use devlock_core::{services, Config, Paths};

pub fn start(target: &str, update_hosts: bool) -> Result<()> {
    let (paths, config) = load()?;
    if update_hosts {
        try_sync_hosts(&paths, &config);
    }
    launcher::ensure_daemon(&paths, &config)?;
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
    launcher::ensure_daemon(&paths, &config)?;
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
    try_sync_hosts(&paths, &config);
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
            "{:<36} {:<12} db {:<14} {}",
            site.url(&config),
            kind,
            db,
            site.docroot.display()
        );
        for alias in &site.domains[1..] {
            println!("  alias {}", sites::url_for(alias, site.https, &config));
        }
        if let Some(warning) = &site.warning {
            println!("  ! {warning}");
        }
    }
    Ok(())
}

pub fn hosts(elevated: bool) -> Result<()> {
    let (paths, config) = load()?;
    match launcher::sync_hosts(&paths, &config, elevated)? {
        HostsSync::Unchanged => println!("File hosts sudah sesuai."),
        HostsSync::Updated { domains } => println!("File hosts diperbarui ({domains} domain)."),
    }
    Ok(())
}

/// Seperti `hosts`, tapi kegagalan hanya jadi peringatan (dipakai `start` / `reload`).
fn try_sync_hosts(paths: &Paths, config: &Config) {
    match launcher::sync_hosts(paths, config, false) {
        Ok(HostsSync::Updated { domains }) => println!("File hosts diperbarui ({domains} domain)."),
        Ok(HostsSync::Unchanged) => {}
        Err(err) => println!("peringatan: file hosts tidak diperbarui ({err:#})"),
    }
}

fn load() -> Result<(Paths, Config)> {
    let paths = Paths::discover()?;
    let config = Config::load(&paths)?;
    Ok((paths, config))
}

fn request(config: &Config, request: Request) -> Result<Response> {
    daemon::send(config, &request)?.context("daemon tidak merespons")
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
