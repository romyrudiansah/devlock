//! Menentukan versi runtime yang dipakai di sebuah folder.
//!
//! Urutan: variabel `DEVLOCK_<KIND>_VERSION` → file project terdekat (naik ke atas,
//! per folder: `.devlock.toml` lalu file khas ekosistem) → `[defaults]` global →
//! versi terinstall tertinggi.

use std::fmt;
use std::path::{Path, PathBuf};

use anyhow::{bail, Result};

use crate::config::Config;
use crate::paths::Paths;
use crate::project::{ProjectConfig, PROJECT_FILE};
use crate::runtime::{self, Kind};
use crate::version::{self, Version};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    Env(String),
    File(PathBuf),
    Global,
    /// Tidak ada yang menentukan; dipakai versi terinstall tertinggi.
    Fallback,
}

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Source::Env(name) => write!(f, "variabel {name}"),
            Source::File(path) => write!(f, "{}", path.display()),
            Source::Global => f.write_str("default global (devlock.toml)"),
            Source::Fallback => f.write_str("versi terinstall tertinggi"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Resolved {
    pub kind: Kind,
    pub spec: String,
    pub version: String,
    pub dir: PathBuf,
    pub source: Source,
}

pub fn env_var(kind: Kind) -> String {
    format!("DEVLOCK_{}_VERSION", kind.as_str().to_uppercase())
}

/// Spec versi (belum dicocokkan ke versi terinstall) beserta asalnya.
pub fn find_spec(config: &Config, kind: Kind, cwd: &Path) -> Option<(String, Source)> {
    let var = env_var(kind);
    if let Ok(spec) = std::env::var(&var) {
        if !spec.trim().is_empty() {
            return Some((spec.trim().to_string(), Source::Env(var)));
        }
    }
    for dir in cwd.ancestors() {
        if let Some(found) = spec_in_dir(kind, dir) {
            return Some(found);
        }
    }
    config
        .defaults
        .get(kind.as_str())
        .map(|spec| (spec.clone(), Source::Global))
}

fn spec_in_dir(kind: Kind, dir: &Path) -> Option<(String, Source)> {
    let project_file = dir.join(PROJECT_FILE);
    if project_file.is_file() {
        if let Ok(project) = ProjectConfig::load(&project_file) {
            let spec = match kind {
                Kind::Php => project.php,
                Kind::Node => project.node,
                _ => None,
            };
            if let Some(spec) = spec {
                return Some((spec, Source::File(project_file)));
            }
        }
    }
    match kind {
        Kind::Php => {
            let file = dir.join("composer.json");
            let spec = read_json(&file).and_then(|json| {
                let platform = json.pointer("/config/platform/php")?.as_str()?;
                Version::parse(platform).map(|v| v.major_minor())
            })?;
            Some((spec, Source::File(file)))
        }
        Kind::Node => {
            for name in [".nvmrc", ".node-version"] {
                let file = dir.join(name);
                if let Ok(text) = std::fs::read_to_string(&file) {
                    let line = text
                        .lines()
                        .map(str::trim)
                        .find(|l| !l.is_empty() && !l.starts_with('#'));
                    if let Some(line) = line {
                        return Some((normalize_node_spec(line), Source::File(file)));
                    }
                }
            }
            let file = dir.join("package.json");
            let spec = read_json(&file)
                .and_then(|json| json.pointer("/volta/node")?.as_str().map(str::to_string))?;
            Some((spec, Source::File(file)))
        }
        _ => None,
    }
}

fn read_json(file: &Path) -> Option<serde_json::Value> {
    let text = std::fs::read_to_string(file).ok()?;
    serde_json::from_str(&text).ok()
}

/// `v20` → `20`, `lts/iron` → `20`, `lts/*` → `lts`, `node`/`stable` → `latest`.
pub fn normalize_node_spec(spec: &str) -> String {
    let spec = spec.trim().to_ascii_lowercase();
    if spec == "node" || spec == "stable" || spec == "current" {
        return "latest".into();
    }
    if let Some(name) = spec.strip_prefix("lts/") {
        let major = match name {
            "hydrogen" => "18",
            "iron" => "20",
            "jod" => "22",
            "krypton" => "24",
            _ => "lts",
        };
        return major.into();
    }
    spec.trim_start_matches('v').to_string()
}

/// Cocokkan spec ke versi terinstall. `lts` = major genap tertinggi (aturan rilis Node).
fn pick_installed(paths: &Paths, kind: Kind, spec: &str) -> Result<Option<String>> {
    let installed = runtime::installed_versions(paths, kind)?;
    if spec == "lts" {
        return Ok(installed
            .iter()
            .find(|v| Version::parse(v).is_some_and(|p| p.major() % 2 == 0))
            .cloned());
    }
    Ok(version::pick(spec, installed.iter().map(String::as_str)).map(str::to_string))
}

pub fn resolve(paths: &Paths, config: &Config, kind: Kind, cwd: &Path) -> Result<Resolved> {
    let (spec, source) =
        find_spec(config, kind, cwd).unwrap_or(("latest".into(), Source::Fallback));
    let Some(version) = pick_installed(paths, kind, &spec)? else {
        if source == Source::Fallback {
            bail!("belum ada {kind} terinstall; jalankan `devlock install {kind} <versi>`");
        }
        bail!("{kind} {spec} (dari {source}) belum terinstall; jalankan `devlock install {kind} {spec}`");
    };
    Ok(Resolved {
        kind,
        dir: paths.runtime_dir(kind.as_str(), &version),
        spec,
        version,
        source,
    })
}

/// Instance database seperti `mysql@8.4`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DbInstance {
    pub kind: Kind,
    /// major.minor, juga nama folder data.
    pub line: String,
}

