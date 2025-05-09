use anyhow::{Context, Ok};
use rusqlite::{named_params, Connection};

use crate::models::ParsedLink;

pub enum CacheType {
    Disk(String),
    Memory,
}

pub struct Cache {
    conn: Connection,
    table_name: String,
}

impl Cache {
    pub fn new(table_name: &str, cache_type: CacheType) -> anyhow::Result<Self> {
        let conn = match cache_type {
            CacheType::Memory => Connection::open_in_memory(),
            CacheType::Disk(name) => Connection::open(name),
        }
        .context("Failed to open database")?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS :table_name (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                url TEXT UNIQUE,
                title TEXT,
                parsed_content TEXT,
                source TEXT CHECK(source IN ('GoodLinks', 'Obsidian')),
                tags JSON,
                archived_at DATETIME
            )",
            named_params![":table_name": table_name],
        )
        .with_context(|| format!("Failed to create table {}", table_name))?;

        Ok(Cache {
            conn,
            table_name: table_name.to_string(),
        })
    }

    pub fn query(&self, url: &str) -> anyhow::Result<Option<ParsedLink>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT url, title, source, tags, parsed_content FROM :table_name WHERE url = :url",
            )
            .with_context(|| {
                format!(
                    "Failed to prepare query for {} looking for link {}",
                    self.table_name, url
                )
            })?;

        let mut rows = stmt
            .query(named_params![":table_name": self.table_name, ":url": url.to_string()])
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
            .prepare("SELECT url, title, source, tags, parsed_content FROM :table_name")
            .with_context(|| {
                format!(
                    "Failed to prepare query for {} looking for all links",
                    self.table_name,
                )
            })?;

        let mut links = Vec::new();
        let mut rows = stmt
            .query(named_params![":table_name": self.table_name])
            .context("Failed to query for all links")?;
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
            .prepare("SELECT url, title, source, tags, parsed_content FROM :name WHERE archived_at IS NULL")
            .with_context(|| {
                format!(
                    "Failed to prepare query for {} looking for unarchived links",
                    self.table_name
                )
            })?;

        let mut links = Vec::new();
        let mut rows = stmt
            .query(named_params![":name": self.table_name])
            .context("Failed to query for all links")?;
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
            "INSERT INTO :name (url, title, source, tags, parsed_content, archived_at) VALUES (:url, :title, :source, :tags, :parsed_content, NULL)",
            named_params![
                ":name": self.table_name,
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
