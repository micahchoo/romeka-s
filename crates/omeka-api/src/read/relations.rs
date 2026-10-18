//! The rows a page of resource documents needs, loaded once per page.
//!
//! Building one item document used to issue its own queries: the item's media,
//! its item-set memberships, its values and the values that point at it
//! (`resources.rs` before the batching change). A 25-row page therefore issued
//! about a hundred queries, which the perf harness measured as the list
//! endpoint's limiter at 53 us of per-row cost (`verification/perf/README.md`).
//!
//! This module loads the same relations for the whole page in a bounded number
//! of queries over the page's ids, and the document builders read them by id.
//! The query count per page is the number of relations, not the number of rows:
//! media, item-set memberships, values (one per annotation level, at most
//! [`MAX_ANNOTATION_DEPTH`] + 1), the reverse references, the referenced
//! resources, the thumbnail assets and the template properties.
//!
//! SQL sits here rather than in `omeka-db` because this wave forbids editing
//! that crate, the same reason `system.rs` carries its own row queries. Every
//! row still parses through the `omeka_db::rows` struct for its table, so the
//! boundary is unchanged.

use std::collections::{BTreeSet, HashMap, HashSet};

use omeka_db::ids::{ItemSetId, MediaId, PropertyId, TemplateId};
use omeka_db::rows::{AssetRow, MediaRow, ResourceRow, ValueRow, resource_type};
use sqlx::Row;
use sqlx::SqlitePool;
use sqlx::sqlite::SqliteRow;

use super::values::MAX_ANNOTATION_DEPTH;
use super::{Api, db_error};
use crate::errors::ApiError;

/// Which per-kind relations a page of documents needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Page {
    /// `items`: each row reads its media and its item-set memberships.
    Items,
    /// `item_sets`: each row reads the first item of the set and that item's
    /// media (`ItemSetRepresentation::primaryMedia`).
    ItemSets,
    /// `media` and `value_annotations`: values and reverse references only.
    Plain,
}

/// Every relation a page of documents reads, keyed by the id it hangs off.
#[derive(Debug, Default)]
pub(crate) struct Relations {
    values: HashMap<u64, Vec<ValueRow>>,
    resources: HashMap<u64, ResourceRow>,
    media: HashMap<u64, Vec<MediaRow>>,
    item_sets: HashMap<u64, Vec<ItemSetId>>,
    pointing: HashMap<u64, Vec<ValueRow>>,
    template_properties: HashMap<u64, Vec<PropertyId>>,
    /// `ItemSetRepresentation::primaryMedia` reads the first item of the set by
    /// id, then that item's stored primary media. The value is the item id and
    /// its `primary_media_id`.
    first_item: HashMap<u64, (u64, Option<MediaId>)>,
    /// The asset each page row's `resource.thumbnail_id` names, keyed by asset
    /// id, for `AbstractRepresentation::thumbnailDisplayUrl`.
    assets: HashMap<u64, AssetRow>,
}

impl Relations {
    /// Load every relation the page's documents read.
    ///
    /// `targets` is each page row's id and its resource template, in page
    /// order. One query per relation, each over the whole id set.
    pub(crate) async fn load<I>(api: &Api<'_>, page: Page, targets: I) -> Result<Self, ApiError>
    where
        I: IntoIterator<Item = (u64, Option<TemplateId>)>,
    {
        let mut ids = Vec::new();
        let mut templates = Vec::new();
        for (id, template) in targets {
            ids.push(id);
            if let Some(template) = template {
                templates.push(template);
            }
        }
        let mut relations = Self::default();
        if ids.is_empty() {
            return Ok(relations);
        }
        match page {
            Page::Items => {
                relations.media = media_for(api.pool(), &ids).await?;
                relations.item_sets = item_sets_for(api.pool(), &ids).await?;
            }
            Page::ItemSets => {
                relations.first_item = first_item_for(api.pool(), &ids).await?;
                let item_ids: Vec<u64> = relations
                    .first_item
                    .values()
                    .map(|(item, _)| *item)
                    .collect();
                relations.media = media_for(api.pool(), &item_ids).await?;
            }
            Page::Plain => {}
        }
        relations.add_shared(api, &ids, &templates).await?;
        Ok(relations)
    }

    /// Load the relations a page of mixed kinds needs: each item's media and
    /// item-set memberships, each item set's first item and that item's media,
    /// plus the relations every document reads. `ResourceAdapter::search`
    /// returns rows of all four kinds in one page, so one call covers them.
    pub(crate) async fn load_many<'r, I>(api: &Api<'_>, rows: I) -> Result<Self, ApiError>
    where
        I: IntoIterator<Item = &'r ResourceRow>,
    {
        let mut ids = Vec::new();
        let mut templates = Vec::new();
        let mut item_ids = Vec::new();
        let mut set_ids = Vec::new();
        for row in rows {
            ids.push(row.id.0);
            if let Some(template) = row.resource_template_id {
                templates.push(template);
            }
            match row.resource_type.as_str() {
                resource_type::ITEM => item_ids.push(row.id.0),
                resource_type::ITEM_SET => set_ids.push(row.id.0),
                _ => {}
            }
        }
        let mut relations = Self::default();
        if ids.is_empty() {
            return Ok(relations);
        }
        relations.media = media_for(api.pool(), &item_ids).await?;
        relations.item_sets = item_sets_for(api.pool(), &item_ids).await?;
        relations.first_item = first_item_for(api.pool(), &set_ids).await?;
        let first_items: Vec<u64> = relations
            .first_item
            .values()
            .map(|(item, _)| *item)
            .collect();
        // A kind id is unique across kinds, so a set's first item is never an
        // item already on the page and the two maps do not collide.
        relations
            .media
            .extend(media_for(api.pool(), &first_items).await?);
        relations.add_shared(api, &ids, &templates).await?;
        Ok(relations)
    }

