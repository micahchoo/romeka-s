//! The search snapshot a page render reads.
//!
//! [`crate::site::load::Loader`] is async and `ResourceProvider` is synchronous,
//! so the blocks' searches run before rendering and land in [`LoadedResources`].
//! A block's `ctx.resources.search(kind, query)` looks its own entry up by
//! equality, so [`load_blocks`] must build exactly the query the block builds:
//! the browse preview's comes from the block's own `search`, and the
//! list-of-sites mirror below cites the block it copies.
//!
//! The snapshot reads the site id, the settings and the request query directly
//! rather than through a `RenderContext`, because a `RenderContext` borrows a
//! `&dyn ResourceProvider` and holding one across an `await` makes the handler
//! future non-`Send`.
//!
//! The item-with-metadata and media blocks read their content from
//! `site_block_attachment`, so the snapshot carries each block's attachments
//! together with the item views and media refs they resolve to. The asset block
//! reads its `data` list and a block's `layout_data` names a background asset
//! (`background_image_asset`), so the snapshot also carries the assets those
//! name, keyed by id for the synchronous [`ResourceProvider::read_asset`].
//! A block's Romeka settings may name a media (the hero's background, the IIIF
//! image's source); the snapshot carries the image of each one the visitor may
//! see, for [`ResourceProvider::read_image`].

use omeka_core::ids::{AssetId, BlockId, MediaId, PageId, SiteId};
use omeka_core::resource::ResourceKind;
use omeka_core::site::{BlockData, ListOfSitesData, SitePage};
use omeka_db::ids as db;
use omeka_db::repos::{AssetRepo, ItemRepo, MediaRepo, SiteBlockAttachmentRepo};
use omeka_db::rows::AssetRow;
use omeka_db::with_db;
use omeka_site::blocks::romeka::ItemSelection;
use omeka_site::blocks::{browse_preview, list_of_pages};
use omeka_site::settings::SiteSettings;
use omeka_site::theme::tokens::ThemeSettings;
use omeka_site::view::{
    AssetView, AttachmentRef, BrowseQuery, MediaRef, PageCard, ResourcePage, ResourceProvider,
    ResourceView, SiteQuery, SiteSearch, SiteSort, Thumbnail,
};

use omeka_api::write::block_carrier::media_image_path;
use omeka_api::write::site_pages::ResolvedAttachment;
use omeka_media::store::FileKind;

use super::load::{Loader, media_options, thumbnail_for_asset};
use omeka_db::DbError;

/// The searches a page's blocks will run, keyed for the synchronous lookup, and
/// the resources the attachment blocks will read.
#[derive(Debug, Default)]
pub(crate) struct LoadedResources {
    searches: Vec<(ResourceKind, BrowseQuery, ResourcePage)>,
    sites: Vec<(SiteQuery, SiteSearch)>,
    attachments: Vec<(BlockId, Vec<AttachmentRef>)>,
    resources: Vec<(ResourceKind, u64, ResourceView)>,
    media: Vec<(MediaId, MediaRef)>,
    assets: Vec<(AssetId, AssetView)>,
    page_cards: Vec<(PageId, PageCard)>,
    images: Vec<(MediaId, Thumbnail)>,
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

    fn read_asset(&self, id: AssetId) -> Option<AssetView> {
        self.assets
            .iter()
            .find(|(stored, _)| *stored == id)
            .map(|(_, view)| view.clone())
    }

    fn read_image(&self, id: MediaId) -> Option<Thumbnail> {
        self.images
            .iter()
            .find(|(stored, _)| *stored == id)
            .map(|(_, image)| image.clone())
    }

    fn attachments(&self, block: BlockId) -> Vec<AttachmentRef> {
        self.attachments
            .iter()
            .find(|(stored, _)| *stored == block)
            .map(|(_, rows)| rows.clone())
            .unwrap_or_default()
    }

