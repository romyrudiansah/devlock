mod commands;
mod db;
mod dns;
mod new;
mod php;
mod runtimes;
mod services;
mod ssl;
mod user_path;
mod versions;

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use devlock_core::Kind;

#[derive(Parser)]
#[command(
    name = "devlock",
    version,
    about = "Local development environment multi-versi untuk Windows"
)]
struct Cli {
    /// Tampilkan log lebih detail (-v, -vv)
    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    verbose: u8,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Siapkan struktur folder dan devlock.toml di root DevLock
    Init,
    /// Tampilkan lokasi dan isi konfigurasi global
    Config,
    /// Diagnosa lingkungan (root, PATH, port, dependency)
    Doctor,
    /// Daftar versi runtime terinstall (atau yang tersedia dengan --available)
    List {
        /// php, node, composer, nginx, apache, mysql, mariadb, postgresql, redis, garnet, adminer
        kind: Kind,
        /// Tampilkan versi yang bisa diinstall
        #[arg(short, long)]
        available: bool,
        /// Dengan --available: tampilkan semua patch, bukan hanya terbaru per cabang
        #[arg(long, requires = "available")]
        all: bool,
        /// Dengan --available: abaikan cache katalog (6 jam) dan ambil ulang dari sumber resmi
        #[arg(long, requires = "available")]
        refresh: bool,
    },
    /// Install runtime, mis. `devlock install php 8.3` atau `devlock install node lts`
    Install {
        kind: Kind,
        /// Versi lengkap, awalan (8.3), `latest`, atau `lts`
        #[arg(default_value = "latest")]
        version: String,
    },
    /// Hapus versi runtime yang terinstall (versi lengkap)
    Uninstall { kind: Kind, version: String },
    /// Pilih versi untuk project ini (.devlock.toml) atau global, mis. `use php 8.2`, `use database mysql@8.4`, `use cache redis@8`
    Use {
        /// php, node, composer, database, atau cache
        target: String,
        version: String,
        /// Simpan sebagai default global di devlock.toml
        #[arg(short, long)]
        global: bool,
    },
    /// Program & versi yang dipakai shim untuk sebuah perintah di folder ini
    Which { tool: String },
    /// Ringkasan versi php/node/composer/database di folder ini
    Current,
    /// Buat ulang shim (php.exe, node.exe, ...) di folder shims
    Shims,
    /// Tambah atau hapus folder shims dari PATH user
    Path {
        #[arg(value_parser = ["add", "remove"])]
        action: String,
    },
    /// Jalankan service (default semua): all, nginx, php, php@8.3, mysql@8.4, db, cache, dev, dev:<site>
    Start {
        #[arg(default_value = "all")]
        target: String,
        /// Jangan ubah file hosts
        #[arg(long)]
        no_hosts: bool,
    },
    /// Hentikan service; tanpa argumen menghentikan semua service dan daemon
    Stop { target: Option<String> },
    /// Restart service (default semua)
    Restart {
        #[arg(default_value = "all")]
        target: String,
    },
    /// Baca ulang konfigurasi & folder www (site baru, versi berubah)
    Reload,
    /// Status semua service
    Status,
    /// Tampilkan log service (nginx, php@8.3, mysql@8.4, dev:<site>, daemon)
    Logs {
        service: String,
        /// Jumlah baris terakhir
        #[arg(short = 'n', long, default_value_t = 50)]
        lines: usize,
        /// Ikuti baris baru
        #[arg(short, long)]
        follow: bool,
    },
    /// Kelola site di folder www
    Site {
        #[command(subcommand)]
        command: SiteCommand,
    },
    /// Aksi cepat database: list, create, drop, dump, restore (instance harus berjalan)
    Db {
        #[command(subcommand)]
        command: DbCommand,
        /// Instance, mis. `mysql@8.4` atau `postgresql@18`; default dari .devlock.toml / global
        #[arg(short, long, global = true)]
        instance: Option<String>,
    },
    /// Buat project baru di www dari template (Laravel, WordPress, Next.js, …)
    New {
        /// laravel, wordpress, symfony, codeigniter, next, nuxt, vite-react, vite-vue, static
        template: Option<String>,
        /// Nama project = nama folder & domain (`<nama>.test`)
        name: Option<String>,
        /// Versi PHP, mis. `8.3` (default dari devlock.toml)
        #[arg(long)]
        php: Option<String>,
        /// Versi Node, mis. `22`
        #[arg(long)]
        node: Option<String>,
        /// Instance database, mis. `mysql@8.4` / `postgresql@18`
        #[arg(long)]
        db: Option<String>,
        /// Tampilkan daftar template
        #[arg(long)]
        list: bool,
    },
    /// Alat PHP: ekstensi, Xdebug, dan php.ini per versi
    Php {
        #[command(subcommand)]
        command: PhpCommand,
        /// Versi PHP, mis. `8.3`; default versi milik folder kerja
        #[arg(long = "php", global = true)]
        version: Option<String>,
    },
    /// HTTPS lokal: CA DevLock dan sertifikat per site
    Ssl {
        #[command(subcommand)]
        command: SslCommand,
    },
    /// Samakan file hosts Windows dengan daftar site (minta UAC bila perlu)
    Hosts {
        /// Hapus seluruh blok DevLock dari file hosts (mis. sebelum uninstall)
        #[arg(long)]
        clear: bool,
        #[arg(long, hide = true)]
        elevated: bool,
        #[arg(long, hide = true)]
        root: Option<PathBuf>,
    },
    /// DNS wildcard *.<tld> lewat DNS lokal (tanpa menulis file hosts tiap project)
    Dns {
        #[command(subcommand)]
        command: Option<DnsCommand>,
        #[arg(long, hide = true, global = true)]
        root: Option<PathBuf>,
    },
    /// Proses latar yang mengawasi service (dijalankan otomatis oleh `start`)
    #[command(hide = true)]
    Daemon,
}

