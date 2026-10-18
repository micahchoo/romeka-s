//! `POST` and `PUT` for `items`.
//!
//! `ItemAdapter::validateRequest` rejects a non-array `o:item_set` or
//! `o:media`; `hydrate` then sets `o:primary_media`, the item-set membership
//! and the site membership on top of the shared resource hydration. A full
//! `PUT` replaces all three, so an omitted `o:item_set` or `o:site` empties the
//! membership. Inline `o:media` rows are not built yet and are reported as a
//! gap.
//!
//! `o:site` follows `ItemAdapter::hydrate` (`Api/Adapter/ItemAdapter.php:233-276`)
//! exactly: a create that does not name the member takes every site whose
//! `assign_new_items` is true, and a named site is attached only when the
//! caller may assign items there or, on a create, the site assigns new items.

use omeka_db::ids::{ItemId, ItemSetId, MediaId, SiteId};
use omeka_db::repos::{ItemRepo, ItemSetRepo, MediaRepo, Paging, SiteFilter, SiteRepo};
use omeka_db::rows::{ItemRow, MediaRow, NewItem, SiteRow};
use serde_json::Value;

use crate::errors::{ApiError, ValidationErrors};
use crate::read::{Api, ApiResponse, db_error, not_found, parse_id};
use crate::write::access::{self, SiteRole};
use crate::write::body::Payload;
use crate::write::fulltext;
use crate::write::media;
use crate::write::resource::{self, not_found as ref_not_found};
use crate::write::values;

const ITEM_CLASS: &str = "Omeka\\Entity\\Item";
const SITE_CLASS: &str = "Omeka\\Entity\\Site";

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
    let sites = sites(api, payload, true, false, &[]).await?;
    let row = ItemRepo::new(api.pool())
        .create(&NewItem {
            resource: hydrated.resource,
            primary_media_id: None,
        })
        .await
        .map_err(db_error)?;
    resource::store_values(
        api,
        row.resource.id,
        &hydrated.values,
        hydrated.keep_stored_values,
    )
    .await?;
    for id in item_sets {
        ItemRepo::new(api.pool())
            .add_to_item_set(row.id(), id)
            .await
            .map_err(db_error)?;
    }
    apply_sites(api, row.id(), sites).await?;
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
    // Every reference the body names is checked before the first write, so a
    // failure leaves the stored row unchanged: upstream hydrates (and so
    // resolves every `findEntity`) before its single flush.
    let wanted_item_sets = if !partial || payload.has("o:item_set") {
        Some(item_sets(api, payload).await?)
    } else {
        None
    };
    let existing_sites = repo.list_sites(row.id()).await.map_err(db_error)?;
    let sites = sites(api, payload, false, partial, &existing_sites).await?;
    let hydrated = resource::hydrate(api, payload, CONTROL, Some(&row.resource), partial).await?;
    resource::apply(&mut row.resource, &hydrated.resource);
    row.primary_media_id = primary_media(api, payload, &row).await?;
    repo.update(&row).await.map_err(db_error)?;
    resource::store_values(
        api,
        row.resource.id,
        &hydrated.values,
        hydrated.keep_stored_values,
    )
    .await?;
    if let Some(wanted) = wanted_item_sets {
        apply_item_sets(api, row.id(), &wanted).await?;
    }
    apply_sites(api, row.id(), sites).await?;
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
    values::clear(api, row.resource.id).await?;
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

/// `ItemAdapter::hydrate`'s `o:site`: the memberships one write adds and drops.
struct SitePlan {
    attach: Vec<SiteId>,
    detach: Vec<SiteId>,
}

/// A site reference as `ItemAdapter::hydrate` reads one: `{"o:id": N}` or a
/// bare numeric. An object whose `o:id` is not numeric still names a site, and
/// the lookup then answers 404, as `findEntity` does.
enum SiteRef {
    Id(u64),
    Unknown(String),
}

