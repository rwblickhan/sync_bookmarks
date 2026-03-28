use std::collections::HashSet;

use anyhow::Context;

use crate::cache::{Cache, CacheType};
use crate::models::{GoodLinksApiResponse, SerializedLink};

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

    // Load existing links.json
    let mut serialized_links: Vec<SerializedLink> = match std::fs::read_to_string("links.json") {
        Ok(contents) => serde_json::from_str(&contents)?,
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
