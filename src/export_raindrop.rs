use std::{fs::File, io::BufWriter};

use crate::cache::{Cache, CacheType};

pub fn export_raindrop() -> anyhow::Result<()> {
    let cache = Cache::new(CacheType::Disk("cache.db".to_owned()))?;
    let links = cache.query_all()?;
    let file = File::create("export.csv")?;
    let mut wtr = csv::Writer::from_writer(BufWriter::new(file));
    wtr.write_record(["folder", "url", "title", "tags"])?;
    for link in links {
        wtr.serialize((link.source, link.url, link.title, link.tags.join(",")))?;
    }
    wtr.flush()?;
    Ok(())
}
