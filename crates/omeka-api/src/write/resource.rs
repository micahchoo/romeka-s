//! Shared hydration for the resource entities (items, item sets, media).
//!
//! `AbstractResourceEntityAdapter::hydrate` sets `o:is_public`, the values,
//! the owner, the resource class and template, the title (through
//! `ResourceTitleHydrator`) and the thumbnail, then stamps the timestamps. On a
//! full update `shouldHydrate` is true for every member, so an absent
//! `o:resource_class` clears the class and an absent `o:is_public` resets it to
//! the default `true`. `o:owner` is the exception: it changes only when the
//! member names an id, and an explicit `{"o:id": null}` clears it.
//!
//! A referenced id that does not exist is a 404, because upstream `findEntity`
//! throws `NotFoundException` before the row is written.

use omeka_db::ids::{AssetId, ClassId, PropertyId, ResourceId, TemplateId, UserId};
use omeka_db::repos::{AssetRepo, ClassRepo, PropertyRepo, TemplateRepo, UserRepo, ValueRepo};
use omeka_db::rows::{NewResource, ResourceRow};
use serde_json::Value;

use crate::errors::{ApiError, ValidationErrors};
use crate::read::{Api, db_error};
use crate::write::body::{Payload, ref_row};
use crate::write::values::{self, ValueInput};

/// The shared resource half of one create or update.
#[derive(Debug, Clone, PartialEq)]
pub struct Hydrated {
    pub resource: NewResource,
    pub values: Vec<ValueInput>,
    /// `ValueHydrator` line 162: a partial update whose body named no valid
    /// value leaves every stored value alone. A create, a full update, and a
    /// partial update that named one all replace the stored set.
    pub keep_stored_values: bool,
}

/// Write the values one hydration produced, unless the write keeps the stored
/// set (`ValueHydrator` line 162).
pub async fn store_values(
    api: &Api<'_>,
    resource: ResourceId,
    values: &[ValueInput],
    keep_stored_values: bool,
) -> Result<(), ApiError> {
    if keep_stored_values {
        return Ok(());
    }
    values::store(api, resource, values).await
}

/// Hydrate the shared resource columns.
///
/// `partial` mirrors `Request::getOption('isPartial')`: a `PATCH` keeps a
/// member the body omits; a `PUT` resets it.
pub async fn hydrate(
    api: &Api<'_>,
    payload: &Payload,
    control: &[&str],
    existing: Option<&ResourceRow>,
    partial: bool,
) -> Result<Hydrated, ApiError> {
    hydrate_at(api, payload, control, existing, partial, 0).await
}

/// `hydrate` with the annotation nesting depth the caller is at.
pub(crate) async fn hydrate_at(
    api: &Api<'_>,
    payload: &Payload,
    control: &[&str],
    existing: Option<&ResourceRow>,
    partial: bool,
    depth: u8,
) -> Result<Hydrated, ApiError> {
    let values = values::collect_at(api, payload, control, depth).await?;
    let keep_stored_values = partial && values.is_empty();

    let is_public = match (partial, payload.get("o:is_public")) {
        (true, None) => existing.map(|row| row.is_public).unwrap_or(true),
        _ => payload.bool_member("o:is_public", true),
    };
    let owner_id = match (partial && !payload.has("o:owner"), existing) {
        (true, Some(row)) => row.owner_id,
        _ => owner(api, payload, existing).await?,
    };
    let resource_class_id = match (partial && !payload.has("o:resource_class"), existing) {
        (true, Some(row)) => row.resource_class_id,
        _ => class(api, payload).await?,
    };
    let resource_template_id = match (partial && !payload.has("o:resource_template"), existing) {
        (true, Some(row)) => row.resource_template_id,
        _ => template(api, payload).await?,
    };
    let thumbnail_id = match (partial && !payload.has("o:thumbnail"), existing) {
        (true, Some(row)) => row.thumbnail_id,
        _ => asset(api, payload).await?,
    };
    let title = match (partial && values.is_empty(), existing) {
        (true, Some(row)) => row.title.clone(),
        _ => values::title_for(api, resource_template_id, &values).await?,
    };
    if let Some(template) = resource_template_id {
        let present = match (keep_stored_values, existing) {
            (true, Some(row)) => stored_properties(api, row.id).await?,
            _ => values.iter().map(|value| value.property).collect(),
        };
        require_template_values(api, template, &present).await?;
    }

    Ok(Hydrated {
        resource: NewResource {
            owner_id,
            resource_class_id,
            resource_template_id,
            thumbnail_id,
            title,
            is_public,
            created: existing.map(|row| row.created),
        },
        values,
        keep_stored_values,
    })
}

