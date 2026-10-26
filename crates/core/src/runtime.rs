//! Jenis runtime dan versi yang sudah terinstall di `runtimes\`.

use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;

use anyhow::{bail, Context, Result};

use crate::paths::Paths;
use crate::version;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Kind {
    Php,
    Node,
    Composer,
    Nginx,
    Mysql,
    Mariadb,
}

impl Kind {
    pub const ALL: [Kind; 6] = [
        Kind::Php,
        Kind::Node,
        Kind::Composer,
        Kind::Nginx,
        Kind::Mysql,
        Kind::Mariadb,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Php => "php",
            Kind::Node => "node",
            Kind::Composer => "composer",
            Kind::Nginx => "nginx",
            Kind::Mysql => "mysql",
            Kind::Mariadb => "mariadb",
        }
    }

    pub fn is_database(self) -> bool {
        matches!(self, Kind::Mysql | Kind::Mariadb)
    }
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Kind {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        let kind = match s.to_ascii_lowercase().as_str() {
            "php" => Kind::Php,
            "node" | "nodejs" => Kind::Node,
            "composer" => Kind::Composer,
            "nginx" => Kind::Nginx,
            "mysql" => Kind::Mysql,
            "mariadb" => Kind::Mariadb,
            other => bail!(
                "runtime `{other}` tidak dikenal (pilihan: {})",
                Kind::ALL.map(Kind::as_str).join(", ")
            ),
        };
        Ok(kind)
    }
}

/// Versi terinstall, terbaru lebih dulu.
pub fn installed_versions(paths: &Paths, kind: Kind) -> Result<Vec<String>> {
    let dir = paths.runtimes().join(kind.as_str());
    let entries = match std::fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e).with_context(|| format!("gagal membaca {}", dir.display())),
    };
    let mut versions: Vec<String> = entries
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().is_dir())
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| !name.starts_with('.') && version::Version::parse(name).is_some())
        .collect();
    version::sort_desc(&mut versions);
    Ok(versions)
}

/// Versi terinstall tertinggi yang cocok dengan `spec` (mis. `8.3` → `8.3.12`).
pub fn resolve_installed(paths: &Paths, kind: Kind, spec: &str) -> Result<Option<String>> {
    let versions = installed_versions(paths, kind)?;
    Ok(version::pick(spec, versions.iter().map(String::as_str)).map(str::to_string))
}

/// Folder versi terinstall; error jelas bila belum terinstall.
pub fn require_installed(paths: &Paths, kind: Kind, spec: &str) -> Result<(String, PathBuf)> {
    match resolve_installed(paths, kind, spec)? {
        Some(version) => {
            let dir = paths.runtime_dir(kind.as_str(), &version);
            Ok((version, dir))
        }
        None => bail!("{kind} {spec} belum terinstall; jalankan `devlock install {kind} {spec}`"),
    }
}

pub fn uninstall(paths: &Paths, kind: Kind, version: &str) -> Result<PathBuf> {
    let dir = paths.runtime_dir(kind.as_str(), version);
    if !dir.is_dir() {
        bail!("{kind} {version} tidak terinstall");
    }
    std::fs::remove_dir_all(&dir).with_context(|| format!("gagal menghapus {}", dir.display()))?;
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_and_resolves_installed() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path());
        for v in ["8.2.24", "8.3.12", "8.3.9", ".8.4.1.tmp"] {
            std::fs::create_dir_all(paths.runtime_dir("php", v)).unwrap();
        }
        assert_eq!(
            installed_versions(&paths, Kind::Php).unwrap(),
            ["8.3.12", "8.3.9", "8.2.24"]
        );
        assert_eq!(
            resolve_installed(&paths, Kind::Php, "8.3")
                .unwrap()
                .as_deref(),
            Some("8.3.12")
        );
        assert!(require_installed(&paths, Kind::Php, "7.4").is_err());
        assert!(installed_versions(&paths, Kind::Node).unwrap().is_empty());
    }

    #[test]
    fn parses_kind() {
        assert_eq!("NodeJS".parse::<Kind>().unwrap(), Kind::Node);
        assert!("python".parse::<Kind>().is_err());
    }
}
