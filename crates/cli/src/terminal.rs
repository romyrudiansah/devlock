use anyhow::Result;
use devlock_core::{terminal, Config, Paths};

pub fn run(action: &str) -> Result<()> {
    match action {
        "install" => install(),
        "remove" => remove(),
        _ => status(),
    }
}

fn status() -> Result<()> {
    println!(
        "Windows Terminal : {}",
        if terminal::is_available() {
            "terpasang"
        } else {
            "tidak ditemukan"
        }
    );
    match terminal::fragment_path() {
        Some(file) if file.is_file() => println!("Profil DevLock   : ada ({})", file.display()),
        _ => println!("Profil DevLock   : belum — `devlock terminal install`"),
    }
    Ok(())
}

fn install() -> Result<()> {
    let paths = Paths::discover()?;
    let config = Config::load(&paths)?;
    let file = terminal::install(&paths, &config)?;
    println!(
        "Profil Windows Terminal \"DevLock\" ditulis ke {}",
        file.display()
    );
    if !terminal::is_available() {
        println!(
            "Windows Terminal belum terpasang; profil muncul setelah Windows Terminal diinstall."
        );
    } else {
        println!("Buka tab baru Windows Terminal dan pilih profil DevLock.");
    }
    Ok(())
}

fn remove() -> Result<()> {
    if terminal::remove()? {
        println!("Profil Windows Terminal \"DevLock\" dihapus.");
    } else {
        println!("Profil Windows Terminal \"DevLock\" tidak ada.");
    }
    Ok(())
}

/// Dipanggil setelah shim dibuat ulang: profil yang sudah dipasang ikut menunjuk folder
/// shims/www terbaru.
pub fn sync_quietly(paths: &Paths) {
    let Ok(config) = Config::load(paths) else {
        return;
    };
    match terminal::sync(paths, &config) {
        Ok(Some(file)) => println!(
            "Profil Windows Terminal \"DevLock\" diperbarui: {}",
            file.display()
        ),
        Ok(None) => {}
        Err(err) => println!("peringatan: profil Windows Terminal tidak diperbarui ({err:#})"),
    }
}
