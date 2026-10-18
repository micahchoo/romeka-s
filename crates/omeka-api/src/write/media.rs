//! `POST` and `PUT` for `media`.
//!
//! `MediaAdapter` sets the ingester and renderer only on create, links the item
//! (and refuses the row when none is given), stores the ingester's `data`, and
//! takes the owner from the item. `position`, `o:lang` and `o:alt_text` hydrate
//! on every write. The ingester pipeline that would turn an upload into a file
//! is `omeka-media`'s work, not this handler's; the API stores the ingester,
//! renderer and `data` it was handed.
//!
//! `Ingester\Html::getRenderer` and its siblings map the ingester name to the
//! renderer name; `Ingester\Fallback` has no renderer, so an unknown ingester is
//! rejected the way upstream rejects the fallback.

use omeka_db::ids::{ItemId, MediaId, ResourceId};
use omeka_db::repos::{ItemRepo, MediaRepo};
use omeka_db::rows::{MediaRow, NewMedia};
use serde_json::Value;

use crate::errors::{ApiError, ValidationErrors};
use crate::read::{Api, ApiResponse, db_error, not_found, parse_id};
use crate::write::access;
use crate::write::body::{Payload, php_json};
use crate::write::fulltext;
use crate::write::resource::{self, not_found as ref_not_found};
use crate::write::values;

const MEDIA_CLASS: &str = "Omeka\\Entity\\Media";
const ITEM_CLASS: &str = "Omeka\\Entity\\Item";

const CONTROL: &[&str] = &[
    "o:is_public",
    "o:owner",
    "o:resource_class",
    "o:resource_template",
    "o:thumbnail",
    "o:ingester",
    "o:item",
    "o:source",
    "o:lang",
    "o:alt_text",
    "data",
    "position",
];

/// `Ingester\*::getRenderer`.
fn renderer_for(ingester: &str) -> Option<&'static str> {
    Some(match ingester {
        "html" => "html",
        "upload" | "url" => "file",
        "oembed" => "oembed",
        "youtube" => "youtube",
        "iiif" => "iiif",
        "iiif_presentation" => "iiif_presentation",
        _ => return None,
    })
}

pub async fn create(api: &Api<'_>, payload: &Payload) -> Result<ApiResponse, ApiError> {
    let Some(ingester) = payload.str_member("o:ingester").map(str::to_string) else {
        return Err(validation("o:ingester", "Media must set an ingester."));
    };
    let Some(renderer) = renderer_for(&ingester) else {
        return Err(validation("o:ingester", "Media must set a valid ingester."));
    };
    let item = item(api, payload).await?;
    let hydrated = resource::hydrate(api, payload, CONTROL, None, false).await?;
    // `MediaAdapter::hydrateOwner`: an explicit `o:owner` wins, otherwise the
    // media is owned by the item's owner.
    let owner_id = if payload.has("o:owner") {
        hydrated.resource.owner_id
    } else {
        item.resource.owner_id
    };
    let row = omeka_db::repos::MediaRepo::new(api.pool())
        .create(&NewMedia {
            resource: omeka_db::rows::NewResource {
                owner_id,
                ..hydrated.resource
            },
            item_id: item.id(),
            ingester,
            renderer: renderer.to_string(),
            data: data(payload),
            source: payload.str_member("o:source").map(str::to_string),
            position: payload.int_member("position"),
            lang: payload.str_member("o:lang").map(str::to_string),
            alt_text: payload.str_member("o:alt_text").map(str::to_string),
            ..NewMedia::default()
        })
        .await
        .map_err(db_error)?;
    values::store(api, row.resource.id, &hydrated.values).await?;
    fulltext::reindex(api, "media", row.resource.id).await?;
    // `Module::saveFulltext`'s media branch: the item's text includes its media.
    fulltext::reindex(api, "items", ResourceId(row.item_id.0)).await?;
    api.read_member("media", &row.resource.id.0.to_string())
        .await
}