    fn page_card(&self, page: PageId) -> Option<PageCard> {
        self.page_cards
            .iter()
            .find(|(stored, _)| *stored == page)
            .map(|(_, card)| card.clone())
    }
}

/// Run every search a page's blocks will ask for and keep the answers.
pub(crate) async fn load_blocks(
    loader: &Loader<'_>,
    page: &SitePage,
    site: SiteId,
    settings: &SiteSettings,
    request_query: &[(String, String)],
    theme_settings: &ThemeSettings,
) -> Result<LoadedResources, DbError> {
    load_blocks_from(
        loader,
        page,
        site,
        settings,
        request_query,
        theme_settings,
        &AttachmentSource::Stored,
    )
    .await
}

/// The stored-page snapshot, with the attachments a preview body carries
/// instead of the `site_block_attachment` rows of a stored page.
pub(crate) async fn load_preview_blocks(
    loader: &Loader<'_>,
    page: &SitePage,
    site: SiteId,
    settings: &SiteSettings,
    request_query: &[(String, String)],
    attachments: &[(BlockId, Vec<ResolvedAttachment>)],
    theme_settings: &ThemeSettings,
) -> Result<LoadedResources, DbError> {
    load_blocks_from(
        loader,
        page,
        site,
        settings,
        request_query,
        theme_settings,
        &AttachmentSource::Preview(attachments),
    )
    .await
}

/// One attachment to resolve, from either source.
struct Pending {
    item: Option<db::ItemId>,
    media: Option<db::MediaId>,
    caption: String,
}

/// Where a block's attachments come from.
enum AttachmentSource<'a> {
    /// The page's stored `site_block_attachment` rows.
    Stored,
    /// The request body of a preview, keyed by the render block id.
    Preview(&'a [(BlockId, Vec<ResolvedAttachment>)]),
}

impl AttachmentSource<'_> {
    /// The attachments of one block, in input order.
    async fn for_block(
        &self,
        loader: &Loader<'_>,
        block: BlockId,
    ) -> Result<Vec<Pending>, DbError> {
        match self {
            Self::Stored => Ok(
                with_db!(loader.pool(), |db| SiteBlockAttachmentRepo::new(db)
                    .list_for_block(loader.db_block_id(block))
                    .await)?
                .into_iter()
                .map(|row| Pending {
                    item: row.item_id,
                    media: row.media_id,
                    caption: row.caption,
                })
                .collect(),
            ),
            Self::Preview(blocks) => Ok(blocks
                .iter()
                .find(|(id, _)| *id == block)
                .map(|(_, attachments)| {
                    attachments
                        .iter()
                        .map(|attachment| Pending {
                            item: attachment.item,
                            media: attachment.media,
                            caption: attachment.caption.clone(),
                        })
                        .collect()
                })
                .unwrap_or_default()),
        }
    }
}

