use super::*;
use crate::blocks::romeka::{CarrierRender, PlainItem, plain_hash, render_carrier};
use crate::blocks::test_support::{TestResources, block_with_layout, context, item, site};
use crate::view::{AssetView, BlockMarkers, MediaRef};
use serde_json::json;

fn media_settings() -> JsonValue {
    json!({
        "source": "media",
        "media": [
            {"item": 4, "media": 40, "caption": "<p>The <em>north</em> face</p>"},
            {"item": 5, "media": null, "caption": ""},
            {"item": 6, "media": null, "caption": "Private"},
        ],
        "assets": [{"asset": 9, "caption": "hidden by the source choice"}],
    })
}

fn resources() -> TestResources {
    let mut resources = TestResources::default();
    resources.images.insert(
        40,
        Thumbnail::new("/files/large/north.jpg", "North face of the tower"),
    );
    resources
        .items
        .insert(4, item(4, "Tower", "/s/main/item/4"));
    let mut bridge = item(5, "Bridge", "/s/main/item/5");
    bridge.primary_media = Some(MediaRef {
        url: "/s/main/media/50".to_string(),
        original_url: Some("/files/original/bridge.tif".to_string()),
        title: "Bridge scan".to_string(),
        media_type: Some("image/tiff".to_string()),
        thumbnail: Some(Thumbnail::new("/files/large/bridge.jpg", "")),
        embed: None,
    });
    resources.items.insert(5, bridge);
    resources
}

fn refs() -> PlainRefs {
    PlainRefs::new()
        .media(
            MediaId::new(40),
            "/files/large/north.jpg",
            "North face of the tower",
        )
        .item(PlainItem::new(ItemId::new(4), "Tower", "/s/main/item/4"))
        .item(PlainItem {
            image: Some(PlainImage {
                url: "/files/large/bridge.jpg".to_string(),
                alt: String::new(),
            }),
            ..PlainItem::new(ItemId::new(5), "Bridge", "/s/main/item/5")
        })
}

/// The plain rendering is a list of image links to each large file, with the
/// curator's caption or else the item's title. A record whose item or media
/// the write could not show to anyone is left out.
#[test]
fn plain_lists_image_links_with_captions() {
    assert_eq!(
        Gallery.plain(&media_settings(), &refs()),
        "<ul><li><a href=\"/files/large/north.jpg\"><img src=\"/files/large/north.jpg\" \
         alt=\"North face of the tower\"></a><div><p>The <em>north</em> face</p></div></li>\
         <li><a href=\"/files/large/bridge.jpg\"><img src=\"/files/large/bridge.jpg\" \
         alt=\"Bridge\"></a><div>Bridge</div></li></ul>"
    );
}

/// With captions off, the plain rendering carries the images alone, and a
/// gallery with nothing to show stores nothing.
#[test]
fn plain_without_captions_or_images() {
    let mut settings = media_settings();
    settings["captions"] = json!(false);
    let plain = Gallery.plain(&settings, &refs());
    assert!(!plain.contains("<div>"), "{plain}");
    assert!(plain.contains("alt=\"Bridge\""), "{plain}");
    assert_eq!(Gallery.plain(&media_settings(), &PlainRefs::new()), "");
}

/// An asset gallery shows the assets in order, with their captions, and
/// ignores the media list the source choice hides.
#[test]
fn asset_source_reads_the_asset_list() {
    let settings = json!({
        "source": "assets",
        "media": [{"item": 4, "media": 40}],
        "assets": [{"asset": 9, "caption": "<p>Map room</p>"}, {"asset": "10"}],
    });
    let refs = PlainRefs::new()
        .asset(AssetId::new(9), "/files/asset/room.jpg", "The map room")
        .asset(AssetId::new(10), "/files/asset/desk.jpg", "A desk");
    assert_eq!(
        Gallery.plain(&settings, &refs),
        "<ul><li><a href=\"/files/asset/room.jpg\"><img src=\"/files/asset/room.jpg\" \
         alt=\"The map room\"></a><div><p>Map room</p></div></li>\
         <li><a href=\"/files/asset/desk.jpg\"><img src=\"/files/asset/desk.jpg\" \
         alt=\"A desk\"></a></li></ul>"
    );
    assert_eq!(
        Gallery.asset_refs(&settings),
        vec![AssetId::new(9), AssetId::new(10)]
    );
    assert!(Gallery.media_refs(&settings).is_empty());
    assert_eq!(Gallery.items(&settings), None);
}

/// A media gallery names the chosen media and every record's item, once
/// each, in record order, and no asset.
#[test]
fn media_source_names_its_media_and_items() {
    let mut settings = media_settings();
    settings["media"]
        .as_array_mut()
        .unwrap()
        .push(json!({"item": 4, "media": 41}));
    assert_eq!(
        Gallery.media_refs(&settings),
        vec![MediaId::new(40), MediaId::new(41)]
    );
    assert_eq!(
        Gallery.items(&settings),
        Some(ItemSelection::Picked(vec![
            ItemId::new(4),
            ItemId::new(5),
            ItemId::new(6)
        ]))
    );
    assert!(Gallery.asset_refs(&settings).is_empty());
}

