#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;

use omeka_core::site::{PageLayout, SiteBlock, SitePage};
use serde_json::{Value as JsonValue, json};

use super::*;
use crate::blocks::romeka::{PlainRefs, plain_for, plain_hash};
use crate::blocks::test_support::{block, block_with_layout, context, page, site};
use crate::blocks::{BlockLayout, render_page_blocks};
use crate::view::{BlockMarkers, NoResources};

/// A Romeka block as a save stores it: the plain rendering and its hash.
fn romeka(name: &str, settings: JsonValue) -> SiteBlock {
    let carrier = json!({ "block": name, "v": 2, "settings": settings });
    let parsed = omeka_core::site::BlockLayoutData::parse(&json!({ "romeka": carrier }));
    let plain = plain_for(parsed.romeka.as_ref().unwrap(), &PlainRefs::new()).unwrap();
    block_with_layout(
        "html",
        json!({ "html": plain }),
        json!({ "romeka": {
            "block": name, "v": 2, "settings": settings, "plain_hash": plain_hash(&plain),
        }}),
    )
}

fn page_of(blocks: Vec<SiteBlock>) -> SitePage {
    let mut blocks = blocks;
    for (index, block) in blocks.iter_mut().enumerate() {
        block.position = index as i64 + 1;
        block.id = omeka_core::ids::BlockId::new(index as u64 + 1);
    }
    page(PageLayout::Normal, json!({}), blocks)
}

fn public(page: &SitePage) -> String {
    let site = site();
    render_page_blocks(page, &context(&site, &NoResources))
}

fn preview(page: &SitePage, hints: &[(&str, &str)]) -> String {
    let site = site();
    let mut ctx = context(&site, &NoResources).with_hints(
        hints
            .iter()
            .map(|(key, text)| ((*key).to_string(), (*text).to_string()))
            .collect::<BTreeMap<_, _>>(),
    );
    ctx.block_markers = BlockMarkers::Preview;
    render_page_blocks(page, &ctx)
}

const EMPTY_PAGE: &str = "<div class=\"blocks-inner page-layout-normal\" style=\"\"></div>";

/// The admin's blank check is tested against these same samples
/// (`blocks.json`, `blankHtml`).
#[test]
fn blank_html_matches_every_sample() {
    for sample in BLANK_HTML_SAMPLES {
        assert_eq!(
            is_blank_html(sample.html),
            sample.blank,
            "{:?}",
            sample.html
        );
    }
}

/// The hero, an HTML block, a quote and a list of pages each name the field
/// that holds their words.
#[test]
fn each_text_block_names_its_primary_field() {
    let key = |schema| primary_field(schema).map(|field: &Field| field.key);
    assert_eq!(key(super::super::hero::Hero.schema()), Some("heading"));
    assert_eq!(key(super::super::quote::Quote.schema()), Some("text"));
    assert_eq!(key(super::super::Html.schema()), Some("html"));
    assert_eq!(key(super::super::ListOfPages.schema()), Some("pagelist"));
    for block in romeka::blocks() {
        assert!(
            block
                .schema()
                .fields
                .iter()
                .filter(|field| field.primary)
                .count()
                <= 1,
            "{}",
            block.name()
        );
    }
}

/// A block whose primary field is empty renders nothing for a visitor: no
/// wrapper, and no heading or call to action left alone.
#[test]
fn a_visitor_sees_no_unwritten_block() {
    let page = page_of(vec![
        romeka(
            "hero",
            json!({ "heading": "", "cta_label": "Explore", "cta_url": "../item" }),
        ),
        block("html", json!({ "html": "<p><br></p>" })),
        romeka("quote", json!({ "text": "", "attribution": "Ada" })),
        block("listOfPages", json!({ "pagelist": "[]" })),
    ]);
    assert_eq!(public(&page), EMPTY_PAGE);
}

/// A written block renders exactly as before.
#[test]
fn a_written_block_renders_as_before() {
    let page = page_of(vec![block("html", json!({ "html": "<p>Hello</p>" }))]);
    assert_eq!(
        public(&page),
        "<div class=\"blocks-inner page-layout-normal\" style=\"\">\
         <div class=\"block block-html\" style=\"\"><p>Hello</p></div></div>"
    );
}

