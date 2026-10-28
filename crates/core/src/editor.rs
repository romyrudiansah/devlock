//! Mencari editor kode untuk tombol "Editor" di GUI (`[gui] editor` atau deteksi otomatis).

use std::path::{Path, PathBuf};

/// Editor yang dideteksi otomatis, urut prioritas: perintah CLI masing-masing.
pub const KNOWN: &[&str] = &[
    "code",
    "cursor",
    "windsurf",
    "zed",
    "subl",
    "phpstorm64",
    "phpstorm",
];

const EXTENSIONS: &[&str] = &["exe", "cmd", "bat"];

/// Editor untuk dipakai: `configured` (nama perintah atau path lengkap) bila diisi, selain itu
/// editor pertama dari [`KNOWN`] yang ditemukan di PATH atau lokasi install bawaannya.
pub fn find(configured: &str) -> Option<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default();
    dirs.extend(default_install_dirs());
    find_in(configured, &dirs)
}

fn find_in(configured: &str, dirs: &[PathBuf]) -> Option<PathBuf> {
    let configured = configured.trim().trim_matches('"');
    if !configured.is_empty() {
        let direct = Path::new(configured);
        if direct.is_absolute() {
            return direct.is_file().then(|| direct.to_path_buf());
        }
        return lookup(configured, dirs);
    }
    KNOWN.iter().find_map(|name| lookup(name, dirs))
}

/// `name` di salah satu `dirs`, dengan atau tanpa ekstensi `.exe/.cmd/.bat`.
fn lookup(name: &str, dirs: &[PathBuf]) -> Option<PathBuf> {
    let has_ext = Path::new(name)
        .extension()
        .is_some_and(|ext| EXTENSIONS.iter().any(|e| ext.eq_ignore_ascii_case(e)));
    dirs.iter().find_map(|dir| {
        if has_ext {
            let file = dir.join(name);
            return file.is_file().then_some(file);
        }
        EXTENSIONS
            .iter()
            .map(|ext| dir.join(format!("{name}.{ext}")))
            .find(|file| file.is_file())
    })
}

/// Folder `bin` VS Code / Cursor yang dipasang tanpa menambah PATH.
fn default_install_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(local) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) {
        dirs.push(local.join("Programs").join("Microsoft VS Code").join("bin"));
        dirs.push(
            local
                .join("Programs")
                .join("cursor")
                .join("resources")
                .join("app")
                .join("bin"),
        );
    }
    if let Some(program_files) = std::env::var_os("ProgramFiles").map(PathBuf::from) {
        dirs.push(program_files.join("Microsoft VS Code").join("bin"));
    }
    dirs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_configured_or_first_known_editor() {
        let tmp = tempfile::tempdir().unwrap();
        let (a, b) = (tmp.path().join("a"), tmp.path().join("b"));
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(&b).unwrap();
        std::fs::write(a.join("zed.exe"), "").unwrap();
        std::fs::write(b.join("code.cmd"), "").unwrap();
        std::fs::write(b.join("notepad++.exe"), "").unwrap();
        let dirs = [a.clone(), b.clone()];

        // `code` lebih diutamakan daripada `zed` walau foldernya belakangan di PATH.
        assert_eq!(find_in("", &dirs), Some(b.join("code.cmd")));
        assert_eq!(find_in("zed", &dirs), Some(a.join("zed.exe")));
        assert_eq!(
            find_in("notepad++.exe", &dirs),
            Some(b.join("notepad++.exe"))
        );
        let full = b.join("notepad++.exe");
        assert_eq!(find_in(&format!("\"{}\"", full.display()), &[]), Some(full));
        assert_eq!(find_in("vim", &dirs), None);
        assert_eq!(find_in("", &[a.join("kosong")]), None);
    }
}
