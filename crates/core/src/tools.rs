//! Daftar perintah yang punya shim dan cara menjalankannya dengan versi yang tepat.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

use crate::config::Config;
use crate::paths::Paths;
use crate::resolve::{self, Resolved, Source};
use crate::runtime::{self, Kind};

enum Launch {
    /// Executable di folder runtime; kandidat dicoba berurutan.
    Exe(&'static [&'static str]),
    /// Script JS dijalankan dengan `node.exe`; kandidat dicoba berurutan.
    NodeScript(&'static [&'static str]),
    /// `composer.phar` dijalankan dengan `php.exe` milik project.
    Composer,
    /// `python.exe -m <modul>` milik project (mis. `pip`).
    PythonModule(&'static str),
    /// Client database di `bin\` instance milik project. `Kind` pada tabel menyatakan
    /// keluarganya: `Mysql` (MySQL/MariaDB) atau `Postgresql`.
    DbClient(&'static [&'static str]),
    /// `redis-cli` dari Redis terinstall, diarahkan ke instance cache milik project
    /// (Redis atau Garnet).
    CacheClient,
    /// `mongosh` terinstall, diarahkan ke instance MongoDB milik project.
    MongoClient,
}

const TOOLS: &[(&str, Kind, Launch)] = &[
    ("php", Kind::Php, Launch::Exe(&["php.exe"])),
    ("php-cgi", Kind::Php, Launch::Exe(&["php-cgi.exe"])),
    ("composer", Kind::Composer, Launch::Composer),
    ("go", Kind::Go, Launch::Exe(&["bin/go.exe"])),
    ("gofmt", Kind::Go, Launch::Exe(&["bin/gofmt.exe"])),
    ("python", Kind::Python, Launch::Exe(&["python.exe"])),
    ("pip", Kind::Python, Launch::PythonModule("pip")),
    ("node", Kind::Node, Launch::Exe(&["node.exe"])),
    (
        "npm",
        Kind::Node,
        Launch::NodeScript(&["node_modules/npm/bin/npm-cli.js"]),
    ),
    (
        "npx",
        Kind::Node,
        Launch::NodeScript(&["node_modules/npm/bin/npx-cli.js"]),
    ),
    (
        "corepack",
        Kind::Node,
        Launch::NodeScript(&["node_modules/corepack/dist/corepack.js"]),
    ),
    (
        "pnpm",
        Kind::Node,
        Launch::NodeScript(&[
            "node_modules/pnpm/bin/pnpm.cjs",
            "node_modules/corepack/dist/pnpm.js",
        ]),
    ),
    (
        "yarn",
        Kind::Node,
        Launch::NodeScript(&[
            "node_modules/yarn/bin/yarn.js",
            "node_modules/corepack/dist/yarn.js",
        ]),
    ),
    (
        "mysql",
        Kind::Mysql,
        Launch::DbClient(&["bin/mysql.exe", "bin/mariadb.exe"]),
    ),
    (
        "mariadb",
        Kind::Mariadb,
        Launch::DbClient(&["bin/mariadb.exe", "bin/mysql.exe"]),
    ),
    (
        "mysqldump",
        Kind::Mysql,
        Launch::DbClient(&["bin/mysqldump.exe", "bin/mariadb-dump.exe"]),
    ),
    (
        "mysqladmin",
        Kind::Mysql,
        Launch::DbClient(&["bin/mysqladmin.exe", "bin/mariadb-admin.exe"]),
    ),
    (
        "psql",
        Kind::Postgresql,
        Launch::DbClient(&["bin/psql.exe"]),
    ),
    (
        "pg_dump",
        Kind::Postgresql,
        Launch::DbClient(&["bin/pg_dump.exe"]),
    ),
    (
        "pg_restore",
        Kind::Postgresql,
        Launch::DbClient(&["bin/pg_restore.exe"]),
    ),
    (
        "createdb",
        Kind::Postgresql,
        Launch::DbClient(&["bin/createdb.exe"]),
    ),
    (
        "dropdb",
        Kind::Postgresql,
        Launch::DbClient(&["bin/dropdb.exe"]),
    ),
    ("redis-cli", Kind::Redis, Launch::CacheClient),
    ("mongosh", Kind::Mongosh, Launch::MongoClient),
];

