use std::collections::HashMap;
use std::fs;
use std::process::Command;
use std::time::Duration;

use anyhow::{bail, Context};
use serde::Deserialize;
use ureq::http;

use crate::fetch::BANNED_HOSTS;
use crate::models::{LinkSource, SerializedLink};

const RAINDROP_API_BASE: &str = "https://api.raindrop.io/rest/v1";
const BATCH_SIZE: usize = 100;
const MAX_RETRIES: u32 = 4;
const IGNORED_COLLECTIONS: &[&str] = &["Papers"];

#[derive(Deserialize)]
struct RaindropCollection {
    #[serde(rename = "_id")]
    id: i64,
    title: String,
}

#[derive(Deserialize)]
struct CollectionsResponse {
    items: Vec<RaindropCollection>,
}

struct RaindropItem {
    id: i64,
    link: String,
    title: String,
    folder: String,
}

fn get_token() -> anyhow::Result<String> {
    let output = Command::new("op")
        .args(["item", "get", "Raindrop.io", "--fields", "label=token"])
        .output()
        .context("Failed to run op CLI — ensure 1Password CLI is installed and you're signed in")?;

    if !output.status.success() {
        bail!(
            "op CLI failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    Ok(String::from_utf8(output.stdout)
        .context("op output was not valid UTF-8")?
        .trim()
        .to_string())
}

fn source_to_collection_name(source: &LinkSource) -> &'static str {
    match source {
        LinkSource::GoodLinks => "GoodLinks",
        LinkSource::Obsidian => "Obsidian",
    }
}

fn fetch_or_create_collection(
    agent: &ureq::Agent,
    token: &str,
    name: &str,
    collections: &mut HashMap<String, i64>,
) -> anyhow::Result<i64> {
    if let Some(&id) = collections.get(name) {
        return Ok(id);
    }

    let resp: serde_json::Value = agent
        .post(&format!("{RAINDROP_API_BASE}/collection"))
        .header("Authorization", &format!("Bearer {token}"))
        .send_json(serde_json::json!({ "title": name }))?
        .body_mut()
        .read_json()
        .with_context(|| format!("Failed to parse create-collection response for '{name}'"))?;

    let id = resp["item"]["_id"]
        .as_i64()
        .with_context(|| format!("Missing _id in create-collection response for '{name}'"))?;

    println!("Created Raindrop collection '{}' (id {})", name, id);
    collections.insert(name.to_string(), id);
    Ok(id)
}

fn parse_export_csv(csv_text: &str) -> anyhow::Result<Vec<RaindropItem>> {
    let mut rdr = csv::Reader::from_reader(csv_text.as_bytes());
    let headers = rdr.headers()?.clone();

    let id_col = headers
        .iter()
        .position(|h| h == "id")
        .context("CSV export is missing an 'id' column — check Raindrop API format")?;
    let url_col = headers
        .iter()
        .position(|h| h == "url")
        .context("CSV export is missing a 'url' column")?;
    let title_col = headers.iter().position(|h| h == "title");
    let folder_col = headers.iter().position(|h| h == "folder");

    let mut items = Vec::new();
    for result in rdr.records() {
        let record = result.context("Failed to parse CSV record")?;

        let id: i64 = record
            .get(id_col)
            .context("Missing id field in CSV row")?
            .parse()
            .context("Failed to parse raindrop id as integer")?;

        let link = record
            .get(url_col)
            .context("Missing url field in CSV row")?
            .to_string();

        let title = title_col
            .and_then(|col| record.get(col))
            .unwrap_or("")
            .to_string();

        let folder = folder_col
            .and_then(|col| record.get(col))
            .unwrap_or("")
            .to_string();

        items.push(RaindropItem { id, link, title, folder });
    }

    Ok(items)
}

fn fetch_all_raindrops(
    agent: &ureq::Agent,
    token: &str,
    collection_ids: &[i64],
) -> anyhow::Result<Vec<RaindropItem>> {
    let mut all = Vec::new();

    for &id in collection_ids {
        let mut resp = agent
            .get(&format!("{RAINDROP_API_BASE}/raindrops/{id}/export.csv"))
            .header("Authorization", &format!("Bearer {token}"))
            .call()
            .with_context(|| format!("Failed to fetch CSV export for collection {id}"))?;

        let csv_text = resp.body_mut().read_to_string()?;
        let items = parse_export_csv(&csv_text)
            .with_context(|| format!("Failed to parse CSV export for collection {id}"))?;
        println!("  Collection {id}: {} raindrops", items.len());
        all.extend(items);
    }

    all.sort_by_key(|r| r.id);
    all.dedup_by_key(|r| r.id);

    Ok(all)
}

