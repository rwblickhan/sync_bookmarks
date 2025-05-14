use std::collections::HashSet;

use anyhow::Context;

use crate::models::{GoodLinksLink, LinkSource, SerializedLink};

pub fn import_goodlinks() -> anyhow::Result<()> {
    let goodlinks_export_file_contents =
        std::fs::read_to_string("goodlinks.json").context("Failed to find goodlinks.json")?;
    let goodlinks_exported_links: Vec<GoodLinksLink> =
        serde_json::from_str::<Vec<GoodLinksLink>>(&goodlinks_export_file_contents)
            .context("Failed to parse goodlinks.json")?;

    println!("Found {} GoodLinks links", goodlinks_exported_links.len());

    let goodlinks_exported_urls: HashSet<_> = goodlinks_exported_links
        .iter()
        .map(|link| link.url.clone())
        .collect();

    let mut serialized_links: Vec<SerializedLink> = match std::fs::read_to_string("links.json") {
        Ok(serialized_links_file_contents) => {
            serde_json::from_str::<Vec<SerializedLink>>(&serialized_links_file_contents)?
                .into_iter()
                .filter(|link| {
                    goodlinks_exported_urls.contains(&link.url)
                        || link.source != LinkSource::GoodLinks
                })
                .collect()
        }
        Err(_) => Vec::new(),
    };

    let serialized_link_urls: HashSet<_> = serialized_links
        .iter()
        .map(|link| link.url.clone())
        .collect();

    let mut not_read_skipped = 0;
    let mut already_serialized_skipped = 0;
    let mut serialized = 0;

    for link in goodlinks_exported_links {
        if link.read_at.is_none() {
            not_read_skipped += 1;
            continue;
        }

        if serialized_link_urls.contains(&link.url) {
            already_serialized_skipped += 1;
            continue;
        }

        serialized += 1;
        serialized_links.push(link.into());
    }

    let serialized_links_file =
        std::fs::File::create("links.json").context("Failed to create links.json")?;
    let serialized_links_file = std::io::BufWriter::new(serialized_links_file);
    serde_json::to_writer_pretty(serialized_links_file, &serialized_links)
        .context("Failed to write to links.json")?;

    println!("Serialized {serialized} GoodLinks links; skipped {not_read_skipped} unread links and {already_serialized_skipped} links already serialized");

    Ok(())
}
