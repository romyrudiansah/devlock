use std::net::TcpListener;

use anyhow::Result;
use devlock_core::{Config, Paths};

use crate::{user_path, versions};

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
        println!(
            "Konfigurasi  : {} (sudah ada)",
            paths.config_file().display()
        );
    }
    println!("Folder www   : {}", config.www_dir(&paths).display());
    match versions::install_shims(&paths) {
        Ok(count) => println!(
            "Shim         : {count} dibuat di {}",
            paths.shims().display()
        ),
        Err(err) => println!("Shim         : dilewati ({err:#})"),
    }
    println!("\nLangkah berikut: `devlock path add` agar shim masuk PATH, lalu `devlock install php 8.3`.");
    Ok(())
}

pub fn path(action: &str) -> Result<()> {
    let paths = Paths::discover()?;
    let shims = paths.shims();
    let changed = match action {
        "add" => user_path::add(&shims)?,
        _ => user_path::remove(&shims)?,
    };
    match (action, changed) {
        ("add", true) => println!(
            "{} ditambahkan di depan PATH user. Buka terminal baru.",
            shims.display()
        ),
        ("add", false) => println!("{} sudah di urutan pertama PATH user.", shims.display()),
        (_, true) => println!("{} dihapus dari PATH user.", shims.display()),
        (_, false) => println!("{} tidak ada di PATH user.", shims.display()),
    }
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
    for tool in ["php", "node", "composer", "mysql"] {
        if let Some(found) = first_in_path(tool) {
            check(
                found
                    .parent()
                    .is_some_and(|dir| same_path(dir, &paths.shims())),
                &format!("`{tool}` di PATH mengarah ke shim DevLock"),
                &format!(
                    "{} lebih dulu ditemukan; pindahkan shims ke awal PATH (`devlock path add`)",
                    found.display()
                ),
            );
        }
    }
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

fn first_in_path(tool: &str) -> Option<std::path::PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).find_map(|dir| {
        ["exe", "cmd", "bat"]
            .iter()
            .map(|ext| dir.join(format!("{tool}.{ext}")))
            .find(|p| p.is_file())
    })
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
