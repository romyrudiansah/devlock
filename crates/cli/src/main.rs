mod commands;
mod runtimes;
mod user_path;
mod versions;

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
        /// php, node, composer, nginx, mysql, mariadb
        kind: Kind,
        /// Tampilkan versi yang bisa diinstall
        #[arg(short, long)]
        available: bool,
        /// Dengan --available: tampilkan semua patch, bukan hanya terbaru per cabang
        #[arg(long, requires = "available")]
        all: bool,
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
    /// Pilih versi untuk project ini (.devlock.toml) atau global, mis. `use php 8.2`, `use database mysql@8.4`
    Use {
        /// php, node, composer, atau database
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
}

fn main() {
    let cli = Cli::parse();
    init_logging(cli.verbose);

    let result = match cli.command {
        Command::Init => commands::init(),
        Command::Config => commands::config(),
        Command::Doctor => commands::doctor(),
        Command::List {
            kind,
            available,
            all,
        } => runtimes::list(kind, available, all),
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
