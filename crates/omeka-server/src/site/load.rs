//! Rows in, `omeka-site` values out.
//!
//! Every function here is a boundary translation: a `omeka-db` row read from
//! the database becomes the owned value `omeka-site` renders (`Site`, `SitePage`,
//! `SiteBlock`, `ResourceView`). The site layer never touches a database; the
//! [`Loader`] carries the pool, the base URL and the term catalog so a caller
//! does not thread four arguments through every call.

use std::cmp::Ordering;
use std::collections::BTreeMap;

use omeka_api::jsonld::BaseUrl;
use omeka_api::read::catalog::Catalog;
use omeka_api::read::params::Params;
use omeka_api::read::query::SearchQuery;
use omeka_core::ids::{BlockId, PageId, ResourceId, SiteId};
use omeka_core::resource::ResourceKind;
use omeka_core::site::{BlockLayout, Site, SiteBlock, SitePage, Slug};
use omeka_db::DbError;
use omeka_db::ids as db;
use omeka_db::repos::{
    FulltextRepo, ItemFilter, ItemRepo, ItemSetFilter, ItemSetRepo, MediaFilter, MediaRepo,
    MediaVisibility, Paging, ResourceRepo, SiteFilter, SiteRepo, TemplateRepo, ValueRepo,
    compare_text_ci,
};
use omeka_db::rows::{
    ItemRow, ItemSetRow, MediaRow, ResourceRow, SiteBlockRow, SitePageRow, SiteRow, ValueRow,
};
use omeka_site::blocks::parse::parse_block_data;
use omeka_site::nav::PageRef;
use omeka_site::search::SearchHit;
use omeka_site::slug::{controller_for, resource_show_url, site_url};
use omeka_site::view::{
    BrowseQuery, BrowseView, ResourcePage, ResourceRef, ResourceView, SiteQuery, SiteSearch,
    SiteSort, SiteView, Thumbnail, ValueRef,
};
use sqlx::SqlitePool;

use super::SiteError;

/// A `site` row as the site layer's [`Site`]. A stored slug that fails
/// `Slug::parse` names no servable site (`SiteAdapter.php:260-266` validates on
/// write), so the route is a 404.
pub(crate) fn site_from_row(row: &SiteRow) -> Result<Site, SiteError> {
    let slug = Slug::parse(&row.slug).map_err(|_| SiteError::NotFound)?;
    let mut site = Site::new(
        SiteId::new(row.id.0),
        slug,
        row.theme.clone(),
        row.title.clone(),
    );
    site.summary = row.summary.clone();
    site.owner = row.owner_id.map(|id| omeka_core::ids::UserId::new(id.0));
    site.is_public = row.is_public;
    site.homepage = row.homepage_id.map(|id| PageId::new(id.0));
    site.assign_new_items = row.assign_new_items;
    Ok(site)
}

/// One public resource row of any kind, kept together so a browse can sort and
/// slice without repeating the per-kind plumbing.
enum LoadedRow {
    Item(ItemRow),
    ItemSet(ItemSetRow),
    Media(Box<MediaRow>),
}

/// Which resources a browse selects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BrowseScope {
    /// The current site (`Site\ItemController::browseAction` sets `site_id`).
    Site,
    /// Items attached to any site (`Site\CrossSiteSearchController` sets
    /// `in_sites`).
    AnySite,
}

/// The rows a query selected, with the fulltext scores when the query asked for
/// a fulltext search.
struct Filtered {
    rows: Vec<LoadedRow>,
    ranks: Option<BTreeMap<u64, f64>>,
    /// `SetBrowseDefaults` left `sort_by` empty, so `Module::searchFulltext`
    /// replaces the sort with relevance.
    sort_by_default: bool,
}

/// The resource name an API search uses for one kind.
fn resource_name(kind: ResourceKind) -> &'static str {
    match kind {
        ResourceKind::Item => "items",
        ResourceKind::ItemSet => "item_sets",
        ResourceKind::Media => "media",
    }
}

