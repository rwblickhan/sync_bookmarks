mod cache;
mod cli;
mod export_raindrop;
mod models;

use clap::Parser;
use cli::{Cli, Commands};

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Raindrop => export_raindrop::export_raindrop(),
    }
}
