//! Penyiapan `php.ini` untuk versi PHP yang baru diinstall.

use std::path::Path;

use anyhow::{Context, Result};

/// Ekstensi yang diaktifkan secara default (umum dipakai Laravel / WordPress).
pub const DEFAULT_EXTENSIONS: &[&str] = &[
    "curl",
    "fileinfo",
    "gd",
    "intl",
    "mbstring",
    "exif",
    "mysqli",
    "openssl",
    "pdo_mysql",
    "pdo_pgsql",
    "pdo_sqlite",
    "sqlite3",
    "sodium",
    "zip",
];

const OVERRIDES: &[(&str, &str)] = &[
    ("memory_limit", "512M"),
    ("upload_max_filesize", "128M"),
    ("post_max_size", "128M"),
    ("max_execution_time", "120"),
];

/// Buat `php.ini` dari `php.ini-development` bila belum ada.
pub fn write_default_ini(php_dir: &Path) -> Result<()> {
    let ini = php_dir.join("php.ini");
    if ini.exists() {
        return Ok(());
    }
    let template = php_dir.join("php.ini-development");
    let text = std::fs::read_to_string(&template)
        .with_context(|| format!("gagal membaca {}", template.display()))?;
    let ext_dir = php_dir.join("ext");
    std::fs::write(&ini, configure_ini(&text, &ext_dir.to_string_lossy()))
        .with_context(|| format!("gagal menulis {}", ini.display()))
}

/// Aktifkan `extension_dir`, ekstensi default, dan naikkan batas umum.
pub fn configure_ini(template: &str, ext_dir: &str) -> String {
    let mut out = String::with_capacity(template.len() + 256);
    let mut ext_dir_set = false;
    for line in template.lines() {
        let trimmed = line.trim();
        let uncommented = trimmed.trim_start_matches(';').trim();

        if !ext_dir_set && uncommented.starts_with("extension_dir") && uncommented.contains("\"ext\"") {
            out.push_str(&format!("extension_dir = \"{ext_dir}\"\n"));
            ext_dir_set = true;
            continue;
        }
        if trimmed.starts_with(";extension=") {
            let name = extension_name(uncommented);
            if DEFAULT_EXTENSIONS.contains(&name) {
                out.push_str(uncommented);
                out.push('\n');
                continue;
            }
        }
        if let Some((key, value)) = override_for(trimmed) {
            out.push_str(&format!("{key} = {value}\n"));
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    if !ext_dir_set {
        out.push_str(&format!("\nextension_dir = \"{ext_dir}\"\n"));
    }
    out
}

/// `extension=php_curl.dll` / `extension=curl` → `curl`
fn extension_name(line: &str) -> &str {
    let value = line.trim_start_matches("extension=").trim();
    let value = value.strip_prefix("php_").unwrap_or(value);
    value.strip_suffix(".dll").unwrap_or(value)
}

fn override_for(line: &str) -> Option<(&'static str, &'static str)> {
    let (key, _) = line.split_once('=')?;
    let key = key.trim();
    OVERRIDES.iter().copied().find(|(k, _)| *k == key)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
memory_limit = 128M
; On windows:
;extension_dir = \"ext\"
;extension=bz2
;extension=curl
;extension=php_mbstring.dll
;extension=pdo_mysql
";

    #[test]
    fn enables_defaults() {
        let ini = configure_ini(SAMPLE, "D:\\DevLock\\runtimes\\php\\8.3.12\\ext");
        assert!(ini.contains("memory_limit = 512M"));
        assert!(ini.contains("extension_dir = \"D:\\DevLock\\runtimes\\php\\8.3.12\\ext\""));
        assert!(ini.contains("\nextension=curl\n"));
        assert!(ini.contains("\nextension=php_mbstring.dll\n"));
        assert!(ini.contains("\nextension=pdo_mysql\n"));
        assert!(ini.contains(";extension=bz2"));
    }
}
