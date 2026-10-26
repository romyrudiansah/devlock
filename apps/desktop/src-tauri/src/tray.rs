//! Ikon tray: buka dashboard, start/stop semua service, buka folder www, keluar.

use devlock_core::daemon::{self, Request};
use devlock_core::{launcher, Config, Paths};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter};

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let item = |id: &str, label: &str| MenuItem::with_id(app, id, label, true, None::<&str>);
    let menu = Menu::with_items(
        app,
        &[
            &item("open", "Buka DevLock")?,
            &PredefinedMenuItem::separator(app)?,
            &item("start", "Start semua")?,
            &item("stop", "Stop semua")?,
            &item("www", "Buka folder www")?,
            &PredefinedMenuItem::separator(app)?,
            &item("quit", "Keluar (service tetap jalan)")?,
            &item("quit-stop", "Stop semua & keluar")?,
        ],
    )?;

    let mut builder = TrayIconBuilder::with_id("main")
        .tooltip("DevLock")
        .menu(&menu)
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
            _ => {}
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
    Ok(())
}

fn load() -> anyhow::Result<(Paths, Config)> {
    let paths = Paths::discover()?;
    let config = Config::load(&paths)?;
    Ok((paths, config))
}

/// Start/stop dari tray tanpa memblokir thread UI; dashboard diberi tahu lewat event.
fn run_in_background(app: &AppHandle, action: &'static str) {
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
