use super::settings::{Alignment, Background, HeroSettings, Overlay};
use super::*;
use crate::blocks::romeka::{CarrierRender, plain_hash, render_carrier};
use crate::blocks::test_support::TestResources;
use crate::blocks::test_support::{block_with_layout, context, site};
use crate::view::{AssetView, NoResources, Thumbnail};
use omeka_core::ids::MediaId;

use omeka_core::site::{RomkaCarrier, SiteBlock};
use serde_json::json;

fn settings() -> JsonValue {
    json!({
        "heading": "Welcome",
        "subheading": "A library of things",
        "body": "<p>Read <em>more</em>.</p>",
        "background": 3,
        "overlay": "strong",
        "alignment": "center",
        "cta_label": "Browse",
        "cta_page": 2,
        "cta_url": ""
    })
}

fn refs() -> PlainRefs {
    PlainRefs::new()
        .asset(AssetId::new(3), "/files/asset/bg.png", "A shelf")
        .page(PageId::new(2), "/s/main/page/about")
}

fn empty_carrier() -> RomkaCarrier {
    RomkaCarrier {
        block: "hero".to_string(),
        v: 1,
        settings: json!({}),
        plain_hash: None,
    }
}

fn carrier_block(plain: &str, carrier: RomkaCarrier) -> SiteBlock {
    block_with_layout(
        "html",
        json!({ "html": plain }),
        json!({ "romeka": {
            "block": carrier.block,
            "v": carrier.v,
            "settings": carrier.settings,
            "plain_hash": carrier.plain_hash,
        }}),
    )
}

/// The plain rendering is upstream's vocabulary: a heading, a paragraph,
/// the image with its alt text, the body copy and a link.
#[test]
fn plain_renders_upstream_markup() {
    assert_eq!(
        Hero.plain(&settings(), &refs()),
        "<h2>Welcome</h2><p>A library of things</p><p>Read <em>more</em>.</p>\
         <img src=\"/files/asset/bg.png\" alt=\"A shelf\">\
         <a href=\"/s/main/page/about\">Browse</a>"
    );
}

/// The plain rendering purifies the settings' html and drops a hostile
/// call to action, so the stored carrier is safe for upstream to echo.
#[test]
fn plain_purifies_body_and_rejects_a_hostile_target() {
    let plain = Hero.plain(
        &json!({
            "heading": "Hi",
            "body": "<p>ok</p><script>alert(1)</script>",
            "cta_label": "Go",
            "cta_url": "javascript:alert(1)"
        }),
        &PlainRefs::new(),
    );
    assert!(!plain.contains("<script"), "{plain}");
    assert!(!plain.contains("javascript:"), "{plain}");
    assert!(plain.contains("<p>ok</p>"), "{plain}");
}

/// A carrier whose stored plain rendering matches its hash renders the
/// rich banner.
#[test]
fn matching_hash_renders_the_rich_banner() {
    let plain = Hero.plain(&settings(), &refs());
    let site = site();
    let mut ctx = context(&site, &NoResources);
    ctx.pages = vec![crate::nav::PageRef {
        id: PageId::new(2),
        slug: omeka_core::site::Slug::parse("about").unwrap(),
        title: "About".to_string(),
        is_public: true,
    }];
    let mut carrier = empty_carrier();
    carrier.settings = settings();
    carrier.plain_hash = Some(plain_hash(&plain));
    let block = carrier_block(&plain, carrier);
    assert_eq!(
        render_carrier(&block, &ctx),
        Some(CarrierRender::Rich(Hero.render(&settings(), &ctx)))
    );
    let rich = Hero.render(&settings(), &ctx);
    assert!(
        rich.contains(
            "class=\"romeka-hero romeka-hero-align-center romeka-hero-overlay-strong romeka-hero-height-medium"
        ),
        "{rich}"
    );
    assert!(
        rich.contains("<h2 class=\"romeka-hero-heading\">Welcome</h2>"),
        "{rich}"
    );
    assert!(
        rich.contains("<p class=\"romeka-hero-subheading\">A library of things</p>"),
        "{rich}"
    );
    assert!(
        rich.contains("<div class=\"romeka-hero-body\"><p>Read <em>more</em>.</p></div>"),
        "{rich}"
    );
    assert!(
        rich.contains("<a href=\"/s/main/page/about\" class=\"romeka-hero-cta\">Browse</a>"),
        "{rich}"
    );
}

/// A background asset resolves to an image behind the overlay, with the
/// asset's alt text.
#[test]
fn rich_renders_the_background_image() {
    let mut resources = TestResources::default();
    resources.assets.insert(
        3,
        AssetView {
            id: AssetId::new(3),
            title: "A shelf".to_string(),
            url: Some("/files/asset/bg.png".to_string()),
            thumbnail: Some(Thumbnail::new("/files/asset/bg.png", "A shelf")),
            kind: crate::view::AssetKind::Image,
            size: None,
        },
    );
    let site = site();
    let ctx = context(&site, &resources);
    let rich = Hero.render(&settings(), &ctx);
    assert!(
        rich.contains(
            "<img class=\"romeka-hero-background\" src=\"/files/asset/bg.png\" alt=\"A shelf\">"
        ),
        "{rich}"
    );
}

