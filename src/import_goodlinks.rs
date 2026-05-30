use std::collections::HashSet;

use anyhow::Context;

use crate::cache::{Cache, CacheType};
use crate::models::{GoodLinksApiResponse, LinkSource, SerializedLink};

const GOODLINKS_OP_BASE_URL: &str = "op://Private/GoodLinks/base_url";
const GOODLINKS_OP_TOKEN: &str = "op://Private/GoodLinks/token";

fn read_op_secret(path: &str) -> anyhow::Result<String> {
    let output = std::process::Command::new("op")
        .args(["read", path])
        .output()
        .context("Failed to run 1Password CLI")?;
    if !output.status.success() {
        anyhow::bail!(
            "1Password CLI failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(String::from_utf8(output.stdout)?.trim().to_string())
}

fn filter_removed_goodlinks(
    existing: Vec<SerializedLink>,
    api_urls: &HashSet<String>,
) -> Vec<SerializedLink> {
    existing
        .into_iter()
        .filter(|link| api_urls.contains(&link.url) || link.source != LinkSource::GoodLinks)
        .collect()
}

pub fn import_goodlinks(verbose: bool) -> anyhow::Result<()> {
    let base_url = read_op_secret(GOODLINKS_OP_BASE_URL)?;
    if verbose {
        println!("GoodLinks base URL: {base_url}");
    }
    let token = read_op_secret(GOODLINKS_OP_TOKEN)?;

    // Fetch all read links from GoodLinks API with pagination
    let mut api_links = Vec::new();
    let mut offset = 0usize;
    const LIMIT: usize = 1000;

    loop {
        let url = format!("{}/api/v1/lists/read?limit={}&offset={}", base_url, LIMIT, offset);

        let response: GoodLinksApiResponse = ureq::get(&url)
            .header("Authorization", format!("Bearer {}", token))
            .call()
            .context("Failed to call GoodLinks API")?
            .body_mut()
            .read_json()
            .context("Failed to parse GoodLinks API response")?;

        let has_more = response.has_more;
        api_links.extend(response.data);

        if !has_more {
            break;
        }
        offset += LIMIT;
    }

    println!("Found {} read GoodLinks links", api_links.len());

    // Load cached URLs to skip already-fetched links
    let cache = Cache::new(CacheType::Disk("cache.db".to_string()))?;
    let cached_urls = cache.query_all_urls()?;

    let api_urls: HashSet<_> = api_links.iter().map(|link| link.url.clone()).collect();

    // Load existing links.json, removing GoodLinks entries no longer in the API response
    let mut serialized_links: Vec<SerializedLink> = match std::fs::read_to_string("links.json") {
        Ok(contents) => {
            filter_removed_goodlinks(serde_json::from_str(&contents)?, &api_urls)
        }
        Err(_) => Vec::new(),
    };

    let serialized_link_urls: HashSet<_> = serialized_links
        .iter()
        .map(|link| link.url.clone())
        .collect();

    let mut already_cached_skipped = 0;
    let mut already_serialized_skipped = 0;
    let mut serialized = 0;

    for link in api_links {
        if cached_urls.contains(&link.url) {
            already_cached_skipped += 1;
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

    println!(
        "Serialized {serialized} GoodLinks links; skipped {already_cached_skipped} already cached and {already_serialized_skipped} already in links.json"
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn goodlinks_link(url: &str) -> SerializedLink {
        SerializedLink {
            url: url.to_string(),
            title: url.to_string(),
            tags: Vec::new(),
            source: LinkSource::GoodLinks,
        }
    }

    fn obsidian_link(url: &str) -> SerializedLink {
        SerializedLink {
            url: url.to_string(),
            title: url.to_string(),
            tags: Vec::new(),
            source: LinkSource::Obsidian,
        }
    }

    #[test]
    fn test_removes_goodlinks_entry_missing_from_api() {
        let existing = vec![
            goodlinks_link("https://keep.example.com"),
            goodlinks_link("https://removed.example.com"),
        ];
        let api_urls: HashSet<String> = ["https://keep.example.com".to_string()].into();

        let result = filter_removed_goodlinks(existing, &api_urls);

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].url, "https://keep.example.com");
    }

    #[test]
    fn test_preserves_obsidian_entries_not_in_api() {
        let existing = vec![
            goodlinks_link("https://goodlinks.example.com"),
            obsidian_link("https://obsidian.example.com"),
        ];
        let api_urls: HashSet<String> = ["https://goodlinks.example.com".to_string()].into();

        let result = filter_removed_goodlinks(existing, &api_urls);

        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_empty_api_removes_all_goodlinks_entries() {
        let existing = vec![
            goodlinks_link("https://a.example.com"),
            goodlinks_link("https://b.example.com"),
            obsidian_link("https://obsidian.example.com"),
        ];
        let api_urls: HashSet<String> = HashSet::new();

        let result = filter_removed_goodlinks(existing, &api_urls);

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].source, LinkSource::Obsidian);
    }

    #[test]
    fn test_all_api_urls_present_keeps_everything() {
        let existing = vec![
            goodlinks_link("https://a.example.com"),
            goodlinks_link("https://b.example.com"),
        ];
        let api_urls: HashSet<String> = [
            "https://a.example.com".to_string(),
            "https://b.example.com".to_string(),
        ]
        .into();

        let result = filter_removed_goodlinks(existing, &api_urls);

        assert_eq!(result.len(), 2);
    }
}
