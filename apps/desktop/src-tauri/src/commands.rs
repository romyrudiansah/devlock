//! Perintah yang dipanggil frontend. Semua kerja berat (daemon, download, scan folder)
//! dijalankan di thread blocking agar UI tetap responsif.

use std::path::PathBuf;

use devlock_core::catalog::{self, Release};
use devlock_core::daemon::{self, Request, Response, ServiceStatus};
use devlock_core::install::{self, Event};
use devlock_core::launcher::{self, HostsSync};
use devlock_core::sites::{self, SiteKind};
use devlock_core::{db, runtime, services, version, Config, Kind, Paths};
use serde::Serialize;
use tauri::{AppHandle, Emitter};

pub(crate) type CmdResult<T> = Result<T, String>;

pub(crate) fn err(e: anyhow::Error) -> String {
    format!("{e:#}")
}

pub(crate) fn context() -> CmdResult<(Paths, Config)> {
    let paths = Paths::discover().map_err(err)?;
    let config = Config::load(&paths).map_err(err)?;
    Ok((paths, config))
}

pub(crate) async fn blocking<T, F>(f: F) -> CmdResult<T>
where
    T: Send + 'static,
    F: FnOnce() -> CmdResult<T> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| e.to_string())?
}

#[derive(Serialize)]
pub struct Overview {
    root: String,
    www: String,
    tld: String,
    http_port: u16,
    daemon_running: bool,
    services: Vec<ServiceStatus>,
    warnings: Vec<String>,
    /// Pesan dari aksi terakhir (start/stop/...).
    message: Option<String>,
    ok: bool,
}

fn overview_from(
    paths: &Paths,
    config: &Config,
    response: Option<Response>,
    extra: Vec<String>,
) -> Overview {
    let (daemon_running, services, mut warnings, message, ok) = match response {
        Some(r) => (
            true,
            r.services,
            r.warnings,
            Some(r.message).filter(|m| !m.is_empty()),
            r.ok,
        ),
        None => (false, Vec::new(), Vec::new(), None, true),
    };
    warnings.extend(extra);
    Overview {
        root: paths.root().display().to_string(),
        www: config.www_dir(paths).display().to_string(),
        tld: config.tld.clone(),
        http_port: config.http.port,
        daemon_running,
        services,
        warnings,
        message,
        ok,
    }
}

#[tauri::command]
pub async fn overview() -> CmdResult<Overview> {
    blocking(|| {
        let (paths, config) = context()?;
        let response = daemon::send(&config, &Request::Status).map_err(err)?;
        Ok(overview_from(&paths, &config, response, Vec::new()))
    })
    .await
}

/// `action`: start, stop, restart, reload, shutdown. `target`: all, nginx, php@8.3, ...
#[tauri::command]
pub async fn service_action(
    action: String,
    target: String,
    update_hosts: bool,
) -> CmdResult<Overview> {
    blocking(move || {
        let (paths, config) = context()?;
        let mut extra = Vec::new();
        if update_hosts && matches!(action.as_str(), "start" | "reload") {
            match launcher::sync_hosts(&paths, &config, false) {
                Ok(HostsSync::Updated { domains }) => {
                    extra.push(format!("File hosts diperbarui ({domains} domain)."))
                }
                Ok(HostsSync::Unchanged) => {}
                Err(e) => extra.push(format!("File hosts tidak diperbarui: {e:#}")),
            }
        }
        let request = match action.as_str() {
            "start" => Request::Start { target },
            "stop" => Request::Stop { target },
            "restart" => Request::Restart { target },
            "reload" => Request::Reload,
            "shutdown" => Request::Shutdown,
            other => return Err(format!("aksi `{other}` tidak dikenal")),
        };
        if matches!(request, Request::Start { .. } | Request::Restart { .. }) {
            launcher::ensure_daemon(&paths, &config).map_err(err)?;
        }
        let response = daemon::send(&config, &request).map_err(err)?;
        // Setelah shutdown daemon tidak lagi berjalan; tampilkan pesannya saja.
        let mut overview = overview_from(&paths, &config, response, extra);
        if matches!(request, Request::Shutdown) {
            overview.daemon_running = false;
            overview.services.clear();
        }
        Ok(overview)
    })
    .await
}

#[derive(Serialize)]
pub struct SiteInfo {
    name: String,
    url: String,
    domains: Vec<String>,
    kind: String,
    detail: String,
    database: Option<String>,
    dir: String,
    docroot: String,
    warning: Option<String>,
}

