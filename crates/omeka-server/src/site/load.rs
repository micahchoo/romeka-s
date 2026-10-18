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
use omeka_core::ids::{BlockId, ItemId, MediaId, PageId, ResourceId, SiteId};
use omeka_core::resource::{Media, ResourceKind, Timestamp};
use omeka_core::site::{
    BlockLayout, BlockLayoutData, PageLayout, PageLayoutData, Site, SiteBlock, SitePage, Slug,
};
use omeka_db::DbError;
use omeka_db::ids as db;
use omeka_db::repos::{
    AssetRepo, FulltextRepo, ItemFilter, ItemRepo, ItemSetFilter, ItemSetRepo, MediaFilter,
    MediaRepo, MediaVisibility, Paging, ResourceRepo, SiteFilter, SiteRepo, TemplateRepo,
    ValueRepo, compare_text_ci,
};
use omeka_db::rows::{
    AssetRow, ItemRow, ItemSetRow, MediaRow, ResourceRow, SiteBlockRow, SitePageRow, SiteRow,
    ValueRow,
};
use omeka_site::blocks::parse::parse_block_data;
use omeka_site::nav::PageRef;
use omeka_site::search::SearchHit;
use omeka_site::settings::SiteSettings;
use omeka_site::slug::{controller_for, resource_show_url, site_url};
use omeka_site::view::{
    BrowseQuery, BrowseView, MediaRef, ResourcePage, ResourceRef, ResourceView, SiteQuery,
    SiteSearch, SiteSort, SiteView, Thumbnail, ValueRef,
};
use sqlx::SqlitePool;

use omeka_media::data::MediaData;
use omeka_media::file_render::{FileRendererRegistry, LinkType, ThumbnailType};
use omeka_media::render::{
    MediaView, RenderEnv, RenderOptions, RendererRegistry, View as MediaRendererView,
};

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
/// catalog, the site being served, and the caller's visibility facts.
pub(crate) struct Loader<'a> {
    pool: &'a SqlitePool,
    base: &'a BaseUrl,
    catalog: &'a Catalog,
    site: &'a Site,
    access: &'a Access<'a>,
    settings: &'a SiteSettings,
}

/// The visibility facts one site render reads: the asking actor and what the
/// site grants that actor. Upstream's `SitePageVisibilityFilter` and
/// `ResourceVisibilityFilter` read exactly these facts for every page and
/// resource a render touches.
#[derive(Clone, Copy)]
pub(crate) struct Access<'a> {
    pub(crate) actor: &'a omeka_api::auth::Actor,
    pub(crate) site_is_public: bool,
    pub(crate) site_owner: Option<db::UserId>,
    pub(crate) site_permission: bool,
}

impl Access<'_> {
    /// Whether the caller may see a page of the site this access belongs to
    /// (`SitePageVisibilityFilter`).
    pub(crate) fn sees_site_page(&self, page_is_public: bool) -> bool {
        self.actor.sees_site_page(
            page_is_public,
            self.site_is_public,
            self.site_owner,
            self.site_permission,
        )
    }

    /// Whether the caller may see a resource (`ResourceVisibilityFilter`).
    pub(crate) fn sees_resource(&self, is_public: bool, owner: Option<db::UserId>) -> bool {
        self.actor.sees_resource(is_public, owner)
    }
}

impl<'a> Loader<'a> {
    pub(crate) fn new(
        pool: &'a SqlitePool,
        base: &'a BaseUrl,
        catalog: &'a Catalog,
        site: &'a Site,
        access: &'a Access<'a>,
        settings: &'a SiteSettings,
    ) -> Self {
        Self {
            pool,
            base,
            catalog,
            site,
            access,
            settings,
        }
    }

