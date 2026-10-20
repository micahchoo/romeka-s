//! `romeka import`: a resource template from JSON, and items from CSV.
//!
//! ## Resource template JSON
//!
//! Upstream's API shape, so an exported template reads back:
//!
//! ```json
//! {
//!   "o:label": "Base Resource",
//!   "o:resource_class": {"o:id": 3},
//!   "o:resource_template_property": [
//!     {"o:property": {"o:id": 1}, "o:alternate_label": "Title",
//!      "o:position": 0, "o:data_type": ["literal"], "o:is_required": true}
//!   ]
//! }
//! ```
//!
//! Every key except `o:label` and `o:property` is optional.
//!
//! ## CSV items
//!
//! The first record is the header. Each header cell names a property as
//! `prefix:local_name` (`dcterms:title`, `foaf:name`). Each later record is one
//! item; a non-empty cell becomes a literal value on that property, and two
//! columns with the same term become two values. A column whose header starts
//! with `#` is a note for people and is ignored, so the `#reason` column of the
//! archive import's `left.csv` reads back. The item's denormalised
//! `title` column is the first non-empty `dcterms:title` cell. Every item joins
//! the item set named by `--item-set`. A row whose column count differs from the
//! header is reported and skipped; the rest import.
//!
//! A row imports whole or not at all: the item, its item-set link and its values
//! commit in one transaction, so a row reported as an error leaves nothing
//! behind. After the commit the item joins every site that keeps in sync with its
//! item pool and whose pool selects it, as an API create that names no `o:site`
//! does (`omeka_api::site_items`).

use std::path::PathBuf;

use serde::Deserialize;

use omeka_api::site_items;
use omeka_db::ids::{ItemId, ItemSetId, PropertyId};
use omeka_db::repos::{ClassRepo, ItemRepo, ItemSetRepo, PropertyRepo, TemplateRepo};
use omeka_db::rows::{
    NewItem, NewResource, NewTemplate, NewTemplateProperty, NewWholeItem, TemplateRow, ValueContent,
};
use omeka_db::{DbError, open_file};

use crate::csv;
use crate::error::CliError;
use crate::system_api;

/// Input for `romeka import template`.
#[derive(Debug, Clone)]
pub struct TemplateOptions {
    pub database: PathBuf,
    pub file: PathBuf,
}

/// Input for `romeka import csv`.
#[derive(Debug, Clone)]
pub struct CsvOptions {
    pub database: PathBuf,
    pub file: PathBuf,
    pub item_set_id: u64,
}

/// One row that did not import.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RowError {
    pub line: usize,
    pub message: String,
}

/// The outcome of a CSV import.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CsvReport {
    pub item_set: u64,
    pub imported: usize,
    pub errors: Vec<RowError>,
}

#[derive(Deserialize)]
struct TemplatePayload {
    #[serde(rename = "o:label")]
    label: String,
    #[serde(rename = "o:resource_class")]
    resource_class: Option<IdRef>,
    #[serde(rename = "o:resource_template_property", default)]
    properties: Vec<TemplatePropertyPayload>,
}

#[derive(Deserialize)]
struct TemplatePropertyPayload {
    #[serde(rename = "o:property")]
    property: IdRef,
    #[serde(rename = "o:alternate_label")]
    alternate_label: Option<String>,
    #[serde(rename = "o:alternate_comment")]
    alternate_comment: Option<String>,
    #[serde(rename = "o:position")]
    position: Option<i64>,
    #[serde(rename = "o:data_type")]
    data_type: Option<Vec<String>>,
    #[serde(rename = "o:is_required", default)]
    is_required: bool,
    #[serde(rename = "o:is_private", default)]
    is_private: bool,
    #[serde(rename = "o:default_lang")]
    default_lang: Option<String>,
}

#[derive(Deserialize)]
struct IdRef {
    #[serde(rename = "o:id")]
    id: u64,
}

