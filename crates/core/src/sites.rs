//! Deteksi project di folder `www\` menjadi site `<nama>.<tld>`.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::config::Config;
use crate::paths::Paths;
use crate::project::ProjectConfig;
use crate::resolve::{self, CacheInstance, DbInstance};
use crate::runtime::Kind;
use crate::version::Version;

#[derive(Debug, Clone, PartialEq)]
pub enum SiteKind {
    /// Diproses php-cgi versi `line` (major.minor).
    Php {
        line: String,
    },
    /// Reverse proxy ke dev server di `127.0.0.1:<port>`.
    Proxy {
        port: u16,
    },
    Static,
}

#[derive(Debug, Clone)]
pub struct Site {
    pub name: String,
    pub dir: PathBuf,
    pub docroot: PathBuf,
    /// Domain utama diikuti alias.
    pub domains: Vec<String>,
    pub kind: SiteKind,
    pub database: Option<DbInstance>,
    /// Instance cache (Redis/Garnet) dari `cache` di `.devlock.toml`.
    pub cache: Option<CacheInstance>,
    /// Perintah dev server (project Node).
    pub dev: Option<String>,
    /// Port dev server dideteksi dari output-nya (`dev` tanpa `proxy_port`).
    pub dev_port_auto: bool,
    /// Dilayani juga lewat HTTPS dengan sertifikat dari CA DevLock.
    pub https: bool,
    /// Request HTTP dialihkan (301) ke HTTPS; hanya berlaku bila `https`.
    pub https_redirect: bool,
    /// Dilayani Apache (di belakang nginx) karena `webserver = "apache"`.
    pub apache: bool,
    /// Masalah konfigurasi yang membuat site tidak bisa dilayani sepenuhnya.
    pub warning: Option<String>,
}

impl Site {
    pub fn domain(&self) -> &str {
        &self.domains[0]
    }

    /// URL utama site: `https://` bila HTTPS aktif, port ditulis bila bukan default.
    pub fn url(&self, config: &Config) -> String {
        url_for(self.domain(), self.https, config)
    }
}

/// URL untuk `domain` sesuai skema dan port DevLock.
pub fn url_for(domain: &str, https: bool, config: &Config) -> String {
    let (scheme, port, default) = if https {
        ("https", config.https.port, 443)
    } else {
        ("http", config.http.port, 80)
    };
    if port == default {
        format!("{scheme}://{domain}")
    } else {
        format!("{scheme}://{domain}:{port}")
    }
}