/// `AbstractResourceEntityAdapter::validateEntity`: every required property of
/// the template needs a value with the same property id, and each one missing
/// is a message under `o:resource_template_property`.
async fn require_template_values(
    api: &Api<'_>,
    template: TemplateId,
    present: &[PropertyId],
) -> Result<(), ApiError> {
    let repo = TemplateRepo::new(api.pool());
    let Some(row) = repo.get(template).await.map_err(db_error)? else {
        return Ok(());
    };
    let mut messages = Vec::new();
    for prop in repo.list_properties(template).await.map_err(db_error)? {
        if !prop.is_required || present.contains(&prop.property_id) {
            continue;
        }
        let label = match prop.alternate_label.filter(|label| !label.is_empty()) {
            Some(label) => label,
            None => PropertyRepo::new(api.pool())
                .get(prop.property_id)
                .await
                .map_err(db_error)?
                .map(|property| property.label)
                .unwrap_or_default(),
        };
        messages.push(format!(
            "The \"{}\" resource template requires a \"{label}\" value",
            row.label
        ));
    }
    if messages.is_empty() {
        return Ok(());
    }
    let mut errors = ValidationErrors::new();
    errors.insert("o:resource_template_property".to_string(), messages);
    Err(ApiError::Validation(errors))
}

/// The properties of a resource's stored values, which a partial update that
/// names no value keeps.
async fn stored_properties(
    api: &Api<'_>,
    resource: ResourceId,
) -> Result<Vec<PropertyId>, ApiError> {
    Ok(ValueRepo::new(api.pool())
        .list_for_resource(resource)
        .await
        .map_err(db_error)?
        .into_iter()
        .map(|row| row.property_id)
        .collect())
}

/// Copy the writable resource columns of `new` onto a stored row, leaving the
/// id, `created` and `resource_type` alone, as `update_resource_columns` does.
pub fn apply(row: &mut ResourceRow, new: &NewResource) {
    row.owner_id = new.owner_id;
    row.resource_class_id = new.resource_class_id;
    row.resource_template_id = new.resource_template_id;
    row.thumbnail_id = new.thumbnail_id;
    row.title = new.title.clone();
    row.is_public = new.is_public;
}

/// The stored `o:owner`, or the caller on create.
async fn owner(
    api: &Api<'_>,
    payload: &Payload,
    existing: Option<&ResourceRow>,
) -> Result<Option<UserId>, ApiError> {
    owner_member(
        api,
        payload,
        existing.is_none(),
        existing.and_then(|row| row.owner_id),
    )
    .await
}

