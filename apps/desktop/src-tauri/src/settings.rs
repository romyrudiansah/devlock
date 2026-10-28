//! Tab Settings: nilai utama devlock.toml dan autostart.

use devlock_core::daemon::{self, Request};
use devlock_core::Config;
use serde::{Deserialize, Serialize};

use crate::commands::{blocking, context, err, CmdResult};

#[derive(Serialize, Deserialize)]
pub struct Settings {
    www: String,
    tld: String,
    http_port: u16,
    https_enabled: bool,
    https_port: u16,
    php_workers: u16,
    autostart_services: bool,
    editor: String,
    start_with_windows: bool,
}

#[tauri::command]
pub async fn get_settings() -> CmdResult<Settings> {
    blocking(|| {
        let (paths, config) = context()?;
        Ok(Settings {
            www: config.www_dir(&paths).display().to_string(),
            tld: config.tld,
            http_port: config.http.port,
            https_enabled: config.https.enabled,
            https_port: config.https.port,
            php_workers: config.php.workers,
            autostart_services: config.gui.autostart_services,
            editor: config.gui.editor,
            start_with_windows: startup::is_enabled(),
        })
    })
    .await
}

/// Simpan ke devlock.toml (komentar dipertahankan) lalu muat ulang daemon bila berjalan.
#[tauri::command]
pub async fn save_settings(settings: Settings) -> CmdResult<String> {
    blocking(move || {
        let (paths, _) = context()?;
        let tld = settings.tld.trim().trim_start_matches('.').to_lowercase();
        if tld.is_empty() || !tld.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return Err("TLD hanya boleh huruf, angka, dan `-` (mis. `test`)".into());
        }
        if settings.www.trim().is_empty() {
            return Err("folder www tidak boleh kosong".into());
        }
        if settings.http_port == 0 || settings.https_port == 0 {
            return Err("port tidak valid".into());
        }
        let set = |table: &str, key: &str, value: toml_edit::Item| {
            Config::set_value(&paths, table, key, value).map_err(err)
        };
        // Nilai top-level ditulis lewat set_top_level karena bukan bagian dari tabel.
        Config::set_top_level(&paths, "www", toml_edit::value(settings.www.trim())).map_err(err)?;
        Config::set_top_level(&paths, "tld", toml_edit::value(tld)).map_err(err)?;
        set(
            "http",
            "port",
            toml_edit::value(i64::from(settings.http_port)),
        )?;
        set("https", "enabled", toml_edit::value(settings.https_enabled))?;
        set(
            "https",
            "port",
            toml_edit::value(i64::from(settings.https_port)),
        )?;
        set(
            "php",
            "workers",
            toml_edit::value(i64::from(settings.php_workers.clamp(1, 10))),
        )?;
        set(
            "gui",
            "autostart_services",
            toml_edit::value(settings.autostart_services),
        )?;
        set("gui", "editor", toml_edit::value(settings.editor.trim()))?;
        startup::set_enabled(settings.start_with_windows).map_err(err)?;

        let config = Config::load(&paths).map_err(err)?;
        let reloaded = daemon::send(&config, &Request::Reload)
            .map_err(err)?
            .is_some();
        Ok(if reloaded {
            "Pengaturan disimpan dan diterapkan.".into()
        } else {
            "Pengaturan disimpan.".into()
        })
    })
    .await
}

/// Jalankan DevLock (diminimalkan ke tray) saat login Windows lewat
/// `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`.
mod startup {
    const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    const VALUE: &str = "DevLock";

    #[cfg(windows)]
    pub fn is_enabled() -> bool {
        use winreg::enums::HKEY_CURRENT_USER;
        winreg::RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey(RUN_KEY)
            .and_then(|key| key.get_value::<String, _>(VALUE))
            .is_ok()
    }

    #[cfg(windows)]
    pub fn set_enabled(enabled: bool) -> anyhow::Result<()> {
        use winreg::enums::HKEY_CURRENT_USER;
        let (key, _) = winreg::RegKey::predef(HKEY_CURRENT_USER).create_subkey(RUN_KEY)?;
        if enabled {
            let exe = std::env::current_exe()?;
            key.set_value(VALUE, &format!("\"{}\" --minimized", exe.display()))?;
        } else if is_enabled() {
            key.delete_value(VALUE)?;
        }
        Ok(())
    }

    #[cfg(not(windows))]
    pub fn is_enabled() -> bool {
        false
    }

    #[cfg(not(windows))]
    pub fn set_enabled(_: bool) -> anyhow::Result<()> {
        Ok(())
    }
}