/// Semua site di folder www (ditambah site bawaan Adminer bila terinstall), urut nama.
pub fn discover(paths: &Paths, config: &Config) -> Result<Vec<Site>> {
    let www = config.www_dir(paths);
    let entries: Vec<std::fs::DirEntry> = match std::fs::read_dir(&www) {
        Ok(entries) => entries.filter_map(|e| e.ok()).collect(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(e) => return Err(e).with_context(|| format!("gagal membaca {}", www.display())),
    };
    let mut sites = Vec::new();
    for entry in entries {
        let dir = entry.path();
        let Some(name) = entry.file_name().to_str().map(str::to_string) else {
            continue;
        };
        if !dir.is_dir() || name.starts_with('.') || !is_valid_label(&name) {
            continue;
        }
        sites.push(load_site(paths, config, &name, &dir)?);
    }
    if !sites
        .iter()
        .any(|s| s.name.eq_ignore_ascii_case(ADMINER_SITE))
    {
        sites.extend(adminer_site(paths, config)?);
    }
    if !sites.iter().any(|s| s.name.eq_ignore_ascii_case(MAIL_SITE)) {
        sites.extend(mail_site(paths, config)?);
    }
    sites.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(sites)
}

/// Nama site bawaan Adminer (`adminer.<tld>`); folder `www\adminer` milik pengguna menang.
pub const ADMINER_SITE: &str = "adminer";

/// Site bawaan untuk Adminer terinstall: docroot di folder runtime-nya, PHP versi default.
/// `index.php` pembungkusnya ditulis oleh rencana service.
fn adminer_site(paths: &Paths, config: &Config) -> Result<Option<Site>> {
    let Some(version) = crate::runtime::resolve_installed(paths, Kind::Adminer, "latest")? else {
        return Ok(None);
    };
    let dir = paths.runtime_dir(Kind::Adminer.as_str(), &version);
    let mut warning = None;
    let kind = match resolve::resolve(paths, config, Kind::Php, &dir) {
        Ok(r) => SiteKind::Php {
            line: Version::parse(&r.version)
                .map(|v| v.major_minor())
                .unwrap_or(r.version),
        },
        Err(err) => {
            warning = Some(format!("Adminer butuh PHP: {err:#}"));
            SiteKind::Static
        }
    };
    Ok(Some(Site {
        name: ADMINER_SITE.into(),
        docroot: dir.clone(),
        dir,
        domains: vec![format!("{ADMINER_SITE}.{}", config.tld)],
        kind,
        database: None,
        cache: None,
        dev: None,
        dev_port_auto: false,
        https: config.https.enabled,
        https_redirect: config.https.enabled && config.https.redirect,
        apache: false,
        warning,
    }))
}

/// Nama site bawaan UI Mailpit (`mail.<tld>`); folder `www\mail` milik pengguna menang.
pub const MAIL_SITE: &str = "mail";

/// Site bawaan untuk Mailpit terinstall: reverse proxy ke UI web Mailpit.
fn mail_site(paths: &Paths, config: &Config) -> Result<Option<Site>> {
    let Some(version) = crate::runtime::resolve_installed(paths, Kind::Mailpit, "latest")? else {
        return Ok(None);
    };
    let dir = paths.runtime_dir(Kind::Mailpit.as_str(), &version);
    Ok(Some(Site {
        name: MAIL_SITE.into(),
        docroot: dir.clone(),
        dir,
        domains: vec![format!("{MAIL_SITE}.{}", config.tld)],
        kind: SiteKind::Proxy {
            port: config.mail.ui_port,
        },
        database: None,
        cache: None,
        dev: None,
        dev_port_auto: false,
        https: config.https.enabled,
        https_redirect: config.https.enabled && config.https.redirect,
        apache: false,
        warning: None,
    }))
}

impl Site {
    /// `true` untuk site bawaan Adminer (bukan folder di www).
    pub fn is_adminer(&self, paths: &Paths) -> bool {
        self.name == ADMINER_SITE && self.dir.starts_with(paths.runtimes().join("adminer"))
    }
}

fn load_site(paths: &Paths, config: &Config, name: &str, dir: &Path) -> Result<Site> {
    let project = ProjectConfig::load_dir(dir)?;
    let docroot = match &project.docroot {
        Some(rel) => dir.join(rel),
        None => detect_docroot(dir),
    };
    let https = project.https.unwrap_or(config.https.enabled);
    let mut domains = vec![format!("{}.{}", name.to_lowercase(), config.tld)];
    domains.extend(project.aliases.iter().cloned());

    let mut warning = None;
    // Project PHP (mis. Laravel + Vite): `dev` tanpa `proxy_port` hanya menjalankan dev server
    // di samping PHP; domain tetap dilayani PHP. Project lain di-proxy ke dev server.
    let php_project = is_php_project(dir, &docroot);
    let dev_port_auto = project.dev.is_some() && project.proxy_port.is_none() && !php_project;
    let kind = if let Some(port) = project.proxy_port {
        SiteKind::Proxy { port }
    } else if dev_port_auto {
        // Port terakhir yang terbaca dari output dev server, atau `PORT` yang diberikan DevLock.
        SiteKind::Proxy {
            port: crate::devport::read_detected(paths, name)
                .unwrap_or_else(|| crate::devport::assigned_port(name)),
        }
    } else if php_project {
        match resolve::resolve(paths, config, Kind::Php, dir) {
            Ok(r) => SiteKind::Php {
                line: Version::parse(&r.version)
                    .map(|v| v.major_minor())
                    .unwrap_or(r.version),
            },
            Err(err) => {
                warning = Some(format!("{err:#}"));
                SiteKind::Static
            }
        }
    } else {
        SiteKind::Static
    };

    // Dev server Node selalu lewat reverse proxy nginx.
    let webserver = project
        .webserver
        .as_deref()
        .unwrap_or(&config.webserver)
        .to_ascii_lowercase();
    let apache = match webserver.as_str() {
        "apache" | "httpd" => !matches!(kind, SiteKind::Proxy { .. }),
        "nginx" | "" => false,
        other => {
            warning.get_or_insert(format!(
                "webserver `{other}` tidak dikenal (pilihan: nginx, apache); memakai nginx"
            ));
            false
        }
    };

    let database = match &project.database {
        Some(db) => match DbInstance::parse(db) {
            Ok(instance) => Some(instance),
            Err(err) => {
                warning.get_or_insert(format!("{err:#}"));
                None
            }
        },
        None => None,
    };

    let cache = match &project.cache {
        Some(cache) => match CacheInstance::parse(cache) {
            Ok(instance) => Some(instance),
            Err(err) => {
                warning.get_or_insert(format!("{err:#}"));
                None
            }
        },
        None => None,
    };

    Ok(Site {
        name: name.to_string(),
        dir: dir.to_path_buf(),
        docroot,
        domains,
        kind,
        database,
        cache,
        dev: project.dev,
        dev_port_auto,
        https,
        https_redirect: https && project.https_redirect.unwrap_or(config.https.redirect),
        apache,
        warning,
    })
}

/// `public\` (Laravel), `web\` (Symfony/Craft), `public_html\`, `dist\` (hasil build
/// front-end), atau folder project.
pub fn detect_docroot(dir: &Path) -> PathBuf {
    ["public", "web", "public_html", "dist"]
        .iter()
        .map(|sub| dir.join(sub))
        .find(|p| p.is_dir())
        .unwrap_or_else(|| dir.to_path_buf())
}

fn is_php_project(dir: &Path, docroot: &Path) -> bool {
    dir.join("composer.json").is_file()
        || docroot.join("index.php").is_file()
        || std::fs::read_dir(docroot)
            .map(|entries| {
                entries
                    .filter_map(|e| e.ok())
                    .any(|e| e.path().extension().is_some_and(|ext| ext == "php"))
            })
            .unwrap_or(false)
}

/// Nama folder harus valid sebagai label DNS.
pub fn is_valid_label(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 63
        && !name.starts_with('-')
        && !name.ends_with('-')
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovers_site_kinds() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path());
        std::fs::create_dir_all(paths.runtime_dir("php", "8.3.12")).unwrap();
        let www = paths.default_www();

        std::fs::create_dir_all(www.join("blog/public")).unwrap();
        std::fs::write(www.join("blog/composer.json"), "{}").unwrap();
        std::fs::create_dir_all(www.join("landing")).unwrap();
        std::fs::write(www.join("landing/index.html"), "").unwrap();
        std::fs::create_dir_all(www.join("dash")).unwrap();
        std::fs::write(
            www.join("dash/.devlock.toml"),
            "node = \"22\"\ndev = \"pnpm dev\"\nproxy_port = 5173\naliases = [\"admin.test\"]\n",
        )
        .unwrap();
        std::fs::create_dir_all(www.join("old")).unwrap();
        std::fs::write(
            www.join("old/.devlock.toml"),
            "php = \"7.4\"\ndatabase = \"mysql@5.7\"\ncache = \"redis@8.10\"\n",
        )
        .unwrap();
        std::fs::write(www.join("old/index.php"), "").unwrap();
        std::fs::write(
            www.join("landing/.devlock.toml"),
            "webserver = \"apache\"\n",
        )
        .unwrap();
        std::fs::create_dir_all(www.join("bad name")).unwrap();

        let sites = discover(&paths, &Config::default()).unwrap();
        let names: Vec<_> = sites.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["blog", "dash", "landing", "old"]);

        assert_eq!(sites[0].kind, SiteKind::Php { line: "8.3".into() });
        assert_eq!(sites[0].docroot, www.join("blog/public"));
        assert_eq!(sites[1].kind, SiteKind::Proxy { port: 5173 });
        assert_eq!(sites[1].domains, ["dash.test", "admin.test"]);
        assert_eq!(sites[1].dev.as_deref(), Some("pnpm dev"));
        assert_eq!(sites[2].kind, SiteKind::Static);
        assert!(sites[2].apache);
        assert!(!sites[0].apache && !sites[1].apache);
        // PHP 7.4 belum terinstall: tetap terdaftar dengan peringatan.
        assert!(sites[3].warning.as_deref().unwrap().contains("php 7.4"));
        assert_eq!(sites[3].database.as_ref().unwrap().name(), "mysql@5.7");
        assert_eq!(sites[3].cache.as_ref().unwrap().name(), "redis@8");
    }
}
