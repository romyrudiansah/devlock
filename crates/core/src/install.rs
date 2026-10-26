//! Download, verifikasi, dan ekstrak runtime ke `runtimes\<kind>\<version>`.

use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use sha2::{Digest, Sha256};

use crate::catalog::{Checksum, Release};
use crate::http;
use crate::paths::Paths;
use crate::php;
use crate::runtime::Kind;

pub enum Event<'a> {
    Downloading { downloaded: u64, total: Option<u64> },
    Verifying,
    /// Checksum tidak tersedia dari upstream; file tidak diverifikasi.
    Unverified,
    Extracting,
    Configuring(&'a str),
}

pub fn install(paths: &Paths, release: &Release, on_event: &mut dyn FnMut(Event)) -> Result<PathBuf> {
    let target = paths.runtime_dir(release.kind.as_str(), &release.version);
    if target.exists() {
        bail!("{} {} sudah terinstall di {}", release.kind, release.version, target.display());
    }
    std::fs::create_dir_all(paths.downloads())?;

    let archive = paths.downloads().join(release.file_name());
    let expected = expected_sha256(release)?;
    let cached_ok = archive.is_file()
        && expected.as_deref().is_some_and(|sum| sha256_file(&archive).ok().as_deref() == Some(sum));
    if !cached_ok {
        http::download(&release.url, &archive, &mut |downloaded, total| {
            on_event(Event::Downloading { downloaded, total })
        })?;
    }

    match &expected {
        Some(sum) => {
            on_event(Event::Verifying);
            let actual = sha256_file(&archive)?;
            if !actual.eq_ignore_ascii_case(sum) {
                let _ = std::fs::remove_file(&archive);
                bail!("checksum {} tidak cocok (diharapkan {sum}, didapat {actual})", release.file_name());
            }
        }
        None => on_event(Event::Unverified),
    }

    let parent = target.parent().context("path runtime tidak valid")?;
    std::fs::create_dir_all(parent)?;
    let staging = parent.join(format!(".{}.tmp", release.version));
    if staging.exists() {
        std::fs::remove_dir_all(&staging)?;
    }
    std::fs::create_dir_all(&staging)?;

    on_event(Event::Extracting);
    let result = (|| -> Result<()> {
        if release.file_name().ends_with(".zip") {
            extract_zip(&archive, &staging)?;
        } else {
            std::fs::copy(&archive, staging.join(release.file_name()))?;
        }
        let content_root = single_child_dir(&staging)?.unwrap_or_else(|| staging.clone());
        std::fs::rename(&content_root, &target)
            .with_context(|| format!("gagal memindahkan ke {}", target.display()))?;
        Ok(())
    })();
    let _ = std::fs::remove_dir_all(&staging);
    result?;

    if release.kind == Kind::Php {
        on_event(Event::Configuring("php.ini"));
        php::write_default_ini(&target)?;
    }
    Ok(target)
}

fn expected_sha256(release: &Release) -> Result<Option<String>> {
    match &release.checksum {
        Checksum::None => Ok(None),
        Checksum::Sha256(sum) => Ok(Some(sum.to_lowercase())),
        Checksum::ShasumsFile(url) => {
            let text = http::get_string(url)?;
            Ok(lookup_shasum(&text, release.file_name()))
        }
    }
}

/// Cari hash untuk `file` di format `<sha256>  <nama file>`; file berisi satu hash juga diterima.
fn lookup_shasum(text: &str, file: &str) -> Option<String> {
    let all: Vec<&str> = text.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    let found = all.iter().find_map(|line| {
        let (hash, name) = line.split_once(char::is_whitespace)?;
        (name.trim().trim_start_matches('*') == file).then_some(hash)
    });
    let hash = match (found, all.as_slice()) {
        (Some(hash), _) => hash,
        (None, [only]) => only.split_whitespace().next()?,
        _ => return None,
    };
    (hash.len() == 64 && hash.chars().all(|c| c.is_ascii_hexdigit())).then(|| hash.to_lowercase())
}

pub fn sha256_file(path: &Path) -> Result<String> {
    let mut file = File::open(path).with_context(|| format!("gagal membuka {}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex(&hasher.finalize()))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn extract_zip(archive: &Path, dest: &Path) -> Result<()> {
    let file = File::open(archive)?;
    let mut zip = zip::ZipArchive::new(file)
        .with_context(|| format!("{} bukan arsip zip yang valid", archive.display()))?;
    zip.extract(dest)
        .with_context(|| format!("gagal mengekstrak {}", archive.display()))
}

/// Bila folder hanya berisi satu subfolder (mis. `node-v22-win-x64\`), kembalikan subfolder itu.
fn single_child_dir(dir: &Path) -> Result<Option<PathBuf>> {
    let entries: Vec<_> = std::fs::read_dir(dir)?.filter_map(|e| e.ok()).collect();
    match entries.as_slice() {
        [only] if only.path().is_dir() => Ok(Some(only.path())),
        _ => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HASH: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    #[test]
    fn shasums_lookup() {
        let text = format!("{}  node-v1-linux.tar.gz\n{HASH}  node-v1-win-x64.zip\n", "f".repeat(64));
        assert_eq!(lookup_shasum(&text, "node-v1-win-x64.zip").as_deref(), Some(HASH));
        assert_eq!(lookup_shasum(&format!("{HASH}  composer.phar"), "x.phar").as_deref(), Some(HASH));
        assert_eq!(lookup_shasum(&text, "missing.zip"), None);
    }

    #[test]
    fn strips_single_root_folder() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("node-v1/bin")).unwrap();
        assert_eq!(single_child_dir(tmp.path()).unwrap(), Some(tmp.path().join("node-v1")));
        std::fs::write(tmp.path().join("php.exe"), b"").unwrap();
        assert_eq!(single_child_dir(tmp.path()).unwrap(), None);
    }

    #[test]
    fn hashes_file() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("a.txt");
        std::fs::write(&file, b"abc").unwrap();
        assert_eq!(
            sha256_file(&file).unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
