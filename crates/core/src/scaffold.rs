//! Quick create project: kerangka project siap pakai di `www\<nama>` lengkap dengan
//! `.devlock.toml`, database, dan konfigurasi koneksi (`.env` / `wp-config.php`).
//!
//! - PHP (Laravel, Symfony, CodeIgniter 4): `composer create-project --no-interaction`
//!   memakai PHP pilihan lewat shim DevLock.
//! - WordPress: zip resmi dari wordpress.org (diverifikasi SHA1).
//! - Node (Next.js, Nuxt, Vite React/Vue): kerangka minimal ditulis DevLock lalu
//!   `npm install <paket>@latest`, tanpa prompt interaktif generator pihak ketiga.
//! - Static: `index.html`.

use std::fmt::Write as _;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::str::FromStr;
use std::sync::mpsc;

use anyhow::{bail, Context, Result};
use serde::Serialize;

use crate::config::Config;
use crate::daemon::{self, Request};
use crate::paths::Paths;
use crate::resolve::{self, DbInstance};
use crate::runtime::{self, Kind};
use crate::version::Version;
use crate::{db, http, launcher, sites};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Template {
    Laravel,
    Symfony,
    Codeigniter,
    Wordpress,
    Next,
    Nuxt,
    ViteReact,
    ViteVue,
    Static,
}

impl Template {
    pub const ALL: [Template; 9] = [
        Template::Laravel,
        Template::Wordpress,
        Template::Symfony,
        Template::Codeigniter,
        Template::Next,
        Template::Nuxt,
        Template::ViteReact,
        Template::ViteVue,
        Template::Static,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Template::Laravel => "laravel",
            Template::Symfony => "symfony",
            Template::Codeigniter => "codeigniter",
            Template::Wordpress => "wordpress",
            Template::Next => "next",
            Template::Nuxt => "nuxt",
            Template::ViteReact => "vite-react",
            Template::ViteVue => "vite-vue",
            Template::Static => "static",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Template::Laravel => "Laravel",
            Template::Symfony => "Symfony",
            Template::Codeigniter => "CodeIgniter 4",
            Template::Wordpress => "WordPress",
            Template::Next => "Next.js",
            Template::Nuxt => "Nuxt",
            Template::ViteReact => "Vite + React",
            Template::ViteVue => "Vite + Vue",
            Template::Static => "HTML statis",
        }
    }

    pub fn uses_php(self) -> bool {
        matches!(
            self,
            Template::Laravel | Template::Symfony | Template::Codeigniter | Template::Wordpress
        )
    }

    pub fn uses_node(self) -> bool {
        matches!(
            self,
            Template::Next | Template::Nuxt | Template::ViteReact | Template::ViteVue
        )
    }

    /// Template yang wajib punya database (dibuat otomatis).
    pub fn needs_database(self) -> bool {
        matches!(self, Template::Laravel | Template::Wordpress)
    }

    /// Paket composer untuk `create-project`.
    fn composer_package(self) -> Option<&'static str> {
        match self {
            Template::Laravel => Some("laravel/laravel"),
            Template::Symfony => Some("symfony/skeleton"),
            Template::Codeigniter => Some("codeigniter4/appstarter"),
            _ => None,
        }
    }
}

impl FromStr for Template {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        let s = s.to_ascii_lowercase();
        Template::ALL
            .into_iter()
            .find(|t| t.id() == s || (s == "ci4" && *t == Template::Codeigniter))
            .with_context(|| {
                format!(
                    "template `{s}` tidak dikenal (pilihan: {})",
                    Template::ALL.map(Template::id).join(", ")
                )
            })
    }
}