/// The preview shows an unwritten hero as a banner with the hint, greyed,
/// where the heading goes, inside the block's marker.
#[test]
fn the_preview_shows_the_hint_where_the_words_go() {
    let page = page_of(vec![romeka("hero", json!({ "heading": "" }))]);
    let rendered = preview(
        &page,
        &[(
            "fieldExample.hero.heading",
            "e.g. Welcome <to> the collection",
        )],
    );
    assert!(rendered.contains("data-romeka-block=\"1\""), "{rendered}");
    assert!(
        rendered.contains(
            "<h2 class=\"romeka-hero-heading\"><span class=\"romeka-hint\">\
             e.g. Welcome &lt;to&gt; the collection</span></h2>"
        ),
        "{rendered}"
    );
    assert!(!rendered.contains(HINT_SLOT), "{rendered}");
}

/// An empty HTML block shows its hint as its text, and an empty list of pages
/// shows its hint as its whole body.
#[test]
fn the_preview_hints_an_html_block_and_a_page_list() {
    let page = page_of(vec![
        block("html", json!({ "html": "" })),
        block("listOfPages", json!({ "pagelist": "[]" })),
    ]);
    let rendered = preview(
        &page,
        &[
            ("fieldExample.html.html", "Write here"),
            ("fieldExample.listOfPages.pagelist", "No pages chosen"),
        ],
    );
    assert!(
        rendered.contains("<span class=\"romeka-hint\">Write here</span>"),
        "{rendered}"
    );
    assert!(
        rendered.contains("<p class=\"romeka-hint\">No pages chosen</p>"),
        "{rendered}"
    );
}

/// A preview request without the hint text renders the block as it is.
#[test]
fn the_preview_without_hint_text_renders_the_block_as_it_is() {
    let page = page_of(vec![block("html", json!({ "html": "" }))]);
    let rendered = preview(&page, &[]);
    assert!(!rendered.contains("romeka-hint"), "{rendered}");
    assert!(rendered.contains("data-romeka-block=\"1\""), "{rendered}");
}

/// A section whose every member is unwritten leaves no empty wrapper, and one
/// with a written member keeps the wrapper and that member only.
#[test]
fn a_section_of_unwritten_blocks_renders_nothing() {
    let group = || {
        block_with_layout(
            "blockGroup",
            json!({ "span": "2" }),
            json!({ "romeka": { "block": "blockGroup", "v": 2,
                "settings": { "section": { "layout": "cards-3" } } } }),
        )
    };
    let empty = page_of(vec![
        group(),
        block("html", json!({ "html": "" })),
        block("html", json!({ "html": "" })),
    ]);
    assert_eq!(public(&empty), EMPTY_PAGE);

    let half = page_of(vec![
        group(),
        block("html", json!({ "html": "" })),
        block("html", json!({ "html": "<p>Card</p>" })),
        block("html", json!({ "html": "<p>After</p>" })),
    ]);
    assert_eq!(
        public(&half),
        "<div class=\"blocks-inner page-layout-normal\" style=\"\">\
         <div class=\"block block-blockGroup romeka-section romeka-section-cards-3\" style=\"\">\
         <div class=\"block block-html\" style=\"\"><p>Card</p></div>\
         </div>\
         <div class=\"block block-html\" style=\"\"><p>After</p></div></div>"
    );
}

/// The plain rendering of an unwritten Romeka block is empty, so an upstream
/// reader sees no lone call to action either.
#[test]
fn an_unwritten_romeka_block_has_an_empty_plain_rendering() {
    let hero = romeka(
        "hero",
        json!({ "heading": "  ", "cta_label": "Explore", "cta_url": "../item" }),
    );
    assert_eq!(hero.data.to_json(), json!({ "html": "" }));
    let written = romeka("hero", json!({ "heading": "Welcome" }));
    assert_eq!(
        written.data.to_json(),
        json!({ "html": "<h2>Welcome</h2>" })
    );
}

/// A carrier edited outside Romeka S renders its stored HTML, so that HTML
/// decides whether the block is written.
#[test]
fn an_edited_carrier_is_written_by_its_stored_html() {
    let edited = block_with_layout(
        "html",
        json!({ "html": "<p>Written upstream</p>" }),
        json!({ "romeka": {
            "block": "hero", "v": 2, "settings": { "heading": "" },
            "plain_hash": plain_hash(""),
        }}),
    );
    assert!(unwritten(&edited).is_none());
    let page = page_of(vec![edited]);
    assert!(public(&page).contains("<p>Written upstream</p>"));
}