/// Order a filtered row list: relevance first when a fulltext query ran under
/// the default sort, otherwise the requested sort.
fn order_rows(
    rows: &mut [LoadedRow],
    sort: Option<&(String, String)>,
    ranks: Option<&BTreeMap<u64, f64>>,
    sort_by_default: bool,
) {
    let (Some(ranks), true) = (ranks, sort_by_default) else {
        rows.sort_by(|a, b| compare_resources(a.resource(), b.resource(), sort));
        return;
    };
    let descending = sort.is_some_and(|(_, order)| order == "desc");
    rows.sort_by(|a, b| {
        let (left, right) = (score(ranks, a), score(ranks, b));
        right.total_cmp(&left).then_with(|| {
            if descending {
                b.resource().id.cmp(&a.resource().id)
            } else {
                a.resource().id.cmp(&b.resource().id)
            }
        })
    });
}

fn score(ranks: &BTreeMap<u64, f64>, row: &LoadedRow) -> f64 {
    ranks.get(&row.resource().id.0).copied().unwrap_or(0.0)
}

impl LoadedRow {
    fn resource(&self) -> &ResourceRow {
        match self {
            Self::Item(row) => &row.resource,
            Self::ItemSet(row) => &row.resource,
            Self::Media(row) => &row.resource,
        }
    }
}

/// The read side of one site render: the pool, the base URL, the property
/// catalog and the site being served.
pub(crate) struct Loader<'a> {
    pool: &'a SqlitePool,
    base: &'a BaseUrl,
    catalog: &'a Catalog,
    site: &'a Site,
}

impl<'a> Loader<'a> {
    pub(crate) fn new(
        pool: &'a SqlitePool,
        base: &'a BaseUrl,
        catalog: &'a Catalog,
        site: &'a Site,
    ) -> Self {
        Self {
            pool,
            base,
            catalog,
            site,
        }
    }

