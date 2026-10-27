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
    /// Client database di `bin\` instance milik project. `Kind` pada tabel menyatakan
    /// keluarganya: `Mysql` (MySQL/MariaDB) atau `Postgresql`.
    DbClient(&'static [&'static str]),
}

const TOOLS: &[(&str, Kind, Launch)] = &[
    ("php", Kind::Php, Launch::Exe(&["php.exe"])),
    ("php-cgi", Kind::Php, Launch::Exe(&["php-cgi.exe"])),
    ("composer", Kind::Composer, Launch::Composer),
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
            if wants_postgres != (instance.kind == Kind::Postgresql) {
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
}
