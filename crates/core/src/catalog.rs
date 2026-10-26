//! Katalog versi yang bisa diinstall, diambil dari sumber resmi masing-masing runtime.

use std::collections::BTreeMap;

use anyhow::{bail, Context, Result};
use serde::Deserialize;

use crate::http;
use crate::runtime::Kind;
use crate::version::{self, Version};

const PHP_RELEASES: &str = "https://downloads.php.net/~windows/releases/";
const NODE_DIST: &str = "https://nodejs.org/dist/";
const NGINX_DOWNLOAD: &str = "https://nginx.org/download/";
const MARIADB_API: &str = "https://downloads.mariadb.org/rest-api/mariadb/";
const COMPOSER: &str = "https://getcomposer.org";
const POSTGRES_VERSIONS: &str = "https://www.postgresql.org/versions.json";
/// Binary zip resmi PostgreSQL untuk Windows (dibangun EnterpriseDB).
const POSTGRES_DOWNLOAD: &str = "https://get.enterprisedb.com/postgresql/";

/// MySQL tidak punya API daftar versi; daftar ini diperbarui tiap rilis DevLock.
const MYSQL_RELEASES: &[(&str, &str)] = &[
    (
        "9.6.0",
        "https://cdn.mysql.com/Downloads/MySQL-9.6/mysql-9.6.0-winx64.zip",
    ),
    (
        "8.4.11",
        "https://cdn.mysql.com/Downloads/MySQL-8.4/mysql-8.4.11-winx64.zip",
    ),
    (
        "8.0.45",
        "https://cdn.mysql.com/Downloads/MySQL-8.0/mysql-8.0.45-winx64.zip",
    ),
    (
        "5.7.44",
        "https://cdn.mysql.com/archives/mysql-5.7/mysql-5.7.44-winx64.zip",
    ),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Checksum {
    None,
    Sha256(String),
    /// File berisi daftar `<sha256>  <nama file>` (mis. SHASUMS256.txt Node).
    ShasumsFile(String),
}

#[derive(Debug, Clone)]
pub struct Release {
    pub kind: Kind,
    pub version: String,
    pub url: String,
    pub checksum: Checksum,
    /// Ditandai LTS / long-term support oleh upstream.
    pub lts: bool,
}

impl Release {
    pub fn file_name(&self) -> &str {
        self.url.rsplit('/').next().unwrap_or(&self.url)
    }
}

/// Semua versi yang tersedia, terbaru lebih dulu.
pub fn available(kind: Kind) -> Result<Vec<Release>> {
    let mut releases = match kind {
        Kind::Php => php_releases()?,
        Kind::Node => node_releases()?,
        Kind::Composer => composer_releases()?,
        Kind::Nginx => nginx_releases()?,
        Kind::Mysql => mysql_releases(),
        Kind::Mariadb => mariadb_releases(None)?,
        Kind::Postgresql => postgresql_releases()?,
    };
    releases.sort_by(|a, b| {
        let a = Version::parse(&a.version);
        let b = Version::parse(&b.version);
        b.cmp(&a)
    });
    Ok(releases)
}

/// Rilis tertinggi yang cocok dengan `spec` (`8.3`, `20`, `latest`, `lts`).
pub fn find(kind: Kind, spec: &str) -> Result<Release> {
    // MariaDB: cukup ambil satu major bila spec sudah menyebut major.minor.
    let releases = match (kind, Version::parse(spec)) {
        (Kind::Mariadb, Some(v)) if v.parts().len() >= 2 => {
            mariadb_releases(Some(&v.major_minor()))?
        }
        _ => available(kind)?,
    };
    let lts_only = spec.eq_ignore_ascii_case("lts") || spec.starts_with("lts/");
    let found = releases
        .iter()
        .filter(|r| {
            if lts_only {
                r.lts
            } else {
                version::matches(spec, &r.version)
            }
        })
        .max_by(|a, b| Version::parse(&a.version).cmp(&Version::parse(&b.version)));
    if let Some(release) = found {
        return Ok(release.clone());
    }
    if kind == Kind::Php {
        if let Some(release) = php_archive_release(spec) {
            return Ok(release);
        }
    }
    bail!("{kind} {spec} tidak ditemukan; lihat `devlock list {kind} --available`")
}

/// Rilis terbaru per cabang: major.minor untuk PHP/nginx/MySQL/MariaDB, major untuk
/// Node/Composer/PostgreSQL.
pub fn latest_per_line(kind: Kind, releases: &[Release]) -> Vec<&Release> {
    let mut lines: BTreeMap<String, &Release> = BTreeMap::new();
    for release in releases {
        let Some(v) = Version::parse(&release.version) else {
            continue;
        };
        let key = match kind {
            Kind::Node | Kind::Composer | Kind::Postgresql => v.major().to_string(),
            _ => v.major_minor(),
        };
        lines
            .entry(key)
            .and_modify(|cur| {
                if Version::parse(&cur.version) < Some(v.clone()) {
                    *cur = release;
                }
            })
            .or_insert(release);
    }
    let mut out: Vec<&Release> = lines.into_values().collect();
    out.sort_by(|a, b| Version::parse(&b.version).cmp(&Version::parse(&a.version)));
    out
}

// --- PHP ---------------------------------------------------------------------

#[derive(Deserialize)]
struct PhpBuild {
    zip: PhpZip,
}

#[derive(Deserialize)]
struct PhpZip {
    path: String,
    sha256: String,
}

fn php_releases() -> Result<Vec<Release>> {
    let data: BTreeMap<String, serde_json::Value> =
        http::get_json(&format!("{PHP_RELEASES}releases.json"))?;
    let mut releases = Vec::new();
    for entry in data.values() {
        let Some(version) = entry.get("version").and_then(|v| v.as_str()) else {
            continue;
        };
        // Build Non-Thread-Safe x64 (untuk FastCGI), mis. `nts-vs16-x64`.
        let Some(build) = entry.as_object().and_then(|obj| {
            obj.iter()
                .find(|(key, _)| key.starts_with("nts-") && key.ends_with("-x64"))
                .and_then(|(_, value)| serde_json::from_value::<PhpBuild>(value.clone()).ok())
        }) else {
            continue;
        };
        releases.push(Release {
            kind: Kind::Php,
            version: version.to_string(),
            url: format!("{PHP_RELEASES}{}", build.zip.path),
            checksum: Checksum::Sha256(build.zip.sha256),
            lts: false,
        });
    }
    Ok(releases)
}

/// Versi patch lama ada di folder `archives/` tanpa checksum di JSON.
fn php_archive_release(spec: &str) -> Option<Release> {
    let v = Version::parse(spec)?;
    if v.parts().len() < 3 {
        return None;
    }
    let compiler = match (v.major(), v.minor()) {
        (7, 2..=4) => "vc15",
        (8, 0..=3) => "vs16",
        (8, _) | (9.., _) => "vs17",
        _ => return None,
    };
    let version = v.to_string();
    Some(Release {
        kind: Kind::Php,
        url: format!("{PHP_RELEASES}archives/php-{version}-nts-Win32-{compiler}-x64.zip"),
        version,
        checksum: Checksum::None,
        lts: false,
    })
}

// --- Node --------------------------------------------------------------------

#[derive(Deserialize)]
struct NodeEntry {
    version: String,
    files: Vec<String>,
    lts: serde_json::Value,
}

fn node_releases() -> Result<Vec<Release>> {
    let entries: Vec<NodeEntry> = http::get_json(&format!("{NODE_DIST}index.json"))?;
    Ok(entries
        .into_iter()
        .filter(|e| e.files.iter().any(|f| f == "win-x64-zip"))
        .map(|e| {
            let version = e.version.trim_start_matches('v').to_string();
            Release {
                kind: Kind::Node,
                url: format!("{NODE_DIST}v{version}/node-v{version}-win-x64.zip"),
                checksum: Checksum::ShasumsFile(format!("{NODE_DIST}v{version}/SHASUMS256.txt")),
                lts: e.lts.is_string(),
                version,
            }
        })
        .collect())
}

// --- Composer ----------------------------------------------------------------

#[derive(Deserialize)]
struct ComposerChannels {
    stable: Vec<ComposerEntry>,
    #[serde(rename = "2.2", default)]
    lts: Vec<ComposerEntry>,
}

#[derive(Deserialize)]
struct ComposerEntry {
    version: String,
}

fn composer_releases() -> Result<Vec<Release>> {
    let channels: ComposerChannels = http::get_json(&format!("{COMPOSER}/versions"))?;
    let entry = |e: ComposerEntry, lts: bool| Release {
        kind: Kind::Composer,
        url: format!("{COMPOSER}/download/{}/composer.phar", e.version),
        checksum: Checksum::ShasumsFile(format!(
            "{COMPOSER}/download/{}/composer.phar.sha256sum",
            e.version
        )),
        version: e.version,
        lts,
    };
    Ok(channels
        .stable
        .into_iter()
        .map(|e| entry(e, false))
        .chain(channels.lts.into_iter().map(|e| entry(e, true)))
        .collect())
}

// --- nginx -------------------------------------------------------------------

fn nginx_releases() -> Result<Vec<Release>> {
    let html = http::get_string(NGINX_DOWNLOAD)?;
    let mut versions: Vec<String> = html
        .split("href=\"nginx-")
        .skip(1)
        .filter_map(|rest| rest.split_once(".zip\"").map(|(v, _)| v.to_string()))
        .filter(|v| Version::parse(v).is_some_and(|p| p >= Version::parse("1.20").unwrap()))
        .collect();
    versions.dedup();
    Ok(versions
        .into_iter()
        .map(|version| Release {
            kind: Kind::Nginx,
            url: format!("{NGINX_DOWNLOAD}nginx-{version}.zip"),
            // Versi minor genap = cabang stable.
            lts: Version::parse(&version).is_some_and(|v| v.minor() % 2 == 0),
            checksum: Checksum::None,
            version,
        })
        .collect())
}

// --- MySQL / MariaDB ---------------------------------------------------------

fn mysql_releases() -> Vec<Release> {
    MYSQL_RELEASES
        .iter()
        .map(|(version, url)| Release {
            kind: Kind::Mysql,
            version: version.to_string(),
            url: url.to_string(),
            checksum: Checksum::None,
            lts: version.starts_with("8.4") || version.starts_with("8.0"),
        })
        .collect()
}

#[derive(Deserialize)]
struct MariadbMajors {
    major_releases: Vec<MariadbMajor>,
}

#[derive(Deserialize)]
struct MariadbMajor {
    release_id: String,
    release_status: String,
    release_support_type: Option<String>,
}

#[derive(Deserialize)]
struct MariadbReleases {
    releases: BTreeMap<String, MariadbRelease>,
}

#[derive(Deserialize)]
struct MariadbRelease {
    release_id: String,
    files: Vec<MariadbFile>,
}

#[derive(Deserialize)]
struct MariadbFile {
    file_name: String,
    os: Option<String>,
    checksum: Option<BTreeMap<String, Option<String>>>,
}

/// `major = None` → semua major stable sejak 10.6.
fn mariadb_releases(major: Option<&str>) -> Result<Vec<Release>> {
    let majors: Vec<(String, bool)> = match major {
        Some(m) => vec![(m.to_string(), false)],
        None => {
            let list: MariadbMajors = http::get_json(MARIADB_API)?;
            list.major_releases
                .into_iter()
                .filter(|m| m.release_status == "Stable")
                .filter(|m| Version::parse(&m.release_id) >= Version::parse("10.6"))
                .map(|m| {
                    let lts = m.release_support_type.as_deref() == Some("Long Term Support");
                    (m.release_id, lts)
                })
                .collect()
        }
    };

    let mut releases = Vec::new();
    for (major, lts) in majors {
        let data: MariadbReleases = http::get_json(&format!("{MARIADB_API}{major}/"))
            .with_context(|| format!("gagal mengambil daftar MariaDB {major}"))?;
        let per_major = data.releases.into_values().filter_map(|release| {
            let file = release.files.into_iter().find(|f| {
                f.os.as_deref() == Some("Windows") && f.file_name.ends_with("-winx64.zip")
            })?;
            Some(Release {
                kind: Kind::Mariadb,
                url: format!("{MARIADB_API}{}/{}", release.release_id, file.file_name),
                checksum: file
                    .checksum
                    .as_ref()
                    .and_then(|sums| sums.get("sha256sum").cloned().flatten())
                    .map_or(Checksum::None, Checksum::Sha256),
                version: release.release_id,
                lts,
            })
        });
        releases.extend(per_major);
    }
    Ok(releases)
}

// --- PostgreSQL --------------------------------------------------------------

#[derive(Deserialize)]
struct PostgresVersion {
    major: String,
    #[serde(rename = "latestMinor")]
    latest_minor: String,
    supported: bool,
}

/// Versi minor terbaru tiap major yang masih didukung upstream.
fn postgresql_releases() -> Result<Vec<Release>> {
    let versions: Vec<PostgresVersion> = http::get_json(POSTGRES_VERSIONS)?;
    Ok(versions
        .into_iter()
        .filter(|v| v.supported && v.major.parse::<u32>().is_ok_and(|m| m >= 13))
        .filter_map(|v| {
            let version = format!("{}.{}", v.major, v.latest_minor);
            let url = postgresql_url(&version)?;
            Some(Release {
                kind: Kind::Postgresql,
                version,
                url,
                // EDB tidak menerbitkan checksum untuk zip binary.
                checksum: Checksum::None,
                lts: false,
            })
        })
        .collect())
}

/// EDB kadang membangun ulang satu versi (`-1`, `-2`, ...); ambil build tertinggi yang ada.
fn postgresql_url(version: &str) -> Option<String> {
    (1..=4)
        .rev()
        .map(|build| postgresql_zip_url(version, build))
        .find(|url| http::exists(url))
}

fn postgresql_zip_url(version: &str, build: u32) -> String {
    format!("{POSTGRES_DOWNLOAD}postgresql-{version}-{build}-windows-x64-binaries.zip")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn php_archive_compiler() {
        let r = php_archive_release("7.4.30").unwrap();
        assert!(r.url.ends_with("php-7.4.30-nts-Win32-vc15-x64.zip"));
        let r = php_archive_release("8.2.10").unwrap();
        assert!(r.url.ends_with("php-8.2.10-nts-Win32-vs16-x64.zip"));
        let r = php_archive_release("8.4.1").unwrap();
        assert!(r.url.ends_with("php-8.4.1-nts-Win32-vs17-x64.zip"));
        assert!(php_archive_release("8.2").is_none());
    }

    #[test]
    fn postgresql_zip_name() {
        assert_eq!(
            postgresql_zip_url("16.15", 2),
            "https://get.enterprisedb.com/postgresql/postgresql-16.15-2-windows-x64-binaries.zip"
        );
    }

    #[test]
    fn release_file_name() {
        let r = &mysql_releases()[0];
        assert_eq!(r.file_name(), "mysql-9.6.0-winx64.zip");
    }
}
