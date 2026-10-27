// Tanpa jendela console di build release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod php_page;
mod project_new;
mod settings;
mod tray;

use tauri::{Manager, WindowEvent};

fn show_main(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn main() {
    tauri::Builder::default()
        // Membuka DevLock lagi cukup memunculkan jendela yang sudah ada.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_main(app)
        }))
        .setup(|app| {
            tray::create(app.handle())?;
            // Dijalankan saat login Windows (`--minimized`): langsung ke tray.
            if std::env::args().any(|a| a == "--minimized") {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.hide();
                }
            }
            let autostart = devlock_core::Paths::discover()
                .and_then(|paths| devlock_core::Config::load(&paths))
                .is_ok_and(|config| config.gui.autostart_services);
            if autostart {
                tray::run_in_background(app.handle(), "start");
            }
            Ok(())
        })
        // Menutup jendela hanya menyembunyikannya ke tray; service tetap diawasi daemon.
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::overview,
            commands::service_action,
            commands::list_sites,
            commands::list_runtimes,
            commands::available_runtimes,
            commands::install_runtime,
            commands::uninstall_runtime,
            commands::set_default_runtime,
            commands::read_log,
            commands::open_url,
            commands::open_folder,
            commands::open_terminal,
            commands::create_site_database,
            php_page::php_info,
            php_page::php_set_extension,
            php_page::php_xdebug,
            php_page::php_set_ini,
            php_page::open_php_ini,
            settings::get_settings,
            settings::save_settings,
            project_new::list_templates,
            project_new::create_project,
        ])
        .run(tauri::generate_context!())
        .expect("gagal menjalankan DevLock");
}
