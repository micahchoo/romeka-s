//! Test-only builders: a block with stored data, a site, and a resource
//! provider a test fills in.

use std::collections::BTreeMap;

use omeka_core::ids::{AssetId, BlockId, MediaId, PageId, SiteId};
use omeka_core::resource::{ResourceKind, Timestamp};
use omeka_core::site::{
    BlockLayoutData, PageLayout, PageLayoutData, Site, SiteBlock, SitePage, Slug,
};
use serde_json::Value as JsonValue;

use crate::view::{
    AssetView, AttachmentRef, BrowseQuery, MediaRef, RenderContext, ResourcePage, ResourceProvider,
    ResourceRef, ResourceView, SiteQuery, SiteSearch, SiteView,
};

/// A site named "Main Site" at slug `main`.
pub(crate) fn site() -> Site {
    Site::new(
        SiteId::new(1),
        Slug::parse("main").unwrap(),
        "default",
        "Main Site",
    )
}

/// A stored block: layout name plus `data` JSON parsed through the boundary
/// parser, so a test exercises the same validation a caller gets. The layout
/// data is empty.
pub(crate) fn block(layout: &str, data: JsonValue) -> SiteBlock {
    let mut block = block_with_layout(layout, data, JsonValue::Null);
    block.layout_data = BlockLayoutData::default();
    block
}

/// A stored block with its `layout_data`, parsed the way the loader parses it.
pub(crate) fn block_with_layout(
    layout: &str,
    data: JsonValue,
    layout_data: JsonValue,
) -> SiteBlock {
    let layout_id = omeka_core::site::BlockLayout::from_wire_name(layout);
    let data = crate::blocks::parse::parse_block_data(&layout_id, &data).unwrap();
    SiteBlock {
        id: BlockId::new(1),
        layout: layout_id,
        position: 1,
        data,
        layout_data: BlockLayoutData::parse(&layout_data),
    }
}

/// A page whose blocks are given, in the order supplied, with the stored page
/// layout and layout data.
pub(crate) fn page(layout: PageLayout, layout_data: JsonValue, blocks: Vec<SiteBlock>) -> SitePage {
    let mut page = SitePage::new(
        PageId::new(2),
        SiteId::new(1),
        Slug::parse("welcome").unwrap(),
        "Welcome",
    );
    page.layout = layout;
    page.layout_data = PageLayoutData::parse(&layout_data);
    page.blocks = blocks;
    page
}

/// A timestamp for a fixed instant, `2026-10-16 09:30:00`.
pub(crate) fn timestamp() -> Timestamp {
    Timestamp::from_naive(
        chrono::NaiveDateTime::parse_from_str("2026-10-16 09:30:00", "%Y-%m-%d %H:%M:%S").unwrap(),
    )
}

/// A resource view with an item's kind.
pub(crate) fn item(id: u64, title: &str, url: &str) -> ResourceView {
    ResourceView::new(
        omeka_core::ids::ResourceId::new(id),
        ResourceKind::Item,
        url,
        title,
    )
}

/// A resource provider a test fills in per lookup.
#[derive(Default)]
pub(crate) struct TestResources {
    /// `attachments(block)` results, keyed by block id.
    pub attachments: BTreeMap<u64, Vec<AttachmentRef>>,
    /// `read(kind, id)` results, keyed by kind and id.
    pub items: BTreeMap<u64, ResourceView>,
    /// `search(kind, query)` results.
    pub browse: Vec<ResourceView>,
    /// `read_media(id)` results.
    pub media: BTreeMap<u64, MediaRef>,
    /// `read_asset(id)` results.
    pub assets: BTreeMap<u64, AssetView>,
    /// `read_image(id)` results.
    pub images: BTreeMap<u64, crate::view::Thumbnail>,
    /// `search_sites(query)` results.
    pub sites: Vec<SiteView>,
}

impl ResourceProvider for TestResources {
    fn search(&self, _kind: ResourceKind, query: &BrowseQuery) -> ResourcePage {
        let limit = query.limit.unwrap_or(self.browse.len());
        ResourcePage {
            resources: self.browse.iter().take(limit).cloned().collect(),
            total: self.browse.len(),
        }
    }

    fn read(&self, _kind: ResourceKind, id: u64) -> Option<ResourceView> {
        self.items.get(&id).cloned()
    }

    fn read_media(&self, id: MediaId) -> Option<MediaRef> {
        self.media.get(&id.get()).cloned()
    }

    fn read_asset(&self, id: AssetId) -> Option<AssetView> {
        self.assets.get(&id.get()).cloned()
    }

    fn read_image(&self, id: MediaId) -> Option<crate::view::Thumbnail> {
        self.images.get(&id.get()).cloned()
    }

    fn attachments(&self, block: BlockId) -> Vec<AttachmentRef> {
        self.attachments
            .get(&block.get())
            .cloned()
            .unwrap_or_default()
    }

    fn search_sites(&self, query: &SiteQuery) -> SiteSearch {
        let mut sites = self.sites.clone();
        if let Some(exclude) = query.exclude {
            sites.retain(|site| site.id != exclude);
        }
        let total = sites.len();
        if let Some(limit) = query.limit {
            sites.truncate(limit);
        }
        SiteSearch { sites, total }
    }
}

/// A resource ref helper.
pub(crate) fn resource_ref(title: &str, url: &str) -> ResourceRef {
    ResourceRef {
        url: url.to_string(),
        title: title.to_string(),
        thumbnail: None,
    }
}

/// A literal value with its plain text and markup the same, the common case in
/// the theme tests.
pub(crate) fn literal_value(property_id: u64, property: &str, text: &str) -> crate::view::ValueRef {
    crate::view::ValueRef {
        property_id,
        property: property.to_string(),
        property_term: format!("dcterms:{}", property.to_lowercase()),
        vocabulary: "Dublin Core".to_string(),
        value: text.to_string(),
        html: text.to_string(),
        lang: None,
        class: "literal".to_string(),
    }
}

/// A media ref helper without an embed.
pub(crate) fn media_ref(title: &str, url: &str) -> MediaRef {
    MediaRef {
        url: url.to_string(),
        original_url: None,
        title: title.to_string(),
        media_type: None,
        thumbnail: None,
        embed: None,
    }
}

/// A context over a site and provider with default settings.
pub(crate) fn context<'a>(site: &'a Site, provider: &'a dyn ResourceProvider) -> RenderContext<'a> {
    RenderContext::new(site, provider)
}
