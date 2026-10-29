//! The counts behind a set's place, a record's "Linked from", an item type's fields and the Items
//! browse's item type chips: Romeka's own reads (`/api/romeka/item-sets/counts`,
//! `/api/romeka/links/{id}`, `/api/romeka/field-counts`, `/api/romeka/item-types/counts`);
//! upstream has none of these routes.
//!
//! Each count stands for a list the admin opens, so it must equal that list's `totalResults` for
//! the same caller: a set's count is `GET /api/items?item_set_id=<id>`, a link row's count is the
//! `res` property search of its kind's endpoint, and a field's count is the `ex` property search
//! on `GET /api/items?resource_template_id=<id>`. Both read the visibility the lists read
//! (`media_visibility`, which is `Actor::sees_resource` as SQL), so a hidden record is never
//! counted and a hidden or missing set or target answers the same.

use std::collections::BTreeSet;

use omeka_db::ids::{ItemSetId, ResourceId, TemplateId};
use omeka_db::repos::{
    ItemRepo, ItemSetRepo, MediaVisibility, RelationRepo, SetItemCount, TemplateRepo, ValueRepo,
};
use omeka_db::rows::resource_type;
use omeka_db::with_db;
use serde::Serialize;

use super::params::Params;
use super::query::media_visibility;
use super::response::{ApiResponse, serialize};
use super::{Api, db_error, not_found};
use crate::errors::ApiError;
use crate::messages;

/// The most distinct set ids one counts request may name: more than any page of the set list
/// shows. A request past it is a 400, so a client never reads a silently shortened answer.
pub const MOST_SET_IDS: usize = 200;

/// Ids per statement when every set of a list is counted.
const ID_CHUNK: usize = 1000;

#[derive(Serialize)]
struct SetCounts {
    item_sets: Vec<SetCount>,
}

#[derive(Serialize)]
struct SetCount {
    id: u64,
    items: i64,
}

/// `GET /api/romeka/item-sets/counts?id[]=..`: the caller-visible item count of each named set
/// the caller may see, in id order. A set the caller may not see, one that does not exist, and
/// an id that is not a positive integer are left out.
pub async fn item_set_counts(api: &Api<'_>, params: &Params) -> Result<ApiResponse, ApiError> {
    let ids: BTreeSet<u64> = params
        .id_values()
        .iter()
        .filter_map(|raw| raw.parse::<u64>().ok())
        .filter(|id| *id > 0 && i64::try_from(*id).is_ok())
        .collect();
    if ids.len() > MOST_SET_IDS {
        return Err(ApiError::bad_request(messages::BAD_REQUEST));
    }
    let ids: Vec<ItemSetId> = ids.into_iter().map(ItemSetId).collect();
    let visibility = media_visibility(api.actor());
    let counts = item_counts(api, &ids, visibility, visibility).await?;
    Ok(ApiResponse::document(serialize(&SetCounts {
        item_sets: counts
            .into_iter()
            .map(|count| SetCount {
                id: count.item_set.0,
                items: count.items,
            })
            .collect(),
    })?))
}

/// The item counts of these sets, a chunk of ids per statement.
pub(crate) async fn item_counts(
    api: &Api<'_>,
    ids: &[ItemSetId],
    sets: MediaVisibility,
    items: MediaVisibility,
) -> Result<Vec<SetItemCount>, ApiError> {
    let mut counts = Vec::with_capacity(ids.len());
    for chunk in ids.chunks(ID_CHUNK) {
        counts.extend(
            with_db!(api.pool(), |db| ItemSetRepo::new(db)
                .item_counts(chunk, sets, items)
                .await)
            .map_err(db_error)?,
        );
    }
    Ok(counts)
}

/// The kinds a link row names, in the order the rows come, with the endpoint each one opens.
const KINDS: [(&str, &str); 3] = [
    (resource_type::ITEM, "items"),
    (resource_type::ITEM_SET, "item_sets"),
    (resource_type::MEDIA, "media"),
];

#[derive(Serialize)]
struct Links {
    links: Vec<Link>,
}

#[derive(Serialize)]
struct Link {
    kind: &'static str,
    property_id: u64,
    term: String,
    label: String,
    count: i64,
}

/// `GET /api/romeka/links/{id}`: the resources the caller may see whose values point at this
/// item, item set or media, one row per (kind, property). A target the caller may not see and
/// a missing one are the same 404.
pub async fn links(api: &Api<'_>, raw_id: &str) -> Result<ApiResponse, ApiError> {
    const CLASS: &str = "Omeka\\Entity\\Resource";
    let id = raw_id
        .parse::<u64>()
        .ok()
        .filter(|id| *id > 0 && i64::try_from(*id).is_ok())
        .ok_or_else(|| not_found(CLASS, raw_id))?;
    let target = with_db!(api.pool(), |db| RelationRepo::new(db)
        .resources(&[ResourceId(id)])
        .await)
    .map_err(db_error)?
    .into_iter()
    .next()
    .filter(|row| KINDS.iter().any(|(kind, _)| *kind == row.resource_type))
    .filter(|row| api.sees(row.is_public, row.owner_id))
    .ok_or_else(|| not_found(CLASS, raw_id))?;
    let counts = with_db!(api.pool(), |db| ValueRepo::new(db)
        .link_counts(target.id, media_visibility(api.actor()))
        .await)
    .map_err(db_error)?;
    api.fill_catalog().await?;
    let mut links = Vec::with_capacity(counts.len());
    for count in counts {
        let Some((order, kind)) = KINDS
            .iter()
            .enumerate()
            .find(|(_, (stored, _))| *stored == count.resource_type)
            .map(|(order, (_, kind))| (order, *kind))
        else {
            // A value annotation's value: no list of the admin opens it.
            continue;
        };
        let property = api.catalog().property(count.property).ok_or_else(|| {
            ApiError::internal(format!(
                "value property {} is not in the catalog",
                count.property.0
            ))
        })?;
        links.push((
            order,
            Link {
                kind,
                property_id: count.property.0,
                term: property.term.clone(),
                label: property.label.clone(),
                count: count.resources,
            },
        ));
    }
    links.sort_by(|(a_order, a), (b_order, b)| {
        a_order
            .cmp(b_order)
            .then(b.count.cmp(&a.count))
            .then_with(|| a.term.cmp(&b.term))
    });
    Ok(ApiResponse::document(serialize(&Links {
        links: links.into_iter().map(|(_, link)| link).collect(),
    })?))
}