#[derive(Debug, Clone, Default)]
pub struct Options {
    pub name: String,
    /// Versi PHP (spec), default dari `[defaults]` atau versi tertinggi.
    pub php: Option<String>,
    pub node: Option<String>,
    /// Instance database, mis. `mysql@8.4`; default dari `[defaults] database`.
    pub database: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Created {
    pub dir: PathBuf,
    pub url: String,
    pub database: Option<String>,
    /// Langkah lanjutan yang disarankan ke pengguna.
    pub notes: Vec<String>,
}

/// Buat project baru. `log` menerima setiap baris output (composer/npm) dan pesan progres.
pub fn create(
    paths: &Paths,
    config: &Config,
    template: Template,
    opts: &Options,
    log: &mut dyn FnMut(&str),
) -> Result<Created> {
    let name = opts.name.trim().to_lowercase();
    if !sites::is_valid_label(&name) {
        bail!("nama `{name}` tidak valid untuk domain; pakai huruf kecil, angka, dan `-`");
    }
    let www = config.www_dir(paths);
    std::fs::create_dir_all(&www)?;
    let dir = www.join(&name);
    if dir.exists() {
        bail!("folder {} sudah ada", dir.display());
    }

    let php = template
        .uses_php()
        .then(|| pick_version(paths, config, Kind::Php, opts.php.as_deref()))
        .transpose()?;
    let node = template
        .uses_node()
        .then(|| pick_version(paths, config, Kind::Node, opts.node.as_deref()))
        .transpose()?;
    let instance = match (&opts.database, template.needs_database()) {
        (Some(db), _) => Some(DbInstance::parse(db)?),
        (None, true) => match config.defaults.get("database") {
            Some(db) => Some(DbInstance::parse(db)?),
            None => bail!(
                "{} butuh database; pilih instance (mis. `--db mysql@8.4`) atau set default global",
                template.label()
            ),
        },
        (None, false) => None,
    };
    if template == Template::Wordpress
        && instance
            .as_ref()
            .is_some_and(|i| i.kind == Kind::Postgresql)
    {
        bail!("WordPress membutuhkan MySQL atau MariaDB");
    }
    if let Some(instance) = &instance {
        runtime::require_installed(paths, instance.kind, &instance.line)?;
    }

    log(&format!(
        "Membuat {} `{name}` di {}",
        template.label(),
        dir.display()
    ));
    let result = generate(
        paths,
        template,
        &name,
        &www,
        &dir,
        php.as_deref(),
        node.as_deref(),
        log,
    );
    if let Err(err) = result {
        // Jangan tinggalkan folder setengah jadi yang akan dianggap site.
        let _ = std::fs::remove_dir_all(&dir);
        return Err(err);
    }

    // .devlock.toml: versi yang dipilih + database + dev server.
    let mut toml = String::new();
    if let Some(php) = &php {
        let _ = writeln!(toml, "php = \"{}\"", major_minor(php));
    }
    if let Some(node) = &node {
        let _ = writeln!(toml, "node = \"{}\"", major(node));
    }
    if let Some(instance) = &instance {
        let _ = writeln!(toml, "database = \"{}\"", instance.name());
    }
    if template.uses_node() {
        let port = free_dev_port(paths, config)?;
        let (dev, _) = dev_command(template, port);
        let _ = writeln!(toml, "dev = \"{dev}\"\nproxy_port = {port}");
    }
    std::fs::write(dir.join(crate::project::PROJECT_FILE), toml)?;

    let mut notes = Vec::new();
    let database = match &instance {
        Some(instance) => {
            let db_name = db::name_for_site(&name);
            log(&format!(
                "Menyiapkan database `{db_name}` di {}",
                instance.name()
            ));
            launcher::ensure_daemon(paths, config)?;
            daemon::send(
                config,
                &Request::Start {
                    target: instance.name(),
                },
            )?;
            db::create(paths, config, instance, &db_name)?;
            Some((instance.clone(), db_name))
        }
        None => None,
    };

    let site_url = sites::url_for(
        &format!("{name}.{}", config.tld),
        config.https.enabled,
        config,
    );
    match template {
        Template::Laravel => {
            if let Some((instance, db_name)) = &database {
                configure_laravel(&dir, instance, db_name, config, &site_url)?;
                notes.push(
                    "Jalankan `php artisan migrate` di folder project untuk membuat tabel.".into(),
                );
            }
        }
        Template::Wordpress => {
            if let Some((instance, db_name)) = &database {
                configure_wordpress(&dir, instance, db_name, config)?;
                notes.push(format!(
                    "Buka {site_url} untuk menyelesaikan instalasi WordPress."
                ));
            }
        }
        _ => {}
    }
    if template.uses_node() {
        notes.push(
            "Dev server dijalankan DevLock (`devlock start dev`) dan diproxy ke domain project."
                .into(),
        );
    }
    notes.push(
        "Jalankan `devlock hosts` (sekali, perlu UAC) agar domain baru bisa dibuka di browser."
            .into(),
    );

    Ok(Created {
        dir,
        url: site_url,
        database: database.map(|(i, n)| format!("{n} @ {}", i.name())),
        notes,
    })
}

fn pick_version(paths: &Paths, config: &Config, kind: Kind, spec: Option<&str>) -> Result<String> {
    let spec = spec
        .map(str::to_string)
        .or_else(|| config.defaults.get(kind.as_str()).cloned())
        .unwrap_or_else(|| "latest".into());
    Ok(runtime::require_installed(paths, kind, &spec)?.0)
}

fn major_minor(version: &str) -> String {
    Version::parse(version).map_or(version.to_string(), |v| v.major_minor())
}

fn major(version: &str) -> String {
    Version::parse(version).map_or(version.to_string(), |v| v.major().to_string())
}

#[allow(clippy::too_many_arguments)]
fn generate(
    paths: &Paths,
    template: Template,
    name: &str,
    www: &Path,
    dir: &Path,
    php: Option<&str>,
    node: Option<&str>,
    log: &mut dyn FnMut(&str),
) -> Result<()> {
    if let Some(package) = template.composer_package() {
        let php = php.context("versi PHP belum dipilih")?;
        let mut cmd = shim(paths, "composer");
        cmd.args([
            "create-project",
            package,
            name,
            "--no-interaction",
            "--prefer-dist",
        ])
        .current_dir(www)
        .env(resolve::env_var(Kind::Php), php);
        return run(cmd, log);
    }
    match template {
        Template::Wordpress => wordpress(paths, dir, log),
        Template::Static => {
            std::fs::create_dir_all(dir)?;
            std::fs::write(dir.join("index.html"), static_page(name))?;
            Ok(())
        }
        _ => {
            let node = node.context("versi Node belum dipilih")?;
            std::fs::create_dir_all(dir)?;
            for (file, content) in node_files(template, name) {
                let path = dir.join(file);
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(path, content)?;
            }
            for (dev, packages) in node_packages(template) {
                let mut cmd = shim(paths, "npm");
                cmd.arg("install").args(packages).current_dir(dir);
                if dev {
                    cmd.arg("--save-dev");
                }
                cmd.env(resolve::env_var(Kind::Node), node);
                run(cmd, log)?;
            }
            Ok(())
        }
    }
}

fn shim(paths: &Paths, tool: &str) -> Command {
    let mut cmd = Command::new(paths.shims().join(format!("{tool}.exe")));
    cmd.stdin(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(crate::process::CREATE_NO_WINDOW);
    }
    cmd
}

/// Jalankan perintah, teruskan stdout & stderr baris demi baris ke `log`.
fn run(mut cmd: Command, log: &mut dyn FnMut(&str)) -> Result<()> {
    let program = cmd.get_program().to_string_lossy().into_owned();
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd
        .spawn()
        .with_context(|| format!("gagal menjalankan {program} (shim DevLock sudah dibuat?)"))?;
    let (tx, rx) = mpsc::channel::<String>();
    let mut readers = Vec::new();
    if let Some(out) = child.stdout.take() {
        let tx = tx.clone();
        readers.push(std::thread::spawn(move || {
            for line in BufReader::new(out).lines().map_while(|l| l.ok()) {
                let _ = tx.send(line);
            }
        }));
    }
    if let Some(err) = child.stderr.take() {
        let tx = tx.clone();
        readers.push(std::thread::spawn(move || {
            for line in BufReader::new(err).lines().map_while(|l| l.ok()) {
                let _ = tx.send(line);
            }
        }));
    }
    drop(tx);
    let mut last = Vec::new();
    for line in rx {
        log(&line);
        last.push(line);
        if last.len() > 5 {
            last.remove(0);
        }
    }
    for reader in readers {
        let _ = reader.join();
    }
    let status = child.wait()?;
    if !status.success() {
        bail!("{program} gagal ({status}): {}", last.join(" | "));
    }
    Ok(())
}

/// Port dev server berikutnya yang belum dipakai site lain (mulai 3001).
fn free_dev_port(paths: &Paths, config: &Config) -> Result<u16> {
    let used: Vec<u16> = sites::discover(paths, config)?
        .iter()
        .filter_map(|s| match s.kind {
            sites::SiteKind::Proxy { port } => Some(port),
            _ => None,
        })
        .collect();
    (3001..4000)
        .find(|p| !used.contains(p) && std::net::TcpListener::bind(("127.0.0.1", *p)).is_ok())
        .context("tidak ada port dev server yang bebas di 3001-3999")
}

/// Perintah dev server dan apakah ia membaca env `PORT`.
fn dev_command(template: Template, port: u16) -> (String, bool) {
    match template {
        // Next.js & Nuxt membaca env PORT yang diberikan DevLock.
        Template::Next | Template::Nuxt => ("npm run dev".into(), true),
        _ => (format!("npm run dev -- --port {port} --strictPort"), false),
    }
}

/// Paket npm per template: (`--save-dev`?, daftar paket).
fn node_packages(template: Template) -> Vec<(bool, Vec<&'static str>)> {
    match template {
        Template::Next => vec![(
            false,
            vec!["next@latest", "react@latest", "react-dom@latest"],
        )],
        Template::Nuxt => vec![(
            false,
            vec!["nuxt@latest", "vue@latest", "vue-router@latest"],
        )],
        Template::ViteReact => vec![
            (false, vec!["react@latest", "react-dom@latest"]),
            (true, vec!["vite@latest", "@vitejs/plugin-react@latest"]),
        ],
        Template::ViteVue => vec![
            (false, vec!["vue@latest"]),
            (true, vec!["vite@latest", "@vitejs/plugin-vue@latest"]),
        ],
        _ => vec![],
    }
}

/// Kerangka minimal project Node.
fn node_files(template: Template, name: &str) -> Vec<(&'static str, String)> {
    let package = |scripts: &str| {
        format!(
            "{{\n  \"name\": \"{name}\",\n  \"private\": true,\n  \"type\": \"module\",\n  \"scripts\": {{\n{scripts}\n  }}\n}}\n"
        )
    };
    match template {
        Template::Next => vec![
            (
                "package.json",
                package("    \"dev\": \"next dev\",\n    \"build\": \"next build\",\n    \"start\": \"next start\""),
            ),
            (
                "app/layout.jsx",
                format!("export const metadata = {{ title: \"{name}\" }};\n\nexport default function RootLayout({{ children }}) {{\n  return (\n    <html lang=\"id\">\n      <body>{{children}}</body>\n    </html>\n  );\n}}\n"),
            ),
            (
                "app/page.jsx",
                format!("export default function Home() {{\n  return <h1>{name} — Next.js di DevLock</h1>;\n}}\n"),
            ),
        ],
        Template::Nuxt => vec![
            (
                "package.json",
                package("    \"dev\": \"nuxt dev\",\n    \"build\": \"nuxt build\",\n    \"preview\": \"nuxt preview\""),
            ),
            ("nuxt.config.ts", "export default defineNuxtConfig({});\n".into()),
            (
                "app.vue",
                format!("<template>\n  <h1>{name} — Nuxt di DevLock</h1>\n</template>\n"),
            ),
        ],
        Template::ViteReact => vec![
            (
                "package.json",
                package("    \"dev\": \"vite\",\n    \"build\": \"vite build\",\n    \"preview\": \"vite preview\""),
            ),
            (
                "vite.config.js",
                "import { defineConfig } from \"vite\";\nimport react from \"@vitejs/plugin-react\";\n\nexport default defineConfig({ plugins: [react()] });\n".into(),
            ),
            ("index.html", vite_index(name, "/src/main.jsx")),
            (
                "src/main.jsx",
                format!("import {{ createRoot }} from \"react-dom/client\";\n\ncreateRoot(document.getElementById(\"app\")).render(<h1>{name} — Vite + React di DevLock</h1>);\n"),
            ),
        ],
        Template::ViteVue => vec![
            (
                "package.json",
                package("    \"dev\": \"vite\",\n    \"build\": \"vite build\",\n    \"preview\": \"vite preview\""),
            ),
            (
                "vite.config.js",
                "import { defineConfig } from \"vite\";\nimport vue from \"@vitejs/plugin-vue\";\n\nexport default defineConfig({ plugins: [vue()] });\n".into(),
            ),
            ("index.html", vite_index(name, "/src/main.js")),
            (
                "src/main.js",
                "import { createApp } from \"vue\";\nimport App from \"./App.vue\";\n\ncreateApp(App).mount(\"#app\");\n".into(),
            ),
            (
                "src/App.vue",
                format!("<template>\n  <h1>{name} — Vite + Vue di DevLock</h1>\n</template>\n"),
            ),
        ],
        _ => vec![],
    }
}

fn vite_index(name: &str, entry: &str) -> String {
    format!("<!doctype html>\n<html lang=\"id\">\n  <head>\n    <meta charset=\"UTF-8\" />\n    <title>{name}</title>\n  </head>\n  <body>\n    <div id=\"app\"></div>\n    <script type=\"module\" src=\"{entry}\"></script>\n  </body>\n</html>\n")
}

fn static_page(name: &str) -> String {
    format!("<!doctype html>\n<html lang=\"id\">\n  <head>\n    <meta charset=\"UTF-8\" />\n    <title>{name}</title>\n  </head>\n  <body>\n    <h1>{name}</h1>\n    <p>Situs statis di DevLock. Ubah file ini di folder project.</p>\n  </body>\n</html>\n")
}

// --- WordPress ----------------------------------------------------------------

const WP_VERSION_API: &str = "https://api.wordpress.org/core/version-check/1.7/";

fn wordpress(paths: &Paths, dir: &Path, log: &mut dyn FnMut(&str)) -> Result<()> {
    #[derive(serde::Deserialize)]
    struct Offers {
        offers: Vec<Offer>,
    }
    #[derive(serde::Deserialize)]
    struct Offer {
        current: String,
        packages: Packages,
    }
    #[derive(serde::Deserialize)]
    struct Packages {
        full: String,
    }
    let offers: Offers = http::get_json(WP_VERSION_API)?;
    let offer = offers
        .offers
        .into_iter()
        .next()
        .context("API WordPress kosong")?;
    log(&format!("Mengunduh WordPress {} …", offer.current));
    std::fs::create_dir_all(paths.downloads())?;
    let zip = paths
        .downloads()
        .join(format!("wordpress-{}.zip", offer.current));
    http::download(&offer.packages.full, &zip, &mut |_, _| {})?;
    let expected = http::get_string(&format!("{}.sha1", offer.packages.full))?;
    let expected = expected
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    if crate::install::sha1_file(&zip)? != expected {
        let _ = std::fs::remove_file(&zip);
        bail!("checksum SHA1 WordPress tidak cocok");
    }
    log("Mengekstrak WordPress …");
    let staging = dir.with_extension("tmp");
    let _ = std::fs::remove_dir_all(&staging);
    crate::install::extract_zip_to(&zip, &staging)?;
    // Zip resmi berisi folder `wordpress\`.
    std::fs::rename(staging.join("wordpress"), dir)
        .with_context(|| format!("gagal memindahkan ke {}", dir.display()))?;
    let _ = std::fs::remove_dir_all(&staging);
    Ok(())
}

fn configure_wordpress(
    dir: &Path,
    instance: &DbInstance,
    db_name: &str,
    config: &Config,
) -> Result<()> {
    let sample = std::fs::read_to_string(dir.join("wp-config-sample.php"))
        .context("wp-config-sample.php tidak ditemukan")?;
    let host = format!("127.0.0.1:{}", instance.port(config));
    std::fs::write(
        dir.join("wp-config.php"),
        wordpress_config(&sample, db_name, &host),
    )?;
    Ok(())
}

/// Isi `wp-config-sample.php`: nama database, user `root` tanpa password, host, dan salt acak.
pub fn wordpress_config(sample: &str, db_name: &str, host: &str) -> String {
    let mut text = sample
        .replacen("database_name_here", db_name, 1)
        .replacen("username_here", "root", 1)
        .replacen("password_here", "", 1)
        .replacen("'localhost'", &format!("'{host}'"), 1);
    while text.contains("put your unique phrase here") {
        text = text.replacen("put your unique phrase here", &random_salt(), 1);
    }
    text
}

/// 64 karakter acak dari generator OS (untuk AUTH_KEY, SALT, dll.).
fn random_salt() -> String {
    const CHARS: &[u8] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789!@#$%^&*()-_[]{}<>~+=,.;:/?|";
    let mut bytes = [0u8; 64];
    getrandom::fill(&mut bytes).expect("generator acak OS tidak tersedia");
    bytes
        .iter()
        .map(|b| CHARS[*b as usize % CHARS.len()] as char)
        .collect()
}

// --- Laravel ------------------------------------------------------------------

fn configure_laravel(
    dir: &Path,
    instance: &DbInstance,
    db_name: &str,
    config: &Config,
    url: &str,
) -> Result<()> {
    let env = dir.join(".env");
    let text = std::fs::read_to_string(&env).context(".env Laravel tidak ditemukan")?;
    std::fs::write(&env, laravel_env(&text, instance, db_name, config, url))?;
    Ok(())
}

/// Isi koneksi database & APP_URL di `.env` Laravel (baris `# DB_HOST=` diaktifkan).
pub fn laravel_env(
    text: &str,
    instance: &DbInstance,
    db_name: &str,
    config: &Config,
    url: &str,
) -> String {
    let (connection, user) = match instance.kind {
        Kind::Postgresql => ("pgsql", "postgres"),
        Kind::Mariadb => ("mariadb", "root"),
        _ => ("mysql", "root"),
    };
    let port = instance.port(config).to_string();
    let values = [
        ("APP_URL", url),
        ("DB_CONNECTION", connection),
        ("DB_HOST", "127.0.0.1"),
        ("DB_PORT", port.as_str()),
        ("DB_DATABASE", db_name),
        ("DB_USERNAME", user),
        ("DB_PASSWORD", ""),
    ];
    let mut out = text.to_string();
    for (key, value) in values {
        out = set_env(&out, key, value);
    }
    out
}

/// Set `KEY=value` di isi `.env`: baris aktif atau `# KEY=` pertama diganti, selain itu ditambah.
pub fn set_env(text: &str, key: &str, value: &str) -> String {
    let wanted = format!("{key}={value}");
    let mut done = false;
    let mut lines: Vec<String> = text
        .lines()
        .map(|line| {
            let bare = line.trim().trim_start_matches('#').trim_start();
            if !done && bare.strip_prefix(key).is_some_and(|r| r.starts_with('=')) {
                done = true;
                wanted.clone()
            } else {
                line.to_string()
            }
        })
        .collect();
    if !done {
        lines.push(wanted);
    }
    let mut out = lines.join("\n");
    out.push('\n');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_templates() {
        assert_eq!("Laravel".parse::<Template>().unwrap(), Template::Laravel);
        assert_eq!("vite-vue".parse::<Template>().unwrap(), Template::ViteVue);
        assert_eq!("ci4".parse::<Template>().unwrap(), Template::Codeigniter);
        assert!("rails".parse::<Template>().is_err());
        assert!(Template::Laravel.needs_database() && !Template::Symfony.needs_database());
    }

    #[test]
    fn laravel_env_uses_instance() {
        let env = "APP_URL=http://localhost\nDB_CONNECTION=sqlite\n# DB_HOST=127.0.0.1\n# DB_PORT=3306\n# DB_DATABASE=laravel\n# DB_USERNAME=root\n# DB_PASSWORD=\n";
        let pg = DbInstance::parse("postgresql@18").unwrap();
        let out = laravel_env(env, &pg, "toko", &Config::default(), "http://toko.test");
        assert!(out.contains("APP_URL=http://toko.test\n"));
        assert!(out.contains("DB_CONNECTION=pgsql\n"));
        assert!(out.contains("\nDB_HOST=127.0.0.1\n"));
        assert!(out.contains("\nDB_PORT=5418\n"));
        assert!(out.contains("\nDB_DATABASE=toko\n"));
        assert!(out.contains("\nDB_USERNAME=postgres\n"));
        assert!(!out.contains("# DB_"));
        let maria = DbInstance::parse("mariadb@11.4").unwrap();
        let out = laravel_env(env, &maria, "toko", &Config::default(), "http://toko.test");
        assert!(out.contains("DB_CONNECTION=mariadb\n") && out.contains("DB_PORT=3424\n"));
    }

    #[test]
    fn wordpress_config_is_filled() {
        let sample = "define( 'DB_NAME', 'database_name_here' );\ndefine( 'DB_USER', 'username_here' );\ndefine( 'DB_PASSWORD', 'password_here' );\ndefine( 'DB_HOST', 'localhost' );\ndefine( 'AUTH_KEY', 'put your unique phrase here' );\ndefine( 'NONCE_SALT', 'put your unique phrase here' );\n";
        let out = wordpress_config(sample, "blog", "127.0.0.1:3384");
        assert!(out.contains("'DB_NAME', 'blog'"));
        assert!(out.contains("'DB_USER', 'root'"));
        assert!(out.contains("'DB_PASSWORD', ''"));
        assert!(out.contains("'DB_HOST', '127.0.0.1:3384'"));
        assert!(!out.contains("put your unique phrase here"));
        let salts: Vec<&str> = out
            .lines()
            .filter(|l| l.contains("_KEY") || l.contains("_SALT"))
            .collect();
        assert_ne!(salts[0], salts[1]);
    }

    #[test]
    fn node_skeletons() {
        let files = node_files(Template::ViteReact, "dash");
        let package = &files.iter().find(|(f, _)| *f == "package.json").unwrap().1;
        assert!(package.contains("\"name\": \"dash\"") && package.contains("\"dev\": \"vite\""));
        assert_eq!(
            dev_command(Template::ViteReact, 3005).0,
            "npm run dev -- --port 3005 --strictPort"
        );
        assert_eq!(dev_command(Template::Next, 3005).0, "npm run dev");
        assert!(node_files(Template::Static, "x").is_empty());
    }
}
