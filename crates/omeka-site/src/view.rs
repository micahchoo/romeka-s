//! What a caller hands the site layer: the site, the page, the settings, the
//! resolved navigation, and a [`ResourceProvider`] for the blocks and views
//! that show stored resources.
//!
//! Upstream's site views reach a database through `$view->api()->search(...)`.
//! The port keeps that lookup behind one trait, so the renderer itself stays a
//! pure transform from values to HTML and a test can render a page with no
//! database. The shapes below carry exactly what the templates read: a
//! resource's display title and description, its thumbnail, its item sets and
//! its media, and a site entry for the list-of-sites block.

use omeka_core::ids::{AssetId, BlockId, ItemId, MediaId, ResourceId, SiteId, UserId};
use omeka_core::permissions::RoleSet;
use omeka_core::resource::{ResourceKind, Timestamp};
use omeka_core::site::{Site, SitePage};

use crate::i18n::{Messages, Translator};
use crate::nav::{NavItem, PageRef};
use crate::resource_page::ResourcePageBlocks;
use crate::search::{AdvancedSearchForm, CrossSitePage, SearchList, SearchResults};
use crate::settings::SiteSettings;
use crate::theme::ThemeSettings;

/// A thumbnail image, as `Laminas`'s `thumbnail` helper returns it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Thumbnail {
    /// The image URL.
    pub url: String,
    /// The alt text.
    pub alt: String,
}

impl Thumbnail {
    /// A thumbnail with the given URL and alt text.
    #[must_use]
    pub fn new(url: impl Into<String>, alt: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            alt: alt.into(),
        }
    }
}

/// A resource as one of the site's resource-backed templates shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceView {
    /// `resource.id`.
    pub id: ResourceId,
    /// The resource kind.
    pub kind: ResourceKind,
    /// The canonical site URL (`ItemRepresentation::siteUrl`).
    pub url: String,
    /// `displayTitle`: the title, or the caller's placeholder.
    pub title: String,
    /// `resource.owner_id`, which decides whether the caller may edit it.
    pub owner: Option<UserId>,
    /// The resource class label (`resourceClass()->label()`), translated by
    /// term, `None` when the resource has no class.
    pub class_label: Option<String>,
    /// `displayDescription`, `None` when the resource has none.
    pub description: Option<String>,
    /// The values `displayValues` prints, in property order.
    pub values: Vec<ValueRef>,
    /// The medium thumbnail, `None` when the resource has no thumbnail.
    pub thumbnail: Option<Thumbnail>,
    /// The resource's media with their rendered markup, for the `mediaEmbeds`
    /// resource page block (`$item->media()` then `$media->render()`).
    pub media_embeds: Vec<MediaRef>,
    /// The item's item sets, for the item-with-metadata block and the
    /// `itemSets` resource page block.
    pub item_sets: Vec<ResourceRef>,
    /// The item's media, for the item-with-metadata and `mediaList` blocks.
    pub media: Vec<ResourceRef>,
    /// The site pages that attach this item, for the `sitePages` block
    /// (`ItemRepresentation::sitePages`).
    pub site_pages: Vec<ResourceRef>,
    /// The resources whose values point at this resource, for the
    /// `linkedResources` block (`subjectValues`).
    pub linked_resources: Vec<LinkedResource>,
    /// The item's primary media, for the media block.
    pub primary_media: Option<MediaRef>,
    /// For a media resource, the item it belongs to (`Media::item()`), for the
    /// media view's "Part of" line.
    pub parent_item: Option<ResourceRef>,
    /// The resource's embedded JSON-LD document, a compact JSON object the show
    /// and browse views print in a `application/ld+json` script (`Module.php`
    /// `view.show.after` / `view.browse.after`, honouring
    /// `disable_jsonld_embed`). `None` when the caller did not build one.
    pub jsonld: Option<String>,
    /// The item's IIIF manifest URL, absolute, when the item has a media a
    /// canvas can paint. The item view links it and opens it in the viewer.
    pub iiif_manifest: Option<String>,
}

