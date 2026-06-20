use std::collections::HashSet;

use anyhow::{Context, Ok};
use rusqlite::{named_params, Connection};

use crate::models::CachedLink;

pub enum CacheType {
    Disk(String),
    Memory,
}

/// How long a failed fetch is remembered before it's retried.
const RETRY_AFTER_DAYS: u32 = 30;

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

        // `fetch_failed_at` is distinct from `archived_at` (which tracks when GoodLinks
        // archived the link). Added after the original schema, so backfill it for existing DBs.
        match conn.execute("ALTER TABLE cache ADD COLUMN fetch_failed_at DATETIME", []) {
            Result::Ok(_) => {}
            Err(rusqlite::Error::SqliteFailure(_, Some(ref msg)))
                if msg.contains("duplicate column name") => {}
            Err(e) => return Err(e).context("Failed to add fetch_failed_at column"),
        }

        Ok(Cache { conn })
    }

    /// Links that failed to fetch are recorded with `fetch_failed_at`; once that's more
    /// than `RETRY_AFTER_DAYS` old, treat the link as uncached so it gets retried.
    pub fn query(&self, url: &str) -> anyhow::Result<Option<CachedLink>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT url, title, source, tags, parsed_content FROM cache
                 WHERE url = :url
                 AND (fetch_failed_at IS NULL OR fetch_failed_at > datetime('now', :retry_after))",
            )
            .with_context(|| format!("Failed to prepare query looking for link {}", url))?;

        let mut rows = stmt
            .query(named_params![
                ":url": url.to_string(),
                ":retry_after": format!("-{RETRY_AFTER_DAYS} days")
            ])
            .with_context(|| format!("Failed to query for links with url {url}"))?;

        if let Some(row) = rows.next()? {
            let tags_sql: String = row.get(3)?;
            return Ok(Some(CachedLink::new(
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                serde_json::from_str(&tags_sql)?,
                row.get(4)?,
            )));
        }
        Ok(None)
    }

    pub fn query_all(&self) -> anyhow::Result<Vec<CachedLink>> {
        let mut stmt = self
            .conn
            .prepare("SELECT url, title, source, tags, parsed_content FROM cache")
            .context("Failed to prepare query looking for all links")?;

        let mut links = Vec::new();
        let mut rows = stmt.query([]).context("Failed to query for all links")?;
        while let Some(row) = rows.next()? {
            let tags_sql: String = row.get(3)?;
            links.push(CachedLink::new(
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                serde_json::from_str(&tags_sql)?,
                row.get(4)?,
            ));
        }
        Ok(links)
    }

    pub fn query_unarchived(&self) -> anyhow::Result<Vec<CachedLink>> {
        let mut stmt = self
            .conn
            .prepare("SELECT url, title, source, tags, parsed_content FROM cache WHERE archived_at IS NULL")
            .context("Failed to prepare query looking for unarchived links")?;

        let mut links = Vec::new();
        let mut rows = stmt.query([]).context("Failed to query for all links")?;
        while let Some(row) = rows.next()? {
            let tags_sql: String = row.get(3)?;
            links.push(CachedLink::new(
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                serde_json::from_str(&tags_sql)?,
                row.get(4)?,
            ));
        }
        Ok(links)
    }

    pub fn query_all_urls(&self) -> anyhow::Result<HashSet<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT url FROM cache")
            .context("Failed to prepare query for all URLs")?;

        let urls = stmt
            .query_map([], |row| row.get(0))
            .context("Failed to query all URLs")?
            .collect::<Result<HashSet<String>, _>>()?;

        Ok(urls)
    }

    pub fn insert(&self, link: &CachedLink) -> anyhow::Result<()> {
        let tags_sql = serde_json::to_string(&link.tags)?;
        self.conn.execute(
            "INSERT INTO cache (url, title, source, tags, parsed_content, fetch_failed_at) VALUES (:url, :title, :source, :tags, :parsed_content, NULL)
             ON CONFLICT(url) DO UPDATE SET
                title = excluded.title,
                source = excluded.source,
                tags = excluded.tags,
                parsed_content = excluded.parsed_content,
                fetch_failed_at = NULL",
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

    /// Record a link whose content could not be fetched, so it isn't retried on every run.
    /// Stored with `fetch_failed_at` set; `query` treats it as uncached again after
    /// `RETRY_AFTER_DAYS`. Distinct from `archived_at`, which tracks GoodLinks archival.
    pub fn insert_failed(&self, link: &CachedLink) -> anyhow::Result<()> {
        let tags_sql = serde_json::to_string(&link.tags)?;
        self.conn.execute(
            "INSERT INTO cache (url, title, source, tags, parsed_content, fetch_failed_at) VALUES (:url, :title, :source, :tags, :parsed_content, CURRENT_TIMESTAMP)
             ON CONFLICT(url) DO UPDATE SET
                title = excluded.title,
                source = excluded.source,
                tags = excluded.tags,
                parsed_content = excluded.parsed_content,
                fetch_failed_at = CURRENT_TIMESTAMP",
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

#[cfg(test)]
mod tests {
    use crate::models::LinkSource;

    use super::*;

    #[test]
    fn test_insert_and_query() -> anyhow::Result<()> {
        let cache = Cache::new(CacheType::Memory)?;
        let link = CachedLink {
            url: "https://example.com".to_string(),
            title: "Example".to_string(),
            source: LinkSource::GoodLinks,
            tags: Vec::new(),
            text_content: "Empty".to_string(),
        };
        cache.insert(&link)?;
        let query_result = cache.query(&link.url)?.unwrap();
        assert_eq!(link, query_result);

        Ok(())
    }

    #[test]
    fn test_insert_failed_is_skipped_until_retry_window_passes() -> anyhow::Result<()> {
        let cache = Cache::new(CacheType::Memory)?;
        let link = CachedLink {
            url: "https://example.com".to_string(),
            title: "Example".to_string(),
            source: LinkSource::GoodLinks,
            tags: Vec::new(),
            text_content: String::new(),
        };
        cache.insert_failed(&link)?;

        // Freshly archived: still treated as cached, so it won't be retried yet.
        assert!(cache.query(&link.url)?.is_some());

        // Backdate the failure past the retry window.
        cache.conn.execute(
            "UPDATE cache SET fetch_failed_at = datetime('now', '-31 days') WHERE url = :url",
            named_params![":url": link.url],
        )?;

        // Now it should look uncached, so it gets retried.
        assert!(cache.query(&link.url)?.is_none());

        Ok(())
    }

    #[test]
    fn test_insert_after_failed_retry_clears_fetch_failed_at() -> anyhow::Result<()> {
        let cache = Cache::new(CacheType::Memory)?;
        let link = CachedLink {
            url: "https://example.com".to_string(),
            title: "Example".to_string(),
            source: LinkSource::GoodLinks,
            tags: Vec::new(),
            text_content: String::new(),
        };
        cache.insert_failed(&link)?;
        cache.conn.execute(
            "UPDATE cache SET fetch_failed_at = datetime('now', '-31 days') WHERE url = :url",
            named_params![":url": link.url],
        )?;
        assert!(cache.query(&link.url)?.is_none());

        // A successful retry should overwrite the stale failure.
        let fetched = CachedLink {
            text_content: "Fetched content".to_string(),
            ..link.clone()
        };
        cache.insert(&fetched)?;
        let result = cache.query(&link.url)?.unwrap();
        assert_eq!(result.text_content, "Fetched content");

        Ok(())
    }

    #[test]
    fn test_query_empty() -> anyhow::Result<()> {
        let cache = Cache::new(CacheType::Memory)?;
        let query_result = cache.query("https://example.com")?;
        assert!(query_result.is_none());

        Ok(())
    }

    #[test]
    fn test_insert_and_query_all() -> anyhow::Result<()> {
        let cache = Cache::new(CacheType::Memory)?;
        let link1 = CachedLink {
            url: "https://example.com".to_string(),
            title: "Example 1".to_string(),
            source: LinkSource::GoodLinks,
            tags: Vec::new(),
            text_content: "Empty".to_string(),
        };

        let link2 = CachedLink {
            url: "https://example.com/sub".to_string(),
            title: "Example 2".to_string(),
            source: LinkSource::Obsidian,
            tags: Vec::new(),
            text_content: "Empty".to_string(),
        };
        cache.insert(&link1)?;
        cache.insert(&link2)?;
        let query_results = cache.query_all()?;
        assert_eq!(query_results.len(), 2);
        assert_eq!(query_results[0], link1);
        assert_eq!(query_results[1], link2);

        Ok(())
    }
}
