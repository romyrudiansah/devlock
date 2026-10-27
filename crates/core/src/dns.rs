//! DNS lokal untuk wildcard `*.<tld>`: server kecil di `127.0.0.1:53` (UDP + TCP) yang
//! menjawab semua nama di bawah TLD DevLock dengan `127.0.0.1`, ditambah aturan NRPT Windows
//! (Name Resolution Policy Table) yang mengarahkan hanya query `.<tld>` ke server itu.
//! DNS lain tetap memakai resolver biasa.

use std::io::{Read, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream, UdpSocket};
use std::process::Command;
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};

use crate::config::Config;
use crate::paths::Paths;
use crate::process;

/// NRPT hanya bisa menunjuk server DNS di port 53.
pub const PORT: u16 = 53;
pub const ADDRESS: Ipv4Addr = Ipv4Addr::LOCALHOST;
const TTL: u32 = 60;
/// Penanda aturan NRPT milik DevLock (kolom Comment).
const RULE_COMMENT: &str = "DevLock";

const TYPE_A: u16 = 1;
const TYPE_ANY: u16 = 255;
const RCODE_FORMERR: u8 = 1;
const RCODE_NXDOMAIN: u8 = 3;
const RCODE_REFUSED: u8 = 5;

/// `true` bila DNS wildcard aktif sehingga domain `*.<tld>` tidak perlu ditulis ke file hosts.
pub fn covers(config: &Config, domain: &str) -> bool {
    config.dns.enabled && in_zone(domain, &config.tld)
}

fn in_zone(name: &str, tld: &str) -> bool {
    let name = name.trim_end_matches('.').to_ascii_lowercase();
    let tld = tld.to_ascii_lowercase();
    name.len() > tld.len() + 1 && name.ends_with(&format!(".{tld}"))
}

/// Susun jawaban untuk satu paket query DNS. `None` = paket diabaikan (bukan query).
pub fn answer(query: &[u8], tld: &str) -> Option<Vec<u8>> {
    if query.len() < 12 {
        return None;
    }
    let flags = u16::from_be_bytes([query[2], query[3]]);
    if flags & 0x8000 != 0 {
        return None; // paket jawaban, bukan query
    }
    let opcode = (flags >> 11) & 0xF;
    let rd = flags & 0x0100;
    let qdcount = u16::from_be_bytes([query[4], query[5]]);

    let mut reply = Vec::with_capacity(query.len() + 16);
    reply.extend_from_slice(&query[0..2]);
    let header = |rcode: u8, answers: u16, questions: u16| -> [u8; 10] {
        // QR=1, opcode disalin, AA=1, RD disalin, RA=0.
        let flags = 0x8000 | (opcode << 11) | 0x0400 | rd | rcode as u16;
        let mut h = [0u8; 10];
        h[0..2].copy_from_slice(&flags.to_be_bytes());
        h[2..4].copy_from_slice(&questions.to_be_bytes());
        h[4..6].copy_from_slice(&answers.to_be_bytes());
        h
    };

    let parsed = (opcode == 0 && qdcount == 1)
        .then(|| parse_question(&query[12..]))
        .flatten();
    let Some((name, qtype, question_len)) = parsed else {
        reply.extend_from_slice(&header(RCODE_FORMERR, 0, 0));
        return Some(reply);
    };
    let question = &query[12..12 + question_len];

    let tld_only = name.eq_ignore_ascii_case(tld);
    if !in_zone(&name, tld) && !tld_only {
        reply.extend_from_slice(&header(RCODE_REFUSED, 0, 1));
        reply.extend_from_slice(question);
        return Some(reply);
    }
    if tld_only {
        // `test` sendiri bukan site.
        reply.extend_from_slice(&header(RCODE_NXDOMAIN, 0, 1));
        reply.extend_from_slice(question);
        return Some(reply);
    }
    let with_answer = qtype == TYPE_A || qtype == TYPE_ANY;
    // AAAA dan tipe lain: NOERROR tanpa jawaban, agar browser memakai IPv4
    // (nginx DevLock hanya mendengarkan di 127.0.0.1).
    reply.extend_from_slice(&header(0, u16::from(with_answer), 1));
    reply.extend_from_slice(question);
    if with_answer {
        reply.extend_from_slice(&[0xC0, 0x0C]); // pointer ke nama di question
        reply.extend_from_slice(&TYPE_A.to_be_bytes());
        reply.extend_from_slice(&1u16.to_be_bytes()); // class IN
        reply.extend_from_slice(&TTL.to_be_bytes());
        reply.extend_from_slice(&4u16.to_be_bytes());
        reply.extend_from_slice(&ADDRESS.octets());
    }
    Some(reply)
}

