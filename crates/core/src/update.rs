//! Update DevLock sendiri dari GitHub Releases: cek versi terbaru, unduh zip rilis (SHA256
//! dari `.sha256` yang ikut terbit), ganti binary di `bin\`, dan kembalikan versi lama bila
//! Windows (Smart App Control) memblokir binary baru.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::Deserialize;

use crate::http;
use crate::paths::Paths;
use crate::version::Version;

const RELEASES: &str = "https://api.github.com/repos/romyrudiansah/devlock/releases?per_page=20";

/// Binary yang dikirim di zip rilis (`bin\`).
pub const BINARIES: &[&str] = &["devlock.exe", "devlock-shim.exe", "devlock-gui.exe"];

#[derive(Debug, Clone, PartialEq)]
pub struct Available {
    pub version: String,
    pub zip_url: String,
    pub sha256_url: String,
}

#[derive(Deserialize)]
struct GhRelease {
    tag_name: String,
    #[serde(default)]
    draft: bool,
    assets: Vec<GhAsset>,
}

#[derive(Deserialize)]
struct GhAsset {
    name: String,
    browser_download_url: String,
}

pub fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Rilis tertinggi yang punya zip Windows dan checksum-nya. Rilis v0.x ditandai prerelease di
/// GitHub, jadi `releases/latest` tidak bisa dipakai.
pub fn latest() -> Result<Option<Available>> {
    let releases: Vec<GhRelease> = http::get_json(RELEASES)?;
    Ok(pick_latest(releases))
}

fn pick_latest(releases: Vec<GhRelease>) -> Option<Available> {
    releases
        .into_iter()
        .filter(|r| !r.draft)
        .filter_map(|r| {
            let version = r.tag_name.trim_start_matches('v').to_string();
            Version::parse(&version)?;
            let zip = format!("devlock-{}-windows-x64.zip", r.tag_name);
            let url = |name: &str| {
                r.assets
                    .iter()
                    .find(|a| a.name == name)
                    .map(|a| a.browser_download_url.clone())
            };
            Some(Available {
                zip_url: url(&zip)?,
                sha256_url: url(&format!("{zip}.sha256"))?,
                version,
            })
        })
        .max_by(|a, b| Version::parse(&a.version).cmp(&Version::parse(&b.version)))
}

/// `true` bila `candidate` lebih baru dari versi yang berjalan.
pub fn is_newer(candidate: &str) -> bool {
    Version::parse(candidate) > Version::parse(current_version())
}

/// Unduh, verifikasi, dan pasang `available` ke `bin\`. Daemon dan GUI harus sudah berhenti.
pub fn apply(
    paths: &Paths,
    available: &Available,
    progress: &mut dyn FnMut(u64, Option<u64>),
) -> Result<()> {
    std::fs::create_dir_all(paths.downloads())?;
    let name = available
        .zip_url
        .rsplit('/')
        .next()
        .unwrap_or("devlock-update.zip");
    let archive = paths.downloads().join(name);
    // Zip rilis tidak dipakai lagi setelah dipasang atau gagal; jangan menumpuk tiap update.
    let result = download_and_install(paths, available, name, &archive, progress);
    let _ = std::fs::remove_file(&archive);
    result
}

fn download_and_install(
    paths: &Paths,
    available: &Available,
    name: &str,
    archive: &Path,
    progress: &mut dyn FnMut(u64, Option<u64>),
) -> Result<()> {
    http::download(&available.zip_url, archive, progress)?;
    let sums = http::get_string(&available.sha256_url)?;
    let expected = crate::install::lookup_shasum(&sums, name)
        .context("checksum rilis DevLock tidak terbaca")?;
    if crate::install::sha256_file(archive)? != expected {
        bail!("checksum {name} tidak cocok");
    }

    let staging = paths.tmp().join(format!("update-{}", available.version));
    if staging.exists() {
        std::fs::remove_dir_all(&staging)?;
    }
    crate::install::extract_zip_to(archive, &staging)?;
    let result = replace_binaries(&paths.bin(), &staging.join("bin"));
    let _ = std::fs::remove_dir_all(&staging);
    result
}

