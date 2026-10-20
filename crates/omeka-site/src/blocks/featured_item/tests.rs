use super::*;
use crate::blocks::romeka::{PlainImage, PlainItem, PlainValue};
use crate::blocks::test_support::{TestResources, context, item, literal_value, site};
use crate::i18n::Locale;
use crate::view::{BlockMarkers, MediaRef, Thumbnail};
use serde_json::json;

fn settings() -> JsonValue {
    json!({
        "item": 4,
        "image": "right",
        "fields": [{"property": "dcterms:date"}, {"property": "dcterms:creator"}, {"property": ""}],
    })
}

fn resources() -> TestResources {
    let mut resources = TestResources::default();
    let mut bridge = item(4, "Tower Bridge", "/s/main/item/4");
    bridge.description = Some("A bascule bridge".to_string());
    bridge.values = vec![
        literal_value(1, "Title", "Tower Bridge"),
        literal_value(8, "Date", "1894"),
        literal_value(8, "Date", "1886"),
        literal_value(4, "Description", "A bascule bridge"),
    ];
    bridge.thumbnail = Some(Thumbnail::new("/files/medium/4.jpg", ""));
    bridge.primary_media = Some(MediaRef {
        url: "/s/main/media/9".to_string(),
        original_url: None,
        title: "Bridge".to_string(),
        media_type: Some("image/jpeg".to_string()),
        thumbnail: Some(Thumbnail::new("/files/large/4.jpg", "The bridge open")),
        embed: None,
    });
    resources.items.insert(4, bridge);
    resources
}

fn plain_item() -> PlainItem {
    let value = |term: &str, label: &str, text: &str| PlainValue {
        term: term.to_string(),
        label: label.to_string(),
        text: text.to_string(),
    };
    PlainItem {
        image: Some(PlainImage {
            url: "/files/large/4.jpg".to_string(),
            alt: "The bridge open".to_string(),
        }),
        values: vec![
            value("dcterms:title", "Title", "Tower Bridge"),
            value("dcterms:date", "Date", "1894"),
            value("dcterms:date", "Date", "1886"),
            value("dcterms:description", "Description", "A <bascule> bridge"),
        ],
        ..PlainItem::new(ItemId::new(4), "Tower Bridge", "/s/main/item/4")
    }
}

/// The plain card is a linked heading, the image, the chosen fields as a
/// definition list (a field the item lacks is left out) and the link.
#[test]
fn plain_renders_heading_image_values_and_link() {
    let refs = PlainRefs::new().item(plain_item());
    assert_eq!(
        FeaturedItem.plain(&settings(), &refs),
        "<h3><a href=\"/s/main/item/4\">Tower Bridge</a></h3>\
         <img src=\"/files/large/4.jpg\" alt=\"The bridge open\">\
         <dl><dt>Date</dt><dd>1894</dd><dd>1886</dd></dl>\
         <p><a href=\"/s/main/item/4\">View item</a></p>"
    );
}

/// With no field chosen the plain card shows the description, and the link
/// says "View item" in the site's language unless the curator wrote words.
#[test]
fn plain_shows_the_description_and_the_sites_link_words() {
    let refs = PlainRefs::new().item(plain_item()).locale(Locale::Es);
    let plain = FeaturedItem.plain(&json!({"item": 4}), &refs);
    assert!(plain.contains("<p>A &lt;bascule&gt; bridge</p>"), "{plain}");
    assert!(plain.ends_with(">Ver ficha</a></p>"), "{plain}");
    let labelled = FeaturedItem.plain(&json!({"item": 4, "link_label": "Read on"}), &refs);
    assert!(labelled.ends_with(">Read on</a></p>"), "{labelled}");
}

/// An item the write could not show to anyone stores no card.
#[test]
fn plain_without_the_item_is_empty() {
    assert_eq!(FeaturedItem.plain(&settings(), &PlainRefs::new()), "");
}

/// The rich card puts the large image on the chosen side, links the title,
/// lists each chosen field's values and ends with the link.
#[test]
fn rich_renders_the_card() {
    let site = site();
    let resources = resources();
    let ctx = context(&site, &resources);
    let rich = FeaturedItem.render(&settings(), &ctx);
    assert!(
        rich.starts_with(
            "<article class=\"romeka-featured romeka-featured-image-right\">\
             <div class=\"romeka-featured-card\">"
        ),
        "{rich}"
    );
    assert!(
        rich.contains("src=\"/files/large/4.jpg\" alt=\"The bridge open\""),
        "{rich}"
    );
    assert!(
        rich.contains(
            "<h3 class=\"romeka-featured-title\"><a href=\"/s/main/item/4\">Tower Bridge</a></h3>"
        ),
        "{rich}"
    );
    assert!(
        rich.contains(
            "<dl class=\"romeka-featured-values\"><div class=\"romeka-featured-value\"><dt>Date</dt>\
             <dd>1894</dd><dd>1886</dd></div></dl>"
        ),
        "{rich}"
    );
    assert!(
        rich.contains("<a class=\"romeka-featured-link\" href=\"/s/main/item/4\">View item</a>"),
        "{rich}"
    );
}

/// With no field chosen the rich card shows the description; an item with no
/// image draws a text card.
#[test]
fn rich_shows_the_description_and_a_card_without_an_image() {
    let site = site();
    let mut resources = resources();
    if let Some(bridge) = resources.items.get_mut(&4) {
        bridge.primary_media = None;
        bridge.thumbnail = None;
    }
    let ctx = context(&site, &resources);
    let rich = FeaturedItem.render(&json!({"item": 4, "image": "top"}), &ctx);
    assert!(
        rich.contains("romeka-featured-image-top romeka-featured-no-image"),
        "{rich}"
    );
    assert!(!rich.contains("<img"), "{rich}");
    assert!(
        rich.contains("<p class=\"romeka-featured-description\">A bascule bridge</p>"),
        "{rich}"
    );
}

/// An item the visitor may not see renders nothing publicly and a hint in
/// the editor preview.
#[test]
fn a_missing_item_renders_nothing_publicly_and_a_hint_in_the_preview() {
    let site = site();
    let resources = TestResources::default();
    let ctx = context(&site, &resources);
    assert_eq!(FeaturedItem.render(&settings(), &ctx), "");
    let mut preview = context(&site, &resources);
    preview.block_markers = BlockMarkers::Preview;
    assert!(
        FeaturedItem
            .render(&settings(), &preview)
            .contains("romeka-featured-empty"),
    );
}

/// The block selects its one item, and nothing before one is picked.
#[test]
fn items_name_the_picked_item() {
    assert_eq!(
        FeaturedItem.items(&json!({"item": "4"})),
        Some(ItemSelection::Picked(vec![ItemId::new(4)]))
    );
    assert_eq!(FeaturedItem.items(&json!({})), None);
}