/// Nama (tanpa titik akhir), qtype, dan panjang bagian question dalam byte.
fn parse_question(data: &[u8]) -> Option<(String, u16, usize)> {
    let mut labels = Vec::new();
    let mut pos = 0;
    loop {
        let len = *data.get(pos)? as usize;
        pos += 1;
        if len == 0 {
            break;
        }
        if len & 0xC0 != 0 || labels.len() > 127 {
            return None; // kompresi tidak dipakai di question
        }
        let label = data.get(pos..pos + len)?;
        labels.push(String::from_utf8_lossy(label).into_owned());
        pos += len;
    }
    let qtype = u16::from_be_bytes([*data.get(pos)?, *data.get(pos + 1)?]);
    let qclass = u16::from_be_bytes([*data.get(pos + 2)?, *data.get(pos + 3)?]);
    if qclass != 1 && qclass != 255 {
        return None;
    }
    Some((labels.join("."), qtype, pos + 4))
}

/// Penjelasan bila query ke `127.0.0.1:53` tidak sampai ke socket DevLock.
pub const PORT_TAKEN_HINT: &str = "query DNS ke 127.0.0.1:53 diambil program lain — biasanya layanan Windows \"Internet Connection Sharing (ICS)\" / SharedAccess yang mendengarkan di 0.0.0.0:53 (dipakai Mobile Hotspot dan switch NAT Hyper-V). Hentikan layanan itu (`Stop-Service SharedAccess` sebagai administrator) atau tetap pakai file hosts";

/// Kirim satu query ke `socket` (sudah di-bind ke 127.0.0.1:53) dan cek apakah benar-benar
/// sampai. Di Windows bind ke 127.0.0.1:53 bisa berhasil padahal paketnya diterima socket
/// `0.0.0.0:53` milik program lain.
fn receives_own_query(socket: &UdpSocket) -> bool {
    let Ok(client) = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)) else {
        return false;
    };
    let Ok(client_addr) = client.local_addr() else {
        return false;
    };
    let probe = build_query(0x5A5A, "devlock-probe.invalid", TYPE_A);
    if client.send_to(&probe, (ADDRESS, PORT)).is_err() {
        return false;
    }
    let _ = socket.set_read_timeout(Some(Duration::from_millis(300)));
    let deadline = Instant::now() + Duration::from_secs(1);
    let mut buf = [0u8; 512];
    let mut received = false;
    while !received && Instant::now() < deadline {
        if let Ok((len, peer)) = socket.recv_from(&mut buf) {
            received = peer == client_addr && buf[..len] == probe[..];
        }
    }
    let _ = socket.set_read_timeout(None);
    received
}

/// `true` bila server DNS bisa dijalankan di 127.0.0.1:53: port bisa di-bind dan query
/// benar-benar sampai (tidak diambil alih program lain).
pub fn port_usable() -> bool {
    UdpSocket::bind((ADDRESS, PORT)).is_ok_and(|socket| receives_own_query(&socket))
}

/// Jalankan server DNS (dipanggil service `dns` lewat `devlock dns serve`). Tidak kembali
/// kecuali socket gagal dibuka.
pub fn serve(tld: &str) -> Result<()> {
    let udp = UdpSocket::bind((ADDRESS, PORT))
        .with_context(|| format!("gagal membuka UDP {ADDRESS}:{PORT} (dipakai program lain?)"))?;
    if !receives_own_query(&udp) {
        bail!("{PORT_TAKEN_HINT}");
    }
    let tcp = TcpListener::bind((ADDRESS, PORT))
        .with_context(|| format!("gagal membuka TCP {ADDRESS}:{PORT} (dipakai program lain?)"))?;
    tracing::info!("DNS DevLock mendengarkan di {ADDRESS}:{PORT} untuk *.{tld}");

    let tcp_tld = tld.to_string();
    std::thread::spawn(move || {
        for stream in tcp.incoming().flatten() {
            let tld = tcp_tld.clone();
            std::thread::spawn(move || {
                let _ = serve_tcp(stream, &tld);
            });
        }
    });

    let mut buf = [0u8; 1500];
    loop {
        let (len, peer) = match udp.recv_from(&mut buf) {
            Ok(r) => r,
            // Windows melaporkan ICMP "port unreachable" dari jawaban sebelumnya sebagai error
            // recv (WSAECONNRESET); abaikan saja.
            Err(_) => continue,
        };
        if let Some(reply) = answer(&buf[..len], tld) {
            let _ = udp.send_to(&reply, peer);
        }
    }
}

