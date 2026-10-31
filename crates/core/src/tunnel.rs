//! Membagikan site ke publik lewat Cloudflare Quick Tunnel (`cloudflared tunnel --url`):
//! tanpa akun, URL acak `https://<nama>.trycloudflare.com`, aktif selama prosesnya berjalan.

use std::path::Path;

use anyhow::{bail, Result};

use crate::config::Config;
use crate::paths::Paths;
use crate::services::Command;
use crate::sites::Site;

/// Perintah `cloudflared` yang meneruskan tunnel ke nginx DevLock dengan header `Host` site.
/// Site yang mengalihkan HTTP ke HTTPS memakai origin HTTPS yang diverifikasi dengan CA DevLock.
pub fn command(paths: &Paths, config: &Config, site: &Site, cloudflared: &Path) -> Result<Command> {
    let domain = site.domain().to_string();
    let host = match config.http.listen.as_str() {
        "0.0.0.0" | "" => "127.0.0.1",
        listen => listen,
    };
    let mut args = vec![
        "tunnel".to_string(),
        "--no-autoupdate".into(),
        "--http-host-header".into(),
        domain.clone(),
    ];
    if site.https_redirect {
        let ca = crate::ssl::ca_cert_path(paths);
        if !ca.is_file() {
            bail!(
                "CA DevLock belum dibuat; jalankan `devlock ssl trust` atau matikan https_redirect"
            );
        }
        args.extend([
            "--url".into(),
            format!("https://{host}:{}", config.https.port),
            "--origin-server-name".into(),
            domain,
            "--origin-ca-pool".into(),
            ca.to_string_lossy().into_owned(),
        ]);
    } else {
        args.extend([
            "--url".into(),
            format!("http://{host}:{}", config.http.port),
        ]);
    }
    Ok(Command {
        program: cloudflared.to_path_buf(),
        args,
        cwd: paths.root().to_path_buf(),
        env: vec![],
    })
}

/// URL publik `https://….trycloudflare.com` dari satu baris log cloudflared.
pub fn public_url(line: &str) -> Option<String> {
    let start = line.find("https://")?;
    let url: String = line[start..]
        .chars()
        .take_while(|c| !c.is_whitespace() && *c != '|')
        .collect();
    let host = url.trim_start_matches("https://");
    (host.ends_with(".trycloudflare.com") && host != "api.trycloudflare.com").then_some(url)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sites::SiteKind;

    fn site(redirect: bool) -> Site {
        Site {
            name: "blog".into(),
            dir: "D:/www/blog".into(),
            group: None,
            docroot: "D:/www/blog".into(),
            domains: vec!["blog.test".into()],
            kind: SiteKind::Static,
            database: None,
            cache: None,
            dev: None,
            dev_port_auto: false,
            https: redirect,
            https_redirect: redirect,
            apache: false,
            warning: None,
        }
    }

    #[test]
    fn tunnels_to_nginx_with_site_host() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path());
        let exe = tmp.path().join("cloudflared.exe");
        let config = Config::default();
        let cmd = command(&paths, &config, &site(false), &exe).unwrap();
        assert_eq!(
            cmd.args,
            [
                "tunnel",
                "--no-autoupdate",
                "--http-host-header",
                "blog.test",
                "--url",
                "http://127.0.0.1:80"
            ]
        );

        // HTTPS-only: tanpa CA ditolak, dengan CA memakai origin TLS yang diverifikasi.
        assert!(command(&paths, &config, &site(true), &exe).is_err());
        crate::ssl::ensure_ca(&paths).unwrap();
        let cmd = command(&paths, &config, &site(true), &exe).unwrap();
        let args = cmd.args.join(" ");
        assert!(args.contains(
            "--url https://127.0.0.1:443 --origin-server-name blog.test --origin-ca-pool "
        ));
    }

    #[test]
    fn reads_public_url_from_log() {
        let line = "2026-10-28T03:00:00Z INF |  https://silly-cat-quiet-river.trycloudflare.com                                 |";
        assert_eq!(
            public_url(line).as_deref(),
            Some("https://silly-cat-quiet-river.trycloudflare.com")
        );
        assert_eq!(
            public_url("INF Requesting new quick Tunnel on trycloudflare.com..."),
            None
        );
        assert_eq!(
            public_url("failed to reach https://api.trycloudflare.com/tunnel"),
            None
        );
    }
}
