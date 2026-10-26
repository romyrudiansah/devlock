//! Rencana service (nginx, php-cgi, database, dev server) dari konfigurasi + site,
//! termasuk file konfigurasi yang harus ditulis sebelum service dijalankan.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::paths::Paths;
use crate::resolve::{self, DbInstance};
use crate::runtime::{self, Kind};
use crate::sites::{self, Site, SiteKind};
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
    let sites = sites::discover(paths, config)?;
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
                log: paths.logs().join(format!("php@{line}.log")),
            });
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
            files.push((
                conf.clone(),
                nginx_conf(paths, config, &dir, &sites, workers),
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
    for instance in instances {
        match runtime::resolve_installed(paths, instance.kind, &instance.line)? {
            Some(version) => {
                let dir = paths.runtime_dir(instance.kind.as_str(), &version);
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

    // Dev server project Node: dijalankan lewat shim agar versi Node sesuai project.
    for site in &sites {
        let (Some(dev), SiteKind::Proxy { port }) = (&site.dev, &site.kind) else {
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
        services.push(ServiceSpec {
            name: format!("dev:{}", site.name),
            group: format!("dev:{}", site.name),
            command: Command {
                program: PathBuf::from(
                    std::env::var("ComSpec").unwrap_or_else(|_| "cmd.exe".into()),
                ),
                args: vec!["/d".into(), "/s".into(), "/c".into(), dev.clone()],
                cwd: site.dir.clone(),
                env: vec![
                    ("PORT".into(), port.to_string()),
                    ("PATH".into(), path.to_string_lossy().into()),
                    ("BROWSER".into(), "none".into()),
                ],
            },
            stop: None,
            init: None,
            init_marker: None,
            port: Some(*port),
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
        log: paths.logs().join(format!("{}.log", instance.name())),
    };
    (spec, Some((ini_path, ini)))
}

/// PostgreSQL: `initdb` sekali (superuser `postgres`, auth trust, hanya 127.0.0.1),
/// lalu `postgres.exe`; berhenti rapi lewat `pg_ctl stop -m fast`.
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
        log: paths.logs().join(format!("{}.log", instance.name())),
    }
}

fn slash(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

pub fn nginx_conf(
    paths: &Paths,
    config: &Config,
    nginx_dir: &Path,
    sites: &[Site],
    workers: u16,
) -> String {
    let tmp = slash(&paths.tmp().join("nginx"));
    let logs = slash(&paths.logs());
    let listen = format!("{}:{}", config.http.listen, config.http.port);
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
        let _ = write!(
            c,
            "\n    # {dir}\n    server {{\n        listen {listen};\n        server_name {names};\n        root \"{root}\";\n",
            dir = site.dir.display(),
            names = site.domains.join(" "),
        );
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
}
