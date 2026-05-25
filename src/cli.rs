use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Synchronize links.json directly to Raindrop.io via the API.
    ///
    /// Requires a 1Password item named "Raindrop.io" with a field labeled "token"
    /// containing your Raindrop.io personal API token.
    Raindrop {
        /// Show what would change without making any API calls
        #[arg(long)]
        dry_run: bool,
    },
    /// Import bookmarks from GoodLinks and Obsidian
    Import {
        #[arg(short, long)]
        verbose: bool,
    },
}
