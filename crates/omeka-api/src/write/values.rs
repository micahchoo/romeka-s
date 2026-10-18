//! Value hydration for the resource-entity writes.
//!
//! `ValueHydrator::hydrate` walks every top-level key that carries an array of
//! value objects. Each object names a property (`property_id`, or `"auto"` to
//! take the term from its key), a data type (`type`, `literal` when absent),
//! and the object of the triple (`@value`, `@id`, or `value_resource_id`). An
//! object with no usable property or object is ignored, as upstream ignores an
//! invalid value. `ResourceTitleHydrator` then takes the resource title from the
//! title property's first non-empty value.
//!
//! An object may also carry `@annotation` (`ValueHydrator` lines 134-153). A
//! non-empty one is hydrated as a `value_annotations` resource with its own
//! values and stored beside the value; a value that carried one and no longer
//! does takes its annotation with it, as `orphanRemoval` does upstream.

use omeka_db::ids::{AnnotationId, PropertyId, ResourceId, TemplateId};
use omeka_db::repos::{ResourceRepo, TemplateRepo, ValueRepo};
use omeka_db::rows::{NewResource, NewValue};
use serde_json::Value;

use crate::errors::{ApiError, ValidationErrors};
use crate::read::{Api, db_error};
use crate::write::body::{Payload, is_numeric, ref_row};
use crate::write::resource;

/// The nesting cap the read side uses for `@annotation` (read/values.rs). A
/// write deeper than this would not survive a read, so it is not stored.
const MAX_ANNOTATION_DEPTH: u8 = 8;

/// The members the shared resource hydrate reads. A value annotation is a plain
/// resource of the `value_annotations` adapter.
const ANNOTATION_CONTROL: &[&str] = &[
    "o:is_public",
    "o:owner",
    "o:resource_class",
    "o:resource_template",
    "o:thumbnail",
];

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
    pub annotation: Option<Box<Annotation>>,
}