async fn load_blocks_from(
    loader: &Loader<'_>,
    page: &SitePage,
    site: SiteId,
    settings: &SiteSettings,
    request_query: &[(String, String)],
    theme_settings: &ThemeSettings,
    source: &AttachmentSource<'_>,
) -> Result<LoadedResources, DbError> {
    let mut loaded = LoadedResources::default();
    // The frame's logo and banner are assets the blocks never name, so the
    // snapshot loads them for `RenderContext::resources.read_asset`.
    for id in [theme_settings.logo, theme_settings.banner]
        .into_iter()
        .flatten()
    {
        load_asset(loader, AssetId::new(id), &mut loaded).await?;
    }
    for block in &page.blocks {
        match &block.data {
            BlockData::BrowsePreview(data) => {
                let (kind, search) = browse_preview::search(data, site, settings);
                let result = loader.search(kind, &search).await?;
                // An empty preview asks whether the site holds any item at all.
                if kind == ResourceKind::Item && result.resources.is_empty() {
                    let probe = browse_preview::site_items_probe(site);
                    let held = loader.search(ResourceKind::Item, &probe).await?;
                    loaded.searches.push((ResourceKind::Item, probe, held));
                }
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
                let pending = source.for_block(loader, block.id).await?;
                load_attachments(loader, block.id, &options, &mut loaded, pending).await?;
            }
            BlockData::ItemWithMetadata => {
                let options = media_options(None, None);
                let pending = source.for_block(loader, block.id).await?;
                load_attachments(loader, block.id, &options, &mut loaded, pending).await?;
            }
            BlockData::Asset(attachments) => {
                for attachment in attachments {
                    if let Some(id) = attachment.id {
                        load_asset(loader, id, &mut loaded).await?;
                    }
                }
            }
            BlockData::ListOfPages {
                pagelist: Some(pagelist),
            } if list_of_pages::shows_cards(block) => {
                let ids: Vec<PageId> = list_of_pages::page_ids(pagelist)
                    .into_iter()
                    .filter(|id| !loaded.page_cards.iter().any(|(stored, _)| stored == id))
                    .collect();
                let cards = loader.page_cards(&ids).await?;
                loaded.page_cards.extend(cards);
            }
            _ => {}
        }
        // `AbstractLayout::getBlockInlineStyles` searches the background asset
        // for every block whose `layout_data` names one.
        if let Some(id) = block.layout_data.background_image_asset {
            load_asset(loader, id, &mut loaded).await?;
        }
        // A Romeka block's settings name their own assets (the hero's
        // background), and its rich render reads them through `read_asset`.
        if let Some(carrier) = &block.layout_data.romeka {
            for id in omeka_site::blocks::romeka::asset_refs(carrier) {
                load_asset(loader, id, &mut loaded).await?;
            }
            let layout = block.layout.wire_name().unwrap_or_default();
            for id in omeka_site::blocks::romeka::media_refs(layout, carrier) {
                load_image(loader, id, &mut loaded).await?;
            }
            if let Some(selection) = omeka_site::blocks::romeka::items(carrier) {
                load_selection(loader, site, selection, &mut loaded).await?;
            }
        }
    }
    Ok(loaded)
}

/// The items a Romeka block selects: each picked item the caller may see, or
/// the answer to its search, keyed as the block's render asks. A picked item's
/// main media carries its large thumbnail, the size a gallery or a card shows.
async fn load_selection(
    loader: &Loader<'_>,
    site: SiteId,
    selection: ItemSelection,
    loaded: &mut LoadedResources,
) -> Result<(), DbError> {
    match selection {
        ItemSelection::Picked(ids) => {
            let access = loader.access();
            for id in ids {
                let Some(row) = with_db!(loader.pool(), |dbp| ItemRepo::new(dbp)
                    .get(db::ItemId(id.get()))
                    .await)?
                else {
                    continue;
                };
                if access.sees_resource(row.resource.is_public, row.resource.owner_id) {
                    let view = loader
                        .item_resource_view_with(&row, &media_options(Some("large"), None))
                        .await?;
                    push_resource(loaded, ResourceKind::Item, id.get(), &view);
                }
            }
        }
        ItemSelection::Query(query) => {
            let search = ItemSelection::browse_query(&query, site, ItemSelection::LIMIT);
            let result = loader.search(ResourceKind::Item, &search).await?;
            loaded.searches.push((ResourceKind::Item, search, result));
        }
    }
    Ok(())
}

/// Keep one asset in the snapshot unless it is already there.
async fn load_asset(
    loader: &Loader<'_>,
    id: AssetId,
    loaded: &mut LoadedResources,
) -> Result<(), DbError> {
    if loaded.assets.iter().any(|(stored, _)| *stored == id) {
        return Ok(());
    }
    let row: Option<AssetRow> = with_db!(loader.pool(), |dbp| AssetRepo::new(dbp)
        .get(db::AssetId::from(id.get()))
        .await)?;
    if let Some(row) = row {
        let thumbnail = thumbnail_for_asset(loader.files(), &row);
        loaded.assets.push((
            id,
            AssetView {
                id,
                title: row.name.clone(),
                url: Some(thumbnail.url.clone()),
                thumbnail: Some(thumbnail),
            },
        ));
    }
    Ok(())
}

