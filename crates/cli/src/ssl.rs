use anyhow::Result;
use devlock_core::{install, sites, ssl, Config, Paths};

pub fn status() -> Result<()> {
    let paths = Paths::discover()?;
    let config = Config::load(&paths)?;
    let ca = ssl::ca_cert_path(&paths);
    println!(
        "CA DevLock     : {}",
        if ca.is_file() {
            ca.display().to_string()
        } else {
            "belum dibuat".into()
        }
    );
    println!(
        "Dipercaya      : {}",
        if ssl::is_trusted() {
            "ya (store Root user)"
        } else {
            "belum — `devlock ssl trust`"
        }
    );
    println!(
        "HTTPS global   : {} (port {})",
        if config.https.enabled {
            "aktif"
        } else {
            "nonaktif"
        },
        config.https.port
    );
    println!(
        "Redirect HTTP  : {}",
        if config.https.redirect {
            "aktif (HTTP → HTTPS)"
        } else {
            "nonaktif"
        }
    );
    let https_sites: Vec<String> = sites::discover(&paths, &config)?
        .iter()
        .filter(|s| s.https)
        .map(|s| {
            if s.https_redirect {
                format!("{} (redirect)", s.url(&config))
            } else {
                s.url(&config)
            }
        })
        .collect();
    if https_sites.is_empty() {
        println!("Site HTTPS     : -");
    } else {
        println!("Site HTTPS     : {}", https_sites.join(", "));
    }
    Ok(())
}

pub fn trust() -> Result<()> {
    let paths = Paths::discover()?;
    if ssl::ensure_ca(&paths)? {
        println!(
            "CA DevLock dibuat di {}",
            ssl::ca_cert_path(&paths).display()
        );
    }
    println!("Windows akan menampilkan dialog konfirmasi pemasangan sertifikat; pilih Yes/Ya.");
    ssl::trust(&paths)?;
    println!("CA DevLock dipercaya untuk user ini (Chrome/Edge memakai store Windows).");
    println!(
        "Firefox memakai store sendiri: impor {} secara manual bila perlu.",
        ssl::ca_cert_path(&paths).display()
    );
    refresh_php_bundle(&paths);
    Ok(())
}

/// `devlock ssl bundle`: unduh/perbarui bundle CA dan pasang ke semua PHP terinstall.
pub fn bundle() -> Result<()> {
    let paths = Paths::discover()?;
    let count = install::apply_ca_bundle_to_php(&paths)?;
    println!(
        "Bundle CA {} dipasang ke {count} versi PHP (curl.cainfo, openssl.cafile).",
        install::ca_bundle_path(&paths).display()
    );
    Ok(())
}

/// PHP perlu percaya CA DevLock juga (mis. request HTTPS antar site lokal).
fn refresh_php_bundle(paths: &Paths) {
    match install::apply_ca_bundle_to_php(paths) {
        Ok(0) => {}
        Ok(count) => println!("Bundle CA PHP diperbarui untuk {count} versi PHP."),
        Err(err) => println!("peringatan: bundle CA PHP belum diperbarui ({err:#}); ulangi dengan `devlock ssl bundle`"),
    }
}

pub fn untrust() -> Result<()> {
    ssl::untrust()?;
    println!("CA DevLock dicabut dari store Root user.");
    Ok(())
}

pub fn set_enabled(enabled: bool) -> Result<()> {
    let paths = Paths::discover()?;
    Config::set_value(&paths, "https", "enabled", toml_edit::value(enabled))?;
    if enabled {
        ssl::ensure_ca(&paths)?;
        refresh_php_bundle(&paths);
        println!("HTTPS aktif untuk semua site (project bisa menimpa dengan `https = false`).");
        if !ssl::is_trusted() {
            println!("Jalankan `devlock ssl trust` agar browser percaya sertifikatnya.");
        }
    } else {
        println!("HTTPS global nonaktif (project masih bisa memakai `https = true`).");
    }
    println!("Terapkan dengan `devlock reload`.");
    Ok(())
}

pub fn set_redirect(enabled: bool) -> Result<()> {
    let paths = Paths::discover()?;
    Config::set_value(&paths, "https", "redirect", toml_edit::value(enabled))?;
    if enabled {
        println!(
            "HTTP dialihkan ke HTTPS untuk site ber-HTTPS (project bisa menimpa dengan `https_redirect = false`)."
        );
    } else {
        println!("Redirect HTTP→HTTPS global nonaktif (project masih bisa memakai `https_redirect = true`).");
    }
    println!("Terapkan dengan `devlock reload`.");
    Ok(())
}