#[derive(Serialize)]
struct FieldCounts {
    total: i64,
    fields: Vec<FieldCount>,
}

#[derive(Serialize)]
struct FieldCount {
    property_id: u64,
    filled: i64,
}

/// `GET /api/romeka/field-counts?resource_template_id=<id>`: how many items of this type the
/// caller may see, and per property how many of them have a value for it. A property no counted
/// item fills is left out. A missing type and an id that is not a positive integer are the same
/// 404; a request that names no type is a 400.
pub async fn field_counts(api: &Api<'_>, params: &Params) -> Result<ApiResponse, ApiError> {
    const CLASS: &str = "Omeka\\Entity\\ResourceTemplate";
    let raw = params
        .get("resource_template_id")
        .ok_or_else(|| ApiError::bad_request(messages::BAD_REQUEST))?;
    let id = raw
        .parse::<u64>()
        .ok()
        .filter(|id| *id > 0 && i64::try_from(*id).is_ok())
        .map(TemplateId)
        .ok_or_else(|| not_found(CLASS, raw))?;
    with_db!(api.pool(), |db| TemplateRepo::new(db).get(id).await)
        .map_err(db_error)?
        .ok_or_else(|| not_found(CLASS, raw))?;
    let counts = with_db!(api.pool(), |db| ItemRepo::new(db)
        .field_counts(id, media_visibility(api.actor()))
        .await)
    .map_err(db_error)?;
    Ok(ApiResponse::document(serialize(&FieldCounts {
        total: counts.total,
        fields: counts
            .fields
            .into_iter()
            .map(|field| FieldCount {
                property_id: field.property.0,
                filled: field.items,
            })
            .collect(),
    })?))
}

#[derive(Serialize)]
struct ItemTypeCounts {
    item_types: Vec<ItemTypeCount>,
    no_item_type: i64,
    with_class: i64,
}

#[derive(Serialize)]
struct ItemTypeCount {
    id: u64,
    items: i64,
}

/// `GET /api/romeka/item-types/counts`: the Items browse's item type chips. Every item type, in id
/// order, with the number of items the Items list answers this caller for
/// `resource_template_id=<id>` (an unused type answers 0); then `romeka_no_resource_template=1`
/// as `no_item_type` and `romeka_has_resource_class=1` as `with_class`. Each is a type's total,
/// not a total under any other filter.
pub async fn item_type_counts(api: &Api<'_>) -> Result<ApiResponse, ApiError> {
    let templates =
        with_db!(api.pool(), |db| TemplateRepo::new(db).list().await).map_err(db_error)?;
    let mut item_types = Vec::with_capacity(templates.len());
    for template in templates {
        item_types.push(ItemTypeCount {
            id: template.id.0,
            items: listed(api, "resource_template_id", &template.id.0.to_string()).await?,
        });
    }
    Ok(ApiResponse::document(serialize(&ItemTypeCounts {
        item_types,
        no_item_type: listed(api, "romeka_no_resource_template", "1").await?,
        with_class: listed(api, "romeka_has_resource_class", "1").await?,
    })?))
}

/// How far back the dashboard's "changed by others" reaches.
const CHANGED_DAYS: i64 = 7;

#[derive(Serialize)]
struct Dashboard {
    no_item_set: i64,
    no_resource_template: i64,
    changed_by_others: ChangedByOthers,
}

#[derive(Serialize)]
struct ChangedByOthers {
    /// The `romeka_changed_by_others_since` value the count was read with, so the list it opens
    /// is the same list.
    since: String,
    items: i64,
}

/// `GET /api/romeka/dashboard`: the dashboard's work counts. Each is the number of items the Items
/// list answers this caller for its filter, counted in SQL over that list's own matching: items in no
/// set (`romeka_no_item_set=1`), with no item type (`romeka_no_resource_template=1`), and changed
/// by someone else in the last seven days (`romeka_changed_by_others_since=<since>`).
pub async fn dashboard(api: &Api<'_>) -> Result<ApiResponse, ApiError> {
    let since = (chrono::Utc::now() - chrono::Duration::days(CHANGED_DAYS))
        .format("%Y-%m-%dT%H:%M:%SZ")
        .to_string();
    Ok(ApiResponse::document(serialize(&Dashboard {
        no_item_set: listed(api, "romeka_no_item_set", "1").await?,
        no_resource_template: listed(api, "romeka_no_resource_template", "1").await?,
        changed_by_others: ChangedByOthers {
            items: listed(api, "romeka_changed_by_others_since", &since).await?,
            since,
        },
    })?))
}

/// The total of `GET /api/items?<name>=<value>` for this caller.
async fn listed(api: &Api<'_>, name: &str, value: &str) -> Result<i64, ApiError> {
    let params = Params::from_decoded(vec![(name.to_string(), value.to_string())]);
    super::matching_item_count(api, &params).await
}
