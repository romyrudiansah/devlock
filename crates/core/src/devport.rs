//! Port dev server project Node tanpa `proxy_port`: DevLock memberi `PORT` tetap per site,
//! lalu membaca port sebenarnya dari output dev server (Vite, Next.js, Nuxt, Astro, …)
//! karena tidak semua dev server menghormati `PORT`.

use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use crate::paths::Paths;

/// Rentang port yang dibagikan lewat `PORT` (di luar port bawaan dev server populer).
const ASSIGNED_BASE: u16 = 3100;
const ASSIGNED_RANGE: u16 = 800;

/// Bagian akhir log yang dibaca untuk mencari URL dev server.
const LOG_TAIL_BYTES: u64 = 64 * 1024;

/// Port `PORT` untuk site `name`: stabil antar restart dan berbeda untuk kebanyakan site.
pub fn assigned_port(name: &str) -> u16 {
    // FNV-1a: hasilnya tetap antar versi Rust (berbeda dengan `DefaultHasher`).
    let hash = name.bytes().fold(0x811c_9dc5_u32, |h, b| {
        (h ^ u32::from(b)).wrapping_mul(0x0100_0193)
    });
    ASSIGNED_BASE + (hash % u32::from(ASSIGNED_RANGE)) as u16
}

/// `tmp\dev-ports\<site>` berisi port hasil deteksi terakhir.
pub fn detected_file(paths: &Paths, site: &str) -> PathBuf {
    paths.tmp().join("dev-ports").join(site)
}

pub fn read_detected(paths: &Paths, site: &str) -> Option<u16> {
    std::fs::read_to_string(detected_file(paths, site))
        .ok()?
        .trim()
        .parse()
        .ok()
}

/// Simpan port hasil deteksi; `true` bila nilainya berubah.
pub fn store_detected(paths: &Paths, site: &str, port: u16) -> std::io::Result<bool> {
    if read_detected(paths, site) == Some(port) {
        return Ok(false);
    }
    let file = detected_file(paths, site);
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(file, port.to_string())?;
    Ok(true)
}

/// Port dari URL loopback terakhir di bagian akhir `log`.
pub fn detect_in_log(log: &Path) -> Option<u16> {
    let mut file = std::fs::File::open(log).ok()?;
    let len = file.metadata().ok()?.len();
    file.seek(SeekFrom::Start(len.saturating_sub(LOG_TAIL_BYTES)))
        .ok()?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).ok()?;
    detect(&String::from_utf8_lossy(&bytes))
}

/// Port dari URL `http(s)://localhost|127.0.0.1|0.0.0.0|[::1]|[::]:<port>` terakhir di `output`.
/// URL alamat jaringan (`Network: http://192.168…`) diabaikan.
pub fn detect(output: &str) -> Option<u16> {
    const HOSTS: &[&str] = &["localhost", "127.0.0.1", "0.0.0.0", "[::1]", "[::]"];
    let text = strip_ansi(output);
    let mut found = None;
    for scheme in ["http://", "https://"] {
        for (at, _) in text.match_indices(scheme) {
            let rest = &text[at + scheme.len()..];
            let Some(host) = HOSTS.iter().find(|h| rest.starts_with(**h)) else {
                continue;
            };
            let Some(after) = rest[host.len()..].strip_prefix(':') else {
                continue;
            };
            let digits: String = after.chars().take_while(char::is_ascii_digit).collect();
            if let Ok(port) = digits.parse::<u16>() {
                if port > 0 && found.map_or(true, |(pos, _)| at > pos) {
                    found = Some((at, port));
                }
            }
        }
    }
    found.map(|(_, port)| port)
}

/// Buang escape warna/kursor ANSI (`ESC [ … huruf`) yang dipakai dev server untuk mewarnai URL.
fn strip_ansi(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            if chars.peek() == Some(&'[') {
                chars.next();
                for c in chars.by_ref() {
                    if c.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
            continue;
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_common_dev_servers() {
        let vite = "\n  VITE v7.1.0  ready in 312 ms\n\n  \u{1b}[32m➜\u{1b}[39m  \u{1b}[1mLocal\u{1b}[22m:   \u{1b}[36mhttp://localhost:\u{1b}[1m5174\u{1b}[22m/\u{1b}[39m\n  ➜  Network: http://192.168.1.10:5174/\n";
        assert_eq!(detect(vite), Some(5174));
        let next = "   ▲ Next.js 16.0.1\n   - Local:        http://localhost:3000\n   - Network:      http://10.0.0.5:3000\n";
        assert_eq!(detect(next), Some(3000));
        let angular = "** Angular Live Development Server is listening on localhost:4200, open your browser on http://localhost:4200/ **";
        assert_eq!(detect(angular), Some(4200));
        assert_eq!(detect("Listening on http://[::1]:4321/"), Some(4321));
        assert_eq!(detect("server at http://0.0.0.0:8080"), Some(8080));
        // Restart dengan port lain: URL terakhir yang menang.
        assert_eq!(
            detect("Local: http://localhost:5173/\nPort 5173 is in use\nLocal: https://127.0.0.1:5175/"),
            Some(5175)
        );
        assert_eq!(detect("Network: http://192.168.1.10:5173/"), None);
        assert_eq!(detect("compiling..."), None);
    }

    #[test]
    fn assigned_port_is_stable_and_in_range() {
        let port = assigned_port("dashboard");
        assert_eq!(port, assigned_port("dashboard"));
        assert!((ASSIGNED_BASE..ASSIGNED_BASE + ASSIGNED_RANGE).contains(&port));
        assert_ne!(assigned_port("dashboard"), assigned_port("shop"));
    }

    #[test]
    fn stores_detected_port_and_reads_log_tail() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path());
        assert_eq!(read_detected(&paths, "dash"), None);
        assert!(store_detected(&paths, "dash", 5173).unwrap());
        assert!(!store_detected(&paths, "dash", 5173).unwrap());
        assert_eq!(read_detected(&paths, "dash"), Some(5173));

        let log = tmp.path().join("dev-dash.log");
        let mut text = "x".repeat(LOG_TAIL_BYTES as usize * 2);
        text.push_str("\nLocal: http://localhost:5199/\n");
        std::fs::write(&log, text).unwrap();
        assert_eq!(detect_in_log(&log), Some(5199));
    }
}
