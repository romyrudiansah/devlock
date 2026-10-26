//! Struktur folder instalasi DevLock. Semua path diturunkan dari satu root agar portable.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

/// Variabel lingkungan untuk menimpa lokasi root (berguna saat development).
pub const HOME_ENV: &str = "DEVLOCK_HOME";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paths {
    root: PathBuf,
}

impl Paths {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Cari root: `DEVLOCK_HOME`, lalu induk dari folder `bin\` atau `shims\` tempat exe berada.
    pub fn discover() -> Result<Self> {
        if let Some(home) = std::env::var_os(HOME_ENV) {
            return Ok(Self::new(home));
        }
        let exe = std::env::current_exe().context("tidak bisa membaca lokasi executable")?;
        if let Some(root) = root_from_exe(&exe) {
            return Ok(Self::new(root));
        }
        bail!(
            "root DevLock tidak ditemukan; set {HOME_ENV} atau jalankan dari folder bin\\ instalasi"
        )
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn config_file(&self) -> PathBuf {
        self.root.join("devlock.toml")
    }

    pub fn bin(&self) -> PathBuf {
        self.root.join("bin")
    }

    pub fn shims(&self) -> PathBuf {
        self.root.join("shims")
    }

    pub fn runtimes(&self) -> PathBuf {
        self.root.join("runtimes")
    }

    /// `runtimes\<kind>\<version>`
    pub fn runtime_dir(&self, kind: &str, version: &str) -> PathBuf {
        self.runtimes().join(kind).join(version)
    }

    pub fn etc(&self) -> PathBuf {
        self.root.join("etc")
    }

    pub fn data(&self) -> PathBuf {
        self.root.join("data")
    }

    pub fn ssl(&self) -> PathBuf {
        self.root.join("ssl")
    }

    pub fn logs(&self) -> PathBuf {
        self.root.join("logs")
    }

    pub fn tmp(&self) -> PathBuf {
        self.root.join("tmp")
    }

    pub fn downloads(&self) -> PathBuf {
        self.tmp().join("downloads")
    }

    /// File state proses (pid, port) milik daemon.
    pub fn run(&self) -> PathBuf {
        self.tmp().join("run")
    }

    /// Lokasi default folder project; bisa ditimpa lewat `www` di `devlock.toml`.
    pub fn default_www(&self) -> PathBuf {
        self.root.join("www")
    }

    /// Ubah path relatif (dari config) menjadi absolut terhadap root.
    pub fn resolve(&self, path: &Path) -> PathBuf {
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.root.join(path)
        }
    }

    /// Buat semua folder standar yang belum ada.
    pub fn ensure_layout(&self) -> Result<()> {
        for dir in [
            self.bin(),
            self.shims(),
            self.runtimes(),
            self.etc(),
            self.data(),
            self.ssl(),
            self.logs(),
            self.downloads(),
            self.run(),
            self.default_www(),
        ] {
            std::fs::create_dir_all(&dir)
                .with_context(|| format!("gagal membuat folder {}", dir.display()))?;
        }
        Ok(())
    }
}

fn root_from_exe(exe: &Path) -> Option<PathBuf> {
    let dir = exe.parent()?;
    let name = dir.file_name()?.to_str()?;
    if name.eq_ignore_ascii_case("bin") || name.eq_ignore_ascii_case("shims") {
        dir.parent().map(Path::to_path_buf)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_from_bin_or_shims() {
        let root = Path::new("D:/DevLock");
        assert_eq!(
            root_from_exe(&root.join("bin/devlock.exe")).as_deref(),
            Some(root)
        );
        assert_eq!(
            root_from_exe(&root.join("shims/php.exe")).as_deref(),
            Some(root)
        );
        assert_eq!(root_from_exe(&root.join("target/devlock.exe")), None);
    }

    #[test]
    fn layout_is_created() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path());
        paths.ensure_layout().unwrap();
        assert!(paths.shims().is_dir());
        assert!(paths.run().is_dir());
        assert_eq!(
            paths.runtime_dir("php", "8.3.12"),
            tmp.path().join("runtimes/php/8.3.12")
        );
    }
}
