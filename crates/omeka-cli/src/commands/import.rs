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
//! columns with the same term become two values. The item's denormalised
//! `title` column is the first non-empty `dcterms:title` cell. Every item joins
//! the item set named by `--item-set`. A row whose column count differs from the
//! header is reported and skipped; the rest import.

use std::path::PathBuf;

use serde::Deserialize;

use omeka_db::ids::{ItemSetId, PropertyId};
use omeka_db::open_file;
use omeka_db::repos::{ClassRepo, ItemRepo, ItemSetRepo, PropertyRepo, TemplateRepo, ValueRepo};
use omeka_db::rows::{
    NewItem, NewResource, NewTemplate, NewTemplateProperty, NewValue, TemplateRow,
};

use crate::csv;
use crate::error::CliError;

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
    let values = ValueRepo::new(&pool);
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
        match import_row(&items, &values, item_set_id, &columns, &record.fields).await {
            Ok(()) => report.imported += 1,
            Err(error) => report.errors.push(RowError {
                line: record.line,
                message: error.to_string(),
            }),
        }
    }
    Ok(report)
}

/// One resolved header column.
struct Column {
    property: PropertyId,
    is_title: bool,
}

async fn resolve_columns(
    pool: &sqlx::SqlitePool,
    header: &[String],
) -> Result<Vec<Column>, CliError> {
    let mut columns = Vec::with_capacity(header.len());
    for (index, cell) in header.iter().enumerate() {
        let property = crate::terms::resolve_property(pool, cell)
            .await
            .map_err(|message| {
                CliError::message(format!("header column {}: {message}", index + 1))
            })?;
        let (prefix, local_name) = cell.split_once(':').unwrap_or(("", cell));
        columns.push(Column {
            property,
            is_title: prefix == "dcterms" && local_name == "title",
        });
    }
    Ok(columns)
}