/// Import a resource template from a JSON file.
///
/// # Errors
/// A [`CliError::Io`] when the file cannot be read, a [`CliError::Json`] when it
/// is not the expected shape, or a [`CliError::Message`] for an empty label, a
/// duplicate label, or a property id that does not exist.
pub async fn import_template(options: &TemplateOptions) -> Result<TemplateRow, CliError> {
    let payload: TemplatePayload = serde_json::from_str(&std::fs::read_to_string(&options.file)?)?;
    if payload.label.trim().is_empty() {
        return Err(CliError::message("the template label is empty"));
    }

    let pool = open_file(&options.database).await?;
    let templates = TemplateRepo::new(&pool);
    if templates.find_by_label(&payload.label).await?.is_some() {
        return Err(CliError::message(format!(
            "a template labelled \"{}\" already exists",
            payload.label
        )));
    }

    let resource_class_id = match payload.resource_class {
        Some(reference) => {
            let id = omeka_db::ids::ClassId::from(reference.id);
            if ClassRepo::new(&pool).get(id).await?.is_none() {
                return Err(CliError::message(format!(
                    "resource class {} does not exist",
                    reference.id
                )));
            }
            Some(id)
        }
        None => None,
    };

    let properties = PropertyRepo::new(&pool);
    let mut resolved = Vec::with_capacity(payload.properties.len());
    for field in payload.properties {
        let id = PropertyId::from(field.property.id);
        if properties.get(id).await?.is_none() {
            return Err(CliError::message(format!(
                "property {} does not exist",
                field.property.id
            )));
        }
        resolved.push((id, field));
    }

    let template = templates
        .create(&NewTemplate {
            owner_id: None,
            resource_class_id,
            title_property_id: None,
            description_property_id: None,
            label: payload.label,
        })
        .await?;

    for (position, (property_id, field)) in resolved.into_iter().enumerate() {
        templates
            .add_property(&NewTemplateProperty {
                resource_template_id: template.id,
                property_id,
                alternate_label: field.alternate_label,
                alternate_comment: field.alternate_comment,
                position: Some(field.position.unwrap_or(position as i64)),
                data_type: field.data_type,
                is_required: field.is_required,
                is_private: field.is_private,
                default_lang: field.default_lang,
            })
            .await?;
    }
    Ok(template)
}

/// Import items from a CSV into an item set.
///
/// # Errors
/// A [`CliError::Io`] when the file cannot be read, a [`CliError::Parse`] for a
/// malformed CSV, or a [`CliError::Message`] for an unknown header term or a
/// missing item set.
pub async fn import_csv(options: &CsvOptions) -> Result<CsvReport, CliError> {
    let pool = open_file(&options.database).await?;
    let item_set_id = ItemSetId::from(options.item_set_id);
    if ItemSetRepo::new(&pool).get(item_set_id).await?.is_none() {
        return Err(CliError::message(format!(
            "item set {} does not exist",
            options.item_set_id
        )));
    }

    let records = csv::parse(&std::fs::read_to_string(&options.file)?)?;
    let Some(header) = records.first() else {
        return Err(CliError::message("the CSV is empty"));
    };
    let columns = resolve_columns(&pool, &header.fields).await?;

    let items = ItemRepo::new(&pool);
    let api = system_api::load(&pool).await?;
    let mut report = CsvReport {
        item_set: options.item_set_id,
        imported: 0,
        errors: Vec::new(),
    };

    for record in records.iter().skip(1) {
        if record.fields.len() != columns.len() {
            report.errors.push(RowError {
                line: record.line,
                message: format!(
                    "expected {} columns, found {}",
                    columns.len(),
                    record.fields.len()
                ),
            });
            continue;
        }
        match import_row(&items, item_set_id, &columns, &record.fields).await {
            Ok(item) => {
                site_items::attach_on_create(&api, item)
                    .await
                    .map_err(|error| {
                        CliError::message(format!(
                            "line {}: item {} imported, but joining its synced sites failed: {error}",
                            record.line, item.0
                        ))
                    })?;
                report.imported += 1;
            }
            Err(error) => report.errors.push(RowError {
                line: record.line,
                message: error.to_string(),
            }),
        }
    }
    Ok(report)
}

/// One resolved header column; `None` in the list is a `#` column, ignored.
struct Column {
    property: PropertyId,
    is_title: bool,
}

async fn resolve_columns(
    pool: &sqlx::SqlitePool,
    header: &[String],
) -> Result<Vec<Option<Column>>, CliError> {
    let mut columns = Vec::with_capacity(header.len());
    for (index, cell) in header.iter().enumerate() {
        if cell.starts_with('#') {
            columns.push(None);
            continue;
        }
        let property = crate::terms::resolve_property(pool, cell)
            .await
            .map_err(|message| {
                CliError::message(format!("header column {}: {message}", index + 1))
            })?;
        let (prefix, local_name) = cell.split_once(':').unwrap_or(("", cell));
        columns.push(Some(Column {
            property,
            is_title: prefix == "dcterms" && local_name == "title",
        }));
    }
    Ok(columns)
}

async fn import_row(
    items: &ItemRepo<'_>,
    item_set_id: ItemSetId,
    columns: &[Option<Column>],
    fields: &[String],
) -> Result<ItemId, DbError> {
    let mut values = Vec::new();
    let mut title = None;
    for (column, field) in columns.iter().zip(fields.iter()) {
        let Some(column) = column else {
            continue;
        };
        let text = field.trim();
        if text.is_empty() {
            continue;
        }
        if column.is_title && title.is_none() {
            title = Some(text.to_string());
        }
        values.push(ValueContent::literal(column.property, text));
    }

    let item = items
        .create_whole(&NewWholeItem {
            item: NewItem {
                resource: NewResource {
                    title,
                    is_public: true,
                    ..NewResource::default()
                },
                primary_media_id: None,
            },
            item_sets: vec![item_set_id],
            values,
        })
        .await?;
    Ok(item.id())
}

#[cfg(test)]
mod tests;
