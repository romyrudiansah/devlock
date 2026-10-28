use std::path::PathBuf;

use anyhow::{bail, Result};
use devlock_core::{import, Config, Paths};

pub fn run(source: Option<PathBuf>, copy: bool, dry_run: bool) -> Result<()> {
    let paths = Paths::discover()?;
    let config = Config::load(&paths)?;
    let www = config.www_dir(&paths);
    let sources: Vec<(String, PathBuf)> = match source {
        Some(dir) => vec![("folder".into(), dir)],
        None => {
            let found = import::detect_sources();
            if found.is_empty() {
                bail!("Laragon (C:\\laragon\\www) atau XAMPP (C:\\xampp\\htdocs) tidak ditemukan; sebutkan foldernya: `devlock import <folder>`");
            }
            found.into_iter().map(|(l, d)| (l.to_string(), d)).collect()
        }
    };

    let mut imported = 0;
    for (label, dir) in sources {
        println!("{label}: {}", dir.display());
        for candidate in import::candidates(&dir, &www)? {
            if let Some(reason) = &candidate.skip {
                println!("  - {:<20} dilewati ({reason})", candidate.name);
                continue;
            }
            if dry_run {
                println!(
                    "  + {:<20} ← {}",
                    candidate.name,
                    candidate.source.display()
                );
                continue;
            }
            let dest = import::import(&candidate, &www, copy)?;
            println!(
                "  + {:<20} {} {}",
                candidate.name,
                if copy { "disalin ke" } else { "junction" },
                dest.display()
            );
            imported += 1;
        }
    }
    if dry_run {
        println!("\nTidak ada yang diubah (--dry-run).");
    } else if imported > 0 {
        println!(
            "\n{imported} project diimpor sebagai <nama>.{}.",
            config.tld
        );
        if !copy {
            println!("Junction menunjuk ke folder asal: menghapus folder di www tidak menghapus project aslinya.");
        }
        println!("Atur versi PHP/database per project dengan `devlock use` di folder project, lalu `devlock reload`.");
    } else {
        println!("\nTidak ada project baru untuk diimpor.");
    }
    Ok(())
}
