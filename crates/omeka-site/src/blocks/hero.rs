//! The `hero` Romeka block: a full-width banner with a background image, an
//! overlay, a heading, optional copy and one call to action.
//!
//! The block is stored as an upstream `html` block. Its plain rendering is the
//! `<h2>`, the paragraphs, the image and the link in upstream's own vocabulary,
//! so upstream shows the same content without this product's styling. The rich
//! rendering is the banner the bundled theme styles (`.romeka-hero` in
//! `theme/style.css`).

use omeka_core::ids::{AssetId, PageId};
use serde_json::Value as JsonValue;

use super::romeka::{PlainRefs, RomkaBlock};
use super::schema::{ChoiceOption, LayoutSchema, asset_ref, choice, html, link, option, text};
use crate::html::{Attr, escape_html, hyperlink, open_tag};
use crate::slug::page_url;
use crate::view::RenderContext;

/// The `hero` block.
#[derive(Debug, Clone, Copy, Default)]
pub struct Hero;

/// `overlay` and `alignment` values, in form order.
const OVERLAY: &[ChoiceOption] = &[
    option("none", "romekaChoice.none"),
    option("light", "romekaChoice.light"),
    option("medium", "romekaChoice.medium"),
    option("strong", "romekaChoice.strong"),
];

const ALIGNMENT: &[ChoiceOption] = &[
    option("left", "blockChoice.left"),
    option("center", "blockChoice.center"),
    option("right", "blockChoice.right"),
];

/// The block's settings fields, in the order its form shows them.
const SCHEMA: LayoutSchema = LayoutSchema {
    fields: &[
        text(
            "heading",
            "romekaField.hero.heading",
            "fieldHelp.hero.heading",
        )
        .placeholder("fieldExample.hero.heading")
        .required(),
        text(
            "subheading",
            "romekaField.hero.subheading",
            "fieldHelp.hero.subheading",
        ),
        html("body", "romekaField.hero.body", "fieldHelp.hero.body"),
        asset_ref(
            "background",
            "romekaField.hero.background",
            "fieldHelp.hero.background",
        ),
        choice(
            "overlay",
            "romekaField.hero.overlay",
            "fieldHelp.hero.overlay",
            OVERLAY,
        )
        .default_text("medium"),
        choice(
            "alignment",
            "romekaField.hero.alignment",
            "fieldHelp.hero.alignment",
            ALIGNMENT,
        )
        .default_text("left"),
        text(
            "cta_label",
            "romekaField.hero.cta_label",
            "fieldHelp.hero.cta_label",
        )
        .placeholder("fieldExample.hero.cta_label"),
        link(
            "cta_page",
            "cta_url",
            "romekaField.hero.cta_page",
            "fieldHelp.hero.cta_page",
        ),
    ],
};

