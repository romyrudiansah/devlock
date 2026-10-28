use anyhow::{bail, Result};
use devlock_core::daemon::{self, Request};
use devlock_core::{update, Config, Paths};
use indicatif::{ProgressBar, ProgressStyle};

pub fn run(check_only: bool) -> Result<()> {
    let current = update::current_version();
    let Some(latest) = update::latest()? else {
        bail!("rilis DevLock di GitHub tidak terbaca");
    };
    if !update::is_newer(&latest.version) {
        println!("DevLock {current} sudah versi terbaru.");
        return Ok(());
    }
    println!("Versi baru tersedia: {current} → {}", latest.version);
    println!(
        "Catatan rilis: https://github.com/romyrudiansah/devlock/releases/tag/v{}",
        latest.version
    );
    if check_only {
        println!("Pasang dengan `devlock update`.");
        return Ok(());
    }

    if gui_running() {
        bail!(
            "tutup DevLock GUI dulu (klik kanan ikon tray → Keluar), lalu ulangi `devlock update`"
        );
    }
    let paths = Paths::discover()?;
    let config = Config::load(&paths)?;
    if daemon::send(&config, &Request::Shutdown)?.is_some() {
        println!("Service dan daemon dihentikan.");
    }

    let bar = ProgressBar::new(0);
    bar.set_style(
        ProgressStyle::with_template("{msg:12} [{bar:30}] {bytes}/{total_bytes}")
            .unwrap()
            .progress_chars("=> "),
    );
    bar.set_message("download");
    let result = update::apply(&paths, &latest, &mut |downloaded, total| {
        if let Some(total) = total {
            bar.set_length(total);
        }
        bar.set_position(downloaded);
    });
    bar.finish_and_clear();
    result?;
    // Shim adalah salinan devlock-shim.exe: salin ulang agar ikut versi baru (dan shim
    // yang baru ditambahkan rilis ini muncul).
    match crate::versions::install_shims(&paths) {
        Ok(count) => println!("{count} shim diperbarui di {}", paths.shims().display()),
        Err(err) => {
            println!("peringatan: shim belum diperbarui ({err:#}); jalankan `devlock shims`")
        }
    }
    println!(
        "DevLock {} terpasang di {}. Jalankan lagi `devlock start` atau buka GUI.",
        latest.version,
        paths.bin().display()
    );
    Ok(())
}

fn gui_running() -> bool {
    std::process::Command::new("tasklist.exe")
        .args(["/FI", "IMAGENAME eq devlock-gui.exe", "/NH"])
        .output()
        .is_ok_and(|out| {
            String::from_utf8_lossy(&out.stdout)
                .to_ascii_lowercase()
                .contains("devlock-gui.exe")
        })
}
