//! Rows in, `omeka-site` values out.
//!
//! Every function here is a boundary translation: a `omeka-db` row read from
//! the database becomes the owned value `omeka-site` renders (`Site`, `SitePage`,
//! `SiteBlock`, `ResourceView`). The site layer never touches a database; the
//! [`Loader`] carries the pool, the base URL and the term catalog so a caller
//! does not thread four arguments through every call.

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::io::Write as _;
use std::sync::LazyLock;

use flate2::Compression;
use flate2::write::GzEncoder;

use omeka_api::annotation::{
    Annotation, AnnotationError, AnnotationId, Annotations, Body, ExtentSource, Iris, Page,
    Selector, Summary,
};
use omeka_api::jsonld::BaseUrl;
use omeka_api::read::catalog::Catalog;
use omeka_api::read::params::Params;
use omeka_api::read::query::SearchQuery;
use omeka_core::ids::{BlockId, ItemId, MediaId, PageId, ResourceId, SiteId};
use omeka_core::resource::{Media, ResourceKind, Timestamp};
use omeka_core::site::{
    BlockLayout, BlockLayoutData, PageLayout, PageLayoutData, Site, SiteBlock, SitePage, Slug,
};
use omeka_core::term_list;
use omeka_core::value_shape::ValueShape;
use omeka_db::DbError;
use omeka_db::DbPool;
use omeka_db::ids as db;
use omeka_db::repos::{
    AssetRepo, ClassRepo, FulltextRepo, ItemFilter, ItemRepo, ItemSetFilter, ItemSetRepo,
    MediaAnnotationRepo, MediaExtentRepo, MediaFilter, MediaRepo, MediaVisibility, Paging,
    ResourceRepo, SiteBlockAttachmentRepo, SiteFilter, SiteRepo, TemplateRepo, ValueRepo,
    compare_opt_text_ci, compare_text_ci,
};
use omeka_db::rows::{
    AssetRow, ItemRow, ItemSetRow, MediaRow, ResourceRow, SiteBlockRow, SitePageRow, SiteRow,
    TemplatePropertyRow, ValueRow,
};
use omeka_db::with_db;
use omeka_site::annotation::{self, NoteLine, NotesView, Place, Surface};
use omeka_site::blocks::page_card::{self, CardImage};
use omeka_site::blocks::parse::parse_block_data;
use omeka_site::i18n::{Messages, sprintf1};
use omeka_site::nav::PageRef;
use omeka_site::resource_page::link_pretty;
use omeka_site::search::SearchHit;
use omeka_site::settings::SiteSettings;
use omeka_site::share::{ShareLink, ShareTarget, ShareView};
use omeka_site::slug::{controller_for, page_url, resource_show_url, site_url};
use omeka_site::transcript::{Cue, Language, Transcript};
use omeka_site::view::{
    BrowseQuery, BrowseView, DownloadLink, LinkedResource, MediaIcon, MediaKindCue, MediaPosition,
    MediaRef, PageCard, ResourcePage, ResourceRef, ResourceView, SiteQuery, SiteSearch, SiteSort,
    SiteView, Thumbnail, ValueRef,
};

use omeka_media::data::MediaData;
use omeka_media::file_render::{FileRendererRegistry, LinkType, ThumbnailType};
use omeka_media::kind::{KindInput, MediaKind};
use omeka_media::render::{
    MediaStart, MediaView, ModelReader, RenderEnv, RenderOptions, RendererRegistry,
    View as MediaRendererView, human_size,
};
use omeka_media::store::{FileKind, FileUrls, download_name};

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

/// What a resource list is ordered by: a column of the resource, or a
/// property's value text, as `AbstractResourceEntityAdapter::sortQuery` reads
/// `sort_by`. Any other name orders by id.
enum SortKey {
    Id,
    Created,
    Modified,
    Title,
    /// Each resource's sort text on the property, by resource id.
    Property(BTreeMap<u64, String>),
}

/// A requested order: its key and whether it descends.
struct Order {
    key: SortKey,
    descending: bool,
}