async fn import_row(
    items: &ItemRepo<'_>,
    values: &ValueRepo<'_>,
    item_set_id: ItemSetId,
    columns: &[Column],
    fields: &[String],
) -> Result<(), CliError> {
    let mut cells = Vec::new();
    let mut title = None;
    for (column, field) in columns.iter().zip(fields.iter()) {
        let text = field.trim();
        if text.is_empty() {
            continue;
        }
        if column.is_title && title.is_none() {
            title = Some(text.to_string());
        }
        cells.push((column.property, text.to_string()));
    }

    let item = items
        .create(&NewItem {
            resource: NewResource {
                title,
                is_public: true,
                ..NewResource::default()
            },
            primary_media_id: None,
        })
        .await?;
    items.add_to_item_set(item.id(), item_set_id).await?;
    for (property, text) in cells {
        values
            .create(&NewValue {
                resource_id: item.resource.id,
                property_id: property,
                value_resource_id: None,
                value_annotation_id: None,
                r#type: "literal".to_string(),
                lang: None,
                value: Some(text),
                uri: None,
                is_public: true,
            })
            .await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{seeded_db, temp_db};
    use omeka_db::ids::PropertyId;
    use omeka_db::repos::{ItemFilter, Paging};

    async fn write(dir: &std::path::Path, name: &str, contents: &str) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, contents).expect("write fixture");
        path
    }

    async fn title_property(pool: &sqlx::SqlitePool) -> PropertyId {
        crate::terms::resolve_property(pool, "dcterms:title")
            .await
            .expect("dcterms:title")
    }

    #[tokio::test]
    async fn template_import_creates_the_template_and_its_fields() {
        let db = seeded_db().await;
        let title = title_property(&db.pool).await;
        let creator = crate::terms::resolve_property(&db.pool, "dcterms:creator")
            .await
            .expect("dcterms:creator");
        let json = format!(
            r#"{{"o:label":"Imported","o:resource_template_property":[
                {{"o:property":{{"o:id":{}}}, "o:alternate_label":"Title", "o:is_required":true}},
                {{"o:property":{{"o:id":{}}}, "o:data_type":["literal"]}}
            ]}}"#,
            title.0, creator.0
        );
        let file = write(db.path.parent().expect("dir"), "template.json", &json).await;

        let template = import_template(&TemplateOptions {
            database: db.path.clone(),
            file,
        })
        .await
        .expect("import template");
        assert_eq!(template.label, "Imported");

        let fields = TemplateRepo::new(&db.pool)
            .list_properties(template.id)
            .await
            .expect("list properties");
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].alternate_label.as_deref(), Some("Title"));
        assert!(fields[0].is_required);
        assert_eq!(fields[1].position, Some(1));
    }

    #[tokio::test]
    async fn template_import_rejects_a_duplicate_label() {
        let db = seeded_db().await;
        let json = r#"{"o:label":"Base Resource"}"#;
        let file = write(db.path.parent().expect("dir"), "dupe.json", json).await;
        let error = import_template(&TemplateOptions {
            database: db.path.clone(),
            file,
        })
        .await
        .expect_err("duplicate label");
        assert!(error.to_string().contains("already exists"), "got {error}");
    }

    #[tokio::test]
    async fn csv_import_creates_items_with_values_in_the_item_set() {
        let db = seeded_db().await;
        let item_set = omeka_db::repos::ItemSetRepo::new(&db.pool)
            .create(&omeka_db::rows::NewItemSet {
                resource: NewResource {
                    title: Some("Set".to_string()),
                    ..NewResource::default()
                },
                is_open: false,
            })
            .await
            .expect("create item set");
        let csv = "dcterms:title,dcterms:creator\nFirst work,Ada\nSecond work,Alan\n";
        let file = write(db.path.parent().expect("dir"), "items.csv", csv).await;

        let report = import_csv(&CsvOptions {
            database: db.path.clone(),
            file,
            item_set_id: item_set.id().0,
        })
        .await
        .expect("import csv");
        assert_eq!(report.imported, 2);
        assert!(report.errors.is_empty(), "errors: {:?}", report.errors);

        let items = ItemRepo::new(&db.pool);
        let listed = items
            .list(
                &ItemFilter {
                    item_set_id: Some(item_set.id()),
                    ..ItemFilter::default()
                },
                &Paging::default(),
            )
            .await
            .expect("list items");
        assert_eq!(listed.len(), 2);
        assert_eq!(listed[0].resource.title.as_deref(), Some("First work"));

        let values = ValueRepo::new(&db.pool)
            .list_for_resource(listed[0].resource.id)
            .await
            .expect("values");
        assert_eq!(values.len(), 2);
        assert!(values.iter().any(|v| v.value.as_deref() == Some("Ada")));
    }

    #[tokio::test]
    async fn csv_import_reports_a_short_row_and_keeps_the_rest() {
        let db = seeded_db().await;
        let item_set = omeka_db::repos::ItemSetRepo::new(&db.pool)
            .create(&omeka_db::rows::NewItemSet::default())
            .await
            .expect("create item set");
        let csv = "dcterms:title,dcterms:creator\nGood,Ada\nBroken\nAlso good,Alan\n";
        let file = write(db.path.parent().expect("dir"), "short.csv", csv).await;

        let report = import_csv(&CsvOptions {
            database: db.path.clone(),
            file,
            item_set_id: item_set.id().0,
        })
        .await
        .expect("import csv");
        assert_eq!(report.imported, 2);
        assert_eq!(report.errors.len(), 1);
        assert_eq!(report.errors[0].line, 3);
        assert!(report.errors[0].message.contains("expected 2 columns"));
    }

    #[tokio::test]
    async fn csv_import_fails_on_an_unknown_header_term() {
        let db = seeded_db().await;
        let item_set = omeka_db::repos::ItemSetRepo::new(&db.pool)
            .create(&omeka_db::rows::NewItemSet::default())
            .await
            .expect("create item set");
        let csv = "nope:title\nvalue\n";
        let file = write(db.path.parent().expect("dir"), "bad.csv", csv).await;
        let error = import_csv(&CsvOptions {
            database: db.path.clone(),
            file,
            item_set_id: item_set.id().0,
        })
        .await
        .expect_err("unknown prefix");
        assert!(
            error.to_string().contains("unknown vocabulary prefix"),
            "got {error}"
        );
    }

    #[tokio::test]
    async fn csv_import_fails_when_the_item_set_is_missing() {
        let db = temp_db().await;
        let file = write(
            db.path.parent().expect("dir"),
            "x.csv",
            "dcterms:title\nA\n",
        )
        .await;
        let error = import_csv(&CsvOptions {
            database: db.path.clone(),
            file,
            item_set_id: 999,
        })
        .await
        .expect_err("missing item set");
        assert!(error.to_string().contains("does not exist"), "got {error}");
    }
}