fn is_banned(url: &str) -> bool {
    url::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(|h| h.to_string()))
        .is_some_and(|host| BANNED_HOSTS.contains(&host.as_str()))
}

fn normalize_url(url: &str) -> String {
    match url::Url::parse(url) {
        Ok(mut parsed) => {
            parsed.set_query(None);
            parsed.set_fragment(None);
            let path = parsed.path().trim_end_matches('/').to_string();
            parsed.set_path(if path.is_empty() { "/" } else { &path });
            parsed.to_string()
        }
        Err(_) => url.to_string(),
    }
}

fn send_with_retry<F>(label: &str, mut send: F) -> anyhow::Result<()>
where
    F: FnMut() -> Result<ureq::http::Response<ureq::Body>, ureq::Error>,
{
    for attempt in 0..MAX_RETRIES {
        match send() {
            Ok(_) => return Ok(()),
            Err(ureq::Error::StatusCode(429)) => {
                let wait = Duration::from_secs(2_u64.pow(attempt + 1));
                eprintln!("Rate limited (429) on {label}, waiting {wait:?}...");
                std::thread::sleep(wait);
            }
            Err(e) => return Err(e).with_context(|| format!("{label} failed")),
        }
    }
    bail!("Exceeded {MAX_RETRIES} retries on {label}");
}

const DRY_RUN_PREVIEW_LIMIT: usize = 20;

fn print_preview<T>(items: &[T], label: &str, fmt: impl Fn(&T) -> String) {
    let shown = items.len().min(DRY_RUN_PREVIEW_LIMIT);
    for item in &items[..shown] {
        println!("  {label} {}", fmt(item));
    }
    if items.len() > DRY_RUN_PREVIEW_LIMIT {
        println!("  ... and {} more", items.len() - DRY_RUN_PREVIEW_LIMIT);
    }
}

