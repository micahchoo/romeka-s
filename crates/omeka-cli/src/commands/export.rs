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
//! Upstream at the frozen SHA has no CSV export. `Admin/*Export*` does not
//! exist; the only export is `Admin/ResourceTemplateController::exportAction`,
//! which writes one resource template as JSON. So there is no upstream column
//! set to match. This command defines one:
//!
//! - One row per item, one header cell per value column.
//! - A cell names a property as `prefix:local_name`, the term
//!   [`crate::terms::resolve_property`] resolves.
//! - The default column set is every property that holds a value on any
//!   exported item: `dcterms:title` first, then the rest by term. `--columns`
//!   names them instead, in the order given.
//! - A property with several values on any item gets one column per value, and
//!   the header repeats the term, so a multi-valued item keeps every value.
//! - Values flatten to text: a literal is its `@value`, a URI its `@id`, a
//!   resource value its display title. A language tag is dropped.
//!
//! The column set is a port decision, not an upstream one. It is recorded here
//! and in `crates/omeka-cli/README.md`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use omeka_api::auth::Actor;
use omeka_api::jsonld::BaseUrl;
use omeka_api::read::export::{self, Selection};
use omeka_api::wire::Value;
use omeka_api::wire::item::Item as ItemDocument;
use omeka_db::ids::UserId;
use omeka_db::open_file;

use crate::csv;
use crate::error::CliError;
use crate::terms;

/// Input for `romeka export jsonld`.
#[derive(Debug, Clone)]
pub struct JsonldOptions {
    pub database: PathBuf,
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
    pub database: PathBuf,
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
    let pool = open_file(&options.database).await?;
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
    let pool = open_file(&options.database).await?;
    for term in &options.columns {
        terms::resolve_property(&pool, term)
            .await
            .map_err(|message| CliError::message(format!("--columns {term}: {message}")))?;
    }
    let base = BaseUrl::parse(CSV_BASE_URL)
        .map_err(|error| CliError::message(format!("invalid base URL: {error}")))?;
    let selection = Selection {
        resource: "items".to_string(),
        item_set: options.item_set,
    };
    let documents = export::documents(&pool, base, export_actor(), &selection).await?;
    let items = item_rows(&documents)?;
    let columns = columns(&items, &options.columns);
    let text = csv::render(&csv_rows(&items, &columns));
    write_output(&options.out, &text)?;
    Ok(CsvReport {
        items: items.len(),
        columns: columns.iter().map(|column| column.term.clone()).collect(),
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

fn write_output(path: &Path, text: &str) -> Result<(), CliError> {
    create_parent(path)?;
    std::fs::write(path, text)?;
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

/// One item's values, keyed by property term.
type ItemRow = BTreeMap<String, Vec<String>>;

/// Project each item document onto its values.
fn item_rows(documents: &[serde_json::Value]) -> Result<Vec<ItemRow>, CliError> {
    let mut rows = Vec::with_capacity(documents.len());
    for document in documents {
        let item: ItemDocument = serde_json::from_value(document.clone())?;
        let mut row = ItemRow::new();
        for (term, values) in &item.values {
            row.insert(term.clone(), values.iter().map(flatten).collect());
        }
        rows.push(row);
    }
    Ok(rows)
}

/// One value as text: a literal is its `@value`, a URI its `@id`, a resource
/// value its display title, or its `@id` when it has no title.
fn flatten(value: &Value) -> String {
    match value {
        Value::Literal(literal) => literal.value.clone(),
        Value::Uri(uri) => uri.id.clone(),
        Value::Resource(resource) => resource
            .display_title
            .clone()
            .unwrap_or_else(|| resource.id.clone()),
    }
}

/// One header column: a property term and which of its values it carries.
struct Column {
    term: String,
    index: usize,
}

/// The header and its repeats.
///
/// `dcterms:title` leads the derived set because it is the item's denormalised
/// title, which is the first non-empty title cell.
fn columns(rows: &[ItemRow], requested: &[String]) -> Vec<Column> {
    let terms = column_terms(rows, requested);
    let mut columns = Vec::new();
    for term in terms {
        let count = rows
            .iter()
            .map(|row| row.get(&term).map_or(0, Vec::len))
            .max()
            .unwrap_or(0)
            .max(1);
        for index in 0..count {
            columns.push(Column {
                term: term.clone(),
                index,
            });
        }
    }
    columns
}

/// The property terms in export order: the requested list, or every term that
/// holds a value, title first.
fn column_terms(rows: &[ItemRow], requested: &[String]) -> Vec<String> {
    if !requested.is_empty() {
        return requested.to_vec();
    }
    let mut terms: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for row in rows {
        terms.extend(row.keys().cloned());
    }
    let mut ordered = Vec::with_capacity(terms.len());
    if terms.remove("dcterms:title") {
        ordered.push("dcterms:title".to_string());
    }
    ordered.extend(terms);
    ordered
}

/// The header row and one row per item.
fn csv_rows(rows: &[ItemRow], columns: &[Column]) -> Vec<Vec<String>> {
    let mut records = vec![columns.iter().map(|column| column.term.clone()).collect()];
    for row in rows {
        let mut record = Vec::with_capacity(columns.len());
        for column in columns {
            let text = row
                .get(&column.term)
                .and_then(|values| values.get(column.index))
                .cloned()
                .unwrap_or_default();
            record.push(text);
        }
        records.push(record);
    }
    records
}

#[cfg(test)]
mod tests;
