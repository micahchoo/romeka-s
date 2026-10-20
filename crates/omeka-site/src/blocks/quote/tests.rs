use super::*;
use crate::blocks::romeka::PlainItem;
use crate::blocks::test_support::{TestResources, context, item, site};
use crate::view::{BlockMarkers, NoResources};
use serde_json::json;

fn settings() -> JsonValue {
    json!({
        "text": "<p>The sea is <em>everything</em>.</p><script>x</script>",
        "attribution": "Jules Verne",
        "item": 4,
        "style": "band",
    })
}

/// The plain rendering is the purified quotation in a `blockquote`, then the
/// attribution with the source item cited and linked.
#[test]
fn plain_renders_blockquote_and_cite() {
    let refs = PlainRefs::new().item(PlainItem::new(
        ItemId::new(4),
        "Twenty Thousand Leagues",
        "/s/main/item/4",
    ));
    assert_eq!(
        Quote.plain(&settings(), &refs),
        "<blockquote><p>The sea is <em>everything</em>.</p></blockquote>\
         <p>Jules Verne, <cite><a href=\"/s/main/item/4\">Twenty Thousand Leagues</a></cite></p>"
    );
}

/// A quotation typed without markup becomes a paragraph; with no
/// attribution and no source the line is left out; no quotation stores
/// nothing.
#[test]
fn plain_wraps_bare_text_and_drops_an_empty_line() {
    let refs = PlainRefs::new();
    assert_eq!(
        Quote.plain(&json!({"text": "Less is more"}), &refs),
        "<blockquote><p>Less is more</p></blockquote>"
    );
    assert_eq!(
        Quote.plain(&json!({"text": "Less is more", "item": 9}), &refs),
        "<blockquote><p>Less is more</p></blockquote>",
        "a source the write could not show is left out"
    );
    assert_eq!(Quote.plain(&json!({"attribution": "Nobody"}), &refs), "");
}

/// The rich rendering is a figure in the chosen style with the attribution
/// as its caption.
#[test]
fn rich_renders_a_styled_figure() {
    let site = site();
    let mut resources = TestResources::default();
    resources
        .items
        .insert(4, item(4, "Twenty Thousand Leagues", "/s/main/item/4"));
    let ctx = context(&site, &resources);
    assert_eq!(
        Quote.render(&settings(), &ctx),
        "<figure class=\"romeka-quote romeka-quote-band\"><blockquote class=\"romeka-quote-text\">\
         <p>The sea is <em>everything</em>.</p></blockquote><figcaption class=\"romeka-quote-attribution\">\
         Jules Verne, <cite><a href=\"/s/main/item/4\">Twenty Thousand Leagues</a></cite></figcaption></figure>"
    );
}

/// An unknown style reads as large, and a source the visitor may not see is
/// left out of the caption.
#[test]
fn rich_defaults_and_hidden_source() {
    let site = site();
    let ctx = context(&site, &NoResources);
    let rich = Quote.render(&json!({"text": "Hi", "item": 4, "style": "neon"}), &ctx);
    assert_eq!(
        rich,
        "<figure class=\"romeka-quote romeka-quote-large\"><blockquote class=\"romeka-quote-text\">\
         <p>Hi</p></blockquote></figure>"
    );
}

/// No quotation shows nothing publicly and a hint in the preview.
#[test]
fn an_empty_quote_renders_nothing_publicly_and_a_hint_in_the_preview() {
    let site = site();
    let ctx = context(&site, &NoResources);
    assert_eq!(Quote.render(&json!({}), &ctx), "");
    let mut preview = context(&site, &NoResources);
    preview.block_markers = BlockMarkers::Preview;
    assert!(
        Quote
            .render(&json!({}), &preview)
            .contains("romeka-quote-empty")
    );
}

/// The block selects its source item, if any.
#[test]
fn items_name_the_source() {
    assert_eq!(
        Quote.items(&settings()),
        Some(ItemSelection::Picked(vec![ItemId::new(4)]))
    );
    assert_eq!(Quote.items(&json!({"text": "x"})), None);
}
