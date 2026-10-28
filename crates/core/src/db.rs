//! Aksi cepat database untuk instance lokal: daftar, buat, hapus, dump, dan restore.
//! Memakai client resmi (`mysql`/`mariadb`, `mysqldump`, `psql`, `pg_dump`) dari runtime
//! yang terinstall; instance harus sedang berjalan.

use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{bail, Context, Result};

use crate::config::Config;
use crate::paths::Paths;
use crate::resolve::DbInstance;
use crate::runtime::{self, Kind};

/// Database bawaan engine yang tidak ditampilkan / tidak boleh dihapus.
const SYSTEM_DATABASES: &[&str] = &[
    "information_schema",
    "mysql",
    "performance_schema",
    "sys",
    "postgres",
];

/// Nama database dibatasi `[A-Za-z_][A-Za-z0-9_]*` (maks. 63) agar aman disisipkan ke SQL.
pub fn validate_name(name: &str) -> Result<()> {
    let mut chars = name.chars();
    let valid = matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
        && name.len() <= 63;
    if !valid {
        bail!("nama database `{name}` tidak valid; pakai huruf, angka, dan `_` (diawali huruf)");
    }
    if SYSTEM_DATABASES.contains(&name.to_ascii_lowercase().as_str()) {
        bail!("`{name}` adalah database sistem");
    }
    Ok(())
}

/// Ubah nama site (boleh `-`) menjadi nama database yang valid, mis. `my-shop` → `my_shop`.
pub fn name_for_site(site: &str) -> String {
    let mut name: String = site
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    if name.starts_with(|c: char| c.is_ascii_digit()) {
        name.insert(0, '_');
    }
    name.truncate(63);
    name
}

/// Lokasi `bin\` instance dan port-nya.
struct Target {
    kind: Kind,
    bin: PathBuf,
    port: String,
}

fn target(paths: &Paths, config: &Config, instance: &DbInstance) -> Result<Target> {
    if !instance.is_sql() {
        bail!(
            "`devlock db` hanya untuk MySQL/MariaDB/PostgreSQL; untuk {} pakai `mongosh`",
            instance.name()
        );
    }
    let (_, dir) = runtime::require_installed(paths, instance.kind, &instance.line)?;
    Ok(Target {
        kind: instance.kind,
        bin: dir.join("bin"),
        port: instance.port(config).to_string(),
    })
}

impl Target {
    fn exe(&self, names: &[&str]) -> PathBuf {
        names
            .iter()
            .map(|n| self.bin.join(n))
            .find(|p| p.is_file())
            .unwrap_or_else(|| self.bin.join(names[0]))
    }

    fn postgres(&self) -> bool {
        self.kind == Kind::Postgresql
    }

    /// Client SQL interaktif, terhubung sebagai superuser lokal.
    fn sql_client(&self, database: Option<&str>) -> Command {
        if self.postgres() {
            let mut cmd = command(self.exe(&["psql.exe"]));
            cmd.args(["-h", "127.0.0.1", "-p", &self.port, "-U", "postgres"])
                .args(["-v", "ON_ERROR_STOP=1", "-X", "-q"])
                .args(["-d", database.unwrap_or("postgres")]);
            cmd
        } else {
            let mut cmd = command(self.exe(&["mysql.exe", "mariadb.exe"]));
            cmd.args(["-h", "127.0.0.1", "-P", &self.port, "-u", "root"]);
            if let Some(db) = database {
                cmd.arg(db);
            }
            cmd
        }
    }

    /// Jalankan satu perintah SQL dan kembalikan baris hasilnya (tanpa header).
    fn query(&self, sql: &str) -> Result<Vec<String>> {
        let mut cmd = self.sql_client(None);
        if self.postgres() {
            cmd.args(["-A", "-t", "-c", sql]);
        } else {
            cmd.args(["--batch", "--skip-column-names", "-e", sql]);
        }
        let out = run(cmd)?;
        Ok(out
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(str::to_string)
            .collect())
    }
}

fn command(program: PathBuf) -> Command {
    let mut cmd = Command::new(program);
    cmd.stdin(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(crate::process::CREATE_NO_WINDOW);
    }
    cmd
}

