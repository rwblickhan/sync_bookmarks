use anyhow::{Context, Ok};
use rusqlite::Connection;

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

        let mut link_iter = stmt
            .query_and_then([self.name.clone(), url.to_string()], |row| {
                let tags_sql: String = row.get(3)?;
                Ok(ParsedLink::new(
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    serde_json::from_str(&tags_sql)?,
                    row.get(4)?,
                ))
            })
            .with_context(|| {
                format!(
                    "Failed to query for for {} looking for link {}",
                    self.name, url
                )
            })?;
        Ok(link_iter.next().transpose()?)
    }
}