    /// The relations every document reads, whatever its kind: the values, the
    /// values pointing at each resource, the referenced resource rows and the
    /// template properties.
    async fn add_shared(
        &mut self,
        api: &Api<'_>,
        ids: &[u64],
        templates: &[TemplateId],
    ) -> Result<(), ApiError> {
        self.values = values_for(api.pool(), ids).await?;
        self.pointing = pointing_at(api.pool(), ids).await?;
        // The resources every document may print: the targets of its
        // resource-valued values, and the subjects of its reverse references.
        let mut referenced = BTreeSet::new();
        for rows in self.values.values() {
            for row in rows {
                if let Some(id) = row.value_resource_id {
                    referenced.insert(id.0);
                }
            }
        }
        for rows in self.pointing.values() {
            for row in rows {
                referenced.insert(row.resource_id.0);
            }
        }
        let referenced: Vec<u64> = referenced.into_iter().collect();
        self.resources = resources_for(api.pool(), &referenced).await?;
        self.assets = thumbnail_assets_for(api.pool(), ids).await?;
        self.template_properties = template_properties_for(api.pool(), templates).await?;
        Ok(())
    }

    /// The values on a resource, in upstream's id order.
    pub(crate) fn values_of(&self, resource: u64) -> &[ValueRow] {
        self.values.get(&resource).map_or(&[], Vec::as_slice)
    }

    /// A referenced resource row, when it exists.
    pub(crate) fn resource(&self, id: u64) -> Option<&ResourceRow> {
        self.resources.get(&id)
    }

    /// The media rows of an item, in `position` order.
    pub(crate) fn media_of(&self, item: u64) -> &[MediaRow] {
        self.media.get(&item).map_or(&[], Vec::as_slice)
    }

    /// The item sets an item belongs to, in id order.
    pub(crate) fn item_sets_of(&self, item: u64) -> &[ItemSetId] {
        self.item_sets.get(&item).map_or(&[], Vec::as_slice)
    }

    /// The values whose `value_resource_id` points at a resource, in id order.
    pub(crate) fn pointing_at(&self, resource: u64) -> &[ValueRow] {
        self.pointing.get(&resource).map_or(&[], Vec::as_slice)
    }

    /// A template's properties, in upstream's `position` order.
    pub(crate) fn template_properties(&self, template: u64) -> &[PropertyId] {
        self.template_properties
            .get(&template)
            .map_or(&[], Vec::as_slice)
    }

    /// The first item of an item set and its stored primary media.
    pub(crate) fn first_item_of(&self, set: u64) -> Option<(u64, Option<MediaId>)> {
        self.first_item.get(&set).copied()
    }

    /// The asset a resource's `thumbnail_id` names, when it is on this page.
    pub(crate) fn asset(&self, id: u64) -> Option<&AssetRow> {
        self.assets.get(&id)
    }
}

/// The `?` list for an `IN (…)` clause of `count` ids.
fn placeholders(count: usize) -> String {
    let mut sql = String::from("?");
    for _ in 1..count {
        sql.push_str(", ?");
    }
    sql
}

