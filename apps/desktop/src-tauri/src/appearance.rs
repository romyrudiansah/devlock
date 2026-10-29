//! Efek jendela Windows 11 (Mica). Di Windows 10 jendela tetap memakai latar solid dari CSS.

use tauri::Manager;

/// Apakah Mica berhasil dipasang di jendela utama; UI membuat latarnya transparan bila ya.
pub struct Mica(pub bool);

/// Mica hanya tersedia di Windows 11 (build 22000 ke atas).
#[cfg(windows)]
fn supports_mica() -> bool {
    use winreg::enums::HKEY_LOCAL_MACHINE;
    winreg::RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion")
        .and_then(|key| key.get_value::<String, _>("CurrentBuildNumber"))
        .ok()
        .and_then(|build| build.trim().parse::<u32>().ok())
        .is_some_and(|build| build >= 22000)
}

#[cfg(not(windows))]
fn supports_mica() -> bool {
    false
}

/// Pasang Mica (mengikuti tema terang/gelap Windows) dan simpan hasilnya untuk UI.
pub fn apply(app: &tauri::AppHandle) {
    let applied = supports_mica()
        && app.get_webview_window("main").is_some_and(|window| {
            use tauri::window::{Effect, EffectsBuilder};
            window
                .set_effects(EffectsBuilder::new().effect(Effect::Mica).build())
                .is_ok()
        });
    app.manage(Mica(applied));
}

#[tauri::command]
pub fn window_mica(state: tauri::State<'_, Mica>) -> bool {
    state.0
}
