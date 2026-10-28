use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};

use anyhow::{bail, Context, Result};
use devlock_core::{runtime, sites, tunnel, Config, Kind, Paths};

pub fn run(name: &str) -> Result<()> {
    let paths = Paths::discover()?;
    let config = Config::load(&paths)?;
    let all = sites::discover(&paths, &config)?;
    let Some(site) = all.iter().find(|s| s.name.eq_ignore_ascii_case(name)) else {
        bail!(
            "site `{name}` tidak ada (site: {})",
            all.iter()
                .map(|s| s.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
    };
    let Some(version) = runtime::resolve_installed(&paths, Kind::Cloudflared, "latest")? else {
        bail!("cloudflared belum terinstall; jalankan `devlock install cloudflared`");
    };
    let exe = paths
        .runtime_dir(Kind::Cloudflared.as_str(), &version)
        .join("cloudflared.exe");
    let spec = tunnel::command(&paths, &config, site, &exe)?;

    println!(
        "Membuka Cloudflare Quick Tunnel untuk {} (cloudflared {version})…",
        site.domain()
    );
    println!("Siapa pun yang tahu URL-nya bisa membuka site ini selama tunnel aktif.");
    // Satu console dengan CLI (tanpa CREATE_NO_WINDOW) agar Ctrl+C ikut menghentikan cloudflared.
    let mut child = Command::new(&spec.program)
        .args(&spec.args)
        .current_dir(&spec.cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("gagal menjalankan {}", spec.program.display()))?;

    // cloudflared menulis log (termasuk URL publik) ke stderr.
    let stderr = child
        .stderr
        .take()
        .context("stderr cloudflared tidak tersedia")?;
    let mut shown = false;
    let mut tail: Vec<String> = Vec::new();
    for line in BufReader::new(stderr).lines() {
        let line = line?;
        if !shown {
            if let Some(url) = tunnel::public_url(&line) {
                shown = true;
                println!("\nURL publik: {url}\n");
                println!("Tekan Ctrl+C untuk menutup tunnel.");
                continue;
            }
        }
        if line.contains(" ERR ") {
            eprintln!("{line}");
        }
        tail.push(line);
        if tail.len() > 20 {
            tail.remove(0);
        }
    }
    let status = child.wait()?;
    if !shown {
        for line in &tail {
            eprintln!("{line}");
        }
        bail!("cloudflared berhenti sebelum tunnel siap ({status})");
    }
    Ok(())
}