/// Fetch every row of one query whose `IN` clause carries `ids`.
async fn rows_in(pool: &SqlitePool, sql: &str, ids: &[u64]) -> Result<Vec<SqliteRow>, ApiError> {
    let mut query = sqlx::query(sql);
    for id in ids {
        query = query.bind(*id as i64);
    }
    query.fetch_all(pool).await.map_err(ApiError::internal)
}

/// Every media row of an item, ordered as `MediaRepo::list` orders them.
pub(crate) async fn media_for(
    pool: &SqlitePool,
    items: &[u64],
) -> Result<HashMap<u64, Vec<MediaRow>>, ApiError> {
    if items.is_empty() {
        return Ok(HashMap::new());
    }
    let sql = format!(
        "SELECT resource.*, media.item_id, media.ingester, media.renderer, media.data, \
         media.source, media.media_type, media.storage_id, media.extension, media.sha256, \
         media.size, media.has_original, media.has_thumbnails, media.position, media.lang, \
         media.alt_text \
         FROM media JOIN resource ON resource.id = media.id \
         WHERE media.item_id IN ({}) \
         ORDER BY media.item_id, media.position, media.id",
        placeholders(items.len())
    );
    let rows = rows_in(pool, &sql, items).await?;
    let mut media: HashMap<u64, Vec<MediaRow>> = HashMap::new();
    for row in &rows {
        let parsed = MediaRow::from_row(row).map_err(db_error)?;
        media.entry(parsed.item_id.0).or_default().push(parsed);
    }
    Ok(media)
}

/// The item-set ids of each item, ordered as `ItemRepo::list_item_sets` orders
/// them.
pub(crate) async fn item_sets_for(
    pool: &SqlitePool,
    items: &[u64],
) -> Result<HashMap<u64, Vec<ItemSetId>>, ApiError> {
    if items.is_empty() {
        return Ok(HashMap::new());
    }
    let sql = format!(
        "SELECT item_id, item_set_id FROM item_item_set WHERE item_id IN ({}) \
         ORDER BY item_id, item_set_id",
        placeholders(items.len())
    );
    let rows = rows_in(pool, &sql, items).await?;
    let mut memberships: HashMap<u64, Vec<ItemSetId>> = HashMap::new();
    for row in &rows {
        let item: i64 = row.try_get("item_id").map_err(ApiError::internal)?;
        let set: i64 = row.try_get("item_set_id").map_err(ApiError::internal)?;
        memberships
            .entry(item as u64)
            .or_default()
            .push(ItemSetId(set as u64));
    }
    Ok(memberships)
}

/// The first item of each item set by id, with its stored primary media.
///
/// `ItemRepo::list` orders by `resource.id` and applies no visibility filter,
/// so the smallest member id is the row it returns first.
pub(crate) async fn first_item_for(
    pool: &SqlitePool,
    sets: &[u64],
) -> Result<HashMap<u64, (u64, Option<MediaId>)>, ApiError> {
    if sets.is_empty() {
        return Ok(HashMap::new());
    }
    let sql = format!(
        "SELECT member.item_set_id AS item_set_id, item.id AS item_id, \
         item.primary_media_id AS primary_media_id \
         FROM (SELECT item_set_id, MIN(item_id) AS item_id FROM item_item_set \
               WHERE item_set_id IN ({}) GROUP BY item_set_id) member \
         JOIN item ON item.id = member.item_id",
        placeholders(sets.len())
    );
    let rows = rows_in(pool, &sql, sets).await?;
    let mut first: HashMap<u64, (u64, Option<MediaId>)> = HashMap::new();
    for row in &rows {
        let set: i64 = row.try_get("item_set_id").map_err(ApiError::internal)?;
        let item: i64 = row.try_get("item_id").map_err(ApiError::internal)?;
        let primary: Option<i64> = row
            .try_get("primary_media_id")
            .map_err(ApiError::internal)?;
        first.insert(
            set as u64,
            (item as u64, primary.map(|id| MediaId(id as u64))),
        );
    }
    Ok(first)
}

/// The asset each page row's `resource.thumbnail_id` names, keyed by asset id.
pub(crate) async fn thumbnail_assets_for(
    pool: &SqlitePool,
    resources: &[u64],
) -> Result<HashMap<u64, AssetRow>, ApiError> {
    if resources.is_empty() {
        return Ok(HashMap::new());
    }
    let sql = format!(
        "SELECT id, owner_id, name, media_type, storage_id, extension, alt_text FROM asset \
         WHERE id IN (SELECT thumbnail_id FROM resource \
                      WHERE thumbnail_id IS NOT NULL AND id IN ({}))",
        placeholders(resources.len())
    );
    let rows = rows_in(pool, &sql, resources).await?;
    let mut assets = HashMap::new();
    for row in &rows {
        let asset = AssetRow::from_row(row).map_err(db_error)?;
        assets.insert(asset.id.0, asset);
    }
    Ok(assets)
}

