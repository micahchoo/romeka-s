//! `POST` and `PUT` for `items`.
//!
//! `ItemAdapter::validateRequest` rejects a non-array `o:item_set` or
//! `o:media`; `hydrate` then sets `o:primary_media` and the item-set
//! membership on top of the shared resource hydration. A full `PUT` replaces
//! both, so an omitted `o:item_set` empties the membership. Inline `o:media`
//! rows and the `o:site` membership are not built yet and are reported as gaps.

use omeka_db::ids::{ItemId, ItemSetId, MediaId};
use omeka_db::repos::{ItemRepo, ItemSetRepo, MediaRepo};
use omeka_db::rows::{ItemRow, MediaRow, NewItem};

use crate::errors::{ApiError, ValidationErrors};
use crate::read::{Api, ApiResponse, db_error, not_found, parse_id};
use crate::write::access;
use crate::write::body::Payload;
use crate::write::fulltext;
use crate::write::media;
use crate::write::resource::{self, not_found as ref_not_found};
use crate::write::values;

const ITEM_CLASS: &str = "Omeka\\Entity\\Item";

/// Members `hydrate` reads that are not property terms.
const CONTROL: &[&str] = &[
    "o:is_public",
    "o:owner",
    "o:resource_class",
    "o:resource_template",
    "o:thumbnail",
    "o:item_set",
    "o:primary_media",
    "o:site",
    "o:media",
];

pub async fn create(
    api: &Api<'_>,
    payload: &Payload,
) -> Result<crate::read::ApiResponse, ApiError> {
    validate_request(payload)?;
    let hydrated = resource::hydrate(api, payload, CONTROL, None, false).await?;
    let item_sets = item_sets(api, payload).await?;
    let row = ItemRepo::new(api.pool())
        .create(&NewItem {
            resource: hydrated.resource,
            primary_media_id: None,
        })
        .await
        .map_err(db_error)?;
    values::store(api, row.resource.id, &hydrated.values).await?;
    for id in item_sets {
        ItemRepo::new(api.pool())
            .add_to_item_set(row.id(), id)
            .await
            .map_err(db_error)?;
    }
    fulltext::reindex(api, "items", row.resource.id).await?;
    api.read_member("items", &row.resource.id.0.to_string())
        .await
}

pub async fn update(
    api: &Api<'_>,
    raw_id: &str,
    payload: &Payload,
    partial: bool,
) -> Result<crate::read::ApiResponse, ApiError> {
    validate_request(payload)?;
    let id = parse_id(ITEM_CLASS, raw_id)?;
    let repo = ItemRepo::new(api.pool());
    let Some(mut row) = repo.get(ItemId(id)).await.map_err(db_error)? else {
        return Err(not_found(ITEM_CLASS, raw_id));
    };
    api.authorize_resource(ITEM_CLASS, row.resource.owner_id)?;
    let hydrated = resource::hydrate(api, payload, CONTROL, Some(&row.resource), partial).await?;
    resource::apply(&mut row.resource, &hydrated.resource);
    row.primary_media_id = primary_media(api, payload, &row).await?;
    repo.update(&row).await.map_err(db_error)?;
    values::store(api, row.resource.id, &hydrated.values).await?;
    if !partial || payload.has("o:item_set") {
        let wanted = item_sets(api, payload).await?;
        apply_item_sets(api, row.id(), &wanted).await?;
    }
    fulltext::reindex(api, "items", row.resource.id).await?;
    api.read_member("items", raw_id).await
}

