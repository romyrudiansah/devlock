//! HTTP client sederhana (blocking) untuk katalog dan download runtime.

use std::io::{Read, Write};
use std::path::Path;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use serde::de::DeserializeOwned;

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .user_agent(concat!("devlock/", env!("CARGO_PKG_VERSION")))
        .timeout_connect(Duration::from_secs(20))
        .timeout_read(Duration::from_secs(60))
        .build()
}

pub fn get_string(url: &str) -> Result<String> {
    agent()
        .get(url)
        .call()
        .with_context(|| format!("gagal mengambil {url}"))?
        .into_string()
        .with_context(|| format!("gagal membaca respons {url}"))
}

/// `true` bila URL bisa diunduh (HEAD berstatus 2xx setelah redirect).
pub fn exists(url: &str) -> bool {
    agent().head(url).call().is_ok()
}

pub fn get_json<T: DeserializeOwned>(url: &str) -> Result<T> {
    let text = get_string(url)?;
    serde_json::from_str(&text).with_context(|| format!("respons {url} bukan JSON yang valid"))
}

/// Download ke `dest` lewat file `.part`. Bila `.part` dari unduhan yang terputus masih ada,
/// unduhan dilanjutkan dengan header `Range` (server yang tidak mendukung → mulai ulang).
/// `progress(downloaded, total)` dipanggil berkala.
pub fn download(url: &str, dest: &Path, progress: &mut dyn FnMut(u64, Option<u64>)) -> Result<()> {
    let part = dest.with_extension("part");
    let existing = std::fs::metadata(&part).map(|m| m.len()).unwrap_or(0);

    let mut request = agent().get(url);
    if existing > 0 {
        request = request.set("Range", &format!("bytes={existing}-"));
    }
    let response = match request.call() {
        // 416: `.part` sudah lengkap (atau lebih besar dari file) → unduh ulang dari awal.
        Err(ureq::Error::Status(416, _)) => {
            let _ = std::fs::remove_file(&part);
            return download(url, dest, progress);
        }
        other => other.with_context(|| format!("gagal download {url}"))?,
    };
    let resumed = existing > 0 && response.status() == 206;
    if resumed {
        tracing::info!("melanjutkan unduhan {url} dari byte {existing}");
    } else if existing > 0 {
        tracing::info!("server tidak mendukung resume; mengunduh ulang {url}");
    }
    let length = response
        .header("Content-Length")
        .and_then(|len| len.parse::<u64>().ok());
    let mut downloaded = if resumed { existing } else { 0 };
    let total = length.map(|len| len + downloaded);

    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .append(resumed)
        .truncate(!resumed)
        .open(&part)
        .with_context(|| format!("gagal membuat {}", part.display()))?;
    let mut reader = response.into_reader();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = reader.read(&mut buf).with_context(|| {
            format!(
                "koneksi terputus saat download {url}; ulangi perintah yang sama untuk melanjutkan"
            )
        })?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])?;
        downloaded += n as u64;
        progress(downloaded, total);
    }
    file.flush()?;
    drop(file);
    if let Some(total) = total {
        if downloaded != total {
            bail!("unduhan {url} tidak lengkap ({downloaded} dari {total} byte); ulangi untuk melanjutkan");
        }
    }
    std::fs::rename(&part, dest).with_context(|| format!("gagal menyimpan {}", dest.display()))
}