/// The rich gallery is a list of figures in the chosen layout and column
/// count, each image a link to its large file, and it asks for the
/// lightbox with the dialog's words.
#[test]
fn rich_renders_figures_in_the_chosen_layout() {
    let site = site();
    let resources = resources();
    let ctx = context(&site, &resources);
    let mut settings = media_settings();
    settings["layout"] = json!("masonry");
    settings["columns"] = json!(4);
    let rich = Gallery.render(&settings, &ctx);
    assert!(
        rich.starts_with(
            "<div class=\"romeka-gallery romeka-gallery-masonry romeka-gallery-columns-4\" \
             data-romeka-lightbox=\"\" data-close=\"Close\" data-previous=\"Previous image\" \
             data-next=\"Next image\" data-position=\"Image {index} of {count}\">"
        ),
        "{rich}"
    );
    assert!(
        rich.contains(
            "<a class=\"romeka-gallery-link\" href=\"/files/large/bridge.jpg\">\
             <img class=\"romeka-gallery-image\" src=\"/files/large/bridge.jpg\" alt=\"Bridge\" \
             loading=\"lazy\"></a><figcaption class=\"romeka-gallery-caption\">Bridge</figcaption>"
        ),
        "{rich}"
    );
    assert!(
        rich.contains("<figcaption class=\"romeka-gallery-caption\"><p>The <em>north</em> face</p></figcaption>"),
        "{rich}"
    );
    assert_eq!(
        rich.matches("<li class=\"romeka-gallery-item\">").count(),
        2,
        "{rich}"
    );
}

/// A column count outside two to six is clamped, a carousel's scroller is
/// focusable and named, and the lightbox and captions can be turned off.
#[test]
fn rich_honours_the_carousel_and_the_switches() {
    let site = site();
    let resources = resources();
    let ctx = context(&site, &resources);
    let mut settings = media_settings();
    settings["layout"] = json!("carousel");
    settings["columns"] = json!(12);
    settings["lightbox"] = json!(false);
    settings["captions"] = json!(false);
    let rich = Gallery.render(&settings, &ctx);
    assert!(
        rich.contains("romeka-gallery-carousel romeka-gallery-columns-6"),
        "{rich}"
    );
    assert!(
        rich.contains("<ul class=\"romeka-gallery-list\" tabindex=\"0\" aria-label=\"Gallery\">"),
        "{rich}"
    );
    assert!(!rich.contains(MARKER), "{rich}");
    assert!(!rich.contains("figcaption"), "{rich}");
}

/// An asset gallery renders the assets' files, named by their alt text or
/// else their title.
#[test]
fn rich_renders_assets() {
    let site = site();
    let mut resources = TestResources::default();
    resources.assets.insert(
        9,
        AssetView {
            id: AssetId::new(9),
            title: "room.jpg".to_string(),
            url: Some("/files/asset/room.jpg".to_string()),
            thumbnail: Some(Thumbnail::new("/files/asset/room.jpg", "")),
        },
    );
    let ctx = context(&site, &resources);
    let rich = Gallery.render(
        &json!({"source": "assets", "assets": [{"asset": 9, "caption": ""}]}),
        &ctx,
    );
    assert!(
        rich.contains("src=\"/files/asset/room.jpg\" alt=\"room.jpg\""),
        "{rich}"
    );
    assert!(!rich.contains("figcaption"), "{rich}");
}

/// With no image to show, the public page shows nothing and the editor
/// preview says what to do.
#[test]
fn an_empty_gallery_renders_nothing_publicly_and_a_hint_in_the_preview() {
    let site = site();
    let resources = TestResources::default();
    let ctx = context(&site, &resources);
    assert_eq!(Gallery.render(&media_settings(), &ctx), "");
    let mut preview = context(&site, &resources);
    preview.block_markers = BlockMarkers::Preview;
    assert_eq!(
        Gallery.render(&media_settings(), &preview),
        "<p class=\"romeka-gallery-empty\">This gallery has no images yet. Pick items or assets \
         in the block&#039;s settings.</p>"
    );
}

/// A page loads the lightbox script only when a gallery on it asks for it.
#[test]
fn with_assets_adds_the_script_only_for_a_lightbox() {
    let site = site();
    let ctx = context(&site, &crate::view::NoResources);
    assert_eq!(with_assets("<p>x</p>".to_string(), &ctx), "<p>x</p>");
    let with = with_assets(format!("<div {MARKER}=\"\"></div>"), &ctx);
    assert!(with.contains("js/lightbox.js"), "{with}");
}

/// A gallery carrier whose plain rendering still matches renders the rich
/// gallery.
#[test]
fn a_matching_carrier_renders_the_gallery() {
    let site = site();
    let resources = resources();
    let ctx = context(&site, &resources);
    let plain = Gallery.plain(&media_settings(), &refs());
    let block = block_with_layout(
        "html",
        json!({ "html": plain }),
        json!({ "romeka": {
            "block": "gallery",
            "v": 2,
            "settings": media_settings(),
            "plain_hash": plain_hash(&plain),
        }}),
    );
    match render_carrier(&block, &ctx) {
        Some(CarrierRender::Rich(html)) => assert!(html.contains("romeka-gallery-grid"), "{html}"),
        other => panic!("expected the rich gallery, got {other:?}"),
    }
}
