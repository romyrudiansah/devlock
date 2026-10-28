//! Rencana service (nginx, php-cgi, database, dev server) dari konfigurasi + site,
//! termasuk file konfigurasi yang harus ditulis sebelum service dijalankan.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::paths::Paths;
use crate::resolve::{self, CacheInstance, DbInstance};
use crate::runtime::{self, Kind};
use crate::sites::{self, Site, SiteKind};
use crate::ssl;
use crate::version::Version;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Command {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub env: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceSpec {
    /// Nama unik, mis. `nginx`, `php@8.3#0`, `mysql@8.4`, `dev:dashboard`.
    pub name: String,
    /// Kelompok untuk start/stop bersama, mis. `php@8.3`.
    pub group: String,
    pub command: Command,
    /// Perintah berhenti dengan rapi (nginx -s quit, mysqladmin shutdown); lalu kill bila lewat batas.
    pub stop: Option<Command>,
    /// Dijalankan sekali sebelum start bila `init_marker` belum ada (inisialisasi data dir).
    pub init: Option<Command>,
    pub init_marker: Option<PathBuf>,
    pub port: Option<u16>,
    /// Port lain yang juga dipakai (mis. 443 untuk nginx saat HTTPS aktif); ikut dicek bentrok.
    #[serde(default)]
    pub extra_ports: Vec<u16>,
    pub log: PathBuf,
}

/// Port php-cgi: `9000 + major*100 + minor*10 + worker` (8.3 → 9830..9839).
pub fn php_port(line: &str, worker: u16) -> u16 {
    let v = Version::parse(line).unwrap_or_else(|| Version::parse("0").unwrap());
    9000 + v.major() as u16 * 100 + v.minor() as u16 * 10 + worker
}

pub struct Plan {
    pub sites: Vec<Site>,
    pub services: Vec<ServiceSpec>,
    /// File konfigurasi yang ditulis sebelum service berjalan.
    pub files: Vec<(PathBuf, String)>,
    pub warnings: Vec<String>,
}

pub fn plan(paths: &Paths, config: &Config) -> Result<Plan> {
    let mut sites = sites::discover(paths, config)?;
    let mut services = Vec::new();
    let mut files = Vec::new();
    let mut warnings: Vec<String> = sites
        .iter()
        .filter_map(|s| s.warning.as_ref().map(|w| format!("{}: {w}", s.domain())))
        .collect();

    // PHP: satu pool per versi yang dipakai site.
    let php_lines: BTreeSet<String> = sites
        .iter()
        .filter_map(|s| match &s.kind {
            SiteKind::Php { line } => Some(line.clone()),
            _ => None,
        })
        .collect();
    let workers = config.php.workers.clamp(1, 10);
    for line in &php_lines {
        let (_, dir) = runtime::require_installed(paths, Kind::Php, line)?;
        for worker in 0..workers {
            let port = php_port(line, worker);
            services.push(ServiceSpec {
                name: format!("php@{line}#{worker}"),
                group: format!("php@{line}"),
                command: Command {
                    program: dir.join("php-cgi.exe"),
                    args: vec![
                        "-b".into(),
                        format!("127.0.0.1:{port}"),
                        "-c".into(),
                        dir.join("php.ini").to_string_lossy().into(),
                    ],
                    cwd: dir.clone(),
                    env: vec![
                        (
                            "PHP_FCGI_MAX_REQUESTS".into(),
                            config.php.max_requests.to_string(),
                        ),
                        ("PHP_FCGI_CHILDREN".into(), "0".into()),
                    ],
                },
                stop: None,
                init: None,
                init_marker: None,
                port: Some(port),
                extra_ports: vec![],
                log: paths.logs().join(format!("php@{line}.log")),
            });
        }
    }

    // Apache (di belakang nginx) untuk site `webserver = "apache"`.
    if sites.iter().any(|s| s.apache) {
        let apache_spec = config
            .defaults
            .get("apache")
            .map_or("latest", String::as_str);
        match runtime::resolve_installed(paths, Kind::Apache, apache_spec)? {
            Some(version) => {
                let dir = paths.runtime_dir("apache", &version);
                let conf = paths.etc().join("apache").join("httpd.conf");
                std::fs::create_dir_all(paths.tmp().join("apache"))?;
                files.push((
                    conf.clone(),
                    apache_conf(paths, config, &dir, &sites, workers),
                ));
                services.push(ServiceSpec {
                    name: "apache".into(),
                    group: "apache".into(),
                    command: Command {
                        program: dir.join("bin").join("httpd.exe"),
                        args: vec!["-d".into(), slash(&dir), "-f".into(), slash(&conf)],
                        cwd: dir.clone(),
                        env: vec![],
                    },
                    stop: None,
                    init: None,
                    init_marker: None,
                    port: Some(config.apache.port),
                    extra_ports: vec![],
                    log: paths.logs().join("apache.log"),
                });
            }
            None => {
                let names: Vec<&str> = sites
                    .iter()
                    .filter(|s| s.apache)
                    .map(|s| s.domain())
                    .collect();
                warnings.push(format!(
                    "Apache belum terinstall; {} sementara dilayani nginx. Jalankan `devlock install apache`",
                    names.join(", ")
                ));
                for site in &mut sites {
                    site.apache = false;
                }
            }
        }
    }

    // Web server
    let nginx_spec = config
        .defaults
        .get("nginx")
        .map_or("latest", String::as_str);
    match runtime::resolve_installed(paths, Kind::Nginx, nginx_spec)? {
        Some(version) => {
            let dir = paths.runtime_dir("nginx", &version);
            let conf = paths.etc().join("nginx").join("nginx.conf");
            for sub in ["client_body", "proxy", "fastcgi", "uwsgi", "scgi"] {
                std::fs::create_dir_all(paths.tmp().join("nginx").join(sub))?;
            }
            // Sertifikat untuk site HTTPS (dibuat/diperbarui seperlunya).
            let mut certs = BTreeMap::new();
            for site in sites.iter().filter(|s| s.https) {
                match ssl::site_cert(paths, &site.name, &site.domains) {
                    Ok(cert) => {
                        certs.insert(site.name.clone(), cert);
                    }
                    Err(err) => warnings.push(format!(
                        "{}: sertifikat HTTPS gagal dibuat: {err:#}",
                        site.domain()
                    )),
                }
            }
            files.push((
                conf.clone(),
                nginx_conf(paths, config, &dir, &sites, workers, &certs),
            ));
            let base_args = vec![
                "-p".to_string(),
                format!("{}/", slash(&dir)),
                "-c".to_string(),
                slash(&conf),
            ];
            let program = dir.join("nginx.exe");
            let mut stop_args = base_args.clone();
            stop_args.extend(["-s".to_string(), "quit".to_string()]);
            services.push(ServiceSpec {
                name: "nginx".into(),
                group: "nginx".into(),
                command: Command {
                    program: program.clone(),
                    args: base_args,
                    cwd: dir.clone(),
                    env: vec![],
                },
                stop: Some(Command {
                    program,
                    args: stop_args,
                    cwd: dir,
                    env: vec![],
                }),
                init: None,
                init_marker: None,
                port: Some(config.http.port),
                extra_ports: if certs.is_empty() {
                    vec![]
                } else {
                    vec![config.https.port]
                },
                log: paths.logs().join("nginx.log"),
            });
        }
        None if !sites.is_empty() => {
            warnings.push("nginx belum terinstall; jalankan `devlock install nginx`".into())
        }
        None => {}
    }

    // Database: instance yang dipakai site + default global.
    let mut instances: Vec<DbInstance> = sites.iter().filter_map(|s| s.database.clone()).collect();
    if let Some(db) = config.defaults.get("database") {
        instances.push(DbInstance::parse(db)?);
    }
    instances.sort_by_key(DbInstance::name);
    instances.dedup();
    if let Some(adminer) = sites.iter().find(|s| s.is_adminer(paths)) {
        files.push((
            adminer.dir.join("index.php"),
            adminer_index(
                config,
                &instances
                    .iter()
                    .filter(|i| i.is_sql())
                    .cloned()
                    .collect::<Vec<_>>(),
            ),
        ));
    }
    for instance in instances {
        match runtime::resolve_installed(paths, instance.kind, &instance.line)? {
            Some(version) => {
                let dir = paths.runtime_dir(instance.kind.as_str(), &version);
                if instance.kind == Kind::Mongodb {
                    // mongod tidak membuat folder dbpath sendiri.
                    std::fs::create_dir_all(instance.data_dir(paths))?;
                }
                let (spec, config_file) = database_service(paths, config, &instance, &dir);
                files.extend(config_file);
                services.push(spec);
            }
            None => warnings.push(format!(
                "{} belum terinstall; jalankan `devlock install {} {}`",
                instance.name(),
                instance.kind,
                instance.line
            )),
        }
    }

    // Cache (Redis/Garnet): instance yang dipakai site + default global.
    let mut caches: Vec<CacheInstance> = sites.iter().filter_map(|s| s.cache.clone()).collect();
    if let Some(cache) = config.defaults.get("cache") {
        caches.push(CacheInstance::parse(cache)?);
    }
    caches.sort_by_key(CacheInstance::name);
    caches.dedup();
    for instance in caches {
        match runtime::resolve_installed(paths, instance.kind, &instance.line)? {
            Some(version) => {
                let dir = paths.runtime_dir(instance.kind.as_str(), &version);
                std::fs::create_dir_all(instance.data_dir(paths))?;
                let (spec, config_file) = cache_service(paths, config, &instance, &dir);
                files.extend(config_file);
                services.push(spec);
            }
            None => warnings.push(format!(
                "{} belum terinstall; jalankan `devlock install {} {}`",
                instance.name(),
                instance.kind,
                instance.line
            )),
        }
    }

    // Mailpit: SMTP penangkap email untuk semua project, UI di mail.<tld>.
    if let Some(version) = runtime::resolve_installed(paths, Kind::Mailpit, "latest")? {
        let dir = paths.runtime_dir(Kind::Mailpit.as_str(), &version);
        services.push(mailpit_service(paths, config, &dir)?);
    }

    // DNS wildcard `*.<tld>`: `devlock dns serve` sebagai service biasa (port 53 juga TCP,
    // jadi cek kesiapan TCP berlaku).
    if config.dns.enabled {
        match crate::launcher::devlock_exe() {
            Ok(exe) => services.push(ServiceSpec {
                name: "dns".into(),
                group: "dns".into(),
                command: Command {
                    program: exe,
                    args: vec!["dns".into(), "serve".into()],
                    cwd: paths.root().to_path_buf(),
                    env: vec![(
                        crate::paths::HOME_ENV.into(),
                        paths.root().to_string_lossy().into(),
                    )],
                },
                stop: None,
                init: None,
                init_marker: None,
                port: Some(crate::dns::PORT),
                extra_ports: vec![],
                log: paths.logs().join("dns.log"),
            }),
            Err(err) => warnings.push(format!("DNS wildcard tidak bisa dijalankan: {err:#}")),
        }
    }

    // Dev server: dijalankan lewat shim agar versi Node sesuai project. Site proxy diarahkan ke
    // port-nya; di project PHP (Laravel + Vite) dev server hanya berjalan di samping PHP.
    for site in &sites {
        let Some(dev) = &site.dev else {
            continue;
        };
        let path = std::env::join_paths(
            std::iter::once(paths.shims()).chain(
                std::env::var_os("PATH")
                    .iter()
                    .flat_map(std::env::split_paths),
            ),
        )
        .context("PATH tidak valid")?;
        // Port otomatis: `PORT` tetap per site agar spec tidak berubah saat port sebenarnya
        // terdeteksi (nginx cukup di-reload); kesiapan tidak dicek lewat port.
        let (env_port, ready_port) = match &site.kind {
            SiteKind::Proxy { .. } if site.dev_port_auto => {
                (Some(crate::devport::assigned_port(&site.name)), None)
            }
            SiteKind::Proxy { port } => (Some(*port), Some(*port)),
            // Vite milik Laravel memilih port sendiri; `PORT` bisa bentrok dengan aplikasi lain.
            _ => (None, None),
        };
        services.push(ServiceSpec {
            name: format!("dev:{}", site.name),
            group: format!("dev:{}", site.name),
            command: Command {
                program: PathBuf::from(
                    std::env::var("ComSpec").unwrap_or_else(|_| "cmd.exe".into()),
                ),
                args: vec!["/d".into(), "/s".into(), "/c".into(), dev.clone()],
                cwd: site.dir.clone(),
                env: env_port
                    .map(|port| ("PORT".to_string(), port.to_string()))
                    .into_iter()
                    .chain([
                        ("PATH".into(), path.to_string_lossy().into()),
                        ("BROWSER".into(), "none".into()),
                    ])
                    .chain(node_ca_env(paths))
                    .collect(),
            },
            stop: None,
            init: None,
            init_marker: None,
            port: ready_port,
            extra_ports: vec![],
            log: paths.logs().join(format!("dev-{}.log", site.name)),
        });
    }

    Ok(Plan {
        sites,
        services,
        files,
        warnings,
    })
}

/// Service database beserta file konfigurasi yang perlu ditulis (my.ini untuk MySQL/MariaDB).
fn database_service(
    paths: &Paths,
    config: &Config,
    instance: &DbInstance,
    dir: &Path,
) -> (ServiceSpec, Option<(PathBuf, String)>) {
    if instance.kind == Kind::Postgresql {
        return (postgresql_service(paths, config, instance, dir), None);
    }
    if instance.kind == Kind::Mongodb {
        return (mongodb_service(paths, config, instance, dir), None);
    }
    let port = instance.port(config);
    let data = instance.data_dir(paths);
    let ini_path = paths
        .etc()
        .join(instance.kind.as_str())
        .join(&instance.line)
        .join("my.ini");
    let ini = format!(
        "[mysqld]\nbasedir=\"{}\"\ndatadir=\"{}\"\nport={port}\nbind-address=127.0.0.1\n\
         character-set-server=utf8mb4\ncollation-server=utf8mb4_unicode_ci\n\
         max_allowed_packet=256M\n\n[client]\nport={port}\nhost=127.0.0.1\n",
        slash(dir),
        slash(&data),
    );
    let defaults = format!("--defaults-file={}", slash(&ini_path));
    let bin = dir.join("bin");
    let exe = |names: &[&str]| {
        names
            .iter()
            .map(|n| bin.join(n))
            .find(|p| p.is_file())
            .unwrap_or_else(|| bin.join(names[0]))
    };

    let server = exe(&["mariadbd.exe", "mysqld.exe"]);
    let init = match instance.kind {
        Kind::Mariadb => Command {
            program: exe(&["mariadb-install-db.exe", "mysql_install_db.exe"]),
            args: vec![
                format!("--datadir={}", slash(&data)),
                format!("--port={port}"),
            ],
            cwd: dir.to_path_buf(),
            env: vec![],
        },
        _ => Command {
            program: server.clone(),
            args: vec![
                defaults.clone(),
                "--initialize-insecure".into(),
                "--console".into(),
            ],
            cwd: dir.to_path_buf(),
            env: vec![],
        },
    };
    let spec = ServiceSpec {
        name: instance.name(),
        group: instance.name(),
        command: Command {
            program: server,
            args: vec![defaults, "--console".into()],
            cwd: dir.to_path_buf(),
            env: vec![],
        },
        stop: Some(Command {
            program: exe(&["mariadb-admin.exe", "mysqladmin.exe"]),
            args: vec![
                "-h".into(),
                "127.0.0.1".into(),
                "-P".into(),
                port.to_string(),
                "-u".into(),
                "root".into(),
                "shutdown".into(),
            ],
            cwd: dir.to_path_buf(),
            env: vec![],
        }),
        init: Some(init),
        // Folder `mysql` (tabel sistem) ada setelah data dir terinisialisasi.
        init_marker: Some(data.join("mysql")),
        port: Some(port),
        extra_ports: vec![],
        log: paths.logs().join(format!("{}.log", instance.name())),
    };
    (spec, Some((ini_path, ini)))
}

/// PostgreSQL: `initdb` sekali (superuser `postgres`, auth trust, hanya 127.0.0.1),
/// lalu `postgres.exe`; berhenti rapi lewat `pg_ctl stop -m fast`.
/// `mailpit` di `127.0.0.1`: SMTP `[mail] smtp_port`, UI `[mail] ui_port`, pesan disimpan di
/// `data\mailpit\mailpit.db` agar tetap ada setelah restart.
fn mailpit_service(paths: &Paths, config: &Config, dir: &Path) -> Result<ServiceSpec> {
    let data = paths.data().join("mailpit");
    std::fs::create_dir_all(&data)?;
    Ok(ServiceSpec {
        name: "mailpit".into(),
        group: "mailpit".into(),
        command: Command {
            program: dir.join("mailpit.exe"),
            args: vec![
                "--smtp".into(),
                format!("127.0.0.1:{}", config.mail.smtp_port),
                "--listen".into(),
                format!("127.0.0.1:{}", config.mail.ui_port),
                "--database".into(),
                data.join("mailpit.db").to_string_lossy().into_owned(),
            ],
            cwd: dir.to_path_buf(),
            env: vec![],
        },
        stop: None,
        init: None,
        init_marker: None,
        port: Some(config.mail.ui_port),
        extra_ports: vec![config.mail.smtp_port],
        log: paths.logs().join("mailpit.log"),
    })
}

/// `mongod` per instance, data di `data\mongodb\<major.minor>`. Berhenti lewat perintah
/// `shutdown` dari `mongosh` bila terinstall; tanpa itu proses dihentikan (journal memulihkan).
fn mongodb_service(
    paths: &Paths,
    config: &Config,
    instance: &DbInstance,
    dir: &Path,
) -> ServiceSpec {
    let port = instance.port(config);
    let stop = runtime::resolve_installed(paths, Kind::Mongosh, "latest")
        .ok()
        .flatten()
        .map(|version| {
            let mongosh = paths.runtime_dir(Kind::Mongosh.as_str(), &version);
            Command {
                program: mongosh.join("bin").join("mongosh.exe"),
                args: vec![
                    "--quiet".into(),
                    format!("mongodb://127.0.0.1:{port}/admin"),
                    "--eval".into(),
                    "db.adminCommand({ shutdown: 1 })".into(),
                ],
                cwd: mongosh,
                env: vec![],
            }
        });
    ServiceSpec {
        name: instance.name(),
        group: instance.name(),
        command: Command {
            program: dir.join("bin").join("mongod.exe"),
            args: vec![
                "--dbpath".into(),
                instance.data_dir(paths).to_string_lossy().into_owned(),
                "--bind_ip".into(),
                "127.0.0.1".into(),
                "--port".into(),
                port.to_string(),
            ],
            cwd: dir.to_path_buf(),
            env: vec![],
        },
        stop,
        init: None,
        init_marker: None,
        port: Some(port),
        extra_ports: vec![],
        log: paths.logs().join(format!("{}.log", instance.name())),
    }
}

fn postgresql_service(
    paths: &Paths,
    config: &Config,
    instance: &DbInstance,
    dir: &Path,
) -> ServiceSpec {
    let port = instance.port(config);
    let data = instance.data_dir(paths);
    let bin = dir.join("bin");
    let data_arg = slash(&data);
    let command = |exe: &str, args: Vec<String>| Command {
        program: bin.join(exe),
        args,
        cwd: dir.to_path_buf(),
        env: vec![],
    };
    ServiceSpec {
        name: instance.name(),
        group: instance.name(),
        command: command(
            "postgres.exe",
            vec![
                "-D".into(),
                data_arg.clone(),
                "-p".into(),
                port.to_string(),
                "-c".into(),
                "listen_addresses=127.0.0.1".into(),
            ],
        ),
        stop: Some(command(
            "pg_ctl.exe",
            vec![
                "stop".into(),
                "-D".into(),
                data_arg.clone(),
                "-m".into(),
                "fast".into(),
                "-w".into(),
            ],
        )),
        init: Some(command(
            "initdb.exe",
            vec![
                "-D".into(),
                data_arg,
                "-U".into(),
                "postgres".into(),
                "--auth=trust".into(),
                "--encoding=UTF8".into(),
                "--locale=C".into(),
            ],
        )),
        // PG_VERSION ditulis initdb setelah data dir siap.
        init_marker: Some(data.join("PG_VERSION")),
        port: Some(port),
        extra_ports: vec![],
        log: paths.logs().join(format!("{}.log", instance.name())),
    }
}

/// `index.php` pembungkus Adminer: login tanpa password diizinkan (user database lokal
/// DevLock memang tanpa password dan site hanya terjangkau dari 127.0.0.1), dan field server
/// menawarkan instance database DevLock.
pub fn adminer_index(config: &Config, instances: &[DbInstance]) -> String {
    let mut options = String::new();
    let mut hints = String::new();
    for instance in instances {
        let (system, user) = if instance.kind == Kind::Postgresql {
            ("PostgreSQL", "postgres")
        } else {
            ("MySQL / MariaDB", "root")
        };
        let address = format!("127.0.0.1:{}", instance.port(config));
        let name = instance.name();
        let _ = write!(
            options,
            "<option value='{address}' label='{name} ({system}, user {user})'>"
        );
        let _ = write!(
            hints,
            "<br><code>{name}</code>: {system}, server <code>{address}</code>, user <code>{user}</code>"
        );
    }
    ADMINER_INDEX
        .replace("__OPTIONS__", &options)
        .replace("__HINTS__", &hints)
}

const ADMINER_INDEX: &str = r#"<?php
// Dibuat otomatis oleh DevLock; perubahan manual akan ditimpa.
// adminer.test hanya terjangkau dari 127.0.0.1 (nginx DevLock), dan user database lokal DevLock
// (root MySQL/MariaDB, postgres) memang tanpa password, jadi login tanpa password diizinkan.

function adminer_object() {
    // Tanpa type hint: method induk di adminer.php rilis (minified) tidak bertipe, dan PHP
    // menolak override yang mempersempit tipe parameter.
    class DevLockAdminer extends \Adminer\Adminer {
        function name() {
            return 'DevLock Adminer';
        }

        function login($login, $password) {
            return true;
        }

        function loginFormField($name, $heading, $value) {
            if ($name == 'server') {
                $value = str_replace("<input name='auth[server]'", "<input list='devlock-servers' name='auth[server]'", $value)
                    . "<datalist id='devlock-servers'>__OPTIONS__</datalist>"
                    . "<small>__HINTS__</small>";
            }
            return parent::loginFormField($name, $heading, $value);
        }
    }
    return new DevLockAdminer;
}

include __DIR__ . '/adminer.php';
"#;

/// Service cache beserta file konfigurasinya (`etc\redis\<major>.conf` untuk Redis).
/// Redis menyimpan snapshot RDB di `data\redis\<major>`; Garnet berjalan di memori saja.
fn cache_service(
    paths: &Paths,
    config: &Config,
    instance: &CacheInstance,
    dir: &Path,
) -> (ServiceSpec, Option<(PathBuf, String)>) {
    let port = instance.port(config);
    let log = paths.logs().join(format!("{}.log", instance.name()));
    if instance.kind == Kind::Garnet {
        // Runtime .NET portable yang dipasang bersama Garnet; DOTNET_ROOT_X64 ikut diisi
        // karena variabel itu (bila ada di lingkungan pengguna) diutamakan apphost.
        let dotnet = dir.join("dotnet").to_string_lossy().into_owned();
        let spec = ServiceSpec {
            name: instance.name(),
            group: instance.name(),
            command: Command {
                program: dir.join("GarnetServer.exe"),
                args: vec![
                    "--bind".into(),
                    "127.0.0.1".into(),
                    "--port".into(),
                    port.to_string(),
                    "--logger-level".into(),
                    "Warning".into(),
                ],
                cwd: dir.to_path_buf(),
                env: vec![
                    ("DOTNET_ROOT".into(), dotnet.clone()),
                    ("DOTNET_ROOT_X64".into(), dotnet),
                ],
            },
            stop: None,
            init: None,
            init_marker: None,
            port: Some(port),
            extra_ports: vec![],
            log,
        };
        return (spec, None);
    }

    let conf = paths
        .etc()
        .join("redis")
        .join(format!("{}.conf", instance.line));
    let content = format!(
        "# Dibuat otomatis oleh DevLock; perubahan manual akan ditimpa.\n\
         bind 127.0.0.1\n\
         port {port}\n\
         protected-mode yes\n\
         daemonize no\n\
         dir \"{data}\"\n\
         dbfilename dump.rdb\n\
         save 3600 1 300 100 60 10000\n\
         appendonly no\n\
         loglevel notice\n\
         logfile \"\"\n",
        data = slash(&instance.data_dir(paths)),
    );
    // Redis build msys2 menganggap path `D:/...` sebagai path relatif saat membuka file
    // config, jadi config diberikan sebagai nama file dari folder `etc\redis`.
    let spec = ServiceSpec {
        name: instance.name(),
        group: instance.name(),
        command: Command {
            program: dir.join("redis-server.exe"),
            args: vec![format!("{}.conf", instance.line)],
            cwd: paths.etc().join("redis"),
            env: vec![],
        },
        // SHUTDOWN menyimpan snapshot RDB sebelum berhenti.
        stop: Some(Command {
            program: dir.join("redis-cli.exe"),
            args: vec![
                "-h".into(),
                "127.0.0.1".into(),
                "-p".into(),
                port.to_string(),
                "shutdown".into(),
            ],
            cwd: dir.to_path_buf(),
            env: vec![],
        }),
        init: None,
        init_marker: None,
        port: Some(port),
        extra_ports: vec![],
        log,
    };
    (spec, Some((conf, content)))
}

fn slash(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// `NODE_EXTRA_CA_CERTS` agar Node (fetch, axios, Vite proxy) percaya sertifikat site HTTPS.
pub fn node_ca_env(paths: &Paths) -> Option<(String, String)> {
    let ca = ssl::ca_cert_path(paths);
    ca.is_file().then(|| {
        (
            "NODE_EXTRA_CA_CERTS".to_string(),
            ca.to_string_lossy().into_owned(),
        )
    })
}

pub fn nginx_conf(
    paths: &Paths,
    config: &Config,
    nginx_dir: &Path,
    sites: &[Site],
    workers: u16,
    certs: &BTreeMap<String, ssl::SiteCert>,
) -> String {
    let tmp = slash(&paths.tmp().join("nginx"));
    let logs = slash(&paths.logs());
    let listen = format!("{}:{}", config.http.listen, config.http.port);
    let https_listen = format!("{}:{}", config.http.listen, config.https.port);
    let mut c = String::new();
    let _ = write!(
        c,
        "# Dibuat otomatis oleh DevLock; perubahan manual akan ditimpa.\n\
         worker_processes 1;\n\
         error_log \"{logs}/nginx-error.log\" warn;\n\
         pid \"{run}/nginx.pid\";\n\n\
         events {{\n    worker_connections 1024;\n}}\n\n\
         http {{\n\
         \x20   include \"{conf}/mime.types\";\n\
         \x20   default_type application/octet-stream;\n\
         \x20   access_log \"{logs}/nginx-access.log\";\n\
         \x20   sendfile off;\n\
         \x20   client_max_body_size 128m;\n\
         \x20   server_names_hash_bucket_size 128;\n\
         \x20   client_body_temp_path \"{tmp}/client_body\";\n\
         \x20   proxy_temp_path \"{tmp}/proxy\";\n\
         \x20   fastcgi_temp_path \"{tmp}/fastcgi\";\n\
         \x20   uwsgi_temp_path \"{tmp}/uwsgi\";\n\
         \x20   scgi_temp_path \"{tmp}/scgi\";\n\n\
         \x20   map $http_upgrade $connection_upgrade {{\n\
         \x20       default upgrade;\n\
         \x20       '' close;\n\
         \x20   }}\n",
        run = slash(&paths.run()),
        conf = slash(&nginx_dir.join("conf")),
    );

    let lines: BTreeSet<&str> = sites
        .iter()
        .filter_map(|s| match &s.kind {
            SiteKind::Php { line } => Some(line.as_str()),
            _ => None,
        })
        .collect();
    for line in lines {
        let _ = writeln!(c, "\n    upstream {} {{", upstream_name(line));
        for worker in 0..workers {
            let _ = writeln!(c, "        server 127.0.0.1:{};", php_port(line, worker));
        }
        let _ = writeln!(c, "    }}");
    }

    let _ = write!(
        c,
        "\n    server {{\n        listen {listen} default_server;\n        server_name _;\n        return 404;\n    }}\n"
    );

    for site in sites {
        let root = slash(&site.docroot);
        let names = site.domains.join(" ");
        let cert = certs.get(&site.name);
        let redirect = site.https_redirect && cert.is_some();
        let _ = write!(c, "\n    # {dir}\n", dir = site.dir.display());
        if redirect {
            let port = match config.https.port {
                443 => String::new(),
                port => format!(":{port}"),
            };
            let _ = write!(
                c,
                "    server {{\n        listen {listen};\n        server_name {names};\n        return 301 https://$host{port}$request_uri;\n    }}\n\n"
            );
        }
        let _ = writeln!(c, "    server {{");
        if !redirect {
            let _ = writeln!(c, "        listen {listen};");
        }
        let _ = write!(
            c,
            "        server_name {names};\n        root \"{root}\";\n"
        );
        if let Some(cert) = cert {
            let _ = write!(
                c,
                "        listen {https_listen} ssl;\n        ssl_certificate \"{crt}\";\n        ssl_certificate_key \"{key}\";\n",
                crt = slash(&cert.cert),
                key = slash(&cert.key),
            );
        }
        if site.apache {
            let _ = write!(
                c,
                "\n        location / {{\n\
                 \x20           proxy_pass http://127.0.0.1:{port};\n\
                 \x20           proxy_http_version 1.1;\n\
                 \x20           proxy_set_header Host $host;\n\
                 \x20           proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;\n\
                 \x20           proxy_set_header X-Forwarded-Proto $scheme;\n\
                 \x20           proxy_set_header X-Forwarded-Port $server_port;\n\
                 \x20           proxy_read_timeout 300;\n\
                 \x20       }}\n    }}\n",
                port = config.apache.port,
            );
            continue;
        }
        match &site.kind {
            SiteKind::Php { line } => {
                let _ = write!(
                    c,
                    "        index index.php index.html;\n\n\
                     \x20       location / {{\n            try_files $uri $uri/ /index.php?$query_string;\n        }}\n\n\
                     \x20       location ~ \\.php$ {{\n\
                     \x20           try_files $uri =404;\n\
                     \x20           fastcgi_pass {upstream};\n\
                     \x20           fastcgi_index index.php;\n\
                     \x20           include \"{conf}/fastcgi_params\";\n\
                     \x20           fastcgi_param SCRIPT_FILENAME $document_root$fastcgi_script_name;\n\
                     \x20           fastcgi_read_timeout 300;\n\
                     \x20       }}\n",
                    upstream = upstream_name(line),
                    conf = slash(&nginx_dir.join("conf")),
                );
            }
            SiteKind::Proxy { port } => {
                let _ = write!(
                    c,
                    "\n        location / {{\n\
                     \x20           proxy_pass http://127.0.0.1:{port};\n\
                     \x20           proxy_http_version 1.1;\n\
                     \x20           proxy_set_header Host $host;\n\
                     \x20           proxy_set_header Upgrade $http_upgrade;\n\
                     \x20           proxy_set_header Connection $connection_upgrade;\n\
                     \x20           proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;\n\
                     \x20           proxy_set_header X-Forwarded-Proto $scheme;\n\
                     \x20       }}\n"
                );
            }
            SiteKind::Static => {
                let _ = write!(
                    c,
                    "        index index.html index.htm;\n\n        location / {{\n            try_files $uri $uri/ =404;\n        }}\n"
                );
            }
        }
        let _ = writeln!(c, "    }}");
    }
    c.push_str("}\n");
    c
}

/// Modul Apache yang dimuat: cukup untuk `.htaccess` umum (rewrite, header, expires, auth
/// basic, gaya lama `Order allow,deny`) dan PHP lewat FastCGI.
const APACHE_MODULES: &[&str] = &[
    "access_compat",
    "alias",
    "auth_basic",
    "authn_core",
    "authn_file",
    "authz_core",
    "authz_host",
    "authz_user",
    "autoindex",
    "deflate",
    "dir",
    "env",
    "expires",
    "filter",
    "headers",
    "lbmethod_byrequests",
    "log_config",
    "mime",
    "proxy",
    "proxy_balancer",
    "proxy_fcgi",
    "remoteip",
    "rewrite",
    "setenvif",
    "slotmem_shm",
];

/// `httpd.conf` untuk site `webserver = "apache"`: hanya mendengarkan di
/// `127.0.0.1:<apache.port>` di belakang nginx, PHP diteruskan ke pool php-cgi DevLock.
pub fn apache_conf(
    paths: &Paths,
    config: &Config,
    apache_dir: &Path,
    sites: &[Site],
    workers: u16,
) -> String {
    let logs = slash(&paths.logs());
    let listen = format!("127.0.0.1:{}", config.apache.port);
    let mut c = String::new();
    let _ = writeln!(
        c,
        "# Dibuat otomatis oleh DevLock; perubahan manual akan ditimpa.\n\
         ServerRoot \"{root}\"\n\
         Listen {listen}\n\
         ServerName localhost\n\
         PidFile \"{run}/httpd.pid\"\n\
         # Shared memory balancer dll. di tmp, bukan di folder runtime.\n\
         DefaultRuntimeDir \"{runtime}\"\n",
        root = slash(apache_dir),
        run = slash(&paths.run()),
        runtime = slash(&paths.tmp().join("apache")),
    );
    for module in APACHE_MODULES {
        let _ = writeln!(c, "LoadModule {module}_module modules/mod_{module}.so");
    }
    let _ = write!(
        c,
        "\nTypesConfig conf/mime.types\n\
         DirectoryIndex index.php index.html index.htm\n\
         AcceptFilter http none\n\
         AcceptFilter https none\n\
         EnableSendfile off\n\
         EnableMMAP off\n\
         ErrorLog \"{logs}/apache-error.log\"\n\
         LogLevel warn\n\
         LogFormat \"%a %l %u %t \\\"%r\\\" %>s %b \\\"%{{Referer}}i\\\" \\\"%{{User-Agent}}i\\\"\" combined\n\
         CustomLog \"{logs}/apache-access.log\" combined\n\n\
         # Request datang dari nginx DevLock: IP klien & HTTPS diambil dari header-nya.\n\
         RemoteIPHeader X-Forwarded-For\n\
         RemoteIPInternalProxy 127.0.0.1\n\
         SetEnvIf X-Forwarded-Proto \"^https$\" HTTPS=on\n\n\
         # php-cgi bukan PHP-FPM: SCRIPT_FILENAME harus path file Windows biasa\n\
         # (`D:/.../index.php`), bukan URL proxy.\n\
         ProxyFCGIBackendType GENERIC\n\
         ProxyFCGISetEnvIf \"true\" SCRIPT_FILENAME \"%{{DOCUMENT_ROOT}}%{{reqenv:SCRIPT_NAME}}\"\n\
         ProxyTimeout 300\n\n\
         <Directory />\n    AllowOverride None\n    Require all denied\n</Directory>\n"
    );

    let lines: BTreeSet<&str> = sites
        .iter()
        .filter(|s| s.apache)
        .filter_map(|s| match &s.kind {
            SiteKind::Php { line } => Some(line.as_str()),
            _ => None,
        })
        .collect();
    for line in &lines {
        let _ = writeln!(c, "\n<Proxy \"balancer://{}\">", balancer_name(line));
        for worker in 0..workers {
            let _ = writeln!(
                c,
                "    BalancerMember \"fcgi://127.0.0.1:{}\"",
                php_port(line, worker)
            );
        }
        let _ = writeln!(c, "</Proxy>");
    }

    // VirtualHost pertama menjadi default untuk Host yang tidak dikenal.
    let _ = write!(
        c,
        "\n<VirtualHost {listen}>\n    ServerName default.invalid\n    DocumentRoot \"{htdocs}\"\n    <Location />\n        Require all denied\n    </Location>\n</VirtualHost>\n",
        htdocs = slash(&apache_dir.join("htdocs")),
    );
    for site in sites.iter().filter(|s| s.apache) {
        let root = slash(&site.docroot);
        let _ = write!(
            c,
            "\n# {dir}\n<VirtualHost {listen}>\n    ServerName {name}\n",
            dir = site.dir.display(),
            name = site.domain(),
        );
        if site.domains.len() > 1 {
            let _ = writeln!(c, "    ServerAlias {}", site.domains[1..].join(" "));
        }
        let _ = write!(
            c,
            "    DocumentRoot \"{root}\"\n    <Directory \"{root}\">\n        Options Indexes FollowSymLinks\n        AllowOverride All\n        Require all granted\n    </Directory>\n"
        );
        if let SiteKind::Php { line } = &site.kind {
            // Garis miring penutup wajib: tanpa itu mod_proxy_balancer tidak mengenali
            // nama balancer dan request berakhir 500 ("No protocol handler").
            let _ = write!(
                c,
                "    <FilesMatch \"\\.php$\">\n        SetHandler \"proxy:balancer://{}/\"\n    </FilesMatch>\n",
                balancer_name(line)
            );
        }
        let _ = writeln!(c, "</VirtualHost>");
    }
    c
}

/// Nama balancer Apache (`8.3` → `php83`); hanya huruf/angka agar aman sebagai host URL.
fn balancer_name(line: &str) -> String {
    format!("php{}", line.replace('.', ""))
}

fn upstream_name(line: &str) -> String {
    format!("php_{}", line.replace('.', "_"))
}

/// Instance database untuk satu site (untuk ditampilkan di `site list`).
pub fn site_database(config: &Config, site: &Site) -> Option<DbInstance> {
    site.database.clone().or_else(|| {
        resolve::find_database(config, &site.dir)
            .ok()
            .flatten()
            .map(|(db, _)| db)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn php_ports() {
        assert_eq!(php_port("8.3", 0), 9830);
        assert_eq!(php_port("7.4", 3), 9743);
        assert_eq!(php_port("8.4", 9), 9849);
    }

    #[test]
    fn plan_builds_services_and_nginx_conf() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path());
        for (kind, v) in [
            ("php", "8.3.12"),
            ("php", "7.4.33"),
            ("nginx", "1.28.0"),
            ("mysql", "5.7.44"),
        ] {
            std::fs::create_dir_all(paths.runtime_dir(kind, v)).unwrap();
        }
        let www = paths.default_www();
        std::fs::create_dir_all(www.join("blog/public")).unwrap();
        std::fs::write(www.join("blog/composer.json"), "{}").unwrap();
        std::fs::create_dir_all(www.join("old")).unwrap();
        std::fs::write(www.join("old/index.php"), "").unwrap();
        std::fs::write(
            www.join("old/.devlock.toml"),
            "php = \"7.4\"\ndatabase = \"mysql@5.7\"\n",
        )
        .unwrap();
        std::fs::create_dir_all(www.join("dash")).unwrap();
        std::fs::write(
            www.join("dash/.devlock.toml"),
            "dev = \"npm run dev\"\nproxy_port = 3000\n",
        )
        .unwrap();

        let mut config = Config::default();
        config.php.workers = 2;
        let plan = plan(&paths, &config).unwrap();
        let names: Vec<_> = plan.services.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "php@7.4#0",
                "php@7.4#1",
                "php@8.3#0",
                "php@8.3#1",
                "nginx",
                "mysql@5.7",
                "dev:dash"
            ]
        );
        let mysql = plan
            .services
            .iter()
            .find(|s| s.name == "mysql@5.7")
            .unwrap();
        assert_eq!(mysql.port, Some(3357));
        assert!(plan.warnings.is_empty(), "{:?}", plan.warnings);

        let (_, conf) = plan
            .files
            .iter()
            .find(|(p, _)| p.ends_with("nginx.conf"))
            .unwrap();
        assert!(conf.contains(
            "upstream php_7_4 {\n        server 127.0.0.1:9740;\n        server 127.0.0.1:9741;"
        ));
        assert!(conf.contains("server_name blog.test;"));
        assert!(conf.contains("fastcgi_pass php_8_3;"));
        assert!(conf.contains("proxy_pass http://127.0.0.1:3000;"));
        assert!(conf.ends_with("}\n"));
    }

    #[test]
    fn mailpit_service_and_mail_site() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path());
        std::fs::create_dir_all(paths.runtime_dir("nginx", "1.30.6")).unwrap();
        let config = Config::default();
        let before = plan(&paths, &config).unwrap();
        assert!(before.services.iter().all(|s| s.name != "mailpit"));
        assert!(before.sites.iter().all(|s| s.name != "mail"));

        std::fs::create_dir_all(paths.runtime_dir("mailpit", "1.32.0")).unwrap();
        let plan = plan(&paths, &config).unwrap();
        let mailpit = plan.services.iter().find(|s| s.name == "mailpit").unwrap();
        assert_eq!(mailpit.port, Some(8025));
        assert_eq!(mailpit.extra_ports, [1025]);
        let args = mailpit.command.args.join(" ");
        assert!(args.contains("--smtp 127.0.0.1:1025 --listen 127.0.0.1:8025 --database "));
        assert!(paths.data().join("mailpit").is_dir());
        let site = plan.sites.iter().find(|s| s.name == "mail").unwrap();
        assert_eq!(site.domains, ["mail.test"]);
        assert_eq!(site.kind, SiteKind::Proxy { port: 8025 });
        let (_, conf) = plan
            .files
            .iter()
            .find(|(p, _)| p.ends_with("nginx.conf"))
            .unwrap();
        assert!(conf.contains("server_name mail.test;"));
        assert!(conf.contains("proxy_pass http://127.0.0.1:8025;"));
    }

    #[test]
    fn mongodb_instance_service() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path());
        std::fs::create_dir_all(paths.runtime_dir("mongodb", "8.0.34")).unwrap();
        std::fs::create_dir_all(paths.runtime_dir("mysql", "8.4.3")).unwrap();
        let mut config = Config::default();
        config
            .defaults
            .insert("database".into(), "mongodb@8.0".into());

        let plan = plan(&paths, &config).unwrap();
        let mongo = plan
            .services
            .iter()
            .find(|s| s.name == "mongodb@8.0")
            .unwrap();
        assert!(mongo.command.program.ends_with("bin/mongod.exe"));
        assert_eq!(mongo.port, Some(27080));
        let data = paths.data().join("mongodb").join("8.0");
        assert!(mongo
            .command
            .args
            .windows(2)
            .any(|w| w[0] == "--dbpath" && w[1] == data.to_string_lossy()));
        assert!(data.is_dir());
        // Tanpa mongosh: dihentikan langsung (tidak ada perintah stop).
        assert!(mongo.stop.is_none());

        std::fs::create_dir_all(paths.runtime_dir("mongosh", "2.14.0")).unwrap();
        let plan = super::plan(&paths, &config).unwrap();
        let mongo = plan
            .services
            .iter()
            .find(|s| s.name == "mongodb@8.0")
            .unwrap();
        let stop = mongo.stop.as_ref().unwrap();
        assert!(stop.program.ends_with("bin/mongosh.exe"));
        assert!(stop
            .args
            .contains(&"mongodb://127.0.0.1:27080/admin".to_string()));
    }

    #[test]
    fn postgresql_instance_service() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path());
        std::fs::create_dir_all(paths.runtime_dir("postgresql", "18.6")).unwrap();
        let www = paths.default_www();
        std::fs::create_dir_all(www.join("api")).unwrap();
        std::fs::write(www.join("api/index.html"), "").unwrap();
        std::fs::write(
            www.join("api/.devlock.toml"),
            "database = \"postgresql@18\"
",
        )
        .unwrap();

        let plan = plan(&paths, &Config::default()).unwrap();
        let pg = plan
            .services
            .iter()
            .find(|s| s.name == "postgresql@18")
            .unwrap();
        assert_eq!(pg.port, Some(5418));
        assert!(pg.command.program.ends_with("bin/postgres.exe"));
        assert!(pg
            .command
            .args
            .contains(&"listen_addresses=127.0.0.1".to_string()));
        assert!(pg
            .init
            .as_ref()
            .unwrap()
            .program
            .ends_with("bin/initdb.exe"));
        assert_eq!(
            pg.init_marker.as_deref(),
            Some(paths.data().join("postgresql/18/PG_VERSION").as_path())
        );
        // PostgreSQL tidak memakai my.ini.
        assert!(!plan.files.iter().any(|(p, _)| p.ends_with("my.ini")));
    }

    #[test]
    fn php_project_dev_server_runs_beside_php() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path());
        std::fs::create_dir_all(paths.runtime_dir("nginx", "1.30.6")).unwrap();
        std::fs::create_dir_all(paths.runtime_dir("php", "8.3.12")).unwrap();
        let www = paths.default_www();
        std::fs::create_dir_all(www.join("toko/public")).unwrap();
        std::fs::write(www.join("toko/composer.json"), "{}").unwrap();
        std::fs::write(www.join("toko/public/index.php"), "").unwrap();
        std::fs::write(www.join("toko/.devlock.toml"), "dev = \"npm run dev\"\n").unwrap();

        let plan = plan(&paths, &Config::default()).unwrap();
        let site = plan.sites.iter().find(|s| s.name == "toko").unwrap();
        assert_eq!(site.kind, SiteKind::Php { line: "8.3".into() });
        assert!(!site.dev_port_auto);
        let dev = plan.services.iter().find(|s| s.name == "dev:toko").unwrap();
        assert_eq!(dev.port, None);
        assert!(dev.command.args.contains(&"npm run dev".to_string()));
        assert!(dev.command.env.iter().all(|(k, _)| k != "PORT"));
        let (_, conf) = plan
            .files
            .iter()
            .find(|(p, _)| p.ends_with("nginx.conf"))
            .unwrap();
        let block = &conf[conf.find("server_name toko.test;").unwrap()..];
        let block = &block[..block.find("\n    }\n").unwrap()];
        assert!(block.contains("fastcgi_pass php_8_3;"));
        assert!(!block.contains("proxy_pass"));
    }

    #[test]
    fn dev_server_without_proxy_port_uses_detected_port() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path());
        std::fs::create_dir_all(paths.runtime_dir("nginx", "1.30.6")).unwrap();
        let www = paths.default_www();
        std::fs::create_dir_all(www.join("spa")).unwrap();
        std::fs::write(www.join("spa/.devlock.toml"), "dev = \"pnpm dev\"\n").unwrap();
        let assigned = crate::devport::assigned_port("spa");
        let conf_of = |plan: &Plan| {
            plan.files
                .iter()
                .find(|(p, _)| p.ends_with("nginx.conf"))
                .unwrap()
                .1
                .clone()
        };

        let before = plan(&paths, &Config::default()).unwrap();
        let dev = before
            .services
            .iter()
            .find(|s| s.name == "dev:spa")
            .unwrap();
        assert_eq!(dev.port, None);
        assert!(dev
            .command
            .env
            .contains(&("PORT".into(), assigned.to_string())));
        assert!(conf_of(&before).contains(&format!("proxy_pass http://127.0.0.1:{assigned};")));
        let site = before.sites.iter().find(|s| s.name == "spa").unwrap();
        assert!(site.dev_port_auto);
        assert_eq!(site.kind, SiteKind::Proxy { port: assigned });

        // Vite memakai 5173 walau diberi PORT: nginx mengikuti, spec dev server tetap.
        crate::devport::store_detected(&paths, "spa", 5173).unwrap();
        let after = plan(&paths, &Config::default()).unwrap();
        assert!(conf_of(&after).contains("proxy_pass http://127.0.0.1:5173;"));
        let dev_after = after.services.iter().find(|s| s.name == "dev:spa").unwrap();
        assert_eq!(dev, dev_after);
    }

    #[test]
    fn https_site_gets_ssl_listener() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path());
        std::fs::create_dir_all(paths.runtime_dir("nginx", "1.30.6")).unwrap();
        let www = paths.default_www();
        for name in ["secure", "plain"] {
            std::fs::create_dir_all(www.join(name)).unwrap();
            std::fs::write(www.join(name).join("index.html"), "").unwrap();
        }
        std::fs::write(www.join("secure/.devlock.toml"), "https = true\n").unwrap();

        let plan = plan(&paths, &Config::default()).unwrap();
        let (_, conf) = plan
            .files
            .iter()
            .find(|(p, _)| p.ends_with("nginx.conf"))
            .unwrap();
        assert_eq!(conf.matches("listen 127.0.0.1:443 ssl;").count(), 1);
        let crt = slash(&paths.ssl().join("sites/secure.crt"));
        assert!(conf.contains(&format!("ssl_certificate \"{crt}\";")));
        assert!(paths.ssl().join("sites/secure.key").is_file());
        assert!(!paths.ssl().join("sites/plain.crt").exists());
        assert!(plan
            .sites
            .iter()
            .any(|s| s.name == "secure" && s.url(&Config::default()) == "https://secure.test"));
        assert!(!conf.contains("return 301"));
    }

    #[test]
    fn https_redirect_moves_http_to_redirect_server() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path());
        std::fs::create_dir_all(paths.runtime_dir("nginx", "1.30.6")).unwrap();
        let www = paths.default_www();
        for name in ["secure", "plain", "optout"] {
            std::fs::create_dir_all(www.join(name)).unwrap();
            std::fs::write(www.join(name).join("index.html"), "").unwrap();
        }
        std::fs::write(
            www.join("secure/.devlock.toml"),
            "https = true\nhttps_redirect = true\n",
        )
        .unwrap();
        // Redirect tanpa HTTPS tidak berlaku.
        std::fs::write(www.join("plain/.devlock.toml"), "https_redirect = true\n").unwrap();
        std::fs::write(
            www.join("optout/.devlock.toml"),
            "https = true\nhttps_redirect = false\n",
        )
        .unwrap();
        let mut config = Config::default();
        config.https.redirect = true;
        config.https.port = 8443;

        let plan = plan(&paths, &config).unwrap();
        let (_, conf) = plan
            .files
            .iter()
            .find(|(p, _)| p.ends_with("nginx.conf"))
            .unwrap();
        let block = |name: &str| {
            let start = conf
                .find(&format!("www{}{name}\n", std::path::MAIN_SEPARATOR))
                .unwrap();
            let end = conf[start + 1..]
                .find("\n    # ")
                .map_or(conf.len(), |i| start + 1 + i);
            conf[start..end].to_string()
        };
        let secure = block("secure");
        assert!(secure.contains(
            "listen 127.0.0.1:80;\n        server_name secure.test;\n        return 301 https://$host:8443$request_uri;"
        ));
        assert_eq!(secure.matches("listen 127.0.0.1:80;").count(), 1);
        assert!(secure.contains("listen 127.0.0.1:8443 ssl;"));
        let plain = block("plain");
        assert!(!plain.contains("return 301") && plain.contains("listen 127.0.0.1:80;"));
        let optout = block("optout");
        assert!(!optout.contains("return 301") && optout.contains("listen 127.0.0.1:8443 ssl;"));
        assert!(plan
            .sites
            .iter()
            .any(|s| s.name == "secure" && s.https_redirect));
        assert!(plan
            .sites
            .iter()
            .any(|s| s.name == "plain" && !s.https_redirect));
    }

    #[test]
    fn adminer_builtin_site() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path());
        std::fs::create_dir_all(paths.runtime_dir("nginx", "1.30.6")).unwrap();
        std::fs::create_dir_all(paths.runtime_dir("php", "8.4.27")).unwrap();
        let mut config = Config::default();
        config
            .defaults
            .insert("database".into(), "mysql@8.4".into());

        // Belum terinstall: tidak ada site adminer.
        assert!(plan(&paths, &config)
            .unwrap()
            .sites
            .iter()
            .all(|s| s.name != "adminer"));

        let dir = paths.runtime_dir("adminer", "6.2.0");
        std::fs::create_dir_all(&dir).unwrap();
        let with = plan(&paths, &config).unwrap();
        let site = with.sites.iter().find(|s| s.name == "adminer").unwrap();
        assert_eq!(site.domains, ["adminer.test"]);
        assert_eq!(site.kind, SiteKind::Php { line: "8.4".into() });
        assert!(site.is_adminer(&paths));
        let (_, index) = with
            .files
            .iter()
            .find(|(p, _)| *p == dir.join("index.php"))
            .unwrap();
        assert!(index.contains(
            "<option value='127.0.0.1:3384' label='mysql@8.4 (MySQL / MariaDB, user root)'>"
        ));
        assert!(index.contains("include __DIR__ . '/adminer.php';"));
        assert!(!index.contains("__OPTIONS__"));
        // Override harus sesignature method Adminer yang tidak bertipe.
        assert!(index.contains("function loginFormField($name, $heading, $value) {"));
        assert!(!index.contains("string $"));

        // Folder www\adminer milik pengguna menggantikan site bawaan.
        let www = paths.default_www().join("adminer");
        std::fs::create_dir_all(&www).unwrap();
        std::fs::write(www.join("index.html"), "").unwrap();
        let user = plan(&paths, &config).unwrap();
        let sites: Vec<_> = user.sites.iter().filter(|s| s.name == "adminer").collect();
        assert_eq!(sites.len(), 1);
        assert!(!sites[0].is_adminer(&paths));
        assert!(user.files.iter().all(|(p, _)| *p != dir.join("index.php")));
    }

    #[test]
    fn cache_instances_from_sites_and_default() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path());
        std::fs::create_dir_all(paths.runtime_dir("redis", "8.10.3")).unwrap();
        std::fs::create_dir_all(paths.runtime_dir("garnet", "2.2.4")).unwrap();
        let www = paths.default_www();
        std::fs::create_dir_all(www.join("app")).unwrap();
        std::fs::write(www.join("app/index.html"), "").unwrap();
        std::fs::write(www.join("app/.devlock.toml"), "cache = \"redis@8\"\n").unwrap();
        let mut config = Config::default();
        config.defaults.insert("cache".into(), "garnet@2".into());

        let plan = plan(&paths, &config).unwrap();
        let redis = plan.services.iter().find(|s| s.name == "redis@8").unwrap();
        assert_eq!(redis.port, Some(6389));
        assert!(redis.command.program.ends_with("redis-server.exe"));
        assert_eq!(redis.command.args, ["8.conf"]);
        assert_eq!(redis.command.cwd, paths.etc().join("redis"));
        assert_eq!(
            redis.stop.as_ref().unwrap().args.last().unwrap(),
            "shutdown"
        );
        let garnet = plan.services.iter().find(|s| s.name == "garnet@2").unwrap();
        assert_eq!(garnet.port, Some(6402));
        assert!(garnet.command.args.contains(&"127.0.0.1".to_string()));
        let dotnet = paths.runtime_dir("garnet", "2.2.4").join("dotnet");
        assert!(garnet
            .command
            .env
            .contains(&("DOTNET_ROOT".into(), dotnet.to_string_lossy().into_owned())));

        let (_, conf) = plan
            .files
            .iter()
            .find(|(p, _)| p.ends_with("8.conf"))
            .unwrap();
        assert!(conf.contains("bind 127.0.0.1\nport 6389\n"));
        let data = slash(&paths.data().join("redis").join("8"));
        assert!(conf.contains(&format!("dir \"{data}\"")));
        assert!(paths.data().join("redis").join("8").is_dir());
    }

    #[test]
    fn apache_sites_are_proxied_from_nginx() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path());
        std::fs::create_dir_all(paths.runtime_dir("nginx", "1.30.6")).unwrap();
        std::fs::create_dir_all(paths.runtime_dir("php", "8.3.12")).unwrap();
        let www = paths.default_www();
        std::fs::create_dir_all(www.join("legacy")).unwrap();
        std::fs::write(www.join("legacy/index.php"), "").unwrap();
        std::fs::write(
            www.join("legacy/.devlock.toml"),
            "webserver = \"apache\"\naliases = [\"old.local\"]\n",
        )
        .unwrap();
        std::fs::create_dir_all(www.join("modern")).unwrap();
        std::fs::write(www.join("modern/index.php"), "").unwrap();
        let config = Config::default();

        // Apache belum terinstall: tetap dilayani nginx, dengan peringatan.
        let without = plan(&paths, &config).unwrap();
        assert!(without.services.iter().all(|s| s.name != "apache"));
        assert!(without.warnings.iter().any(|w| w.contains("legacy.test")));
        assert!(without.sites.iter().all(|s| !s.apache));

        std::fs::create_dir_all(paths.runtime_dir("apache", "2.4.69")).unwrap();
        let with = plan(&paths, &config).unwrap();
        let names: Vec<&str> = with.services.iter().map(|s| s.name.as_str()).collect();
        let apache = names.iter().position(|n| *n == "apache").unwrap();
        let nginx = names.iter().position(|n| *n == "nginx").unwrap();
        assert!(apache < nginx, "Apache siap sebelum nginx: {names:?}");
        assert_eq!(with.services[apache].port, Some(8180));

        let file = |suffix: &str| {
            with.files
                .iter()
                .find(|(p, _)| p.ends_with(suffix))
                .map(|(_, c)| c.clone())
                .unwrap()
        };
        let nginx_conf = file("nginx.conf");
        let legacy = nginx_conf.split("server_name legacy.test").nth(1).unwrap();
        let legacy = legacy.split("\n    }\n").next().unwrap();
        assert!(legacy.contains("proxy_pass http://127.0.0.1:8180;"));
        assert!(!legacy.contains("fastcgi_pass"));
        let modern = nginx_conf.split("server_name modern.test").nth(1).unwrap();
        assert!(modern.contains("fastcgi_pass"));

        let httpd = file("httpd.conf");
        assert!(httpd.contains("Listen 127.0.0.1:8180"));
        assert!(httpd.contains("ServerName legacy.test\n    ServerAlias old.local"));
        assert!(httpd.contains("AllowOverride All"));
        assert!(httpd.contains("BalancerMember \"fcgi://127.0.0.1:9830\""));
        assert!(httpd.contains("SetHandler \"proxy:balancer://php83/\""));
        assert!(httpd.contains("ProxyFCGIBackendType GENERIC"));
        assert!(httpd.contains(
            "ProxyFCGISetEnvIf \"true\" SCRIPT_FILENAME \"%{DOCUMENT_ROOT}%{reqenv:SCRIPT_NAME}\""
        ));
        let runtime = slash(&paths.tmp().join("apache"));
        assert!(httpd.contains(&format!("DefaultRuntimeDir \"{runtime}\"")));
        assert!(!httpd.contains("modern.test"));
    }
}
