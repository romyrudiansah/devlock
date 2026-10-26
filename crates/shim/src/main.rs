//! Shim generik: disalin menjadi `shims\php.exe`, `shims\node.exe`, dst. Nama file
//! menentukan perintah; versi dipilih dari folder kerja lalu proses asli dijalankan.

use std::process::Command;

use anyhow::{Context, Result};
use devlock_core::{tools, Config, Paths};

fn main() {
    let code = run().unwrap_or_else(|err| {
        eprintln!("devlock: {err:#}");
        1
    });
    std::process::exit(code);
}

fn run() -> Result<i32> {
    let exe = std::env::current_exe().context("tidak bisa membaca lokasi shim")?;
    let tool = exe
        .file_stem()
        .and_then(|s| s.to_str())
        .context("nama shim tidak valid")?
        .to_ascii_lowercase();
    let paths = Paths::discover()?;
    let config = Config::load(&paths)?;
    let cwd = std::env::current_dir().context("tidak bisa membaca folder kerja")?;

    let plan = tools::plan(&paths, &config, &cwd, &tool)?;
    let mut command = Command::new(&plan.program);
    command
        .args(&plan.prefix_args)
        .args(std::env::args_os().skip(1))
        .env("PATH", plan.child_path()?);
    for (key, value) in &plan.env {
        command.env(key, value);
    }

    ignore_ctrl_c();
    let status = command
        .status()
        .with_context(|| format!("gagal menjalankan {}", plan.program.display()))?;
    Ok(status.code().unwrap_or(1))
}

/// Ctrl+C diteruskan Windows ke semua proses di console; shim cukup menunggu anak selesai.
fn ignore_ctrl_c() {
    #[cfg(windows)]
    // SAFETY: handler NULL + TRUE hanya mengabaikan Ctrl+C untuk proses ini.
    unsafe {
        windows_sys::Win32::System::Console::SetConsoleCtrlHandler(None, 1);
    }
}
