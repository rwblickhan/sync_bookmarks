use anyhow::{Context, Ok};
use rusqlite::{named_params, Connection};

use crate::models::ParsedLink;

pub enum CacheType {
    Disk(String),
    Memory,
}

pub struct Cache {
    conn: Connection,
}

impl Cache {
    pub fn new(cache_type: CacheType) -> anyhow::Result<Self> {
        let conn = match cache_type {
            CacheType::Memory => Connection::open_in_memory(),
            CacheType::Disk(name) => Connection::open(name),
        }
        .context("Failed to open database")?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS cache (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                url TEXT UNIQUE,
                title TEXT,
                parsed_content TEXT,
                source TEXT CHECK(source IN ('GoodLinks', 'Obsidian')),
                tags JSON,
                archived_at DATETIME
            )",
            [],
        )
        .context("Failed to create table")?;

        Ok(Cache { conn })
    }

    pub fn query(&self, url: &str) -> anyhow::Result<Option<ParsedLink>> {
        let mut stmt = self
            .conn
            .prepare("SELECT url, title, source, tags, parsed_content FROM cache WHERE url = :url")
            .with_context(|| format!("Failed to prepare query looking for link {}", url))?;

        let mut rows = stmt
            .query(named_params![":url": url.to_string()])
            .with_context(|| format!("Failed to query for links with url {url}"))?;

        if let Some(row) = rows.next()? {
            let tags_sql: String = row.get(3)?;
            return Ok(Some(ParsedLink::new(
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                serde_json::from_str(&tags_sql)?,
                row.get(4)?,
            )));
        }
        Ok(None)
    }

    pub fn query_all(&self) -> anyhow::Result<Vec<ParsedLink>> {
        let mut stmt = self
            .conn
            .prepare("SELECT url, title, source, tags, parsed_content FROM cache")
            .with_context(|| format!("Failed to prepare query looking for all links",))?;

        let mut links = Vec::new();
        let mut rows = stmt.query([]).context("Failed to query for all links")?;
        while let Some(row) = rows.next()? {
            let tags_sql: String = row.get(3)?;
            links.push(ParsedLink::new(
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                serde_json::from_str(&tags_sql)?,
                row.get(4)?,
            ));
        }
        Ok(links)
    }

    pub fn query_unarchived(&self) -> anyhow::Result<Vec<ParsedLink>> {
        let mut stmt = self
            .conn
            .prepare("SELECT url, title, source, tags, parsed_content FROM cache WHERE archived_at IS NULL")
            .with_context(|| {
                format!(
                    "Failed to prepare query looking for unarchived links",
                )
            })?;

        let mut links = Vec::new();
        let mut rows = stmt.query([]).context("Failed to query for all links")?;
        while let Some(row) = rows.next()? {
            let tags_sql: String = row.get(3)?;
            links.push(ParsedLink::new(
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                serde_json::from_str(&tags_sql)?,
                row.get(4)?,
            ));
        }
        Ok(links)
    }

    pub fn insert(&self, link: &ParsedLink) -> anyhow::Result<()> {
        let tags_sql = serde_json::to_string(&link.tags)?;
        self.conn.execute(
            "INSERT INTO cache (url, title, source, tags, parsed_content, archived_at) VALUES (:url, :title, :source, :tags, :parsed_content, NULL)",
            named_params![
                ":url": link.url,
                ":title": link.title,
                ":source": link.source,
                ":tags": tags_sql,
                ":parsed_content": link.text_content
            ],
        )?;
        Ok(())
    }
}