impl RomkaBlock for Hero {
    fn name(&self) -> &'static str {
        "hero"
    }

    fn label(&self) -> &'static str {
        "romeka.hero"
    }

    fn schema(&self) -> &'static LayoutSchema {
        &SCHEMA
    }

    fn render(&self, settings: &JsonValue, ctx: &RenderContext<'_>) -> String {
        let settings = HeroSettings::parse(settings);
        let mut classes = format!(
            "romeka-hero romeka-hero-align-{} romeka-hero-overlay-{}",
            settings.alignment.as_str(),
            settings.overlay.as_str()
        );
        if settings.background.is_none() {
            classes.push_str(" romeka-hero-no-image");
        }
        let mut out = open_tag("section", &[Attr::text("class", &classes)]);
        if let Some(asset) = settings
            .background
            .and_then(|id| ctx.resources.read_asset(id))
            && let Some(url) = &asset.url
        {
            out.push_str(&open_tag(
                "img",
                &[
                    Attr::text("class", "romeka-hero-background"),
                    Attr::text("src", url),
                    Attr::text("alt", ""),
                ],
            ));
        }
        out.push_str("<div class=\"romeka-hero-overlay\" aria-hidden=\"true\"></div>");
        out.push_str("<div class=\"romeka-hero-content\">");
        if !settings.heading.is_empty() {
            out.push_str(&format!(
                "<h2 class=\"romeka-hero-heading\">{}</h2>",
                escape_html(&settings.heading)
            ));
        }
        if !settings.subheading.is_empty() {
            out.push_str(&format!(
                "<p class=\"romeka-hero-subheading\">{}</p>",
                escape_html(&settings.subheading)
            ));
        }
        if !settings.body.is_empty() {
            out.push_str(&format!(
                "<div class=\"romeka-hero-body\">{}</div>",
                omeka_core::purify::purify(&settings.body)
            ));
        }
        if let Some(href) = settings.href(ctx) {
            out.push_str(&hyperlink(
                &settings.cta_label,
                Some(&href),
                &[Attr::text("class", "romeka-hero-cta")],
            ));
        }
        out.push_str("</div></section>");
        out
    }

    fn plain(&self, settings: &JsonValue, refs: &PlainRefs) -> String {
        let settings = HeroSettings::parse(settings);
        let mut out = String::new();
        if !settings.heading.is_empty() {
            out.push_str(&format!("<h2>{}</h2>", escape_html(&settings.heading)));
        }
        if !settings.subheading.is_empty() {
            out.push_str(&format!("<p>{}</p>", escape_html(&settings.subheading)));
        }
        if !settings.body.is_empty() {
            out.push_str(&omeka_core::purify::purify(&settings.body));
        }
        if let Some(asset) = settings.background.and_then(|id| refs.asset_of(id)) {
            out.push_str(&open_tag(
                "img",
                &[Attr::text("src", &asset.url), Attr::text("alt", &asset.alt)],
            ));
        }
        if let Some(href) = settings.plain_href(refs)
            && !settings.cta_label.is_empty()
        {
            out.push_str(&hyperlink(&settings.cta_label, Some(&href), &[]));
        }
        out
    }

    fn asset_refs(&self, settings: &JsonValue) -> Vec<AssetId> {
        HeroSettings::parse(settings)
            .background
            .into_iter()
            .collect()
    }

    fn page_refs(&self, settings: &JsonValue) -> Vec<PageId> {
        HeroSettings::parse(settings).cta_page.into_iter().collect()
    }
}

/// The block's typed settings.
#[derive(Debug, Clone, PartialEq, Eq)]
struct HeroSettings {
    heading: String,
    subheading: String,
    body: String,
    background: Option<AssetId>,
    overlay: Overlay,
    alignment: Alignment,
    cta_label: String,
    cta_page: Option<PageId>,
    cta_url: String,
}

/// The overlay strength: how much the dark layer dims the background image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Overlay {
    None,
    Light,
    Medium,
    Strong,
}

impl Overlay {
    fn parse(value: &JsonValue) -> Self {
        match value.as_str() {
            Some("none") => Self::None,
            Some("light") => Self::Light,
            Some("strong") => Self::Strong,
            _ => Self::Medium,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Light => "light",
            Self::Medium => "medium",
            Self::Strong => "strong",
        }
    }
}

/// Where the banner's content sits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Alignment {
    Left,
    Center,
    Right,
}

impl Alignment {
    fn parse(value: &JsonValue) -> Self {
        match value.as_str() {
            Some("center") => Self::Center,
            Some("right") => Self::Right,
            _ => Self::Left,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Center => "center",
            Self::Right => "right",
        }
    }
}

impl HeroSettings {
    fn parse(settings: &JsonValue) -> Self {
        let text_of = |key: &str| {
            settings
                .get(key)
                .and_then(JsonValue::as_str)
                .unwrap_or("")
                .to_string()
        };
        Self {
            heading: text_of("heading"),
            subheading: text_of("subheading"),
            body: text_of("body"),
            background: settings.get("background").and_then(id_of),
            overlay: settings
                .get("overlay")
                .map_or(Overlay::Medium, Overlay::parse),
            alignment: settings
                .get("alignment")
                .map_or(Alignment::Left, Alignment::parse),
            cta_label: text_of("cta_label"),
            cta_page: settings.get("cta_page").and_then(page_id_of),
            cta_url: text_of("cta_url"),
        }
    }

