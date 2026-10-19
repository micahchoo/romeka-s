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
//!
//! [`render_page_blocks`] reproduces `View/Helper/PageLayout::render`: it reads
//! the page's `layout` and `layout_data`, and each block's `layout_data`, and
//! adds the wrapper classes and inline styles [`layout`] derives from them.

pub mod parse;
pub mod schema;

#[cfg(test)]
pub(crate) mod test_support;

pub mod asset;
pub mod block_group;
pub mod browse_preview;
pub mod canonical;
pub mod fallback;
pub mod hero;
pub mod html;
pub mod iiif_image;
pub mod iiif_presentation;
pub mod item_with_metadata;
pub mod layout;
pub mod line_break;
pub mod list_of_pages;
pub mod list_of_sites;
pub mod media;
pub mod oembed;
pub mod page_date_time;
pub mod page_title;
pub mod romeka;
pub mod table_of_contents;
pub mod templates;

use omeka_core::site::{BlockData, BlockLayout as LayoutId, SiteBlock, SitePage};

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

    /// The `data` fields this layout edits, declared beside its renderer from
    /// the layout's upstream form. [`schema::registry`] carries them to the
    /// admin, which generates its block forms from them.
    ///
    /// The default is [`schema::EMPTY`]: a layout that lives outside this
    /// crate (a plugin's) has no schema the admin can render. Every registered
    /// layout declares its own, and a test fails if one leaves it empty.
    fn schema(&self) -> &'static schema::LayoutSchema {
        &schema::EMPTY
    }
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
/// Each block is wrapped the way `BlockLayout::render` wraps it, with the
/// classes and inline styles `AbstractLayout` derives from the block's layout
/// data, and the page element carries the page layout's own classes. The page
/// layout data and every block's layout data come from the page.
#[must_use]
pub fn render_page_blocks(page: &SitePage, ctx: &RenderContext<'_>) -> String {
    let mut ordered: Vec<&SiteBlock> = page.blocks.iter().collect();
    ordered.sort_by_key(|block| block.position);

    let columns = page.layout_data.grid_columns.unwrap_or(0);
    let mut out = layout::page_open(page.layout, &page.layout_data);
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
            out.push_str(&layout::group_open(block, page.layout, columns, ctx));
        } else {
            let classes = layout::block_classes(block, page.layout, columns);
            let styles = layout::block_inline_styles(block, ctx);
            let body = render_block(block, ctx);
            out.push_str(&layout::block_wrapper(
                &classes,
                &styles,
                ctx.block_marker(block.id),
                &body,
            ));
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
    use crate::blocks::test_support::{block, block_with_layout, context, page, site};
    use omeka_core::site::{BlockLayoutData, PageLayout};
    use serde_json::json;

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

    /// `BlockLayout::render` wraps every block in `block block-{layout}` with the
    /// inline styles its layout data names, and `PageLayout::render` wraps them
    /// in `blocks-inner page-layout-normal`.
    #[test]
    fn normal_page_wraps_each_block_like_block_layout_render() {
        let site = site();
        let ctx = context(&site, &crate::view::NoResources);
        let mut first = block("html", json!({"html": "<p>One</p>"}));
        first.position = 1;
        let mut group = block("blockGroup", json!({"span": "1"}));
        group.position = 2;
        let mut grouped = block("html", json!({"html": "<p>Two</p>"}));
        grouped.position = 3;
        let mut last = block("html", json!({"html": "<p>Three</p>"}));
        last.position = 4;
        let page = page(
            PageLayout::Normal,
            json!({}),
            vec![group, last, first, grouped],
        );
        assert_eq!(
            render_page_blocks(&page, &ctx),
            "<div class=\"blocks-inner page-layout-normal\" style=\"\">\
             <div class=\"block block-html\" style=\"\"><p>One</p></div>\
             <div class=\"block block-blockGroup\" style=\"\">\
             <div class=\"block block-html\" style=\"\"><p>Two</p></div>\
             </div>\
             <div class=\"block block-html\" style=\"\"><p>Three</p></div>\
             </div>"
        );
    }

    /// A group whose span outruns the page's blocks is closed at the end, as
    /// `PageLayout::render` closes an unterminated group.
    #[test]
    fn page_layout_closes_an_unterminated_group() {
        let site = site();
        let ctx = context(&site, &crate::view::NoResources);
        let mut group = block("blockGroup", json!({"span": "5"}));
        group.position = 1;
        let mut trailing = block("html", json!({"html": "<p>Two</p>"}));
        trailing.position = 2;
        let page = page(PageLayout::Normal, json!({}), vec![group, trailing]);
        assert_eq!(
            render_page_blocks(&page, &ctx),
            "<div class=\"blocks-inner page-layout-normal\" style=\"\">\
             <div class=\"block block-blockGroup\" style=\"\">\
             <div class=\"block block-html\" style=\"\"><p>Two</p></div>\
             </div></div>"
        );
    }

    /// A group with `span` 0 wraps nothing, and a second group closes the first
    /// rather than nesting it, as `PageLayout::render` does.
    #[test]
    fn page_layout_groups_do_not_overlap() {
        let site = site();
        let ctx = context(&site, &crate::view::NoResources);
        let mut first = block("blockGroup", json!({"span": "0"}));
        first.position = 1;
        let mut second = block("blockGroup", json!({"span": "1"}));
        second.position = 2;
        let mut body = block("html", json!({"html": "<p>Body</p>"}));
        body.position = 3;
        let page = page(PageLayout::Normal, json!({}), vec![first, second, body]);
        assert_eq!(
            render_page_blocks(&page, &ctx),
            "<div class=\"blocks-inner page-layout-normal\" style=\"\">\
             <div class=\"block block-blockGroup\" style=\"\"></div>\
             <div class=\"block block-blockGroup\" style=\"\">\
             <div class=\"block block-html\" style=\"\"><p>Body</p></div>\
             </div></div>"
        );
    }

    /// The measured upstream output for a grid page: the page element carries
    /// `page-layout-grid grid-template-columns-12` and the pixel gaps, and each
    /// block carries its grid placement, class, background and padding.
    #[test]
    fn grid_page_renders_upstream_page_and_block_markup() {
        let site = site();
        let ctx = context(&site, &crate::view::NoResources);
        let mut hero = block_with_layout(
            "html",
            json!({"html": "<p>Hero</p>"}),
            json!({
                "class": "r-hero",
                "grid_column_span": "6",
                "background_color": "#ffeeee",
                "padding_top": "1em"
            }),
        );
        hero.position = 1;
        let mut group = block_with_layout(
            "blockGroup",
            json!({"span": "1"}),
            json!({"padding_left": "8px"}),
        );
        group.position = 2;
        let mut inner = block("html", json!({"html": "<p>Inside</p>"}));
        inner.position = 3;
        let page = page(
            PageLayout::Grid,
            json!({"grid_columns": 12, "grid_column_gap": 10, "grid_row_gap": 10}),
            vec![hero, group, inner],
        );
        assert_eq!(
            render_page_blocks(&page, &ctx),
            "<div class=\"blocks-inner page-layout-grid grid-template-columns-12\" \
             style=\"column-gap: 10px; row-gap: 10px;\">\
             <div class=\"block block-html grid-position-auto grid-span-6 r-hero has-background\" \
             style=\"background-color: #ffeeee; padding-top: 1em\"><p>Hero</p></div>\
             <div class=\"block block-blockGroup block-group-grid grid-position-1 grid-span-12\" \
             style=\"padding-left: 8px\">\
             <div class=\"block block-html grid-position-auto grid-span-12\" style=\"\">\
             <p>Inside</p></div>\
             </div></div>"
        );
    }

    /// `AbstractLayout::getBlockClasses`: a stored position and span become the
    /// classes when they name a valid column, and the grid's column count is the
    /// fallback for both, as `in_array` decides upstream.
    #[test]
    fn block_classes_match_abstract_layout() {
        let site = site();
        let ctx = context(&site, &crate::view::NoResources);
        let mut placed = block_with_layout(
            "html",
            json!({"html": "<p>One</p>"}),
            json!({"grid_column_position": "3", "grid_column_span": "20"}),
        );
        placed.position = 1;
        let mut out_of_range = block_with_layout(
            "html",
            json!({"html": "<p>Two</p>"}),
            json!({"grid_column_position": "99", "grid_column_span": "0"}),
        );
        out_of_range.position = 2;
        let mut aligned = block_with_layout(
            "html",
            json!({"html": "<p>Three</p>"}),
            json!({
                "alignment_block": "center",
                "alignment_text": "justify",
                "background_color": "#fff",
                "background_image_asset": "4",
                "background_image_position_y": "bottom",
                "background_image_position_x": "right",
                "background_image_size": "cover"
            }),
        );
        aligned.position = 3;
        let page = page(
            PageLayout::Grid,
            json!({"grid_columns": 6}),
            vec![placed, out_of_range, aligned],
        );
        let rendered = render_page_blocks(&page, &ctx);
        assert!(
            rendered.contains(
                "<div class=\"block block-html grid-position-3 grid-span-6\" style=\"\">"
            ),
            "{rendered}"
        );
        assert!(
            rendered.contains(
                "<div class=\"block block-html grid-position-auto grid-span-6\" style=\"\">"
            ),
            "{rendered}"
        );
        assert!(
            rendered.contains(
                "class=\"block block-html grid-position-auto grid-span-6 \
                 block-layout-alignment-block-center block-layout-alignment-text-justify \
                 has-background block-layout-background-image-position-y-bottom \
                 block-layout-background-image-position-x-right \
                 block-layout-background-image-size-cover\""
            ),
            "{rendered}"
        );
        // The grid's own gaps have their upstream defaults when unset.
        assert!(
            rendered.starts_with(
                "<div class=\"blocks-inner page-layout-grid grid-template-columns-6\" \
                 style=\"column-gap: 10px; row-gap: 10px;\">"
            ),
            "{rendered}"
        );
    }

    /// `AbstractLayout::getBlockInlineStyles`: a bare number is pixels, a valid
    /// length passes through, an invalid one is dropped, and an alignment class
    /// never appears for a value upstream does not know.
    #[test]
    fn block_inline_styles_match_abstract_layout() {
        let site = site();
        let ctx = context(&site, &crate::view::NoResources);
        let mut styled = block_with_layout(
            "html",
            json!({"html": "<p>One</p>"}),
            json!({
                "max_width": "40rem",
                "min_height": "200",
                "padding_top": "abc",
                "padding_right": "1.5em",
                "alignment_block": "sideways"
            }),
        );
        styled.position = 1;
        // A background colour that is not a hex colour is still truthy, so the
        // class is there, but no `background-color` style is.
        let mut coloured = block_with_layout(
            "html",
            json!({"html": "<p>Two</p>"}),
            json!({"background_color": "red"}),
        );
        coloured.position = 2;
        let page = page(PageLayout::Normal, json!({}), vec![styled, coloured]);
        let rendered = render_page_blocks(&page, &ctx);
        assert!(
            rendered.contains(
                "<div class=\"block block-html\" \
                 style=\"max-width: 40rem; min-height: 200px; padding-right: 1.5em\">\
                 <p>One</p></div>"
            ),
            "{rendered}"
        );
        assert!(!rendered.contains("background-color:"), "{rendered}");
        assert!(!rendered.contains("alignment-block"), "{rendered}");
        assert!(!rendered.contains("padding-top"), "{rendered}");
        assert!(
            rendered.contains("<div class=\"block block-html has-background\" style=\"\">"),
            "{rendered}"
        );

        // A number on a text member reads as absent, as `is_string` ignores it.
        let numeric = block_with_layout(
            "html",
            json!({"html": "<p>x</p>"}),
            json!({"min_height": 200}),
        );
        assert_eq!(numeric.layout_data.min_height, None);
    }

    /// A block's own class and the classes derived from its layout data are
    /// escaped, as `BlockLayout::render` escapes the whole class attribute.
    #[test]
    fn block_class_attribute_is_escaped() {
        let site = site();
        let ctx = context(&site, &crate::view::NoResources);
        let mut hostile = block_with_layout(
            "html",
            json!({"html": "<p>One</p>"}),
            json!({"class": "a\"b<c"}),
        );
        hostile.position = 1;
        let page = page(PageLayout::Normal, json!({}), vec![hostile]);
        let rendered = render_page_blocks(&page, &ctx);
        assert!(
            rendered.contains("class=\"block block-html a&quot;b&lt;c\""),
            "{rendered}"
        );
        assert_eq!(
            BlockLayoutData::parse(&json!({})),
            BlockLayoutData::default()
        );
    }
}
