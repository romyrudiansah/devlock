use std::path::PathBuf;

use anyhow::{bail, Result};
use devlock_core::db;
use devlock_core::resolve::{self, DbInstance};
use devlock_core::{Config, Paths};

/// Instance dari `--instance`, `.devlock.toml` terdekat, atau default global.
fn instance(config: &Config, explicit: Option<&str>) -> Result<DbInstance> {
    if let Some(text) = explicit {
        return DbInstance::parse(text);
    }
    let cwd = std::env::current_dir()?;
    match resolve::find_database(config, &cwd)? {
        Some((instance, _)) => Ok(instance),
        None => bail!(
            "instance database tidak diketahui; pakai `--instance mysql@8.4` atau set `database` di .devlock.toml"
        ),
    }
}

fn load(explicit: Option<&str>) -> Result<(Paths, Config, DbInstance)> {
    let paths = Paths::discover()?;
    let config = Config::load(&paths)?;
    let instance = instance(&config, explicit)?;
    Ok((paths, config, instance))
}

pub fn list(explicit: Option<&str>) -> Result<()> {
    let (paths, config, instance) = load(explicit)?;
    let names = db::list(&paths, &config, &instance)?;
    if names.is_empty() {
        println!("Belum ada database di {}.", instance.name());
    }
    for name in names {
        println!("{name}");
    }
    Ok(())
}

pub fn create(name: &str, explicit: Option<&str>) -> Result<()> {
    let (paths, config, instance) = load(explicit)?;
    if db::create(&paths, &config, &instance, name)? {
        println!(
            "Database `{name}` dibuat di {} (port {}).",
            instance.name(),
            instance.port(&config)
        );
    } else {
        println!("Database `{name}` sudah ada di {}.", instance.name());
    }
    Ok(())
}

pub fn drop(name: &str, explicit: Option<&str>, yes: bool) -> Result<()> {
    if !yes {
        bail!("menghapus database tidak bisa dibatalkan; ulangi dengan `--yes` (pertimbangkan `devlock db dump {name}` dulu)");
    }
    let (paths, config, instance) = load(explicit)?;
    if db::drop(&paths, &config, &instance, name)? {
        println!("Database `{name}` dihapus dari {}.", instance.name());
    } else {
        println!("Database `{name}` tidak ada di {}.", instance.name());
    }
    Ok(())
}

pub fn dump(name: &str, explicit: Option<&str>, output: Option<PathBuf>) -> Result<()> {
    let (paths, config, instance) = load(explicit)?;
    let out = output.unwrap_or_else(|| db::default_dump_path(&paths, &instance, name));
    db::dump(&paths, &config, &instance, name, &out)?;
    let size = std::fs::metadata(&out).map(|m| m.len()).unwrap_or(0);
    println!(
        "Dump `{name}` ({} KB) → {}",
        size.div_ceil(1024),
        out.display()
    );
    Ok(())
}

pub fn restore(name: &str, file: &std::path::Path, explicit: Option<&str>) -> Result<()> {
    let (paths, config, instance) = load(explicit)?;
    db::restore(&paths, &config, &instance, name, file)?;
    println!(
        "{} dijalankan ke `{name}` di {}.",
        file.display(),
        instance.name()
    );
    Ok(())
}
