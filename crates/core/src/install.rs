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
    Downloading {
        downloaded: u64,
        total: Option<u64>,
    },
    Verifying,
    /// Checksum tidak tersedia dari upstream; file tidak diverifikasi.
    Unverified,
    Extracting,
    Configuring(&'a str),
}

pub fn install(
    paths: &Paths,
    release: &Release,
    on_event: &mut dyn FnMut(Event),
) -> Result<PathBuf> {
    let target = paths.runtime_dir(release.kind.as_str(), &release.version);
    if target.exists() {
        bail!(
            "{} {} sudah terinstall di {}",
            release.kind,
            release.version,
            target.display()
        );
    }
    std::fs::create_dir_all(paths.downloads())?;

    // Nama aset tanpa versi (mis. Garnet `win-x64-based-readytorun.zip`) diberi awalan versi
    // agar unduhan versi berbeda tidak saling menimpa atau dilanjutkan dari file yang salah.
    let archive = paths
        .downloads()
        .join(if release.file_name().contains(&release.version) {
            release.file_name().to_string()
        } else {
            format!(
                "{}-{}-{}",
                release.kind,
                release.version,
                release.file_name()
            )
        });
    let expected = expected_sha256(release)?;
    let cached_ok = archive.is_file()
        && expected
            .as_deref()
            .is_some_and(|sum| sha256_file(&archive).ok().as_deref() == Some(sum));
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
                bail!(
                    "checksum {} tidak cocok (diharapkan {sum}, didapat {actual})",
                    release.file_name()
                );
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
            extract_zip(&archive, &staging, skipped_dirs(release.kind))?;
        } else {
            std::fs::copy(&archive, staging.join(release.file_name()))?;
        }
        // Zip Apache Lounge berisi `Apache24\` ditambah ReadMe di root.
        let apache_root = staging.join("Apache24");
        let content_root = if release.kind == Kind::Apache && apache_root.is_dir() {
            apache_root
        } else {
            single_child_dir(&staging)?.unwrap_or_else(|| staging.clone())
        };
        std::fs::rename(&content_root, &target)
            .with_context(|| format!("gagal memindahkan ke {}", target.display()))?;
        Ok(())
    })();
    let _ = std::fs::remove_dir_all(&staging);
    result?;

    if release.kind == Kind::Php {
        on_event(Event::Configuring("php.ini"));
        php::write_default_ini(&target)?;
        // Tanpa bundle CA, curl/openssl PHP di Windows gagal memverifikasi HTTPS. Kegagalan
        // mengunduh bundle tidak membatalkan install; bisa diulang lewat `devlock ssl bundle`.
        match ensure_ca_bundle(paths) {
            Ok(bundle) => set_php_ca_bundle(&target, &bundle)?,
            Err(err) => tracing::warn!("bundle CA untuk PHP belum dipasang: {err:#}"),
        }
    }
    Ok(target)
}

/// Bundle CA Mozilla yang dikurasi proyek curl.
const CURL_CA_BUNDLE: &str = "https://curl.se/ca/cacert.pem";
/// Bundle Mozilla diperbarui bila lebih tua dari ini.
const CA_BUNDLE_MAX_AGE_DAYS: u64 = 30;

/// `ssl\ca-bundle.pem`: bundle CA Mozilla ditambah CA DevLock (bila sudah dibuat), dipakai
/// `curl.cainfo` / `openssl.cafile` PHP.
pub fn ca_bundle_path(paths: &Paths) -> PathBuf {
    paths.ssl().join("ca-bundle.pem")
}

/// Unduh bundle CA Mozilla (diverifikasi dengan `.sha256` dari curl.se) bila belum ada atau
/// sudah tua, lalu tulis ulang `ssl\ca-bundle.pem` beserta CA DevLock.
pub fn ensure_ca_bundle(paths: &Paths) -> Result<PathBuf> {
    std::fs::create_dir_all(paths.ssl())?;
    let mozilla = paths.ssl().join("cacert.pem");
    let stale = std::fs::metadata(&mozilla)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.elapsed().ok())
        .map_or(true, |age| age.as_secs() > CA_BUNDLE_MAX_AGE_DAYS * 86_400);
    if stale {
        std::fs::create_dir_all(paths.downloads())?;
        let tmp = paths.downloads().join("cacert.pem");
        http::download(CURL_CA_BUNDLE, &tmp, &mut |_, _| {})?;
        let sums = http::get_string(&format!("{CURL_CA_BUNDLE}.sha256"))?;
        let expected = lookup_shasum(&sums, "cacert.pem")
            .context("checksum cacert.pem dari curl.se tidak terbaca")?;
        if sha256_file(&tmp)? != expected {
            let _ = std::fs::remove_file(&tmp);
            bail!("checksum cacert.pem tidak cocok");
        }
        std::fs::rename(&tmp, &mozilla)?;
    }

    let mut bundle = std::fs::read_to_string(&mozilla)?;
    let devlock_ca = crate::ssl::ca_cert_path(paths);
    if let Ok(ca) = std::fs::read_to_string(&devlock_ca) {
        bundle.push_str("\n# DevLock Local CA\n");
        bundle.push_str(&ca);
    }
    let out = ca_bundle_path(paths);
    std::fs::write(&out, bundle).with_context(|| format!("gagal menulis {}", out.display()))?;
    Ok(out)
}

