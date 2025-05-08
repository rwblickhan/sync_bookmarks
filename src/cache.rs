use anyhow::{Context, Ok};
use rusqlite::{params, Connection};

use crate::types::ParsedLink;

pub struct Cache {
    conn: Connection,
    name: String,
}

impl Cache {
    pub fn new(name: &str) -> anyhow::Result<Self> {
        let conn = Connection::open(name).context("Failed to open database")?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS (?1) (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                url TEXT UNIQUE,
                title TEXT,
                parsed_content TEXT,
                source TEXT CHECK(source IN ('GoodLinks', 'Obsidian')),
                tags JSON,
                archived_at DATETIME
            )",
            [name],
        )
        .with_context(|| format!("Failed to create table {}", name))?;

        Ok(Cache {
            conn,
            name: name.to_string(),
        })
    }

    pub fn query(&self, url: &str) -> anyhow::Result<Option<ParsedLink>> {
        let mut stmt = self
            .conn
            .prepare("SELECT url, title, source, tags, parsed_content FROM (?1) WHERE url = (?2)")
            .with_context(|| {
                format!(
                    "Failed to prepare query for {} looking for link {}",
                    self.name, url
                )
            })?;

        let mut rows = stmt
            .query([self.name.clone(), url.to_string()])
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

    pub fn query_all(&self, url: &str) -> anyhow::Result<Vec<ParsedLink>> {
        let mut stmt = self
            .conn
            .prepare("SELECT url, title, source, tags, parsed_content FROM (?1)")
            .with_context(|| {
                format!(
                    "Failed to prepare query for {} looking for link {}",
                    self.name, url
                )
            })?;

        let mut links = Vec::new();
        let mut rows = stmt
            .query([self.name.clone(), url.to_string()])
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
            .prepare("SELECT url, title, source, tags, parsed_content FROM (?1) WHERE archived_at IS NULL")
            .with_context(|| {
                format!(
                    "Failed to prepare query for {} looking for unarchived links",
                    self.name
                )
            })?;

        let mut links = Vec::new();
        let mut rows = stmt
            .query([self.name.clone()])
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
            "INSERT INTO (?1) (url, title, source, tags, parsed_content, archived_at) VALUES (?2, ?3, ?4, ?5, ?6, NULL)",
            params![
                self.name.clone(),
                link.url,
                link.title,
                link.source,
                tags_sql,
                link.text_content
            ],
        )?;
        Ok(())
    }
}
