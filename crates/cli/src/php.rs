use std::path::PathBuf;

use anyhow::Result;
use devlock_core::daemon;
use devlock_core::version::Version;
use devlock_core::{php, resolve, runtime, xdebug, Config, Kind, Paths};

/// Folder & versi PHP: `--php <versi>` atau versi milik folder kerja.
struct Target {
    paths: Paths,
    config: Config,
    version: String,
    dir: PathBuf,
}

impl Target {
    fn load(spec: Option<&str>) -> Result<Self> {
        let paths = Paths::discover()?;
        let config = Config::load(&paths)?;
        let (version, dir) = match spec {
            Some(spec) => runtime::require_installed(&paths, Kind::Php, spec)?,
            None => {
                let r = resolve::resolve(&paths, &config, Kind::Php, &std::env::current_dir()?)?;
                (r.version, r.dir)
            }
        };
        Ok(Self {
            paths,
            config,
            version,
            dir,
        })
    }

    fn line(&self) -> String {
        Version::parse(&self.version).map_or(self.version.clone(), |v| v.major_minor())
    }

    /// Restart pool php-cgi versi ini bila sedang berjalan, agar `php.ini` baru dipakai.
    fn apply(&self) {
        let target = format!("php@{}", self.line());
        match daemon::restart_group_if_running(&self.config, &target) {
            Ok(true) => println!("Pool {target} di-restart."),
            Ok(false) => {}
            Err(err) => println!("peringatan: {err:#}; jalankan `devlock restart {target}`"),
        }
    }
}

pub fn ext_list(spec: Option<&str>) -> Result<()> {
    let t = Target::load(spec)?;
    println!("PHP {} ({})", t.version, t.dir.display());
    for ext in php::extensions(&t.dir)? {
        let mark = if ext.enabled { "[x]" } else { "[ ]" };
        let zend = if ext.zend { "  (zend)" } else { "" };
        println!("{mark} {}{zend}", ext.name);
    }
    Ok(())
}

pub fn ext_set(names: &[String], enabled: bool, spec: Option<&str>) -> Result<()> {
    let t = Target::load(spec)?;
    for name in names {
        php::set_extension(&t.dir, name, enabled)?;
        let state = if enabled { "aktif" } else { "nonaktif" };
        println!("{name}: {state} di PHP {}", t.version);
    }
    t.apply();
    Ok(())
}

pub fn xdebug_status(spec: Option<&str>) -> Result<()> {
    let t = Target::load(spec)?;
    let installed = xdebug::is_installed(&t.dir);
    println!("PHP {}", t.version);
    println!(
        "Terpasang : {}",
        if installed {
            "ya"
        } else {
            "belum — `devlock php xdebug install`"
        }
    );
    println!(
        "Aktif     : {}",
        if xdebug::is_enabled(&t.dir) {
            "ya"
        } else {
            "tidak"
        }
    );
    if installed {
        let ini = std::fs::read_to_string(t.dir.join("php.ini")).unwrap_or_default();
        for key in [
            "xdebug.mode",
            "xdebug.start_with_request",
            "xdebug.client_port",
        ] {
            println!(
                "{key:<26}: {}",
                php::ini_value(&ini, key).unwrap_or_else(|| "-".into())
            );
        }
    }
    Ok(())
}

pub fn xdebug_install(spec: Option<&str>) -> Result<()> {
    let t = Target::load(spec)?;
    println!("Mencari Xdebug untuk PHP {} di xdebug.org…", t.line());
    let version = xdebug::install(&t.paths, &t.dir, &t.line())?;
    println!(
        "Xdebug {version} terpasang (SHA256 cocok) di {}",
        xdebug::dll_path(&t.dir).display()
    );
    xdebug::set_enabled(&t.dir, true)?;
    println!("Xdebug aktif: mode debug,develop, mulai dengan trigger XDEBUG_SESSION, port 9003.");
    t.apply();
    Ok(())
}

pub fn xdebug_set(enabled: bool, spec: Option<&str>) -> Result<()> {
    let t = Target::load(spec)?;
    xdebug::set_enabled(&t.dir, enabled)?;
    println!(
        "Xdebug {} di PHP {}",
        if enabled { "aktif" } else { "nonaktif" },
        t.version
    );
    t.apply();
    Ok(())
}

pub fn ini_get(key: &str, spec: Option<&str>) -> Result<()> {
    let t = Target::load(spec)?;
    let ini = std::fs::read_to_string(t.dir.join("php.ini"))?;
    match php::ini_value(&ini, key) {
        Some(value) => println!("{value}"),
        None => println!("(tidak diset; memakai default PHP)"),
    }
    Ok(())
}

pub fn ini_set(key: &str, value: &str, spec: Option<&str>) -> Result<()> {
    let t = Target::load(spec)?;
    php::set_ini_value(&t.dir, key, value)?;
    println!("{key} = \"{value}\" di PHP {}", t.version);
    t.apply();
    Ok(())
}

pub fn ini_path(spec: Option<&str>) -> Result<()> {
    let t = Target::load(spec)?;
    println!("{}", t.dir.join("php.ini").display());
    Ok(())
}
