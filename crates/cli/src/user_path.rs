//! Menambah/menghapus folder di PATH user (registry `HKCU\Environment`).
//!
//! Nilai dibaca dan ditulis mentah agar tipe `REG_EXPAND_SZ` dan referensi `%VAR%` tetap utuh.

use std::path::Path;

use anyhow::{Context, Result};

#[cfg(windows)]
pub fn add(dir: &Path) -> Result<bool> {
    let (mut entries, vtype) = read()?;
    let dir = dir.to_string_lossy().to_string();
    let before = entries.clone();
    entries.retain(|e| !same(e, &dir));
    entries.insert(0, dir);
    if entries == before {
        return Ok(false);
    }
    write(&entries, vtype)?;
    Ok(true)
}

#[cfg(windows)]
pub fn remove(dir: &Path) -> Result<bool> {
    let (mut entries, vtype) = read()?;
    let dir = dir.to_string_lossy().to_string();
    let len = entries.len();
    entries.retain(|e| !same(e, &dir));
    if entries.len() == len {
        return Ok(false);
    }
    write(&entries, vtype)?;
    Ok(true)
}

#[cfg(not(windows))]
pub fn add(_: &Path) -> Result<bool> {
    anyhow::bail!("hanya didukung di Windows")
}

#[cfg(not(windows))]
pub fn remove(_: &Path) -> Result<bool> {
    anyhow::bail!("hanya didukung di Windows")
}

fn same(a: &str, b: &str) -> bool {
    let norm = |s: &str| s.trim().trim_end_matches(['\\', '/']).to_lowercase();
    norm(a) == norm(b)
}

#[cfg(windows)]
fn read() -> Result<(Vec<String>, winreg::enums::RegType)> {
    use winreg::enums::{RegType, HKEY_CURRENT_USER};
    let env = winreg::RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey("Environment")
        .context("gagal membuka HKCU\\Environment")?;
    let raw = match env.get_raw_value("Path") {
        Ok(raw) => raw,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok((Vec::new(), RegType::REG_EXPAND_SZ))
        }
        Err(e) => return Err(e).context("gagal membaca PATH user"),
    };
    let wide: Vec<u16> = raw
        .bytes
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .take_while(|&c| c != 0)
        .collect();
    let text = String::from_utf16_lossy(&wide);
    let entries = text
        .split(';')
        .filter(|e| !e.trim().is_empty())
        .map(str::to_string)
        .collect();
    Ok((entries, raw.vtype))
}

#[cfg(windows)]
fn write(entries: &[String], vtype: winreg::enums::RegType) -> Result<()> {
    use winreg::enums::{HKEY_CURRENT_USER, KEY_SET_VALUE};
    let env = winreg::RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey_with_flags("Environment", KEY_SET_VALUE)
        .context("gagal membuka HKCU\\Environment")?;
    let bytes: Vec<u8> = entries
        .join(";")
        .encode_utf16()
        .chain(std::iter::once(0))
        .flat_map(u16::to_le_bytes)
        .collect();
    env.set_raw_value("Path", &winreg::RegValue { bytes, vtype })
        .context("gagal menulis PATH user")?;
    broadcast_change();
    Ok(())
}

/// Beri tahu Explorer & aplikasi lain bahwa environment berubah (terminal baru langsung dapat PATH baru).
#[cfg(windows)]
fn broadcast_change() {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SendMessageTimeoutW, HWND_BROADCAST, SMTO_ABORTIFHUNG, WM_SETTINGCHANGE,
    };
    let name: Vec<u16> = "Environment"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    // SAFETY: `name` hidup selama panggilan; hasil diabaikan (best effort).
    unsafe {
        SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            0,
            name.as_ptr() as isize,
            SMTO_ABORTIFHUNG,
            5000,
            std::ptr::null_mut(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::same;

    #[test]
    fn compares_paths_loosely() {
        assert!(same("D:\\DevLock\\shims\\", "d:\\devlock\\shims"));
        assert!(!same("D:\\DevLock\\shims", "D:\\DevLock\\bin"));
    }
}
