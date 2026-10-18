//! The search snapshot a page render reads.
//!
//! [`crate::site::load::Loader`] is async and `ResourceProvider` is synchronous,
//! so the blocks' searches run before rendering and land in [`LoadedResources`].
//! A block's `ctx.resources.search(kind, query)` looks its own entry up by
//! equality, so [`load_blocks`] must build exactly the query the block builds;
//! the two mirrors below cite the block each one copies.
//!
//! The snapshot reads the site id, the settings and the request query directly
//! rather than through a `RenderContext`, because a `RenderContext` borrows a
//! `&dyn ResourceProvider` and holding one across an `await` makes the handler
//! future non-`Send`.
//!
//! The item-with-metadata and media blocks read their content from
//! `site_block_attachment`, so the snapshot carries each block's attachments
//! together with the item views and media refs they resolve to. The asset block
//! reads its `data` list and the asset repository is not ported, so
//! `read_asset` stays empty.

use omeka_core::ids::{BlockId, MediaId, SiteId};
use omeka_core::resource::ResourceKind;
use omeka_core::site::{BlockData, BrowsePreviewData, ListOfSitesData, SitePage};
use omeka_db::repos::{ItemRepo, MediaRepo, SiteBlockAttachmentRepo};
use omeka_site::blocks::browse_preview::parse_query;
use omeka_site::settings::SiteSettings;
use omeka_site::view::{
    AssetView, AttachmentRef, BrowseQuery, MediaRef, ResourcePage, ResourceProvider, ResourceView,
    SiteQuery, SiteSearch, SiteSort,
};

use super::load::{Loader, media_options};
use omeka_db::DbError;

/// `BlockLayout/BrowsePreview.php::form`: the default `limit`.
pub(crate) const BROWSE_PREVIEW_DEFAULT_LIMIT: i64 = 12;

/// The searches a page's blocks will run, keyed for the synchronous lookup, and
/// the resources the attachment blocks will read.
#[derive(Debug, Default)]
pub(crate) struct LoadedResources {
    searches: Vec<(ResourceKind, BrowseQuery, ResourcePage)>,
    sites: Vec<(SiteQuery, SiteSearch)>,
    attachments: Vec<(BlockId, Vec<AttachmentRef>)>,
    resources: Vec<(ResourceKind, u64, ResourceView)>,
    media: Vec<(MediaId, MediaRef)>,
}

impl ResourceProvider for LoadedResources {
    fn search(&self, kind: ResourceKind, query: &BrowseQuery) -> ResourcePage {
        self.searches
            .iter()
            .find(|(stored_kind, stored_query, _)| *stored_kind == kind && stored_query == query)
            .map(|(_, _, page)| page.clone())
            .unwrap_or_default()
    }

    fn search_sites(&self, query: &SiteQuery) -> SiteSearch {
        self.sites
            .iter()
            .find(|(stored, _)| stored == query)
            .map(|(_, search)| search.clone())
            .unwrap_or_default()
    }

    fn read(&self, kind: ResourceKind, id: u64) -> Option<ResourceView> {
        self.resources
            .iter()
            .find(|(stored_kind, stored_id, _)| *stored_kind == kind && *stored_id == id)
            .map(|(_, _, view)| view.clone())
    }

    fn read_media(&self, id: MediaId) -> Option<MediaRef> {
        self.media
            .iter()
            .find(|(stored, _)| *stored == id)
            .map(|(_, media)| media.clone())
    }

    fn read_asset(&self, _id: omeka_core::ids::AssetId) -> Option<AssetView> {
        None
    }

    fn attachments(&self, block: BlockId) -> Vec<AttachmentRef> {
        self.attachments
            .iter()
            .find(|(stored, _)| *stored == block)
            .map(|(_, rows)| rows.clone())
            .unwrap_or_default()
    }
}

/// Run every search a page's blocks will ask for and keep the answers.
pub(crate) async fn load_blocks(
    loader: &Loader<'_>,
    page: &SitePage,
    site: SiteId,
    settings: &SiteSettings,
    request_query: &[(String, String)],
) -> Result<LoadedResources, DbError> {
    let mut loaded = LoadedResources::default();
    for block in &page.blocks {
        match &block.data {
            BlockData::BrowsePreview(data) => {
                let (kind, search) = browse_preview_search(data, site, settings);
                let result = loader.search(kind, &search).await?;
                loaded.searches.push((kind, search, result));
            }
            BlockData::ListOfSites(data) => {
                let search = list_of_sites_search(data, site, request_query);
                let result = loader.search_sites(&search).await?;
                loaded.sites.push((search, result));
            }
            BlockData::Media(data) => {
                let options = media_options(
                    data.thumbnail_type.as_deref(),
                    Some(settings.attachment_link_type()),
                );
                load_attachments(loader, block.id, &options, &mut loaded).await?;
            }
            BlockData::ItemWithMetadata => {
                let options = media_options(None, None);
                load_attachments(loader, block.id, &options, &mut loaded).await?;
            }
            _ => {}
        }
    }
    Ok(loaded)
}

