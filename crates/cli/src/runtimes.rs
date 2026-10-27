use anyhow::Result;
use devlock_core::catalog::{self, Release};
use devlock_core::install::{self, Event};
use devlock_core::runtime;
use devlock_core::{Config, Kind, Paths};
use indicatif::{ProgressBar, ProgressStyle};

pub fn list(kind: Kind, available: bool, all: bool, refresh: bool) -> Result<()> {
    let paths = Paths::discover()?;
    let installed = runtime::installed_versions(&paths, kind)?;

    if !available {
        if installed.is_empty() {
            println!("Belum ada {kind} terinstall. Lihat `devlock list {kind} --available`.");
            return Ok(());
        }
        let default = Config::load(&paths)?.defaults.get(kind.as_str()).cloned();
        let default_version = default.as_deref().and_then(|spec| {
            devlock_core::version::pick(spec, installed.iter().map(String::as_str))
        });
        for version in &installed {
            let mark = if Some(version.as_str()) == default_version {
                "*"
            } else {
                " "
            };
            println!("{mark} {version}");
        }
        return Ok(());
    }

    let releases = catalog::available_cached(&paths, kind, refresh)?;
    let shown: Vec<&Release> = if all {
        releases.iter().collect()
    } else {
        catalog::latest_per_line(kind, &releases)
    };
    for release in shown {
        let mut tags = Vec::new();
        if release.lts {
            tags.push(if kind.is_webserver() { "stable" } else { "lts" });
        }
        if installed.contains(&release.version) {
            tags.push("terinstall");
        }
        let tags = if tags.is_empty() {
            String::new()
        } else {
            format!("  ({})", tags.join(", "))
        };
        println!("{}{tags}", release.version);
    }
    if !all {
        println!("\nHanya versi terbaru per cabang. Pakai --all untuk semua versi.");
    }
    Ok(())
}

pub fn install(kind: Kind, spec: &str) -> Result<()> {
    let paths = Paths::discover()?;
    paths.ensure_layout()?;
    let release = catalog::find_cached(&paths, kind, spec)?;
    println!("Install {kind} {} dari {}", release.version, release.url);

    let bar = ProgressBar::new(0);
    bar.set_style(
        ProgressStyle::with_template("{msg:12} [{bar:30}] {bytes}/{total_bytes} {bytes_per_sec}")
            .unwrap()
            .progress_chars("=> "),
    );
    bar.set_message("download");
    let target = install::install(&paths, &release, &mut |event| match event {
        Event::Downloading { downloaded, total } => {
            if let Some(total) = total {
                bar.set_length(total);
            }
            bar.set_position(downloaded);
        }
        Event::Verifying => bar.set_message("verifikasi"),
        Event::Unverified => {
            bar.println("peringatan: upstream tidak menyediakan SHA256, file tidak diverifikasi")
        }
        Event::Extracting => bar.set_message("ekstrak"),
        Event::Configuring(what) => bar.set_message(format!("konfigurasi {what}")),
    })?;
    bar.finish_and_clear();

    println!(
        "{kind} {} terinstall di {}",
        release.version,
        target.display()
    );
    let config = Config::load(&paths)?;
    if !config.defaults.contains_key(kind.as_str()) && kind.has_default_version() {
        Config::set_default(&paths, kind.as_str(), &release.version)?;
        println!(
            "Dijadikan versi default {kind} (ubah dengan `devlock use {kind} <versi> --global`)."
        );
    }
    Ok(())
}

pub fn uninstall(kind: Kind, version: &str) -> Result<()> {
    let paths = Paths::discover()?;
    let installed = runtime::installed_versions(&paths, kind)?;
    if !installed.iter().any(|v| v == version) {
        anyhow::bail!(
            "{kind} {version} tidak terinstall (terinstall: {}); tulis versi lengkap",
            if installed.is_empty() {
                "-".to_string()
            } else {
                installed.join(", ")
            }
        );
    }
    let dir = runtime::uninstall(&paths, kind, version)?;
    println!("{kind} {version} dihapus ({})", dir.display());
    if kind.is_database() {
        println!(
            "Data database di {} tidak ikut dihapus.",
            paths.data().display()
        );
    }
    Ok(())
}