/// Order a filtered row list: relevance first when a fulltext query ran under
/// the default sort, otherwise the requested order.
fn order_rows(
    rows: &mut [LoadedRow],
    order: Option<&Order>,
    ranks: Option<&BTreeMap<u64, f64>>,
    sort_by_default: bool,
) {
    let (Some(ranks), true) = (ranks, sort_by_default) else {
        rows.sort_by(|a, b| compare_resources(a.resource(), b.resource(), order));
        return;
    };
    let descending = order.is_some_and(|order| order.descending);
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

/// Where the bundled theme's files are served (`crate::assets`).
const ASSET_BASE: &str = "/assets";

/// The vendored 3D viewer a model file's page loads when the reader presses
/// "Turn the model".
const MODEL_VIEWER_LIBRARY: &str = "vendor/model-viewer/model-viewer-umd.min.js";

/// The viewer's download: the vendored bytes gzipped the way the asset route
/// sends them to a browser. The bytes are compiled in, so one process pays the
/// compression once.
static MODEL_VIEWER_BYTES: LazyLock<u64> = LazyLock::new(|| {
    omeka_site::theme::theme_asset(omeka_site::theme::DEFAULT_INFO.name, MODEL_VIEWER_LIBRARY)
        .map_or(0, |asset| gzipped_len(asset.content))
});

/// The length of `bytes` after a gzip round at the level the HTTP layer uses.
fn gzipped_len(bytes: &[u8]) -> u64 {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    if encoder.write_all(bytes).is_err() {
        return bytes.len() as u64;
    }
    encoder
        .finish()
        .map_or(bytes.len() as u64, |out| out.len() as u64)
}

/// The read side of one site render: the pool, the base URL, the property
/// catalog, the site being served, and the caller's visibility facts.
pub(crate) struct Loader<'a> {
    pool: &'a DbPool,
    base: &'a BaseUrl,
    files: &'a FileUrls,
    catalog: &'a Catalog,
    site: &'a Site,
    access: &'a Access<'a>,
    settings: &'a SiteSettings,
    /// The size and run time of a stored media, for a note's overlay and rail.
    extents: &'a dyn ExtentSource,
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
    /// The catalogue of the site's locale, for the placeholders a loaded view
    /// carries.
    pub(crate) fn messages(&self) -> &'static Messages {
        self.settings.locale().translator().messages()
    }

    /// `$translate($property->label())`: a built-in property's label in the
    /// site's locale, any other label as stored.
    fn property_label(&self, info: &omeka_api::read::catalog::PropertyInfo) -> String {
        self.settings
            .locale()
            .translator()
            .term_label(&info.term, &info.label)
            .to_string()
    }

    // The loader is the one struct a site render hands everything it reads
    // through; one more borrow is the honest shape, not a parameter to bundle.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        pool: &'a DbPool,
        base: &'a BaseUrl,
        files: &'a FileUrls,
        catalog: &'a Catalog,
        site: &'a Site,
        access: &'a Access<'a>,
        settings: &'a SiteSettings,
        extents: &'a dyn ExtentSource,
    ) -> Self {
        Self {
            pool,
            base,
            files,
            catalog,
            site,
            access,
            settings,
            extents,
        }
    }

    /// The caller's visibility facts for this site render.
    pub(crate) fn access(&self) -> &Access<'a> {
        self.access
    }

    pub(crate) fn pool(&self) -> &'a DbPool {
        self.pool
    }

    /// The request's term catalog.
    pub(crate) fn catalog(&self) -> &'a Catalog {
        self.catalog
    }

    /// The stored files' URLs.
    pub(crate) fn files(&self) -> &'a FileUrls {
        self.files
    }

    /// The WAP container a media's notes are read from.
    pub(crate) fn notes_container(&self, id: db::MediaId) -> String {
        Iris::new(self.base.as_str()).container(id)
    }

    /// Whether an item is public, for the file URLs of a media shown without
    /// its item. A missing item counts as private.
    pub(crate) async fn item_is_public(&self, item: db::ItemId) -> Result<bool, DbError> {
        Ok(with_db!(self.pool, |db| ItemRepo::new(db).get(item).await)?
            .is_some_and(|row| row.resource.is_public))
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
        with_db!(self.pool, |db| ItemRepo::new(db)
            .in_site(item, self.db_site_id())
            .await)
    }

    /// Whether the item set is attached to this site (`site_item_set`), for the
    /// `noindex` meta (`Module.php:819-826`).
    pub(crate) async fn item_set_in_site(&self, item_set: db::ItemSetId) -> Result<bool, DbError> {
        Ok(with_db!(self.pool, |db| SiteRepo::new(db)
            .list_item_sets(self.db_site_id())
            .await)?
        .iter()
        .any(|row| row.item_set_id == item_set))
    }

    /// The site's visible pages as navigation resolution needs them. A page
    /// whose stored slug fails `Slug::parse` is skipped, as it can never resolve
    /// to a URL, and a page the caller cannot see (`SitePageVisibilityFilter`)
    /// is dropped so it never reaches the navigation, the table of contents or
    /// the pagination.
    pub(crate) async fn page_refs(&self) -> Result<Vec<PageRef>, DbError> {
        let rows = with_db!(self.pool, |db| SiteRepo::new(db)
            .list_pages(self.db_site_id())
            .await)?;
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
        let rows = with_db!(self.pool, |db| SiteRepo::new(db)
            .list_pages(self.db_site_id())
            .await)?;
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
        let blocks = with_db!(self.pool, |db| SiteRepo::new(db).list_blocks(row.id).await)?;
        page.blocks = blocks.iter().filter_map(block_from_row).collect();
        Ok(page)
    }

    /// The cards of the listed pages a list-of-pages block shows: each page
    /// of this site the caller can see, with the image and summary its own
    /// blocks give ([`page_card::source`]).
    pub(crate) async fn page_cards(
        &self,
        ids: &[PageId],
    ) -> Result<Vec<(PageId, PageCard)>, DbError> {
        let rows = with_db!(self.pool, |db| SiteRepo::new(db)
            .list_pages(self.db_site_id())
            .await)?;
        let mut cards = Vec::new();
        for id in ids {
            let Some(row) = rows
                .iter()
                .find(|row| row.id.0 == id.get() && self.access.sees_site_page(row.is_public))
            else {
                continue;
            };
            let Ok(slug) = Slug::parse(&row.slug) else {
                continue;
            };
            let mut page = SitePage::new(*id, self.site.id, slug, row.title.clone());
            page.blocks = with_db!(self.pool, |db| SiteRepo::new(db).list_blocks(row.id).await)?
                .iter()
                .filter_map(block_from_row)
                .collect();
            let source = page_card::source(&page);
            let thumbnail = match source.image {
                Some(CardImage::Asset(asset)) => with_db!(self.pool, |dbp| AssetRepo::new(dbp)
                    .get(db::AssetId::from(asset.get()))
                    .await)?
                .map(|row| thumbnail_for_asset(self.files, &row)),
                Some(CardImage::MediaBlock(block)) => {
                    self.first_block_media_thumbnail(block).await?
                }
                None => None,
            };
            cards.push((
                *id,
                PageCard {
                    thumbnail,
                    summary: source.summary,
                },
            ));
        }
        Ok(cards)
    }

    /// The medium thumbnail of the first media attached to a block that the
    /// caller can see.
    async fn first_block_media_thumbnail(
        &self,
        block: BlockId,
    ) -> Result<Option<Thumbnail>, DbError> {
        for attachment in with_db!(self.pool, |db| SiteBlockAttachmentRepo::new(db)
            .list_for_block(self.db_block_id(block))
            .await)?
        {
            let Some(id) = attachment.media_id else {
                continue;
            };
            if let Some(row) = with_db!(self.pool, |db| MediaRepo::new(db).get(id).await)?
                && self
                    .access
                    .sees_resource(row.resource.is_public, row.resource.owner_id)
            {
                let item_is_public = self.item_is_public(row.item_id).await?;
                return Ok(media_thumbnail(self.files, &row, item_is_public, ""));
            }
        }
        Ok(None)
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
            // `AbstractResourceEntityRepresentation::title()`: the stored
            // title, null when there is none. The placeholder is display text.
            "o:title": row.title,
        })
        .to_string()
    }

    /// A resource view for its loaded row.
    async fn view_of(&self, row: &LoadedRow) -> Result<ResourceView, DbError> {
        match row {
            LoadedRow::Item(row) => self.item_resource_view(row).await,
            LoadedRow::ItemSet(row) => self.item_set_resource_view(row).await,
            LoadedRow::Media(row) => self.media_resource_view(row, None, None).await,
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
        for value in with_db!(self.pool, |db| ValueRepo::new(db)
            .list_for_resource(row.id)
            .await)?
        {
            if !value.is_public {
                continue;
            }
            if filter_locale && !in_locale(value.lang.as_deref(), locale) {
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
        self.item_resource_view_with(row, &media_options(None, None))
            .await
    }

    /// An item as the item view shows it, with the primary media rendered under
    /// the given options. The media block's attachments use this so an
    /// item-only attachment honours the block's `thumbnail_type` and link type.
    pub(crate) async fn item_resource_view_with(
        &self,
        row: &ItemRow,
        options: &RenderOptions,
    ) -> Result<ResourceView, DbError> {
        let mut view = self.base_view(ResourceKind::Item, &row.resource).await?;
        let media = match row.primary_media_id {
            Some(id) => {
                with_db!(self.pool, |db| MediaRepo::new(db).get(id).await)?.filter(|media| {
                    self.access
                        .sees_resource(media.resource.is_public, media.resource.owner_id)
                })
            }
            None => self.first_media(row.id()).await?,
        };
        if let Some(media_row) = media {
            let media_id = media_row.id();
            let item_is_public = row.resource.is_public;
            let kind = media_kind(&media_row);
            view.media_kind = Some(media_kind_cue(kind, self.messages()));
            if is_recording(kind) {
                view.duration_ms = self.media_duration(&media_row).await?;
            }
            if view.thumbnail.is_none() {
                // The card shows the item's name as text beside this image, so
                // the image is decorative and carries no alt.
                view.thumbnail = self.thumbnail_for(media_id, item_is_public, "").await?;
            }
            let item_url = view.url.clone();
            view.primary_media = Some(
                self.media_ref(&media_row, &item_url, item_is_public, options)
                    .await?,
            );
        }
        self.add_item_lists(row, &mut view).await?;
        let share = self.share_view(
            ShareTarget::Item,
            &view.title,
            &view.url,
            view.thumbnail.as_ref(),
            view.iiif_manifest.clone(),
        );
        view.share = Some(share);
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
        item_is_public: bool,
        options: &RenderOptions,
    ) -> Result<MediaRef, DbError> {
        let media_url = resource_show_url(&self.site.slug, "media", row.id().0);
        let alt_text = omeka_api::settings::media_alt_text(self.pool, self.catalog, row).await?;
        // The renderers read `display_title` for the link's `title` and for an
        // iframe's name, so the media value carries the display name rather
        // than the raw title column. `MediaView` then has no fallback of its
        // own to take.
        let title = media_display_title(row, self.messages());
        let mut core = media_core(row, alt_text.clone());
        core.core.title = Some(title.clone());
        // The kind and the caption languages are read before the media view is
        // built: a `MediaView` is not `Send`, so nothing may await while it is
        // alive.
        let kind = MediaKind::of(&KindInput {
            renderer: &row.renderer,
            ingester: &row.ingester,
            media_type: row.media_type.as_deref(),
            has_thumbnails: row.has_thumbnails,
        });
        let tracks = self.caption_tracks(row, kind).await?;
        let view = MediaView::new(
            core,
            MediaData::parse(&row.ingester, row.data.as_ref()).unwrap_or(MediaData::None),
            self.render_env(),
            item_url.to_string(),
            media_url.clone(),
        )
        .in_item(item_is_public);
        let thumbnail = view
            .thumbnail_url(options.thumbnail_type.unwrap_or(ThumbnailType::Square))
            .map(|url| Thumbnail::new(url.to_string(), alt_text.unwrap_or_default()));
        // The renderer registries are built here rather than held on the loader:
        // a `RendererRegistry` is not `Send`, and a value that lives across an
        // `await` would make the site handler's future non-`Send`.
        let renderers = RendererRegistry::with_defaults();
        let file_renderers = FileRendererRegistry::with_defaults();
        // A 3D file gets the viewer only on the page that shows it, and the
        // reader starts the download: the renderer emits a facade, and the
        // theme's script loads the library when the button is pressed.
        let is_model = kind == MediaKind::Model3d;
        let mut options = options.clone();
        if is_model {
            options.model_reader = self.model_reader();
        }
        let rendered = renderers.render(
            &row.renderer,
            &MediaRendererView {
                file_renderers: &file_renderers,
            },
            &view,
            &options,
        );
        // A recording's caption languages are `<track>` elements inside the
        // renderer's own player, so a media with none keeps its exact markup.
        let rendered = omeka_site::annotation::with_tracks(&rendered, &tracks);
        // A transcript line's `?t=` link names the player, so the reloaded page
        // lands on the file it cues. The notes figure adds the same id when the
        // page draws notes, and leaves an element that already has one alone.
        let rendered = if options.reader && is_recording(kind) {
            omeka_site::annotation::player_embed(&rendered)
        } else {
            rendered
        };
        // `Renderer/IIIF.php` appends OpenSeadragon to `headScript`; the theme
        // owns the head here, so the embed loads the library it calls. The 3D
        // facade is a theme script for the same reason: no inline code and no
        // library before the reader asks for the model.
        let library = if row.renderer == "iiif" {
            omeka_site::theme::library_script(
                ASSET_BASE,
                "vendor/openseadragon/openseadragon.min.js",
            )
        } else if is_model {
            omeka_site::theme::library_script(ASSET_BASE, "js/model.js")
        } else {
            String::new()
        };
        Ok(MediaRef {
            url: media_url,
            original_url: view.original_url.clone(),
            title: title.clone(),
            media_type: row.media_type.clone(),
            thumbnail,
            embed: Some(format!(
                "<div class=\"media-render {}\">{library}{rendered}</div>",
                row.renderer
            )),
        })
    }

    /// The 3D viewer a model file offers on the reader's press: the vendored
    /// library's versioned URL, the size it downloads, and the site's words for
    /// the facade. `None` when the theme ships no such library, so the renderer
    /// offers the file as a link rather than a button that cannot work.
    fn model_reader(&self) -> Option<ModelReader> {
        omeka_site::theme::theme_asset(omeka_site::theme::DEFAULT_INFO.name, MODEL_VIEWER_LIBRARY)?;
        let messages = self.messages();
        Some(ModelReader {
            library_url: omeka_site::theme::library_url(ASSET_BASE, MODEL_VIEWER_LIBRARY),
            turn: sprintf1(messages.model_turn, &human_size(*MODEL_VIEWER_BYTES)),
            loading: messages.model_loading.to_string(),
            failed: messages.model_failed.to_string(),
            no_javascript: messages.model_no_javascript.to_string(),
        })
    }

    /// The environment the media renderers read: the request base URL, the
    /// bundled theme's assets, where OpenSeadragon's images are served, and the
    /// site's two IIIF viewer settings (`Renderer/IiifPresentation.php`).
    fn render_env(&self) -> RenderEnv {
        let base = self.base.as_str();
        RenderEnv {
            server_url: base.to_string(),
            files: self.files.clone(),
            asset_base_path: ASSET_BASE.to_string(),
            viewer_url: format!("{base}/iiif-viewer"),
            iiif_viewer_sidebar: self.settings.iiif_viewer_sidebar(),
            iiif_viewer_theme: self.settings.iiif_viewer_theme().to_string(),
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
    /// `start` is the moment a `?t=` link asks the player to open at; a probed
    /// run time holds it inside the file. `transcript` is the language a
    /// `?transcript=` link asks for, when the recording offers it.
    pub(crate) async fn media_resource_view(
        &self,
        row: &MediaRow,
        start: Option<MediaStart>,
        transcript: Option<&str>,
    ) -> Result<ResourceView, DbError> {
        let mut view = self.base_view(ResourceKind::Media, &row.resource).await?;
        let title = media_display_title(row, self.messages());
        view.title = title.clone();
        let kind = media_kind(row);
        view.media_kind = Some(media_kind_cue(kind, self.messages()));
        if is_recording(kind) {
            view.duration_ms = self.media_duration(row).await?;
        }
        let start = start.map(|start| start.clamped_to(view.duration_ms));
        let alt = title;
        let item = with_db!(self.pool, |db| ItemRepo::new(db).get(row.item_id).await)?;
        let item_is_public = item.as_ref().is_some_and(|item| item.resource.is_public);
        if view.thumbnail.is_none() {
            view.thumbnail = media_thumbnail(self.files, row, item_is_public, &alt);
        }
        let item_url = match item {
            Some(item) => {
                let url = resource_show_url(&self.site.slug, "item", item.id().0);
                view.parent_item = Some(ResourceRef {
                    url: url.clone(),
                    title: display_title(&item.resource, self.messages()),
                    thumbnail: None,
                    media_kind: None,
                    notes: 0,
                });
                url
            }
            None => String::new(),
        };
        view.media_position = self.media_position(row).await?;
        // The transcript is read before the renderer builds its view, for the
        // same reason the tracks are: nothing may await while a `MediaView` is
        // alive.
        view.transcript = self.media_transcript(row, kind, transcript).await?;
        // `mediaRender.phtml`: `$resource->render()` on the media itself, with
        // the renderer's own defaults (the large thumbnail, linked to the
        // original). This is the file's own page, so a document may draw its
        // reader here and nowhere else.
        view.media_embeds.push(
            self.media_ref(
                row,
                &item_url,
                item_is_public,
                &RenderOptions {
                    reader: true,
                    start,
                    ..RenderOptions::default()
                },
            )
            .await?,
        );
        // The media manifest exists only for a kind that paints a canvas. The
        // page head links it and the "Use elsewhere" block opens it, so a
        // model or a document links neither.
        view.iiif_manifest = kind
            .canvas_body()
            .map(|_| Iris::new(self.base.as_str()).media_manifest(row.id()));
        let mut share = self.share_view(
            ShareTarget::Media,
            &view.title,
            &view.url,
            view.thumbnail.as_ref(),
            view.iiif_manifest.clone(),
        );
        share.captions = self.share_captions(row, kind).await?;
        view.share = Some(share);
        view.download = download_link(row, &view.url);
        Ok(view)
    }

    /// The one-line share block an item or a media page carries. The viewer
    /// URL is built here because the loader holds the site's viewer settings.
    fn share_view(
        &self,
        target: ShareTarget,
        title: &str,
        url: &str,
        image: Option<&Thumbnail>,
        manifest: Option<String>,
    ) -> ShareView {
        let viewer = manifest.as_ref().map(|manifest| {
            omeka_site::iiif_viewer::viewer_src(
                &format!("{}/iiif-viewer", self.base.as_str()),
                manifest,
                self.settings,
            )
        });
        ShareView::page(
            target,
            title,
            format!("{}{url}", self.base.as_str()),
            image.map(|image| image.url.clone()),
            manifest,
            viewer,
        )
    }

    /// A recording's captions files, one link per language with cues.
    async fn share_captions(
        &self,
        row: &MediaRow,
        kind: MediaKind,
    ) -> Result<Vec<ShareLink>, DbError> {
        if !is_recording(kind) {
            return Ok(Vec::new());
        }
        let tracks = self.caption_tracks(row, kind).await?;
        Ok(tracks
            .iter()
            .map(|track| {
                omeka_site::share::captions_link(self.messages(), &track.label, track.src.clone())
            })
            .collect())
    }

    /// The caption languages of a recording, as the `<track>` elements the
    /// embed plays. A kind with no timeline asks no question and runs no query.
    async fn caption_tracks(
        &self,
        row: &MediaRow,
        kind: MediaKind,
    ) -> Result<Vec<omeka_site::annotation::TrackLink>, DbError> {
        if !matches!(kind, MediaKind::Audio | MediaKind::Video) {
            return Ok(Vec::new());
        }
        let counts = with_db!(self.pool, |db| MediaAnnotationRepo::new(db)
            .track_langs(&[row.id()])
            .await)?;
        let tracks: Vec<(Option<String>, u32)> = counts
            .into_iter()
            .map(|(_, lang, cues)| (lang, cues))
            .collect();
        let iris = Iris::new(self.base.as_str());
        Ok(omeka_site::annotation::tracks(
            &tracks,
            self.settings.locale().tag(),
            &iris.captions(row.id(), None),
            self.messages().kind_captions,
        ))
    }

    /// A recording's transcript: every language it offers and the cues of the
    /// one the page shows. `requested` is the language a `?transcript=` link
    /// asks for; a tag the recording does not offer falls back to the default
    /// track, as the `<track>` elements do.
    async fn media_transcript(
        &self,
        row: &MediaRow,
        kind: MediaKind,
        requested: Option<&str>,
    ) -> Result<Option<Transcript>, DbError> {
        if !matches!(kind, MediaKind::Audio | MediaKind::Video) {
            return Ok(None);
        }
        let counts = with_db!(self.pool, |db| MediaAnnotationRepo::new(db)
            .track_langs(&[row.id()])
            .await)?;
        let counts: Vec<(Option<String>, u32)> = counts
            .into_iter()
            .map(|(_, lang, cues)| (lang, cues))
            .collect();
        let tracks = omeka_site::annotation::tracks(
            &counts,
            self.settings.locale().tag(),
            &Iris::new(self.base.as_str()).captions(row.id(), None),
            self.messages().kind_captions,
        );
        let languages: Vec<Language> = tracks
            .iter()
            .map(|track| Language {
                lang: track.lang.clone(),
                label: track.label.clone(),
                cues: track.cues,
            })
            .collect();
        // Most recordings carry no captions, so the cue read is skipped rather
        // than asked and discarded.
        if languages.is_empty() {
            return Ok(None);
        }
        let current = omeka_site::transcript::requested_language(&languages, requested)
            .map(str::to_string)
            .or_else(|| {
                tracks
                    .iter()
                    .find(|track| track.default)
                    .and_then(|track| track.lang.clone())
            });
        // Every supplementing row of the language is a cue, the hand-written
        // ones included: the transcript is the same text `captions.vtt` serves,
        // so the two surfaces cannot disagree.
        let rows = with_db!(self.pool, |db| MediaAnnotationRepo::new(db)
            .cues(row.id(), current.as_deref())
            .await)?;
        let cues: Vec<Cue> = rows
            .iter()
            .map(|row| Cue {
                id: row.id,
                start_ms: u64::try_from(row.t_start_ms.unwrap_or_default()).unwrap_or_default(),
                end_ms: u64::try_from(row.t_end_ms.unwrap_or_default()).unwrap_or_default(),
                text: omeka_site::transcript::cue_text(&row.body_json),
            })
            .filter(|cue| !cue.text.is_empty())
            .collect();
        if cues.is_empty() {
            return Ok(None);
        }
        Ok(Some(Transcript {
            languages,
            current,
            audio: kind == MediaKind::Audio,
            cues,
        }))
    }

    /// The notes one media page shows: a page of a hundred, or the page a cited
    /// note starts. `None` when the media has no note a reader may see, so its
    /// page is byte for byte the page without this unit.
    pub(crate) async fn media_notes(
        &self,
        row: &MediaRow,
        after: Option<i64>,
        cited: Option<i64>,
    ) -> Result<Option<NotesView>, DbError> {
        let Some(mut page) = self
            .notes_page(row, after.map(AnnotationId), NOTES_PER_PAGE)
            .await?
        else {
            return Ok(None);
        };
        // A cited note on a later page opens the page that starts at it; one on
        // the first page keeps the page a reader would have been reading.
        if after.is_none()
            && let Some(cited) = cited
            && page.next.is_some_and(|next| cited > next.0)
            && let Some(later) = self
                .notes_page(
                    row,
                    Some(AnnotationId(cited.saturating_sub(1))),
                    NOTES_PER_PAGE,
                )
                .await?
        {
            page = later;
        }
        if page.items.is_empty() {
            return Ok(None);
        }
        let page_url = resource_show_url(&self.site.slug, "media", row.id().0);
        let next_url = page
            .next
            .map(|next| format!("{page_url}?notes_after={}", next.0));
        Ok(Some(notes_view(&page, page_url, next_url, None, cited)))
    }

    /// The notes of every media on the page, one query, as the item page shows
    /// them: the first file's on the picture, the rest as a count on the strip.
    async fn item_notes(
        &self,
        media: &[db::MediaId],
    ) -> Result<std::collections::HashMap<db::MediaId, Summary>, DbError> {
        let annotations = Annotations::for_reader(self.pool, self.access.actor, self.extents);
        match annotations.summaries(media, ITEM_NOTES).await {
            Ok(summaries) => Ok(summaries),
            Err(AnnotationError::Db(error)) => Err(error),
            // A read the caller may not make is a media the caller may not see,
            // which this page already left out.
            Err(_) => Ok(std::collections::HashMap::new()),
        }
    }

    /// One notes page, with the service's own refusal read as "no notes".
    async fn notes_page(
        &self,
        row: &MediaRow,
        after: Option<AnnotationId>,
        limit: u32,
    ) -> Result<Option<Page>, DbError> {
        let annotations = Annotations::for_reader(self.pool, self.access.actor, self.extents);
        match annotations.notes_page(row.id(), after, limit).await {
            Ok(page) => Ok(Some(page)),
            Err(AnnotationError::NotFound) => Ok(None),
            Err(AnnotationError::Db(error)) => Err(error),
            Err(error) => Err(DbError::Column {
                column: "annotation",
                reason: error.to_string(),
            }),
        }
    }

    /// Wrap a media's rendered markup with its notes. The item page shows its
    /// first file whole, so that file's notes are drawn on it; the other files
    /// are cropped tiles, which name their count instead.
    pub(crate) fn annotate_first(&self, view: &mut ResourceView, notes: &NotesView) {
        let Some(embed) = view
            .media_embeds
            .first_mut()
            .and_then(|media| media.embed.take())
        else {
            return;
        };
        let wrapped = annotation::annotated(&embed, Some(notes), self.messages(), ASSET_BASE);
        if let Some(first) = view.media_embeds.first_mut() {
            first.embed = Some(wrapped);
        }
    }

    /// A media's place among the files of its item that the caller can see, in
    /// the item's position order. `None` when the caller cannot see the media
    /// itself, which the route refuses before this point anyway.
    async fn media_position(&self, row: &MediaRow) -> Result<Option<MediaPosition>, DbError> {
        let media = with_db!(self.pool, |db| MediaRepo::new(db)
            .list(
                &MediaFilter {
                    item_id: Some(row.item_id),
                    ..MediaFilter::default()
                },
                &Paging::default()
            )
            .await)?;
        let visible: Vec<&MediaRow> = media
            .iter()
            .filter(|entry| {
                self.access
                    .sees_resource(entry.resource.is_public, entry.resource.owner_id)
            })
            .collect();
        let Some(index) = visible.iter().position(|entry| entry.id() == row.id()) else {
            return Ok(None);
        };
        let link = |entry: &MediaRow| ResourceRef {
            url: resource_show_url(&self.site.slug, "media", entry.id().0),
            title: media_display_title(entry, self.messages()),
            thumbnail: None,
            media_kind: None,
            notes: 0,
        };
        Ok(Some(MediaPosition {
            number: index + 1,
            total: visible.len(),
            previous: index
                .checked_sub(1)
                .and_then(|i| visible.get(i))
                .map(|e| link(e)),
            next: visible.get(index + 1).map(|e| link(e)),
        }))
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
        let order = match &query.sort {
            Some(sort) => Some(self.order(sort).await?),
            None => None,
        };
        order_rows(
            &mut rows,
            order.as_ref(),
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
                with_db!(self.pool, |db| FulltextRepo::new(db)
                    .search(resource, needle)
                    .await)?
                .into_iter()
                .map(|hit| (hit.id.0, hit.score))
                .collect::<BTreeMap<u64, f64>>(),
            ),
            None => None,
        };
        let rows: Vec<LoadedRow> = match kind {
            ResourceKind::Item => with_db!(self.pool, |db| ItemRepo::new(db)
                .list_filtered(&ItemFilter::default(), &search.filter, &Paging::default())
                .await)?
            .into_iter()
            .map(LoadedRow::Item)
            .collect(),
            ResourceKind::ItemSet => with_db!(self.pool, |db| ItemSetRepo::new(db)
                .list_filtered(
                    &ItemSetFilter::default(),
                    &search.filter,
                    &Paging::default()
                )
                .await)?
            .into_iter()
            .map(LoadedRow::ItemSet)
            .collect(),
            ResourceKind::Media => with_db!(self.pool, |db| MediaRepo::new(db)
                .list_filtered(&MediaFilter::default(), &search.filter, &Paging::default())
                .await)?
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
        let messages = self.messages();
        let title = match kind {
            ResourceKind::Item => messages.items,
            ResourceKind::ItemSet => messages.item_sets,
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
                // `Site\ItemController::browseAction`: the browse is narrowed to
                // items attached to a block when `browse_attached_items` is on.
                if self.settings.browse_attached_items()
                    && !effective
                        .iter()
                        .any(|(key, _)| key == "site_attachments_only")
                {
                    effective.push(("site_attachments_only".to_string(), "1".to_string()));
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
            let order = self.order(&sort).await?;
            order_rows(&mut rows, Some(&order), None, false);
        }
        Ok(rows)
    }

    /// The order a `sort_by`/`sort_order` pair names. A property term loads
    /// each resource's sort text on that property; a term the catalog does
    /// not know orders by id.
    async fn order(&self, (sort_by, sort_order): &(String, String)) -> Result<Order, DbError> {
        let key = match sort_by.as_str() {
            "created" => SortKey::Created,
            "modified" => SortKey::Modified,
            "title" => SortKey::Title,
            term => match self.catalog.property_id(term) {
                Some(property) => SortKey::Property(
                    with_db!(self.pool, |db| ValueRepo::new(db)
                        .sort_texts(property)
                        .await)?
                    .into_iter()
                    .map(|(resource, text)| (resource.0, text))
                    .collect(),
                ),
                None => SortKey::Id,
            },
        };
        Ok(Order {
            key,
            descending: sort_order.eq_ignore_ascii_case("desc"),
        })
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
        let mut rows = with_db!(self.pool, |db| SiteRepo::new(db)
            .list(&SiteFilter::default(), &Paging::default())
            .await)?;
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
        Ok(with_db!(self.pool, |db| AssetRepo::new(db).get(id).await)?
            .as_ref()
            .map(|row| thumbnail_for_asset(self.files, row)))
    }

    /// The shared fields of every resource view: id, kind, URL, title, values
    /// and description.
    async fn base_view(
        &self,
        kind: ResourceKind,
        row: &ResourceRow,
    ) -> Result<ResourceView, DbError> {
        let url = resource_show_url(&self.site.slug, controller_for(kind), row.id.0);
        let mut view = ResourceView::new(
            ResourceId::new(row.id.0),
            kind,
            url,
            display_title(row, self.messages()),
        );
        view.jsonld = Some(self.jsonld(kind, row));
        view.owner = row.owner_id.map(|id| omeka_core::ids::UserId::new(id.0));
        view.is_public = row.is_public;
        view.class_label = self.class_label(row.resource_class_id).await?;
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
        let all = with_db!(self.pool, |db| ValueRepo::new(db)
            .list_for_resource(row.id)
            .await)?;
        let locale = self.settings.locale().tag();
        let filter_locale = self.settings.filter_locale_values();
        let exclude = self.settings.exclude_resources_not_in_site();
        let mut public: Vec<&ValueRow> = Vec::new();
        for value in &all {
            if !value.is_public || self.catalog.property(value.property_id).is_none() {
                continue;
            }
            // `resource-values.phtml` keeps a value whose language is empty or
            // the site's locale when `filter_locale_values` is on.
            if filter_locale && !in_locale(value.lang.as_deref(), locale) {
                continue;
            }
            // `displayValues` drops a referenced item or item set that is not
            // assigned to the site when `exclude_resources_not_in_site` is on.
            if exclude && self.value_resource_not_in_site(value).await? {
                continue;
            }
            public.push(value);
        }
        let template = self.template_properties(row.resource_template_id).await?;
        let order: Option<Vec<u64>> = template
            .as_ref()
            .map(|rows| rows.iter().map(|entry| entry.property_id.0).collect());
        let ordered = order_values(public, order.as_deref(), self.catalog);

        let mut values = Vec::with_capacity(ordered.len());
        let mut description = None;
        for value in ordered {
            let Some(info) = self.catalog.property(value.property_id) else {
                continue;
            };
            // `resource-values.phtml`: `alternate_label ?: $translate(label)`.
            let alternate = template
                .iter()
                .flatten()
                .find(|entry| entry.property_id == value.property_id)
                .and_then(|entry| entry.alternate_label.as_deref())
                .filter(|label| !label.is_empty());
            let label = alternate.map_or_else(|| self.property_label(info), str::to_string);
            let value_ref = self.value_ref(value, info, label).await?;
            if description.is_none() && info.term == "dcterms:description" {
                description = Some(value_ref.value.clone());
            }
            values.push(value_ref);
        }
        Ok((values, description))
    }

    /// One value as the theme renders it: its plain text, its `asHtml()` markup
    /// and the class and language the `<dd>` carries.
    async fn value_ref(
        &self,
        value: &ValueRow,
        info: &omeka_api::read::catalog::PropertyInfo,
        label: String,
    ) -> Result<ValueRef, DbError> {
        let (text, html, class) = if term_list::list_id(&value.r#type).is_some() {
            self.term_list_value(value).await?
        } else {
            match value.r#type.as_str() {
                // `DataType\Uri::render`: a link with the stored label, else the URI.
                "uri" => {
                    let uri = value.uri.clone().unwrap_or_default();
                    let label = value.value.clone().unwrap_or_default();
                    let text = if label.is_empty() { uri.clone() } else { label };
                    let html = crate::site::load::link_new_tab(&text, &uri);
                    (text, html, "uri".to_string())
                }
                // `DataType\Resource\AbstractResource::render`: `linkPretty('square')`.
                name if name.starts_with("resource") => match value.value_resource_id {
                    Some(id) => match self.value_resource_ref(id).await? {
                        Some((kind, reference)) => {
                            let text = reference.title.clone();
                            let html = link_pretty(&reference);
                            (text, html, format!("resource {}", resource_name(kind)))
                        }
                        None => (String::new(), String::new(), "resource".to_string()),
                    },
                    None => (String::new(), String::new(), "resource".to_string()),
                },
                // `DataType\Literal::render`: escaped text with newlines as `<br />`.
                _ => {
                    let text = value.value.clone().unwrap_or_default();
                    (
                        text.clone(),
                        nl2br(&omeka_site::html::escape_html(&text)),
                        "literal".to_string(),
                    )
                }
            }
        };
        Ok(ValueRef {
            property_id: value.property_id.0,
            property: label,
            property_term: info.term.clone(),
            vocabulary: info.vocabulary_label.clone(),
            value: text,
            html,
            lang: value.lang.clone().filter(|lang| !lang.is_empty()),
            class,
        })
    }

    /// A term list value as `DataType\CustomVocab::render` draws it: the item as
    /// `linkPretty('square')`, a URI as a new-tab link with its label, else the
    /// text. `resource-values.phtml` gives the `<dd>` the type name with `:`
    /// and `_` as `-`, and no `resource` or `uri` class, because the type name
    /// is neither.
    async fn term_list_value(&self, value: &ValueRow) -> Result<(String, String, String), DbError> {
        let class = value.r#type.replace([':', '_'], "-");
        let shape = ValueShape::of(
            &value.r#type,
            value.uri.is_some(),
            value.value_resource_id.is_some(),
        );
        let (text, html) = match (shape, value.value_resource_id) {
            (ValueShape::Resource, Some(id)) => match self.value_resource_ref(id).await? {
                Some((_, reference)) => (reference.title.clone(), link_pretty(&reference)),
                None => (String::new(), String::new()),
            },
            (ValueShape::Uri, _) => {
                let uri = value.uri.clone().unwrap_or_default();
                let label = value.value.clone().unwrap_or_default();
                let text = if label.is_empty() { uri.clone() } else { label };
                let html = omeka_site::html::hyperlink(
                    &text,
                    Some(&uri),
                    &[omeka_site::html::Attr::text("target", "_blank")],
                );
                (text, html)
            }
            _ => {
                let text = value.value.clone().unwrap_or_default();
                let html = nl2br(&omeka_site::html::escape_html(&text));
                (text, html)
            }
        };
        Ok((text, html, class))
    }

    /// The template's property rows in `list_properties` order, `None` when
    /// the resource carries no template.
    async fn template_properties(
        &self,
        template_id: Option<db::TemplateId>,
    ) -> Result<Option<Vec<TemplatePropertyRow>>, DbError> {
        let Some(template_id) = template_id else {
            return Ok(None);
        };
        let properties = with_db!(self.pool, |db| TemplateRepo::new(db)
            .list_properties(template_id)
            .await)?;
        Ok(Some(properties))
    }

    /// The value text for its data type: a literal, a URI, or the display title
    /// of the resource a `resource*` value points at.
    async fn value_text(&self, value: &ValueRow) -> Result<String, DbError> {
        if term_list::list_id(&value.r#type).is_some() {
            // `CustomVocab::toString` is the stored value unless the value is
            // an item, which reads as the item's title here as it does for the
            // core resource types.
            return match value.value_resource_id {
                Some(id) => self.resource_title(id).await,
                None => Ok(value.value.clone().unwrap_or_default()),
            };
        }
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
        Ok(
            with_db!(self.pool, |db| ResourceRepo::new(db).get(id).await)?
                .map(|row| display_title(&row, self.messages()))
                .unwrap_or_default(),
        )
    }

    /// The item's item sets, media and site pages, plus the resources that link
    /// to it, for the resource page blocks and the item-with-metadata block. A
    /// hidden item set or media row is dropped, matching
    /// `ResourceVisibilityFilter` on the related entities.
    async fn add_item_lists(&self, row: &ItemRow, view: &mut ResourceView) -> Result<(), DbError> {
        let exclude = self.settings.exclude_resources_not_in_site();
        // `ItemSets.php`: only item sets assigned to the site when
        // `exclude_resources_not_in_site` is on.
        let site_item_sets: Vec<db::ItemSetId> = if exclude {
            with_db!(self.pool, |db| SiteRepo::new(db)
                .list_item_sets(self.db_site_id())
                .await)?
            .into_iter()
            .map(|entry| entry.item_set_id)
            .collect()
        } else {
            Vec::new()
        };
        for id in with_db!(self.pool, |db| ItemRepo::new(db)
            .list_item_sets(row.id())
            .await)?
        {
            if exclude && !site_item_sets.contains(&id) {
                continue;
            }
            if let Some(set) = with_db!(self.pool, |db| ItemSetRepo::new(db).get(id).await)?
                && self
                    .access
                    .sees_resource(set.resource.is_public, set.resource.owner_id)
            {
                view.item_sets.push(ResourceRef {
                    url: resource_show_url(&self.site.slug, "item-set", set.id().0),
                    title: display_title(&set.resource, self.messages()),
                    thumbnail: None,
                    media_kind: None,
                    notes: 0,
                });
            }
        }
        let media = with_db!(self.pool, |db| MediaRepo::new(db)
            .list(
                &MediaFilter {
                    item_id: Some(row.id()),
                    ..MediaFilter::default()
                },
                &Paging::default()
            )
            .await)?;
        let item_url = view.url.clone();
        let visible: Vec<MediaRow> = media
            .into_iter()
            .filter(|entry| {
                self.access
                    .sees_resource(entry.resource.is_public, entry.resource.owner_id)
            })
            .collect();
        // One summary read for every file's notes: the first file's are drawn
        // on the picture, and each tile of the strip names its own count.
        let ids: Vec<db::MediaId> = visible.iter().map(MediaRow::id).collect();
        let notes = if ids.is_empty() {
            std::collections::HashMap::new()
        } else {
            self.item_notes(&ids).await?
        };
        for entry in &visible {
            let kind = media_kind(entry);
            // A kind that paints a canvas gives the item a manifest to share;
            // a PDF without a first page, a model or an embed gives none.
            if view.iiif_manifest.is_none() && kind.canvas_body().is_some() {
                view.iiif_manifest = Some(format!(
                    "{}/iiif/3/item/{}/manifest",
                    self.base.as_str(),
                    row.id().0
                ));
            }
            let title = media_display_title(entry, self.messages());
            view.media.push(ResourceRef {
                url: resource_show_url(&self.site.slug, "media", entry.id().0),
                title: title.clone(),
                thumbnail: square_thumbnail(self.files, entry, row.resource.is_public, &title),
                media_kind: Some(media_kind_cue(kind, self.messages())),
                notes: notes.get(&entry.id()).map_or(0, |summary| summary.total),
            });
            // `media-embeds.phtml`: `$media->render()` with the renderer's own
            // defaults, except that the file opens its media page rather than
            // the raw original (a Romeka divergence, `docs/compat.md`).
            view.media_embeds.push(
                self.media_ref(
                    entry,
                    &item_url,
                    row.resource.is_public,
                    &RenderOptions {
                        link: Some(LinkType::Media),
                        ..RenderOptions::default()
                    },
                )
                .await?,
            );
        }
        // The item page draws the first file whole, so its notes are drawn on
        // it, capped at eight with a link to the rest.
        if let Some(first) = visible.first()
            && let Some(summary) = notes.get(&first.id())
        {
            let page_url = resource_show_url(&self.site.slug, "media", first.id().0);
            let more_url = (summary.total > ITEM_NOTES).then(|| page_url.clone());
            let view_notes =
                summary_view(first.id(), media_kind(first), summary, page_url, more_url);
            self.annotate_first(view, &view_notes);
        }
        view.file_count = view.media.len();
        for page in with_db!(self.pool, |db| SiteRepo::new(db)
            .list_pages_attaching_item(self.db_site_id(), row.id())
            .await)?
        {
            if let Ok(slug) = Slug::parse(&page.slug) {
                view.site_pages.push(ResourceRef {
                    url: page_url(&self.site.slug, &slug),
                    title: page.title.clone(),
                    thumbnail: None,
                    media_kind: None,
                    notes: 0,
                });
            }
        }
        view.linked_resources = self.linked_resources(row.resource.id).await?;
        Ok(())
    }

    /// The resources whose values point at this resource, for the
    /// `linkedResources` block (`subjectValues`).
    async fn linked_resources(&self, id: db::ResourceId) -> Result<Vec<LinkedResource>, DbError> {
        let exclude = self.settings.exclude_resources_not_in_site();
        let mut out = Vec::new();
        for value in with_db!(self.pool, |db| ValueRepo::new(db)
            .list_pointing_at(id, &Paging::default())
            .await)?
        {
            if !value.is_public {
                continue;
            }
            let Some((kind, resource)) = self.value_resource_ref(value.resource_id).await? else {
                continue;
            };
            if exclude
                && matches!(kind, ResourceKind::Item | ResourceKind::ItemSet)
                && !self.resource_in_site(value.resource_id, kind).await?
            {
                continue;
            }
            let property = self
                .catalog
                .property(value.property_id)
                .map(|info| self.property_label(info))
                .unwrap_or_default();
            let class = with_db!(self.pool, |db| ResourceRepo::new(db)
                .get(value.resource_id)
                .await)?
            .and_then(|row| row.resource_class_id);
            out.push(LinkedResource {
                property,
                kind,
                resource,
                class_label: self.class_label(class).await?,
            });
        }
        Ok(out)
    }

    /// A resource class's label, translated by term the way a property label
    /// is (`resourceClass()->label()` through `translate`). `None` when the
    /// resource has no class or the row is gone.
    async fn class_label(&self, class: Option<db::ClassId>) -> Result<Option<String>, DbError> {
        let Some(id) = class else {
            return Ok(None);
        };
        Ok(
            with_db!(self.pool, |db| ClassRepo::new(db).get(id).await)?.map(|row| {
                let term = format!(
                    "{}:{}",
                    self.catalog.prefix(row.vocabulary_id),
                    row.local_name
                );
                self.settings
                    .locale()
                    .translator()
                    .term_label(&term, &row.label)
                    .to_string()
            }),
        )
    }

    /// A resource a value points at, as its pretty link needs it. `None` when
    /// the row is gone or the caller cannot see it.
    async fn value_resource_ref(
        &self,
        id: db::ResourceId,
    ) -> Result<Option<(ResourceKind, ResourceRef)>, DbError> {
        if let Some(row) = with_db!(self.pool, |dbp| ItemRepo::new(dbp)
            .get(db::ItemId::from(id.0))
            .await)?
        {
            if !self
                .access
                .sees_resource(row.resource.is_public, row.resource.owner_id)
            {
                return Ok(None);
            }
            let title = display_title(&row.resource, self.messages());
            let thumbnail = self.item_square_thumbnail(&row, &title).await?;
            return Ok(Some((
                ResourceKind::Item,
                ResourceRef {
                    url: resource_show_url(&self.site.slug, "item", id.0),
                    title,
                    thumbnail,
                    media_kind: None,
                    notes: 0,
                },
            )));
        }
        if let Some(row) = with_db!(self.pool, |dbp| ItemSetRepo::new(dbp)
            .get(db::ItemSetId::from(id.0))
            .await)?
        {
            if !self
                .access
                .sees_resource(row.resource.is_public, row.resource.owner_id)
            {
                return Ok(None);
            }
            let thumbnail = match row.resource.thumbnail_id {
                Some(asset) => self.asset_thumbnail(asset).await?,
                None => None,
            };
            return Ok(Some((
                ResourceKind::ItemSet,
                ResourceRef {
                    url: resource_show_url(&self.site.slug, "item-set", id.0),
                    title: display_title(&row.resource, self.messages()),
                    thumbnail,
                    media_kind: None,
                    notes: 0,
                },
            )));
        }
        if let Some(row) = with_db!(self.pool, |dbp| MediaRepo::new(dbp)
            .get(db::MediaId::from(id.0))
            .await)?
        {
            if !self
                .access
                .sees_resource(row.resource.is_public, row.resource.owner_id)
            {
                return Ok(None);
            }
            let title = media_display_title(&row, self.messages());
            let item_is_public = self.item_is_public(row.item_id).await?;
            let thumbnail = square_thumbnail(self.files, &row, item_is_public, &title);
            return Ok(Some((
                ResourceKind::Media,
                ResourceRef {
                    url: resource_show_url(&self.site.slug, "media", id.0),
                    title,
                    thumbnail,
                    media_kind: None,
                    notes: 0,
                },
            )));
        }
        Ok(None)
    }

    /// Whether a value's resource is not assigned to this site, for
    /// `exclude_resources_not_in_site`. Only items and item sets carry the
    /// membership upstream checks.
    async fn value_resource_not_in_site(&self, value: &ValueRow) -> Result<bool, DbError> {
        let Some(id) = value.value_resource_id else {
            return Ok(false);
        };
        for kind in [ResourceKind::Item, ResourceKind::ItemSet] {
            if self.resource_exists(id, kind).await? {
                return Ok(!self.resource_in_site(id, kind).await?);
            }
        }
        Ok(false)
    }

    /// Whether a resource of a kind has this id.
    async fn resource_exists(
        &self,
        id: db::ResourceId,
        kind: ResourceKind,
    ) -> Result<bool, DbError> {
        Ok(match kind {
            ResourceKind::Item => with_db!(self.pool, |dbp| ItemRepo::new(dbp)
                .get(db::ItemId::from(id.0))
                .await)?
            .is_some(),
            ResourceKind::ItemSet => with_db!(self.pool, |dbp| ItemSetRepo::new(dbp)
                .get(db::ItemSetId::from(id.0))
                .await)?
            .is_some(),
            ResourceKind::Media => with_db!(self.pool, |dbp| MediaRepo::new(dbp)
                .get(db::MediaId::from(id.0))
                .await)?
            .is_some(),
        })
    }

    /// Whether a resource is attached to this site (`item_site` for an item, a
    /// media's item for a media, `site_item_set` for an item set).
    async fn resource_in_site(
        &self,
        id: db::ResourceId,
        kind: ResourceKind,
    ) -> Result<bool, DbError> {
        match kind {
            ResourceKind::Item => self.item_in_site(db::ItemId::from(id.0)).await,
            ResourceKind::ItemSet => self.item_set_in_site(db::ItemSetId::from(id.0)).await,
            ResourceKind::Media => match with_db!(self.pool, |dbp| MediaRepo::new(dbp)
                .get(db::MediaId::from(id.0))
                .await)?
            {
                Some(media) => self.item_in_site(media.item_id).await,
                None => Ok(false),
            },
        }
    }

    /// The square thumbnail a resource value links, from the item's primary (or
    /// first) media.
    async fn item_square_thumbnail(
        &self,
        row: &ItemRow,
        alt: &str,
    ) -> Result<Option<Thumbnail>, DbError> {
        let media = match row.primary_media_id {
            Some(id) => {
                with_db!(self.pool, |db| MediaRepo::new(db).get(id).await)?.filter(|media| {
                    self.access
                        .sees_resource(media.resource.is_public, media.resource.owner_id)
                })
            }
            None => self.first_media(row.id()).await?,
        };
        Ok(media
            .and_then(|media| square_thumbnail(self.files, &media, row.resource.is_public, alt)))
    }

    /// The item's first media row the caller can see, `ItemRepresentation`'s
    /// primary-media fallback under `ResourceVisibilityFilter`.
    async fn first_media(&self, item_id: db::ItemId) -> Result<Option<MediaRow>, DbError> {
        Ok(with_db!(self.pool, |db| MediaRepo::new(db)
            .list(
                &MediaFilter {
                    item_id: Some(item_id),
                    ..MediaFilter::default()
                },
                &Paging::default()
            )
            .await)?
        .into_iter()
        .find(|media| {
            self.access
                .sees_resource(media.resource.is_public, media.resource.owner_id)
        }))
    }

    async fn thumbnail_for(
        &self,
        media_id: db::MediaId,
        item_is_public: bool,
        alt: &str,
    ) -> Result<Option<Thumbnail>, DbError> {
        Ok(
            with_db!(self.pool, |db| MediaRepo::new(db).get(media_id).await)?
                .and_then(|media| media_thumbnail(self.files, &media, item_is_public, alt)),
        )
    }

    /// The run time `romeka_media_extent` records for a media's original,
    /// `None` when no probe has written a row or the row is stale. A card shows
    /// a duration only from here: the file itself is never read to guess one.
    async fn media_duration(&self, row: &MediaRow) -> Result<Option<u64>, DbError> {
        let Some(storage_id) = row.storage_id.as_deref() else {
            return Ok(None);
        };
        let path = match row.extension.as_deref() {
            Some(extension) => format!("original/{storage_id}.{extension}"),
            None => format!("original/{storage_id}"),
        };
        Ok(with_db!(self.pool, |db| MediaExtentRepo::new(db)
            .get(row.id(), &path)
            .await)?
        .and_then(|extent| extent.duration_ms))
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

/// `displayTitle`: the stored title, else upstream's translated placeholder.
pub(crate) fn display_title(row: &ResourceRow, messages: &Messages) -> String {
    row.title
        .clone()
        .unwrap_or_else(|| messages.untitled.to_string())
}

/// A media's original as the media page offers it: the site's download route
/// and the stored byte count. `None` when the media stores no original, as an
/// embed or a remote IIIF image does not.
fn download_link(row: &MediaRow, media_url: &str) -> Option<DownloadLink> {
    if !row.has_original || row.storage_id.is_none() {
        return None;
    }
    Some(DownloadLink {
        url: format!("{media_url}/download"),
        size: row.size.and_then(|size| u64::try_from(size).ok()),
    })
}

/// The port's kind for a media row, from the same table the API's `o:kind` and
/// the IIIF canvas read.
fn media_kind(row: &MediaRow) -> MediaKind {
    MediaKind::of(&KindInput {
        renderer: &row.renderer,
        ingester: &row.ingester,
        media_type: row.media_type.as_deref(),
        has_thumbnails: row.has_thumbnails,
    })
}

/// The kind as a card draws it: the server's name, the kind in words and the
/// theme's glyph. The words are the site catalogue's, so every locale names a
/// kind in its own language.
fn media_kind_cue(kind: MediaKind, messages: &Messages) -> MediaKindCue {
    let (label, icon) = match kind {
        MediaKind::LocalImage | MediaKind::IiifImage => (messages.kind_image, MediaIcon::Image),
        MediaKind::Audio => (messages.kind_audio, MediaIcon::Audio),
        MediaKind::Video => (messages.kind_video, MediaIcon::Video),
        MediaKind::Pdf { .. } => (messages.kind_pdf, MediaIcon::Pdf),
        MediaKind::Model3d => (messages.kind_model_3d, MediaIcon::Model),
        MediaKind::Captions => (messages.kind_captions, MediaIcon::Captions),
        MediaKind::Embed | MediaKind::IiifPresentation => (messages.kind_embed, MediaIcon::Embed),
        MediaKind::OtherFile => (messages.kind_other_file, MediaIcon::File),
    };
    MediaKindCue {
        kind: kind.name(),
        label,
        icon,
    }
}

/// Whether a kind can carry a run time, and so may show one from
/// `romeka_media_extent`.
const fn is_recording(kind: MediaKind) -> bool {
    matches!(kind, MediaKind::Audio | MediaKind::Video)
}

/// The most notes a media page lists at once. The rest are a "Next" link.
const NOTES_PER_PAGE: u32 = 100;

/// The most notes the item page shows for its first file.
const ITEM_NOTES: u32 = 8;

/// A notes page as the site renders it.
fn notes_view(
    page: &Page,
    page_url: String,
    next_url: Option<String>,
    more_url: Option<String>,
    cited: Option<i64>,
) -> NotesView {
    notes_of(
        page.media.id.0,
        page.media.kind,
        page.media.extent,
        page.total,
        &page.items,
        page_url,
        next_url,
        more_url,
        cited,
    )
}

/// One item's summary of a media as the item page renders it. The summary
/// carries the media's extent; its kind comes from the media row the item page
/// read, so an image draws its overlay and a recording its rail.
fn summary_view(
    media: db::MediaId,
    kind: MediaKind,
    summary: &Summary,
    page_url: String,
    more_url: Option<String>,
) -> NotesView {
    notes_of(
        media.0,
        kind,
        summary.extent,
        summary.total,
        &summary.first,
        page_url,
        None,
        more_url,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
fn notes_of(
    media_id: u64,
    kind: MediaKind,
    extent: omeka_media::kind::Extent,
    total: u32,
    items: &[Annotation],
    page_url: String,
    next_url: Option<String>,
    more_url: Option<String>,
    cited: Option<i64>,
) -> NotesView {
    let surface = surface_of(kind);
    NotesView {
        media_id,
        page_url,
        surface,
        total,
        notes: note_lines(items),
        canvas: if surface == Surface::Image {
            extent.size
        } else {
            None
        },
        duration_ms: extent.duration_ms,
        next_url,
        more_url,
        cited,
    }
}

/// What a media's notes address: a picture's regions, a recording's moments,
/// any other file as a whole.
fn surface_of(kind: MediaKind) -> Surface {
    match kind.surface() {
        omeka_media::kind::Surface::Image => Surface::Image,
        omeka_media::kind::Surface::Time => Surface::Time,
        omeka_media::kind::Surface::Whole => Surface::Whole,
    }
}

fn note_lines(items: &[Annotation]) -> Vec<NoteLine> {
    items
        .iter()
        .map(|note| NoteLine {
            id: note.id.0,
            place: match &note.draft.selector {
                Selector::Whole => Place::Whole,
                Selector::Rect { x, y, w, h } => Place::Rect {
                    x: *x,
                    y: *y,
                    w: *w,
                    h: *h,
                },
                Selector::Polygon(points) => Place::Polygon(points.clone()),
                Selector::Time { start_ms, end_ms } => Place::Time {
                    start_ms: *start_ms,
                    end_ms: *end_ms,
                },
            },
            text: note
                .draft
                .bodies
                .iter()
                .find_map(|body| match body {
                    Body::Text { value, .. } => Some(value.clone()),
                    Body::Link { .. } => None,
                })
                .unwrap_or_default(),
        })
        .collect()
}

/// `MediaRepresentation::displayTitle`: the stored title, else the file's own
/// name, else `[Untitled]`.
///
/// Upstream passes `source()` as the parent's default, so a media with no
/// title reads as its whole source: for an upload, the file name a curator
/// chose, and for a fetched file the entire URL. The name is the last path
/// segment here, because the heading, the card, the item page's strip and the
/// document title all show it, and a URL is not a name a reader can scan. The
/// name comes from the same function that names a download, so the page and
/// the saved file agree.
pub(crate) fn media_display_title(row: &MediaRow, messages: &Messages) -> String {
    let title = row
        .resource
        .title
        .as_deref()
        .map(str::trim)
        .filter(|title| !title.is_empty());
    if let Some(title) = title {
        return title.to_string();
    }
    match row
        .source
        .as_deref()
        .map(|source| download_name(Some(source), ""))
    {
        Some(name) if !name.trim().is_empty() => name,
        _ => messages.untitled.to_string(),
    }
}

/// A `media` row as the core media value the renderers read. The columns the
/// media subsystem does not use (owner, class, template, thumbnail, language)
/// are left at their defaults. `alt_text` is the resolved alt text, which the
/// caller reads through [`omeka_api::settings::media_alt_text`].
pub(crate) fn media_core(row: &MediaRow, alt_text: Option<String>) -> Media {
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
    media.alt_text = Some(alt_text.unwrap_or_default());
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
fn media_thumbnail(
    files: &FileUrls,
    media: &MediaRow,
    item_is_public: bool,
    alt: &str,
) -> Option<Thumbnail> {
    thumbnail_at(files, media, "medium", item_is_public, alt)
}

/// The square thumbnail of a media file, the size `linkPretty('square')` asks
/// for (`FileRenderer/*` exposes `square/{storage_id}.jpg`).
fn square_thumbnail(
    files: &FileUrls,
    media: &MediaRow,
    item_is_public: bool,
    alt: &str,
) -> Option<Thumbnail> {
    thumbnail_at(files, media, "square", item_is_public, alt)
}

/// A thumbnail URL is plain only when the media and its item are public.
fn thumbnail_at(
    files: &FileUrls,
    media: &MediaRow,
    size: &str,
    item_is_public: bool,
    alt: &str,
) -> Option<Thumbnail> {
    if !media.has_thumbnails {
        return None;
    }
    let storage_id = media.storage_id.as_deref()?;
    let kind = FileKind::Thumbnail {
        public: media.resource.is_public && item_is_public,
    };
    Some(Thumbnail::new(
        files.url(&format!("{size}/{storage_id}.jpg"), kind),
        alt,
    ))
}

/// `Hyperlink` with a new tab, `DataType\Uri::render`'s `class="uri-value-link"
/// target="_blank"`.
fn link_new_tab(text: &str, href: &str) -> String {
    omeka_site::html::hyperlink(
        text,
        Some(href),
        &[
            omeka_site::html::Attr::text("class", "uri-value-link"),
            omeka_site::html::Attr::text("target", "_blank"),
        ],
    )
}

/// PHP's `nl2br`: `<br />` before every newline, keeping the newline.
fn nl2br(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '\r' => {
                out.push_str("<br />");
                out.push('\r');
                if chars.peek() == Some(&'\n') {
                    out.push('\n');
                    chars.next();
                }
            }
            '\n' => {
                out.push_str("<br />");
                out.push('\n');
            }
            _ => out.push(ch),
        }
    }
    out
}

/// Whether a value counts under `filter_locale_values`: an untagged or empty
/// language, or the site locale in any case (`resource-values.phtml:11-14`
/// with `strcasecmp`, and `AbstractResourceEntityRepresentation::value`, which
/// lowercases both sides). Both the browse body and the show values read it.
fn in_locale(lang: Option<&str>, locale: &str) -> bool {
    lang.is_none_or(|lang| lang.is_empty() || lang.eq_ignore_ascii_case(locale))
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

/// Two resources in a requested order. A tie, and a resource with no value
/// on a property sort, fall back to the id; a missing value sorts first, as
/// MySQL orders `NULL` ascending.
fn compare_resources(a: &ResourceRow, b: &ResourceRow, order: Option<&Order>) -> Ordering {
    let Some(order) = order else {
        return a.id.cmp(&b.id);
    };
    let by_key = match &order.key {
        SortKey::Id => Ordering::Equal,
        SortKey::Created => a.created.cmp(&b.created),
        SortKey::Modified => a.modified.cmp(&b.modified),
        SortKey::Title => compare_opt_text_ci(a.title.as_deref(), b.title.as_deref()),
        SortKey::Property(texts) => compare_opt_text_ci(
            texts.get(&a.id.0).map(String::as_str),
            texts.get(&b.id.0).map(String::as_str),
        ),
    };
    let ordering = by_key.then(a.id.cmp(&b.id));
    if order.descending {
        ordering.reverse()
    } else {
        ordering
    }
}

/// The thumbnail an asset shows: `AssetRepresentation::thumbnail` returns the
/// asset itself, so the image is the asset's file URL and the alt is its
/// alt text (`AbstractRepresentation::thumbnailAltText`).
pub(crate) fn thumbnail_for_asset(files: &FileUrls, row: &AssetRow) -> Thumbnail {
    Thumbnail::new(
        asset_url(files, row),
        row.alt_text.clone().unwrap_or_default(),
    )
}

/// `AssetRepresentation::assetUrl`: the stored file under `asset/<filename>`.
fn asset_url(files: &FileUrls, row: &AssetRow) -> String {
    let filename = match row.extension.as_deref() {
        Some(extension) if !extension.is_empty() => format!("{}.{extension}", row.storage_id),
        _ => row.storage_id.clone(),
    };
    files.url(&format!("asset/{filename}"), FileKind::Asset)
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
