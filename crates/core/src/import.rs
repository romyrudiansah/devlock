//! Impor project dari Laragon (`www`) atau XAMPP (`htdocs`) ke folder www DevLock, sebagai
//! directory junction (file tetap di tempat asal) atau salinan.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

use crate::sites::is_valid_label;

/// Folder bawaan XAMPP di `htdocs` yang bukan project.
const XAMPP_BUILTIN: &[&str] = &[
    "dashboard",
    "img",
    "webalizer",
    "xampp",
    "forbidden",
    "restricted",
];

/// Lokasi umum folder project Laragon dan XAMPP yang ada di komputer ini.
pub fn detect_sources() -> Vec<(&'static str, PathBuf)> {
    ["C:", "D:"]
        .iter()
        .flat_map(|drive| {
            [
                ("Laragon", PathBuf::from(format!(r"{drive}\laragon\www"))),
                ("XAMPP", PathBuf::from(format!(r"{drive}\xampp\htdocs"))),
            ]
        })
        .filter(|(_, dir)| dir.is_dir())
        .collect()
}

#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    /// Nama site di DevLock (label DNS valid).
    pub name: String,
    pub source: PathBuf,
    /// Alasan dilewati; `None` bila akan diimpor.
    pub skip: Option<String>,
}

/// Nama site dari nama folder: huruf kecil, `_`/`.`/spasi → `-`; `None` bila tetap tidak valid.
pub fn site_name(folder: &str) -> Option<String> {
    let name: String = folder
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| if matches!(c, '_' | '.' | ' ') { '-' } else { c })
        .collect();
    let name = name.trim_matches('-').to_string();
    is_valid_label(&name).then_some(name)
}

