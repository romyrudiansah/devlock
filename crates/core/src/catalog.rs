//! Katalog versi yang bisa diinstall, diambil dari sumber resmi masing-masing runtime.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::http;
use crate::paths::Paths;
use crate::runtime::Kind;
use crate::version::{self, Version};

const PHP_RELEASES: &str = "https://downloads.php.net/~windows/releases/";
const NODE_DIST: &str = "https://nodejs.org/dist/";
const NGINX_DOWNLOAD: &str = "https://nginx.org/download/";
/// Build Redis untuk Windows (komunitas, msys2); Redis resmi tidak merilis binary Windows.
const REDIS_WINDOWS: &str =
    "https://api.github.com/repos/redis-windows/redis-windows/releases?per_page=100";
/// Garnet: server cache berprotokol Redis dari Microsoft, build Windows resmi.
const GARNET: &str = "https://api.github.com/repos/microsoft/garnet/releases?per_page=30";
/// Adminer resmi: satu file PHP per rilis.
const ADMINER: &str = "https://api.github.com/repos/vrana/adminer/releases?per_page=30";
const MAILPIT: &str = "https://api.github.com/repos/axllent/mailpit/releases?per_page=30";
/// Rilis MongoDB yang masih didukung (satu per cabang), lengkap dengan SHA256 per arsip.
const MONGODB_CURRENT: &str = "https://downloads.mongodb.org/current.json";
const MONGOSH: &str = "https://api.github.com/repos/mongodb-js/mongosh/releases?per_page=30";
/// Build Apache httpd Windows (Win64, VS) dari Apache Lounge; hanya rilis terbaru yang tersedia.
const APACHE_LOUNGE: &str = "https://www.apachelounge.com";
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Checksum {
    None,
    Sha256(String),
    /// File berisi daftar `<sha256>  <nama file>` (mis. SHASUMS256.txt Node).
    ShasumsFile(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

/// Umur maksimal cache katalog sebelum diambil ulang dari sumber resmi.
pub const CACHE_TTL: Duration = Duration::from_secs(6 * 60 * 60);

fn cache_file(paths: &Paths, kind: Kind) -> PathBuf {
    paths.tmp().join("catalog").join(format!("{kind}.json"))
}

/// Seperti [`available`], tapi memakai cache `tmp\catalog\<kind>.json` yang belum lewat
/// [`CACHE_TTL`]. Bila sumber resmi gagal dihubungi, cache lama tetap dipakai.
pub fn available_cached(paths: &Paths, kind: Kind, refresh: bool) -> Result<Vec<Release>> {
    let file = cache_file(paths, kind);
    let read_cache = || -> Option<Vec<Release>> {
        serde_json::from_str(&std::fs::read_to_string(&file).ok()?).ok()
    };
    let fresh = std::fs::metadata(&file)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.elapsed().ok())
        .is_some_and(|age| age < CACHE_TTL);
    if fresh && !refresh {
        if let Some(cached) = read_cache() {
            return Ok(cached);
        }
    }
    match available(kind) {
        Ok(releases) => {
            if let Some(dir) = file.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::write(&file, serde_json::to_string(&releases)?);
            Ok(releases)
        }
        Err(err) => match read_cache() {
            Some(stale) => {
                tracing::warn!("katalog {kind} memakai cache lama: {err:#}");
                Ok(stale)
            }
            None => Err(err),
        },
    }
}

/// Seperti [`find`], tapi mencari di katalog ter-cache dulu.
pub fn find_cached(paths: &Paths, kind: Kind, spec: &str) -> Result<Release> {
    if let Ok(releases) = available_cached(paths, kind, false) {
        if let Some(release) = best_match(&releases, spec) {
            return Ok(release.clone());
        }
    }
    find(kind, spec)
}

fn best_match<'a>(releases: &'a [Release], spec: &str) -> Option<&'a Release> {
    let lts_only = spec.eq_ignore_ascii_case("lts") || spec.starts_with("lts/");
    releases
        .iter()
        .filter(|r| {
            if lts_only {
                r.lts
            } else {
                version::matches(spec, &r.version)
            }
        })
        .max_by(|a, b| Version::parse(&a.version).cmp(&Version::parse(&b.version)))
}