#[derive(Subcommand)]
enum DnsCommand {
    /// Status DNS wildcard, aturan NRPT, dan uji resolusi
    Status,
    /// Aktifkan: pasang aturan NRPT `.<tld>` → 127.0.0.1 (UAC sekali) dan jalankan service dns
    Enable,
    /// Nonaktifkan dan hapus aturan NRPT (UAC sekali)
    Disable,
    /// Server DNS (dijalankan daemon sebagai service `dns`)
    #[command(hide = true)]
    Serve,
    #[command(hide = true)]
    Install {
        #[arg(long)]
        tld: String,
    },
    #[command(hide = true)]
    Remove,
}

#[derive(Subcommand)]
enum PhpCommand {
    /// Ekstensi PHP: list, enable, disable
    Ext {
        #[command(subcommand)]
        command: ExtCommand,
    },
    /// Xdebug: status, install (unduh dari xdebug.org + aktifkan), on, off
    Xdebug {
        #[arg(value_parser = ["status", "install", "on", "off"], default_value = "status")]
        action: String,
    },
    /// php.ini: get <key>, set <key> <value>, path
    Ini {
        #[command(subcommand)]
        command: IniCommand,
    },
}

#[derive(Subcommand)]
enum ExtCommand {
    /// Semua ekstensi di ext\ beserta statusnya
    List,
    /// Aktifkan ekstensi, mis. `devlock php ext enable redis gmp`
    Enable { names: Vec<String> },
    /// Nonaktifkan ekstensi
    Disable { names: Vec<String> },
}

#[derive(Subcommand)]
enum IniCommand {
    /// Nilai aktif sebuah key, mis. `memory_limit`
    Get { key: String },
    /// Set nilai, mis. `devlock php ini set memory_limit 1G`
    Set { key: String, value: String },
    /// Lokasi php.ini
    Path,
}

