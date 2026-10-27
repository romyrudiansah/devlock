use anyhow::Result;
use devlock_core::{sites, ssl, Config, Paths};

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
    let https_sites: Vec<String> = sites::discover(&paths, &config)?
        .iter()
        .filter(|s| s.https)
        .map(|s| s.url(&config))
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
    Ok(())
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