impl ResourceView {
    /// A resource with only the fields every view reads.
    #[must_use]
    pub fn new(
        id: ResourceId,
        kind: ResourceKind,
        url: impl Into<String>,
        title: impl Into<String>,
    ) -> Self {
        Self {
            id,
            kind,
            url: url.into(),
            title: title.into(),
            owner: None,
            class_label: None,
            description: None,
            values: Vec::new(),
            thumbnail: None,
            media_embeds: Vec::new(),
            item_sets: Vec::new(),
            media: Vec::new(),
            site_pages: Vec::new(),
            linked_resources: Vec::new(),
            primary_media: None,
            parent_item: None,
            jsonld: None,
            iiif_manifest: None,
        }
    }
}

/// A linked resource in a list, e.g. one of an item's item sets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceRef {
    /// The resource URL.
    pub url: String,
    /// The display title.
    pub title: String,
    /// The square thumbnail, `None` when there is none.
    pub thumbnail: Option<Thumbnail>,
}

/// One displayed metadata value, the pair `displayValues` prints.
///
/// `value` is the plain text (a literal, a URI, or a referenced resource's
/// display title), which the browse views and the description read. `html` is
/// the value's `asHtml()` markup: escaped literal text, a URI link, or a pretty
/// link to the referenced resource. The site layer builds both at the storage
/// boundary; the theme only places them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValueRef {
    /// The property id, so values of one property group under one heading.
    pub property_id: u64,
    /// The property label.
    pub property: String,
    /// The property term, e.g. `dcterms:title`, for `property_label_information`
    /// `term`.
    pub property_term: String,
    /// The property's vocabulary label, for `property_label_information` `vocab`.
    pub vocabulary: String,
    /// The value text.
    pub value: String,
    /// The `asHtml()` markup.
    pub html: String,
    /// The value's language tag, `None` when unset.
    pub lang: Option<String>,
    /// The `<dd>` class after `value`, e.g. `literal`, `uri` or `resource items`.
    pub class: String,
}

/// A resource whose value points at the resource being shown, for the
/// `linkedResources` block (`subjectValues`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkedResource {
    /// The property label the linking value uses.
    pub property: String,
    /// The kind of the linking resource.
    pub kind: ResourceKind,
    /// The linking resource.
    pub resource: ResourceRef,
    /// The linking resource's class label, `None` when it has no class.
    pub class_label: Option<String>,
}

/// A media file, as the media block shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaRef {
    /// The media URL (`media->url()`).
    pub url: String,
    /// The original file URL (`media->originalUrl()`), `None` when the file is
    /// not stored locally.
    pub original_url: Option<String>,
    /// The display title (`MediaRepresentation::displayTitle`).
    pub title: String,
    /// `media.media_type`, e.g. `image/jpeg`, `None` when the media has no file.
    pub media_type: Option<String>,
    /// The thumbnail, `None` when there is none.
    pub thumbnail: Option<Thumbnail>,
    /// The rendered embed HTML from the media renderer, `None` when the media
    /// type has no embed. This is the `omeka-media` seam: the site layer places
    /// the markup, it never builds it.
    pub embed: Option<String>,
}

/// An asset, as the asset block shows it. The background of a block names an
/// asset by id, so this carries the URL the background-image style uses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetView {
    /// The asset id.
    pub id: AssetId,
    /// The display title.
    pub title: String,
    /// The asset's file URL (`AssetRepresentation::assetUrl`), `None` when the
    /// caller cannot resolve one.
    pub url: Option<String>,
    /// The square thumbnail, `None` when there is none.
    pub thumbnail: Option<Thumbnail>,
}

/// One `site_block_attachment` row (`schema.sql:251-263`), which core leaves to
/// wave 2. The item-with-metadata and media blocks read their content from it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttachmentRef {
    /// `item_id`, null after the item is deleted.
    pub item: Option<ItemId>,
    /// `media_id`, null after the media is deleted.
    pub media: Option<MediaId>,
    /// `caption`, never null upstream.
    pub caption: String,
}

/// A site entry, as the list-of-sites block shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SiteView {
    /// `site.id`.
    pub id: SiteId,
    /// `site.title`.
    pub title: String,
    /// The site URL.
    pub url: String,
    /// `site.summary`, `None` when unset.
    pub summary: Option<String>,
    /// `site.thumbnail`, `None` when unset.
    pub thumbnail: Option<Thumbnail>,
}

/// The order a list-of-sites block asks for (`BlockLayout/ListOfSites.php`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SiteSort {
    /// `alpha`, by title.
    Alpha,
    /// `oldest`, by creation time.
    Oldest,
    /// `newest`, by creation time, newest first.
    Newest,
}

