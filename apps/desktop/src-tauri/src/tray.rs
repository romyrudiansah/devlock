//! Ikon tray: status service, start/stop semua, menu cepat per project, buka folder www, keluar.
//! Menu dibangun ulang bila status service atau daftar site berubah (dicek tiap 5 detik,
//! dan langsung setelah aksi dari tray).

use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::Duration;

use devlock_core::daemon::{self, Request};
use devlock_core::{launcher, sites, Config, Paths};
use tauri::menu::{IsMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Listener, Wry};

const REFRESH_EVERY: Duration = Duration::from_secs(5);

/// Isi menu yang berubah-ubah; menu hanya dibangun ulang bila nilainya berbeda.
#[derive(PartialEq)]
struct Snapshot {
    /// Menu dalam bahasa Inggris (`[gui] language = "en"`).
    english: bool,
    status: String,
    /// (nama site, URL)
    sites: Vec<(String, String)>,
}

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let english = load().is_ok_and(|(_, config)| config.gui.english());
    let initial = Snapshot {
        english,
        status: tr(english, "Service: memuat…").into(),
        sites: Vec::new(),
    };
    let mut builder = TrayIconBuilder::with_id("main")
        .tooltip("DevLock")
        .menu(&build_menu(app, &initial)?)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => crate::show_main(app),
            "start" => run_in_background(app, "start"),
            "stop" => run_in_background(app, "stop"),
            "www" => {
                if let Ok((paths, config)) = load() {
                    let _ = tauri_plugin_opener::open_path(config.www_dir(&paths), None::<&str>);
                }
            }
            "quit" => app.exit(0),
            "quit-stop" => {
                if let Ok((_, config)) = load() {
                    let _ = daemon::send(&config, &Request::Shutdown);
                }
                app.exit(0);
            }
            id => {
                if let Some((action, site)) = id.split_once(':') {
                    let (action, site) = (action.to_string(), site.to_string());
                    std::thread::spawn(move || site_action(&action, &site));
                }
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                crate::show_main(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    spawn_refresher(app.clone());
    Ok(())
}

/// Label menu tray dalam bahasa terpilih; teks Indonesia adalah kuncinya.
fn tr(english: bool, text: &'static str) -> &'static str {
    if !english {
        return text;
    }
    match text {
        "Service: memuat…" => "Services: loading…",
        "Buka folder" => "Open folder",
        "Buka di editor" => "Open in editor",
        "Terminal di folder ini" => "Terminal in this folder",
        "Project (belum ada)" => "Projects (none yet)",
        "Project" => "Projects",
        "Buka DevLock" => "Open DevLock",
        "Start semua" => "Start all",
        "Stop semua" => "Stop all",
        "Buka folder www" => "Open www folder",
        "Keluar (service tetap jalan)" => "Quit (services keep running)",
        "Stop semua & keluar" => "Stop all & quit",
        "Konfigurasi DevLock tidak terbaca" => "DevLock configuration is unreadable",
        "Service: belum ada yang berjalan" => "Services: none running",
        "Service: daemon tidak berjalan" => "Services: daemon not running",
        "Service: status tidak terbaca" => "Services: status unavailable",
        other => other,
    }
}

fn build_menu(app: &AppHandle, snap: &Snapshot) -> tauri::Result<Menu<Wry>> {
    let en = snap.english;
    let item = |id: &str, label: &str| MenuItem::with_id(app, id, label, true, None::<&str>);

    let mut projects = Vec::new();
    for (name, url) in &snap.sites {
        projects.push(Submenu::with_items(
            app,
            name,
            true,
            &[
                &item(
                    &format!("site-open:{name}"),
                    &format!("{} {url}", if en { "Open" } else { "Buka" }),
                )?,
                &item(&format!("site-dir:{name}"), tr(en, "Buka folder"))?,
                &item(&format!("site-editor:{name}"), tr(en, "Buka di editor"))?,
                &item(
                    &format!("site-term:{name}"),
                    tr(en, "Terminal di folder ini"),
                )?,
            ],
        )?);
    }
    let project_refs: Vec<&dyn IsMenuItem<Wry>> =
        projects.iter().map(|s| s as &dyn IsMenuItem<Wry>).collect();
    let projects_menu = if project_refs.is_empty() {
        Submenu::with_items(app, tr(en, "Project (belum ada)"), false, &[])?
    } else {
        Submenu::with_items(app, tr(en, "Project"), true, &project_refs)?
    };

    Menu::with_items(
        app,
        &[
            &item("open", tr(en, "Buka DevLock"))?,
            &MenuItem::with_id(app, "status", &snap.status, false, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &item("start", tr(en, "Start semua"))?,
            &item("stop", tr(en, "Stop semua"))?,
            &projects_menu,
            &item("www", tr(en, "Buka folder www"))?,
            &PredefinedMenuItem::separator(app)?,
            &item("quit", tr(en, "Keluar (service tetap jalan)"))?,
            &item("quit-stop", tr(en, "Stop semua & keluar"))?,
        ],
    )
}

fn snapshot() -> Snapshot {
    let Ok((paths, config)) = load() else {
        return Snapshot {
            english: false,
            status: "Konfigurasi DevLock tidak terbaca".into(),
            sites: Vec::new(),
        };
    };
    let en = config.gui.english();
    let status = match daemon::send(&config, &Request::Status) {
        Ok(Some(r)) if r.services.is_empty() => tr(en, "Service: belum ada yang berjalan").into(),
        Ok(Some(r)) => {
            let running = r.services.iter().filter(|s| s.state == "running").count();
            if en {
                format!("Services: {running}/{} running", r.services.len())
            } else {
                format!("Service: {running}/{} berjalan", r.services.len())
            }
        }
        Ok(None) => tr(en, "Service: daemon tidak berjalan").into(),
        Err(_) => tr(en, "Service: status tidak terbaca").into(),
    };
    let sites = sites::discover(&paths, &config)
        .map(|sites| {
            sites
                .iter()
                .map(|s| (s.name.clone(), s.url(&config)))
                .collect()
        })
        .unwrap_or_default();
    Snapshot {
        english: en,
        status,
        sites,
    }
}

/// Bangun ulang menu tray bila isinya berubah: tiap `REFRESH_EVERY`, atau segera setelah
/// event `services-changed`.
fn spawn_refresher(app: AppHandle) {
    let (tx, rx) = mpsc::channel::<()>();
    app.listen_any("services-changed", move |_| {
        let _ = tx.send(());
    });
    std::thread::spawn(move || {
        let mut last: Option<Snapshot> = None;
        loop {
            let snap = snapshot();
            if last.as_ref() != Some(&snap) {
                if let (Some(tray), Ok(menu)) = (app.tray_by_id("main"), build_menu(&app, &snap)) {
                    let _ = tray.set_menu(Some(menu));
                    let _ = tray.set_tooltip(Some(format!("DevLock — {}", snap.status)));
                }
                last = Some(snap);
            }
            if let Err(RecvTimeoutError::Disconnected) = rx.recv_timeout(REFRESH_EVERY) {
                break;
            }
        }
    });
}

fn site_action(action: &str, name: &str) {
    let Ok((paths, config)) = load() else { return };
    let Ok(sites) = sites::discover(&paths, &config) else {
        return;
    };
    let Some(site) = sites.iter().find(|s| s.name == name) else {
        return;
    };
    match action {
        "site-open" => {
            let _ = tauri_plugin_opener::open_url(site.url(&config), None::<&str>);
        }
        "site-dir" => {
            let _ = tauri_plugin_opener::open_path(&site.dir, None::<&str>);
        }
        "site-editor" => {
            let _ = crate::commands::open_editor(site.dir.display().to_string());
        }
        "site-term" => {
            let _ = crate::commands::open_terminal(site.dir.display().to_string());
        }
        _ => {}
    }
}

fn load() -> anyhow::Result<(Paths, Config)> {
    let paths = Paths::discover()?;
    let config = Config::load(&paths)?;
    Ok((paths, config))
}

/// Start/stop dari tray tanpa memblokir thread UI; dashboard diberi tahu lewat event.
pub(crate) fn run_in_background(app: &AppHandle, action: &'static str) {
    let app = app.clone();
    std::thread::spawn(move || {
        let result = (|| -> anyhow::Result<()> {
            let (paths, config) = load()?;
            if action == "start" {
                let _ = launcher::sync_hosts(&paths, &config, false);
                launcher::ensure_daemon(&paths, &config)?;
            }
            let request = match action {
                "start" => Request::Start {
                    target: "all".into(),
                },
                _ => Request::Stop {
                    target: "all".into(),
                },
            };
            daemon::send(&config, &request)?;
            Ok(())
        })();
        let _ = app.emit("services-changed", result.err().map(|e| format!("{e:#}")));
    });
}