fn site_ref(entry: &Value) -> Option<SiteRef> {
    if let Some(object) = entry.as_object() {
        let id = object.get("o:id")?;
        return Some(match numeric(id) {
            Some(raw) => SiteRef::Id(raw),
            None => SiteRef::Unknown(id.to_string()),
        });
    }
    numeric(entry).map(SiteRef::Id)
}

fn numeric(value: &Value) -> Option<u64> {
    value.as_u64().or_else(|| {
        value
            .as_str()
            .and_then(|raw| raw.trim().parse::<u64>().ok())
    })
}

/// Resolve the membership changes one write makes, before it writes anything.
///
/// `None` means the write leaves the memberships alone: a partial update that
/// does not name `o:site`. Every other case returns a plan, because a full
/// `PUT` and a `PATCH` that names the member both replace it upstream.
async fn sites(
    api: &Api<'_>,
    payload: &Payload,
    is_create: bool,
    partial: bool,
    existing: &[SiteId],
) -> Result<Option<SitePlan>, ApiError> {
    let named = payload.get("o:site").and_then(Value::as_array);
    if !is_create && partial && named.is_none() {
        return Ok(None);
    }
    // On CREATE without an `o:site` array, take every site that assigns new
    // items. The `elseif` below only runs when the member should hydrate.
    if is_create && named.is_none() {
        let rows = SiteRepo::new(api.pool())
            .list(
                &SiteFilter {
                    assign_new_items: Some(true),
                    ..SiteFilter::default()
                },
                &Paging::default(),
            )
            .await
            .map_err(db_error)?;
        return Ok(Some(SitePlan {
            attach: rows.into_iter().map(|row| row.id).collect(),
            detach: Vec::new(),
        }));
    }

    let repo = SiteRepo::new(api.pool());
    let mut retained: Vec<SiteId> = Vec::new();
    let mut attach: Vec<SiteId> = Vec::new();
    for entry in named.into_iter().flatten() {
        let Some(reference) = site_ref(entry) else {
            continue;
        };
        let site = match reference {
            SiteRef::Id(raw) => SiteId(raw),
            SiteRef::Unknown(raw) => return Err(crate::read::not_found(SITE_CLASS, &raw)),
        };
        if !retained.contains(&site) {
            retained.push(site);
        }
        if existing.contains(&site) {
            continue;
        }
        let Some(row) = repo.get(site).await.map_err(db_error)? else {
            return Err(ref_not_found(SITE_CLASS, site.0));
        };
        if can_assign(api, &row).await? || (is_create && row.assign_new_items) {
            attach.push(site);
        }
    }

    let mut detach = Vec::new();
    if !is_create {
        for site in existing {
            if retained.contains(site) {
                continue;
            }
            if let Some(row) = repo.get(*site).await.map_err(db_error)?
                && can_assign(api, &row).await?
            {
                detach.push(*site);
            }
        }
    }
    Ok(Some(SitePlan { attach, detach }))
}

/// `Acl::userIsAllowed($site, 'can-assign-items')`: the site's owner, or a
/// `site_permission` role of admin or editor.
async fn can_assign(api: &Api<'_>, site: &SiteRow) -> Result<bool, ApiError> {
    if access::is_admin_role(api.actor().role()) {
        return Ok(true);
    }
    let Some(user) = api.actor().user() else {
        return Ok(false);
    };
    if site.owner_id == Some(user) {
        return Ok(true);
    }
    let permissions = SiteRepo::new(api.pool())
        .list_permissions(site.id)
        .await
        .map_err(db_error)?;
    Ok(access::has_site_permission(
        &permissions,
        user,
        SiteRole::Editor,
    ))
}

async fn apply_sites(api: &Api<'_>, item: ItemId, plan: Option<SitePlan>) -> Result<(), ApiError> {
    let Some(plan) = plan else {
        return Ok(());
    };
    let repo = ItemRepo::new(api.pool());
    for site in plan.detach {
        repo.remove_from_site(item, site).await.map_err(db_error)?;
    }
    for site in plan.attach {
        repo.add_to_site(item, site).await.map_err(db_error)?;
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
