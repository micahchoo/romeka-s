//! Test-only builders: a block with stored data, a site, and a resource
//! provider a test fills in.

use std::collections::BTreeMap;

use omeka_core::ids::{AssetId, BlockId, MediaId, SiteId};
use omeka_core::resource::{ResourceKind, Timestamp};
use omeka_core::site::{Site, SiteBlock, Slug};
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
/// parser, so a test exercises the same validation a caller gets.
pub(crate) fn block(layout: &str, data: JsonValue) -> SiteBlock {
    let layout_id = omeka_core::site::BlockLayout::from_wire_name(layout);
    let data = crate::blocks::parse::parse_block_data(&layout_id, &data).unwrap();
    SiteBlock {
        id: BlockId::new(1),
        layout: layout_id,
        position: 1,
        data,
    }
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

/// A media ref helper without an embed.
pub(crate) fn media_ref(title: &str, url: &str) -> MediaRef {
    MediaRef {
        url: url.to_string(),
        original_url: None,
        title: title.to_string(),
        thumbnail: None,
        embed: None,
    }
}

/// A context over a site and provider with default settings.
pub(crate) fn context<'a>(site: &'a Site, provider: &'a dyn ResourceProvider) -> RenderContext<'a> {
    RenderContext::new(site, provider)
}
