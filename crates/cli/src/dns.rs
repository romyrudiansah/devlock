use std::net::{IpAddr, Ipv4Addr, ToSocketAddrs};

use anyhow::{bail, Result};
use devlock_core::daemon::{self, Request};
use devlock_core::{dns, launcher, Config, Paths};

fn load() -> Result<(Paths, Config)> {
    let paths = Paths::discover()?;
    let config = Config::load(&paths)?;
    Ok((paths, config))
}

/// `true` bila resolver Windows menjawab `devlock-check.<tld>` dengan 127.0.0.1.
pub fn system_resolves(tld: &str) -> bool {
    format!("devlock-check.{tld}:80")
        .to_socket_addrs()
        .map(|mut addrs| addrs.any(|a| a.ip() == IpAddr::V4(Ipv4Addr::LOCALHOST)))
        .unwrap_or(false)
}

pub fn status() -> Result<()> {
    let (_, config) = load()?;
    let tld = &config.tld;
    let yes_no = |ok: bool, yes: &str, no: &str| if ok { yes.to_string() } else { no.to_string() };
    println!(
        "DNS wildcard   : {}",
        yes_no(
            config.dns.enabled,
            &format!("aktif untuk *.{tld}"),
            "nonaktif — `devlock dns enable`"
        )
    );
    let rules = dns::installed_rules().unwrap_or_default();
    println!(
        "Aturan NRPT    : {}",
        if rules.is_empty() {
            "tidak ada".to_string()
        } else {
            rules.join(", ")
        }
    );
    let responds = dns::server_responds(tld);
    let server = if responds {
        format!("menjawab di {}:{}", dns::ADDRESS, dns::PORT)
    } else if dns::port_usable() {
        "tidak berjalan (jalan bersama `devlock start`)".to_string()
    } else {
        format!("tidak bisa berjalan: {}", dns::PORT_TAKEN_HINT)
    };
    println!("Server DNS     : {server}");
    println!(
        "Resolusi sistem: {}",
        yes_no(
            system_resolves(tld),
            &format!("devlock-check.{tld} → 127.0.0.1"),
            &format!("devlock-check.{tld} belum mengarah ke 127.0.0.1")
        )
    );
    if config.dns.enabled
        && !rules
            .iter()
            .any(|r| r.eq_ignore_ascii_case(&format!(".{tld}")))
    {
        println!(
            "\nAturan NRPT tidak cocok dengan TLD `{tld}`; jalankan ulang `devlock dns enable`."
        );
    }
    Ok(())
}

pub fn enable() -> Result<()> {
    let (paths, config) = load()?;
    let tld = config.tld.clone();
    if !dns::server_responds(&tld) && !dns::port_usable() {
        bail!(
            "DNS wildcard tidak bisa diaktifkan: {}",
            dns::PORT_TAKEN_HINT
        );
    }
    Config::set_value(&paths, "dns", "enabled", toml_edit::value(true))?;
    println!("Windows akan meminta izin administrator (UAC) untuk memasang aturan DNS `.{tld}`.");
    if let Err(err) = dns::apply_rule(&paths, Some(&tld)) {
        Config::set_value(
            &paths,
            "dns",
            "enabled",
            toml_edit::value(config.dns.enabled),
        )?;
        return Err(err);
    }
    println!("Aturan NRPT dipasang: query *.{tld} diarahkan ke DNS DevLock (127.0.0.1:53).");
    println!("Domain *.{tld} kini tidak lagi ditulis ke file hosts (alias di luar .{tld} tetap).");
    apply_to_daemon(&config, true)?;
    if dns::server_responds(&tld) && system_resolves(&tld) {
        println!("Uji: devlock-check.{tld} → 127.0.0.1 ✓");
    }
    Ok(())
}

pub fn disable() -> Result<()> {
    let (paths, config) = load()?;
    Config::set_value(&paths, "dns", "enabled", toml_edit::value(false))?;
    println!("Windows akan meminta izin administrator (UAC) untuk menghapus aturan DNS DevLock.");
    if let Err(err) = dns::apply_rule(&paths, None) {
        Config::set_value(
            &paths,
            "dns",
            "enabled",
            toml_edit::value(config.dns.enabled),
        )?;
        return Err(err);
    }
    println!("Aturan NRPT DevLock dihapus; domain site kembali ditulis ke file hosts.");
    apply_to_daemon(&config, false)
}

/// Daemon yang berjalan langsung memuat ulang konfigurasi (service `dns` ikut start/stop).
fn apply_to_daemon(config: &Config, start: bool) -> Result<()> {
    if daemon::send(config, &Request::Reload)?.is_none() {
        println!("DNS DevLock ikut berjalan saat `devlock start`.");
        return Ok(());
    }
    if start {
        if let Some(response) = daemon::send(
            config,
            &Request::Start {
                target: "dns".into(),
            },
        )? {
            match response.services.iter().find(|s| s.name == "dns") {
                Some(s) if s.state == "running" => println!("Service dns berjalan."),
                Some(s) => println!(
                    "Service dns: {}{} — lihat `devlock logs dns`",
                    s.state,
                    s.error
                        .as_deref()
                        .map(|e| format!(" ({e})"))
                        .unwrap_or_default()
                ),
                None => println!("Service dns belum muncul; jalankan `devlock reload`."),
            }
        }
    }
    Ok(())
}

/// Bagian elevated dari `enable`/`disable`: ubah aturan NRPT lalu samakan file hosts.
pub fn apply_elevated(tld: Option<&str>) -> Result<()> {
    dns::apply_rule_elevated(tld)?;
    let (paths, config) = load()?;
    if let Err(err) = launcher::sync_hosts(&paths, &config, true) {
        tracing::warn!("file hosts tidak diperbarui: {err:#}");
    }
    Ok(())
}

pub fn serve() -> Result<()> {
    let (_, config) = load()?;
    dns::serve(&config.tld)
}
