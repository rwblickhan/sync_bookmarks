use clap::CommandFactory;
use clap_complete::{generate_to, shells::Fish};

include!("src/cli.rs");

fn main() -> std::io::Result<()> {
    let mut cmd = Cli::command();

    generate_to(Fish, &mut cmd, "sync_bookmarks", "target/release/")?;

    let man = clap_mangen::Man::new(cmd);
    let mut buffer: Vec<u8> = Default::default();
    man.render(&mut buffer)?;
    std::fs::write("target/release/sync_bookmarks.1", buffer)?;

    Ok(())
}
