use crate::cache::{Cache, CacheType};

pub fn export_raindrop() -> anyhow::Result<()> {
    let cache = Cache::new("cache", CacheType::Disk("cache.db".to_owned()))?;
    let links = cache.query_all()?;
    let mut wtr = csv::Writer::from_writer(std::io::stdout());
    wtr.write_record(["folder", "url", "title", "tags"])?;
    for link in links {
        wtr.serialize((link.source, link.url, link.title, link.tags.join(",")))?;
    }
    wtr.flush()?;
    Ok(())
}
