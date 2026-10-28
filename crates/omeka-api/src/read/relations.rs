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
//! The statements are `omeka_db::repos::RelationRepo`'s, one per relation;
//! this module groups their rows by id and walks the annotation levels.

use std::collections::{BTreeSet, HashMap, HashSet};

use omeka_db::DbPool;
use omeka_db::ids::{ItemId, ItemSetId, MediaId, PropertyId, ResourceId, SiteId, TemplateId};
use omeka_db::repos::{MediaDerivativeRepo, MediaVisibility, RelationRepo};
use omeka_db::rows::{AssetRow, DerivativeRow, MediaRow, ResourceRow, ValueRow, resource_type};
use omeka_db::with_db;

use super::query::media_visibility;
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
    sites: HashMap<u64, Vec<SiteId>>,
    pointing: HashMap<u64, Vec<ValueRow>>,
    template_properties: HashMap<u64, Vec<PropertyId>>,
    /// `ItemSetRepresentation::primaryMedia` reads the first item of the set by
    /// id, then that item's stored primary media. The value is the item id and
    /// its `primary_media_id`.
    first_item: HashMap<u64, (u64, Option<MediaId>)>,
    /// The asset each page row's `resource.thumbnail_id` names, keyed by asset
    /// id, for `AbstractRepresentation::thumbnailDisplayUrl`.
    assets: HashMap<u64, AssetRow>,
    /// Whether each item that owns a media the page shows outside its own
    /// item is public, for the media's file URLs.
    owners: HashMap<u64, bool>,
    /// What each page media's derivatives are doing, for
    /// `o-module-romeka:preview`, keyed by media id.
    derivatives: HashMap<u64, Vec<DerivativeRow>>,
    /// The title each page or referenced resource shows instead of its stored
    /// one, where the stored title came from a value the caller may not see.
    hidden_titles: HashMap<u64, Option<String>>,
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
                relations.item_sets = item_sets_for(api, &ids).await?;
                relations.sites = sites_for(api.pool(), &ids).await?;
            }
            Page::ItemSets => {
                relations.first_item = first_item_for(api.pool(), &ids).await?;
                let item_ids: Vec<u64> = relations
                    .first_item
                    .values()
                    .map(|(item, _)| *item)
                    .collect();
                relations.media = media_for(api.pool(), &item_ids).await?;
                relations.load_owners(api, item_ids).await?;
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
        relations.item_sets = item_sets_for(api, &item_ids).await?;
        relations.sites = sites_for(api.pool(), &item_ids).await?;
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
        relations.load_owners(api, first_items).await?;
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
        let visibility = media_visibility(api.actor());
        self.values = values_for(api.pool(), ids, visibility).await?;
        self.pointing = pointing_at(api.pool(), ids, visibility).await?;
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
        let titled: Vec<ResourceId> = ids
            .iter()
            .chain(&referenced)
            .copied()
            .map(ResourceId)
            .collect();
        self.hidden_titles = with_db!(api.pool(), |db| RelationRepo::new(db)
            .hidden_titles(&titled, visibility)
            .await)
        .map_err(db_error)?
        .into_iter()
        .map(|(id, title)| (id.0, title))
        .collect();
        self.assets = thumbnail_assets_for(api.pool(), ids).await?;
        self.template_properties = template_properties_for(api.pool(), templates).await?;
        Ok(())
    }

    /// Load the derivative rows a page of media documents reports.
    ///
    /// One query over the page's ids. A page of items, item sets or annotations
    /// never loads them, because their documents carry media references and not
    /// media documents.
    pub(crate) async fn load_derivatives<I>(
        &mut self,
        api: &Api<'_>,
        media: I,
    ) -> Result<(), ApiError>
    where
        I: IntoIterator<Item = MediaId>,
    {
        let ids: Vec<MediaId> = media.into_iter().collect();
        if ids.is_empty() {
            return Ok(());
        }
        let rows = with_db!(api.pool(), |db| MediaDerivativeRepo::new(db)
            .for_media_ids(&ids)
            .await)
        .map_err(db_error)?;
        for row in rows {
            self.derivatives
                .entry(row.media_id.0)
                .or_default()
                .push(row);
        }
        Ok(())
    }

    /// The derivative rows of one media, in kind order.
    pub(crate) fn derivatives(&self, media: MediaId) -> &[DerivativeRow] {
        self.derivatives.get(&media.0).map_or(&[], Vec::as_slice)
    }

    /// The title a resource shows to the caller: its stored title, unless that
    /// came from a value the caller may not see (`ValueVisibilityFilter`).
    pub(crate) fn title(&self, resource: &ResourceRow) -> Option<String> {
        match self.hidden_titles.get(&resource.id.0) {
            Some(shown) => shown.clone(),
            None => resource.title.clone(),
        }
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

    /// The sites an item belongs to, in `site_id` order.
    pub(crate) fn sites_of(&self, item: u64) -> &[SiteId] {
        self.sites.get(&item).map_or(&[], Vec::as_slice)
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

    /// Read whether each of `items` is public, for [`Relations::item_is_public`].
    pub(crate) async fn load_owners(
        &mut self,
        api: &Api<'_>,
        items: impl IntoIterator<Item = u64>,
    ) -> Result<(), ApiError> {
        let ids: Vec<u64> = items
            .into_iter()
            .filter(|id| !self.owners.contains_key(id))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        for (id, row) in resources_for(api.pool(), &ids).await? {
            self.owners.insert(id, row.is_public);
        }
        Ok(())
    }

    /// Whether an item loaded by [`Relations::load_owners`] is public. An item
    /// that was not loaded counts as private, so its media's URLs are signed.
    pub(crate) fn item_is_public(&self, item: u64) -> bool {
        self.owners.get(&item).copied().unwrap_or(false)
    }
}

/// Every media row of an item, ordered as `MediaRepo::list` orders them.
pub(crate) async fn media_for(
    pool: &DbPool,
    items: &[u64],
) -> Result<HashMap<u64, Vec<MediaRow>>, ApiError> {
    let items: Vec<ItemId> = items.iter().copied().map(ItemId).collect();
    let rows = with_db!(pool, |db| RelationRepo::new(db)
        .media_of_items(&items)
        .await)
    .map_err(db_error)?;
    let mut media: HashMap<u64, Vec<MediaRow>> = HashMap::new();
    for row in rows {
        media.entry(row.item_id.0).or_default().push(row);
    }
    Ok(media)
}

/// The item-set ids of each item the caller can see, ordered as
/// `ItemRepo::list_item_sets` orders them.
///
/// `Item::$itemSets` loads through `ResourceVisibilityFilter`, so a set the
/// caller cannot see is not one of the item's sets to them.
pub(crate) async fn item_sets_for(
    api: &Api<'_>,
    items: &[u64],
) -> Result<HashMap<u64, Vec<ItemSetId>>, ApiError> {
    let items: Vec<ItemId> = items.iter().copied().map(ItemId).collect();
    let pairs = with_db!(api.pool(), |db| RelationRepo::new(db)
        .item_sets_of_items(&items)
        .await)
    .map_err(db_error)?;
    let hidden = hidden_sets(api, &pairs).await?;
    let mut memberships: HashMap<u64, Vec<ItemSetId>> = HashMap::new();
    for (item, set) in pairs {
        if !hidden.contains(&set.0) {
            memberships.entry(item.0).or_default().push(set);
        }
    }
    Ok(memberships)
}

/// The sets among `pairs` the caller cannot see. A role with `view-all` sees
/// every set, so it reads no rows.
async fn hidden_sets(
    api: &Api<'_>,
    pairs: &[(ItemId, ItemSetId)],
) -> Result<HashSet<u64>, ApiError> {
    if api.actor().is_privileged() || pairs.is_empty() {
        return Ok(HashSet::new());
    }
    let sets: BTreeSet<u64> = pairs.iter().map(|(_, set)| set.0).collect();
    let ids: Vec<ResourceId> = sets.into_iter().map(ResourceId).collect();
    let rows =
        with_db!(api.pool(), |db| RelationRepo::new(db).resources(&ids).await).map_err(db_error)?;
    Ok(rows
        .into_iter()
        .filter(|row| !api.sees(row.is_public, row.owner_id))
        .map(|row| row.id.0)
        .collect())
}

/// The site ids of each item, ordered as `ItemRepo::list_sites` orders them.
pub(crate) async fn sites_for(
    pool: &DbPool,
    items: &[u64],
) -> Result<HashMap<u64, Vec<SiteId>>, ApiError> {
    let items: Vec<ItemId> = items.iter().copied().map(ItemId).collect();
    let pairs = with_db!(pool, |db| RelationRepo::new(db)
        .sites_of_items(&items)
        .await)
    .map_err(db_error)?;
    let mut memberships: HashMap<u64, Vec<SiteId>> = HashMap::new();
    for (item, site) in pairs {
        memberships.entry(item.0).or_default().push(site);
    }
    Ok(memberships)
}

/// The first item of each item set by id, with its stored primary media.
pub(crate) async fn first_item_for(
    pool: &DbPool,
    sets: &[u64],
) -> Result<HashMap<u64, (u64, Option<MediaId>)>, ApiError> {
    let sets: Vec<ItemSetId> = sets.iter().copied().map(ItemSetId).collect();
    let rows =
        with_db!(pool, |db| RelationRepo::new(db).first_items(&sets).await).map_err(db_error)?;
    Ok(rows
        .into_iter()
        .map(|(set, item, primary)| (set.0, (item.0, primary)))
        .collect())
}

/// The asset each page row's `resource.thumbnail_id` names, keyed by asset id.
pub(crate) async fn thumbnail_assets_for(
    pool: &DbPool,
    resources: &[u64],
) -> Result<HashMap<u64, AssetRow>, ApiError> {
    let resources: Vec<ResourceId> = resources.iter().copied().map(ResourceId).collect();
    let rows = with_db!(pool, |db| RelationRepo::new(db)
        .thumbnail_assets(&resources)
        .await)
    .map_err(db_error)?;
    Ok(rows.into_iter().map(|asset| (asset.id.0, asset)).collect())
}

/// Every value on each resource, plus the values on the annotations those
/// values carry, to the depth the builder follows them, as `visibility` may
/// see them (`ValueVisibilityFilter`).
pub(crate) async fn values_for(
    pool: &DbPool,
    resources: &[u64],
    visibility: MediaVisibility,
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
        let level: Vec<ResourceId> = frontier.iter().copied().map(ResourceId).collect();
        let rows = with_db!(pool, |db| RelationRepo::new(db)
            .values_of(&level, visibility)
            .await)
        .map_err(db_error)?;
        let mut next = Vec::new();
        for parsed in rows {
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
    pool: &DbPool,
    resources: &[u64],
    visibility: MediaVisibility,
) -> Result<HashMap<u64, Vec<ValueRow>>, ApiError> {
    let resources: Vec<ResourceId> = resources.iter().copied().map(ResourceId).collect();
    let rows = with_db!(pool, |db| RelationRepo::new(db)
        .values_pointing_at(&resources, visibility)
        .await)
    .map_err(db_error)?;
    let mut pointing: HashMap<u64, Vec<ValueRow>> = HashMap::new();
    for parsed in rows {
        if let Some(target) = parsed.value_resource_id {
            pointing.entry(target.0).or_default().push(parsed);
        }
    }
    Ok(pointing)
}

/// The referenced resource rows, by id.
pub(crate) async fn resources_for(
    pool: &DbPool,
    ids: &[u64],
) -> Result<HashMap<u64, ResourceRow>, ApiError> {
    let ids: Vec<ResourceId> = ids.iter().copied().map(ResourceId).collect();
    let rows =
        with_db!(pool, |db| RelationRepo::new(db).resources(&ids).await).map_err(db_error)?;
    Ok(rows.into_iter().map(|row| (row.id.0, row)).collect())
}

/// The property ids of each resource template, in upstream's order.
pub(crate) async fn template_properties_for(
    pool: &DbPool,
    templates: &[TemplateId],
) -> Result<HashMap<u64, Vec<PropertyId>>, ApiError> {
    let ids: Vec<TemplateId> = templates
        .iter()
        .copied()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let pairs = with_db!(pool, |db| RelationRepo::new(db)
        .template_properties(&ids)
        .await)
    .map_err(db_error)?;
    let mut properties: HashMap<u64, Vec<PropertyId>> = HashMap::new();
    for (template, property) in pairs {
        properties
            .entry(u64::from(template))
            .or_default()
            .push(property);
    }
    Ok(properties)
}