/// Buat database bernama sesuai site (`my-shop` → `my_shop`) di instance database site itu.
#[tauri::command]
pub async fn create_site_database(site: String) -> CmdResult<String> {
    blocking(move || {
        let (paths, config) = context()?;
        let sites = sites::discover(&paths, &config).map_err(err)?;
        let site = sites
            .iter()
            .find(|s| s.name == site)
            .ok_or_else(|| format!("site `{site}` tidak ditemukan"))?;
        let instance = services::site_database(&config, site).ok_or_else(|| {
            format!(
                "{} belum punya database; set `database` di .devlock.toml",
                site.domain()
            )
        })?;
        let name = db::name_for_site(&site.name);
        let created = db::create(&paths, &config, &instance, &name).map_err(err)?;
        Ok(if created {
            format!(
                "Database `{name}` dibuat di {} (port {}).",
                instance.name(),
                instance.port(&config)
            )
        } else {
            format!("Database `{name}` sudah ada di {}.", instance.name())
        })
    })
    .await
}

#[tauri::command]
pub async fn list_sites() -> CmdResult<Vec<SiteInfo>> {
    blocking(|| {
        let (paths, config) = context()?;
        let sites = sites::discover(&paths, &config).map_err(err)?;
        Ok(sites
            .iter()
            .map(|site| {
                let (kind, detail) = match &site.kind {
                    SiteKind::Php { line } => ("php", format!("PHP {line}")),
                    SiteKind::Proxy { port } => ("proxy", format!("dev server :{port}")),
                    SiteKind::Static => ("static", "Static".to_string()),
                };
                let detail = if site.apache {
                    format!("{detail} · Apache")
                } else {
                    detail
                };
                SiteInfo {
                    name: site.name.clone(),
                    url: site.url(&config),
                    domains: site.domains.clone(),
                    kind: kind.into(),
                    detail,
                    database: services::site_database(&config, site).map(|db| db.name()),
                    dir: site.dir.display().to_string(),
                    docroot: site.docroot.display().to_string(),
                    warning: site.warning.clone(),
                }
            })
            .collect())
    })
    .await
}

#[derive(Serialize)]
pub struct RuntimeInfo {
    kind: String,
    installed: Vec<String>,
    /// Spec default di devlock.toml, mis. `8.3`.
    default_spec: Option<String>,
    /// Versi terinstall yang dipakai sebagai default.
    default_version: Option<String>,
}

#[tauri::command]
pub async fn list_runtimes() -> CmdResult<Vec<RuntimeInfo>> {
    blocking(|| {
        let (paths, config) = context()?;
        Kind::ALL
            .iter()
            .map(|&kind| {
                let installed = runtime::installed_versions(&paths, kind).map_err(err)?;
                let default_spec = config.defaults.get(kind.as_str()).cloned();
                let default_version = default_spec
                    .as_deref()
                    .and_then(|spec| version::pick(spec, installed.iter().map(String::as_str)))
                    .map(str::to_string);
                Ok(RuntimeInfo {
                    kind: kind.to_string(),
                    installed,
                    default_spec,
                    default_version,
                })
            })
            .collect()
    })
    .await
}

#[derive(Serialize)]
pub struct AvailableRelease {
    version: String,
    lts: bool,
}

#[tauri::command]
pub async fn available_runtimes(kind: String) -> CmdResult<Vec<AvailableRelease>> {
    blocking(move || {
        let kind: Kind = kind.parse().map_err(err)?;
        let (paths, _) = context()?;
        let releases = catalog::available_cached(&paths, kind, false).map_err(err)?;
        Ok(catalog::latest_per_line(kind, &releases)
            .into_iter()
            .map(|r: &Release| AvailableRelease {
                version: r.version.clone(),
                lts: r.lts,
            })
            .collect())
    })
    .await
}

#[derive(Clone, Serialize)]
struct InstallProgress {
    kind: String,
    version: String,
    stage: String,
    downloaded: u64,
    total: Option<u64>,
}

#[tauri::command]
pub async fn install_runtime(app: AppHandle, kind: String, version: String) -> CmdResult<String> {
    blocking(move || {
        let (paths, config) = context()?;
        paths.ensure_layout().map_err(err)?;
        let kind: Kind = kind.parse().map_err(err)?;
        let release = catalog::find_cached(&paths, kind, &version).map_err(err)?;
        let emit = |stage: &str, downloaded: u64, total: Option<u64>| {
            let _ = app.emit(
                "install-progress",
                InstallProgress {
                    kind: kind.to_string(),
                    version: release.version.clone(),
                    stage: stage.into(),
                    downloaded,
                    total,
                },
            );
        };
        install::install(&paths, &release, &mut |event| match event {
            Event::Downloading { downloaded, total } => emit("download", downloaded, total),
            Event::Verifying => emit("verifikasi", 0, None),
            Event::Unverified => emit("tanpa checksum", 0, None),
            Event::Extracting => emit("ekstrak", 0, None),
            Event::Configuring(what) => emit(&format!("konfigurasi {what}"), 0, None),
        })
        .map_err(err)?;
        emit("selesai", 0, None);
        // Sama seperti CLI: install pertama suatu runtime menjadi default.
        if !config.defaults.contains_key(kind.as_str()) && kind.has_default_version() {
            Config::set_default(&paths, kind.as_str(), &release.version).map_err(err)?;
        }
        Ok(release.version)
    })
    .await
}

