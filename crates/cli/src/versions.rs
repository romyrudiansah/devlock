use std::path::Path;

use anyhow::{bail, Context, Result};
use devlock_core::project::{find_project_file, ProjectConfig};
use devlock_core::resolve::{self, DbInstance};
use devlock_core::{runtime, tools, Config, Kind, Paths};

/// `devlock use <php|node|composer|database> <spec> [--global]`
pub fn use_version(target: &str, spec: &str, global: bool) -> Result<()> {
    let paths = Paths::discover()?;
    let cwd = std::env::current_dir()?;

    let (key, value) =
        if target.eq_ignore_ascii_case("database") || target.eq_ignore_ascii_case("db") {
            let instance = DbInstance::parse(spec)?;
            if runtime::resolve_installed(&paths, instance.kind, &instance.line)?.is_none() {
                println!(
                    "peringatan: {} belum terinstall (`devlock install {} {}`)",
                    instance.name(),
                    instance.kind,
                    instance.line
                );
            }
            ("database", instance.name())
        } else {
            let kind: Kind = target.parse()?;
            if !matches!(kind, Kind::Php | Kind::Node | Kind::Composer) {
                bail!("`use` untuk php, node, composer, atau database");
            }
            match runtime::resolve_installed(&paths, kind, spec)? {
                Some(version) => println!("{kind} {spec} → {version}"),
                None => println!(
                    "peringatan: {kind} {spec} belum terinstall (`devlock install {kind} {spec}`)"
                ),
            }
            (kind.as_str(), spec.to_string())
        };

    if global {
        Config::set_default(&paths, key, &value)?;
        println!(
            "Default global {key} = {value} ({})",
            paths.config_file().display()
        );
    } else {
        // Pakai .devlock.toml terdekat bila sudah ada, selain itu buat di folder kerja.
        let dir = find_project_file(&cwd)
            .and_then(|f| f.parent().map(Path::to_path_buf))
            .unwrap_or(cwd);
        let file = ProjectConfig::set_value(&dir, key, &value)?;
        println!("{key} = {value} ({})", file.display());
    }
    Ok(())
}

/// `devlock which <tool>`: program yang akan dijalankan shim di folder ini.
pub fn which(tool: &str) -> Result<()> {
    let paths = Paths::discover()?;
    let config = Config::load(&paths)?;
    let cwd = std::env::current_dir()?;
    let plan = tools::plan(&paths, &config, &cwd, tool)?;

    print!("{}", plan.program.display());
    for arg in &plan.prefix_args {
        print!(" {}", arg.to_string_lossy());
    }
    println!();
    for (name, version, source) in &plan.resolved {
        println!("  {name} {version}  ← {source}");
    }
    Ok(())
}

/// `devlock current`: ringkasan versi untuk folder kerja.
pub fn current() -> Result<()> {
    let paths = Paths::discover()?;
    let config = Config::load(&paths)?;
    let cwd = std::env::current_dir()?;

    for kind in [Kind::Php, Kind::Node, Kind::Composer] {
        match resolve::resolve(&paths, &config, kind, &cwd) {
            Ok(r) => println!("{:<9} {:<10} ← {}", kind.as_str(), r.version, r.source),
            Err(err) => println!("{:<9} -          ({err})", kind.as_str()),
        }
    }
    match resolve::find_database(&config, &cwd)? {
        Some((instance, source)) => {
            let installed = runtime::resolve_installed(&paths, instance.kind, &instance.line)?;
            let version = installed.unwrap_or_else(|| "belum terinstall".into());
            println!(
                "{:<9} {} ({version}, port {}) ← {source}",
                "database",
                instance.name(),
                instance.port(&config)
            );
        }
        None => println!("{:<9} -", "database"),
    }
    Ok(())
}

/// Salin `devlock-shim.exe` menjadi `shims\<tool>.exe`.
pub fn install_shims(paths: &Paths) -> Result<usize> {
    let source = std::env::current_exe()?.with_file_name("devlock-shim.exe");
    if !source.is_file() {
        bail!(
            "{} tidak ditemukan (harus satu folder dengan devlock.exe)",
            source.display()
        );
    }
    std::fs::create_dir_all(paths.shims())?;
    let mut count = 0;
    for name in tools::names() {
        let dest = paths.shims().join(format!("{name}.exe"));
        // Shim yang sedang berjalan tidak bisa ditimpa di Windows, tapi bisa di-rename.
        if dest.exists() {
            let old = dest.with_extension("exe.old");
            let _ = std::fs::remove_file(&old);
            std::fs::rename(&dest, &old)
                .with_context(|| format!("gagal mengganti {}", dest.display()))?;
            let _ = std::fs::remove_file(&old);
        }
        std::fs::copy(&source, &dest)
            .with_context(|| format!("gagal membuat {}", dest.display()))?;
        count += 1;
    }
    Ok(count)
}

pub fn shims() -> Result<()> {
    let paths = Paths::discover()?;
    let count = install_shims(&paths)?;
    println!("{count} shim dibuat di {}", paths.shims().display());
    Ok(())
}
