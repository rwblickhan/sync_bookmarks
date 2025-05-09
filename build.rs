use clap::{CommandFactory, Parser, Subcommand};

// #KeepArgsInSync
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Raindrop,
}

fn main() -> std::io::Result<()> {
    let man = clap_mangen::Man::new(Cli::command());
    let mut buffer: Vec<u8> = Default::default();
    man.render(&mut buffer)?;

    std::fs::write("target/release/sync_bookmarks.1", buffer)?;

    Ok(())
}
