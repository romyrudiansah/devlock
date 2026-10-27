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

/// Ekstensi yang dimuat sebagai `zend_extension`, bukan `extension`.
const ZEND_EXTENSIONS: &[&str] = &["opcache", "xdebug"];

/// Ekstensi PHP yang tersedia di `ext\` beserta statusnya di `php.ini`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Extension {
    pub name: String,
    pub enabled: bool,
    pub zend: bool,
}

/// Semua `ext\php_*.dll` di folder PHP, urut nama, dengan status aktif dari `php.ini`.
pub fn extensions(php_dir: &Path) -> Result<Vec<Extension>> {
    let ini = std::fs::read_to_string(php_dir.join("php.ini")).unwrap_or_default();
    let active = active_extensions(&ini);
    let mut list: Vec<Extension> = std::fs::read_dir(php_dir.join("ext"))
        .with_context(|| format!("folder ext tidak ada di {}", php_dir.display()))?
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let file = e.file_name().into_string().ok()?;
            let name = file.strip_prefix("php_")?.strip_suffix(".dll")?.to_string();
            Some(Extension {
                enabled: active.contains(&name),
                zend: ZEND_EXTENSIONS.contains(&name.as_str()),
                name,
            })
        })
        .collect();
    list.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(list)
}

/// Nama ekstensi dari baris aktif `extension=` / `zend_extension=`.
fn active_extensions(ini: &str) -> Vec<String> {
    ini.lines()
        .map(str::trim)
        .filter_map(|line| {
            let value = line
                .strip_prefix("zend_extension")
                .or_else(|| line.strip_prefix("extension"))?
                .trim_start()
                .strip_prefix('=')?
                .trim()
                .trim_matches('"');
            Some(extension_name(&format!("extension={value}")).to_string())
        })
        .collect()
}

/// Aktifkan / nonaktifkan ekstensi di `php.ini` folder PHP.
pub fn set_extension(php_dir: &Path, name: &str, enabled: bool) -> Result<()> {
    if !php_dir
        .join("ext")
        .join(format!("php_{name}.dll"))
        .is_file()
    {
        anyhow::bail!(
            "ekstensi `{name}` tidak ada di {}",
            php_dir.join("ext").display()
        );
    }
    let ini = php_dir.join("php.ini");
    let text = std::fs::read_to_string(&ini)
        .with_context(|| format!("gagal membaca {}", ini.display()))?;
    std::fs::write(&ini, with_extension(&text, name, enabled))
        .with_context(|| format!("gagal menulis {}", ini.display()))
}

/// Versi murni dari [`set_extension`]: baris aktif dikomentari (`;`) saat dimatikan; saat
/// dinyalakan, baris komentar pertama diaktifkan atau baris baru ditambahkan.
pub fn with_extension(text: &str, name: &str, enabled: bool) -> String {
    let directive = if ZEND_EXTENSIONS.contains(&name) {
        "zend_extension"
    } else {
        "extension"
    };
    let refers_to = |line: &str| {
        let bare = line.trim().trim_start_matches(';').trim();
        active_extensions(bare).first().is_some_and(|n| n == name)
    };
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    if enabled {
        if lines
            .iter()
            .any(|l| !l.trim_start().starts_with(';') && refers_to(l))
        {
            return text.to_string();
        }
        match lines
            .iter()
            .position(|l| l.trim_start().starts_with(';') && refers_to(l))
        {
            Some(i) => lines[i] = lines[i].trim().trim_start_matches(';').trim().to_string(),
            None => lines.push(format!("{directive}={name}")),
        }
    } else {
        for line in lines.iter_mut() {
            if !line.trim_start().starts_with(';') && refers_to(line) {
                *line = format!(";{}", line.trim());
            }
        }
    }
    let mut out = lines.join("\n");
    out.push('\n');
    out
}

/// Nilai `key` aktif di isi `php.ini` (tanpa tanda kutip), bila ada.
pub fn ini_value(text: &str, key: &str) -> Option<String> {
    text.lines().rev().find_map(|line| {
        let line = line.trim();
        let rest = line.strip_prefix(key)?.trim_start().strip_prefix('=')?;
        let value = rest.split(';').next().unwrap_or_default().trim();
        Some(value.trim_matches('"').to_string())
    })
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
    fn toggles_extensions() {
        let ini =
            ";extension=bz2\nextension=curl\n;extension=php_intl.dll\n;zend_extension=opcache\n";
        let on = with_extension(ini, "bz2", true);
        assert!(on.starts_with("extension=bz2\n"));
        let off = with_extension(&on, "curl", false);
        assert!(off.contains("\n;extension=curl\n"));
        let intl = with_extension(&off, "intl", true);
        assert!(intl.contains("\nextension=php_intl.dll\n"));
        let opcache = with_extension(&intl, "opcache", true);
        assert!(opcache.contains("\nzend_extension=opcache\n"));
        let xdebug = with_extension(&opcache, "xdebug", true);
        assert!(xdebug.ends_with("\nzend_extension=xdebug\n"));
        assert_eq!(with_extension(&xdebug, "xdebug", true), xdebug);
        let active = active_extensions(&xdebug);
        assert_eq!(active, ["bz2", "intl", "opcache", "xdebug"]);
    }

    #[test]
    fn reads_ini_values() {
        let ini = ";memory_limit = 1G\nmemory_limit = 512M ; komentar\ndate.timezone = \"Asia/Jakarta\"\n";
        assert_eq!(ini_value(ini, "memory_limit").as_deref(), Some("512M"));
        assert_eq!(
            ini_value(ini, "date.timezone").as_deref(),
            Some("Asia/Jakarta")
        );
        assert_eq!(ini_value(ini, "display_errors"), None);
    }

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
