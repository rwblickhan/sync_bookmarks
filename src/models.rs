use rusqlite::{
    types::{FromSql, FromSqlError, ToSqlOutput, Value, ValueRef},
    ToSql,
};
use serde::Serialize;

#[derive(serde::Serialize, serde::Deserialize, PartialEq, Eq, Hash, Debug)]
pub enum LinkSource {
    GoodLinks,
    Obsidian,
}

impl FromSql for LinkSource {
    fn column_result(value: ValueRef) -> std::result::Result<LinkSource, FromSqlError> {
        match value.as_str()? {
            "GoodLinks" => Ok(LinkSource::GoodLinks),
            "Obsidian" => Ok(LinkSource::Obsidian),
            _ => Err(FromSqlError::InvalidType),
        }
    }
}

impl ToSql for LinkSource {
    fn to_sql(&self) -> std::result::Result<ToSqlOutput<'_>, rusqlite::Error> {
        match self {
            LinkSource::GoodLinks => Ok(ToSqlOutput::Owned(Value::Text("GoodLinks".into()))),
            LinkSource::Obsidian => Ok(ToSqlOutput::Owned(Value::Text("Obsidian".into()))),
        }
    }
}

#[derive(PartialEq, Eq, Debug)]
pub struct CachedLink {
    pub url: String,
    pub title: String,
    pub source: LinkSource,
    pub tags: Vec<String>,
    pub text_content: String,
}

impl CachedLink {
    pub fn new(
        url: String,
        title: String,
        source: LinkSource,
        tags: Vec<String>,
        text_content: String,
    ) -> Self {
        CachedLink {
            url,
            title,
            source,
            tags,
            text_content,
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, PartialEq, Eq, Hash)]
pub struct SerializedLink {
    pub url: String,
    pub title: String,
    pub tags: Vec<String>,
    pub source: LinkSource,
}

impl From<GoodLinksLink> for SerializedLink {
    fn from(val: GoodLinksLink) -> Self {
        SerializedLink {
            url: val.url,
            title: val.title.unwrap_or_default(),
            tags: val.tags,
            source: LinkSource::GoodLinks,
        }
    }
}

impl From<ObsidianLink> for SerializedLink {
    fn from(val: ObsidianLink) -> Self {
        SerializedLink {
            url: val.url,
            title: val.title,
            tags: Vec::new(),
            source: LinkSource::Obsidian,
        }
    }
}

#[derive(serde::Deserialize)]
pub struct GoodLinksLink {
    #[serde(rename = "readAt")]
    pub read_at: Option<f32>,
    pub title: Option<String>,
    pub tags: Vec<String>,
    pub url: String,
}

pub struct ObsidianLink {
    pub title: String,
    pub url: String,
}

pub struct Article {
    pub title: String,
    pub text_content: String,
}
