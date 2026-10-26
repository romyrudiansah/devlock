//! HTTP client sederhana (blocking) untuk katalog dan download runtime.

use std::io::{Read, Write};
use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result};
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

pub fn get_json<T: DeserializeOwned>(url: &str) -> Result<T> {
    let text = get_string(url)?;
    serde_json::from_str(&text).with_context(|| format!("respons {url} bukan JSON yang valid"))
}

/// Download ke `dest` (lewat file `.part`). `progress(downloaded, total)` dipanggil berkala.
pub fn download(url: &str, dest: &Path, progress: &mut dyn FnMut(u64, Option<u64>)) -> Result<()> {
    let response = agent()
        .get(url)
        .call()
        .with_context(|| format!("gagal download {url}"))?;
    let total = response
        .header("Content-Length")
        .and_then(|len| len.parse::<u64>().ok());

    let part = dest.with_extension("part");
    let mut file = std::fs::File::create(&part)
        .with_context(|| format!("gagal membuat {}", part.display()))?;
    let mut reader = response.into_reader();
    let mut buf = vec![0u8; 64 * 1024];
    let mut downloaded = 0u64;
    loop {
        let n = reader
            .read(&mut buf)
            .with_context(|| format!("koneksi terputus saat download {url}"))?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])?;
        downloaded += n as u64;
        progress(downloaded, total);
    }
    file.flush()?;
    drop(file);
    std::fs::rename(&part, dest).with_context(|| format!("gagal menyimpan {}", dest.display()))
}
