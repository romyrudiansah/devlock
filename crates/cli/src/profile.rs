use std::path::Path;

use anyhow::{bail, Context, Result};
use devlock_core::daemon::{self, Request};
use devlock_core::export::{self, Manifest};
use devlock_core::{launcher, Config, Paths};

/// `devlock config export <file>`.
pub fn export_config(file: &Path) -> Result<()> {
    let paths = Paths::discover()?;
    let manifest = Manifest::build(&paths)?;
    manifest.write(file)?;
    println!(
        "Konfigurasi dan {} runtime diekspor ke {}",
        manifest.runtimes.len(),
        file.display()
    );
    println!("Project ikut dipindah bersama foldernya (beserta .devlock.toml masing-masing).");
    Ok(())
}

/// `devlock config import <file> [--no-install]`.
pub fn import_config(file: &Path, install: bool) -> Result<()> {
    let paths = Paths::discover()?;
    paths.ensure_layout()?;
    let manifest = Manifest::read(file)?;
    match manifest.apply_config(&paths)? {
        Some(backup) => println!(
            "devlock.toml diganti dari {} (yang lama: {})",
            file.display(),
            backup.display()
        ),
        None => println!("devlock.toml dibuat dari {}", file.display()),
    }
    let config = Config::load(&paths)?;
    let www = config.www_dir(&paths);
    if !www.is_dir() {
        println!(
            "peringatan: folder www {} belum ada di komputer ini",
            www.display()
        );
    }

    let missing = manifest.missing_runtimes(&paths)?;
    if missing.is_empty() {
        println!("Semua runtime dari ekspor sudah terinstall.");
    } else if !install {
        println!("Runtime yang belum terinstall:");
        for entry in &missing {
            println!("  devlock install {} {}", entry.kind, entry.version);
        }
    } else {
        let mut failed = Vec::new();
        for entry in &missing {
            if let Err(err) = crate::runtimes::install(entry.kind, &entry.version) {
                println!("gagal: {} {}: {err:#}", entry.kind, entry.version);
                failed.push(format!("{} {}", entry.kind, entry.version));
            }
        }
        if !failed.is_empty() {
            bail!("sebagian runtime gagal diinstall: {}", failed.join(", "));
        }
    }
    println!("Terapkan dengan `devlock reload` (atau `devlock start`).");
    Ok(())
}

pub fn list() -> Result<()> {
    let paths = Paths::discover()?;
    let config = Config::load(&paths)?;
    if config.profiles.is_empty() {
        println!("Belum ada profil; simpan service yang sedang berjalan dengan `devlock profile save <nama>`.");
    }
    for (name, groups) in &config.profiles {
        println!("{name:<16} {}", groups.join(", "));
    }
    Ok(())
}

/// Simpan grup service yang sedang berjalan sebagai profil.
pub fn save(name: &str) -> Result<()> {
    export::check_profile_name(name)?;
    let paths = Paths::discover()?;
    let config = Config::load(&paths)?;
    let response = daemon::send(&config, &Request::Status)?.context(
        "daemon tidak berjalan; jalankan service yang diinginkan dulu (`devlock start ...`)",
    )?;
    let groups = export::running_groups(&response.services);
    if groups.is_empty() {
        bail!("tidak ada service yang berjalan untuk disimpan");
    }
    let mut array = toml_edit::Array::new();
    for group in &groups {
        array.push(group.as_str());
    }
    Config::set_value(&paths, "profiles", name, toml_edit::value(array))?;
    println!("Profil `{name}`: {}", groups.join(", "));
    Ok(())
}

/// Hentikan semua service lalu jalankan grup milik profil.
pub fn use_profile(name: &str) -> Result<()> {
    let paths = Paths::discover()?;
    let config = Config::load(&paths)?;
    let Some(groups) = config.profiles.get(name) else {
        bail!(
            "profil `{name}` tidak ada (profil: {})",
            config
                .profiles
                .keys()
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        );
    };
    launcher::ensure_daemon(&paths, &config)?;
    daemon::send(
        &config,
        &Request::Stop {
            target: "all".into(),
        },
    )?;
    for group in groups {
        let response = daemon::send(
            &config,
            &Request::Start {
                target: group.clone(),
            },
        )?
        .context("daemon tidak menjawab")?;
        if !response.ok {
            println!("peringatan: {group}: {}", response.message);
        }
    }
    println!("Profil `{name}` aktif: {}", groups.join(", "));
    crate::services::status()
}

pub fn delete(name: &str) -> Result<()> {
    let paths = Paths::discover()?;
    if Config::remove_value(&paths, "profiles", name)? {
        println!("Profil `{name}` dihapus.");
    } else {
        println!("Profil `{name}` tidak ada.");
    }
    Ok(())
}
