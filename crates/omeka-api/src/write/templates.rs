//! `POST`, `PUT`, `PATCH` and `DELETE` for `resource_templates`.
//!
//! `ResourceTemplateAdapter::hydrate` writes `o:owner`, `o:resource_class`,
//! `o:label`, `o:title_property`, `o:description_property` and the
//! `o:resource_template_property` list. Each list entry names a property by id
//! (`findEntity` throws when it does not exist), carries the alternate labels,
//! the data types, the two flags and a default language, and takes its position
//! from its place in the list, one-based.
//!
//! The collection block runs only when the body carries the member: a full
//! `PUT` that omits `o:resource_template_property` leaves the list alone. When
//! it runs, an entry whose property is already on the template updates that
//! row, a new property adds one, and a row the list does not name is removed.

use omeka_db::ids::{ClassId, PropertyId, TemplateId, UserId};
use omeka_db::repos::{ClassRepo, PropertyRepo, TemplateRepo};
use omeka_db::rows::{NewTemplate, NewTemplateProperty, TemplatePropertyRow, TemplateRow};

use crate::errors::{ApiError, ValidationErrors};
use crate::messages;
use crate::read::{Api, ApiResponse, db_error, not_found, parse_id};
use crate::write::access;
use crate::write::body::{Payload, ref_row};
use crate::write::resource;
use crate::write::resource::not_found as ref_not_found;
use crate::write::vocab_member;
use serde_json::Value;

const TEMPLATE_CLASS: &str = "Omeka\\Entity\\ResourceTemplate";
const PROPERTY_CLASS: &str = "Omeka\\Entity\\Property";

/// The hydratable columns of one template.
struct TemplateInput {
    owner_id: Option<UserId>,
    resource_class_id: Option<ClassId>,
    label: String,
    title_property_id: Option<PropertyId>,
    description_property_id: Option<PropertyId>,
}

/// One `o:resource_template_property` entry after hydration.
struct TemplatePropertyInput {
    property_id: PropertyId,
    alternate_label: Option<String>,
    alternate_comment: Option<String>,
    data_type: Option<Vec<String>>,
    is_required: bool,
    is_private: bool,
    default_lang: Option<String>,
    /// One-based, from the entry's place in the list.
    position: i64,
}

pub async fn create(api: &Api<'_>, payload: &Payload) -> Result<ApiResponse, ApiError> {
    let request_errors = validate_request(payload);
    let input = hydrate(api, payload, None, false).await?;
    let properties = property_inputs(api, payload).await?;
    validate(api, &input, None, request_errors).await?;
    let row = TemplateRepo::new(api.pool())
        .create(&NewTemplate {
            owner_id: input.owner_id,
            resource_class_id: input.resource_class_id,
            title_property_id: input.title_property_id,
            description_property_id: input.description_property_id,
            label: input.label,
        })
        .await
        .map_err(db_error)?;
    sync_properties(api, row.id, &properties).await?;
    api.read_member("resource_templates", &row.id.0.to_string())
        .await
}

pub async fn update(
    api: &Api<'_>,
    raw_id: &str,
    payload: &Payload,
    partial: bool,
) -> Result<ApiResponse, ApiError> {
    let request_errors = validate_request(payload);
    let id = parse_id(TEMPLATE_CLASS, raw_id)?;
    let repo = TemplateRepo::new(api.pool());
    let Some(stored) = repo.get(TemplateId(id)).await.map_err(db_error)? else {
        return Err(not_found(TEMPLATE_CLASS, raw_id));
    };
    access::resource_entity_update(api.actor(), TEMPLATE_CLASS, stored.owner_id)?;
    let input = hydrate(api, payload, Some(&stored), partial).await?;
    // The list is replaced only when the body carries it; a full update that
    // omits `o:resource_template_property` keeps the stored rows.
    let properties = if payload.is_array_like("o:resource_template_property") {
        Some(property_inputs(api, payload).await?)
    } else {
        None
    };
    validate(api, &input, Some(id), request_errors).await?;
    repo.update(&TemplateRow {
        id: stored.id,
        owner_id: input.owner_id,
        resource_class_id: input.resource_class_id,
        title_property_id: input.title_property_id,
        description_property_id: input.description_property_id,
        label: input.label,
    })
    .await
    .map_err(db_error)?;
    if let Some(properties) = &properties {
        sync_properties(api, stored.id, properties).await?;
    }
    api.read_member("resource_templates", raw_id).await
}

/// Delete a template and the property rows it lists.
///
/// `TemplateRepo::delete` removes the rows first, because
/// `resource_template_property.resource_template_id` carries no database delete
/// action, then the template; a resource that used the template keeps its row
/// and loses the reference (`resource.resource_template_id` is
/// `ON DELETE SET NULL`).
pub async fn delete(api: &Api<'_>, raw_id: &str) -> Result<ApiResponse, ApiError> {
    let id = parse_id(TEMPLATE_CLASS, raw_id)?;
    let repo = TemplateRepo::new(api.pool());
    let Some(row) = repo.get(TemplateId(id)).await.map_err(db_error)? else {
        return Err(not_found(TEMPLATE_CLASS, raw_id));
    };
    access::resource_entity(api.actor(), TEMPLATE_CLASS, row.owner_id)?;
    let answer = api.read_member("resource_templates", raw_id).await?;
    repo.delete(row.id).await.map_err(db_error)?;
    Ok(answer)
}