/// `AbstractEntityAdapter::hydrateOwner` for any entity that carries an owner.
///
/// A member that names no `o:id` at all, `{"o:owner":{}}` among them, says
/// nothing about the owner. An `o:id` that is present but not numeric, an
/// explicit null included, reads as a null id, which clears the owner on an
/// update. A numeric id is cast with `(int)`, and upstream acts on it only when
/// it differs from the stored one: `0` clears the owner, anything else must
/// exist (`findEntity` throws). A create with no usable id belongs to the
/// caller.
pub(crate) async fn owner_member(
    api: &Api<'_>,
    payload: &Payload,
    create: bool,
    stored: Option<UserId>,
) -> Result<Option<UserId>, ApiError> {
    // `array_key_exists('o:id', $data['o:owner'])`, not `isset`: an explicit
    // null enters the branch upstream.
    let names_id = payload
        .get("o:owner")
        .and_then(Value::as_object)
        .is_some_and(|owner| owner.contains_key("o:id"));
    if !names_id {
        return Ok(if create { api.actor().user() } else { stored });
    }
    let Some(raw) = payload.numeric_ref("o:owner") else {
        return Ok(None);
    };
    let integer = crate::write::body::cast_int(raw).unwrap_or(0);
    if integer == 0 {
        // `$newOwnerId ? ... : null`: a zero id clears the owner, and a create
        // then falls back to the caller.
        return Ok(if create { api.actor().user() } else { None });
    }
    if integer < 0 {
        // `findEntity((int) $id)`: the criteria is the cast integer, not the
        // string the body carried.
        return Err(not_found(USER_CLASS, &Value::from(integer)));
    }
    let id = UserId(integer as u64);
    if UserRepo::new(api.pool())
        .get(id)
        .await
        .map_err(db_error)?
        .is_none()
    {
        return Err(not_found(USER_CLASS, &Value::from(integer)));
    }
    Ok(Some(id))
}

const USER_CLASS: &str = "Omeka\\Entity\\User";

async fn class(api: &Api<'_>, payload: &Payload) -> Result<Option<ClassId>, ApiError> {
    let Some(raw) = payload.numeric_ref("o:resource_class") else {
        return Ok(None);
    };
    let Some(id) = ref_row(raw) else {
        return Err(not_found(CLASS_CLASS, raw));
    };
    let id = ClassId(id);
    if ClassRepo::new(api.pool())
        .get(id)
        .await
        .map_err(db_error)?
        .is_none()
    {
        return Err(not_found(CLASS_CLASS, raw));
    }
    Ok(Some(id))
}

const CLASS_CLASS: &str = "Omeka\\Entity\\ResourceClass";

async fn template(api: &Api<'_>, payload: &Payload) -> Result<Option<TemplateId>, ApiError> {
    let Some(raw) = payload.numeric_ref("o:resource_template") else {
        return Ok(None);
    };
    let Some(id) = ref_row(raw) else {
        return Err(not_found(TEMPLATE_CLASS, raw));
    };
    let id = TemplateId(id);
    if TemplateRepo::new(api.pool())
        .get(id)
        .await
        .map_err(db_error)?
        .is_none()
    {
        return Err(not_found(TEMPLATE_CLASS, raw));
    }
    Ok(Some(id))
}

const TEMPLATE_CLASS: &str = "Omeka\\Entity\\ResourceTemplate";

/// `AbstractEntityAdapter::hydrateThumbnail` for `o:thumbnail`: a numeric id
/// that names no asset is the 404 `findEntity` throws, and a member that names
/// no usable id clears the thumbnail.
pub(crate) async fn asset(api: &Api<'_>, payload: &Payload) -> Result<Option<AssetId>, ApiError> {
    let Some(raw) = payload.numeric_ref("o:thumbnail") else {
        return Ok(None);
    };
    let Some(id) = ref_row(raw) else {
        return Err(not_found(ASSET_CLASS, raw));
    };
    let id = AssetId(id);
    if AssetRepo::new(api.pool())
        .get(id)
        .await
        .map_err(db_error)?
        .is_none()
    {
        return Err(not_found(ASSET_CLASS, raw));
    }
    Ok(Some(id))
}

const ASSET_CLASS: &str = "Omeka\\Entity\\Asset";

/// `AbstractEntityAdapter::findEntity` for a body-supplied reference: the
/// criteria is the value the body carried, printed as `json_encode` prints it.
pub fn not_found(class: &str, raw: &Value) -> ApiError {
    ApiError::not_found(format!(
        "{class} entity with criteria {} not found",
        crate::write::body::ref_criteria(raw)
    ))
}
