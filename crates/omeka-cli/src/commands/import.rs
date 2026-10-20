//! `romeka import`: the importer's second front end, and a resource template
//! from JSON.
//!
//! `run`, `undo` and `list` call the same `omeka_api::import2` functions the
//! routes do and run the job bodies in this process
//! (`docs/import-v2.md`, "Command line"). `template` reads one resource
//! template.
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

use std::path::PathBuf;

use serde::Deserialize;

use omeka_db::Location;
use omeka_db::ids::PropertyId;
use omeka_db::repos::{ClassRepo, PropertyRepo, TemplateRepo};
use omeka_db::rows::{NewTemplate, NewTemplateProperty, TemplateRow};
use omeka_db::with_db;

use crate::error::CliError;

pub mod list;
pub mod run;
mod session;
pub mod undo;

pub use session::{Format, Where};

/// Input for `romeka import template`.
#[derive(Debug, Clone)]
pub struct TemplateOptions {
    pub database: Location,
    pub file: PathBuf,
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

    let pool = options.database.open().await?;
    if with_db!(&pool, |db| TemplateRepo::new(db)
        .find_by_label(&payload.label)
        .await)?
    .is_some()
    {
        return Err(CliError::message(format!(
            "a template labelled \"{}\" already exists",
            payload.label
        )));
    }

    let resource_class_id = match payload.resource_class {
        Some(reference) => {
            let id = omeka_db::ids::ClassId::from(reference.id);
            if with_db!(&pool, |db| ClassRepo::new(db).get(id).await)?.is_none() {
                return Err(CliError::message(format!(
                    "resource class {} does not exist",
                    reference.id
                )));
            }
            Some(id)
        }
        None => None,
    };

    let mut resolved = Vec::with_capacity(payload.properties.len());
    for field in payload.properties {
        let id = PropertyId::from(field.property.id);
        if with_db!(&pool, |db| PropertyRepo::new(db).get(id).await)?.is_none() {
            return Err(CliError::message(format!(
                "property {} does not exist",
                field.property.id
            )));
        }
        resolved.push((id, field));
    }

    let template = with_db!(&pool, |db| TemplateRepo::new(db)
        .create(&NewTemplate {
            owner_id: None,
            resource_class_id,
            title_property_id: None,
            description_property_id: None,
            label: payload.label,
        })
        .await)?;

    for (position, (property_id, field)) in resolved.into_iter().enumerate() {
        with_db!(&pool, |db| TemplateRepo::new(db)
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
            .await)?;
    }
    Ok(template)
}

#[cfg(test)]
mod tests;
