use rusqlite::types::{FromSql, FromSqlError};

pub enum LinkSource {
    GoodLinks,
    Obsidian,
}

impl FromSql for LinkSource {
    fn column_result(
        value: rusqlite::types::ValueRef,
    ) -> std::result::Result<LinkSource, FromSqlError> {
        match value.as_str()? {
            "GoodLinks" => Ok(LinkSource::GoodLinks),
            "Obsidian" => Ok(LinkSource::Obsidian),
            _ => Err(FromSqlError::InvalidType),
        }
    }
}

pub struct ParsedLink {
    url: String,
    title: String,
    source: LinkSource,
    tags: Vec<String>,
    text_content: String,
}

impl ParsedLink {
    pub fn new(
        url: String,
        title: String,
        source: LinkSource,
        tags: Vec<String>,
        text_content: String,
    ) -> Self {
        ParsedLink {
            url,
            title,
            source,
            tags,
            text_content,
        }
    }
}
