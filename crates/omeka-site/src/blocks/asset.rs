//! `BlockLayout/Asset.php` and `view/common/block-layout/asset.phtml`.
//!
//! Each entry in the stored `data` list is one asset slot with a link target
//! page, an alternative link title and a caption. The class resolves the asset
//! id to an asset and the page id to a page of the current site
//! (`prepareAssetAttachments`); the template wraps the thumbnail in a link when
//! the page resolves, with the page title or the alternative as the link text.
//!
//! Core keeps the stored slots as `AssetAttachment`, which holds ids. The site
//! layer resolves them through [`ResourceProvider::read_asset`] and
//! [`RenderContext::pages`].

use omeka_core::site::{AssetAttachment, BlockData, SiteBlock};

use crate::blocks::BlockLayout;
use crate::blocks::schema::{LayoutSchema, asset_ref, page_ref, repeat, text};
use crate::html::escape_html;
use crate::slug::page_url;
use crate::view::{AssetView, RenderContext, Thumbnail};

/// The `asset` block.
#[derive(Debug, Clone, Copy, Default)]
pub struct Asset;

/// `BlockLayout/Asset.php::form` and `common/asset-block-form.phtml`: a repeat
/// group keyed by slot index, one `id`, `page`, `alt_link_title` and `caption`
/// per attachment.
const SCHEMA: LayoutSchema = LayoutSchema {
    fields: &[repeat(
        "blockField.asset.attachments",
        &[
            asset_ref("id", "blockField.asset.id"),
            page_ref("page", "blockField.asset.page"),
            text("alt_link_title", "blockField.asset.alt_link_title"),
            text("caption", "blockField.asset.caption"),
        ],
    )],
};

impl BlockLayout for Asset {
    fn name(&self) -> &'static str {
        "asset"
    }

    fn label(&self) -> &'static str {
        "Asset"
    }

    fn render(&self, block: &SiteBlock, ctx: &RenderContext<'_>) -> String {
        let BlockData::Asset(attachments) = &block.data else {
            return String::new();
        };
        render_attachments(attachments, ctx)
    }

    fn schema(&self) -> &'static LayoutSchema {
        &SCHEMA
    }
}

fn render_attachments(attachments: &[AssetAttachment], ctx: &RenderContext<'_>) -> String {
    let mut out = String::from("<div class=\"assets\">");
    for attachment in attachments {
        out.push_str("<div class=\"asset\">");
        let image = attachment
            .id
            .and_then(|id| ctx.resources.read_asset(id))
            .as_ref()
            .and_then(thumbnail_markup)
            .unwrap_or_default();
        let page = attachment.page.and_then(|id| {
            ctx.pages
                .iter()
                .find(|page| page.id == id)
                .map(|page| (page.title.as_str(), page_url(&ctx.site.slug, &page.slug)))
        });
        match page {
            Some((page_title, url)) => {
                let link_title = attachment
                    .alt_link_title
                    .as_deref()
                    .filter(|title| !title.is_empty())
                    .unwrap_or(page_title);
                out.push_str(&format!(
                    "<a href=\"{}\">{}<span class=\"link-title\">{}</span></a>",
                    escape_html(&url),
                    image,
                    escape_html(link_title)
                ));
            }
            None => out.push_str(&image),
        }
        if let Some(caption) = attachment.caption.as_deref().filter(|c| !c.is_empty()) {
            out.push_str(&format!(
                "<div class=\"caption\">{}</div>",
                escape_html(caption)
            ));
        }
        out.push_str("</div>");
    }
    out.push_str("</div>");
    out
}

fn thumbnail_markup(asset: &AssetView) -> Option<String> {
    let Thumbnail { url, alt } = asset.thumbnail.clone()?;
    Some(format!(
        "<img src=\"{}\" alt=\"{}\">",
        escape_html(&url),
        escape_html(&alt)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks::test_support::{TestResources, block, context, site};
    use crate::nav::PageRef;
    use crate::view::NoResources;
    use omeka_core::ids::{AssetId, PageId};
    use omeka_core::site::Slug;
    use serde_json::json;

    fn pages() -> Vec<PageRef> {
        vec![PageRef {
            id: PageId::new(7),
            slug: Slug::parse("about").unwrap(),
            title: "About".to_string(),
            is_public: true,
        }]
    }

    /// `block-layout/asset.phtml`: a linked thumbnail with a caption, the page
    /// title inside `link-title`.
    #[test]
    fn asset_matches_upstream_block_layout() {
        let site = site();
        let mut resources = TestResources::default();
        resources.assets.insert(
            4,
            AssetView {
                id: AssetId::new(4),
                title: "Logo".to_string(),
                url: None,
                thumbnail: Some(Thumbnail::new("/files/square/4.jpg", "Logo")),
            },
        );
        let mut ctx = context(&site, &resources);
        ctx.pages = pages();
        let stored = block(
            "asset",
            json!([{"id": 4, "page": 7, "alt_link_title": "", "caption": "A caption"}]),
        );
        assert_eq!(
            Asset.render(&stored, &ctx),
            "<div class=\"assets\"><div class=\"asset\"><a href=\"/s/main/page/about\"><img src=\"/files/square/4.jpg\" alt=\"Logo\"><span class=\"link-title\">About</span></a><div class=\"caption\">A caption</div></div></div>"
        );
    }

    /// An alternative link title replaces the page title, and a slot whose page
    /// is gone renders the image alone. An empty asset id means no image.
    #[test]
    fn asset_uses_alt_link_title_and_skips_missing_pages() {
        let site = site();
        let mut resources = TestResources::default();
        resources.assets.insert(
            4,
            AssetView {
                id: AssetId::new(4),
                title: "Logo".to_string(),
                url: None,
                thumbnail: Some(Thumbnail::new("/files/square/4.jpg", "Logo")),
            },
        );
        let mut ctx = context(&site, &resources);
        ctx.pages = pages();
        let stored = block(
            "asset",
            json!([
                {"id": 4, "page": 7, "alt_link_title": "Read on", "caption": ""},
                {"id": 4, "page": 99, "alt_link_title": "", "caption": ""}
            ]),
        );
        let out = Asset.render(&stored, &ctx);
        assert!(
            out.contains("<a href=\"/s/main/page/about\"><img src=\"/files/square/4.jpg\" alt=\"Logo\"><span class=\"link-title\">Read on</span></a>"),
            "{out}"
        );
        assert_eq!(
            out,
            "<div class=\"assets\"><div class=\"asset\"><a href=\"/s/main/page/about\"><img src=\"/files/square/4.jpg\" alt=\"Logo\"><span class=\"link-title\">Read on</span></a></div><div class=\"asset\"><img src=\"/files/square/4.jpg\" alt=\"Logo\"></div></div>"
        );
    }

    /// A slot with no asset and no page renders an empty entry, as the template
    /// does when `$assetImage` is empty.
    #[test]
    fn asset_without_data_renders_empty_entries() {
        let site = site();
        let ctx = context(&site, &NoResources);
        let stored = block("asset", json!([{"id": "", "page": 99, "caption": ""}]));
        assert_eq!(
            Asset.render(&stored, &ctx),
            "<div class=\"assets\"><div class=\"asset\"></div></div>"
        );
    }
}