#[tauri::command]
pub async fn uninstall_runtime(kind: String, version: String) -> CmdResult<()> {
    blocking(move || {
        let (paths, _) = context()?;
        let kind: Kind = kind.parse().map_err(err)?;
        runtime::uninstall(&paths, kind, &version).map_err(err)?;
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn set_default_runtime(kind: String, version: String) -> CmdResult<()> {
    blocking(move || {
        let (paths, _) = context()?;
        let kind: Kind = kind.parse().map_err(err)?;
        Config::set_default(&paths, kind.as_str(), &version).map_err(err)
    })
    .await
}

/// Baris terakhir log sebuah service (nama atau grup) atau `daemon`.
#[tauri::command]
pub async fn read_log(service: String, lines: usize) -> CmdResult<String> {
    blocking(move || {
        let (paths, config) = context()?;
        let log: PathBuf = if service == "daemon" {
            paths.logs().join("daemon.log")
        } else {
            let plan = services::plan(&paths, &config).map_err(err)?;
            plan.services
                .iter()
                .find(|s| s.name == service || s.group == service)
                .map(|s| s.log.clone())
                .ok_or_else(|| format!("service `{service}` tidak ditemukan"))?
        };
        let text = std::fs::read_to_string(&log).unwrap_or_default();
        let all: Vec<&str> = text.lines().collect();
        Ok(all[all.len().saturating_sub(lines)..].join("\n"))
    })
    .await
}

#[tauri::command]
pub fn open_url(url: String) -> CmdResult<()> {
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err("hanya URL http/https".into());
    }
    tauri_plugin_opener::open_url(url, None::<&str>).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn open_folder(path: String) -> CmdResult<()> {
    tauri_plugin_opener::open_path(path, None::<&str>).map_err(|e| e.to_string())
}

/// Buka folder project di editor (`[gui] editor` atau deteksi otomatis VS Code, Cursor, …).
#[tauri::command]
pub fn open_editor(dir: String) -> CmdResult<()> {
    let (_, config) = context()?;
    let Some(editor) = devlock_core::editor::find(&config.gui.editor) else {
        return Err(if config.gui.editor.trim().is_empty() {
            "editor tidak ditemukan (VS Code, Cursor, Windsurf, Zed, Sublime Text, PhpStorm); isi `[gui] editor` di Settings"
                .into()
        } else {
            format!("editor `{}` tidak ditemukan", config.gui.editor.trim())
        });
    };
    let mut command = std::process::Command::new(&editor);
    command.arg(&dir).current_dir(&dir);
    {
        use std::os::windows::process::CommandExt;
        // `code.cmd` dkk. dijalankan lewat cmd.exe; jangan munculkan jendela console.
        command.creation_flags(devlock_core::process::CREATE_NO_WINDOW);
    }
    command
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("gagal membuka {}: {e}", editor.display()))
}

/// Buka terminal di folder project (Windows Terminal bila ada, selain itu PowerShell).
/// Folder shims ditaruh di depan PATH agar `php`/`node` langsung memakai versi project.
#[tauri::command]
pub fn open_terminal(dir: String) -> CmdResult<()> {
    let (paths, _) = context()?;
    let shims = paths.shims();
    let path = std::env::var_os("PATH").unwrap_or_default();
    let new_path = std::env::join_paths(std::iter::once(shims).chain(std::env::split_paths(&path)))
        .map_err(|e| e.to_string())?;
    let spawn = |program: &str, args: &[&str]| {
        std::process::Command::new(program)
            .args(args)
            .current_dir(&dir)
            .env("PATH", &new_path)
            .spawn()
    };
    spawn("wt.exe", &["-d", &dir])
        .or_else(|_| spawn("cmd.exe", &["/c", "start", "powershell.exe", "-NoExit"]))
        .map(|_| ())
        .map_err(|e| format!("gagal membuka terminal: {e}"))
}