/// A `value_annotation`: the resource a value carries. `ValueHydrator` hydrates
/// it with the `value_annotations` adapter, which is the shared resource
/// adapter, so it is a resource row plus its own values.
#[derive(Debug, Clone, PartialEq)]
pub struct Annotation {
    pub resource: NewResource,
    pub values: Vec<ValueInput>,
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
///
/// A resource-valued object is checked against the `resource` table here,
/// before anything is written. `DataType\Resource\AbstractResource::hydrate`
/// throws `NotFoundException` at the same point in upstream, and because that
/// runs during hydration the flush never happens.
pub async fn collect(
    api: &Api<'_>,
    payload: &Payload,
    control: &[&str],
) -> Result<Vec<ValueInput>, ApiError> {
    collect_at(api, payload, control, 0).await
}

/// `collect` with the annotation nesting depth the caller is at.
pub(crate) async fn collect_at(
    api: &Api<'_>,
    payload: &Payload,
    control: &[&str],
    depth: u8,
) -> Result<Vec<ValueInput>, ApiError> {
    let mut inputs = Vec::new();
    let mut errors = ValidationErrors::new();
    for (key, value) in payload.members() {
        if is_control(api, key, control) {
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
            if let Some(input) = parse_one(api, key, item, depth).await? {
                if let ValueKind::Resource(id) = &input.data {
                    ensure_resource(api, *id).await?;
                }
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

/// Whether a top-level member is control rather than a property term.
///
/// The `o:` namespace is the API's own: the hydrators read the members they
/// name and ignore every other `o:` member, and no vocabulary is registered
/// under the prefix `o`, so an `o:` member states no property value. A member
/// the catalog does know as a term is never skipped this way.
fn is_control(api: &Api<'_>, key: &str, control: &[&str]) -> bool {
    control.contains(&key)
        || JSONLD_KEYWORDS.contains(&key)
        || (key.starts_with("o:") && !api.catalog().knows_term(key))
}

/// `DataType\Resource\AbstractResource::hydrate`: the named value resource must
/// exist, and a missing one is the 404 `EntityManager::find` leaves to the
/// exception strategy.
async fn ensure_resource(api: &Api<'_>, id: ResourceId) -> Result<(), ApiError> {
    if ResourceRepo::new(api.pool())
        .get(id)
        .await
        .map_err(db_error)?
        .is_none()
    {
        return Err(ApiError::not_found(format!(
            "Resource not found with id {}.",
            id.0
        )));
    }
    Ok(())
}

/// The scalar half of one value object: its property, its object, its language
/// and its visibility.
type Scalar = (PropertyId, ValueKind, Option<String>, bool);

/// The scalar half of one value object; `None` when the object names no usable
/// property, type or object (`ValueHydrator` ignores an invalid value). A
/// resource object whose `value_resource_id` is numeric but names no row is the
/// 404 `DataType\Resource\AbstractResource::hydrate` throws.
fn parse_scalar(
    api: &Api<'_>,
    term: &str,
    object: &serde_json::Map<String, Value>,
) -> Result<Option<Scalar>, ApiError> {
    let property = match object.get("property_id") {
        Some(Value::String(name)) if name == "auto" => {
            let Some(property) = api.catalog().property_id(term) else {
                return Ok(None);
            };
            property
        }
        Some(Value::Number(number)) => {
            let Some(id) = number.as_u64() else {
                return Ok(None);
            };
            PropertyId(id)
        }
        _ => return Ok(None),
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
            let Some(uri) = object.get("@id").and_then(Value::as_str) else {
                return Ok(None);
            };
            let label = object
                .get("o:label")
                .and_then(Value::as_str)
                .map(str::to_string);
            ValueKind::Uri {
                uri: uri.to_string(),
                label,
            }
        }
        kind if kind.starts_with("resource") => {
            // `AbstractResource::isValid` requires `is_numeric`, so a value
            // that is not numeric is ignored rather than looked up.
            let Some(raw) = object.get("value_resource_id") else {
                return Ok(None);
            };
            if !is_numeric(raw) {
                return Ok(None);
            }
            let Some(id) = ref_row(raw) else {
                return Err(ApiError::not_found(format!(
                    "Resource not found with id {}.",
                    crate::write::body::php_display(raw)
                )));
            };
            ValueKind::Resource(ResourceId(id))
        }
        _ => {
            let Some(text) = object.get("@value").and_then(Value::as_str) else {
                return Ok(None);
            };
            ValueKind::Literal(text.to_string())
        }
    };
    Ok(Some((property, data, lang, is_public)))
}

/// A value as PHP interpolates it into a message string.
async fn parse_one(
    api: &Api<'_>,
    term: &str,
    value: &Value,
    depth: u8,
) -> Result<Option<ValueInput>, ApiError> {
    let Some(object) = value.as_object() else {
        return Ok(None);
    };
    let Some((property, data, lang, is_public)) = parse_scalar(api, term, object)? else {
        return Ok(None);
    };
    let annotation = match object.get("@annotation") {
        Some(Value::Object(map)) if !map.is_empty() && depth < MAX_ANNOTATION_DEPTH => {
            let payload = Payload::from_object(map.clone());
            let hydrated = Box::pin(resource::hydrate_at(
                api,
                &payload,
                ANNOTATION_CONTROL,
                None,
                false,
                depth + 1,
            ))
            .await?;
            Some(Box::new(Annotation {
                resource: hydrated.resource,
                values: hydrated.values,
            }))
        }
        _ => None,
    };
    Ok(Some(ValueInput {
        property,
        data,
        lang,
        is_public,
        annotation,
    }))
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
        let annotation_id = match &input.annotation {
            Some(annotation) => {
                let row = ResourceRepo::new(api.pool())
                    .create_annotation(&annotation.resource)
                    .await
                    .map_err(db_error)?;
                Box::pin(store(api, row.resource.id, &annotation.values)).await?;
                Some(row.id())
            }
            None => None,
        };
        let (r#type, value, uri, value_resource_id) = match &input.data {
            ValueKind::Literal(text) => ("literal", Some(text.clone()), None, None),
            ValueKind::Uri { uri, label } => ("uri", label.clone(), Some(uri.clone()), None),
            ValueKind::Resource(id) => ("resource", None, None, Some(*id)),
        };
        repo.create(&NewValue {
            resource_id: resource,
            property_id: input.property,
            value_resource_id,
            value_annotation_id: annotation_id,
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

/// Delete every value on a resource, and each value's annotation resource.
pub async fn clear(api: &Api<'_>, resource: ResourceId) -> Result<(), ApiError> {
    let rows = ValueRepo::new(api.pool())
        .list_for_resource(resource)
        .await
        .map_err(db_error)?;
    let repo = ValueRepo::new(api.pool());
    for row in rows {
        delete_annotation(api, row.value_annotation_id).await?;
        repo.delete(row.id).await.map_err(db_error)?;
    }
    Ok(())
}

/// Delete a value's annotation resource, and the annotations of the values the
/// annotation itself carries, before the annotation row goes.
async fn delete_annotation(api: &Api<'_>, id: Option<AnnotationId>) -> Result<(), ApiError> {
    let Some(id) = id else {
        return Ok(());
    };
    let resource = id.as_resource();
    let rows = ValueRepo::new(api.pool())
        .list_for_resource(resource)
        .await
        .map_err(db_error)?;
    for row in rows {
        Box::pin(delete_annotation(api, row.value_annotation_id)).await?;
    }
    ResourceRepo::new(api.pool())
        .delete(resource)
        .await
        .map_err(db_error)?;
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