    pub(crate) fn pool(&self) -> &'a SqlitePool {
        self.pool
    }

    fn db_site_id(&self) -> db::SiteId {
        db::SiteId::from(self.site.id.get())
    }

    /// The site's pages as navigation resolution needs them. A page whose stored
    /// slug fails `Slug::parse` is skipped, as it can never resolve to a URL.
    pub(crate) async fn page_refs(&self) -> Result<Vec<PageRef>, DbError> {
        let rows = SiteRepo::new(self.pool)
            .list_pages(self.db_site_id())
            .await?;
        Ok(rows.iter().filter_map(page_ref).collect())
    }

    /// The page of this site whose stored slug is `slug`.
    ///
    /// The comparison is case-insensitive because `site_page.slug` is declared
    /// `COLLATE NOCASE` (`migrations/0001_init.sql:249`), which stands in for
    /// upstream's `utf8mb4_unicode_ci`. Every slug the rule allows is ASCII
    /// (`SiteAdapter.php:260-266`), so an ASCII fold is exact.
    pub(crate) async fn page_row(&self, slug: &str) -> Result<Option<SitePageRow>, DbError> {
        let rows = SiteRepo::new(self.pool)
            .list_pages(self.db_site_id())
            .await?;
        Ok(rows
            .into_iter()
            .find(|row| row.slug.eq_ignore_ascii_case(slug)))
    }

    /// A `site_page` row plus its blocks as the site layer's [`SitePage`].
    pub(crate) async fn page_from_row(&self, row: &SitePageRow) -> Result<SitePage, SiteError> {
        let slug = Slug::parse(&row.slug).map_err(|_| SiteError::NotFound)?;
        let mut page = SitePage::new(PageId::new(row.id.0), self.site.id, slug, row.title.clone());
        page.is_public = row.is_public;
        let blocks = SiteRepo::new(self.pool).list_blocks(row.id).await?;
        page.blocks = blocks.iter().filter_map(block_from_row).collect();
        Ok(page)
    }

    /// A resource view for its loaded row.
    async fn view_of(&self, row: &LoadedRow) -> Result<ResourceView, DbError> {
        match row {
            LoadedRow::Item(row) => self.item_resource_view(row).await,
            LoadedRow::ItemSet(row) => self.item_set_resource_view(row).await,
            LoadedRow::Media(row) => self.media_resource_view(row).await,
        }
    }

    /// A public item as the item view shows it.
    pub(crate) async fn item_resource_view(&self, row: &ItemRow) -> Result<ResourceView, DbError> {
        let mut view = self.base_view(ResourceKind::Item, &row.resource).await?;
        let media_id = match row.primary_media_id {
            Some(id) => Some(id),
            // `ItemRepresentation::primaryMedia` falls back to the first media.
            None => self.first_media_id(row.id()).await?,
        };
        if let Some(media_id) = media_id {
            let alt = view.title.clone();
            view.thumbnail = self.thumbnail_for(media_id, &alt).await?;
        }
        self.add_item_lists(row, &mut view).await?;
        Ok(view)
    }

    /// A public item set as the item-set view shows it.
    pub(crate) async fn item_set_resource_view(
        &self,
        row: &ItemSetRow,
    ) -> Result<ResourceView, DbError> {
        self.base_view(ResourceKind::ItemSet, &row.resource).await
    }

    /// A public media as the media view shows it, with its parent item named.
    pub(crate) async fn media_resource_view(
        &self,
        row: &MediaRow,
    ) -> Result<ResourceView, DbError> {
        let mut view = self.base_view(ResourceKind::Media, &row.resource).await?;
        let alt = view.title.clone();
        view.thumbnail = media_thumbnail(self.base, row, &alt);
        if let Some(item) = ItemRepo::new(self.pool).get(row.item_id).await? {
            view.parent_item = Some(ResourceRef {
                url: resource_show_url(&self.site.slug, "item", item.id().0),
                title: display_title(&item.resource),
                thumbnail: None,
            });
        }
        Ok(view)
    }

    /// Search public resources of one kind, as a block's `$view->api()->search()`
    /// does. `ItemFilter` carries no `site_id` (`repos/resources.rs`), so a
    /// preview shows every public resource, not only this site's (gap).
    pub(crate) async fn search(
        &self,
        kind: ResourceKind,
        query: &BrowseQuery,
    ) -> Result<ResourcePage, DbError> {
        let filtered = self.filtered(kind, &query.query).await?;
        let mut rows = filtered.rows;
        order_rows(
            &mut rows,
            query.sort.as_ref(),
            filtered.ranks.as_ref(),
            filtered.sort_by_default,
        );
        let total = rows.len();
        if let Some(limit) = query.limit {
            rows.truncate(limit);
        }
        let mut resources = Vec::with_capacity(rows.len());
        for row in &rows {
            resources.push(self.view_of(row).await?);
        }
        Ok(ResourcePage { resources, total })
    }

    /// The public rows of one kind that a query selects.
    ///
    /// `Site\ItemController::browseAction` and `BlockLayout\BrowsePreview` both
    /// search through the API with the request query, so the same parameters
    /// the `/api` list endpoints accept apply here: `fulltext_search` against
    /// the index, the property rows, `site_id`, `in_sites`, `has_media` and the
    /// item-set membership. An anonymous visitor is the actor, so the media a
    /// `has_media` test counts are the public ones.
    async fn filtered(
        &self,
        kind: ResourceKind,
        query: &[(String, String)],
    ) -> Result<Filtered, DbError> {
        let params = Params::from_decoded(query.to_vec());
        let search = SearchQuery::parse(&params, self.catalog, MediaVisibility::Actor(None));
        let resource = resource_name(kind);
        let ranks = match &search.fulltext {
            Some(needle) => Some(
                FulltextRepo::new(self.pool)
                    .search(resource, needle)
                    .await?
                    .into_iter()
                    .map(|hit| (hit.id.0, hit.score))
                    .collect::<BTreeMap<u64, f64>>(),
            ),
            None => None,
        };
        let rows: Vec<LoadedRow> = match kind {
            ResourceKind::Item => ItemRepo::new(self.pool)
                .list_filtered(
                    &ItemFilter {
                        is_public: Some(true),
                        ..ItemFilter::default()
                    },
                    &search.filter,
                    &Paging::default(),
                )
                .await?
                .into_iter()
                .map(LoadedRow::Item)
                .collect(),
            ResourceKind::ItemSet => ItemSetRepo::new(self.pool)
                .list_filtered(
                    &ItemSetFilter {
                        is_public: Some(true),
                        ..ItemSetFilter::default()
                    },
                    &search.filter,
                    &Paging::default(),
                )
                .await?
                .into_iter()
                .map(LoadedRow::ItemSet)
                .collect(),
            ResourceKind::Media => MediaRepo::new(self.pool)
                .list_filtered(
                    &MediaFilter {
                        is_public: Some(true),
                        ..MediaFilter::default()
                    },
                    &search.filter,
                    &Paging::default(),
                )
                .await?
                .into_iter()
                .map(|row| LoadedRow::Media(Box::new(row)))
                .collect(),
        };
        let rows: Vec<LoadedRow> = match &ranks {
            Some(ranks) => rows
                .into_iter()
                .filter(|row| ranks.contains_key(&row.resource().id.0))
                .collect(),
            None => rows,
        };
        Ok(Filtered {
            rows,
            ranks,
            sort_by_default: params.sort_by_is_default(),
        })
    }

    /// The public browse view for one kind. Items sort `id` desc
    /// (`config/module.config.php:687-691`); item sets keep the repository's
    /// `id` asc order.
    pub(crate) async fn browse(
        &self,
        kind: ResourceKind,
        per_page: usize,
        page: usize,
        query: &[(String, String)],
        scope: BrowseScope,
    ) -> Result<BrowseView, SiteError> {
        let title = match kind {
            ResourceKind::Item => "Items",
            ResourceKind::ItemSet => "Item sets",
            // `Site\MediaController` has no browse action.
            ResourceKind::Media => return Err(SiteError::NotFound),
        };
        let rows = self.selected_rows(kind, query, scope).await?;
        let total = rows.len();
        let mut resources = Vec::new();
        for row in rows
            .into_iter()
            .skip(page.saturating_sub(1).saturating_mul(per_page))
            .take(per_page)
        {
            resources.push(self.view_of(&row).await?);
        }
        Ok(BrowseView {
            kind,
            title: title.to_string(),
            resources,
            total,
            query: query.to_vec(),
            page,
            per_page,
        })
    }

    /// The ordered rows a browse or a search section selects, with the scope's
    /// site filter on the query.
    async fn selected_rows(
        &self,
        kind: ResourceKind,
        query: &[(String, String)],
        scope: BrowseScope,
    ) -> Result<Vec<LoadedRow>, SiteError> {
        let mut effective = query.to_vec();
        match scope {
            BrowseScope::Site => {
                if !effective.iter().any(|(key, _)| key == "site_id") {
                    effective.push(("site_id".to_string(), self.site.id.get().to_string()));
                }
            }
            BrowseScope::AnySite => {
                if !effective.iter().any(|(key, _)| key == "in_sites") {
                    effective.push(("in_sites".to_string(), "1".to_string()));
                }
            }
        }
        let filtered = self.filtered(kind, &effective).await?;
        let mut rows = filtered.rows;
        if filtered.ranks.is_some() && filtered.sort_by_default {
            // `Module::searchFulltext` replaces the sort with relevance.
            order_rows(&mut rows, None, filtered.ranks.as_ref(), true);
        } else if kind == ResourceKind::Item {
            rows.sort_by_key(|row| std::cmp::Reverse(row.resource().id));
        }
        Ok(rows)
    }

    /// The hits and the total one search page's section shows for a resource
    /// kind, at most `limit` hits.
    pub(crate) async fn search_hits(
        &self,
        kind: ResourceKind,
        query: &[(String, String)],
        limit: Option<usize>,
        scope: BrowseScope,
    ) -> Result<(Vec<SearchHit>, usize), SiteError> {
        let rows = self.selected_rows(kind, query, scope).await?;
        let total = rows.len();
        let take = limit.unwrap_or(usize::MAX);
        let mut hits = Vec::with_capacity(rows.len().min(take));
        for row in rows.iter().take(take) {
            hits.push(self.hit_of(row).await?);
        }
        Ok((hits, total))
    }

    /// One row as a search result.
    async fn hit_of(&self, row: &LoadedRow) -> Result<SearchHit, DbError> {
        let view = self.view_of(row).await?;
        Ok(SearchHit {
            title: view.title,
            url: view.url,
            thumbnail: view.thumbnail,
            site: None,
        })
    }

    /// Search public sites, `$view->api()->search('sites', ...)`.
    pub(crate) async fn search_sites(&self, query: &SiteQuery) -> Result<SiteSearch, DbError> {
        let mut rows = SiteRepo::new(self.pool)
            .list(&SiteFilter::default(), &Paging::default())
            .await?;
        // `SiteFilter` has no `is_public`; upstream's public site search keeps
        // only public sites, so the filter runs here (gap).
        rows.retain(|row| row.is_public && Slug::parse(&row.slug).is_ok());
        if let Some(exclude) = query.exclude {
            rows.retain(|row| row.id.0 != exclude.get());
        }
        sort_site_rows(&mut rows, query.sort);
        let total = rows.len();
        if let Some(limit) = query.limit {
            let offset = query.page.map_or(0, |page| (page.max(1) - 1) * limit);
            rows = rows.into_iter().skip(offset).take(limit).collect();
        }
        Ok(SiteSearch {
            sites: rows.iter().filter_map(site_view).collect(),
            total,
        })
    }

    /// The shared fields of every resource view: id, kind, URL, title, values
    /// and description.
    async fn base_view(
        &self,
        kind: ResourceKind,
        row: &ResourceRow,
    ) -> Result<ResourceView, DbError> {
        let url = resource_show_url(&self.site.slug, controller_for(kind), row.id.0);
        let mut view = ResourceView::new(ResourceId::new(row.id.0), kind, url, display_title(row));
        let (values, description) = self.values_and_description(row).await?;
        view.values = values;
        view.description = description;
        Ok(view)
    }

    /// The values `displayValues` prints. Only public values whose property the
    /// catalog knows are kept. With a template the template's properties come
    /// first in `list_properties` order; without one `dcterms:title` then
    /// `dcterms:description` lead, and the rest keep value-id order.
    async fn values_and_description(
        &self,
        row: &ResourceRow,
    ) -> Result<(Vec<ValueRef>, Option<String>), DbError> {
        let all = ValueRepo::new(self.pool).list_for_resource(row.id).await?;
        let public: Vec<&ValueRow> = all
            .iter()
            .filter(|value| value.is_public && self.catalog.property(value.property_id).is_some())
            .collect();
        let template = self.template_properties(row.resource_template_id).await?;
        let ordered = order_values(public, template.as_deref(), self.catalog);

        let mut values = Vec::with_capacity(ordered.len());
        let mut description = None;
        for value in ordered {
            let info = self.catalog.property(value.property_id);
            let text = self.value_text(value).await?;
            if description.is_none() && info.is_some_and(|info| info.term == "dcterms:description")
            {
                description = Some(text.clone());
            }
            values.push(ValueRef {
                property: info.map(|info| info.label.clone()).unwrap_or_default(),
                value: text,
            });
        }
        Ok((values, description))
    }

    /// The template's properties in `list_properties` order, `None` when the
    /// resource carries no template.
    async fn template_properties(
        &self,
        template_id: Option<db::TemplateId>,
    ) -> Result<Option<Vec<u64>>, DbError> {
        let Some(template_id) = template_id else {
            return Ok(None);
        };
        let properties = TemplateRepo::new(self.pool)
            .list_properties(template_id)
            .await?;
        Ok(Some(
            properties.iter().map(|row| row.property_id.0).collect(),
        ))
    }

    /// The value text for its data type: a literal, a URI, or the display title
    /// of the resource a `resource*` value points at.
    async fn value_text(&self, value: &ValueRow) -> Result<String, DbError> {
        match value.r#type.as_str() {
            "uri" => Ok(value.uri.clone().unwrap_or_default()),
            name if name.starts_with("resource") => match value.value_resource_id {
                Some(id) => self.resource_title(id).await,
                None => Ok(String::new()),
            },
            _ => Ok(value.value.clone().unwrap_or_default()),
        }
    }

    async fn resource_title(&self, id: db::ResourceId) -> Result<String, DbError> {
        Ok(ResourceRepo::new(self.pool)
            .get(id)
            .await?
            .map(|row| display_title(&row))
            .unwrap_or_default())
    }

    /// The item's item sets and media, for the item-with-metadata block.
    async fn add_item_lists(&self, row: &ItemRow, view: &mut ResourceView) -> Result<(), DbError> {
        for id in ItemRepo::new(self.pool).list_item_sets(row.id()).await? {
            if let Some(set) = ItemSetRepo::new(self.pool).get(id).await? {
                view.item_sets.push(ResourceRef {
                    url: resource_show_url(&self.site.slug, "item-set", set.id().0),
                    title: display_title(&set.resource),
                    thumbnail: None,
                });
            }
        }
        let media = MediaRepo::new(self.pool)
            .list(
                &MediaFilter {
                    item_id: Some(row.id()),
                    ..MediaFilter::default()
                },
                &Paging::default(),
            )
            .await?;
        for entry in media {
            view.media.push(ResourceRef {
                url: resource_show_url(&self.site.slug, "media", entry.id().0),
                title: display_title(&entry.resource),
                thumbnail: None,
            });
        }
        Ok(())
    }

    async fn first_media_id(&self, item_id: db::ItemId) -> Result<Option<db::MediaId>, DbError> {
        Ok(MediaRepo::new(self.pool)
            .list(
                &MediaFilter {
                    item_id: Some(item_id),
                    ..MediaFilter::default()
                },
                &Paging::default(),
            )
            .await?
            .first()
            .map(MediaRow::id))
    }

    async fn thumbnail_for(
        &self,
        media_id: db::MediaId,
        alt: &str,
    ) -> Result<Option<Thumbnail>, DbError> {
        Ok(MediaRepo::new(self.pool)
            .get(media_id)
            .await?
            .and_then(|media| media_thumbnail(self.base, &media, alt)))
    }
}

