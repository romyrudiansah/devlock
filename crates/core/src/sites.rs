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
    /// Folder grup di www yang memuat project ini (`www\<grup>\<nama>`).
    pub group: Option<String>,
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
    let mut sites: Vec<Site> = Vec::new();
    for ProjectDir { name, dir, group } in scan(&config.www_dir(paths))? {
        // Nama sama di grup lain: domainnya bentrok, jadi yang pertama (www langsung, lalu grup
        // urut abjad) yang dipakai.
        if let Some(first) = sites
            .iter_mut()
            .find(|s| s.name.eq_ignore_ascii_case(&name))
        {
            first.warning.get_or_insert(format!(
                "folder {} bernama sama diabaikan; ganti nama salah satunya",
                dir.display()
            ));
            continue;
        }
        let mut site = load_site(paths, config, &name, &dir)?;
        site.group = group;
        sites.push(site);
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

/// Folder project di www.
pub struct ProjectDir {
    pub name: String,
    pub dir: PathBuf,
    pub group: Option<String>,
}

/// Folder project di `www`: yang langsung di www lebih dulu, lalu isi tiap grup urut abjad.
/// Folder tanpa tanda project yang berisi project adalah grup; subfoldernya dibaca sebagai
/// project (satu tingkat), domainnya tetap `<nama>.<tld>`. Nama bisa berulang antar-grup.
pub fn scan(www: &Path) -> Result<Vec<ProjectDir>> {
    let entries: Vec<std::fs::DirEntry> = match std::fs::read_dir(www) {
        Ok(entries) => entries.filter_map(|e| e.ok()).collect(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(e) => return Err(e).with_context(|| format!("gagal membaca {}", www.display())),
    };
    let mut found = Vec::new();
    let mut groups = Vec::new();
    for entry in entries {
        let dir = entry.path();
        let Some(name) = entry.file_name().to_str().map(str::to_string) else {
            continue;
        };
        if !dir.is_dir() || name.starts_with('.') {
            continue;
        }
        let children = if looks_like_project(&dir) {
            Vec::new()
        } else {
            group_children(&dir)
        };
        if !children.is_empty() {
            groups.push((name, children));
        } else if is_valid_label(&name) {
            found.push(ProjectDir {
                name,
                dir,
                group: None,
            });
        }
    }
    groups.sort_by_key(|g| g.0.to_lowercase());
    for (group, children) in groups {
        found.extend(children.into_iter().map(|(name, dir)| ProjectDir {
            name,
            dir,
            group: Some(group.clone()),
        }));
    }
    Ok(found)
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
        group: None,
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
        group: None,
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
        group: None,
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

/// Ada tanda project di `dir`: konfigurasi/manifest, folder docroot, atau file halaman.
fn looks_like_project(dir: &Path) -> bool {
    [".devlock.toml", "composer.json", "package.json"]
        .iter()
        .any(|f| dir.join(f).is_file())
        || detect_docroot(dir) != dir
        || std::fs::read_dir(dir)
            .map(|entries| {
                entries.filter_map(|e| e.ok()).any(|e| {
                    e.path().is_file()
                        && e.path().extension().is_some_and(|ext| {
                            ["php", "html", "htm"]
                                .iter()
                                .any(|x| ext.eq_ignore_ascii_case(x))
                        })
                })
            })
            .unwrap_or(false)
}

/// Subfolder project di dalam calon grup (nama valid sebagai domain, punya tanda project).
fn group_children(dir: &Path) -> Vec<(String, PathBuf)> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut children: Vec<(String, PathBuf)> = entries
        .filter_map(|e| e.ok())
        .filter_map(|e| Some((e.file_name().to_str()?.to_string(), e.path())))
        .filter(|(name, path)| path.is_dir() && is_valid_label(name) && looks_like_project(path))
        .collect();
    children.sort_by(|a, b| a.0.cmp(&b.0));
    children
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

    #[test]
    fn discovers_projects_inside_group_folders() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path());
        let www = paths.default_www();
        for dir in [
            "solo",
            "Personal Project/blog",
            "Personal Project/notes",
            "Learn Stack/solo",
            "learn-go/kata",
        ] {
            std::fs::create_dir_all(www.join(dir)).unwrap();
            std::fs::write(www.join(dir).join("index.html"), "").unwrap();
        }
        // Folder grup boleh berisi file lain; project-nya tetap terbaca.
        std::fs::write(www.join("Personal Project/README.md"), "").unwrap();
        // Project yang punya subfolder tetap satu project, bukan grup.
        std::fs::create_dir_all(www.join("shop/public/assets")).unwrap();
        std::fs::write(www.join("shop/composer.json"), "{}").unwrap();
        std::fs::create_dir_all(www.join("shop/modules/cart")).unwrap();
        std::fs::write(www.join("shop/modules/cart/index.html"), "").unwrap();

        let sites = discover(&paths, &Config::default()).unwrap();
        let found: Vec<_> = sites
            .iter()
            .map(|s| (s.name.as_str(), s.group.as_deref()))
            .collect();
        assert_eq!(
            found,
            [
                ("blog", Some("Personal Project")),
                ("kata", Some("learn-go")),
                ("notes", Some("Personal Project")),
                ("shop", None),
                ("solo", None),
            ]
        );
        let blog = sites.iter().find(|s| s.name == "blog").unwrap();
        assert_eq!(blog.domains, ["blog.test"]);
        assert_eq!(blog.dir, www.join("Personal Project/blog"));
        // `Learn Stack/solo` bentrok dengan `www/solo`: yang langsung di www dipakai.
        let solo = sites.iter().find(|s| s.name == "solo").unwrap();
        assert_eq!(solo.dir, www.join("solo"));
        assert!(solo.warning.as_deref().unwrap().contains("Learn Stack"));
    }
}