/// Semua versi yang tersedia, terbaru lebih dulu.
pub fn available(kind: Kind) -> Result<Vec<Release>> {
    let mut releases = match kind {
        Kind::Php => php_releases()?,
        Kind::Node => node_releases()?,
        Kind::Composer => composer_releases()?,
        Kind::Nginx => nginx_releases()?,
        Kind::Apache => apache_releases()?,
        Kind::Mysql => mysql_releases(),
        Kind::Mariadb => mariadb_releases(None)?,
        Kind::Postgresql => postgresql_releases()?,
        Kind::Redis => github_releases(REDIS_WINDOWS, Kind::Redis, |v| {
            format!("Redis-{v}-Windows-x64-msys2.zip")
        })?,
        Kind::Garnet => github_releases(GARNET, Kind::Garnet, |_| {
            "win-x64-based-readytorun.zip".to_string()
        })?,
        Kind::Adminer => github_releases(ADMINER, Kind::Adminer, |v| format!("adminer-{v}.php"))?,
        Kind::Mailpit => github_releases(MAILPIT, Kind::Mailpit, |_| {
            "mailpit-windows-amd64.zip".to_string()
        })?,
        Kind::Mongodb => mongodb_from(http::get_json(MONGODB_CURRENT)?),
        Kind::Mongosh => github_releases(MONGOSH, Kind::Mongosh, |v| {
            format!("mongosh-{v}-win32-x64.zip")
        })?,
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
    if let Some(release) = best_match(&releases, spec) {
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

// --- GitHub Releases (Redis, Garnet) ----------------------------------------

#[derive(Deserialize)]
struct GithubRelease {
    tag_name: String,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    draft: bool,
    assets: Vec<GithubAsset>,
}

#[derive(Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
    /// `sha256:<hex>`, dihitung GitHub saat aset diunggah.
    #[serde(default)]
    digest: Option<String>,
}

/// Rilis stabil dari API GitHub Releases dengan aset bernama `asset(versi)`. Rilis tanpa
/// SHA256 dari GitHub dilewati agar setiap install terverifikasi.
fn github_releases(url: &str, kind: Kind, asset: impl Fn(&str) -> String) -> Result<Vec<Release>> {
    let releases: Vec<GithubRelease> = http::get_json(url)?;
    Ok(releases_from_github(releases, kind, asset))
}

fn releases_from_github(
    releases: Vec<GithubRelease>,
    kind: Kind,
    asset: impl Fn(&str) -> String,
) -> Vec<Release> {
    releases
        .into_iter()
        .filter(|r| !r.prerelease && !r.draft)
        .filter_map(|r| {
            let version = r.tag_name.trim_start_matches('v').to_string();
            Version::parse(&version)?;
            let wanted = asset(&version);
            let found = r.assets.into_iter().find(|a| a.name == wanted)?;
            let sha256 = found
                .digest
                .as_deref()?
                .strip_prefix("sha256:")?
                .to_string();
            Some(Release {
                kind,
                version,
                url: found.browser_download_url,
                checksum: Checksum::Sha256(sha256),
                lts: false,
            })
        })
        .collect()
}

// --- MongoDB -----------------------------------------------------------------

#[derive(Deserialize)]
struct MongoCatalog {
    versions: Vec<MongoVersion>,
}

#[derive(Deserialize)]
struct MongoVersion {
    version: String,
    #[serde(default)]
    production_release: bool,
    #[serde(default)]
    downloads: Vec<MongoDownload>,
}

#[derive(Deserialize)]
struct MongoDownload {
    #[serde(default)]
    target: String,
    #[serde(default)]
    arch: String,
    #[serde(default)]
    edition: String,
    archive: MongoArchive,
}

#[derive(Deserialize)]
struct MongoArchive {
    url: String,
    #[serde(default)]
    sha256: Option<String>,
}

/// Zip Community (`edition = base`) Windows x64 dari rilis produksi, wajib ber-SHA256.
fn mongodb_from(catalog: MongoCatalog) -> Vec<Release> {
    catalog
        .versions
        .into_iter()
        .filter(|v| v.production_release && Version::parse(&v.version).is_some())
        .filter_map(|v| {
            let download = v
                .downloads
                .into_iter()
                .find(|d| d.target == "windows" && d.arch == "x86_64" && d.edition == "base")?;
            let sha256 = download.archive.sha256?;
            download.archive.url.ends_with(".zip").then(|| Release {
                kind: Kind::Mongodb,
                version: v.version,
                url: download.archive.url,
                checksum: Checksum::Sha256(sha256.to_lowercase()),
                lts: false,
            })
        })
        .collect()
}

// --- .NET runtime (untuk Garnet) ---------------------------------------------

/// Metadata rilis resmi .NET per channel, mis. `.../10.0/releases.json`.
const DOTNET_RELEASES: &str = "https://builds.dotnet.microsoft.com/dotnet/release-metadata";

/// Zip runtime .NET portable (`dotnet-runtime-<v>-win-x64.zip`) beserta SHA512-nya.
#[derive(Debug, Clone, PartialEq)]
pub struct DotnetRuntime {
    pub version: String,
    pub url: String,
    pub sha512: String,
}

#[derive(Deserialize)]
struct DotnetChannel {
    #[serde(rename = "latest-runtime")]
    latest_runtime: String,
    releases: Vec<DotnetRelease>,
}

#[derive(Deserialize)]
struct DotnetRelease {
    #[serde(default)]
    runtime: Option<DotnetFiles>,
}

#[derive(Deserialize)]
struct DotnetFiles {
    version: String,
    files: Vec<DotnetFile>,
}

#[derive(Deserialize)]
struct DotnetFile {
    name: String,
    #[serde(default)]
    rid: String,
    url: String,
    hash: String,
}

/// Runtime .NET terbaru di `channel` (mis. `10.0`) untuk Windows x64.
pub fn dotnet_runtime(channel: &str) -> Result<DotnetRuntime> {
    let meta: DotnetChannel =
        http::get_json(&format!("{DOTNET_RELEASES}/{channel}/releases.json"))?;
    dotnet_runtime_from(meta)
        .with_context(|| format!("runtime .NET {channel} untuk win-x64 tidak ditemukan"))
}

fn dotnet_runtime_from(meta: DotnetChannel) -> Option<DotnetRuntime> {
    let runtime = meta
        .releases
        .into_iter()
        .filter_map(|r| r.runtime)
        .find(|r| r.version == meta.latest_runtime)?;
    let file = runtime
        .files
        .into_iter()
        .find(|f| f.rid == "win-x64" && f.name == "dotnet-runtime-win-x64.zip")?;
    Some(DotnetRuntime {
        version: runtime.version,
        url: file.url,
        sha512: file.hash.to_lowercase(),
    })
}

// --- Apache ------------------------------------------------------------------

fn apache_releases() -> Result<Vec<Release>> {
    let html = http::get_string(&format!("{APACHE_LOUNGE}/download/"))?;
    let mut releases = Vec::new();
    for path in apache_zip_paths(&html) {
        let Some(version) = apache_version(&path) else {
            continue;
        };
        let url = format!("{APACHE_LOUNGE}{path}");
        let sums = http::get_string(&format!("{url}.txt"))?;
        let file = path.rsplit('/').next().unwrap_or_default();
        let Some(sha256) = apache_sha256(&sums, file) else {
            bail!("SHA256 untuk {file} tidak ditemukan di {url}.txt");
        };
        releases.push(Release {
            kind: Kind::Apache,
            version,
            url,
            checksum: Checksum::Sha256(sha256),
            lts: true,
        });
    }
    if releases.is_empty() {
        bail!("tidak ada build Apache Win64 di {APACHE_LOUNGE}/download/");
    }
    Ok(releases)
}

/// `/download/VS18/binaries/httpd-2.4.69-261002-Win64-VS18.zip` dari halaman unduhan.
fn apache_zip_paths(html: &str) -> Vec<String> {
    let mut paths: Vec<String> = html
        .split("href=\"")
        .skip(1)
        .filter_map(|rest| rest.split_once('"').map(|(href, _)| href))
        .filter(|href| {
            href.contains("/binaries/httpd-") && href.contains("-Win64-") && href.ends_with(".zip")
        })
        .map(String::from)
        .collect();
    paths.dedup();
    paths
}

fn apache_version(path: &str) -> Option<String> {
    let file = path.rsplit('/').next()?;
    let version = file.strip_prefix("httpd-")?.split('-').next()?;
    Version::parse(version).map(|_| version.to_string())
}

/// File `.zip.txt` Apache Lounge: baris `SHA256-Checksum for: <file>:` lalu hash di baris berikutnya.
fn apache_sha256(text: &str, file: &str) -> Option<String> {
    let mut lines = text.lines().map(str::trim);
    while let Some(line) = lines.next() {
        if line.starts_with("SHA256-Checksum for:") && line.contains(file) {
            let hash = lines.next()?.to_ascii_lowercase();
            return (hash.len() == 64 && hash.chars().all(|c| c.is_ascii_hexdigit()))
                .then_some(hash);
        }
    }
    None
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
    fn mongodb_catalog() {
        let json = r#"{"versions": [
            {"version": "8.0.34", "production_release": true, "downloads": [
                {"target": "windows", "arch": "x86_64", "edition": "enterprise",
                 "archive": {"url": "https://x/mongodb-windows-x86_64-enterprise-8.0.34.zip", "sha256": "EE"}},
                {"target": "windows", "arch": "x86_64", "edition": "base",
                 "archive": {"url": "https://x/mongodb-windows-x86_64-8.0.34.zip", "sha256": "AB"}},
                {"target": "ubuntu2404", "arch": "x86_64", "edition": "base",
                 "archive": {"url": "https://x/linux.tgz", "sha256": "CD"}}
            ]},
            {"version": "8.1.0-rc0", "production_release": false, "downloads": []},
            {"version": "7.0.45", "production_release": true, "downloads": [
                {"target": "windows", "arch": "x86_64", "edition": "base",
                 "archive": {"url": "https://x/mongodb-windows-x86_64-7.0.45.zip"}}
            ]}
        ]}"#;
        let releases = mongodb_from(serde_json::from_str(json).unwrap());
        assert_eq!(releases.len(), 1);
        assert_eq!(releases[0].version, "8.0.34");
        assert!(releases[0]
            .url
            .ends_with("mongodb-windows-x86_64-8.0.34.zip"));
        assert!(matches!(&releases[0].checksum, Checksum::Sha256(s) if s == "ab"));
    }

    #[test]
    fn dotnet_runtime_metadata() {
        let json = r#"{"latest-runtime": "10.0.14", "releases": [
            {"runtime": {"version": "10.0.14", "files": [
                {"name": "dotnet-runtime-win-arm64.zip", "rid": "win-arm64", "url": "https://x/arm.zip", "hash": "AA"},
                {"name": "dotnet-runtime-win-x64.exe", "rid": "win-x64", "url": "https://x/rt.exe", "hash": "BB"},
                {"name": "dotnet-runtime-win-x64.zip", "rid": "win-x64", "url": "https://x/rt.zip", "hash": "CC"}
            ]}},
            {"runtime": null},
            {"runtime": {"version": "10.0.13", "files": []}}
        ]}"#;
        let rt = dotnet_runtime_from(serde_json::from_str(json).unwrap()).unwrap();
        assert_eq!(rt.version, "10.0.14");
        assert_eq!(rt.url, "https://x/rt.zip");
        assert_eq!(rt.sha512, "cc");
    }

    #[test]
    fn github_release_assets() {
        let json = r#"[
            {"tag_name": "8.10.3", "prerelease": false, "draft": false, "assets": [
                {"name": "Redis-8.10.3-Windows-x64-msys2.zip", "browser_download_url": "https://x/r.zip",
                 "digest": "sha256:af16df340525b431e9ec8dabfd625d5e7a233f82805b5418819904ea909c447f"},
                {"name": "Redis-8.10.3-Windows-x64-cygwin.zip", "browser_download_url": "https://x/c.zip",
                 "digest": "sha256:00"}]},
            {"tag_name": "8.12.0-rc1", "prerelease": true, "draft": false, "assets": []},
            {"tag_name": "7.0.0", "assets": [
                {"name": "Redis-7.0.0-Windows-x64-msys2.zip", "browser_download_url": "https://x/old.zip"}]}
        ]"#;
        let releases: Vec<GithubRelease> = serde_json::from_str(json).unwrap();
        let found = releases_from_github(releases, Kind::Redis, |v| {
            format!("Redis-{v}-Windows-x64-msys2.zip")
        });
        assert_eq!(found.len(), 1, "prerelease & rilis tanpa digest dilewati");
        assert_eq!(found[0].version, "8.10.3");
        assert_eq!(found[0].url, "https://x/r.zip");
        assert_eq!(
            found[0].checksum,
            Checksum::Sha256(
                "af16df340525b431e9ec8dabfd625d5e7a233f82805b5418819904ea909c447f".into()
            )
        );
    }

    #[test]
    fn apache_lounge_parsing() {
        let html = r#"<a href="/download/VS18/binaries/httpd-2.4.69-261002-Win64-VS18.zip">x</a>
            <a href="/download/VS18/binaries/httpd-2.4.69-261002-Win64-VS18.zip.asc">sig</a>
            <a href="/download/VS18/modules/mod_foo-Win64-VS18.zip">m</a>"#;
        let paths = apache_zip_paths(html);
        assert_eq!(
            paths,
            ["/download/VS18/binaries/httpd-2.4.69-261002-Win64-VS18.zip"]
        );
        assert_eq!(apache_version(&paths[0]).as_deref(), Some("2.4.69"));

        let sums = "SHA1-Checksum for: httpd-2.4.69-261002-Win64-VS18.zip:\r\nB737\r\n\r\n\
                    SHA256-Checksum for: httpd-2.4.69-261002-Win64-VS18.zip:\r\n\
                    77D715D0D9CEE731E2251E7DDC2090A7A8E6D41FEDEF2E9827CD918593C5A652\r\n";
        assert_eq!(
            apache_sha256(sums, "httpd-2.4.69-261002-Win64-VS18.zip").as_deref(),
            Some("77d715d0d9cee731e2251e7ddc2090a7a8e6d41fedef2e9827cd918593c5a652")
        );
        assert!(apache_sha256(sums, "other.zip").is_none());
    }

    #[test]
    fn release_file_name() {
        let r = &mysql_releases()[0];
        assert_eq!(r.file_name(), "mysql-9.6.0-winx64.zip");
    }
}