/// Every value on each resource, plus the values on the annotations those
/// values carry, to the depth the builder follows them.
pub(crate) async fn values_for(
    pool: &SqlitePool,
    resources: &[u64],
) -> Result<HashMap<u64, Vec<ValueRow>>, ApiError> {
    let mut by_resource: HashMap<u64, Vec<ValueRow>> = HashMap::new();
    let mut loaded: HashSet<u64> = HashSet::new();
    let mut frontier: Vec<u64> = resources
        .iter()
        .copied()
        .filter(|id| loaded.insert(*id))
        .collect();
    // Level 0 is the page's resources; each later level is the annotations the
    // previous level's values carry. `build_value` follows an annotation only
    // below `MAX_ANNOTATION_DEPTH`, so the deepest resource it reads sits at
    // that depth, and the range is inclusive.
    for _ in 0..=MAX_ANNOTATION_DEPTH {
        if frontier.is_empty() {
            break;
        }
        let sql = format!(
            "SELECT * FROM value WHERE resource_id IN ({}) ORDER BY resource_id, id",
            placeholders(frontier.len())
        );
        let rows = rows_in(pool, &sql, &frontier).await?;
        let mut next = Vec::new();
        for row in &rows {
            let parsed = ValueRow::from_row(row).map_err(db_error)?;
            if let Some(annotation) = parsed.value_annotation_id {
                let id = annotation.as_resource().0;
                if loaded.insert(id) {
                    next.push(id);
                }
            }
            by_resource
                .entry(parsed.resource_id.0)
                .or_default()
                .push(parsed);
        }
        frontier = next;
    }
    Ok(by_resource)
}

/// Every value pointing at each resource, ordered as
/// `ValueRepo::list_pointing_at` orders them.
pub(crate) async fn pointing_at(
    pool: &SqlitePool,
    resources: &[u64],
) -> Result<HashMap<u64, Vec<ValueRow>>, ApiError> {
    let sql = format!(
        "SELECT * FROM value WHERE value_resource_id IN ({}) ORDER BY id",
        placeholders(resources.len())
    );
    let rows = rows_in(pool, &sql, resources).await?;
    let mut pointing: HashMap<u64, Vec<ValueRow>> = HashMap::new();
    for row in &rows {
        let parsed = ValueRow::from_row(row).map_err(db_error)?;
        if let Some(target) = parsed.value_resource_id {
            pointing.entry(target.0).or_default().push(parsed);
        }
    }
    Ok(pointing)
}

/// The referenced resource rows, by id.
pub(crate) async fn resources_for(
    pool: &SqlitePool,
    ids: &[u64],
) -> Result<HashMap<u64, ResourceRow>, ApiError> {
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    let sql = format!(
        "SELECT id, owner_id, resource_class_id, resource_template_id, thumbnail_id, title, \
         is_public, created, modified, resource_type FROM resource WHERE id IN ({})",
        placeholders(ids.len())
    );
    let rows = rows_in(pool, &sql, ids).await?;
    let mut resources = HashMap::new();
    for row in &rows {
        let parsed = ResourceRow::from_row(row).map_err(db_error)?;
        resources.insert(parsed.id.0, parsed);
    }
    Ok(resources)
}

/// The property ids of each resource template, in upstream's order.
pub(crate) async fn template_properties_for(
    pool: &SqlitePool,
    templates: &[TemplateId],
) -> Result<HashMap<u64, Vec<PropertyId>>, ApiError> {
    let ids: Vec<u64> = templates
        .iter()
        .copied()
        .map(u64::from)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    let sql = format!(
        "SELECT resource_template_id, property_id FROM resource_template_property \
         WHERE resource_template_id IN ({}) ORDER BY resource_template_id, position, id",
        placeholders(ids.len())
    );
    let rows = rows_in(pool, &sql, &ids).await?;
    let mut properties: HashMap<u64, Vec<PropertyId>> = HashMap::new();
    for row in &rows {
        let template: i64 = row
            .try_get("resource_template_id")
            .map_err(ApiError::internal)?;
        let property: i64 = row.try_get("property_id").map_err(ApiError::internal)?;
        properties
            .entry(template as u64)
            .or_default()
            .push(PropertyId(property as u64));
    }
    Ok(properties)
}