/// The search a browse-preview block or a browse view runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowseQuery {
    /// The parsed advanced-search query, in stored order.
    pub query: Vec<(String, String)>,
    /// The site the resources must belong to.
    pub site: SiteId,
    /// `limit`, the maximum number of resources.
    pub limit: Option<usize>,
    /// `sort_by` and `sort_order`, when set.
    pub sort: Option<(String, String)>,
}

/// A search result: the resources plus the total the query matched.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResourcePage {
    /// The resources on this page.
    pub resources: Vec<ResourceView>,
    /// The total number of matching resources.
    pub total: usize,
}

/// The search a list-of-sites block runs (`BlockLayout/ListOfSites.php`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SiteQuery {
    /// The requested order.
    pub sort: SiteSort,
    /// `limit`, `None` for no limit.
    pub limit: Option<usize>,
    /// `exclude_id`, the current site to hide, `None` to show every site.
    pub exclude: Option<SiteId>,
    /// `page`, one-based, when pagination is on.
    pub page: Option<usize>,
}

/// A site search result: the sites plus the total the query matched.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SiteSearch {
    /// The sites on this page.
    pub sites: Vec<SiteView>,
    /// The total number of matching sites.
    pub total: usize,
}

/// Where the blocks and views read stored resources.
///
/// The site layer never opens a database. A caller that has one (the server)
/// implements this trait over `omeka-db`; a test uses [`NoResources`].
pub trait ResourceProvider {
    /// Search resources of one kind, as `$view->api()->search()` does.
    fn search(&self, kind: ResourceKind, query: &BrowseQuery) -> ResourcePage;

    /// Read one resource by id, `None` when it does not exist or is not in the
    /// site. `kind` picks the id space, matching `$view->api()->read()`.
    fn read(&self, kind: ResourceKind, id: u64) -> Option<ResourceView>;

    /// Read one media by id, `None` when it does not exist. The media block
    /// needs the media's own URLs and its rendered embed.
    fn read_media(&self, id: MediaId) -> Option<MediaRef>;

    /// Read one asset by id, `None` when it does not exist.
    fn read_asset(&self, id: AssetId) -> Option<AssetView>;

    /// The `site_block_attachment` rows of a block, ordered by `position`.
    ///
    /// The item-with-metadata, media and asset blocks read their content here;
    /// an asset block keys its attachments by index into its `data` list.
    fn attachments(&self, block: BlockId) -> Vec<AttachmentRef>;

    /// Search sites, `$view->api()->search('sites', ...)`.
    fn search_sites(&self, query: &SiteQuery) -> SiteSearch;
}

/// A provider with no data: every search is empty, every read is missing.
///
/// This is what a caller with no storage gets, and what a block render with no
/// provider uses in tests.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoResources;

impl ResourceProvider for NoResources {
    fn search(&self, _kind: ResourceKind, _query: &BrowseQuery) -> ResourcePage {
        ResourcePage::default()
    }

    fn read(&self, _kind: ResourceKind, _id: u64) -> Option<ResourceView> {
        None
    }

    fn read_media(&self, _id: MediaId) -> Option<MediaRef> {
        None
    }

    fn read_asset(&self, _id: AssetId) -> Option<AssetView> {
        None
    }

    fn attachments(&self, _block: BlockId) -> Vec<AttachmentRef> {
        Vec::new()
    }

    fn search_sites(&self, _query: &SiteQuery) -> SiteSearch {
        SiteSearch::default()
    }
}