/// Subfolder `source` yang bisa diimpor ke `www`, urut nama.
pub fn candidates(source: &Path, www: &Path) -> Result<Vec<Candidate>> {
    let entries =
        std::fs::read_dir(source).with_context(|| format!("gagal membaca {}", source.display()))?;
    // Project di dalam grup juga memakai `<nama>.test`.
    let grouped: Vec<crate::sites::ProjectDir> = crate::sites::scan(www)?
        .into_iter()
        .filter(|p| p.group.is_some())
        .collect();
    let mut out: Vec<Candidate> = Vec::new();
    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        let Some(folder) = entry.file_name().to_str().map(str::to_string) else {
            continue;
        };
        if !path.is_dir() || folder.starts_with('.') {
            continue;
        }
        let lower = folder.to_lowercase();
        let (name, skip) = match site_name(&folder) {
            _ if XAMPP_BUILTIN.contains(&lower.as_str()) => {
                (lower, Some("folder bawaan XAMPP".into()))
            }
            None => (
                folder.clone(),
                Some("nama folder tidak bisa dijadikan domain".into()),
            ),
            Some(name) if www.join(&name).exists() => {
                let reason = format!("sudah ada di www\\{name}");
                (name, Some(reason))
            }
            Some(name) if grouped.iter().any(|p| p.name.eq_ignore_ascii_case(&name)) => {
                let reason = "nama sudah dipakai project di grup www".to_string();
                (name, Some(reason))
            }
            Some(name) if out.iter().any(|c| c.name == name && c.skip.is_none()) => {
                (name, Some("nama site bentrok dengan folder lain".into()))
            }
            Some(name) => (name, None),
        };
        out.push(Candidate {
            name,
            source: path,
            skip,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

/// Impor satu project: junction `www\<name>` → sumber, atau salinan bila `copy`.
pub fn import(candidate: &Candidate, www: &Path, copy: bool) -> Result<PathBuf> {
    if let Some(reason) = &candidate.skip {
        bail!("{} dilewati: {reason}", candidate.name);
    }
    std::fs::create_dir_all(www)?;
    let dest = www.join(&candidate.name);
    if copy {
        copy_dir(&candidate.source, &dest)?;
    } else {
        junction(&candidate.source, &dest)?;
    }
    Ok(dest)
}

/// Directory junction (`mklink /J`): tidak butuh hak admin dan, berbeda dengan symlink,
/// bekerja tanpa Developer Mode.
fn junction(target: &Path, link: &Path) -> Result<()> {
    let mut command = std::process::Command::new("cmd.exe");
    command
        .args(["/d", "/c", "mklink", "/J"])
        .arg(link)
        .arg(target);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(crate::process::CREATE_NO_WINDOW);
    }
    let output = command.output().context("gagal menjalankan mklink")?;
    if !output.status.success() || !link.is_dir() {
        bail!(
            "gagal membuat junction {} → {}: {}",
            link.display(),
            target.display(),
            String::from_utf8_lossy(&output.stdout).trim()
        );
    }
    Ok(())
}

fn copy_dir(from: &Path, to: &Path) -> Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let dest = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &dest)?;
        } else {
            std::fs::copy(entry.path(), &dest)
                .with_context(|| format!("gagal menyalin {}", entry.path().display()))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn site_names_are_dns_safe() {
        assert_eq!(site_name("My_Shop").as_deref(), Some("my-shop"));
        assert_eq!(site_name("blog.v2").as_deref(), Some("blog-v2"));
        assert_eq!(site_name("_tmp_").as_deref(), Some("tmp"));
        assert_eq!(site_name("toko online").as_deref(), Some("toko-online"));
        assert_eq!(site_name("café"), None);
    }

    #[test]
    fn lists_candidates_and_imports() {
        let tmp = tempfile::tempdir().unwrap();
        let htdocs = tmp.path().join("htdocs");
        let www = tmp.path().join("www");
        for dir in [
            "Blog",
            "dashboard",
            "shop_app",
            "shop-app",
            "café",
            "lama",
            "kasir",
        ] {
            std::fs::create_dir_all(htdocs.join(dir)).unwrap();
        }
        std::fs::write(htdocs.join("Blog/index.php"), "<?php echo 1;").unwrap();
        std::fs::write(htdocs.join("index.php"), "").unwrap();
        std::fs::create_dir_all(www.join("lama")).unwrap();
        std::fs::create_dir_all(www.join("Klien A/kasir")).unwrap();
        std::fs::write(www.join("Klien A/kasir/index.html"), "").unwrap();

        let list = candidates(&htdocs, &www).unwrap();
        let view: Vec<_> = list
            .iter()
            .map(|c| (c.name.as_str(), c.skip.is_none()))
            .collect();
        assert!(view.contains(&("blog", true)));
        assert!(view.contains(&("dashboard", false)));
        assert!(view.contains(&("lama", false)));
        assert!(view.contains(&("kasir", false)));
        assert!(view.contains(&("café", false)));
        // `shop_app` dan `shop-app` sama-sama menjadi `shop-app`: hanya satu yang diimpor.
        assert_eq!(
            view.iter()
                .filter(|(n, ok)| *n == "shop-app" && *ok)
                .count(),
            1
        );

        let blog = list.iter().find(|c| c.name == "blog").unwrap();
        let dest = import(blog, &www, true).unwrap();
        assert_eq!(
            std::fs::read_to_string(dest.join("index.php")).unwrap(),
            "<?php echo 1;"
        );
        let lama = list.iter().find(|c| c.name == "lama").unwrap();
        assert!(import(lama, &www, true).is_err());
    }

    #[cfg(windows)]
    #[test]
    fn junction_points_to_source() {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("laragon www").join("api");
        std::fs::create_dir_all(&source).unwrap();
        std::fs::write(source.join("index.php"), "x").unwrap();
        let candidate = Candidate {
            name: "api".into(),
            source: source.clone(),
            skip: None,
        };
        let www = tmp.path().join("devlock www");
        let dest = import(&candidate, &www, false).unwrap();
        assert_eq!(
            std::fs::read_to_string(dest.join("index.php")).unwrap(),
            "x"
        );
        std::fs::write(source.join("baru.txt"), "y").unwrap();
        assert!(dest.join("baru.txt").is_file());
    }
}