/// A page row as navigation resolution needs it.
fn page_ref(row: &SitePageRow) -> Option<PageRef> {
    let slug = Slug::parse(&row.slug).ok()?;
    Some(PageRef {
        id: PageId::new(row.id.0),
        slug,
        title: row.title.clone(),
        is_public: row.is_public,
    })
}

/// A block row as the site layer's [`SiteBlock`]. A block whose stored `data`
/// does not parse for its layout is skipped, so the rest of the page renders
/// (`Site\BlockLayout\Manager` falls back rather than failing the page).
fn block_from_row(row: &SiteBlockRow) -> Option<SiteBlock> {
    let layout = BlockLayout::from_wire_name(&row.layout);
    let data = parse_block_data(&layout, &row.data).ok()?;
    Some(SiteBlock {
        id: BlockId::new(row.id.0),
        layout,
        position: row.position,
        data,
    })
}

/// `displayTitle`: the stored title, else the upstream placeholder.
pub(crate) fn display_title(row: &ResourceRow) -> String {
    row.title
        .clone()
        .unwrap_or_else(|| "[Untitled]".to_string())
}

/// The medium thumbnail of a media file, `None` without thumbnails or a stored
/// path (`FileRenderer/*` exposes `medium/{storage_id}.jpg`).
fn media_thumbnail(base: &BaseUrl, media: &MediaRow, alt: &str) -> Option<Thumbnail> {
    if !media.has_thumbnails {
        return None;
    }
    let storage_id = media.storage_id.as_deref()?;
    Some(Thumbnail::new(
        base.file_url(&format!("medium/{storage_id}.jpg")),
        alt,
    ))
}

