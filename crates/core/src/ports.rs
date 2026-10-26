//! Siapa yang sedang memakai sebuah port TCP (IPv4 & IPv6), untuk mendeteksi bentrok
//! dengan Laragon/XAMPP/IIS sebelum service dijalankan dan di `devlock doctor`.

use std::fmt;
use std::net::IpAddr;
use std::path::{Path, PathBuf};

/// Socket TCP yang sedang listen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listener {
    pub addr: IpAddr,
    pub port: u16,
    pub pid: u32,
    /// Path executable pemilik; `None` bila tidak bisa dibaca (mis. proses System).
    pub exe: Option<PathBuf>,
}

impl Listener {
    pub fn process_name(&self) -> String {
        self.exe
            .as_deref()
            .and_then(Path::file_name)
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| format!("PID {}", self.pid))
    }

    fn owned_by(&self, root: &Path) -> bool {
        self.exe.as_deref().is_some_and(|exe| {
            let exe = exe.to_string_lossy().to_lowercase().replace('/', "\\");
            let root = root.to_string_lossy().to_lowercase().replace('/', "\\");
            exe.starts_with(root.trim_end_matches('\\'))
        })
    }
}

/// Hasil pemeriksaan port untuk alamat yang akan dipakai DevLock.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Check {
    /// Program lain memakai alamat yang sama: service DevLock tidak bisa start.
    pub blocking: Vec<Listener>,
    /// Program lain memakai `0.0.0.0` / `[::]` / alamat loopback lain di port yang sama.
    /// DevLock tetap bisa jalan di alamatnya sendiri, tapi request lewat alamat lain
    /// (IPv6, LAN) masuk ke program itu.
    pub shadowing: Vec<Listener>,
}

impl Check {
    pub fn is_clear(&self) -> bool {
        self.blocking.is_empty() && self.shadowing.is_empty()
    }
}

/// Ringkas daftar listener: `httpd.exe (PID 3448) di 0.0.0.0, ::`.
pub struct Summary<'a>(pub &'a [Listener]);

impl fmt::Display for Summary<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut groups: Vec<(u32, String, Vec<String>)> = Vec::new();
        for l in self.0 {
            match groups.iter_mut().find(|(pid, _, _)| *pid == l.pid) {
                Some((_, _, addrs)) => addrs.push(l.addr.to_string()),
                None => groups.push((l.pid, l.process_name(), vec![l.addr.to_string()])),
            }
        }
        let parts: Vec<String> = groups
            .into_iter()
            .map(|(pid, name, addrs)| format!("{name} (PID {pid}) di {}", addrs.join(", ")))
            .collect();
        f.write_str(&parts.join("; "))
    }
}

/// Periksa port untuk `listen` (alamat yang akan dipakai DevLock). Proses yang
/// executable-nya berada di bawah `root` (milik DevLock sendiri) diabaikan.
pub fn check(port: u16, listen: IpAddr, root: &Path) -> Check {
    classify(&listeners(port), listen, root)
}

fn classify(listeners: &[Listener], listen: IpAddr, root: &Path) -> Check {
    let mut result = Check::default();
    for l in listeners.iter().filter(|l| !l.owned_by(root)) {
        if l.addr == listen {
            result.blocking.push(l.clone());
        } else if l.addr.is_unspecified() || l.addr.is_loopback() {
            result.shadowing.push(l.clone());
        }
    }
    result
}

/// Semua socket yang listen di `port` (IPv4 dan IPv6).
#[cfg(windows)]
pub fn listeners(port: u16) -> Vec<Listener> {
    let mut out = win::tcp_listeners_v4(port);
    out.extend(win::tcp_listeners_v6(port));
    for l in &mut out {
        l.exe = win::process_path(l.pid);
    }
    out
}

#[cfg(not(windows))]
pub fn listeners(_port: u16) -> Vec<Listener> {
    Vec::new()
}

#[cfg(windows)]
mod win {
    use std::ffi::c_void;
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
    use std::path::PathBuf;

