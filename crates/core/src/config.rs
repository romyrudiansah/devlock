//! Konfigurasi global `devlock.toml` di root instalasi.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::paths::Paths;

/// Isi awal `devlock.toml` saat `devlock init`.
pub const DEFAULT_CONFIG: &str = r#"# Konfigurasi global DevLock.
# Path relatif dihitung dari folder root DevLock.

# Folder berisi project; tiap subfolder menjadi <nama>.<tld>
www = "www"
tld = "test"

# Web server default untuk site: "nginx", atau "apache" (Apache di belakang nginx,
# mendukung .htaccess; butuh `devlock install apache`). Bisa per project: `webserver`
# di .devlock.toml.
webserver = "nginx"

# Versi default bila project tidak menentukan sendiri.
# Contoh: php = "8.3", node = "22", database = "mariadb@11.4", cache = "redis@8"
[defaults]

[http]
listen = "127.0.0.1"
port = 80

# HTTPS lokal dengan sertifikat dari CA DevLock (`devlock ssl trust` sekali).
# Bisa juga per project: `https = true` di .devlock.toml.
# redirect = true mengalihkan HTTP ke HTTPS (per project: `https_redirect`).
[https]
enabled = false
port = 443
redirect = false

# Apache untuk site dengan webserver = "apache": hanya di 127.0.0.1, nginx meneruskan
# request site itu ke port ini.
[apache]
port = 8180

# DNS lokal untuk wildcard *.<tld>: semua subdomain langsung mengarah ke 127.0.0.1
# tanpa menulis file hosts per project. Aktifkan dengan `devlock dns enable` (UAC sekali).
[dns]
enabled = false

[php]
# Jumlah proses php-cgi per versi PHP
workers = 4
# php-cgi di-restart setelah melayani sejumlah request
max_requests = 500

[daemon]
port = 7433

[gui]
# Jalankan semua service saat aplikasi DevLock (GUI) dibuka
autostart_services = false
# Editor untuk tombol "Editor" (perintah atau path lengkap). Kosong = deteksi otomatis:
# VS Code, Cursor, Windsurf, Zed, Sublime Text, PhpStorm.
editor = ""