/// Ganti binary di `bin` dengan yang ada di `new_bin`; semua dikembalikan bila ada yang gagal
/// atau diblokir Windows.
pub fn replace_binaries(bin: &Path, new_bin: &Path) -> Result<()> {
    remove_old_backups(bin);
    let mut replaced: Vec<(PathBuf, Option<PathBuf>)> = Vec::new();
    let outcome = (|| -> Result<()> {
        for name in BINARIES {
            let new = new_bin.join(name);
            if !new.is_file() {
                continue;
            }
            let target = bin.join(name);
            let backup = if target.is_file() {
                let backup = bin.join(format!("{name}.old"));
                // Exe yang sedang berjalan (devlock.exe ini sendiri) boleh di-rename di Windows.
                std::fs::rename(&target, &backup)
                    .with_context(|| format!("gagal memindahkan {}", target.display()))?;
                Some(backup)
            } else {
                None
            };
            replaced.push((target.clone(), backup));
            std::fs::copy(&new, &target)
                .with_context(|| format!("gagal menyalin {}", target.display()))?;
        }
        for (target, _) in &replaced {
            if is_blocked(target) {
                bail!(
                    "Windows (Smart App Control) memblokir {}; versi lama dipertahankan",
                    target.file_name().unwrap_or_default().to_string_lossy()
                );
            }
        }
        Ok(())
    })();
    if outcome.is_err() {
        for (target, backup) in replaced.iter().rev() {
            let _ = std::fs::remove_file(target);
            if let Some(backup) = backup {
                let _ = std::fs::rename(backup, target);
            }
        }
    }
    outcome
}

/// `.old` dari update sebelumnya; yang masih dipakai proses berjalan dibiarkan.
fn remove_old_backups(bin: &Path) {
    for name in BINARIES {
        let _ = std::fs::remove_file(bin.join(format!("{name}.old")));
    }
}

/// Jalankan `exe` dalam keadaan suspended lalu langsung hentikan: cukup untuk memicu
/// pemeriksaan Smart App Control tanpa benar-benar menjalankan programnya (mis. GUI).
pub fn is_blocked(exe: &Path) -> bool {
    let mut command = std::process::Command::new(exe);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_SUSPENDED: u32 = 0x0000_0004;
        command.creation_flags(CREATE_SUSPENDED | crate::process::CREATE_NO_WINDOW);
    }
    match command.spawn() {
        Ok(mut child) => {
            let _ = child.kill();
            let _ = child.wait();
            false
        }
        Err(err) => err.raw_os_error() == Some(crate::process::ERROR_APPLICATION_CONTROL_POLICY),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_highest_release_with_assets() {
        let json = r#"[
            {"tag_name": "v0.9.2", "prerelease": true, "assets": [
                {"name": "devlock-v0.9.2-windows-x64.zip", "browser_download_url": "https://x/a.zip"},
                {"name": "devlock-v0.9.2-windows-x64.zip.sha256", "browser_download_url": "https://x/a.sha256"}
            ]},
            {"tag_name": "v0.10.0", "prerelease": true, "assets": [
                {"name": "devlock-v0.10.0-windows-x64.zip", "browser_download_url": "https://x/b.zip"},
                {"name": "devlock-v0.10.0-windows-x64.zip.sha256", "browser_download_url": "https://x/b.sha256"}
            ]},
            {"tag_name": "v0.11.0", "draft": true, "assets": []},
            {"tag_name": "v0.10.1", "assets": [
                {"name": "devlock-v0.10.1-windows-x64.zip", "browser_download_url": "https://x/c.zip"}
            ]}
        ]"#;
        let latest = pick_latest(serde_json::from_str(json).unwrap()).unwrap();
        assert_eq!(latest.version, "0.10.0");
        assert_eq!(latest.sha256_url, "https://x/b.sha256");
        assert!(is_newer("999.0.0"));
        assert!(!is_newer(current_version()));
    }

    #[test]
    fn replaces_binaries_and_keeps_backup() {
        let tmp = tempfile::tempdir().unwrap();
        let (bin, new) = (tmp.path().join("bin"), tmp.path().join("new"));
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::create_dir_all(&new).unwrap();
        std::fs::write(bin.join("devlock-shim.exe"), "lama").unwrap();
        // Bukan exe sungguhan: spawn gagal biasa (bukan 4551), jadi tidak dianggap diblokir.
        std::fs::write(new.join("devlock-shim.exe"), "baru").unwrap();
        replace_binaries(&bin, &new).unwrap();
        assert_eq!(
            std::fs::read_to_string(bin.join("devlock-shim.exe")).unwrap(),
            "baru"
        );
        assert_eq!(
            std::fs::read_to_string(bin.join("devlock-shim.exe.old")).unwrap(),
            "lama"
        );
        assert!(!bin.join("devlock.exe").exists());
    }
}