/// `ResourceTemplateAdapter::validateRequest`: one property may appear once.
///
/// The member is walked only when it is a list, and a non-list member is
/// ignored rather than refused, which is what the upstream condition does. The
/// errors join the ones [`validate`] raises after hydration, because upstream
/// collects both in one `ErrorStore`.
fn validate_request(payload: &Payload) -> ValidationErrors {
    let mut errors = ValidationErrors::new();
    let mut ids: Vec<Value> = Vec::new();
    for entry in payload.entries("o:resource_template_property") {
        // `isset($resTemPropData['o:property']['o:id'])` skips a missing or null
        // id; `in_array` compares the values loosely.
        let Some(raw) = entry.present_ref("o:property") else {
            continue;
        };
        if ids
            .iter()
            .any(|other| crate::write::body::same_reference(other, raw))
        {
            vocab_member::push(
                &mut errors,
                "o:property",
                messages::duplicate_template_property(crate::write::body::php_display(raw)),
            );
        }
        ids.push(raw.clone());
    }
    errors
}

/// `ResourceTemplateAdapter::hydrate`'s scalar members.
async fn hydrate(
    api: &Api<'_>,
    payload: &Payload,
    existing: Option<&TemplateRow>,
    partial: bool,
) -> Result<TemplateInput, ApiError> {
    let create = existing.is_none();
    let owner_id =
        resource::owner_member(api, payload, create, existing.and_then(|row| row.owner_id)).await?;
    let resource_class_id = match (partial && !payload.has("o:resource_class"), existing) {
        (true, Some(row)) => row.resource_class_id,
        _ => class(api, payload).await?,
    };
    let label = match (
        partial && !payload.has("o:label"),
        payload.str_member("o:label"),
    ) {
        (true, _) => existing.map(|row| row.label.clone()).unwrap_or_default(),
        (false, Some(value)) => value.to_string(),
        (false, None) => String::new(),
    };
    let title_property_id = match (partial && !payload.has("o:title_property"), existing) {
        (true, Some(row)) => row.title_property_id,
        _ => property(api, payload, "o:title_property").await?,
    };
    let description_property_id =
        match (partial && !payload.has("o:description_property"), existing) {
            (true, Some(row)) => row.description_property_id,
            _ => property(api, payload, "o:description_property").await?,
        };
    Ok(TemplateInput {
        owner_id,
        resource_class_id,
        label,
        title_property_id,
        description_property_id,
    })
}

/// `hydrateResourceClass`: a numeric `o:id` names a stored class, anything else
/// clears it. A named id that does not exist is a 404.
async fn class(api: &Api<'_>, payload: &Payload) -> Result<Option<ClassId>, ApiError> {
    let Some(raw) = payload.numeric_ref("o:resource_class") else {
        return Ok(None);
    };
    let Some(id) = ref_row(raw) else {
        return Err(ref_not_found("Omeka\\Entity\\ResourceClass", raw));
    };
    let id = ClassId(id);
    if ClassRepo::new(api.pool())
        .get(id)
        .await
        .map_err(db_error)?
        .is_none()
    {
        return Err(ref_not_found("Omeka\\Entity\\ResourceClass", raw));
    }
    Ok(Some(id))
}

/// `o:title_property` / `o:description_property`: a numeric `o:id` names a
/// stored property, anything else clears it.
async fn property(
    api: &Api<'_>,
    payload: &Payload,
    key: &str,
) -> Result<Option<PropertyId>, ApiError> {
    let Some(raw) = payload.numeric_ref(key) else {
        return Ok(None);
    };
    let Some(id) = ref_row(raw) else {
        return Err(ref_not_found(PROPERTY_CLASS, raw));
    };
    let id = PropertyId(id);
    if PropertyRepo::new(api.pool())
        .get(id)
        .await
        .map_err(db_error)?
        .is_none()
    {
        return Err(ref_not_found(PROPERTY_CLASS, raw));
    }
    Ok(Some(id))
}

/// `ResourceTemplateAdapter::validateEntity`, with the duplicate-property
/// errors from `validate_request` folded in.
async fn validate(
    api: &Api<'_>,
    input: &TemplateInput,
    self_id: Option<u64>,
    mut errors: ValidationErrors,
) -> Result<(), ApiError> {
    if input.label.trim().is_empty() {
        vocab_member::push(&mut errors, "o:label", messages::LABEL_EMPTY);
    }
    if label_taken(api, &input.label, self_id).await? {
        vocab_member::push(&mut errors, "o:label", messages::LABEL_TAKEN);
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(ApiError::Validation(errors))
    }
}