# Override port instance database, contoh:
# [ports]
# "mysql@8.4" = 3306
[ports]
"#;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub www: PathBuf,
    pub tld: String,
    pub webserver: String,
    /// Versi default per runtime, mis. `php = "8.3"`, `database = "mariadb@11.4"`.
    pub defaults: BTreeMap<String, String>,
    pub http: HttpConfig,
    pub https: HttpsConfig,
    pub apache: ApacheConfig,
    pub dns: DnsConfig,
    pub php: PhpConfig,
    pub daemon: DaemonConfig,
    pub gui: GuiConfig,
    /// Override port per instance, mis. `"mysql@8.4" = 3306`.
    pub ports: BTreeMap<String, u16>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct HttpConfig {
    pub listen: String,
    pub port: u16,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct HttpsConfig {
    /// HTTPS untuk semua site (project bisa menimpa lewat `https` di `.devlock.toml`).
    pub enabled: bool,
    pub port: u16,
    /// Alihkan HTTP ke HTTPS (301) untuk site ber-HTTPS; project bisa menimpa lewat
    /// `https_redirect` di `.devlock.toml`.
    pub redirect: bool,
}

impl Default for HttpsConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            port: 443,
            redirect: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ApacheConfig {
    /// Port internal Apache (`127.0.0.1`), di belakang nginx.
    pub port: u16,
}

impl Default for ApacheConfig {
    fn default() -> Self {
        Self { port: 8180 }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DnsConfig {
    /// Jalankan DNS lokal (`127.0.0.1:53`) yang menjawab `*.<tld>`.
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PhpConfig {
    pub workers: u16,
    pub max_requests: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DaemonConfig {
    pub port: u16,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            www: PathBuf::from("www"),
            tld: "test".into(),
            webserver: "nginx".into(),
            defaults: BTreeMap::new(),
            http: HttpConfig::default(),
            https: HttpsConfig::default(),
            apache: ApacheConfig::default(),
            dns: DnsConfig::default(),
            php: PhpConfig::default(),
            daemon: DaemonConfig::default(),
            gui: GuiConfig::default(),
            ports: BTreeMap::new(),
        }
    }
}

impl Default for HttpConfig {
    fn default() -> Self {
        Self {
            listen: "127.0.0.1".into(),
            port: 80,
        }
    }
}

impl Default for PhpConfig {
    fn default() -> Self {
        Self {
            workers: 4,
            max_requests: 500,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GuiConfig {
    /// Jalankan semua service saat GUI dibuka.
    pub autostart_services: bool,
    /// Perintah atau path editor; kosong = deteksi otomatis (lihat `editor::KNOWN`).
    pub editor: String,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self { port: 7433 }
    }
}

impl Config {
    /// Baca `devlock.toml`; file yang belum ada dianggap konfigurasi default.
    pub fn load(paths: &Paths) -> Result<Self> {
        Self::load_from(&paths.config_file())
    }

    pub fn load_from(file: &Path) -> Result<Self> {
        match std::fs::read_to_string(file) {
            Ok(text) => toml::from_str(&text)
                .with_context(|| format!("format {} tidak valid", file.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e).with_context(|| format!("gagal membaca {}", file.display())),
        }
    }

    /// Tulis `devlock.toml` default bila belum ada. Mengembalikan `true` jika file dibuat.
    pub fn write_default_if_missing(paths: &Paths) -> Result<bool> {
        let file = paths.config_file();
        if file.exists() {
            return Ok(false);
        }
        std::fs::write(&file, DEFAULT_CONFIG)
            .with_context(|| format!("gagal menulis {}", file.display()))?;
        Ok(true)
    }

    /// Ubah satu nilai `[defaults]` tanpa menghilangkan komentar di file.
    pub fn set_default(paths: &Paths, key: &str, value: &str) -> Result<()> {
        let file = paths.config_file();
        let text = std::fs::read_to_string(&file).unwrap_or_else(|_| DEFAULT_CONFIG.to_string());
        let mut doc: toml_edit::DocumentMut = text
            .parse()
            .with_context(|| format!("format {} tidak valid", file.display()))?;
        if !doc.contains_table("defaults") {
            doc["defaults"] = toml_edit::table();
        }
        doc["defaults"][key] = toml_edit::value(value);
        std::fs::write(&file, doc.to_string())
            .with_context(|| format!("gagal menulis {}", file.display()))
    }

    /// Ubah satu nilai `[table] key = value` tanpa menghilangkan komentar di file.
    pub fn set_value(paths: &Paths, table: &str, key: &str, value: toml_edit::Item) -> Result<()> {
        let file = paths.config_file();
        let text = std::fs::read_to_string(&file).unwrap_or_else(|_| DEFAULT_CONFIG.to_string());
        let mut doc: toml_edit::DocumentMut = text
            .parse()
            .with_context(|| format!("format {} tidak valid", file.display()))?;
        if !doc.contains_table(table) {
            doc[table] = toml_edit::table();
        }
        doc[table][key] = value;
        std::fs::write(&file, doc.to_string())
            .with_context(|| format!("gagal menulis {}", file.display()))
    }

    /// Ubah nilai top-level (mis. `www`, `tld`) tanpa menghilangkan komentar di file.
    pub fn set_top_level(paths: &Paths, key: &str, value: toml_edit::Item) -> Result<()> {
        let file = paths.config_file();
        let text = std::fs::read_to_string(&file).unwrap_or_else(|_| DEFAULT_CONFIG.to_string());
        let mut doc: toml_edit::DocumentMut = text
            .parse()
            .with_context(|| format!("format {} tidak valid", file.display()))?;
        doc[key] = value;
        std::fs::write(&file, doc.to_string())
            .with_context(|| format!("gagal menulis {}", file.display()))
    }

    pub fn www_dir(&self, paths: &Paths) -> PathBuf {
        paths.resolve(&self.www)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_template_matches_default_struct() {
        let parsed: Config = toml::from_str(DEFAULT_CONFIG).unwrap();
        assert_eq!(parsed, Config::default());
    }

    #[test]
    fn set_default_keeps_comments() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path());
        Config::write_default_if_missing(&paths).unwrap();
        Config::set_default(&paths, "php", "8.2").unwrap();

        let text = std::fs::read_to_string(paths.config_file()).unwrap();
        assert!(text.contains("# Konfigurasi global DevLock."));
        let config = Config::load(&paths).unwrap();
        assert_eq!(config.defaults.get("php").map(String::as_str), Some("8.2"));
    }

    #[test]
    fn set_value_in_table() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path());
        Config::write_default_if_missing(&paths).unwrap();
        Config::set_value(&paths, "https", "enabled", toml_edit::value(true)).unwrap();
        let config = Config::load(&paths).unwrap();
        assert!(config.https.enabled);
        assert_eq!(config.https.port, 443);
    }

    #[test]
    fn missing_file_is_default() {
        let tmp = tempfile::tempdir().unwrap();
        let config = Config::load(&Paths::new(tmp.path())).unwrap();
        assert_eq!(config, Config::default());
    }
}
