use anyhow::Context;
use pulldown_cmark::{Event, LinkType, Options, Parser, Tag, TagEnd};
use regex::Regex;
use std::collections::HashSet;
use walkdir::{DirEntry, WalkDir};

use crate::models::{LinkSource, ObsidianLink, SerializedLink};

fn parse_markdown_links(file_contents: &str) -> anyhow::Result<Vec<ObsidianLink>> {
    let mut obsidian_links = Vec::new();

    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_YAML_STYLE_METADATA_BLOCKS);

    let mut current_link = None;
    let mut current_link_title: Option<String> = None;

    // Find Markdown-formatted links
    let parser = Parser::new_ext(file_contents, options);
    for event in parser {
        match event {
            Event::Start(Tag::Link {
                link_type: LinkType::Inline,
                dest_url,
                title: _,
                id: _,
            }) => {
                current_link = Some(dest_url.to_string());
            }
            Event::Text(text) | Event::Code(text) => {
                if current_link.is_some() {
                    current_link_title =
                        Some(current_link_title.unwrap_or_default() + text.as_ref());
                }
            }
            Event::End(TagEnd::Link) => {
                if let Some(url) = current_link.clone() {
                    if let Some(title) = current_link_title.clone() {
                        current_link = None;
                        current_link_title = None;
                        obsidian_links.push(ObsidianLink { title, url });
                    }
                }
            }
            _ => {}
        }
    }

    let mut parsed_urls: HashSet<_> = obsidian_links.iter().map(|link| link.url.clone()).collect();

    // Find bare links
    let url_regex = Regex::new(r"https?:\/\/[^\s\)\]]*")?;
    for capture in url_regex.captures_iter(file_contents) {
        let url = capture[0].to_string();
        if parsed_urls.contains(&url) {
            continue;
        }
        obsidian_links.push(ObsidianLink {
            title: url.clone(),
            url: url.clone(),
        });
        parsed_urls.insert(url);
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
            let file_contents = std::fs::read_to_string(entry.path())?;
            let links = parse_markdown_links(file_contents.as_ref())?;
            acc.extend(links);
            Ok(acc)
        })
}

pub fn import_obsidian() -> anyhow::Result<()> {
    let obsidian_links = process_markdown_files("/Users/rwblickhan/Developer/notes")?;

    println!("Found {} Obsidian links", obsidian_links.len());

    let obsidian_urls: HashSet<_> = obsidian_links.iter().map(|link| link.url.clone()).collect();

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

    let serialized_link_urls: HashSet<_> = serialized_links
        .iter()
        .map(|link| link.url.clone())
        .collect();

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_markdown_links_only() -> anyhow::Result<()> {
        let file_contents = r#"
# Test Document

This is a [test link](https://example.com).
This is another [link with title](https://example.org/page).
        "#;

        let links = parse_markdown_links(file_contents)?;

        assert_eq!(links.len(), 2);
        assert_eq!(links[0].title, "test link");
        assert_eq!(links[0].url, "https://example.com");
        assert_eq!(links[1].title, "link with title");
        assert_eq!(links[1].url, "https://example.org/page");

        Ok(())
    }

    #[test]
    fn test_parse_bare_urls_only() -> anyhow::Result<()> {
        let file_contents = r#"
# Test Document

This document has https://example.com as a bare URL.
And another one: https://example.org/page?q=test
        "#;

        let links = parse_markdown_links(file_contents)?;

        assert_eq!(links.len(), 2);
        assert_eq!(links[0].title, "https://example.com");
        assert_eq!(links[0].url, "https://example.com");
        assert_eq!(links[1].title, "https://example.org/page?q=test");
        assert_eq!(links[1].url, "https://example.org/page?q=test");

        Ok(())
    }

    #[test]
    fn test_parse_bare_urls_in_parens() -> anyhow::Result<()> {
        let file_contents = r#"
# Test Document

This document has (https://example.com) as a bare URL.
And another one: (https://example.org/page?q=test)
        "#;

        let links = parse_markdown_links(file_contents)?;

        assert_eq!(links.len(), 2);
        assert_eq!(links[0].title, "https://example.com");
        assert_eq!(links[0].url, "https://example.com");
        assert_eq!(links[1].title, "https://example.org/page?q=test");
        assert_eq!(links[1].url, "https://example.org/page?q=test");

        Ok(())
    }

    #[test]
    fn test_parse_mixed_links() -> anyhow::Result<()> {
        let file_contents = r#"
# Test Document with Mixed Links

This is a [Markdown link](https://example.com).
This is a bare URL: https://example.org/page
This is a duplicate bare URL: https://example.com (should be ignored)
        "#;

        let links = parse_markdown_links(file_contents)?;

        assert_eq!(links.len(), 2);

        let urls: HashSet<String> = links.iter().map(|link| link.url.clone()).collect();
        assert_eq!(urls.len(), 2);
        assert!(urls.contains("https://example.com"));
        assert!(urls.contains("https://example.org/page"));

        Ok(())
    }

    #[test]
    fn test_complex_url_patterns() -> anyhow::Result<()> {
        let file_contents = r#"
# Complex URL Patterns

Regular HTTP: http://example.com
HTTPS with path: https://api.example.org/v1/data
With query params: https://example.com/search?q=test&page=1
With anchor: https://docs.example.com/guide#section-3
With port: https://example.com:8080/app
        "#;

        let links = parse_markdown_links(file_contents)?;

        assert_eq!(links.len(), 5);

        let urls: HashSet<_> = links.iter().map(|link| link.url.clone()).collect();
        assert!(urls.contains("http://example.com"));
        assert!(urls.contains("https://api.example.org/v1/data"));
        assert!(urls.contains("https://example.com/search?q=test&page=1"));
        assert!(urls.contains("https://docs.example.com/guide#section-3"));
        assert!(urls.contains("https://example.com:8080/app"));

        Ok(())
    }

    #[test]
    fn test_no_links() -> anyhow::Result<()> {
        let file_contents = r#"
# Document with No Links

This document doesn't contain any links.
Just plain text content.
        "#;

        let links = parse_markdown_links(file_contents)?;

        assert_eq!(links.len(), 0);

        Ok(())
    }

    #[test]
    fn test_markdown_links_with_special_characters() -> anyhow::Result<()> {
        let file_contents = r#"
# Document with Special Characters in Links

[Link with query params](https://example.com/search?name=John+Doe&age=25)
[Link with unicode](https://example.com/café)
        "#;

        let links = parse_markdown_links(file_contents)?;

        assert_eq!(links.len(), 2);
        assert!(links
            .iter()
            .any(|link| link.url == "https://example.com/search?name=John+Doe&age=25"));
        assert!(links
            .iter()
            .any(|link| link.url == "https://example.com/café"));

        Ok(())
    }
}