/// Nama semua shim (tanpa `.exe`).
pub fn names() -> impl Iterator<Item = &'static str> {
    TOOLS.iter().map(|(name, _, _)| *name)
}

/// Hasil resolusi: program yang dijalankan beserta argumen awal dan environment.
#[derive(Debug)]
pub struct Plan {
    pub program: PathBuf,
    pub prefix_args: Vec<OsString>,
    pub env: Vec<(String, OsString)>,
    /// Folder yang ditaruh di depan PATH proses anak.
    pub path_prepend: Vec<PathBuf>,
    /// Ringkasan versi yang dipakai, untuk `devlock which`.
    pub resolved: Vec<(String, String, Source)>,
}

impl Plan {
    fn new(program: PathBuf) -> Self {
        Self {
            program,
            prefix_args: Vec::new(),
            env: Vec::new(),
            path_prepend: Vec::new(),
            resolved: Vec::new(),
        }
    }

    fn record(&mut self, r: &Resolved) {
        self.resolved
            .push((r.kind.to_string(), r.version.clone(), r.source.clone()));
    }

    /// PATH baru untuk proses anak.
    pub fn child_path(&self) -> Result<OsString> {
        let current = std::env::var_os("PATH").unwrap_or_default();
        let dirs = self
            .path_prepend
            .iter()
            .cloned()
            .chain(std::env::split_paths(&current));
        std::env::join_paths(dirs).context("PATH tidak valid")
    }
}