/// Which theme view to render.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View<'a> {
    /// `view/omeka/site/index/index.phtml`: a site with no pages.
    Index,
    /// `view/omeka/site/page/show.phtml`: a page's blocks.
    Page,
    /// `view/omeka/site/item/browse.phtml` and its item-set and page siblings.
    Browse(&'a BrowseView),
    /// `view/omeka/site/item/show.phtml`.
    Item(&'a ResourceView),
    /// `view/omeka/site/item-set/browse.phtml` with a single item set.
    ItemSet(&'a ResourceView),
    /// `view/omeka/site/media/show.phtml`.
    Media(&'a ResourceView),
    /// `view/omeka/site/index/search.phtml`: the combined site search results.
    Search(&'a SearchResults),
    /// `view/omeka/site/cross-site-search/*.phtml`: one cross-site search page.
    CrossSiteSearch(&'a CrossSitePage),
    /// A single-resource result list with pagination:
    /// `view/omeka/site/page/browse.phtml`.
    SearchList(&'a SearchList),
    /// `view/omeka/site/item/search.phtml` and its siblings:
    /// `common/advanced-search.phtml`.
    AdvancedSearch(&'a AdvancedSearchForm),
    /// `view/error/404.phtml` and `view/error/403.phtml`, the two error pages
    /// `Mvc\ExceptionListener` renders inside the site layout.
    Error(&'a ErrorView),
}

/// An error page, rendered inside the site layout as
/// `Mvc\ExceptionListener::modifyResponse` renders `error/403` and `error/404`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorView {
    /// The heading `pageTitle(...)` writes, e.g. `Page not found`.
    pub title: String,
    /// The body paragraph, `None` for an error page that has none.
    pub message: Option<String>,
}

impl ErrorView {
    /// `view/error/404.phtml`, in the catalogue's locale.
    #[must_use]
    pub fn not_found(messages: &Messages) -> Self {
        Self {
            title: messages.page_not_found.to_string(),
            message: Some(messages.page_not_found_message.to_string()),
        }
    }

    /// `view/error/403.phtml`, which prints only the heading unless
    /// `display_errors` is on.
    #[must_use]
    pub fn forbidden(messages: &Messages) -> Self {
        Self {
            title: messages.forbidden.to_string(),
            message: None,
        }
    }
}

/// The data a browse view shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowseView {
    /// The resource kind being browsed.
    pub kind: ResourceKind,
    /// The heading, `pageTitle(...)`.
    pub title: String,
    /// The resources on this page.
    pub resources: Vec<ResourceView>,
    /// The total number of matching resources.
    pub total: usize,
    /// The query string pairs, as `$this->params()->fromQuery()` returns.
    pub query: Vec<(String, String)>,
    /// The one-based page number.
    pub page: usize,
    /// `per_page`.
    pub per_page: usize,
}

impl BrowseView {
    /// A browse view for a kind and a resource list.
    #[must_use]
    pub fn new(kind: ResourceKind, title: impl Into<String>, resources: Vec<ResourceView>) -> Self {
        let total = resources.len();
        Self {
            kind,
            title: title.into(),
            resources,
            total,
            query: Vec::new(),
            page: 1,
            per_page: 0,
        }
    }
}

/// Whether a render stamps `data-romeka-block` on block wrappers. Off on every
/// public render; only the admin preview turns it on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BlockMarkers {
    #[default]
    Off,
    Preview,
}

/// The signed-in caller of a public render: who the user bar names, and the
/// roles that decide which edit links it offers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Viewer {
    /// `user.name`.
    pub name: String,
    /// The caller's id, global role and permission on the rendered site.
    pub roles: RoleSet,
}

/// Everything the site layer reads besides the site and page rows.
pub struct RenderContext<'a> {
    /// The site being rendered (`site` row).
    pub site: &'a Site,
    /// The current page, `None` on a browse view.
    pub page: Option<&'a SitePage>,
    /// `site_page.created`, which core leaves to wave 2, `None` when unknown.
    pub page_created: Option<Timestamp>,
    /// `site_page.modified`, `None` when unknown.
    pub page_modified: Option<Timestamp>,
    /// The site settings the templates read.
    pub settings: SiteSettings,
    /// The bundled theme's design tokens, validated at the storage boundary.
    pub theme_settings: ThemeSettings,
    /// The resolved site navigation, for menus and the table of contents.
    pub nav: Vec<NavItem>,
    /// The site's pages, for navigation resolution.
    pub pages: Vec<PageRef>,
    /// The site's stored `resource_page_blocks` configuration, `None` when the
    /// site stores none and the theme's default applies
    /// (`Site/ResourcePageBlockLayout/Manager.php::getResourcePageBlocks`).
    pub resource_page_blocks: Option<ResourcePageBlocks>,
    /// The current query string pairs (`$this->params()->fromQuery()`).
    pub query: Vec<(String, String)>,
    /// Where theme assets live, e.g. `/application/asset`. Default `/assets`.
    pub asset_base: String,
    /// The IIIF viewer route (`routes.config.php:370-379`), absolute when the
    /// caller knows the host. Default `/iiif-viewer`.
    pub iiif_viewer_url: String,
    /// The request path, for pagination and canonical links. Default empty.
    pub current_path: String,
    /// The favicon URL the layout links as `rel="icon"`, resolved from the
    /// `favicon` setting (`Module.php:188-203`). `None` when the setting is
    /// unset or names an asset the caller cannot resolve.
    pub favicon_url: Option<String>,
    /// The signed-in caller, `None` when nobody is signed in
    /// (`View/Helper/UserBar.php:35-43`).
    pub viewer: Option<Viewer>,
    /// Whether the shown resource is not in this site, so the document carries
    /// `<meta name="robots" content="noindex">` (`Module.php:925-931`).
    pub noindex: bool,
    /// The resource lookup.
    pub resources: &'a dyn ResourceProvider,
    /// Crate-private, so another crate can turn markers on only through
    /// [`RenderContext::with_preview_markers`], which clippy confines to the
    /// preview route.
    pub(crate) block_markers: BlockMarkers,
}

impl<'a> RenderContext<'a> {
    /// A context with no resources, no navigation and default settings.
    #[must_use]
    pub fn new(site: &'a Site, resources: &'a dyn ResourceProvider) -> Self {
        Self {
            site,
            page: None,
            page_created: None,
            page_modified: None,
            settings: SiteSettings::default(),
            theme_settings: ThemeSettings::default(),
            nav: Vec::new(),
            pages: Vec::new(),
            resource_page_blocks: None,
            query: Vec::new(),
            asset_base: "/assets".to_string(),
            iiif_viewer_url: "/iiif-viewer".to_string(),
            current_path: String::new(),
            favicon_url: None,
            viewer: None,
            noindex: false,
            resources,
            block_markers: BlockMarkers::Off,
        }
    }

    /// The same context, with block markers on. Disallowed by clippy outside
    /// the preview route (`clippy.toml`).
    #[must_use]
    pub fn with_preview_markers(mut self) -> Self {
        self.block_markers = BlockMarkers::Preview;
        self
    }

    /// Whether this render is the admin preview: a block that needs a script
    /// shows what it will be instead, and the page may point the curator at
    /// the admin.
    #[must_use]
    pub(crate) fn is_preview(&self) -> bool {
        self.block_markers == BlockMarkers::Preview
    }

    /// The marker value for one block, `None` when markers are off.
    #[must_use]
    pub(crate) fn block_marker(&self, id: BlockId) -> Option<BlockId> {
        match self.block_markers {
            BlockMarkers::Off => None,
            BlockMarkers::Preview => Some(id),
        }
    }

    /// The same context for a page render.
    #[must_use]
    pub fn with_page(mut self, page: &'a SitePage) -> Self {
        self.page = Some(page);
        self
    }

    /// The locale and message catalogue for this render, from the site's
    /// `locale` setting. Every label the theme and the blocks print goes
    /// through it.
    #[must_use]
    pub fn translator(&self) -> Translator {
        Translator::new(self.settings.locale())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `NoResources` answers empty for every lookup.
    #[test]
    fn no_resources_provider_is_empty() {
        let provider = NoResources;
        let query = BrowseQuery {
            query: Vec::new(),
            site: SiteId::new(1),
            limit: None,
            sort: None,
        };
        assert_eq!(provider.search(ResourceKind::Item, &query).total, 0);
        assert_eq!(provider.read(ResourceKind::Item, 1), None);
        assert_eq!(provider.read_asset(AssetId::new(1)), None);
        assert!(provider.attachments(BlockId::new(1)).is_empty());
        assert!(
            provider
                .search_sites(&SiteQuery {
                    sort: SiteSort::Alpha,
                    limit: None,
                    exclude: None,
                    page: None
                })
                .sites
                .is_empty()
        );
    }

    /// A browse view's total counts the resources it holds.
    #[test]
    fn browse_view_counts_resources() {
        let resources = vec![
            ResourceView::new(
                ResourceId::new(1),
                ResourceKind::Item,
                "/s/main/item/1",
                "One",
            ),
            ResourceView::new(
                ResourceId::new(2),
                ResourceKind::Item,
                "/s/main/item/2",
                "Two",
            ),
        ];
        let view = BrowseView::new(ResourceKind::Item, "Items", resources);
        assert_eq!(view.total, 2);
        assert_eq!(view.page, 1);
    }
}
