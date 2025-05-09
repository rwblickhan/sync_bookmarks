use rusqlite::{
    types::{FromSql, FromSqlError, ToSqlOutput, Value, ValueRef},
    ToSql,
};

#[derive(serde::Serialize)]
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

pub struct ParsedLink {
    pub url: String,
    pub title: String,
    pub source: LinkSource,
    pub tags: Vec<String>,
    pub text_content: String,
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