/// Order values as `displayValues` does.
fn order_values<'r>(
    rows: Vec<&'r ValueRow>,
    template_properties: Option<&[u64]>,
    catalog: &Catalog,
) -> Vec<&'r ValueRow> {
    let priorities: Vec<u64> = match template_properties {
        Some(properties) => properties.to_vec(),
        None => ["dcterms:title", "dcterms:description"]
            .iter()
            .filter_map(|term| property_id_for_term(&rows, catalog, term))
            .collect(),
    };
    let mut ordered = Vec::with_capacity(rows.len());
    let mut remaining = rows;
    for property in priorities {
        let mut index = 0;
        while index < remaining.len() {
            if remaining[index].property_id.0 == property {
                ordered.push(remaining.remove(index));
            } else {
                index += 1;
            }
        }
    }
    ordered.extend(remaining);
    ordered
}

/// The property id carrying `term`, found among the resource's own values.
fn property_id_for_term(rows: &[&ValueRow], catalog: &Catalog, term: &str) -> Option<u64> {
    rows.iter()
        .find(|row| {
            catalog
                .property(row.property_id)
                .is_some_and(|info| info.term == term)
        })
        .map(|row| row.property_id.0)
}

/// The order a browse-preview block asks for. `created` orders by the resource's
/// creation time, `id` by its primary key; a missing sort keeps insertion order.
fn compare_resources(
    a: &ResourceRow,
    b: &ResourceRow,
    sort: Option<&(String, String)>,
) -> Ordering {
    let order = match sort.map(|(key, _)| key.as_str()) {
        Some("created") => a.created.cmp(&b.created).then(a.id.cmp(&b.id)),
        _ => a.id.cmp(&b.id),
    };
    if sort.is_some_and(|(_, order)| order == "desc") {
        order.reverse()
    } else {
        order
    }
}

/// A `site` row as the list-of-sites block shows it.
fn site_view(row: &SiteRow) -> Option<SiteView> {
    let slug = Slug::parse(&row.slug).ok()?;
    Some(SiteView {
        id: SiteId::new(row.id.0),
        title: row.title.clone(),
        url: site_url(&slug),
        summary: row.summary.clone(),
        // `site.thumbnail_id` is an asset id and the port has no asset
        // repository yet, so a site thumbnail is never shown (gap).
        thumbnail: None,
    })
}

fn sort_site_rows(rows: &mut [SiteRow], sort: SiteSort) {
    match sort {
        SiteSort::Alpha => {
            rows.sort_by(|a, b| compare_text_ci(&a.title, &b.title).then(a.id.0.cmp(&b.id.0)))
        }
        SiteSort::Oldest => {
            rows.sort_by(|a, b| a.created.cmp(&b.created).then(a.id.0.cmp(&b.id.0)))
        }
        SiteSort::Newest => {
            rows.sort_by(|a, b| b.created.cmp(&a.created).then(b.id.0.cmp(&a.id.0)));
        }
    }
}
