//! Ekspor/impor konfigurasi DevLock ke mesin lain: isi `devlock.toml` beserta daftar runtime
//! terinstall dalam satu file TOML. Project (dan `.devlock.toml`-nya) dipindah bersama
//! foldernya sendiri.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::paths::Paths;
use crate::runtime::{self, Kind};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuntimeEntry {
    pub kind: Kind,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    /// Versi DevLock yang membuat ekspor.
    pub devlock: String,
    /// Isi `devlock.toml` apa adanya (komentar ikut).
    pub config: String,
    #[serde(default)]
    pub runtimes: Vec<RuntimeEntry>,
}

impl Manifest {
    pub fn build(paths: &Paths) -> Result<Self> {
        let config = std::fs::read_to_string(paths.config_file()).with_context(|| {
            format!(
                "gagal membaca {} (jalankan `devlock init`)",
                paths.config_file().display()
            )
        })?;
        let mut runtimes = Vec::new();
        for kind in Kind::ALL {
            for version in runtime::installed_versions(paths, kind)? {
                runtimes.push(RuntimeEntry { kind, version });
            }
        }
        Ok(Self {
            devlock: env!("CARGO_PKG_VERSION").into(),
            config,
            runtimes,
        })
    }

    pub fn write(&self, file: &Path) -> Result<()> {
        let text = format!(
            "# Ekspor DevLock: `devlock config import {}` di komputer lain.\n{}",
            file.file_name().unwrap_or_default().to_string_lossy(),
            toml::to_string(self).context("gagal membuat file ekspor")?
        );
        std::fs::write(file, text).with_context(|| format!("gagal menulis {}", file.display()))
    }

    pub fn read(file: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(file)
            .with_context(|| format!("gagal membaca {}", file.display()))?;
        let manifest: Self = toml::from_str(&text)
            .with_context(|| format!("{} bukan file ekspor DevLock", file.display()))?;
        // Pastikan isi konfigurasi bisa dibaca sebelum menimpa milik pengguna.
        toml::from_str::<crate::config::Config>(&manifest.config)
            .context("devlock.toml di file ekspor tidak valid")?;
        Ok(manifest)
    }

    /// Tulis konfigurasi ke `devlock.toml`; yang lama disimpan sebagai `devlock.toml.bak`.
    pub fn apply_config(&self, paths: &Paths) -> Result<Option<PathBuf>> {
        let file = paths.config_file();
        let backup = if file.is_file() {
            let backup = file.with_extension("toml.bak");
            std::fs::copy(&file, &backup)
                .with_context(|| format!("gagal mencadangkan {}", file.display()))?;
            Some(backup)
        } else {
            None
        };
        if let Some(dir) = file.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(&file, &self.config)
            .with_context(|| format!("gagal menulis {}", file.display()))?;
        Ok(backup)
    }

    /// Runtime di ekspor yang belum terinstall di sini.
    pub fn missing_runtimes(&self, paths: &Paths) -> Result<Vec<RuntimeEntry>> {
        let mut missing = Vec::new();
        for entry in &self.runtimes {
            if !runtime::installed_versions(paths, entry.kind)?.contains(&entry.version) {
                missing.push(entry.clone());
            }
        }
        Ok(missing)
    }
}

/// Grup service dari daftar status daemon yang sedang berjalan (untuk `devlock profile save`).
pub fn running_groups(services: &[crate::daemon::ServiceStatus]) -> Vec<String> {
    let mut groups: Vec<String> = services
        .iter()
        .filter(|s| matches!(s.state.as_str(), "running" | "starting"))
        .map(|s| s.group.clone())
        .collect();
    groups.sort();
    groups.dedup();
    groups
}

/// Nama profil: huruf, angka, `-`, `_`.
pub fn check_profile_name(name: &str) -> Result<()> {
    if name.is_empty()
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        bail!("nama profil hanya boleh huruf, angka, `-`, dan `_`");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_roundtrip_and_apply() {
        let tmp = tempfile::tempdir().unwrap();
        let src = Paths::new(tmp.path().join("a"));
        std::fs::create_dir_all(src.root()).unwrap();
        std::fs::write(
            src.config_file(),
            "# komentar\ntld = \"test\"\n[defaults]\nphp = \"8.3\"\n",
        )
        .unwrap();
        std::fs::create_dir_all(src.runtime_dir("php", "8.3.12")).unwrap();
        std::fs::create_dir_all(src.runtime_dir("redis", "8.10.3")).unwrap();

        let manifest = Manifest::build(&src).unwrap();
        let file = tmp.path().join("devlock-export.toml");
        manifest.write(&file).unwrap();
        let read = Manifest::read(&file).unwrap();
        assert_eq!(read, manifest);
        assert!(read.runtimes.contains(&RuntimeEntry {
            kind: Kind::Php,
            version: "8.3.12".into()
        }));

        let dst = Paths::new(tmp.path().join("b"));
        std::fs::create_dir_all(dst.root()).unwrap();
        std::fs::write(dst.config_file(), "tld = \"lama\"\n").unwrap();
        std::fs::create_dir_all(dst.runtime_dir("php", "8.3.12")).unwrap();
        let backup = read.apply_config(&dst).unwrap().unwrap();
        assert_eq!(std::fs::read_to_string(backup).unwrap(), "tld = \"lama\"\n");
        assert!(std::fs::read_to_string(dst.config_file())
            .unwrap()
            .starts_with("# komentar\n"));
        let missing = read.missing_runtimes(&dst).unwrap();
        assert_eq!(
            missing,
            [RuntimeEntry {
                kind: Kind::Redis,
                version: "8.10.3".into()
            }]
        );

        std::fs::write(&file, "devlock = \"1\"\nconfig = \"tld = [\"\n").unwrap();
        assert!(Manifest::read(&file).is_err());
    }

    #[test]
    fn profile_groups_from_status() {
        let status = |name: &str, group: &str, state: &str| crate::daemon::ServiceStatus {
            name: name.into(),
            group: group.into(),
            state: state.into(),
            pid: None,
            port: None,
            uptime_secs: None,
            restarts: 0,
            error: None,
            log: String::new(),
        };
        let groups = running_groups(&[
            status("php@8.3#0", "php@8.3", "running"),
            status("php@8.3#1", "php@8.3", "running"),
            status("nginx", "nginx", "running"),
            status("mysql@8.4", "mysql@8.4", "stopped"),
            status("redis@8", "redis@8", "starting"),
        ]);
        assert_eq!(groups, ["nginx", "php@8.3", "redis@8"]);
        assert!(check_profile_name("laravel-stack_1").is_ok());
        assert!(check_profile_name("a b").is_err());
    }
}