pub fn sync_raindrop(dry_run: bool) -> anyhow::Result<()> {
    let links_json = fs::read_to_string("links.json").context("Failed to read links.json")?;
    let links: Vec<SerializedLink> =
        serde_json::from_str(&links_json).context("Failed to parse links.json")?;

    println!("Loaded {} links from links.json", links.len());

    let token = get_token().context("Failed to get Raindrop API token from 1Password")?;
    let agent = ureq::Agent::new_with_defaults();

    // Fetch existing collections (root + children) for collection name → id lookup
    let root_resp: CollectionsResponse = agent
        .get(&format!("{RAINDROP_API_BASE}/collections"))
        .header("Authorization", &format!("Bearer {token}"))
        .call()
        .context("Failed to fetch collections")?
        .body_mut()
        .read_json()
        .context("Failed to parse collections response")?;

    let child_resp: CollectionsResponse = agent
        .get(&format!("{RAINDROP_API_BASE}/collections/childrens"))
        .header("Authorization", &format!("Bearer {token}"))
        .call()
        .context("Failed to fetch child collections")?
        .body_mut()
        .read_json()
        .context("Failed to parse child collections response")?;

    let all_collections: Vec<RaindropCollection> = root_resp
        .items
        .into_iter()
        .chain(child_resp.items)
        .filter(|c| !IGNORED_COLLECTIONS.contains(&c.title.as_str()))
        .collect();

    let collection_ids: Vec<i64> = all_collections.iter().map(|c| c.id).collect();

    let mut collections: HashMap<String, i64> = all_collections
        .into_iter()
        .map(|c| (c.title, c.id))
        .collect();

    println!("Found {} existing Raindrop collections", collections.len());

    println!("Fetching all raindrops via export API...");
    let existing = fetch_all_raindrops(&agent, &token, &collection_ids)?;
    println!("Found {} existing raindrops", existing.len());

    // Build lookup maps keyed by normalized URL
    let links_by_url: HashMap<String, &SerializedLink> =
        links.iter().map(|l| (normalize_url(&l.url), l)).collect();

    let existing_by_url: HashMap<String, &RaindropItem> =
        existing.iter().map(|r| (normalize_url(&r.link), r)).collect();

    // Compute diff
    let to_add: Vec<&SerializedLink> = links
        .iter()
        .filter(|l| !is_banned(&l.url) && !existing_by_url.contains_key(&normalize_url(&l.url)))
        .collect();

    let to_delete: Vec<&RaindropItem> = existing
        .iter()
        .filter(|r| !is_banned(&r.link) && !links_by_url.contains_key(&normalize_url(&r.link)))
        .collect();

    println!("\nTo add:    {}", to_add.len());
    println!("To delete: {}", to_delete.len());

    if to_add.is_empty() && to_delete.is_empty() {
        println!("\nRaindrop is already in sync.");
        return Ok(());
    }

    if dry_run {
        println!("\n--- DRY RUN: no changes will be made ---");

        if !to_add.is_empty() {
            let mut by_collection: HashMap<&str, usize> = HashMap::new();
            for l in &to_add {
                *by_collection.entry(source_to_collection_name(&l.source)).or_default() += 1;
            }
            let mut counts: Vec<_> = by_collection.iter().collect();
            counts.sort_by_key(|(name, _)| *name);

            println!("\nTo add by collection:");
            for (name, count) in &counts {
                println!("  {name}: {count}");
            }

            println!("\nFirst {} links to add:", DRY_RUN_PREVIEW_LIMIT.min(to_add.len()));
            print_preview(&to_add, "ADD", |l| {
                format!(
                    "[{}] {} ({})",
                    source_to_collection_name(&l.source),
                    l.title,
                    l.url
                )
            });
        }

        if !to_delete.is_empty() {
            let mut by_collection: HashMap<&str, usize> = HashMap::new();
            for r in &to_delete {
                *by_collection.entry(r.folder.as_str()).or_default() += 1;
            }
            let mut counts: Vec<_> = by_collection.iter().collect();
            counts.sort_by_key(|(name, _)| *name);

            println!("\nTo delete by collection:");
            for (name, count) in &counts {
                println!("  {name}: {count}");
            }

            println!("\nFirst {} raindrops to delete:", DRY_RUN_PREVIEW_LIMIT.min(to_delete.len()));
            print_preview(&to_delete, "DEL", |r| format!("{} ({})", r.title, r.link));
        }

        return Ok(());
    }

    // Group adds by collection so we can assign the right collection id
    let mut by_collection: HashMap<&str, Vec<&SerializedLink>> = HashMap::new();
    for link in &to_add {
        by_collection
            .entry(source_to_collection_name(&link.source))
            .or_default()
            .push(link);
    }

    // Add missing links in batches of BATCH_SIZE
    for (collection_name, collection_links) in &by_collection {
        let collection_id =
            fetch_or_create_collection(&agent, &token, collection_name, &mut collections)?;

        for chunk in collection_links.chunks(BATCH_SIZE) {
            let items: Vec<serde_json::Value> = chunk
                .iter()
                .map(|link| {
                    serde_json::json!({
                        "link": link.url,
                        "title": link.title,
                        "tags": link.tags,
                        "collection": { "$id": collection_id }
                    })
                })
                .collect();

            send_with_retry(
                &format!("create {} links in '{collection_name}'", chunk.len()),
                || {
                    agent
                        .post(&format!("{RAINDROP_API_BASE}/raindrops"))
                        .header("Authorization", &format!("Bearer {token}"))
                        .send_json(serde_json::json!({ "items": items }))
                },
            )?;

            println!("Added {} links to '{}'", chunk.len(), collection_name);
        }
    }

    // Delete extra raindrops in batches of BATCH_SIZE
    let delete_ids: Vec<i64> = to_delete.iter().map(|r| r.id).collect();
    for chunk in delete_ids.chunks(BATCH_SIZE) {
        let body = serde_json::to_vec(&serde_json::json!({ "ids": chunk }))?;
        send_with_retry(&format!("delete {} raindrops", chunk.len()), || {
            agent.run(
                http::Request::builder()
                    .method(http::Method::DELETE)
                    .uri(format!("{RAINDROP_API_BASE}/raindrops/-1"))
                    .header("Authorization", format!("Bearer {token}"))
                    .header("Content-Type", "application/json")
                    .body(body.clone())
                    .expect("failed to build DELETE request"),
            )
        })?;

        println!("Deleted {} raindrops", chunk.len());
    }

    println!("\nSync complete!");
    Ok(())
}
