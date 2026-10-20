//! The section layouts and the Romeka-only styling, as `render_page_blocks`
//! writes them on a normal page and on a grid page.

use omeka_core::site::{PageLayout, SiteBlock, SitePage};
use serde_json::{Value as JsonValue, json};

use super::render_page_blocks;
use super::style::SectionLayout;
use crate::blocks::test_support::{block, block_with_layout, context, page, site};

/// A page with blocks at positions 1, 2, ...
fn page_of(layout: PageLayout, layout_data: JsonValue, blocks: Vec<SiteBlock>) -> SitePage {
    let mut blocks = blocks;
    for (index, block) in blocks.iter_mut().enumerate() {
        block.position = index as i64 + 1;
    }
    page(layout, layout_data, blocks)
}

fn render(page: &SitePage) -> String {
    let site = site();
    let ctx = context(&site, &crate::view::NoResources);
    render_page_blocks(page, &ctx)
}

/// A group of `members` blocks whose `o:layout_data.romeka` is `romeka`, then
/// its members and one block after it.
fn section_page(layout: PageLayout, romeka: JsonValue) -> SitePage {
    let columns = match layout {
        PageLayout::Grid => json!({"grid_columns": 12}),
        PageLayout::Normal => json!({}),
    };
    page_of(
        layout,
        columns,
        vec![
            block_with_layout(
                "blockGroup",
                json!({"span": "2"}),
                json!({"romeka": {"block": "blockGroup", "v": 2, "settings": romeka}}),
            ),
            block("html", json!({"html": "<p>Left</p>"})),
            block("html", json!({"html": "<p>Right</p>"})),
            block("html", json!({"html": "<p>After</p>"})),
        ],
    )
}

/// On a normal page each layout adds its class to the group's wrapper and
/// leaves the members as they were, inside it.
#[test]
fn every_section_layout_renders_on_a_normal_page() {
    for (name, _) in SectionLayout::ALL {
        let rendered = render(&section_page(
            PageLayout::Normal,
            json!({"section": {"layout": name}}),
        ));
        assert_eq!(
            rendered,
            format!(
                "<div class=\"blocks-inner page-layout-normal\" style=\"\">\
                 <div class=\"block block-blockGroup romeka-section romeka-section-{name}\" style=\"\">\
                 <div class=\"block block-html\" style=\"\"><p>Left</p></div>\
                 <div class=\"block block-html\" style=\"\"><p>Right</p></div>\
                 </div>\
                 <div class=\"block block-html\" style=\"\"><p>After</p></div>\
                 </div>"
            ),
            "{name}"
        );
    }
}

/// On a grid page the group keeps upstream's full-width placement and adds the
/// section's classes; its members keep their grid classes, which the sheet
/// sets aside inside a section.
#[test]
fn every_section_layout_renders_on_a_grid_page() {
    for (name, _) in SectionLayout::ALL {
        let rendered = render(&section_page(
            PageLayout::Grid,
            json!({"section": {"layout": name}}),
        ));
        assert!(
            rendered.contains(&format!(
                "<div class=\"block block-blockGroup block-group-grid grid-position-1 grid-span-12 \
                 romeka-section romeka-section-{name}\" style=\"\">\
                 <div class=\"block block-html grid-position-auto grid-span-12\" style=\"\">\
                 <p>Left</p></div>"
            )),
            "{name}: {rendered}"
        );
    }
}

/// Alignment, gap and overlay are classes beside the layout's.
#[test]
fn a_section_carries_its_alignment_gap_and_overlay() {
    let rendered = render(&section_page(
        PageLayout::Normal,
        json!({"section": {"layout": "split-40-60", "align": "center", "gap": "large", "overlay": "dark"}}),
    ));
    assert!(
        rendered.contains(
            "class=\"block block-blockGroup romeka-section romeka-section-split-40-60 \
             romeka-section-align-center romeka-section-gap-large romeka-section-overlay-dark\""
        ),
        "{rendered}"
    );
}

/// The text a visitor reads and the blocks that hold it, with every tag's
/// attributes dropped: what upstream renders from the same stored page.
fn content(rendered: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    let mut tag = String::new();
    for ch in rendered.chars() {
        match ch {
            '<' => {
                in_tag = true;
                tag.clear();
            }
            '>' => {
                in_tag = false;
                let name: String = tag.chars().take_while(|c| !c.is_whitespace()).collect();
                out.push_str(&format!("<{name}>"));
            }
            _ if in_tag => tag.push(ch),
            _ => out.push(ch),
        }
    }
    out
}

/// A section and the Romeka styling change only the wrappers' attributes: the
/// same blocks, in the same order and nesting, hold the same text as the page
/// without them, which is the page upstream renders.
#[test]
fn sections_and_styling_leave_the_content_as_upstream_renders_it() {
    let styled = section_page(
        PageLayout::Normal,
        json!({
            "section": {"layout": "cards-3", "gap": "small"},
            "anchor": "highlights",
            "background_role": "surface",
            "bleed": true,
        }),
    );
    let plain = section_page(PageLayout::Normal, JsonValue::Null);
    let styled = render(&styled);
    let plain = render(&plain);
    assert_ne!(styled, plain);
    assert_eq!(content(&styled), content(&plain));
}

/// The shared styling: margins from the spacing scale, a theme text colour, a
/// theme background, the full-bleed class and the anchor as the wrapper's id.
#[test]
fn shared_styling_renders_on_any_block() {
    let rendered = render(&page_of(
        PageLayout::Normal,
        json!({}),
        vec![block_with_layout(
            "html",
            json!({"html": "<p>Styled</p>"}),
            json!({
                "padding_top": "4px",
                "romeka": {"block": "html", "v": 2, "settings": {
                    "anchor": "intro",
                    "space_top": "none",
                    "space_bottom": "large",
                    "bleed": true,
                    "text_color": "on-brand",
                    "background_role": "brand"
                }}
            }),
        )],
    ));
    assert!(
        rendered.contains(
            "<div id=\"intro\" class=\"block block-html has-background romeka-bleed\" \
             style=\"padding-top: 4px; margin-top: 0; margin-bottom: var(--space-5); \
             color: var(--color-on-brand); background-color: var(--color-brand)\">\
             <p>Styled</p></div>"
        ),
        "{rendered}"
    );
}

/// An anchor is the id of the first block that names it; a second block with
/// the same anchor, and a block whose anchor is not a slug, render none.
#[test]
fn an_anchor_is_unique_on_the_page() {
    let anchored = |anchor: &str, text: &str| {
        block_with_layout(
            "html",
            json!({"html": format!("<p>{text}</p>")}),
            json!({"romeka": {"block": "html", "v": 2, "settings": {"anchor": anchor}}}),
        )
    };
    let rendered = render(&page_of(
        PageLayout::Normal,
        json!({}),
        vec![
            anchored("story", "One"),
            anchored("story", "Two"),
            anchored("Our story", "Three"),
        ],
    ));
    assert_eq!(rendered.matches(" id=").count(), 1, "{rendered}");
    assert!(
        rendered.contains("<div id=\"story\" class=\"block block-html\" style=\"\"><p>One</p>"),
        "{rendered}"
    );
}