/// DNS lewat TCP: tiap pesan diawali panjang 2 byte.
fn serve_tcp(mut stream: TcpStream, tld: &str) -> std::io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    loop {
        let mut len = [0u8; 2];
        if stream.read_exact(&mut len).is_err() {
            return Ok(());
        }
        let mut msg = vec![0u8; u16::from_be_bytes(len) as usize];
        stream.read_exact(&mut msg)?;
        if let Some(reply) = answer(&msg, tld) {
            stream.write_all(&(reply.len() as u16).to_be_bytes())?;
            stream.write_all(&reply)?;
        }
    }
}

/// TLD dipakai di skrip PowerShell; hanya huruf, angka, dan `-`.
fn checked_tld(tld: &str) -> Result<&str> {
    if !crate::sites::is_valid_label(tld) {
        bail!("TLD `{tld}` tidak valid");
    }
    Ok(tld)
}

fn powershell(script: &str) -> Result<std::process::Output> {
    let mut command = Command::new("powershell.exe");
    command.args(["-NoProfile", "-NonInteractive", "-Command", script]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(process::CREATE_NO_WINDOW);
    }
    command.output().context("gagal menjalankan powershell")
}

/// Namespace aturan NRPT DevLock yang terpasang (mis. `.test`); kosong bila tidak ada.
pub fn installed_rules() -> Result<Vec<String>> {
    let output = powershell(&format!(
        "Get-DnsClientNrptRule | Where-Object {{ $_.Comment -eq '{RULE_COMMENT}' }} | ForEach-Object {{ $_.Namespace }}"
    ))?;
    if !output.status.success() {
        bail!(
            "Get-DnsClientNrptRule gagal: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(String::from)
        .collect())
}

/// `true` bila aturan NRPT DevLock untuk `.<tld>` terpasang.
pub fn rule_installed(tld: &str) -> bool {
    let wanted = format!(".{}", tld.to_ascii_lowercase());
    installed_rules()
        .map(|rules| rules.iter().any(|r| r.eq_ignore_ascii_case(&wanted)))
        .unwrap_or(false)
}

/// Pasang (`Some(tld)`) atau hapus (`None`) aturan NRPT DevLock. Butuh hak administrator;
/// dipanggil dari `devlock dns install|remove` (elevated).
pub fn apply_rule_elevated(tld: Option<&str>) -> Result<()> {
    let mut script = format!(
        "$ErrorActionPreference = 'Stop'; Get-DnsClientNrptRule | Where-Object {{ $_.Comment -eq '{RULE_COMMENT}' }} | ForEach-Object {{ Remove-DnsClientNrptRule -Name $_.Name -Force }};"
    );
    if let Some(tld) = tld {
        let tld = checked_tld(tld)?;
        script.push_str(&format!(
            " Add-DnsClientNrptRule -Namespace '.{tld}' -NameServers '{ADDRESS}' -Comment '{RULE_COMMENT}' -DisplayName 'DevLock *.{tld}' | Out-Null;"
        ));
    }
    script.push_str(" Clear-DnsClientCache");
    let output = powershell(&script)?;
    if !output.status.success() {
        bail!(
            "gagal mengubah aturan NRPT: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}

/// Pasang/hapus aturan NRPT lewat `devlock dns install|remove` yang dijalankan elevated
/// (UAC sekali); proses itu juga menyamakan file hosts.
pub fn apply_rule(paths: &Paths, tld: Option<&str>) -> Result<()> {
    if let Some(tld) = tld {
        checked_tld(tld)?;
    }
    let exe = crate::launcher::devlock_exe()?;
    let escape = |s: &str| s.replace('\'', "''");
    let action = match tld {
        Some(tld) => format!("'install','--tld','{tld}'"),
        None => "'remove'".to_string(),
    };
    let script = format!(
        "Start-Process -FilePath '{exe}' -ArgumentList @('dns',{action},'--root','\"{root}\"') -Verb RunAs -Wait -WindowStyle Hidden",
        exe = escape(&exe.to_string_lossy()),
        root = escape(&paths.root().to_string_lossy()),
    );
    let output = powershell(&script)?;
    if !output.status.success() {
        bail!("elevasi dibatalkan");
    }
    let wanted = tld.map(|t| format!(".{t}"));
    let rules = installed_rules()?;
    let ok = match &wanted {
        Some(ns) => rules.iter().any(|r| r.eq_ignore_ascii_case(ns)),
        None => rules.is_empty(),
    };
    if !ok {
        bail!("aturan NRPT belum berubah (UAC ditolak?)");
    }
    Ok(())
}

/// Tanya server DNS DevLock langsung (tanpa lewat resolver Windows): `true` bila menjawab
/// `127.0.0.1` untuk `devlock-check.<tld>`.
pub fn server_responds(tld: &str) -> bool {
    let Ok(socket) = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)) else {
        return false;
    };
    let _ = socket.set_read_timeout(Some(Duration::from_millis(500)));
    let query = build_query(0x4444, &format!("devlock-check.{tld}"), TYPE_A);
    if socket.send_to(&query, (ADDRESS, PORT)).is_err() {
        return false;
    }
    let mut buf = [0u8; 512];
    match socket.recv(&mut buf) {
        Ok(len) => len >= 4 && buf[..len].ends_with(&ADDRESS.octets()),
        Err(_) => false,
    }
}

fn build_query(id: u16, name: &str, qtype: u16) -> Vec<u8> {
    let mut q = Vec::new();
    q.extend_from_slice(&id.to_be_bytes());
    q.extend_from_slice(&0x0100u16.to_be_bytes()); // RD
    q.extend_from_slice(&[0, 1, 0, 0, 0, 0, 0, 0]);
    for label in name.split('.') {
        q.push(label.len() as u8);
        q.extend_from_slice(label.as_bytes());
    }
    q.push(0);
    q.extend_from_slice(&qtype.to_be_bytes());
    q.extend_from_slice(&1u16.to_be_bytes());
    q
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rcode(reply: &[u8]) -> u8 {
        reply[3] & 0x0F
    }

    fn ancount(reply: &[u8]) -> u16 {
        u16::from_be_bytes([reply[6], reply[7]])
    }

    #[test]
    fn answers_wildcard_with_localhost() {
        let query = build_query(7, "Blog.TEST", TYPE_A);
        let reply = answer(&query, "test").unwrap();
        assert_eq!(&reply[0..2], &7u16.to_be_bytes());
        assert_eq!(reply[2] & 0x80, 0x80, "QR");
        assert_eq!(rcode(&reply), 0);
        assert_eq!(ancount(&reply), 1);
        assert!(reply.ends_with(&[127, 0, 0, 1]));

        let deep = answer(&build_query(8, "api.shop.test", TYPE_A), "test").unwrap();
        assert_eq!(ancount(&deep), 1);
    }

    #[test]
    fn aaaa_gets_empty_noerror() {
        let reply = answer(&build_query(9, "blog.test", 28), "test").unwrap();
        assert_eq!(rcode(&reply), 0);
        assert_eq!(ancount(&reply), 0);
    }

    #[test]
    fn refuses_other_zones_and_bare_tld() {
        let other = answer(&build_query(1, "example.com", TYPE_A), "test").unwrap();
        assert_eq!(rcode(&other), RCODE_REFUSED);
        let fake = answer(&build_query(1, "nottest", TYPE_A), "test").unwrap();
        assert_eq!(rcode(&fake), RCODE_REFUSED);
        let bare = answer(&build_query(1, "test", TYPE_A), "test").unwrap();
        assert_eq!(rcode(&bare), RCODE_NXDOMAIN);
    }

    #[test]
    fn ignores_responses_and_rejects_garbage() {
        let mut query = build_query(1, "blog.test", TYPE_A);
        query[2] |= 0x80;
        assert!(answer(&query, "test").is_none());
        assert!(answer(&[0u8; 5], "test").is_none());
        let mut truncated = build_query(1, "blog.test", TYPE_A);
        truncated.truncate(16);
        assert_eq!(rcode(&answer(&truncated, "test").unwrap()), RCODE_FORMERR);
    }

    #[test]
    fn covers_only_when_enabled() {
        let mut config = Config::default();
        assert!(!covers(&config, "blog.test"));
        config.dns.enabled = true;
        assert!(covers(&config, "blog.test"));
        assert!(!covers(&config, "blog.local"));
        assert!(!covers(&config, "test"));
    }
}
