pub mod cache;
pub mod export_raindrop;
pub mod models;

use clap::{Parser, Subcommand};

// #KeepArgsInSync
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Raindrop,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Raindrop => export_raindrop::export_raindrop(),
    }
}
