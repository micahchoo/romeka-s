//! Value hydration for the resource-entity writes.
//!
//! `ValueHydrator::hydrate` walks every top-level key that carries an array of
//! value objects. Each object names a property (`property_id`, or `"auto"` to
//! take the term from its key), a data type (`type`, `literal` when absent),
//! and the object of the triple (`@value`, `@id`, or `value_resource_id`). An
//! object with no usable property or object is ignored, as upstream ignores an
//! invalid value. `ResourceTitleHydrator` then takes the resource title from the
//! title property's first non-empty value.

use omeka_db::ids::{PropertyId, ResourceId, TemplateId};
use omeka_db::repos::{TemplateRepo, ValueRepo};
use omeka_db::rows::NewValue;
use serde_json::Value;

use crate::errors::{ApiError, ValidationErrors};
use crate::read::{Api, db_error};
use crate::write::body::Payload;

/// Top-level members that are never property terms. JSON-LD keywords are
/// ignored the way upstream ignores them (it only reads the members it knows).
const JSONLD_KEYWORDS: &[&str] = &[
    "@id",
    "@type",
    "@context",
    "clear_property_values",
    "set_value_visibility",
    "convert_data_types",
];

/// One value object reduced to the columns it sets.
#[derive(Debug, Clone, PartialEq)]
pub struct ValueInput {
    pub property: PropertyId,
    pub data: ValueKind,
    pub lang: Option<String>,
    pub is_public: bool,
}

/// The three data type shapes the port hydrates.
#[derive(Debug, Clone, PartialEq)]
pub enum ValueKind {
    Literal(String),
    Uri { uri: String, label: Option<String> },
    Resource(ResourceId),
}

/// Collect every value object in the body for the given resource.
///
/// A top-level key that is not a control member and not a known property term
/// is a validation error keyed by that member. An array member with no valid
/// value object contributes nothing.
pub fn collect(
    api: &Api<'_>,
    payload: &Payload,
    control: &[&str],
) -> Result<Vec<ValueInput>, ApiError> {
    let mut inputs = Vec::new();
    let mut errors = ValidationErrors::new();
    for (key, value) in payload.members() {
        if control.contains(&key.as_str()) || JSONLD_KEYWORDS.contains(&key.as_str()) {
            continue;
        }
        if !api.catalog().knows_term(key) {
            errors
                .entry(key.clone())
                .or_default()
                .push(format!("The property \"{key}\" is not recognized."));
            continue;
        }
        let Some(array) = value.as_array() else {
            continue;
        };
        for item in array {
            if let Some(input) = parse_one(api, key, item) {
                inputs.push(input);
            }
        }
    }
    if errors.is_empty() {
        Ok(inputs)
    } else {
        Err(ApiError::Validation(errors))
    }
}

fn parse_one(api: &Api<'_>, term: &str, value: &Value) -> Option<ValueInput> {
    let object = value.as_object()?;
    let property = match object.get("property_id") {
        Some(Value::String(name)) if name == "auto" => api.catalog().property_id(term)?,
        Some(Value::Number(number)) => PropertyId(number.as_u64()?),
        _ => return None,
    };
    let is_public = object
        .get("is_public")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let lang = object
        .get("@language")
        .and_then(Value::as_str)
        .map(str::to_string);
    let data = match object.get("type").and_then(Value::as_str).unwrap_or("") {
        "uri" => {
            let uri = object.get("@id").and_then(Value::as_str)?.to_string();
            let label = object
                .get("o:label")
                .and_then(Value::as_str)
                .map(str::to_string);
            ValueKind::Uri { uri, label }
        }
        kind if kind.starts_with("resource") => {
            let id = object.get("value_resource_id").and_then(Value::as_u64)?;
            ValueKind::Resource(ResourceId(id))
        }
        _ => {
            let text = object.get("@value").and_then(Value::as_str)?.to_string();
            ValueKind::Literal(text)
        }
    };
    Some(ValueInput {
        property,
        data,
        lang,
        is_public,
    })
}

/// Replace a resource's values with `inputs`.
pub async fn store(
    api: &Api<'_>,
    resource: ResourceId,
    inputs: &[ValueInput],
) -> Result<(), ApiError> {
    clear(api, resource).await?;
    let repo = ValueRepo::new(api.pool());
    for input in inputs {
        let (r#type, value, uri, value_resource_id) = match &input.data {
            ValueKind::Literal(text) => ("literal", Some(text.clone()), None, None),
            ValueKind::Uri { uri, label } => ("uri", label.clone(), Some(uri.clone()), None),
            ValueKind::Resource(id) => ("resource", None, None, Some(*id)),
        };
        repo.create(&NewValue {
            resource_id: resource,
            property_id: input.property,
            value_resource_id,
            value_annotation_id: None,
            r#type: r#type.to_string(),
            lang: input.lang.clone(),
            value,
            uri,
            is_public: input.is_public,
        })
        .await
        .map_err(db_error)?;
    }
    Ok(())
}

/// Delete every value on a resource.
pub async fn clear(api: &Api<'_>, resource: ResourceId) -> Result<(), ApiError> {
    let rows = ValueRepo::new(api.pool())
        .list_for_resource(resource)
        .await
        .map_err(db_error)?;
    let repo = ValueRepo::new(api.pool());
    for row in rows {
        repo.delete(row.id).await.map_err(db_error)?;
    }
    Ok(())
}

/// `ResourceTitleHydrator`: the first non-empty value of the template's title
/// property, or of `dcterms:title` when there is no template.
pub async fn title_for(
    api: &Api<'_>,
    template: Option<TemplateId>,
    inputs: &[ValueInput],
) -> Result<Option<String>, ApiError> {
    let title_property = match template {
        Some(id) => TemplateRepo::new(api.pool())
            .get(id)
            .await
            .map_err(db_error)?
            .and_then(|row| row.title_property_id),
        None => None,
    };
    let title_property = match title_property {
        Some(property) => Some(property),
        None => api.catalog().property_id("dcterms:title"),
    };
    let Some(title_property) = title_property else {
        return Ok(None);
    };
    for input in inputs {
        if input.property != title_property {
            continue;
        }
        let text = match &input.data {
            ValueKind::Literal(text) => Some(text.clone()),
            ValueKind::Uri { label, .. } => label.clone(),
            ValueKind::Resource(_) => None,
        };
        if let Some(text) = text.filter(|text| !text.is_empty()) {
            return Ok(Some(text));
        }
    }
    Ok(None)
}