/// Load one block's `site_block_attachment` rows, the item views and media refs
/// they resolve to, and the attachment list the block reads.
async fn load_attachments(
    loader: &Loader<'_>,
    block: BlockId,
    options: &omeka_media::render::RenderOptions,
    loaded: &mut LoadedResources,
) -> Result<(), DbError> {
    let rows = SiteBlockAttachmentRepo::new(loader.pool())
        .list_for_block(loader.db_block_id(block))
        .await?;
    let items = ItemRepo::new(loader.pool());
    let media = MediaRepo::new(loader.pool());
    let mut refs = Vec::with_capacity(rows.len());
    for row in &rows {
        let item_view = match row.item_id {
            Some(id) => match items.get(id).await? {
                Some(item_row) => {
                    let view = loader.item_resource_view(&item_row).await?;
                    push_resource(loaded, ResourceKind::Item, id.0, &view);
                    Some(view)
                }
                None => None,
            },
            None => None,
        };
        if let Some(id) = row.media_id
            && let Some(media_row) = media.get(id).await?
        {
            let item_url = item_view
                .as_ref()
                .map(|view| view.url.clone())
                .unwrap_or_default();
            let media_ref = loader.media_ref(&media_row, &item_url, options).await?;
            if !loaded
                .media
                .iter()
                .any(|(stored, _)| *stored == MediaId::new(id.0))
            {
                loaded.media.push((MediaId::new(id.0), media_ref));
            }
        }
        refs.push(AttachmentRef {
            item: row.item_id.map(|id| omeka_core::ids::ItemId::new(id.0)),
            media: row.media_id.map(|id| MediaId::new(id.0)),
            caption: row.caption.clone(),
        });
    }
    loaded.attachments.push((block, refs));
    Ok(())
}

/// Keep one item view in the snapshot unless it is already there.
fn push_resource(loaded: &mut LoadedResources, kind: ResourceKind, id: u64, view: &ResourceView) {
    if !loaded
        .resources
        .iter()
        .any(|(stored_kind, stored_id, _)| *stored_kind == kind && *stored_id == id)
    {
        loaded.resources.push((kind, id, view.clone()));
    }
}

/// The `BrowseQuery` `crates/omeka-site/src/blocks/browse_preview.rs::render_block`
/// builds. Mirror it key for key: the snapshot lookup is by equality.
fn browse_preview_search(
    data: &BrowsePreviewData,
    site: SiteId,
    settings: &SiteSettings,
) -> (ResourceKind, BrowseQuery) {
    let kind = match data.resource_type.as_deref() {
        Some("item_sets") => ResourceKind::ItemSet,
        Some("media") => ResourceKind::Media,
        _ => ResourceKind::Item,
    };
    let mut query = parse_query(data.query.as_deref().unwrap_or_default());
    if settings.browse_attached_items() {
        query.push(("site_attachments_only".to_string(), "true".to_string()));
    }
    query.push(("site_id".to_string(), site.get().to_string()));
    query.push((
        "limit".to_string(),
        data.limit
            .unwrap_or(BROWSE_PREVIEW_DEFAULT_LIMIT)
            .to_string(),
    ));
    if !query.iter().any(|(key, _)| key == "sort_by") {
        query.push(("sort_by_default".to_string(), String::new()));
        query.push(("sort_by".to_string(), "created".to_string()));
    }
    if !query.iter().any(|(key, _)| key == "sort_order") {
        query.push(("sort_order_default".to_string(), String::new()));
        query.push(("sort_order".to_string(), "desc".to_string()));
    }
    let search = BrowseQuery {
        query,
        site,
        limit: data.limit.map(|limit| usize::try_from(limit).unwrap_or(0)),
        sort: Some(("created".to_string(), "desc".to_string())),
    };
    (kind, search)
}

/// The `SiteQuery` `crates/omeka-site/src/blocks/list_of_sites.rs::render_block`
/// builds, mirrored for the same equality lookup.
fn list_of_sites_search(
    data: &ListOfSitesData,
    site: SiteId,
    request_query: &[(String, String)],
) -> SiteQuery {
    let sort = match data.sort.as_deref() {
        Some("oldest") => SiteSort::Oldest,
        Some("newest") => SiteSort::Newest,
        _ => SiteSort::Alpha,
    };
    let limit = data
        .limit
        .and_then(|limit| usize::try_from(limit).ok())
        .filter(|limit| *limit > 0);
    let paginate = limit.is_some() && data.pagination.unwrap_or(false);
    let page = if paginate {
        query_page(request_query).max(1)
    } else {
        1
    };
    SiteQuery {
        sort,
        limit,
        exclude: data.exclude_current.unwrap_or(true).then_some(site),
        page: paginate.then_some(page),
    }
}

fn query_page(query: &[(String, String)]) -> usize {
    query
        .iter()
        .find(|(key, _)| key == "page")
        .and_then(|(_, value)| value.parse().ok())
        .unwrap_or(1)
}
