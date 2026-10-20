//! `POST` and `PUT` for `item_sets`.
//!
//! `ItemSetAdapter::hydrate` adds `o:is_open` to the shared resource hydration.
//! `isOpen` defaults to false, and a full `PUT` that omits `o:is_open` resets it
//! to false.

use omeka_db::ids::ItemSetId;
use omeka_db::repos::ItemSetRepo;
use omeka_db::rows::NewItemSet;
use omeka_db::with_db;

use crate::errors::ApiError;
use crate::read::{Api, ApiResponse, db_error, not_found, parse_id};
use crate::write::access;
use crate::write::body::Payload;
use crate::write::fulltext;
use crate::write::resource;
use crate::write::values;

const ITEM_SET_CLASS: &str = "Omeka\\Entity\\ItemSet";

const CONTROL: &[&str] = &[
    "o:is_public",
    "o:owner",
    "o:resource_class",
    "o:resource_template",
    "o:thumbnail",
    "o:is_open",
];

pub async fn create(api: &Api<'_>, payload: &Payload) -> Result<ApiResponse, ApiError> {
    let hydrated = resource::hydrate(api, payload, CONTROL, None, false).await?;
    let row = with_db!(api.pool(), |db| ItemSetRepo::new(db)
        .create(&NewItemSet {
            resource: hydrated.resource,
            is_open: payload.bool_member("o:is_open", false),
        })
        .await)
    .map_err(db_error)?;
    resource::store_values(
        api,
        row.resource.id,
        &hydrated.values,
        hydrated.keep_stored_values,
    )
    .await?;
    fulltext::reindex(api, "item_sets", row.resource.id).await?;
    api.read_member("item_sets", &row.resource.id.0.to_string())
        .await
}

pub async fn update(
    api: &Api<'_>,
    raw_id: &str,
    payload: &Payload,
    partial: bool,
) -> Result<ApiResponse, ApiError> {
    let id = parse_id(ITEM_SET_CLASS, raw_id)?;
    let Some(mut row) = with_db!(api.pool(), |db| ItemSetRepo::new(db)
        .get(ItemSetId(id))
        .await)
    .map_err(db_error)?
    else {
        return Err(not_found(ITEM_SET_CLASS, raw_id));
    };
    api.authorize_resource(ITEM_SET_CLASS, row.resource.owner_id)?;
    let hydrated = resource::hydrate(api, payload, CONTROL, Some(&row.resource), partial).await?;
    resource::apply(&mut row.resource, &hydrated.resource);
    if !partial || payload.has("o:is_open") {
        row.is_open = payload.bool_member("o:is_open", false);
    }
    with_db!(api.pool(), |db| ItemSetRepo::new(db).update(&row).await).map_err(db_error)?;
    resource::store_values(
        api,
        row.resource.id,
        &hydrated.values,
        hydrated.keep_stored_values,
    )
    .await?;
    fulltext::reindex(api, "item_sets", row.resource.id).await?;
    api.read_member("item_sets", raw_id).await
}

/// Delete an item set. `item_item_set` and `site_item_set` go with the database
/// cascade, which is Doctrine's rule here too (`Entity/ItemSet.php` maps the
/// memberships, it does not cascade remove to them); the values go first, as
/// `Entity/Resource.php:85` cascades remove to them.
pub async fn delete(api: &Api<'_>, raw_id: &str) -> Result<ApiResponse, ApiError> {
    let id = parse_id(ITEM_SET_CLASS, raw_id)?;
    let Some(row) = with_db!(api.pool(), |db| ItemSetRepo::new(db)
        .get(ItemSetId(id))
        .await)
    .map_err(db_error)?
    else {
        return Err(not_found(ITEM_SET_CLASS, raw_id));
    };
    access::resource_entity(api.actor(), ITEM_SET_CLASS, row.resource.owner_id)?;
    let answer = crate::read::item_set_document(api, &row).await?;
    values::clear(api, row.resource.id).await?;
    with_db!(api.pool(), |db| ItemSetRepo::new(db).delete(row.id()).await).map_err(db_error)?;
    fulltext::remove(api, "item_sets", row.resource.id).await?;
    Ok(answer)
}