/// A background asset slot accepts images, so a stored PDF draws nothing: the
/// banner keeps its text and never prints an `<img>` for a file no browser can
/// show.
#[test]
fn a_non_image_asset_background_renders_no_image() {
    let mut resources = TestResources::default();
    resources.assets.insert(
        3,
        AssetView {
            id: AssetId::new(3),
            title: "report.pdf".to_string(),
            url: Some("/files/asset/report.pdf".to_string()),
            thumbnail: Some(Thumbnail::new("/files/asset/report.pdf", "")),
            kind: crate::view::AssetKind::Pdf,
            size: Some(4096),
        },
    );
    let site = site();
    let ctx = context(&site, &resources);
    let rich = Hero.render(&settings(), &ctx);
    assert!(!rich.contains("<img"), "{rich}");
    assert!(rich.contains("romeka-hero-no-image"), "{rich}");
    assert!(
        rich.contains("<h2 class=\"romeka-hero-heading\">Welcome</h2>"),
        "{rich}"
    );
}

/// A media background the provider resolved no still for, such as audio,
/// renders no image either.
#[test]
fn a_media_background_without_a_still_renders_no_image() {
    let resources = TestResources::default();
    let site = site();
    let ctx = context(&site, &resources);
    let rich = Hero.render(&media_settings(), &ctx);
    assert!(!rich.contains("<img"), "{rich}");
    assert!(rich.contains("romeka-hero-no-image"), "{rich}");
}

/// An edited plain rendering renders that edit, purified, not the rich
/// banner.
#[test]
fn mismatched_hash_renders_the_edited_html() {
    let plain = Hero.plain(&settings(), &refs());
    let site = site();
    let ctx = context(&site, &NoResources);
    let mut carrier = empty_carrier();
    carrier.settings = settings();
    carrier.plain_hash = Some(plain_hash(&plain));
    let block = carrier_block("<p>Edited outside</p><script>x</script>", carrier);
    match render_carrier(&block, &ctx) {
        Some(CarrierRender::Edited(html)) => {
            assert_eq!(html, "<p>Edited outside</p>");
        }
        other => panic!("expected the edited plain html, got {other:?}"),
    }
}

/// A block with no hash renders its stored html, because no Romeka write
/// has vouched for the plain rendering yet.
#[test]
fn missing_hash_renders_the_stored_html() {
    let site = site();
    let ctx = context(&site, &NoResources);
    let mut carrier = empty_carrier();
    carrier.settings = settings();
    let block = carrier_block("<p>Plain</p>", carrier);
    assert_eq!(
        render_carrier(&block, &ctx),
        Some(CarrierRender::Edited("<p>Plain</p>".to_string()))
    );
}

/// A carrier naming an unregistered block renders nothing through this
/// path, so the html layout's own rendering applies.
#[test]
fn unknown_block_is_not_a_carrier() {
    let site = site();
    let ctx = context(&site, &NoResources);
    let mut carrier = empty_carrier();
    carrier.block = "noSuchBlock".to_string();
    assert_eq!(
        render_carrier(&carrier_block("<p>Plain</p>", carrier), &ctx),
        None
    );
}

/// The page reference wins over the URL, and only a safe URL renders.
#[test]
fn call_to_action_prefers_the_page_and_rejects_a_hostile_url() {
    assert_eq!(
        HeroSettings::parse(&settings()).buttons[0].plain_href(&refs()),
        Some("/s/main/page/about".to_string())
    );
    let url_only = HeroSettings::parse(&json!({
        "cta_label": "Go",
        "cta_url": "https://example.org/x"
    }));
    assert_eq!(
        url_only.buttons[0].plain_href(&PlainRefs::new()),
        Some("https://example.org/x".to_string())
    );
    let hostile = HeroSettings::parse(&json!({
        "cta_label": "Go",
        "cta_url": "javascript:alert(1)"
    }));
    assert_eq!(hostile.buttons[0].plain_href(&PlainRefs::new()), None);
}

/// The parsed settings read the ids from both shapes a form or a document
/// can carry.
#[test]
fn settings_read_ids_from_numbers_and_numeric_strings() {
    let parsed = HeroSettings::parse(&json!({
        "heading": "Hi",
        "background": "3",
        "cta_page": 2
    }));
    assert_eq!(parsed.background, Background::Asset(AssetId::new(3)));
    assert_eq!(parsed.buttons[0].page, Some(PageId::new(2)));
    assert_eq!(parsed.overlay, Overlay::Medium);
    assert_eq!(parsed.alignment, Alignment::Left);
}

/// A hero whose background is a site media: the source choice and the media id.
fn media_settings() -> JsonValue {
    json!({
        "heading": "Maps",
        "background_source": "media",
        "background_media": 9,
        "background": 3
    })
}