fn set_php_ca_bundle(php_dir: &Path, bundle: &Path) -> Result<()> {
    let value = bundle.to_string_lossy();
    php::set_ini_value(php_dir, "curl.cainfo", &value)?;
    php::set_ini_value(php_dir, "openssl.cafile", &value)
}

/// Pasang ulang bundle CA ke semua PHP terinstall (mis. setelah CA DevLock dibuat).
/// Mengembalikan jumlah versi PHP yang diperbarui.
pub fn apply_ca_bundle_to_php(paths: &Paths) -> Result<usize> {
    let bundle = ensure_ca_bundle(paths)?;
    let versions = crate::runtime::installed_versions(paths, Kind::Php)?;
    for version in &versions {
        let dir = paths.runtime_dir("php", version);
        if dir.join("php.ini").is_file() {
            set_php_ca_bundle(&dir, &bundle)?;
        }
    }
    Ok(versions.len())
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
    let all: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
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

/// SHA1 file (dipakai untuk verifikasi zip WordPress yang hanya menerbitkan `.sha1`).
pub fn sha1_file(path: &Path) -> Result<String> {
    use sha1::Sha1;
    let mut file = File::open(path).with_context(|| format!("gagal membuka {}", path.display()))?;
    let mut hasher = Sha1::new();
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

/// Ekstrak seluruh isi zip ke `dest`.
pub fn extract_zip_to(archive: &Path, dest: &Path) -> Result<()> {
    std::fs::create_dir_all(dest)?;
    extract_zip(archive, dest, &[])
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

/// Folder di arsip yang tidak dibutuhkan untuk menjalankan server, dilewati saat ekstrak.
/// Zip PostgreSQL dari EDB ~900 MB, ~720 MB di antaranya pgAdmin 4.
fn skipped_dirs(kind: Kind) -> &'static [&'static str] {
    match kind {
        Kind::Postgresql => &["pgAdmin 4", "StackBuilder", "doc", "include", "symbols"],
        // Dokumentasi HTML Apache (~ribuan file) tidak dibutuhkan.
        Kind::Apache => &["manual"],
        _ => &[],
    }
}

/// `true` bila salah satu dari dua komponen pertama path (root arsip + subfolder) ada di `skip`.
fn is_skipped(rel: &Path, skip: &[&str]) -> bool {
    rel.components().take(2).any(|c| {
        c.as_os_str()
            .to_str()
            .is_some_and(|name| skip.contains(&name))
    })
}

fn extract_zip(archive: &Path, dest: &Path, skip: &[&str]) -> Result<()> {
    let file = File::open(archive)?;
    let mut zip = zip::ZipArchive::new(file)
        .with_context(|| format!("{} bukan arsip zip yang valid", archive.display()))?;
    if skip.is_empty() {
        return zip
            .extract(dest)
            .with_context(|| format!("gagal mengekstrak {}", archive.display()));
    }
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        // Nama yang keluar dari folder tujuan (`..`, path absolut) diabaikan.
        let Some(rel) = entry.enclosed_name() else {
            continue;
        };
        let rel = rel.to_path_buf();
        if is_skipped(&rel, skip) {
            continue;
        }
        let out = dest.join(&rel);
        if entry.is_dir() {
            std::fs::create_dir_all(&out)?;
            continue;
        }
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut file =
            File::create(&out).with_context(|| format!("gagal membuat {}", out.display()))?;
        std::io::copy(&mut entry, &mut file)
            .with_context(|| format!("gagal mengekstrak {}", rel.display()))?;
    }
    Ok(())
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
        let text = format!(
            "{}  node-v1-linux.tar.gz\n{HASH}  node-v1-win-x64.zip\n",
            "f".repeat(64)
        );
        assert_eq!(
            lookup_shasum(&text, "node-v1-win-x64.zip").as_deref(),
            Some(HASH)
        );
        assert_eq!(
            lookup_shasum(&format!("{HASH}  composer.phar"), "x.phar").as_deref(),
            Some(HASH)
        );
        assert_eq!(lookup_shasum(&text, "missing.zip"), None);
    }

    #[test]
    fn skips_unneeded_postgres_folders() {
        let skip = skipped_dirs(Kind::Postgresql);
        assert!(is_skipped(Path::new("pgsql/pgAdmin 4/python/x.dll"), skip));
        assert!(is_skipped(Path::new("pgsql/doc/index.html"), skip));
        assert!(!is_skipped(Path::new("pgsql/bin/postgres.exe"), skip));
        assert!(!is_skipped(Path::new("pgsql/share/locale/x.mo"), skip));
        // Hanya dua level teratas yang dicek.
        assert!(!is_skipped(Path::new("pgsql/share/doc/readme"), skip));
        assert!(skipped_dirs(Kind::Php).is_empty());
    }

    #[test]
    fn strips_single_root_folder() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("node-v1/bin")).unwrap();
        assert_eq!(
            single_child_dir(tmp.path()).unwrap(),
            Some(tmp.path().join("node-v1"))
        );
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