    /// The caller's visibility facts for this site render.
    pub(crate) fn access(&self) -> &Access<'a> {
        self.access
    }

    pub(crate) fn pool(&self) -> &'a SqlitePool {
        self.pool
    }

    /// The request base URL, for building a stored file's URL.
    pub(crate) fn base(&self) -> &'a BaseUrl {
        self.base
    }

    fn db_site_id(&self) -> db::SiteId {
        db::SiteId::from(self.site.id.get())
    }

    pub(crate) fn db_block_id(&self, block: BlockId) -> db::BlockId {
        db::BlockId::from(block.get())
    }

    /// Whether the item is attached to this site (`item_site`), for the
    /// `noindex` meta (`Module.php:807-811`).
    pub(crate) async fn item_in_site(&self, item: db::ItemId) -> Result<bool, DbError> {
        ItemRepo::new(self.pool)
            .in_site(item, self.db_site_id())
            .await
    }

    /// Whether the item set is attached to this site (`site_item_set`), for the
    /// `noindex` meta (`Module.php:819-826`).
    pub(crate) async fn item_set_in_site(&self, item_set: db::ItemSetId) -> Result<bool, DbError> {
        Ok(SiteRepo::new(self.pool)
            .list_item_sets(self.db_site_id())
            .await?
            .iter()
            .any(|row| row.item_set_id == item_set))
    }

    /// The site's visible pages as navigation resolution needs them. A page
    /// whose stored slug fails `Slug::parse` is skipped, as it can never resolve
    /// to a URL, and a page the caller cannot see (`SitePageVisibilityFilter`)
    /// is dropped so it never reaches the navigation, the table of contents or
    /// the pagination.
    pub(crate) async fn page_refs(&self) -> Result<Vec<PageRef>, DbError> {
        let rows = SiteRepo::new(self.pool)
            .list_pages(self.db_site_id())
            .await?;
        Ok(rows
            .iter()
            .filter(|row| self.access.sees_site_page(row.is_public))
            .filter_map(page_ref)
            .collect())
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
        page.layout = PageLayout::from_stored(row.layout.as_deref());
        page.layout_data = PageLayoutData::parse(&row.layout_data.clone().unwrap_or_default());
        let blocks = SiteRepo::new(self.pool).list_blocks(row.id).await?;
        page.blocks = blocks.iter().filter_map(block_from_row).collect();
        Ok(page)
    }

    /// The `application/ld+json` document a show or browse view embeds
    /// (`Module.php`'s `embeddedJsonLd`). The port builds the identifying
    /// members from the row; it does not reproduce the full API representation.
    fn jsonld(&self, kind: ResourceKind, row: &ResourceRow) -> String {
        let (resource, jsonld_type) = match kind {
            ResourceKind::Item => ("items", "o:Item"),
            ResourceKind::ItemSet => ("item_sets", "o:ItemSet"),
            ResourceKind::Media => ("media", "o:Media"),
        };
        serde_json::json!({
            "@context": self.base.api_context(),
            "@id": self.base.api_resource(resource, row.id.0),
            "@type": [jsonld_type],
            "o:id": row.id.0,
            "o:title": display_title(row),
        })
        .to_string()
    }

    /// A resource view for its loaded row.
    async fn view_of(&self, row: &LoadedRow) -> Result<ResourceView, DbError> {
        match row {
            LoadedRow::Item(row) => self.item_resource_view(row).await,
            LoadedRow::ItemSet(row) => self.item_set_resource_view(row).await,
            LoadedRow::Media(row) => self.media_resource_view(row).await,
        }
    }

    /// A browse list entry. `item/browse.phtml:43-47` prints the value of the
    /// site's `browse_heading_property_term` as the heading and of
    /// `browse_body_property_term` as the body, falling back to the display
    /// title and description when the resource has no such value.
    async fn browse_resource(&self, row: &LoadedRow) -> Result<ResourceView, DbError> {
        let mut view = self.view_of(row).await?;
        if let Some(term) = self.settings.browse_heading_property_term()
            && let Some(value) = self.value_for_term(row.resource(), term).await?
        {
            view.title = value;
        }
        if let Some(term) = self.settings.browse_body_property_term() {
            view.description = self.value_for_term(row.resource(), term).await?;
        }
        Ok(view)
    }

    /// The first public value of a property term, as the browse view prints it.
    /// With `filter_locale_values` set, only a value whose language is empty or
    /// the site's locale counts (`item/browse.phtml:7-9`).
    async fn value_for_term(
        &self,
        row: &ResourceRow,
        term: &str,
    ) -> Result<Option<String>, DbError> {
        let locale = self.settings.locale().tag();
        let filter_locale = self.settings.filter_locale_values();
        for value in ValueRepo::new(self.pool).list_for_resource(row.id).await? {
            if !value.is_public {
                continue;
            }
            if filter_locale && !value.lang.as_deref().is_none_or(|lang| lang == locale) {
                continue;
            }
            let Some(info) = self.catalog.property(value.property_id) else {
                continue;
            };
            if info.term == term {
                return Ok(Some(self.value_text(&value).await?));
            }
        }
        Ok(None)
    }

    /// An item as the item view shows it.
    ///
    /// `ItemRepresentation::primaryMedia` reads the primary media through the
    /// visibility filter, so a hidden primary media yields no media rather than
    /// falling through; with no primary media it takes the first media the
    /// caller can see (`ItemRepresentation::media`).
    pub(crate) async fn item_resource_view(&self, row: &ItemRow) -> Result<ResourceView, DbError> {
        let mut view = self.base_view(ResourceKind::Item, &row.resource).await?;
        let media = match row.primary_media_id {
            Some(id) => MediaRepo::new(self.pool).get(id).await?.filter(|media| {
                self.access
                    .sees_resource(media.resource.is_public, media.resource.owner_id)
            }),
            None => self.first_media(row.id()).await?,
        };
        if let Some(media_row) = media {
            let alt = view.title.clone();
            let media_id = media_row.id();
            if view.thumbnail.is_none() {
                view.thumbnail = self.thumbnail_for(media_id, &alt).await?;
            }
            let item_url = view.url.clone();
            view.primary_media = Some(
                self.media_ref(&media_row, &item_url, &media_options(None, None))
                    .await?,
            );
        }
        self.add_item_lists(row, &mut view).await?;
        Ok(view)
    }

    /// A media as the media block shows it: the stored or fallback thumbnail
    /// (`MediaRepresentation::thumbnailUrl`), the original file URL, the display
    /// title and the renderer's markup wrapped as `view/helper/media.phtml`
    /// wraps it (`Media` view helper).
    pub(crate) async fn media_ref(
        &self,
        row: &MediaRow,
        item_url: &str,
        options: &RenderOptions,
    ) -> Result<MediaRef, DbError> {
        let media_url = resource_show_url(&self.site.slug, "media", row.id().0);
        let view = MediaView::new(
            media_core(row),
            MediaData::parse(&row.ingester, row.data.as_ref()).unwrap_or(MediaData::None),
            self.render_env(),
            item_url.to_string(),
            media_url.clone(),
        );
        let thumbnail = view
            .thumbnail_url(options.thumbnail_type.unwrap_or(ThumbnailType::Square))
            .map(|url| Thumbnail::new(url.to_string(), row.alt_text.clone().unwrap_or_default()));
        // The renderer registries are built here rather than held on the loader:
        // a `RendererRegistry` is not `Send`, and a value that lives across an
        // `await` would make the site handler's future non-`Send`.
        let renderers = RendererRegistry::with_defaults();
        let file_renderers = FileRendererRegistry::with_defaults();
        let rendered = renderers.render(
            &row.renderer,
            &MediaRendererView {
                file_renderers: &file_renderers,
            },
            &view,
            options,
        );
        Ok(MediaRef {
            url: media_url,
            original_url: view.original_url.clone(),
            title: view.display_title.clone(),
            thumbnail,
            embed: Some(format!(
                "<div class=\"media-render {}\">{}</div>",
                row.renderer, rendered
            )),
        })
    }

    /// The environment the media renderers read, from the request base URL. The
    /// IIIF and oEmbed settings are not ported to the site settings yet, so the
    /// renderer defaults stand.
    fn render_env(&self) -> RenderEnv {
        RenderEnv {
            server_url: self.base.as_str().to_string(),
            file_base_uri: format!("{}/files", self.base.as_str()),
            asset_base_path: "/application/asset".to_string(),
            ..RenderEnv::default()
        }
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
        if view.thumbnail.is_none() {
            view.thumbnail = media_thumbnail(self.base, row, &alt);
        }
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
                .list_filtered(&ItemFilter::default(), &search.filter, &Paging::default())
                .await?
                .into_iter()
                .map(LoadedRow::Item)
                .collect(),
            ResourceKind::ItemSet => ItemSetRepo::new(self.pool)
                .list_filtered(
                    &ItemSetFilter::default(),
                    &search.filter,
                    &Paging::default(),
                )
                .await?
                .into_iter()
                .map(LoadedRow::ItemSet)
                .collect(),
            ResourceKind::Media => MediaRepo::new(self.pool)
                .list_filtered(&MediaFilter::default(), &search.filter, &Paging::default())
                .await?
                .into_iter()
                .map(|row| LoadedRow::Media(Box::new(row)))
                .collect(),
        };
        // `ResourceVisibilityFilter` reads with the actor: a public row, the
        // caller's own row, or anything a `view-all` role may read. Reading the
        // public rows only would hide an owner's private items on their site.
        let rows: Vec<LoadedRow> = rows
            .into_iter()
            .filter(|row| {
                let resource = row.resource();
                self.access
                    .sees_resource(resource.is_public, resource.owner_id)
            })
            .collect();
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
            resources.push(self.browse_resource(&row).await?);
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
        } else {
            let sort = self.effective_sort(kind, &effective);
            order_rows(&mut rows, Some(&sort), None, false);
        }
        Ok(rows)
    }

    /// The sort a browse starts with: the query's `sort_by`/`sort_order` when it
    /// names them, otherwise the kind's stored default
    /// (`Stdlib/Browse.php:167-170`).
    fn effective_sort(&self, kind: ResourceKind, query: &[(String, String)]) -> (String, String) {
        let find = |key: &str| {
            query
                .iter()
                .find(|(name, _)| name == key)
                .map(|(_, value)| value.clone())
        };
        let default = match kind {
            ResourceKind::Item => {
                let defaults = self.settings.browse_defaults_items();
                (defaults.sort_by, defaults.sort_order)
            }
            // `Site\ItemSetController::browseAction` does not call
            // `setDefaults`, so the item-set browse has no stored default and
            // the adapter orders `omeka_root.id` ASC
            // (`AbstractEntityAdapter.php:244-297`). The brief's "created desc"
            // is not what upstream does on this route.
            ResourceKind::ItemSet => ("id".to_string(), "asc".to_string()),
            ResourceKind::Media => ("id".to_string(), "asc".to_string()),
        };
        (
            find("sort_by").unwrap_or(default.0),
            find("sort_order").unwrap_or(default.1),
        )
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
        let mut sites = Vec::with_capacity(rows.len());
        for row in &rows {
            if let Some(view) = self.site_view(row).await? {
                sites.push(view);
            }
        }
        Ok(SiteSearch { sites, total })
    }

    /// A `site` row as the list-of-sites block shows it. `SiteRepresentation::thumbnail`
    /// is the asset `site.thumbnail_id` names, `None` when there is none.
    async fn site_view(&self, row: &SiteRow) -> Result<Option<SiteView>, DbError> {
        let Ok(slug) = Slug::parse(&row.slug) else {
            return Ok(None);
        };
        let thumbnail = match row.thumbnail_id {
            Some(id) => self.asset_thumbnail(id).await?,
            None => None,
        };
        Ok(Some(SiteView {
            id: SiteId::new(row.id.0),
            title: row.title.clone(),
            url: site_url(&slug),
            summary: row.summary.clone(),
            thumbnail,
        }))
    }

    /// The thumbnail a resource or site shows when its `thumbnail_id` names an
    /// asset: the asset's own file URL and alt text
    /// (`AbstractRepresentation::thumbnailDisplayUrl`).
    async fn asset_thumbnail(&self, id: db::AssetId) -> Result<Option<Thumbnail>, DbError> {
        Ok(AssetRepo::new(self.pool)
            .get(id)
            .await?
            .as_ref()
            .map(|row| thumbnail_for_asset(self.base, row)))
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
        view.jsonld = Some(self.jsonld(kind, row));
        let (values, description) = self.values_and_description(row).await?;
        view.values = values;
        view.description = description;
        // `AbstractResourceEntityRepresentation::thumbnail`: a custom
        // `thumbnail_id` asset wins over the primary media.
        if let Some(id) = row.thumbnail_id {
            view.thumbnail = self.asset_thumbnail(id).await?;
        }
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

    /// The item's item sets and media, for the item-with-metadata block. A
    /// hidden item set or media row is dropped, matching
    /// `ResourceVisibilityFilter` on the related entities.
    async fn add_item_lists(&self, row: &ItemRow, view: &mut ResourceView) -> Result<(), DbError> {
        for id in ItemRepo::new(self.pool).list_item_sets(row.id()).await? {
            if let Some(set) = ItemSetRepo::new(self.pool).get(id).await?
                && self
                    .access
                    .sees_resource(set.resource.is_public, set.resource.owner_id)
            {
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
            if !self
                .access
                .sees_resource(entry.resource.is_public, entry.resource.owner_id)
            {
                continue;
            }
            view.media.push(ResourceRef {
                url: resource_show_url(&self.site.slug, "media", entry.id().0),
                title: display_title(&entry.resource),
                thumbnail: None,
            });
        }
        Ok(())
    }

    /// The item's first media row the caller can see, `ItemRepresentation`'s
    /// primary-media fallback under `ResourceVisibilityFilter`.
    async fn first_media(&self, item_id: db::ItemId) -> Result<Option<MediaRow>, DbError> {
        Ok(MediaRepo::new(self.pool)
            .list(
                &MediaFilter {
                    item_id: Some(item_id),
                    ..MediaFilter::default()
                },
                &Paging::default(),
            )
            .await?
            .into_iter()
            .find(|media| {
                self.access
                    .sees_resource(media.resource.is_public, media.resource.owner_id)
            }))
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
/// (`Site\BlockLayout\Manager` falls back rather than failing the page). Its
/// `layout_data` parses the way `AbstractLayout` reads it: a member of the wrong
/// kind reads as absent.
fn block_from_row(row: &SiteBlockRow) -> Option<SiteBlock> {
    let layout = BlockLayout::from_wire_name(&row.layout);
    let data = parse_block_data(&layout, &row.data).ok()?;
    Some(SiteBlock {
        id: BlockId::new(row.id.0),
        layout,
        position: row.position,
        data,
        layout_data: BlockLayoutData::parse(&row.layout_data.clone().unwrap_or_default()),
    })
}

/// `displayTitle`: the stored title, else the upstream placeholder.
pub(crate) fn display_title(row: &ResourceRow) -> String {
    row.title
        .clone()
        .unwrap_or_else(|| "[Untitled]".to_string())
}

/// A `media` row as the core media value the renderers read. The columns the
/// media subsystem does not use (owner, class, template, thumbnail, language)
/// are left at their defaults.
fn media_core(row: &MediaRow) -> Media {
    let mut media = Media::new(
        MediaId::new(row.id().0),
        ItemId::new(row.item_id.0),
        row.ingester.clone(),
        row.renderer.clone(),
        Timestamp::from_naive(row.resource.created),
    );
    media.core.title = row.resource.title.clone();
    media.core.is_public = row.resource.is_public;
    media.core.modified = row.resource.modified.map(Timestamp::from_naive);
    media.source = row.source.clone();
    media.media_type = row.media_type.clone();
    media.storage_id = row.storage_id.clone();
    media.extension = row.extension.clone();
    media.sha256 = row.sha256.clone();
    media.size = row.size;
    media.has_original = row.has_original;
    media.has_thumbnails = row.has_thumbnails;
    media.position = row.position;
    // `altTextResolved` is null without alt text, but `Thumbnail` always writes
    // an `alt` attribute, so the empty string stands in for the null here.
    media.alt_text = Some(row.alt_text.clone().unwrap_or_default());
    media
}

/// The renderer options a media block passes: `thumbnail_type` (default
/// `square`) and the `attachment_link_type` site setting (default `item`).
/// `None` stands for the default.
pub(crate) fn media_options(thumbnail_type: Option<&str>, link: Option<&str>) -> RenderOptions {
    RenderOptions {
        thumbnail_type: Some(
            thumbnail_type
                .and_then(ThumbnailType::parse)
                .unwrap_or(ThumbnailType::Square),
        ),
        link: Some(link.and_then(LinkType::parse).unwrap_or(LinkType::Item)),
        ..RenderOptions::default()
    }
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

/// The thumbnail an asset shows: `AssetRepresentation::thumbnail` returns the
/// asset itself, so the image is the asset's file URL and the alt is its
/// alt text (`AbstractRepresentation::thumbnailAltText`).
pub(crate) fn thumbnail_for_asset(base: &BaseUrl, row: &AssetRow) -> Thumbnail {
    Thumbnail::new(
        asset_url(base, row),
        row.alt_text.clone().unwrap_or_default(),
    )
}

/// `AssetRepresentation::assetUrl`: the stored file under `asset/<filename>`.
fn asset_url(base: &BaseUrl, row: &AssetRow) -> String {
    let filename = match row.extension.as_deref() {
        Some(extension) if !extension.is_empty() => format!("{}.{extension}", row.storage_id),
        _ => row.storage_id.clone(),
    };
    base.file_url(&format!("asset/{filename}"))
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
