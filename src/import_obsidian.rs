use std::collections::HashSet;

use anyhow::Context;
use pulldown_cmark::{Event, LinkType, Options, Parser, Tag};
use walkdir::{DirEntry, WalkDir};

use crate::models::{LinkSource, ObsidianLink, SerializedLink};

fn parse_markdown_links(entry: DirEntry) -> anyhow::Result<Vec<ObsidianLink>> {
    let mut obsidian_links = Vec::new();
    let contents = std::fs::read_to_string(entry.path())?;

    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_YAML_STYLE_METADATA_BLOCKS);

    let parser = Parser::new_ext(&contents, options);
    for event in parser {
        if let Event::Start(Tag::Link {
            link_type: LinkType::Inline,
            dest_url,
            title,
            id: _,
        }) = event
        {
            obsidian_links.push(ObsidianLink {
                title: title.to_string(),
                url: dest_url.to_string(),
            });
        }
    }

    Ok(obsidian_links)
}

fn is_markdown_file(entry: &DirEntry) -> bool {
    if !entry.file_type().is_file() {
        return false;
    }
    entry.path().extension().is_some_and(|ext| ext == "md")
}

fn process_markdown_files(directory: &str) -> anyhow::Result<Vec<ObsidianLink>> {
    WalkDir::new(directory)
        .into_iter()
        .filter_map(Result::ok)
        .filter(is_markdown_file)
        .try_fold(Vec::new(), |mut acc, entry| {
            let links = parse_markdown_links(entry)?;
            acc.extend(links);
            Ok(acc)
        })
}

pub fn import_obsidian() -> anyhow::Result<()> {
    let obsidian_links = process_markdown_files("/Users/rwblickhan/Developer/notes")?;
    println!("Found {} Obsidian links", obsidian_links.len());

    let mut obsidian_urls = HashSet::new();
    for link in &obsidian_links {
        obsidian_urls.insert(link.url.clone());
    }

    let mut serialized_links: Vec<SerializedLink> = match std::fs::read_to_string("links.json") {
        Ok(serialized_links_file_contents) => {
            serde_json::from_str::<Vec<SerializedLink>>(&serialized_links_file_contents)?
                .into_iter()
                .filter(|link| {
                    obsidian_urls.contains(&link.url) || link.source != LinkSource::Obsidian
                })
                .collect()
        }
        Err(_) => Vec::new(),
    };

    let mut serialized_link_urls = HashSet::new();
    for link in &serialized_links {
        serialized_link_urls.insert(link.url.clone());
    }

    let mut already_serialized_skipped = 0;
    let mut serialized = 0;

    for link in obsidian_links {
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

    println!("Serialized {serialized} Obsidian links; {already_serialized_skipped} links already serialized");

    Ok(())
}
