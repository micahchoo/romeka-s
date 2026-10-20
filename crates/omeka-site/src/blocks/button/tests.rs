use super::*;
use crate::blocks::test_support::{context, site};
use crate::view::{BlockMarkers, NoResources};
use omeka_core::site::Slug;
use serde_json::json;

fn settings() -> JsonValue {
    json!({
        "button_label": "Visit us",
        "button_page": 2,
        "button2_label": "Write to us",
        "button2_url": "mailto:desk@example.org",
        "style": "outline",
        "alignment": "center",
        "size": "large",
    })
}

/// The plain rendering is both links in one paragraph, the page resolved.
#[test]
fn plain_renders_the_links_in_a_paragraph() {
    let refs = PlainRefs::new().page(PageId::new(2), "/s/main/page/visit");
    assert_eq!(
        ButtonBlock.plain(&settings(), &refs),
        "<p><a href=\"/s/main/page/visit\">Visit us</a> \
         <a href=\"mailto:desk@example.org\">Write to us</a></p>"
    );
}

/// A button with no text, no target or a hostile URL is left out, and a
/// block with no button left stores nothing.
#[test]
fn plain_leaves_out_an_incomplete_or_hostile_button() {
    let refs = PlainRefs::new();
    let plain = ButtonBlock.plain(
        &json!({"button_label": "Go", "button_url": "javascript:alert(1)", "button2_label": "Home", "button2_url": "/s/main"}),
        &refs,
    );
    assert_eq!(plain, "<p><a href=\"/s/main\">Home</a></p>");
    assert_eq!(ButtonBlock.plain(&json!({"button_url": "/x"}), &refs), "");
}

/// The rich rendering draws each link as a button in the chosen style and
/// size, aligned as chosen.
#[test]
fn rich_renders_styled_buttons() {
    let site = site();
    let mut ctx = context(&site, &NoResources);
    ctx.pages = vec![crate::nav::PageRef {
        id: PageId::new(2),
        slug: Slug::parse("visit").unwrap(),
        title: "Visit".to_string(),
        is_public: true,
    }];
    assert_eq!(
        ButtonBlock.render(&settings(), &ctx),
        "<div class=\"romeka-buttons romeka-buttons-align-center\">\
         <a href=\"/s/main/page/visit\" class=\"romeka-button romeka-button-outline romeka-button-large\">Visit us</a>\
         <a href=\"mailto:desk@example.org\" class=\"romeka-button romeka-button-outline romeka-button-large\">Write to us</a></div>"
    );
}

/// Unknown choices read as the defaults: solid, left, medium.
#[test]
fn unknown_choices_read_as_the_defaults() {
    let site = site();
    let ctx = context(&site, &NoResources);
    let rich = ButtonBlock.render(
        &json!({"button_label": "Go", "button_url": "/go", "style": "glow", "alignment": "up", "size": "huge"}),
        &ctx,
    );
    assert!(rich.contains("romeka-buttons-align-left"), "{rich}");
    assert!(
        rich.contains("romeka-button romeka-button-solid romeka-button-medium"),
        "{rich}"
    );
}

/// With no complete button the public page shows nothing and the preview
/// says what is missing.
#[test]
fn an_empty_block_renders_nothing_publicly_and_a_hint_in_the_preview() {
    let site = site();
    let ctx = context(&site, &NoResources);
    assert_eq!(ButtonBlock.render(&json!({}), &ctx), "");
    let mut preview = context(&site, &NoResources);
    preview.block_markers = BlockMarkers::Preview;
    assert!(
        ButtonBlock
            .render(&json!({}), &preview)
            .contains("romeka-buttons-empty")
    );
}

/// The block names the pages its buttons link, so the write can resolve them.
#[test]
fn page_refs_name_the_linked_pages() {
    assert_eq!(ButtonBlock.page_refs(&settings()), vec![PageId::new(2)]);
}