impl DbInstance {
    pub fn parse(text: &str) -> Result<Self> {
        let (engine, spec) = text.split_once('@').unwrap_or((text, ""));
        let kind: Kind = engine.trim().parse()?;
        if !kind.is_database() {
            bail!("`{engine}` bukan engine database (pilihan: mysql, mariadb)");
        }
        let Some(v) = Version::parse(spec).filter(|v| v.parts().len() >= 2) else {
            bail!("versi database harus major.minor, mis. `{engine}@8.4`");
        };
        Ok(Self {
            kind,
            line: v.major_minor(),
        })
    }

    pub fn name(&self) -> String {
        format!("{}@{}", self.kind, self.line)
    }

    /// Port default: MySQL `3300 + major*10 + minor` (8.4 → 3384),
    /// MariaDB `3400 + (major-10)*20 + minor` (11.4 → 3424). Bisa ditimpa `[ports]`.
    pub fn port(&self, config: &Config) -> u16 {
        if let Some(port) = config.ports.get(&self.name()) {
            return *port;
        }
        let v = Version::parse(&self.line).unwrap_or_else(|| Version::parse("0.0").unwrap());
        let (major, minor) = (v.major() as u16, v.minor() as u16);
        match self.kind {
            Kind::Mariadb => 3400 + major.saturating_sub(10) * 20 + minor,
            _ => 3300 + major * 10 + minor,
        }
    }

    pub fn data_dir(&self, paths: &Paths) -> PathBuf {
        paths.data().join(self.kind.as_str()).join(&self.line)
    }
}

/// Instance database untuk folder: `database` di `.devlock.toml` terdekat, lalu default global.
pub fn find_database(config: &Config, cwd: &Path) -> Result<Option<(DbInstance, Source)>> {
    for dir in cwd.ancestors() {
        let file = dir.join(PROJECT_FILE);
        if file.is_file() {
            if let Some(db) = ProjectConfig::load(&file)?.database {
                return Ok(Some((DbInstance::parse(&db)?, Source::File(file))));
            }
        }
    }
    match config.defaults.get("database") {
        Some(db) => Ok(Some((DbInstance::parse(db)?, Source::Global))),
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (tempfile::TempDir, Paths) {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().join("root"));
        for (kind, v) in [
            ("php", "8.2.24"),
            ("php", "8.3.12"),
            ("node", "20.18.0"),
            ("node", "22.11.0"),
            ("node", "23.1.0"),
        ] {
            std::fs::create_dir_all(paths.runtime_dir(kind, v)).unwrap();
        }
        (tmp, paths)
    }

    #[test]
    fn project_file_beats_global() {
        let (tmp, paths) = setup();
        let project = tmp.path().join("www/blog");
        std::fs::create_dir_all(project.join("src")).unwrap();
        std::fs::write(project.join(PROJECT_FILE), "php = \"8.2\"\n").unwrap();
        let mut config = Config::default();
        config.defaults.insert("php".into(), "8.3".into());

        let r = resolve(&paths, &config, Kind::Php, &project.join("src")).unwrap();
        assert_eq!(r.version, "8.2.24");
        assert_eq!(r.source, Source::File(project.join(PROJECT_FILE)));

        let r = resolve(&paths, &config, Kind::Php, tmp.path()).unwrap();
        assert_eq!((r.version.as_str(), r.source), ("8.3.12", Source::Global));
    }

    #[test]
    fn ecosystem_files() {
        let (tmp, paths) = setup();
        let app = tmp.path().join("app");
        std::fs::create_dir_all(&app).unwrap();
        std::fs::write(app.join(".nvmrc"), "lts/*\n").unwrap();
        std::fs::write(
            app.join("composer.json"),
            r#"{"config":{"platform":{"php":"8.2.0"}}}"#,
        )
        .unwrap();

        let config = Config::default();
        assert_eq!(
            resolve(&paths, &config, Kind::Node, &app).unwrap().version,
            "22.11.0"
        );
        assert_eq!(
            resolve(&paths, &config, Kind::Php, &app).unwrap().version,
            "8.2.24"
        );
    }

    #[test]
    fn missing_version_is_error() {
        let (tmp, paths) = setup();
        std::fs::write(tmp.path().join(".node-version"), "18\n").unwrap();
        let err = resolve(&paths, &Config::default(), Kind::Node, tmp.path()).unwrap_err();
        assert!(err.to_string().contains("devlock install node 18"));
    }

    #[test]
    fn node_spec_normalization() {
        assert_eq!(normalize_node_spec("v20.11.1"), "20.11.1");
        assert_eq!(normalize_node_spec("lts/iron"), "20");
        assert_eq!(normalize_node_spec("lts/*"), "lts");
        assert_eq!(normalize_node_spec("node"), "latest");
    }

    #[test]
    fn database_instances() {
        let mut config = Config::default();
        let mysql = DbInstance::parse("mysql@8.4.3").unwrap();
        assert_eq!(
            (mysql.name(), mysql.port(&config)),
            ("mysql@8.4".to_string(), 3384)
        );
        assert_eq!(DbInstance::parse("mysql@5.7").unwrap().port(&config), 3357);
        assert_eq!(
            DbInstance::parse("mariadb@11.4").unwrap().port(&config),
            3424
        );
        assert_eq!(
            DbInstance::parse("mariadb@10.11").unwrap().port(&config),
            3411
        );
        config.ports.insert("mysql@8.4".into(), 3306);
        assert_eq!(mysql.port(&config), 3306);
        assert!(DbInstance::parse("mysql").is_err());
        assert!(DbInstance::parse("php@8.3").is_err());
    }
}
