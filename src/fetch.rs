use anyhow::Context;
use indicatif::ProgressBar;
use readability::extractor;
use url::Url;

use crate::{
    cache::{Cache, CacheType},
    models::{Article, CachedLink, SerializedLink},
};

const BANNED_HOSTS: &[&str] = &[
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

pub fn fetch_to_cache(verbose: bool) -> anyhow::Result<()> {
    let cache = Cache::new(CacheType::Disk("cache.db".to_owned()))?;

    let serialized_links: Vec<SerializedLink> = serde_json::from_str::<Vec<SerializedLink>>(
        &std::fs::read_to_string("links.json").context("links.json must be created")?,
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

        let article = match &cached_link {
            Some(link) => Article {
                title: link.title.clone(),
                text_content: link.text_content.clone(),
            },
            None => {
                let product = match extractor::scrape(&link.url) {
                    Ok(product) => product,
                    Err(e) => {
                        if verbose {
                            println!("Failed to parse link {}: {e}", link.url);
                        }
                        continue;
                    }
                };
                Article {
                    title: product.title,
                    text_content: product.text,
                }
            }
        };

        if cached_link.is_none() {
            cache.insert(&CachedLink::new(
                link.url,
                article.title,
                link.source,
                link.tags,
                article.text_content,
            ))?;
        }
        pb.inc(1);
    }

    pb.finish_with_message("Done!");

    Ok(())
}