pub fn plan(paths: &Paths, config: &Config, cwd: &Path, tool: &str) -> Result<Plan> {
    let Some((_, kind, launch)) = TOOLS
        .iter()
        .find(|(name, _, _)| name.eq_ignore_ascii_case(tool))
    else {
        bail!("`{tool}` bukan perintah yang dikelola DevLock");
    };

    match launch {
        Launch::Exe(candidates) => {
            let r = resolve::resolve(paths, config, *kind, cwd)?;
            let mut plan = Plan::new(first_existing(&r.dir, candidates, tool)?);
            plan.path_prepend.push(r.dir.clone());
            match kind {
                // `go` memanggil `gofmt` dkk. dari bin; skrip pip (`Scripts\`) ikut di PATH.
                Kind::Go => plan.path_prepend.push(r.dir.join("bin")),
                Kind::Python => plan.path_prepend.push(r.dir.join("Scripts")),
                _ => {}
            }
            if *kind == Kind::Node {
                plan.env
                    .push(("NPM_CONFIG_PREFIX".into(), r.dir.clone().into()));
                plan.env.extend(node_ca_env(paths));
            }
            plan.record(&r);
            Ok(plan)
        }
        Launch::NodeScript(candidates) => {
            let r = resolve::resolve(paths, config, Kind::Node, cwd)?;
            let script = first_existing(&r.dir, candidates, tool).with_context(|| {
                format!(
                    "install dulu: `npm install -g {tool}` atau `corepack enable` (Node {})",
                    r.version
                )
            })?;
            let mut plan = Plan::new(r.dir.join("node.exe"));
            plan.prefix_args.push(script.into());
            plan.path_prepend.push(r.dir.clone());
            plan.env
                .push(("NPM_CONFIG_PREFIX".into(), r.dir.clone().into()));
            plan.env.extend(node_ca_env(paths));
            plan.record(&r);
            Ok(plan)
        }
        Launch::PythonModule(module) => {
            let r = resolve::resolve(paths, config, Kind::Python, cwd)?;
            let mut plan = Plan::new(first_existing(&r.dir, &["python.exe"], tool)?);
            plan.prefix_args
                .extend(["-m".into(), OsString::from(*module)]);
            plan.path_prepend.push(r.dir.clone());
            plan.path_prepend.push(r.dir.join("Scripts"));
            plan.record(&r);
            Ok(plan)
        }
        Launch::Composer => {
            let php = resolve::resolve(paths, config, Kind::Php, cwd)?;
            let composer = resolve::resolve(paths, config, Kind::Composer, cwd)?;
            let mut plan = Plan::new(php.dir.join("php.exe"));
            plan.prefix_args
                .push(composer.dir.join("composer.phar").into());
            plan.path_prepend.push(php.dir.clone());
            plan.record(&php);
            plan.record(&composer);
            Ok(plan)
        }
        Launch::DbClient(candidates) => {
            let Some((instance, source)) = resolve::find_database(config, cwd)? else {
                bail!("belum ada database untuk folder ini; set `database = \"mysql@8.4\"` atau `\"postgresql@16\"` di .devlock.toml, atau `devlock use database <engine@versi> --global`");
            };
            let wants_postgres = *kind == Kind::Postgresql;
            let is_postgres = instance.kind == Kind::Postgresql;
            if wants_postgres != is_postgres || !instance.is_sql() {
                bail!(
                    "database folder ini {} (dari {source}); `{tool}` untuk {}",
                    instance.name(),
                    if wants_postgres {
                        "PostgreSQL"
                    } else {
                        "MySQL/MariaDB"
                    }
                );
            }
            let (version, dir) = runtime::require_installed(paths, instance.kind, &instance.line)?;
            let mut plan = Plan::new(first_existing(&dir, candidates, tool)?);
            let port = instance.port(config).to_string();
            if wants_postgres {
                plan.env.push(("PGHOST".into(), "127.0.0.1".into()));
                plan.env.push(("PGPORT".into(), port.into()));
                plan.env.push(("PGUSER".into(), "postgres".into()));
                plan.path_prepend.push(dir.join("bin"));
            } else {
                plan.env.push(("MYSQL_HOST".into(), "127.0.0.1".into()));
                plan.env.push(("MYSQL_TCP_PORT".into(), port.into()));
            }
            plan.resolved.push((instance.name(), version, source));
            Ok(plan)
        }
        Launch::CacheClient => {
            let Some((instance, source)) = resolve::find_cache(config, cwd)? else {
                bail!("belum ada cache untuk folder ini; set `cache = \"redis@8\"` di .devlock.toml, atau `devlock use cache redis@8 --global`");
            };
            // Garnet tidak membawa redis-cli: pakai milik Redis terinstall terbaru.
            let spec = if instance.kind == Kind::Redis {
                instance.line.as_str()
            } else {
                "latest"
            };
            let Some(version) = runtime::resolve_installed(paths, Kind::Redis, spec)? else {
                bail!("redis-cli butuh Redis terinstall (`devlock install redis {spec}`)");
            };
            let dir = paths.runtime_dir("redis", &version);
            let mut plan = Plan::new(first_existing(&dir, &["redis-cli.exe"], tool)?);
            // Di depan argumen pengguna: `-h`/`-p` yang ditulis sendiri tetap menang.
            plan.prefix_args.extend([
                "-h".into(),
                "127.0.0.1".into(),
                "-p".into(),
                instance.port(config).to_string().into(),
            ]);
            plan.resolved.push((instance.name(), version, source));
            Ok(plan)
        }
        Launch::MongoClient => {
            let Some((instance, source)) = resolve::find_database(config, cwd)? else {
                bail!("belum ada database untuk folder ini; set `database = \"mongodb@8.0\"` di .devlock.toml, atau `devlock use database mongodb@8.0 --global`");
            };
            if instance.kind != Kind::Mongodb {
                bail!(
                    "database folder ini {} (dari {source}); `mongosh` untuk MongoDB",
                    instance.name()
                );
            }
            let Some(version) = runtime::resolve_installed(paths, Kind::Mongosh, "latest")? else {
                bail!("mongosh belum terinstall (`devlock install mongosh`)");
            };
            let dir = paths.runtime_dir(Kind::Mongosh.as_str(), &version);
            let mut plan = Plan::new(first_existing(&dir, &["bin/mongosh.exe"], tool)?);
            plan.prefix_args
                .push(format!("mongodb://127.0.0.1:{}", instance.port(config)).into());
            plan.resolved.push((instance.name(), version, source));
            Ok(plan)
        }
    }
}

