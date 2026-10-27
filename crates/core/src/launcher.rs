//! Aksi yang dipakai bersama CLI dan GUI: menjalankan daemon di latar dan
//! menyamakan file hosts (dengan elevasi UAC bila perlu).

use std::fs::OpenOptions;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};

use crate::config::Config;
use crate::daemon::{self, Request};
use crate::paths::Paths;
use crate::{hosts, process, sites};

/// Lokasi `devlock.exe`: executable ini sendiri, atau `devlock.exe` di folder yang sama (GUI).
pub fn devlock_exe() -> Result<PathBuf> {
    let exe = std::env::current_exe().context("tidak bisa membaca lokasi executable")?;
    if exe
        .file_stem()
        .is_some_and(|s| s.eq_ignore_ascii_case("devlock"))
    {
        return Ok(exe);
    }
    let sibling = exe.with_file_name("devlock.exe");
    if !sibling.is_file() {
        bail!("{} tidak ditemukan", sibling.display());
    }
    Ok(sibling)
}

/// Jalankan `devlock daemon` di latar bila belum berjalan, lalu tunggu sampai merespons.
pub fn ensure_daemon(paths: &Paths, config: &Config) -> Result<()> {
    if daemon::send(config, &Request::Status)?.is_some() {
        return Ok(());
    }
    paths.ensure_layout()?;
    let exe = devlock_exe()?;
    let log = paths.logs().join("daemon.log");
    // Hanya handle log yang diwariskan, bukan stdout/stderr pemanggil: pipe seperti
    // `devlock start | Out-String` harus EOF begitu perintah ini selesai.
    let spawn = |flags: u32| -> std::io::Result<u32> {
        let out = OpenOptions::new().create(true).append(true).open(&log)?;
        let mut command = Command::new(&exe);
        command
            .arg("daemon")
            .env(crate::paths::HOME_ENV, paths.root())
            .current_dir(paths.root());
        process::spawn_detached(command, out, flags)
    };
    let base =
        process::DETACHED_PROCESS | process::CREATE_NEW_PROCESS_GROUP | process::CREATE_NO_WINDOW;
    // Keluar dari job object pemanggil (bila diizinkan) agar daemon tidak ikut mati
    // saat terminal atau GUI ditutup.
    spawn(base | process::CREATE_BREAKAWAY_FROM_JOB)
        .or_else(|_| spawn(base))
        .context("gagal menjalankan daemon")?;

    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(200));
        if daemon::send(config, &Request::Status)?.is_some() {
            return Ok(());
        }
    }
    bail!("daemon tidak merespons; lihat {}", log.display())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostsSync {
    Unchanged,
    Updated { domains: usize },
}

/// Samakan blok DevLock di file hosts dengan daftar site. Bila tidak punya izin dan
/// `elevated == false`, jalankan `devlock hosts --elevated` lewat UAC.
pub fn sync_hosts(paths: &Paths, config: &Config, elevated: bool) -> Result<HostsSync> {
    let domains: Vec<String> = sites::discover(paths, config)?
        .into_iter()
        .flat_map(|s| s.domains)
        .collect();
    write_hosts(paths, &domains, elevated, false)
}

/// Hapus seluruh blok DevLock dari file hosts (untuk uninstall).
pub fn clear_hosts(paths: &Paths, elevated: bool) -> Result<HostsSync> {
    write_hosts(paths, &[], elevated, true)
}

fn write_hosts(
    paths: &Paths,
    domains: &[String],
    elevated: bool,
    clear: bool,
) -> Result<HostsSync> {
    let file = hosts::hosts_file();
    let current = std::fs::read_to_string(&file)
        .with_context(|| format!("gagal membaca {}", file.display()))?;
    let wanted = hosts::apply(&current, domains);
    if wanted == current {
        return Ok(HostsSync::Unchanged);
    }
    match std::fs::write(&file, &wanted) {
        Ok(()) => Ok(HostsSync::Updated {
            domains: domains.len(),
        }),
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied && !elevated => {
            run_elevated_hosts(paths, clear)?;
            if std::fs::read_to_string(&file)? != wanted {
                bail!("file hosts belum berubah (UAC ditolak?)");
            }
            Ok(HostsSync::Updated {
                domains: domains.len(),
            })
        }
        Err(e) => Err(e).with_context(|| format!("gagal menulis {}", file.display())),
    }
}

fn run_elevated_hosts(paths: &Paths, clear: bool) -> Result<()> {
    let exe = devlock_exe()?;
    let escape = |p: &Path| p.to_string_lossy().replace('\'', "''");
    // Proses elevated tidak selalu mewarisi environment, jadi root dikirim sebagai argumen
    // (dibungkus kutip ganda agar path berspasi tetap satu argumen).
    let script = format!(
        "Start-Process -FilePath '{exe}' -ArgumentList @('hosts','--elevated',{clear}'--root','\"{root}\"') -Verb RunAs -Wait -WindowStyle Hidden",
        exe = escape(&exe),
        root = escape(paths.root()),
        clear = if clear { "'--clear'," } else { "" },
    );
    let mut command = Command::new("powershell.exe");
    command.args(["-NoProfile", "-NonInteractive", "-Command", &script]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(process::CREATE_NO_WINDOW);
    }
    let status = command.status().context("gagal menjalankan powershell")?;
    if !status.success() {
        bail!("elevasi dibatalkan");
    }
    Ok(())
}
