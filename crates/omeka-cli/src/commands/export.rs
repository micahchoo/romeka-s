//! `romeka export`: the JSON-LD the read layer serves, and a CSV of item values.
//!
//! ## JSON-LD
//!
//! `romeka export jsonld` writes the documents `/api/{resource}` answers, built
//! by `omeka-api`'s read layer, so no second serializer exists. To an existing
//! directory it writes one file per document, `<resource>-<o:id>.json`; to any
//! other path it writes one JSON array. The `@id` of every document is built
//! from `--base-url`, because the same document served over HTTP carries the
//! request's origin.
//!
//! ## CSV
//!
//! `romeka export csv` writes [`export::items_csv`], the file the admin
//! downloads from `GET /api/romeka/items.csv`, so both share one column set.
//! That module states the column set. This command adds `--item-set`, and
//! `--columns`, which names the property terms (each one
//! [`crate::terms::resolve_property`] resolves) in the order given.

use std::path::{Path, PathBuf};

use omeka_api::auth::Actor;
use omeka_api::jsonld::BaseUrl;
use omeka_api::read::export::{self, Selection};
use omeka_api::read::{Api, Params};
use omeka_db::Location;
use omeka_db::ids::UserId;

use crate::error::CliError;
use crate::terms;

/// Input for `romeka export jsonld`.
#[derive(Debug, Clone)]
pub struct JsonldOptions {
    pub database: Location,
    pub out: PathBuf,
    /// An [`export::resources`] name.
    pub resource: String,
    pub item_set: Option<u64>,
    /// The origin every `@id` is built from, as `romeka serve --base-url`.
    pub base_url: String,
}

/// Input for `romeka export csv`.
#[derive(Debug, Clone)]
pub struct CsvOptions {
    pub database: Location,
    pub out: PathBuf,
    pub item_set: Option<u64>,
    /// The property terms to export, in order. Empty derives the column set
    /// from the data.
    pub columns: Vec<String>,
}

/// The outcome of `romeka export jsonld`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonldReport {
    pub resource: String,
    pub documents: usize,
    pub out: PathBuf,
    /// True when `--out` was an existing directory and each document got a file.
    pub per_file: bool,
}

/// The outcome of `romeka export csv`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CsvReport {
    pub items: usize,
    /// The header, with a term repeated once per value column.
    pub columns: Vec<String>,
    pub out: PathBuf,
}

/// The actor an export runs as.
///
/// The command owns the database file, so it exports as a global admin and
/// sees private rows. Upstream checks `users` against an authenticated caller
/// (`AbstractAdapter` ACL), so the id has to be present; only the role drives
/// visibility, so the placeholder id is never compared.
fn export_actor() -> Actor {
    Actor::new(UserId(0), "global_admin")
}

/// The base URL a CSV export builds documents with. CSV never reads `@id`, so
/// any valid origin does.
const CSV_BASE_URL: &str = "http://localhost";

/// Write the documents the list endpoint for `options.resource` serves.
///
/// # Errors
/// A [`CliError::Message`] for an unknown resource or a bad base URL, the read
/// layer's error for the resource, or a [`CliError::Io`] / [`CliError::Json`]
/// when the output cannot be written.
pub async fn run_jsonld(options: &JsonldOptions) -> Result<JsonldReport, CliError> {
    if !export::resources().contains(&options.resource.as_str()) {
        return Err(CliError::message(format!(
            "unknown --resource \"{}\"; try one of {}",
            options.resource,
            export::resources().join(", ")
        )));
    }
    if options.item_set.is_some() && options.resource != "items" {
        return Err(CliError::message(format!(
            "--item-set filters items, so it needs --resource items, not {}",
            options.resource
        )));
    }
    let base = BaseUrl::parse(&options.base_url)
        .map_err(|error| CliError::message(format!("invalid --base-url: {error}")))?;
    let pool = options.database.open().await?;
    let selection = Selection {
        resource: options.resource.clone(),
        item_set: options.item_set,
    };
    let documents = export::documents(&pool, base, export_actor(), &selection).await?;
    let per_file = write_documents(&documents, &options.resource, &options.out)?;
    Ok(JsonldReport {
        resource: options.resource.clone(),
        documents: documents.len(),
        out: options.out.clone(),
        per_file,
    })
}

/// Write the items as CSV, with a header row.
///
/// # Errors
/// A [`CliError::Message`] for an unknown column term, the read layer's error,
/// or a [`CliError::Io`] when the file cannot be written.
pub async fn run_csv(options: &CsvOptions) -> Result<CsvReport, CliError> {
    let pool = options.database.open().await?;
    for term in &options.columns {
        terms::resolve_property(&pool, term)
            .await
            .map_err(|message| CliError::message(format!("--columns {term}: {message}")))?;
    }
    let base = BaseUrl::parse(CSV_BASE_URL)
        .map_err(|error| CliError::message(format!("invalid base URL: {error}")))?;
    let query = Params::from_decoded(
        options
            .item_set
            .map(|id| ("item_set_id".to_string(), id.to_string()))
            .into_iter()
            .collect(),
    );
    let api = Api::load(&pool, base, export_actor()).await?;
    let file = export::items_csv(&api, &query, &options.columns).await?;
    create_parent(&options.out)?;
    std::fs::write(&options.out, &file.text)?;
    Ok(CsvReport {
        items: file.items,
        columns: file.columns,
        out: options.out.clone(),
    })
}

/// Write one array, or one file per document when `out` is a directory.
fn write_documents(
    documents: &[serde_json::Value],
    resource: &str,
    out: &Path,
) -> Result<bool, CliError> {
    if out.is_dir() {
        for (position, document) in documents.iter().enumerate() {
            let name = document_name(document, position + 1);
            let path = out.join(format!("{resource}-{name}.json"));
            write_json(&path, document)?;
        }
        return Ok(true);
    }
    let array = serde_json::Value::Array(documents.to_vec());
    write_json(out, &array)?;
    Ok(false)
}

fn write_json(path: &Path, value: &serde_json::Value) -> Result<(), CliError> {
    create_parent(path)?;
    std::fs::write(path, format!("{}\n", serde_json::to_string_pretty(value)?))?;
    Ok(())
}

fn create_parent(path: &Path) -> Result<(), CliError> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)?;
    }
    Ok(())
}

/// The file stem for one document: its `o:id`, made safe for a file name, and
/// the document's position when it has none.
fn document_name(document: &serde_json::Value, position: usize) -> String {
    let id = match document.get("o:id") {
        Some(serde_json::Value::String(id)) => id.clone(),
        Some(serde_json::Value::Number(id)) => id.to_string(),
        _ => position.to_string(),
    };
    let safe: String = id
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.') {
                character
            } else {
                '_'
            }
        })
        .collect();
    if safe.is_empty() {
        position.to_string()
    } else {
        safe
    }
}

#[cfg(test)]
mod tests;