/// A media background writes the media's large rendition and its alt text
/// into the plain rendering, and the stale asset id the source hides is not
/// read.
#[test]
fn plain_names_the_media_image() {
    let refs = PlainRefs::new()
        .asset(AssetId::new(3), "/files/asset/bg.png", "A shelf")
        .media(MediaId::new(9), "/files/large/abc.jpg", "An old map");
    assert_eq!(
        Hero.plain(&media_settings(), &refs),
        "<h2>Maps</h2><img src=\"/files/large/abc.jpg\" alt=\"An old map\">"
    );
}

/// The block names the media a write must check and a page load must read,
/// and only for the source the curator chose.
#[test]
fn the_source_decides_which_reference_counts() {
    assert_eq!(Hero.media_refs(&media_settings()), vec![MediaId::new(9)]);
    assert!(Hero.asset_refs(&media_settings()).is_empty());
    let asset = json!({ "background": 3, "background_media": 9 });
    assert!(
        Hero.media_refs(&asset).is_empty(),
        "an old hero reads its asset"
    );
    assert_eq!(Hero.asset_refs(&asset), vec![AssetId::new(3)]);
}

/// The rich banner shows the media image the provider resolved, with its alt
/// text and the focal point as the image's position in the frame.
#[test]
fn rich_renders_the_media_image_at_its_focal_point() {
    let mut resources = TestResources::default();
    resources
        .images
        .insert(9, Thumbnail::new("/files/large/abc.jpg", "An old map"));
    let site = site();
    let ctx = context(&site, &resources);
    let mut settings = media_settings();
    settings["focal_x"] = json!(20);
    settings["focal_y"] = json!("80");
    let rich = Hero.render(&settings, &ctx);
    assert!(
        rich.contains(
            "<img class=\"romeka-hero-background\" src=\"/files/large/abc.jpg\" \
             alt=\"An old map\" style=\"object-position: 20% 80%\">"
        ),
        "{rich}"
    );
    assert!(!rich.contains("romeka-hero-no-image"), "{rich}");
}

/// A media the provider did not resolve (private, or gone) leaves the banner
/// without an image rather than with a broken one.
#[test]
fn an_unresolved_media_renders_no_image() {
    let site = site();
    let ctx = context(&site, &NoResources);
    let rich = Hero.render(&media_settings(), &ctx);
    assert!(rich.contains("romeka-hero-no-image"), "{rich}");
    assert!(!rich.contains("<img"), "{rich}");
}

/// The height, the overlay colour and the heading level reach the markup; a
/// hostile overlay colour is dropped.
#[test]
fn height_overlay_colour_and_heading_level_render() {
    let site = site();
    let ctx = context(&site, &NoResources);
    let rich = Hero.render(
        &json!({
            "heading": "Welcome",
            "heading_level": "h1",
            "height": "full",
            "overlay_color": "brand",
        }),
        &ctx,
    );
    assert!(rich.contains("romeka-hero-height-full"), "{rich}");
    assert!(
        rich.contains(
            "<div class=\"romeka-hero-overlay\" aria-hidden=\"true\" \
             style=\"background: var(--color-brand)\"></div>"
        ),
        "{rich}"
    );
    assert!(
        rich.contains("<h1 class=\"romeka-hero-heading\">Welcome</h1>"),
        "{rich}"
    );
    let hostile = Hero.render(&json!({ "overlay_color": "red; background: url(x)" }), &ctx);
    assert!(!hostile.contains("style="), "{hostile}");
}

/// The second button renders as an outline beside the first, in both
/// renderings; a button without text renders nothing.
#[test]
fn two_buttons_render_and_an_empty_one_does_not() {
    let settings = json!({
        "heading": "Welcome",
        "cta_label": "Browse",
        "cta_url": "/s/main/item",
        "cta2_label": "About",
        "cta2_url": "https://example.org/about",
    });
    assert_eq!(
        Hero.plain(&settings, &PlainRefs::new()),
        "<h2>Welcome</h2><a href=\"/s/main/item\">Browse</a>\
         <a href=\"https://example.org/about\">About</a>"
    );
    let site = site();
    let ctx = context(&site, &NoResources);
    let rich = Hero.render(&settings, &ctx);
    assert!(
        rich.contains(
            "<div class=\"romeka-hero-actions\">\
             <a href=\"/s/main/item\" class=\"romeka-hero-cta\">Browse</a>\
             <a href=\"https://example.org/about\" class=\"romeka-hero-cta romeka-hero-cta-outline\">About</a></div>"
        ),
        "{rich}"
    );
    let unlabelled = Hero.render(&json!({ "cta_url": "/s/main/item" }), &ctx);
    assert!(!unlabelled.contains("<a "), "{unlabelled}");
}

/// A focal point clamps a stored percent over 100 and fills a missing one
/// with the centre; none stored means no position at all.
#[test]
fn focal_point_parses_clamps_and_defaults() {
    let parsed = HeroSettings::parse(&json!({ "focal_x": 140 }));
    assert_eq!(
        parsed.focal.map(|focal| (focal.x, focal.y)),
        Some((100, 50))
    );
    assert_eq!(HeroSettings::parse(&json!({})).focal, None);
}