/// Keep the image of one media a Romeka setting names, when the visitor may
/// see the media and its item, as `ResourceVisibilityFilter` decides for an
/// attachment.
async fn load_image(
    loader: &Loader<'_>,
    id: MediaId,
    loaded: &mut LoadedResources,
) -> Result<(), DbError> {
    if loaded.images.iter().any(|(stored, _)| *stored == id) {
        return Ok(());
    }
    let access = loader.access();
    let Some(media) = with_db!(loader.pool(), |dbp| MediaRepo::new(dbp)
        .get(db::MediaId(id.get()))
        .await)?
    .filter(|media| access.sees_resource(media.resource.is_public, media.resource.owner_id)) else {
        return Ok(());
    };
    let item = with_db!(loader.pool(), |db| ItemRepo::new(db)
        .get(media.item_id)
        .await)?;
    let item_seen = item
        .as_ref()
        .is_some_and(|item| access.sees_resource(item.resource.is_public, item.resource.owner_id));
    if let Some(path) = media_image_path(&media).filter(|_| item_seen) {
        let kind = if path.starts_with("original/") {
            FileKind::Original
        } else {
            FileKind::Thumbnail {
                public: media.resource.is_public
                    && item.is_some_and(|item| item.resource.is_public),
            }
        };
        loaded.images.push((
            id,
            Thumbnail::new(
                loader.files().url(&path, kind),
                media.alt_text.clone().unwrap_or_default(),
            ),
        ));
    }
    Ok(())
}

/// Load one block's attachments: the item views and media refs they resolve to,
/// and the attachment list the block reads.
async fn load_attachments(
    loader: &Loader<'_>,
    block: BlockId,
    options: &omeka_media::render::RenderOptions,
    loaded: &mut LoadedResources,
    pendings: Vec<Pending>,
) -> Result<(), DbError> {
    let access = loader.access();
    let mut refs = Vec::with_capacity(pendings.len());
    for pending in &pendings {
        // `ResourceVisibilityFilter` drops a `SiteBlockAttachment` whose item or
        // media the caller cannot see, so the whole attachment is skipped.
        let item_id = pending.item;
        let media_id = pending.media;
        let item_row = match item_id {
            Some(id) => match with_db!(loader.pool(), |db| ItemRepo::new(db).get(id).await)? {
                Some(item)
                    if access.sees_resource(item.resource.is_public, item.resource.owner_id) =>
                {
                    Some(item)
                }
                _ => continue,
            },
            None => None,
        };
        let media_row = match media_id {
            Some(id) => match with_db!(loader.pool(), |db| MediaRepo::new(db).get(id).await)? {
                Some(media)
                    if access.sees_resource(media.resource.is_public, media.resource.owner_id) =>
                {
                    Some(media)
                }
                _ => continue,
            },
            None => None,
        };
        let item_view = match &item_row {
            Some(item_row) => {
                let view = loader.item_resource_view_with(item_row, options).await?;
                push_resource(loaded, ResourceKind::Item, item_row.id().0, &view);
                Some(view)
            }
            None => None,
        };
        if let Some(media_row) = &media_row {
            let item_url = item_view
                .as_ref()
                .map(|view| view.url.clone())
                .unwrap_or_default();
            let item_is_public = loader.item_is_public(media_row.item_id).await?;
            let media_ref = loader
                .media_ref(media_row, &item_url, item_is_public, options)
                .await?;
            let id = media_row.id();
            if !loaded
                .media
                .iter()
                .any(|(stored, _)| *stored == MediaId::new(id.0))
            {
                loaded.media.push((MediaId::new(id.0), media_ref));
            }
        }
        refs.push(AttachmentRef {
            item: item_id.map(|id| omeka_core::ids::ItemId::new(id.0)),
            media: media_id.map(|id| MediaId::new(id.0)),
            caption: pending.caption.clone(),
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