/// `NODE_EXTRA_CA_CERTS` dalam bentuk env shim, bila CA DevLock sudah dibuat.
fn node_ca_env(paths: &Paths) -> Option<(String, OsString)> {
    crate::services::node_ca_env(paths).map(|(k, v)| (k, v.into()))
}

fn first_existing(dir: &Path, candidates: &[&str], tool: &str) -> Result<PathBuf> {
    candidates
        .iter()
        .map(|c| dir.join(c))
        .find(|p| p.is_file())
        .with_context(|| format!("`{tool}` tidak ditemukan di {}", dir.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_script_plan() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path());
        let node = paths.runtime_dir("node", "22.11.0");
        std::fs::create_dir_all(node.join("node_modules/npm/bin")).unwrap();
        std::fs::write(node.join("node.exe"), b"").unwrap();
        std::fs::write(node.join("node_modules/npm/bin/npm-cli.js"), b"").unwrap();

        let plan = plan(&paths, &Config::default(), tmp.path(), "npm").unwrap();
        assert_eq!(plan.program, node.join("node.exe"));
        assert_eq!(
            plan.prefix_args,
            vec![OsString::from(node.join("node_modules/npm/bin/npm-cli.js"))]
        );
        assert!(plan.env.iter().any(|(k, _)| k == "NPM_CONFIG_PREFIX"));

        let err = super::plan(&paths, &Config::default(), tmp.path(), "pnpm").unwrap_err();
        assert!(format!("{err:#}").contains("corepack enable"));
    }

    #[test]
    fn db_client_uses_instance_port() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path());
        let dir = paths.runtime_dir("mariadb", "11.4.3");
        std::fs::create_dir_all(dir.join("bin")).unwrap();
        std::fs::write(dir.join("bin/mariadb.exe"), b"").unwrap();
        let mut config = Config::default();
        config
            .defaults
            .insert("database".into(), "mariadb@11.4".into());

        let plan = plan(&paths, &config, tmp.path(), "mysql").unwrap();
        assert_eq!(plan.program, dir.join("bin/mariadb.exe"));
        assert!(plan.env.contains(&("MYSQL_TCP_PORT".into(), "3424".into())));
    }

    #[test]
    fn unknown_tool() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(plan(
            &Paths::new(tmp.path()),
            &Config::default(),
            tmp.path(),
            "python"
        )
        .is_err());
    }

    #[test]
    fn postgres_client_uses_instance_port() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path());
        let dir = paths.runtime_dir("postgresql", "16.15");
        std::fs::create_dir_all(dir.join("bin")).unwrap();
        std::fs::write(dir.join("bin/psql.exe"), b"").unwrap();
        let mut config = Config::default();
        config
            .defaults
            .insert("database".into(), "postgresql@16".into());

        let plan = plan(&paths, &config, tmp.path(), "psql").unwrap();
        assert_eq!(plan.program, dir.join("bin/psql.exe"));
        assert!(plan.env.contains(&("PGPORT".into(), "5416".into())));
        assert!(plan.env.contains(&("PGUSER".into(), "postgres".into())));

        let err = super::plan(&paths, &config, tmp.path(), "mysql").unwrap_err();
        assert!(err.to_string().contains("postgresql@16"));
    }

    #[test]
    fn redis_cli_targets_cache_instance() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path());
        let redis = paths.runtime_dir("redis", "8.10.3");
        std::fs::create_dir_all(&redis).unwrap();
        std::fs::write(redis.join("redis-cli.exe"), b"").unwrap();
        let mut config = Config::default();
        assert!(plan(&paths, &config, tmp.path(), "redis-cli").is_err());

        config.defaults.insert("cache".into(), "redis@8".into());
        let p = plan(&paths, &config, tmp.path(), "redis-cli").unwrap();
        assert_eq!(p.program, redis.join("redis-cli.exe"));
        let args: Vec<_> = p.prefix_args.iter().map(|a| a.to_string_lossy()).collect();
        assert_eq!(args, ["-h", "127.0.0.1", "-p", "6389"]);

        // Garnet tidak punya redis-cli sendiri: memakai milik Redis terinstall.
        config.defaults.insert("cache".into(), "garnet@2".into());
        let p = plan(&paths, &config, tmp.path(), "redis-cli").unwrap();
        assert_eq!(p.program, redis.join("redis-cli.exe"));
        assert_eq!(p.prefix_args[3].to_string_lossy(), "6402");
    }

    #[test]
    fn go_and_python_shims_use_project_version() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path());
        let go = paths.runtime_dir("go", "1.27.3");
        std::fs::create_dir_all(go.join("bin")).unwrap();
        std::fs::write(go.join("bin/go.exe"), b"").unwrap();
        let py = paths.runtime_dir("python", "3.13.18");
        std::fs::create_dir_all(&py).unwrap();
        std::fs::write(py.join("python.exe"), b"").unwrap();
        let project = tmp.path().join("app");
        std::fs::create_dir_all(&project).unwrap();
        std::fs::write(project.join("go.mod"), "module x\n\ngo 1.27\n").unwrap();
        std::fs::write(project.join(".python-version"), "3.13\n").unwrap();
        let config = Config::default();

        let p = plan(&paths, &config, &project, "go").unwrap();
        assert_eq!(p.program, go.join("bin/go.exe"));
        assert!(p.path_prepend.contains(&go.join("bin")));
        let p = plan(&paths, &config, &project, "pip").unwrap();
        assert_eq!(p.program, py.join("python.exe"));
        let args: Vec<_> = p.prefix_args.iter().map(|a| a.to_string_lossy()).collect();
        assert_eq!(args, ["-m", "pip"]);
        assert_eq!(p.resolved[0].1, "3.13.18");
    }

    #[test]
    fn mongosh_targets_mongodb_instance() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path());
        let mongosh = paths.runtime_dir("mongosh", "2.14.0");
        std::fs::create_dir_all(mongosh.join("bin")).unwrap();
        std::fs::write(mongosh.join("bin/mongosh.exe"), b"").unwrap();
        let mysql = paths.runtime_dir("mysql", "8.4.3");
        std::fs::create_dir_all(mysql.join("bin")).unwrap();
        std::fs::write(mysql.join("bin/mysql.exe"), b"").unwrap();
        let mut config = Config::default();
        config
            .defaults
            .insert("database".into(), "mongodb@8.0".into());

        let p = plan(&paths, &config, tmp.path(), "mongosh").unwrap();
        assert_eq!(p.program, mongosh.join("bin/mongosh.exe"));
        assert_eq!(
            p.prefix_args[0].to_string_lossy(),
            "mongodb://127.0.0.1:27080"
        );
        // Client SQL menolak instance MongoDB dengan pesan yang jelas.
        let err = plan(&paths, &config, tmp.path(), "mysql").unwrap_err();
        assert!(err.to_string().contains("mongodb@8.0"));

        config
            .defaults
            .insert("database".into(), "mysql@8.4".into());
        let err = plan(&paths, &config, tmp.path(), "mongosh").unwrap_err();
        assert!(err.to_string().contains("untuk MongoDB"));
    }
}
