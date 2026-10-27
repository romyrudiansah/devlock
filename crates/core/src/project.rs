//! Konfigurasi per project `.devlock.toml`.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

pub const PROJECT_FILE: &str = ".devlock.toml";

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProjectConfig {
    pub php: Option<String>,
    pub node: Option<String>,
    /// Instance database, mis. `mysql@8.4` atau `mariadb@11.4`.
    pub database: Option<String>,
    /// Instance cache, mis. `redis@8` atau `garnet@2`.
    pub cache: Option<String>,
    pub webserver: Option<String>,
    /// Document root relatif ke folder project; default dideteksi otomatis.
    pub docroot: Option<PathBuf>,
    /// Domain tambahan selain `<folder>.<tld>`.
    pub aliases: Vec<String>,
    /// Perintah dev server (project Node), mis. `pnpm dev`.
    pub dev: Option<String>,
    /// Port dev server yang di-reverse-proxy dari domain project.
    pub proxy_port: Option<u16>,
    /// Layani juga lewat HTTPS; default mengikuti `[https] enabled` global.
    pub https: Option<bool>,
}

impl ProjectConfig {
    pub fn load(file: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(file)
            .with_context(|| format!("gagal membaca {}", file.display()))?;
        toml::from_str(&text).with_context(|| format!("format {} tidak valid", file.display()))
    }

    /// Baca `.devlock.toml` di folder project; kosong bila tidak ada.
    pub fn load_dir(dir: &Path) -> Result<Self> {
        let file = dir.join(PROJECT_FILE);
        if file.is_file() {
            Self::load(&file)
        } else {
            Ok(Self::default())
        }
    }

    /// Set satu nilai di `<dir>/.devlock.toml` tanpa menghilangkan isi lain.
    pub fn set_value(dir: &Path, key: &str, value: &str) -> Result<PathBuf> {
        let file = dir.join(PROJECT_FILE);
        let text = std::fs::read_to_string(&file).unwrap_or_default();
        let mut doc: toml_edit::DocumentMut = text
            .parse()
            .with_context(|| format!("format {} tidak valid", file.display()))?;
        doc[key] = toml_edit::value(value);
        std::fs::write(&file, doc.to_string())
            .with_context(|| format!("gagal menulis {}", file.display()))?;
        Ok(file)
    }
}

/// Cari `.devlock.toml` terdekat dari `start` ke atas.
pub fn find_project_file(start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .map(|dir| dir.join(PROJECT_FILE))
        .find(|file| file.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_nearest_file_upwards() {
        let tmp = tempfile::tempdir().unwrap();
        let nested = tmp.path().join("app/src/deep");
        std::fs::create_dir_all(&nested).unwrap();
        ProjectConfig::set_value(&tmp.path().join("app"), "php", "8.2").unwrap();

        let file = find_project_file(&nested).unwrap();
        assert_eq!(file, tmp.path().join("app").join(PROJECT_FILE));
        let config = ProjectConfig::load(&file).unwrap();
        assert_eq!(config.php.as_deref(), Some("8.2"));
    }

    #[test]
    fn parses_full_file() {
        let config: ProjectConfig = toml::from_str(
            r#"
            php = "7.4"
            node = "20"
            database = "mysql@5.7"
            aliases = ["shop.local.test"]
            dev = "pnpm dev"
            proxy_port = 5173
            "#,
        )
        .unwrap();
        assert_eq!(config.proxy_port, Some(5173));
        assert_eq!(config.aliases, vec!["shop.local.test"]);
    }
}