    use windows_sys::Win32::Foundation::{CloseHandle, ERROR_INSUFFICIENT_BUFFER, NO_ERROR};
    use windows_sys::Win32::NetworkManagement::IpHelper::{
        GetExtendedTcpTable, MIB_TCP6TABLE_OWNER_PID, MIB_TCPTABLE_OWNER_PID,
        TCP_TABLE_OWNER_PID_LISTENER,
    };
    use windows_sys::Win32::Networking::WinSock::{AF_INET, AF_INET6};
    use windows_sys::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
    };

    use super::Listener;

    /// Isi tabel TCP (buffer `u32` agar alignment struct Windows terpenuhi).
    fn table(family: u32) -> Option<Vec<u32>> {
        let mut size = 0u32;
        for _ in 0..3 {
            let mut buf = vec![0u32; (size as usize).div_ceil(4).max(1)];
            // SAFETY: `buf` berukuran `size` byte (dibulatkan ke atas); API menulis tabel ke sana.
            let ret = unsafe {
                GetExtendedTcpTable(
                    buf.as_mut_ptr() as *mut c_void,
                    &mut size,
                    0,
                    family,
                    TCP_TABLE_OWNER_PID_LISTENER,
                    0,
                )
            };
            match ret {
                NO_ERROR => return Some(buf),
                ERROR_INSUFFICIENT_BUFFER => continue,
                _ => return None,
            }
        }
        None
    }

    pub fn tcp_listeners_v4(port: u16) -> Vec<Listener> {
        let Some(buf) = table(AF_INET as u32) else {
            return Vec::new();
        };
        // SAFETY: API sukses menulis MIB_TCPTABLE_OWNER_PID dengan dwNumEntries baris.
        let rows = unsafe {
            let table = &*(buf.as_ptr() as *const MIB_TCPTABLE_OWNER_PID);
            std::slice::from_raw_parts(table.table.as_ptr(), table.dwNumEntries as usize)
        };
        rows.iter()
            // Port disimpan dalam network byte order di 16 bit bawah.
            .filter(|r| u16::from_be(r.dwLocalPort as u16) == port)
            .map(|r| Listener {
                addr: IpAddr::V4(Ipv4Addr::from(r.dwLocalAddr.to_ne_bytes())),
                port,
                pid: r.dwOwningPid,
                exe: None,
            })
            .collect()
    }

    pub fn tcp_listeners_v6(port: u16) -> Vec<Listener> {
        let Some(buf) = table(AF_INET6 as u32) else {
            return Vec::new();
        };
        // SAFETY: API sukses menulis MIB_TCP6TABLE_OWNER_PID dengan dwNumEntries baris.
        let rows = unsafe {
            let table = &*(buf.as_ptr() as *const MIB_TCP6TABLE_OWNER_PID);
            std::slice::from_raw_parts(table.table.as_ptr(), table.dwNumEntries as usize)
        };
        rows.iter()
            .filter(|r| u16::from_be(r.dwLocalPort as u16) == port)
            .map(|r| Listener {
                addr: IpAddr::V6(Ipv6Addr::from(r.ucLocalAddr)),
                port,
                pid: r.dwOwningPid,
                exe: None,
            })
            .collect()
    }

    pub fn process_path(pid: u32) -> Option<PathBuf> {
        if pid == 0 {
            return None;
        }
        // SAFETY: handle ditutup sebelum keluar; buffer `len` karakter u16.
        unsafe {
            let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if handle.is_null() {
                return None;
            }
            let mut buf = vec![0u16; 1024];
            let mut len = buf.len() as u32;
            let ok = QueryFullProcessImageNameW(handle, 0, buf.as_mut_ptr(), &mut len);
            CloseHandle(handle);
            (ok != 0).then(|| PathBuf::from(String::from_utf16_lossy(&buf[..len as usize])))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, Ipv6Addr};

    fn listener(addr: IpAddr, exe: &str) -> Listener {
        Listener {
            addr,
            port: 80,
            pid: 3448,
            exe: Some(PathBuf::from(exe)),
        }
    }

    #[test]
    fn classifies_conflicts() {
        let local = IpAddr::V4(Ipv4Addr::LOCALHOST);
        let all = [
            listener(
                IpAddr::V4(Ipv4Addr::UNSPECIFIED),
                "D:\\laragon\\bin\\httpd.exe",
            ),
            listener(
                IpAddr::V6(Ipv6Addr::UNSPECIFIED),
                "D:\\laragon\\bin\\httpd.exe",
            ),
            listener(
                local,
                "D:\\DevLock-Apps\\runtimes\\nginx\\1.30.6\\nginx.exe",
            ),
            listener(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 5)), "C:\\other.exe"),
        ];
        let check = classify(&all, local, Path::new("D:/DevLock-Apps"));
        // nginx milik DevLock diabaikan; alamat LAN spesifik tidak dihitung.
        assert!(check.blocking.is_empty());
        assert_eq!(check.shadowing.len(), 2);
        assert_eq!(
            Summary(&check.shadowing).to_string(),
            "httpd.exe (PID 3448) di 0.0.0.0, ::"
        );

        let iis = [listener(local, "C:\\Windows\\system32\\inetsrv\\w3wp.exe")];
        let check = classify(&iis, local, Path::new("D:/DevLock-Apps"));
        assert_eq!(check.blocking.len(), 1);
        assert!(!check.is_clear());
    }

    #[cfg(windows)]
    #[test]
    fn sees_own_listeners_with_pid_and_exe() {
        use std::net::TcpListener;
        let v4 = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = v4.local_addr().unwrap().port();
        let found = listeners(port);
        let me = std::process::id();
        let own = found
            .iter()
            .find(|l| l.addr == IpAddr::V4(Ipv4Addr::LOCALHOST) && l.pid == me)
            .expect("listener IPv4 sendiri terdeteksi");
        let exe_name = |p: &Path| p.file_name().map(|n| n.to_ascii_lowercase());
        assert_eq!(
            own.exe.as_deref().and_then(exe_name),
            exe_name(&std::env::current_exe().unwrap())
        );

        if let Ok(v6) = TcpListener::bind("[::1]:0") {
            let port = v6.local_addr().unwrap().port();
            assert!(listeners(port)
                .iter()
                .any(|l| l.addr == IpAddr::V6(Ipv6Addr::LOCALHOST) && l.pid == me));
        }
    }
}
