mod commands;

use clap::{Parser, Subcommand};

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
}

fn main() {
    let cli = Cli::parse();
    init_logging(cli.verbose);

    let result = match cli.command {
        Command::Init => commands::init(),
        Command::Config => commands::config(),
        Command::Doctor => commands::doctor(),
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