/// `AbstractEntityAdapter::isUnique` for `resource_template.label`.
async fn label_taken(api: &Api<'_>, label: &str, self_id: Option<u64>) -> Result<bool, ApiError> {
    let Some(row) = TemplateRepo::new(api.pool())
        .find_by_label(label)
        .await
        .map_err(db_error)?
    else {
        return Ok(false);
    };
    Ok(Some(row.id.0) != self_id)
}

/// The `o:resource_template_property` entries, in list order.
///
/// An entry with no usable property id is skipped; a property id that does not
/// exist is the 404 `findEntity` throws.
async fn property_inputs(
    api: &Api<'_>,
    payload: &Payload,
) -> Result<Vec<TemplatePropertyInput>, ApiError> {
    let mut inputs = Vec::new();
    let mut position = 1;
    for entry in payload.entries("o:resource_template_property") {
        // `empty($resTemPropData['o:property']['o:id'])` skips a missing, null,
        // empty, zero or false id; every other value is cast with `(int)` and
        // looked up, so `"abc"` reaches `findEntity(0)`.
        let Some(raw) = entry.present_ref("o:property") else {
            continue;
        };
        if crate::write::body::is_empty(raw) {
            continue;
        }
        let integer = crate::write::body::cast_int(raw).unwrap_or(0);
        let criteria = serde_json::Value::from(integer);
        if integer < 0 {
            return Err(ref_not_found(PROPERTY_CLASS, &criteria));
        }
        let property_id = PropertyId(integer as u64);
        if PropertyRepo::new(api.pool())
            .get(property_id)
            .await
            .map_err(db_error)?
            .is_none()
        {
            return Err(ref_not_found(PROPERTY_CLASS, &criteria));
        }
        inputs.push(TemplatePropertyInput {
            property_id,
            alternate_label: non_blank(&entry, "o:alternate_label"),
            alternate_comment: non_blank(&entry, "o:alternate_comment"),
            data_type: data_types(&entry),
            is_required: entry.bool_member("o:is_required", false),
            is_private: entry.bool_member("o:is_private", false),
            default_lang: non_blank(&entry, "o:default_lang"),
            position,
        });
        position += 1;
    }
    Ok(inputs)
}

/// A member whose trimmed value is not empty; a whitespace-only value is
/// absent, and the stored value keeps its whitespace.
fn non_blank(entry: &Payload, key: &str) -> Option<String> {
    entry
        .str_member(key)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
}

/// `array_values(array_unique(array_filter(array_map('trim', $dataTypes))))`:
/// trimmed, empty values dropped, first occurrences kept in order.
fn data_types(entry: &Payload) -> Option<Vec<String>> {
    let values = match entry.get("o:data_type") {
        Some(serde_json::Value::Array(values)) => values,
        _ => return None,
    };
    let mut types: Vec<String> = Vec::new();
    for value in values {
        let Some(value) = value.as_str() else {
            continue;
        };
        let value = value.trim();
        if value.is_empty() || types.iter().any(|kept| kept == value) {
            continue;
        }
        types.push(value.to_string());
    }
    if types.is_empty() { None } else { Some(types) }
}

/// Apply the list: update the row of each property already on the template, add
/// the rest, and remove the rows the list does not name.
async fn sync_properties(
    api: &Api<'_>,
    template_id: TemplateId,
    inputs: &[TemplatePropertyInput],
) -> Result<(), ApiError> {
    let repo = TemplateRepo::new(api.pool());
    let existing = repo.list_properties(template_id).await.map_err(db_error)?;
    let mut retained: Vec<u64> = Vec::new();
    for input in inputs {
        match existing
            .iter()
            .find(|row| row.property_id == input.property_id)
        {
            Some(row) => {
                retained.push(row.id.0);
                repo.update_property(&TemplatePropertyRow {
                    id: row.id,
                    resource_template_id: template_id,
                    property_id: input.property_id,
                    alternate_label: input.alternate_label.clone(),
                    alternate_comment: input.alternate_comment.clone(),
                    position: Some(input.position),
                    data_type: input.data_type.clone(),
                    is_required: input.is_required,
                    is_private: input.is_private,
                    default_lang: input.default_lang.clone(),
                })
                .await
                .map_err(db_error)?;
            }
            None => {
                let row = repo
                    .add_property(&NewTemplateProperty {
                        resource_template_id: template_id,
                        property_id: input.property_id,
                        alternate_label: input.alternate_label.clone(),
                        alternate_comment: input.alternate_comment.clone(),
                        position: Some(input.position),
                        data_type: input.data_type.clone(),
                        is_required: input.is_required,
                        is_private: input.is_private,
                        default_lang: input.default_lang.clone(),
                    })
                    .await
                    .map_err(db_error)?;
                retained.push(row.id.0);
            }
        }
    }
    for row in &existing {
        if !retained.contains(&row.id.0) {
            repo.delete_property(row.id).await.map_err(db_error)?;
        }
    }
    Ok(())
}
