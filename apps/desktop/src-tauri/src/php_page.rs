//! Tab PHP: ekstensi, Xdebug, dan nilai php.ini umum per versi PHP.

use std::path::PathBuf;

use devlock_core::php::{self, Extension};
use devlock_core::version::Version;
use devlock_core::{daemon, runtime, xdebug, Kind};
use serde::Serialize;

use crate::commands::{blocking, context, err, CmdResult};

/// Key php.ini yang bisa diubah langsung dari GUI.
pub const INI_KEYS: &[&str] = &[
    "memory_limit",
    "upload_max_filesize",
    "post_max_size",
    "max_execution_time",
    "display_errors",
    "date.timezone",
];

#[derive(Serialize)]
pub struct PhpInfo {
    version: String,
    ini_path: String,
    extensions: Vec<Extension>,
    xdebug_installed: bool,
    xdebug_enabled: bool,
    /// Nilai aktif untuk [`INI_KEYS`] (`None` = default PHP).
    ini: Vec<(String, Option<String>)>,
}

fn php_dir(version: &str) -> CmdResult<(String, PathBuf)> {
    let (paths, _) = context()?;
    runtime::require_installed(&paths, Kind::Php, version).map_err(err)
}

/// Restart pool php-cgi versi ini bila sedang berjalan (agar php.ini baru dipakai).
fn apply(version: &str) -> CmdResult<()> {
    let (_, config) = context()?;
    let line = Version::parse(version).map_or(version.to_string(), |v| v.major_minor());
    daemon::restart_group_if_running(&config, &format!("php@{line}")).map_err(err)?;
    Ok(())
}

fn info(version: &str) -> CmdResult<PhpInfo> {
    let (version, dir) = php_dir(version)?;
    let ini_path = dir.join("php.ini");
    let text = std::fs::read_to_string(&ini_path).unwrap_or_default();
    Ok(PhpInfo {
        version,
        ini_path: ini_path.display().to_string(),
        extensions: php::extensions(&dir).map_err(err)?,
        xdebug_installed: xdebug::is_installed(&dir),
        xdebug_enabled: xdebug::is_enabled(&dir),
        ini: INI_KEYS
            .iter()
            .map(|k| (k.to_string(), php::ini_value(&text, k)))
            .collect(),
    })
}

#[tauri::command]
pub async fn php_info(version: String) -> CmdResult<PhpInfo> {
    blocking(move || info(&version)).await
}

#[tauri::command]
pub async fn php_set_extension(version: String, name: String, enabled: bool) -> CmdResult<PhpInfo> {
    blocking(move || {
        let (_, dir) = php_dir(&version)?;
        php::set_extension(&dir, &name, enabled).map_err(err)?;
        apply(&version)?;
        info(&version)
    })
    .await
}

/// `action`: `install` (unduh dari xdebug.org lalu aktifkan), `on`, `off`.
#[tauri::command]
pub async fn php_xdebug(version: String, action: String) -> CmdResult<PhpInfo> {
    blocking(move || {
        let (paths, _) = context()?;
        let (full, dir) = php_dir(&version)?;
        match action.as_str() {
            "install" => {
                let line = Version::parse(&full).map_or(full.clone(), |v| v.major_minor());
                xdebug::install(&paths, &dir, &line).map_err(err)?;
                xdebug::set_enabled(&dir, true).map_err(err)?;
            }
            "on" => xdebug::set_enabled(&dir, true).map_err(err)?,
            "off" => xdebug::set_enabled(&dir, false).map_err(err)?,
            other => return Err(format!("aksi Xdebug `{other}` tidak dikenal")),
        }
        apply(&version)?;
        info(&version)
    })
    .await
}

#[tauri::command]
pub async fn php_set_ini(version: String, values: Vec<(String, String)>) -> CmdResult<PhpInfo> {
    blocking(move || {
        let (_, dir) = php_dir(&version)?;
        for (key, value) in &values {
            if !INI_KEYS.contains(&key.as_str()) {
                return Err(format!("key `{key}` tidak bisa diubah dari GUI"));
            }
            php::set_ini_value(&dir, key, value.trim()).map_err(err)?;
        }
        apply(&version)?;
        info(&version)
    })
    .await
}

/// Buka php.ini dengan editor bawaan Windows untuk file .ini.
#[tauri::command]
pub fn open_php_ini(version: String) -> CmdResult<()> {
    let (_, dir) = php_dir(&version)?;
    tauri_plugin_opener::open_path(dir.join("php.ini"), None::<&str>).map_err(|e| e.to_string())
}
