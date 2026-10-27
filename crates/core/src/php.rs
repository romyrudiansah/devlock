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

        if !ext_dir_set
            && uncommented.starts_with("extension_dir")
            && uncommented.contains("\"ext\"")
        {
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

/// Set `key = "value"` di `php.ini` folder PHP: baris aktif atau baris komentar (`;key =`)
/// pertama diganti; bila tidak ada, ditambahkan di akhir.
pub fn set_ini_value(php_dir: &Path, key: &str, value: &str) -> Result<()> {
    let ini = php_dir.join("php.ini");
    let text = std::fs::read_to_string(&ini)
        .with_context(|| format!("gagal membaca {}", ini.display()))?;
    std::fs::write(&ini, with_ini_value(&text, key, value))
        .with_context(|| format!("gagal menulis {}", ini.display()))
}

/// Versi murni dari [`set_ini_value`] untuk isi file.
pub fn with_ini_value(text: &str, key: &str, value: &str) -> String {
    let wanted = format!("{key} = \"{value}\"");
    let mut done = false;
    let mut out: Vec<String> = text
        .lines()
        .map(|line| {
            let bare = line.trim().trim_start_matches(';').trim_start();
            let matches = bare
                .strip_prefix(key)
                .is_some_and(|rest| rest.trim_start().starts_with('='));
            if !done && matches {
                done = true;
                wanted.clone()
            } else {
                line.to_string()
            }
        })
        .collect();
    if !done {
        out.push(wanted);
    }
    let mut joined = out.join("\n");
    joined.push('\n');
    joined
}

/// `extension=php_curl.dll` / `extension=curl` → `curl`
fn extension_name(line: &str) -> &str {
    let value = line.trim_start_matches("extension=");
    // Buang komentar di baris yang sama: `extension=exif  ; Must be after mbstring`.
    let value = value
        .split(|c: char| c == ';' || c.is_whitespace())
        .next()
        .unwrap_or_default();
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
;extension=exif      ; Must be after mbstring as it depends on it
;extension=php_mbstring.dll
;extension=pdo_mysql
";

    #[test]
    fn sets_ini_values() {
        let text = ";curl.cainfo =\n; comment\n;openssl.cafile=\nmemory_limit = 128M\n";
        let out = with_ini_value(text, "curl.cainfo", r"D:\DevLock\ssl\ca-bundle.pem");
        assert!(out.starts_with("curl.cainfo = \"D:\\DevLock\\ssl\\ca-bundle.pem\"\n"));
        let out = with_ini_value(&out, "openssl.cafile", "x.pem");
        assert!(out.contains("\nopenssl.cafile = \"x.pem\"\n"));
        let out = with_ini_value(&out, "curl.cainfo", "y.pem");
        assert_eq!(out.matches("curl.cainfo").count(), 1);
        let out = with_ini_value(&out, "xdebug.mode", "debug");
        assert!(out.ends_with("xdebug.mode = \"debug\"\n"));
        // `memory_limit` tidak boleh tersentuh oleh key lain yang berawalan sama.
        assert!(with_ini_value(&out, "memory", "1").contains("memory_limit = 128M"));
    }

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
