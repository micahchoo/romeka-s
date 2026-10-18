//! The block layouts: one module per registered layout in
//! `application/config/module.config.php:805-825`, ported from the class in
//! `src/Site/BlockLayout/*.php` and its view script in
//! `view/common/block-layout/*.phtml`.
//!
//! The registry holds the fifteen registered names plus the `Fallback` layout
//! that the manager substitutes for an unregistered name. `BlockGroup` is
//! registered upstream and renders nothing itself; its children are blocks of
//! the same page and carry the group's span (`BlockLayout/BlockGroup.php`).
//!
//! [`parse`] validates a block's `data` JSON at the storage boundary and names
//! the failing path and the kind it expected.

pub mod parse;

#[cfg(test)]
pub(crate) mod test_support;

pub mod asset;
pub mod block_group;
pub mod browse_preview;
pub mod fallback;
pub mod html;
pub mod iiif_image;
pub mod iiif_presentation;
pub mod item_with_metadata;
pub mod line_break;
pub mod list_of_pages;
pub mod list_of_sites;
pub mod media;
pub mod oembed;
pub mod page_date_time;
pub mod page_title;
pub mod table_of_contents;

use omeka_core::site::{BlockData, BlockLayout as LayoutId, SiteBlock};

use crate::view::RenderContext;

pub use asset::Asset;
pub use block_group::BlockGroup;
pub use browse_preview::BrowsePreview;
pub use fallback::Fallback;
pub use html::Html;
pub use iiif_image::IiifImage;
pub use iiif_presentation::IiifPresentation;
pub use item_with_metadata::ItemWithMetadata;
pub use line_break::LineBreak;
pub use list_of_pages::ListOfPages;
pub use list_of_sites::ListOfSites;
pub use media::Media;
pub use oembed::Oembed;
pub use page_date_time::PageDateTime;
pub use page_title::PageTitle;
pub use table_of_contents::TableOfContents;

/// One block layout: the upstream class's name, label and `render`.
///
/// `Sync` lets the registry live in a `static`.
pub trait BlockLayout: Sync {
    /// `site_page_block.layout`, the registered name.
    fn name(&self) -> &'static str;

    /// `getLabel`, the name the admin shows.
    fn label(&self) -> &'static str;

    /// Render the block to an HTML fragment (`render`).
    fn render(&self, block: &SiteBlock, ctx: &RenderContext<'_>) -> String;
}

/// The registry, in the order `module.config.php` lists (`invokables` then
/// `factories`), with `Fallback` last.
pub static LAYOUTS: [&dyn BlockLayout; 16] = [
    &PageTitle,
    &Media,
    &BrowsePreview,
    &ListOfSites,
    &TableOfContents,
    &LineBreak,
    &ItemWithMetadata,
    &PageDateTime,
    &BlockGroup,
    &IiifPresentation,
    &IiifImage,
    &Asset,
    &Html,
    &ListOfPages,
    &Oembed,
    &Fallback,
];

/// Every registered layout plus the fallback.
#[must_use]
pub fn layouts() -> &'static [&'static dyn BlockLayout] {
    &LAYOUTS
}

/// The layout for a registered name, or [`Fallback`] when the name is not
/// registered, exactly as `Site\BlockLayout\Manager::get` does.
#[must_use]
pub fn layout_for(name: &str) -> &'static dyn BlockLayout {
    LAYOUTS
        .iter()
        .copied()
        .find(|layout| layout.name() == name)
        .unwrap_or(&Fallback)
}

/// The layout for a stored block, by its `layout`.
#[must_use]
pub fn layout_for_block(block: &SiteBlock) -> &'static dyn BlockLayout {
    match block.layout.wire_name() {
        Some(name) => layout_for(name),
        None => &Fallback,
    }
}

/// Render one block through its layout.
#[must_use]
pub fn render_block(block: &SiteBlock, ctx: &RenderContext<'_>) -> String {
    layout_for_block(block).render(block, ctx)
}

/// `PageLayout::render`: a page's blocks in `position` order inside the
/// `blocks-inner` element, with a `blockGroup` opening a wrapper around the
/// blocks that follow it, for the number of blocks its `span` names.
///
/// Upstream takes the wrapper's classes and inline styles from the block's
/// layout data (`class`, alignment, background, padding, grid placement). Core
/// carries no layout data, so a group wrapper is `block block-{layout}`, its
/// `style` is empty, and the page layout is the default `normal` (`site_page`
/// has no `layout` column either).
#[must_use]
pub fn render_page_blocks(blocks: &[SiteBlock], ctx: &RenderContext<'_>) -> String {
    let mut ordered: Vec<&SiteBlock> = blocks.iter().collect();
    ordered.sort_by_key(|block| block.position);

    let mut out = String::from("<div class=\"blocks-inner page-layout-normal\" style=\"\">");
    let mut in_group = false;
    let mut span = 0_i64;
    let mut counted = 0_i64;
    for block in ordered {
        if block.layout == LayoutId::BlockGroup {
            if in_group {
                // Blocks may not overlap, so a group inside a group closes the
                // open one first.
                out.push_str("</div>");
            }
            in_group = true;
            span = group_span(block);
            counted = 0;
            let class = match block.layout.wire_name() {
                Some(name) => format!("block block-{name}"),
                None => "block".to_string(),
            };
            out.push_str(&format!("<div class=\"{class}\" style=\"\">"));
        } else {
            out.push_str(&render_block(block, ctx));
        }
        if in_group {
            if counted == span {
                out.push_str("</div>");
                in_group = false;
            } else {
                counted += 1;
            }
        }
    }
    if in_group {
        out.push_str("</div>");
    }
    out.push_str("</div>");
    out
}