#[derive(Subcommand)]
enum DbCommand {
    /// Daftar database (tanpa database sistem)
    List,
    /// Buat database UTF-8
    Create { name: String },
    /// Hapus database (wajib `--yes`)
    Drop {
        name: String,
        #[arg(long)]
        yes: bool,
    },
    /// Simpan database ke file SQL (default: data\backups\<instance>\<db>-<waktu>.sql)
    Dump {
        name: String,
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Jalankan file SQL ke database (dibuat bila belum ada)
    Restore { name: String, file: PathBuf },
}

#[derive(Subcommand)]
enum SslCommand {
    /// Status CA, kepercayaan Windows, dan site yang memakai HTTPS
    Status,
    /// Pasang CA DevLock ke store Root user (Windows meminta konfirmasi)
    Trust {
        /// Pasang ke store Root komputer (UAC) agar Firefox ikut percaya
        #[arg(long)]
        machine: bool,
    },
    /// Cabut CA DevLock dari store Root user
    Untrust {
        /// Cabut dari store Root komputer (UAC)
        #[arg(long)]
        machine: bool,
    },
    /// Aktifkan HTTPS untuk semua site ([https] enabled = true)
    Enable,
    /// Nonaktifkan HTTPS global
    Disable,
    /// Perbarui bundle CA (Mozilla + CA DevLock) dan pasang ke php.ini semua versi PHP
    Bundle,
}

#[derive(Subcommand)]
enum SiteCommand {
    /// Daftar site beserta domain, tipe, versi PHP, dan database
    List,
}

fn main() {
    let cli = Cli::parse();
    // Daemon menulis log ke file (stdout/stderr-nya diarahkan ke logs\daemon.log).
    let background = matches!(
        cli.command,
        Command::Daemon
            | Command::Dns {
                command: Some(DnsCommand::Serve),
                ..
            }
    );
    init_logging(if background {
        cli.verbose.max(1)
    } else {
        cli.verbose
    });

    let result = match cli.command {
        Command::Init => commands::init(),
        Command::Config => commands::config(),
        Command::Doctor => commands::doctor(),
        Command::List {
            kind,
            available,
            all,
            refresh,
        } => runtimes::list(kind, available, all, refresh),
        Command::Install { kind, version } => runtimes::install(kind, &version),
        Command::Uninstall { kind, version } => runtimes::uninstall(kind, &version),
        Command::Use {
            target,
            version,
            global,
        } => versions::use_version(&target, &version, global),
        Command::Which { tool } => versions::which(&tool),
        Command::Current => versions::current(),
        Command::Shims => versions::shims(),
        Command::Path { action } => commands::path(&action),
        Command::Start { target, no_hosts } => services::start(&target, !no_hosts),
        Command::Stop { target } => services::stop(target.as_deref()),
        Command::Restart { target } => services::restart(&target),
        Command::Reload => services::reload(),
        Command::Status => services::status(),
        Command::Logs {
            service,
            lines,
            follow,
        } => services::logs(&service, lines, follow),
        Command::Site {
            command: SiteCommand::List,
        } => services::site_list(),
        Command::Hosts {
            clear,
            elevated,
            root,
        } => {
            if let Some(root) = root {
                std::env::set_var(devlock_core::paths::HOME_ENV, root);
            }
            services::hosts(elevated, clear)
        }
        Command::Db { command, instance } => {
            let instance = instance.as_deref();
            match command {
                DbCommand::List => db::list(instance),
                DbCommand::Create { name } => db::create(&name, instance),
                DbCommand::Drop { name, yes } => db::drop(&name, instance, yes),
                DbCommand::Dump { name, output } => db::dump(&name, instance, output),
                DbCommand::Restore { name, file } => db::restore(&name, &file, instance),
            }
        }
        Command::New {
            template,
            name,
            php,
            node,
            db,
            list,
        } => {
            if list {
                new::list()
            } else {
                new::create(template.as_deref(), name.as_deref(), php, node, db)
            }
        }
        Command::Php { command, version } => {
            let v = version.as_deref();
            match command {
                PhpCommand::Ext { command } => match command {
                    ExtCommand::List => php::ext_list(v),
                    ExtCommand::Enable { names } => php::ext_set(&names, true, v),
                    ExtCommand::Disable { names } => php::ext_set(&names, false, v),
                },
                PhpCommand::Xdebug { action } => match action.as_str() {
                    "install" => php::xdebug_install(v),
                    "on" => php::xdebug_set(true, v),
                    "off" => php::xdebug_set(false, v),
                    _ => php::xdebug_status(v),
                },
                PhpCommand::Ini { command } => match command {
                    IniCommand::Get { key } => php::ini_get(&key, v),
                    IniCommand::Set { key, value } => php::ini_set(&key, &value, v),
                    IniCommand::Path => php::ini_path(v),
                },
            }
        }
        Command::Ssl { command } => match command {
            SslCommand::Status => ssl::status(),
            SslCommand::Trust { machine } => ssl::trust(machine),
            SslCommand::Untrust { machine } => ssl::untrust(machine),
            SslCommand::Enable => ssl::set_enabled(true),
            SslCommand::Disable => ssl::set_enabled(false),
            SslCommand::Bundle => ssl::bundle(),
        },
        Command::Dns { command, root } => {
            if let Some(root) = root {
                std::env::set_var(devlock_core::paths::HOME_ENV, root);
            }
            match command.unwrap_or(DnsCommand::Status) {
                DnsCommand::Status => dns::status(),
                DnsCommand::Enable => dns::enable(),
                DnsCommand::Disable => dns::disable(),
                DnsCommand::Serve => dns::serve(),
                DnsCommand::Install { tld } => dns::apply_elevated(Some(&tld)),
                DnsCommand::Remove => dns::apply_elevated(None),
            }
        }
        Command::Daemon => services::run_daemon(),
    };

    if let Err(err) = result {
        eprintln!("error: {err:#}");
        std::process::exit(1);
    }
}

fn init_logging(verbose: u8) {
    let level = match verbose {
        0 => "warn",
        1 => "info",
        _ => "debug",
    };
    let filter = tracing_subscriber::EnvFilter::try_from_env("DEVLOCK_LOG")
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(level));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_writer(std::io::stderr)
        .init();
}
