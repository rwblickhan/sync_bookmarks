mod cache;
mod cli;
mod export_raindrop;
mod fetch;
mod import_goodlinks;
mod import_obsidian;
mod models;

use clap::Parser;
use cli::{Cli, Commands};
use export_raindrop::export_raindrop;
use fetch::fetch_to_cache;
use import_goodlinks::import_goodlinks;
use import_obsidian::import_obsidian;

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Raindrop => export_raindrop(),
        Commands::Import => {
            import_goodlinks()?;
            import_obsidian()?;
            fetch_to_cache()?;
            Ok(())
        }
    }
}