/// Delete an item, its media and their stored files, and the rows that cascade
/// from it.
///
/// `Item::$media` cascades remove (`Entity/Item.php:24`), and every removed
/// media deletes its files through `Module::deleteMediaFiles`, so the media go
/// before the item. `item_item_set` and `item_site` go with the database
/// cascade; a site block attachment and another resource's value keep their row
/// and lose the reference.
pub async fn delete(api: &Api<'_>, raw_id: &str) -> Result<ApiResponse, ApiError> {
    let id = parse_id(ITEM_CLASS, raw_id)?;
    let repo = ItemRepo::new(api.pool());
    let Some(row) = repo.get(ItemId(id)).await.map_err(db_error)? else {
        return Err(not_found(ITEM_CLASS, raw_id));
    };
    access::resource_entity(api.actor(), ITEM_CLASS, row.resource.owner_id)?;
    let answer = crate::read::item_document(api, &row).await?;
    for media in media_rows(api, row.id()).await? {
        media::remove(api, &media).await?;
    }
    repo.delete(row.id()).await.map_err(db_error)?;
    fulltext::remove(api, "items", row.resource.id).await?;
    Ok(answer)
}

/// Every media the item owns, in the order `MediaRepo` stores them.
async fn media_rows(api: &Api<'_>, item: ItemId) -> Result<Vec<MediaRow>, ApiError> {
    MediaRepo::new(api.pool())
        .list(
            &omeka_db::repos::MediaFilter {
                item_id: Some(item),
                ..Default::default()
            },
            &omeka_db::repos::Paging::default(),
        )
        .await
        .map_err(db_error)
}

/// `ItemAdapter::validateRequest`.
fn validate_request(payload: &Payload) -> Result<(), ApiError> {
    let mut errors = ValidationErrors::new();
    if payload.has("o:item_set") && !payload.get("o:item_set").is_some_and(|v| v.is_array()) {
        errors.insert(
            "o:item_set".to_string(),
            vec!["Item sets must be an array".to_string()],
        );
    }
    if payload.has("o:media") && !payload.get("o:media").is_some_and(|v| v.is_array()) {
        errors.insert(
            "o:media".to_string(),
            vec!["Media must be an array".to_string()],
        );
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(ApiError::Validation(errors))
    }
}

/// The item-set ids a body names, each checked to exist.
async fn item_sets(api: &Api<'_>, payload: &Payload) -> Result<Vec<ItemSetId>, ApiError> {
    let Some(array) = payload.get("o:item_set").and_then(|value| value.as_array()) else {
        return Ok(Vec::new());
    };
    let mut ids = Vec::new();
    for item in array {
        let raw = item
            .get("o:id")
            .and_then(|value| value.as_u64())
            .or_else(|| item.as_u64());
        let Some(raw) = raw else {
            continue;
        };
        let id = ItemSetId(raw);
        if ItemSetRepo::new(api.pool())
            .get(id)
            .await
            .map_err(db_error)?
            .is_none()
        {
            return Err(ref_not_found("Omeka\\Entity\\ItemSet", raw));
        }
        ids.push(id);
    }
    Ok(ids)
}

async fn apply_item_sets(
    api: &Api<'_>,
    item: ItemId,
    wanted: &[ItemSetId],
) -> Result<(), ApiError> {
    let repo = ItemRepo::new(api.pool());
    for existing in repo.list_item_sets(item).await.map_err(db_error)? {
        if !wanted.contains(&existing) {
            repo.remove_from_item_set(item, existing)
                .await
                .map_err(db_error)?;
        }
    }
    for id in wanted {
        repo.add_to_item_set(item, *id).await.map_err(db_error)?;
    }
    Ok(())
}

/// `ItemAdapter::hydrate`'s `o:primary_media`: the named media when it belongs
/// to this item, otherwise `None`.
async fn primary_media(
    api: &Api<'_>,
    payload: &Payload,
    row: &ItemRow,
) -> Result<Option<MediaId>, ApiError> {
    let Some(raw) = payload.ref_id("o:primary_media") else {
        return Ok(None);
    };
    let wanted = MediaId(raw);
    let media = MediaRepo::new(api.pool())
        .list(
            &omeka_db::repos::MediaFilter {
                item_id: Some(row.id()),
                ..Default::default()
            },
            &omeka_db::repos::Paging::default(),
        )
        .await
        .map_err(db_error)?;
    Ok(media.iter().find(|m| m.id() == wanted).map(|_| wanted))
}