fn run(mut cmd: Command) -> Result<String> {
    let program = cmd.get_program().to_string_lossy().into_owned();
    let output = cmd
        .output()
        .with_context(|| format!("gagal menjalankan {program}"))?;
    if !output.status.success() {
        bail!("{}", client_error(&String::from_utf8_lossy(&output.stderr)));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Baris utama stderr client (psql menaruh petunjuk di baris berikutnya), ditambah saran
/// bila instance tidak bisa dihubungi.
fn client_error(stderr: &str) -> String {
    let first = stderr
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("perintah database gagal");
    let refused = ["Connection refused", "10061", "Can't connect"]
        .iter()
        .any(|needle| stderr.contains(needle));
    if refused {
        format!("{first}\ninstance belum berjalan? jalankan `devlock start <instance>` dulu")
    } else {
        first.to_string()
    }
}

/// Database milik pengguna (tanpa database sistem), urut nama.
pub fn list(paths: &Paths, config: &Config, instance: &DbInstance) -> Result<Vec<String>> {
    let t = target(paths, config, instance)?;
    let sql = if t.postgres() {
        "SELECT datname FROM pg_database WHERE NOT datistemplate ORDER BY datname"
    } else {
        "SHOW DATABASES"
    };
    let mut names: Vec<String> = t
        .query(sql)?
        .into_iter()
        .filter(|n| !SYSTEM_DATABASES.contains(&n.to_ascii_lowercase().as_str()))
        .collect();
    names.sort();
    Ok(names)
}

/// Buat database (UTF-8). Mengembalikan `false` bila sudah ada.
pub fn create(paths: &Paths, config: &Config, instance: &DbInstance, name: &str) -> Result<bool> {
    validate_name(name)?;
    if list(paths, config, instance)?.iter().any(|n| n == name) {
        return Ok(false);
    }
    let t = target(paths, config, instance)?;
    let sql = if t.postgres() {
        format!("CREATE DATABASE \"{name}\" ENCODING 'UTF8' TEMPLATE template0")
    } else {
        format!("CREATE DATABASE `{name}` CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci")
    };
    t.query(&sql)?;
    Ok(true)
}

/// Hapus database. Mengembalikan `false` bila memang tidak ada.
pub fn drop(paths: &Paths, config: &Config, instance: &DbInstance, name: &str) -> Result<bool> {
    validate_name(name)?;
    if !list(paths, config, instance)?.iter().any(|n| n == name) {
        return Ok(false);
    }
    let t = target(paths, config, instance)?;
    let sql = if t.postgres() {
        // FORCE (PostgreSQL 13+) memutus koneksi yang masih terbuka.
        format!("DROP DATABASE IF EXISTS \"{name}\" WITH (FORCE)")
    } else {
        format!("DROP DATABASE IF EXISTS `{name}`")
    };
    t.query(&sql)?;
    Ok(true)
}

/// Lokasi default file dump: `data\backups\<instance>\<db>-<YYYYmmdd-HHMMSS>.sql`.
pub fn default_dump_path(paths: &Paths, instance: &DbInstance, name: &str) -> PathBuf {
    let stamp = time::OffsetDateTime::now_local()
        .unwrap_or_else(|_| time::OffsetDateTime::now_utc())
        .format(time::macros::format_description!(
            "[year][month][day]-[hour][minute][second]"
        ))
        .unwrap_or_default();
    paths
        .data()
        .join("backups")
        .join(instance.name().replace('@', "-"))
        .join(format!("{name}-{stamp}.sql"))
}

/// Dump satu database ke file SQL biasa.
pub fn dump(
    paths: &Paths,
    config: &Config,
    instance: &DbInstance,
    name: &str,
    out: &Path,
) -> Result<()> {
    validate_name(name)?;
    if let Some(dir) = out.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let t = target(paths, config, instance)?;
    let mut cmd = if t.postgres() {
        let mut cmd = command(t.exe(&["pg_dump.exe"]));
        cmd.args(["-h", "127.0.0.1", "-p", &t.port, "-U", "postgres"])
            .args(["--no-owner", "-f"])
            .arg(out)
            .arg(name);
        cmd
    } else {
        let mut cmd = command(t.exe(&["mysqldump.exe", "mariadb-dump.exe"]));
        cmd.args(["-h", "127.0.0.1", "-P", &t.port, "-u", "root"])
            .args(["--single-transaction", "--routines", "--triggers"])
            .arg(format!("--result-file={}", out.display()))
            .arg(name);
        cmd
    };
    cmd.stdout(Stdio::null());
    run(cmd).map(|_| ())
}

/// Jalankan file SQL ke database (database dibuat dulu bila belum ada).
pub fn restore(
    paths: &Paths,
    config: &Config,
    instance: &DbInstance,
    name: &str,
    input: &Path,
) -> Result<()> {
    validate_name(name)?;
    let file = File::open(input).with_context(|| format!("gagal membuka {}", input.display()))?;
    create(paths, config, instance, name)?;
    let t = target(paths, config, instance)?;
    let mut cmd = t.sql_client(Some(name));
    cmd.stdin(Stdio::from(file));
    run(cmd).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn database_names() {
        assert!(validate_name("toko_online").is_ok());
        assert!(validate_name("_tmp1").is_ok());
        assert!(validate_name("1shop").is_err());
        assert!(validate_name("shop-db").is_err());
        assert!(validate_name("x`; DROP DATABASE mysql; --").is_err());
        assert!(validate_name("mysql").is_err());
        assert!(validate_name("Postgres").is_err());
        assert!(validate_name(&"a".repeat(64)).is_err());
    }

    #[test]
    fn site_names_become_database_names() {
        assert_eq!(name_for_site("my-shop"), "my_shop");
        assert_eq!(name_for_site("2024app"), "_2024app");
        assert!(validate_name(&name_for_site("blog.v2")).is_ok());
    }

    #[test]
    fn client_errors_are_short_with_hint() {
        let psql = "psql: error: connection to server at \"127.0.0.1\", port 5418 failed: Connection refused (0x0000274D/10061)
	Is the server running on that host and accepting TCP/IP connections?
";
        let msg = client_error(psql);
        assert!(msg.starts_with("psql: error: connection to server"));
        assert!(msg.contains("devlock start"));
        let mysql = "ERROR 1007 (HY000) at line 1: Can't create database 'toko'; database exists
";
        assert_eq!(client_error(mysql), mysql.trim());
    }

    #[test]
    fn dump_path_layout() {
        let paths = Paths::new("D:/DevLock");
        let instance = DbInstance::parse("postgresql@18").unwrap();
        let p = default_dump_path(&paths, &instance, "toko");
        assert!(p.starts_with(Path::new("D:/DevLock/data/backups/postgresql-18")));
        let file = p.file_name().unwrap().to_string_lossy().into_owned();
        assert!(
            file.starts_with("toko-") && file.ends_with(".sql"),
            "{file}"
        );
    }
}
