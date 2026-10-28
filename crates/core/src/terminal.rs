//! Profil Windows Terminal "DevLock" lewat JSON fragment, tanpa menyentuh `settings.json`
//! milik pengguna: <https://learn.microsoft.com/windows/terminal/json-fragment-extensions>.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::config::Config;
use crate::paths::Paths;

/// GUID tetap agar Windows Terminal mengenali profil yang sama setiap kali fragment ditulis ulang.
pub const PROFILE_GUID: &str = "{5e0c1f3a-7d2b-4f6e-9a8c-de71c0c4a001}";
pub const PROFILE_NAME: &str = "DevLock";

/// `%LOCALAPPDATA%\Microsoft\Windows Terminal\Fragments\DevLock`.
pub fn fragment_dir() -> Option<PathBuf> {
    let local = std::env::var_os("LOCALAPPDATA")?;
    Some(
        PathBuf::from(local)
            .join("Microsoft")
            .join("Windows Terminal")
            .join("Fragments")
            .join("DevLock"),
    )
}

pub fn fragment_path() -> Option<PathBuf> {
    fragment_dir().map(|dir| dir.join("devlock.json"))
}

/// Windows Terminal terpasang (alias `wt.exe` milik paket Store/winget).
pub fn is_available() -> bool {
    std::env::var_os("LOCALAPPDATA").is_some_and(|local| {
        PathBuf::from(local)
            .join("Microsoft")
            .join("WindowsApps")
            .join("wt.exe")
            .exists()
    })
}

/// Isi fragment: PowerShell di folder www dengan folder shims di depan PATH.
pub fn fragment_json(paths: &Paths, config: &Config) -> String {
    let shims = paths.shims().to_string_lossy().replace('\'', "''");
    let command =
        format!("powershell.exe -NoExit -NoLogo -Command \"$env:Path = '{shims};' + $env:Path\"");
    let mut profile = serde_json::json!({
        "guid": PROFILE_GUID,
        "name": PROFILE_NAME,
        "commandline": command,
        "startingDirectory": config.www_dir(paths).to_string_lossy(),
        "tabTitle": PROFILE_NAME,
    });
    let icon = paths.root().join("bin").join("devlock-gui.exe");
    if icon.is_file() {
        profile["icon"] = icon.to_string_lossy().into_owned().into();
    }
    let fragment = serde_json::json!({ "profiles": [profile] });
    serde_json::to_string_pretty(&fragment).expect("JSON fragment valid") + "\n"
}

/// Tulis fragment bila isinya berubah. Mengembalikan path fragment.
pub fn install(paths: &Paths, config: &Config) -> Result<PathBuf> {
    let file = fragment_path().context("LOCALAPPDATA tidak ditemukan")?;
    write_if_changed(&file, &fragment_json(paths, config))?;
    Ok(file)
}

/// Perbarui profil yang sudah dipasang (`devlock terminal install`) agar mengikuti lokasi
/// www/shims terkini. Tidak pernah membuat profil baru: DevLock hanya mengubah hal di luar
/// foldernya saat pengguna menjalankan perintahnya.
pub fn sync(paths: &Paths, config: &Config) -> Result<Option<PathBuf>> {
    match fragment_path() {
        Some(file) if file.is_file() => install(paths, config).map(Some),
        _ => Ok(None),
    }
}

/// Hapus fragment (profil hilang dari Windows Terminal saat dibuka berikutnya).
pub fn remove() -> Result<bool> {
    let Some(dir) = fragment_dir() else {
        return Ok(false);
    };
    if !dir.exists() {
        return Ok(false);
    }
    std::fs::remove_dir_all(&dir).with_context(|| format!("gagal menghapus {}", dir.display()))?;
    Ok(true)
}

fn write_if_changed(file: &Path, content: &str) -> Result<()> {
    if std::fs::read_to_string(file).is_ok_and(|old| old == content) {
        return Ok(());
    }
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(file, content).with_context(|| format!("gagal menulis {}", file.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fragment_has_profile_with_shims_first() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().join("O'Brien"));
        let json: serde_json::Value =
            serde_json::from_str(&fragment_json(&paths, &Config::default())).unwrap();
        let profile = &json["profiles"][0];
        assert_eq!(profile["guid"], PROFILE_GUID);
        assert_eq!(profile["name"], "DevLock");
        let command = profile["commandline"].as_str().unwrap();
        let shims = paths.shims().to_string_lossy().replace('\'', "''");
        assert!(command.contains(&format!("$env:Path = '{shims};' + $env:Path")));
        assert!(command.contains("O''Brien"));
        assert_eq!(
            profile["startingDirectory"],
            paths.default_www().to_string_lossy().as_ref()
        );
        assert!(profile.get("icon").is_none());
    }

    #[test]
    fn write_if_changed_skips_identical_content() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("Fragments/DevLock/devlock.json");
        write_if_changed(&file, "a").unwrap();
        let before = std::fs::metadata(&file).unwrap().modified().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        write_if_changed(&file, "a").unwrap();
        assert_eq!(
            std::fs::metadata(&file).unwrap().modified().unwrap(),
            before
        );
        write_if_changed(&file, "b").unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "b");
    }
}
