mod cache;
mod cli;
mod fetch;
mod import_goodlinks;
mod import_obsidian;
mod models;
mod sync_raindrop;

use clap::Parser;
use cli::{Cli, Commands};
use fetch::fetch_to_cache;
use import_goodlinks::import_goodlinks;
use import_obsidian::import_obsidian;
use sync_raindrop::sync_raindrop;

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let links = &cli.links;
    let cache = &cli.cache;

    match cli.command {
        Commands::Raindrop { dry_run } => sync_raindrop(dry_run, links, cache),
        Commands::Import { verbose } => {
            import_goodlinks(verbose, links, cache)?;
            import_obsidian(links, cache)?;
            fetch_to_cache(verbose, links, cache)?;
            Ok(())
        }
    }
}