pub async fn update(
    api: &Api<'_>,
    raw_id: &str,
    payload: &Payload,
    partial: bool,
) -> Result<ApiResponse, ApiError> {
    let id = parse_id(MEDIA_CLASS, raw_id)?;
    let repo = omeka_db::repos::MediaRepo::new(api.pool());
    let Some(mut row) = repo
        .get(omeka_db::ids::MediaId(id))
        .await
        .map_err(db_error)?
    else {
        return Err(not_found(MEDIA_CLASS, raw_id));
    };
    api.authorize_resource(MEDIA_CLASS, row.resource.owner_id)?;
    let hydrated = resource::hydrate(api, payload, CONTROL, Some(&row.resource), partial).await?;
    resource::apply(&mut row.resource, &hydrated.resource);
    if let Some(position) = payload.int_member("position") {
        row.position = Some(position);
    }
    if !partial || payload.has("o:lang") {
        row.lang = payload.str_member("o:lang").map(str::to_string);
    }
    if !partial || payload.has("o:alt_text") {
        row.alt_text = payload.str_member("o:alt_text").map(str::to_string);
    }
    repo.update(&row).await.map_err(db_error)?;
    values::store(api, row.resource.id, &hydrated.values).await?;
    fulltext::reindex(api, "media", row.resource.id).await?;
    fulltext::reindex(api, "items", ResourceId(row.item_id.0)).await?;
    api.read_member("media", raw_id).await
}

/// Delete a media: its stored bytes, then its row, its values and its resource
/// row.
pub async fn delete(api: &Api<'_>, raw_id: &str) -> Result<ApiResponse, ApiError> {
    let id = parse_id(MEDIA_CLASS, raw_id)?;
    let repo = MediaRepo::new(api.pool());
    let Some(row) = repo.get(MediaId(id)).await.map_err(db_error)? else {
        return Err(not_found(MEDIA_CLASS, raw_id));
    };
    access::resource_entity(api.actor(), MEDIA_CLASS, row.resource.owner_id)?;
    let answer = crate::read::media_document(api, &row).await?;
    remove(api, &row).await?;
    Ok(answer)
}

/// Remove one media row and the bytes it owns, in the order a failure should
/// leave things: the files first, so a store failure keeps a readable row.
///
/// `Item::$media` cascades remove, so an item delete reaches this for each of
/// its media; `item.primary_media_id` and any site block attachment lose the
/// reference (`ON DELETE SET NULL`).
pub(crate) async fn remove(api: &Api<'_>, row: &MediaRow) -> Result<(), ApiError> {
    crate::write::files::delete_media_files(api.store(), row)?;
    MediaRepo::new(api.pool())
        .delete(row.id())
        .await
        .map_err(db_error)?;
    fulltext::remove(api, "media", row.resource.id).await?;
    // `Module::deleteFulltextMedia`: the item's index text included this media,
    // so the item is re-indexed after the media row is gone.
    fulltext::reindex(api, "items", ResourceId(row.item_id.0)).await?;
    Ok(())
}

/// The item a create names. `MediaAdapter::validateEntity` rejects a media with
/// no item; a named item that does not exist is a 404.
async fn item(api: &Api<'_>, payload: &Payload) -> Result<omeka_db::rows::ItemRow, ApiError> {
    let Some(raw) = payload.ref_id("o:item") else {
        return Err(validation("o:item", "Media must belong to an item."));
    };
    let Some(item) = ItemRepo::new(api.pool())
        .get(ItemId(raw))
        .await
        .map_err(db_error)?
    else {
        return Err(ref_not_found(ITEM_CLASS, raw));
    };
    api.authorize_resource(ITEM_CLASS, item.resource.owner_id)?;
    Ok(item)
}

fn data(payload: &Payload) -> Option<Value> {
    payload.get("data").map(php_json)
}

fn validation(key: &str, message: &str) -> ApiError {
    let mut errors = ValidationErrors::new();
    errors.insert(key.to_string(), vec![message.to_string()]);
    ApiError::Validation(errors)
}
