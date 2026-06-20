use std::path::Path;

use anyhow::Context;
use indicatif::ProgressBar;
use readability::extractor;
use url::Url;

use crate::{
    cache::{Cache, CacheType},
    models::{CachedLink, SerializedLink},
};

pub const BANNED_HOSTS: &[&str] = &[
    "vitalik.ca",
    "archive.ph",
    "archive.is",
    "historic-cities.huji.ac.il",
    "society.robinsloan.com",
    "esoteric.codes",
    "l.bulletin.com",
    "probmods.org",
    "example.com",
    "xn--url-u63b6dn8esao8c4jh9d2c1a0lk29262bmhrb.com",
];

pub fn fetch_to_cache(verbose: bool, links_path: &Path, cache_path: &Path) -> anyhow::Result<()> {
    let cache = Cache::new(CacheType::Disk(cache_path.to_string_lossy().into_owned()))?;

    let serialized_links: Vec<SerializedLink> = serde_json::from_str::<Vec<SerializedLink>>(
        &std::fs::read_to_string(links_path).context("links.json must be created")?,
    )
    .context("Failed to parse links.json")?;

    let pb = ProgressBar::new(serialized_links.len().try_into()?);

    for link in serialized_links {
        if let Ok(parsed_url) = Url::parse(&link.url) {
            if let Some(host) = parsed_url.host_str() {
                if BANNED_HOSTS.contains(&host) {
                    continue;
                }
            }
        }

        let cached_link = cache.query(&link.url)?;

        if cached_link.is_none() {
            if verbose {
                println!("Fetching {}", link.url);
            }
            match extractor::scrape(&link.url) {
                Ok(product) => {
                    cache.insert(&CachedLink::new(
                        link.url,
                        product.title,
                        link.source,
                        link.tags,
                        product.text,
                    ))?;
                }
                Err(e) => {
                    if verbose {
                        println!("Failed to parse link {}: {e}", link.url);
                    }
                    // Record the failure so this link isn't retried on every future run.
                    cache.insert_failed(&CachedLink::new(
                        link.url,
                        link.title,
                        link.source,
                        link.tags,
                        String::new(),
                    ))?;
                }
            }
        }
        pb.inc(1);
    }

    pb.finish_with_message("Done!");

    Ok(())
}