    /// The call to action's target on the rendered page: the stored page when
    /// it is still in the site, else the URL when it is a safe one.
    fn href(&self, ctx: &RenderContext<'_>) -> Option<String> {
        if let Some(id) = self.cta_page
            && let Some(page) = ctx.pages.iter().find(|page| page.id == id)
        {
            return Some(page_url(&ctx.site.slug, &page.slug));
        }
        safe_url(&self.cta_url)
    }

    /// The same target at write time, where only the resolved page URLs are
    /// available.
    fn plain_href(&self, refs: &PlainRefs) -> Option<String> {
        if let Some(id) = self.cta_page
            && let Some(url) = refs.page_of(id)
        {
            return Some(url.to_string());
        }
        safe_url(&self.cta_url)
    }
}

/// An id from a JSON number or a numeric string, the two shapes a form and a
/// stored document use.
fn id_of(value: &JsonValue) -> Option<AssetId> {
    match value {
        JsonValue::Number(number) => number.as_u64().map(AssetId::new),
        JsonValue::String(text) => text.trim().parse::<u64>().ok().map(AssetId::new),
        _ => None,
    }
}

/// A page id, the same two shapes.
fn page_id_of(value: &JsonValue) -> Option<PageId> {
    match value {
        JsonValue::Number(number) => number.as_u64().map(PageId::new),
        JsonValue::String(text) => text.trim().parse::<u64>().ok().map(PageId::new),
        _ => None,
    }
}

/// A URL safe to put in an `href`: relative, or an absolute http, https or
/// mailto URL. A stored `javascript:` or `data:` target renders nothing.
pub(crate) fn safe_url(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    match trimmed.split_once(':') {
        Some((scheme, _)) if !scheme.contains('/') => {
            let scheme = scheme.to_ascii_lowercase();
            matches!(scheme.as_str(), "http" | "https" | "mailto").then(|| trimmed.to_string())
        }
        _ => Some(trimmed.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks::romeka::{CarrierRender, plain_hash, render_carrier};
    use crate::blocks::test_support::{block_with_layout, context, site};
    use crate::view::{AssetView, NoResources, Thumbnail};

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
                "class=\"romeka-hero romeka-hero-align-center romeka-hero-overlay-strong\""
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

    /// A background asset resolves to an image behind the overlay.
    #[test]
    fn rich_renders_the_background_image() {
        let mut resources = crate::blocks::test_support::TestResources::default();
        resources.assets.insert(
            3,
            AssetView {
                id: AssetId::new(3),
                title: "A shelf".to_string(),
                url: Some("/files/asset/bg.png".to_string()),
                thumbnail: Some(Thumbnail::new("/files/asset/bg.png", "A shelf")),
            },
        );
        let site = site();
        let ctx = context(&site, &resources);
        let rich = Hero.render(&settings(), &ctx);
        assert!(
            rich.contains(
                "<img class=\"romeka-hero-background\" src=\"/files/asset/bg.png\" alt=\"\">"
            ),
            "{rich}"
        );
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
            HeroSettings::parse(&settings()).plain_href(&refs()),
            Some("/s/main/page/about".to_string())
        );
        let url_only = HeroSettings::parse(&json!({
            "cta_label": "Go",
            "cta_url": "https://example.org/x"
        }));
        assert_eq!(
            url_only.plain_href(&PlainRefs::new()),
            Some("https://example.org/x".to_string())
        );
        let hostile = HeroSettings::parse(&json!({
            "cta_label": "Go",
            "cta_url": "javascript:alert(1)"
        }));
        assert_eq!(hostile.plain_href(&PlainRefs::new()), None);
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
        assert_eq!(parsed.background, Some(AssetId::new(3)));
        assert_eq!(parsed.cta_page, Some(PageId::new(2)));
        assert_eq!(parsed.overlay, Overlay::Medium);
        assert_eq!(parsed.alignment, Alignment::Left);
        assert_eq!(parsed.background.unwrap().get(), 3);
    }
}