/// `(int) $block->dataValue('span')`: a missing or non-numeric span is 0.
fn group_span(block: &SiteBlock) -> i64 {
    match &block.data {
        BlockData::BlockGroup { span } => span
            .as_deref()
            .and_then(|span| span.trim().parse::<i64>().ok())
            .unwrap_or(0),
        _ => 0,
    }
}

/// The core layout id for a registry entry, or `None` for [`Fallback`].
#[must_use]
pub fn layout_id(name: &str) -> Option<LayoutId> {
    match LayoutId::from_wire_name(name) {
        LayoutId::Fallback(_) => None,
        id => Some(id),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `module.config.php:805-825`: the registry holds every registered id, in
    /// upstream's order, plus the fallback. Each label is core's label, which
    /// is the class's `getLabel`.
    #[test]
    fn registry_matches_upstream_registration() {
        let names: Vec<&str> = layouts().iter().map(|layout| layout.name()).collect();
        assert_eq!(
            &names[..15],
            [
                "pageTitle",
                "media",
                "browsePreview",
                "listOfSites",
                "tableOfContents",
                "lineBreak",
                "itemWithMetadata",
                "pageDateTime",
                "blockGroup",
                "iiifPresentation",
                "iiifImage",
                "asset",
                "html",
                "listOfPages",
                "oembed",
            ]
        );
        assert_eq!(names[15], "fallback");
        for layout in layouts() {
            if layout.name() == "fallback" {
                continue;
            }
            let id = layout_id(layout.name()).expect("registered name maps to a core id");
            assert_eq!(layout.label(), id.label(), "{} label", layout.name());
        }
    }

    /// An unregistered name renders through the fallback, which emits nothing,
    /// as `BlockLayout/Fallback.php::render` returns an empty string.
    #[test]
    fn unregistered_layout_uses_fallback() {
        assert_eq!(layout_for("myModuleLayout").name(), "fallback");
    }

    /// `PageLayout::render`: the blocks sit inside `blocks-inner`, and a
    /// `blockGroup` with `span` 1 wraps the one block that follows it.
    #[test]
    fn page_layout_matches_upstream_page_layout_helper() {
        let site = test_support::site();
        let ctx = test_support::context(&site, &crate::view::NoResources);
        let mut first = test_support::block("html", serde_json::json!({"html": "<p>One</p>"}));
        first.position = 1;
        let mut group = test_support::block("blockGroup", serde_json::json!({"span": "1"}));
        group.position = 2;
        let mut grouped = test_support::block("html", serde_json::json!({"html": "<p>Two</p>"}));
        grouped.position = 3;
        let mut last = test_support::block("html", serde_json::json!({"html": "<p>Three</p>"}));
        last.position = 4;
        assert_eq!(
            render_page_blocks(&[group, last, first, grouped], &ctx),
            "<div class=\"blocks-inner page-layout-normal\" style=\"\"><p>One</p><div class=\"block block-blockGroup\" style=\"\"><p>Two</p></div><p>Three</p></div>"
        );
    }

    /// A group whose span outruns the page's blocks is closed at the end, as
    /// `PageLayout::render` closes an unterminated group.
    #[test]
    fn page_layout_closes_an_unterminated_group() {
        let site = test_support::site();
        let ctx = test_support::context(&site, &crate::view::NoResources);
        let mut group = test_support::block("blockGroup", serde_json::json!({"span": "5"}));
        group.position = 1;
        let mut trailing = test_support::block("html", serde_json::json!({"html": "<p>Two</p>"}));
        trailing.position = 2;
        assert_eq!(
            render_page_blocks(&[group, trailing], &ctx),
            "<div class=\"blocks-inner page-layout-normal\" style=\"\"><div class=\"block block-blockGroup\" style=\"\"><p>Two</p></div></div>"
        );
    }

    /// A group with `span` 0 wraps nothing, and a second group closes the first
    /// rather than nesting it, as `PageLayout::render` does.
    #[test]
    fn page_layout_groups_do_not_overlap() {
        let site = test_support::site();
        let ctx = test_support::context(&site, &crate::view::NoResources);
        let mut first = test_support::block("blockGroup", serde_json::json!({"span": "0"}));
        first.position = 1;
        let mut second = test_support::block("blockGroup", serde_json::json!({"span": "1"}));
        second.position = 2;
        let mut body = test_support::block("html", serde_json::json!({"html": "<p>Body</p>"}));
        body.position = 3;
        assert_eq!(
            render_page_blocks(&[first, second, body], &ctx),
            "<div class=\"blocks-inner page-layout-normal\" style=\"\"><div class=\"block block-blockGroup\" style=\"\"></div><div class=\"block block-blockGroup\" style=\"\"><p>Body</p></div></div>"
        );
    }
}
