use std::net::TcpListener;

use anyhow::Result;
use devlock_core::{Config, Paths};

pub fn init() -> Result<()> {
    let paths = Paths::discover()?;
    paths.ensure_layout()?;
    let created = Config::write_default_if_missing(&paths)?;
    let config = Config::load(&paths)?;
    std::fs::create_dir_all(config.www_dir(&paths))?;

    println!("Root DevLock : {}", paths.root().display());
    if created {
        println!("Konfigurasi  : {} (baru)", paths.config_file().display());
    } else {
        println!("Konfigurasi  : {} (sudah ada)", paths.config_file().display());
    }
    println!("Folder www   : {}", config.www_dir(&paths).display());
    Ok(())
}

pub fn config() -> Result<()> {
    let paths = Paths::discover()?;
    let file = paths.config_file();
    println!("# {}", file.display());
    match std::fs::read_to_string(&file) {
        Ok(text) => print!("{text}"),
        Err(_) => println!("# (belum ada, jalankan `devlock init`)"),
    }
    Ok(())
}

pub fn doctor() -> Result<()> {
    let paths = Paths::discover()?;
    let config = Config::load(&paths)?;
    let mut problems = 0;

    let mut check = |ok: bool, label: &str, hint: &str| {
        if ok {
            println!("[ok]   {label}");
        } else {
            problems += 1;
            println!("[!!]   {label}\n       → {hint}");
        }
    };

    check(
        paths.config_file().is_file(),
        &format!("devlock.toml di {}", paths.root().display()),
        "jalankan `devlock init`",
    );
    check(
        path_contains(&paths.shims()),
        "folder shims ada di PATH",
        &format!("tambahkan {} ke PATH user", paths.shims().display()),
    );
    check(
        vc_redist_installed(),
        "Visual C++ Redistributable (vcruntime140.dll)",
        "install dari https://aka.ms/vs/17/release/vc_redist.x64.exe (dibutuhkan PHP, Apache, MySQL)",
    );
    let http = format!("{}:{}", config.http.listen, config.http.port);
    check(
        port_free(&http),
        &format!("port HTTP {http} bebas"),
        "port dipakai aplikasi lain (IIS, Laragon, XAMPP, Skype?) — hentikan atau ubah [http].port",
    );

    if problems == 0 {
        println!("\nSemua pemeriksaan lolos.");
    } else {
        println!("\n{problems} masalah ditemukan.");
    }
    Ok(())
}

fn path_contains(dir: &std::path::Path) -> bool {
    std::env::var_os("PATH")
        .map(|path| std::env::split_paths(&path).any(|p| same_path(&p, dir)))
        .unwrap_or(false)
}

fn same_path(a: &std::path::Path, b: &std::path::Path) -> bool {
    let norm = |p: &std::path::Path| {
        p.to_string_lossy()
            .trim_end_matches(['\\', '/'])
            .replace('/', "\\")
            .to_lowercase()
    };
    norm(a) == norm(b)
}

fn vc_redist_installed() -> bool {
    let system_root = std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into());
    std::path::Path::new(&system_root)
        .join("System32")
        .join("vcruntime140.dll")
        .is_file()
}

fn port_free(addr: &str) -> bool {
    TcpListener::bind(addr).is_ok()
}
