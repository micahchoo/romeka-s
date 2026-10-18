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

use omeka_db::ids::{AssetId, ClassId, ResourceId, TemplateId, UserId};
use omeka_db::repos::{ClassRepo, TemplateRepo, UserRepo};
use omeka_db::rows::{NewResource, ResourceRow};
use serde_json::json;

use crate::errors::ApiError;
use crate::read::{Api, db_error};
use crate::write::body::Payload;
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
        _ => asset(payload),
    };
    let title = match (partial && values.is_empty(), existing) {
        (true, Some(row)) => row.title.clone(),
        _ => values::title_for(api, resource_template_id, &values).await?,
    };

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
/// An absent `o:owner`, or a member that names no `o:id`, keeps the stored
/// owner (`{"o:owner":{}}` says nothing about the owner). An explicit
/// `{"o:owner":{"o:id":null}}` clears it, and a named id must exist
/// (`findEntity` throws). A create with no usable id belongs to the caller.
pub(crate) async fn owner_member(
    api: &Api<'_>,
    payload: &Payload,
    create: bool,
    stored: Option<UserId>,
) -> Result<Option<UserId>, ApiError> {
    let names_id = payload
        .get("o:owner")
        .and_then(|value| value.get("o:id"))
        .is_some();
    match (names_id, payload.ref_id("o:owner")) {
        (true, Some(id)) => {
            let id = UserId(id);
            if UserRepo::new(api.pool())
                .get(id)
                .await
                .map_err(db_error)?
                .is_none()
            {
                return Err(not_found("Omeka\\Entity\\User", id.0));
            }
            Ok(Some(id))
        }
        // An explicit `o:id: null` clears the owner on an update. On a create
        // the new entity has no owner yet, so the default below applies.
        (true, None) if !create => Ok(None),
        // No usable id: a create belongs to the caller, an update keeps what is
        // stored.
        _ => Ok(if create { api.actor().user() } else { stored }),
    }
}

async fn class(api: &Api<'_>, payload: &Payload) -> Result<Option<ClassId>, ApiError> {
    let Some(id) = payload.ref_id("o:resource_class") else {
        return Ok(None);
    };
    let id = ClassId(id);
    if ClassRepo::new(api.pool())
        .get(id)
        .await
        .map_err(db_error)?
        .is_none()
    {
        return Err(not_found("Omeka\\Entity\\ResourceClass", id.0));
    }
    Ok(Some(id))
}

async fn template(api: &Api<'_>, payload: &Payload) -> Result<Option<TemplateId>, ApiError> {
    let Some(id) = payload.ref_id("o:resource_template") else {
        return Ok(None);
    };
    let id = TemplateId(id);
    if TemplateRepo::new(api.pool())
        .get(id)
        .await
        .map_err(db_error)?
        .is_none()
    {
        return Err(not_found("Omeka\\Entity\\ResourceTemplate", id.0));
    }
    Ok(Some(id))
}

/// Assets have no repository in the port yet, so a thumbnail id is stored
/// without a lookup. Reported as a gap.
pub(crate) fn asset(payload: &Payload) -> Option<AssetId> {
    payload.ref_id("o:thumbnail").map(AssetId)
}

/// `AbstractEntityAdapter::findEntity` for a body-supplied id: the criteria
/// value is the integer the body carried.
pub fn not_found(class: &str, id: u64) -> ApiError {
    let criteria = json!({ "id": id });
    ApiError::not_found(format!("{class} entity with criteria {criteria} not found"))
}
