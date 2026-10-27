//! Xdebug satu klik: unduh DLL resmi dari xdebug.org yang cocok dengan versi PHP
//! (NTS x64, compiler sama), verifikasi SHA256, lalu aktifkan di `php.ini`.

use std::path::Path;

use anyhow::{bail, Context, Result};

use crate::http;
use crate::install::sha256_file;
use crate::paths::Paths;
use crate::php;
use crate::version::Version;

const DOWNLOAD_PAGE: &str = "https://xdebug.org/download/historical";
const FILES: &str = "https://xdebug.org/files/";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Build {
    pub version: String,
    pub file: String,
    pub sha256: String,
}

/// File DLL Xdebug yang terpasang di folder PHP.
pub fn dll_path(php_dir: &Path) -> std::path::PathBuf {
    php_dir.join("ext").join("php_xdebug.dll")
}

pub fn is_installed(php_dir: &Path) -> bool {
    dll_path(php_dir).is_file()
}

pub fn is_enabled(php_dir: &Path) -> bool {
    php::extensions(php_dir)
        .map(|list| list.iter().any(|e| e.name == "xdebug" && e.enabled))
        .unwrap_or(false)
}

/// Build Xdebug stabil terbaru untuk PHP `line` (mis. `8.3`).
pub fn find_build(line: &str) -> Result<Build> {
    let page = http::get_string(DOWNLOAD_PAGE)?;
    pick_build(&parse_builds(&page), line)
        .with_context(|| format!("tidak ada build Xdebug Windows (NTS x64) untuk PHP {line}"))
}

/// Ambil semua link DLL beserta SHA256 dari atribut `title` halaman unduhan.
fn parse_builds(page: &str) -> Vec<Build> {
    page.split("<a title=\"SHA256:")
        .skip(1)
        .filter_map(|chunk| {
            let sha256 = chunk
                .trim_start_matches("&nbsp;")
                .split('"')
                .next()?
                .trim()
                .to_ascii_lowercase();
            let href = chunk.split("href='/files/").nth(1)?.split('\'').next()?;
            if !href.ends_with(".dll") || sha256.len() != 64 {
                return None;
            }
            let version = href
                .strip_prefix("php_xdebug-")?
                .split('-')
                .next()?
                .to_string();
            Some(Build {
                version,
                file: href.to_string(),
                sha256,
            })
        })
        .collect()
}

/// Build stabil (tanpa alpha/beta/RC) NTS x64 tertinggi untuk `line`.
fn pick_build(builds: &[Build], line: &str) -> Option<Build> {
    builds
        .iter()
        .filter(|b| {
            let parts: Vec<&str> = b.file.trim_end_matches(".dll").split('-').collect();
            // php_xdebug-<ver>-<php>-<nts|vc15>-<vs16|nts>-x86_64
            parts.get(2) == Some(&line)
                && parts.contains(&"nts")
                && parts.last() == Some(&"x86_64")
                && b.version.chars().all(|c| c.is_ascii_digit() || c == '.')
        })
        .max_by(|a, b| Version::parse(&a.version).cmp(&Version::parse(&b.version)))
        .cloned()
}

/// Pasang Xdebug ke `php_dir\ext\php_xdebug.dll`. Mengembalikan versi Xdebug.
pub fn install(paths: &Paths, php_dir: &Path, line: &str) -> Result<String> {
    let build = find_build(line)?;
    std::fs::create_dir_all(paths.downloads())?;
    let tmp = paths.downloads().join(&build.file);
    http::download(&format!("{FILES}{}", build.file), &tmp, &mut |_, _| {})?;
    let actual = sha256_file(&tmp)?;
    if actual != build.sha256 {
        let _ = std::fs::remove_file(&tmp);
        bail!(
            "checksum {} tidak cocok (diharapkan {}, didapat {actual})",
            build.file,
            build.sha256
        );
    }
    std::fs::copy(&tmp, dll_path(php_dir))
        .with_context(|| format!("gagal menyalin ke {}", dll_path(php_dir).display()))?;
    Ok(build.version)
}

/// Aktifkan / matikan Xdebug. Saat aktif: mode debug + develop, mulai saat ada trigger
/// (`XDEBUG_SESSION`), client `127.0.0.1:9003` (default VS Code / PhpStorm).
pub fn set_enabled(php_dir: &Path, enabled: bool) -> Result<()> {
    if enabled && !is_installed(php_dir) {
        bail!("Xdebug belum terpasang; jalankan `devlock php xdebug install`");
    }
    if is_installed(php_dir) {
        php::set_extension(php_dir, "xdebug", enabled)?;
    }
    if enabled {
        for (key, value) in [
            ("xdebug.mode", "debug,develop"),
            ("xdebug.start_with_request", "trigger"),
            ("xdebug.client_host", "127.0.0.1"),
            ("xdebug.client_port", "9003"),
        ] {
            php::set_ini_value(php_dir, key, value)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAGE: &str = r#"
<li><a title="SHA256:&nbsp;AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA" href='/files/php_xdebug-3.5.1-8.3-nts-vs16-x86_64.dll'>PHP 8.3 VS16</a></li>
<li><a title="SHA256:&nbsp;bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb" href='/files/php_xdebug-3.5.3-8.3-nts-vs16-x86_64.dll'>PHP 8.3 VS16</a></li>
<li><a title="SHA256:&nbsp;cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc" href='/files/php_xdebug-3.5.3-8.3-ts-vs16-x86_64.dll'>PHP 8.3 TS</a></li>
<li><a title="SHA256:&nbsp;dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd" href='/files/php_xdebug-3.6.0alpha1-8.3-nts-vs16-x86_64.dll'>alpha</a></li>
<li><a title="SHA256:&nbsp;eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee" href='/files/php_xdebug-3.1.6-7.4-vc15-nts-x86_64.dll'>PHP 7.4</a></li>
<li><a title="SHA256:&nbsp;ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff" href='/files/xdebug-3.5.3.tgz'>source</a></li>
"#;

    #[test]
    fn picks_latest_stable_nts_build() {
        let builds = parse_builds(PAGE);
        assert_eq!(builds.len(), 5);
        let b = pick_build(&builds, "8.3").unwrap();
        assert_eq!(b.file, "php_xdebug-3.5.3-8.3-nts-vs16-x86_64.dll");
        assert_eq!(b.sha256, "b".repeat(64));
        let old = pick_build(&builds, "7.4").unwrap();
        assert_eq!(old.version, "3.1.6");
        assert!(pick_build(&builds, "8.5").is_none());
    }
}
