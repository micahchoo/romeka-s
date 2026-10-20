//! The `hero` Romeka block: a full-width banner with a background image, an
//! overlay, a heading, optional copy and up to two calls to action.
//!
//! The block is stored as an upstream `html` block. Its plain rendering is the
//! heading, the paragraphs, the image and the links in upstream's own
//! vocabulary, so upstream shows the same content without this product's
//! styling. The rich rendering is the banner the bundled theme styles
//! (`.romeka-hero` in `theme/style.css`).
//!
//! The background is an asset or a media of the site. A media must be public:
//! the write refuses a private one, because the plain rendering names its file
//! on a public page.

mod settings;

use omeka_core::ids::{AssetId, MediaId, PageId};
use serde_json::Value as JsonValue;

use super::romeka::{PlainImage, PlainRefs, RomkaBlock};
use super::schema::{
    ChoiceOption, LayoutSchema, When, asset_ref, choice, focal_point, html, link, media_ref,
    option, text, theme_color,
};
use crate::html::{Attr, escape_html, hyperlink, open_tag};
use crate::view::{RenderContext, Thumbnail};
use settings::{Background, HeroSettings};

/// The `hero` block.
#[derive(Debug, Clone, Copy, Default)]
pub struct Hero;

const SOURCE: &[ChoiceOption] = &[
    option("asset", "romekaChoice.backgroundAsset"),
    option("media", "romekaChoice.backgroundMedia"),
];

const HEIGHT: &[ChoiceOption] = &[
    option("short", "romekaChoice.short"),
    option("medium", "romekaChoice.medium"),
    option("tall", "romekaChoice.tall"),
    option("full", "romekaChoice.fullScreen"),
];

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

const HEADING_LEVEL: &[ChoiceOption] = &[
    option("h2", "romekaChoice.h2"),
    option("h1", "romekaChoice.h1"),
];

/// The image fields the focal point reads, in the order the control tries them.
const IMAGES: &[&str] = &["background", "background_media"];

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
        choice(
            "heading_level",
            "romekaField.hero.heading_level",
            "fieldHelp.hero.heading_level",
            HEADING_LEVEL,
        )
        .default_text("h2"),
        text(
            "subheading",
            "romekaField.hero.subheading",
            "fieldHelp.hero.subheading",
        ),
        html("body", "romekaField.hero.body", "fieldHelp.hero.body"),
        choice(
            "background_source",
            "romekaField.hero.background_source",
            "fieldHelp.hero.background_source",
            SOURCE,
        )
        .default_text("asset"),
        asset_ref(
            "background",
            "romekaField.hero.background",
            "fieldHelp.hero.background",
        )
        .when(When::equals("background_source", "asset")),
        media_ref(
            "background_media",
            "romekaField.hero.background_media",
            "fieldHelp.hero.background_media",
        )
        .when(When::equals("background_source", "media")),
        focal_point(
            "focal_x",
            "focal_y",
            "romekaField.hero.focal",
            "fieldHelp.hero.focal",
            IMAGES,
        ),
        choice(
            "height",
            "romekaField.hero.height",
            "fieldHelp.hero.height",
            HEIGHT,
        )
        .default_text("medium"),
        theme_color(
            "overlay_color",
            "overlay_color",
            "romekaField.hero.overlay_color",
            "fieldHelp.hero.overlay_color",
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
        text(
            "cta2_label",
            "romekaField.hero.cta2_label",
            "fieldHelp.hero.cta2_label",
        )
        .placeholder("fieldExample.hero.cta2_label"),
        link(
            "cta2_page",
            "cta2_url",
            "romekaField.hero.cta2_page",
            "fieldHelp.hero.cta2_page",
        ),
    ],
};

/// The class each button carries, the second drawn as an outline.
const BUTTON_CLASSES: [&str; 2] = ["romeka-hero-cta", "romeka-hero-cta romeka-hero-cta-outline"];

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
        let image = background_image(settings.background, ctx);
        let mut classes = format!(
            "romeka-hero romeka-hero-align-{} romeka-hero-overlay-{} romeka-hero-height-{}",
            settings.alignment.as_str(),
            settings.overlay.as_str(),
            settings.height.as_str(),
        );
        if image.is_none() {
            classes.push_str(" romeka-hero-no-image");
        }
        let mut out = open_tag("section", &[Attr::text("class", &classes)]);
        if let Some(image) = &image {
            let focal = settings.focal.map(settings::Focal::css);
            let mut attrs = vec![
                Attr::text("class", "romeka-hero-background"),
                Attr::text("src", &image.url),
                Attr::text("alt", &image.alt),
            ];
            if let Some(focal) = &focal {
                attrs.push(Attr::text("style", focal));
            }
            out.push_str(&open_tag("img", &attrs));
        }
        let overlay_style = settings
            .overlay_color
            .as_ref()
            .map(|colour| format!("background: {}", colour.css()));
        let mut overlay = vec![
            Attr::text("class", "romeka-hero-overlay"),
            Attr::text("aria-hidden", "true"),
        ];
        if let Some(style) = &overlay_style {
            overlay.push(Attr::text("style", style));
        }
        out.push_str(&open_tag("div", &overlay));
        out.push_str("</div>");
        out.push_str("<div class=\"romeka-hero-content\">");
        if !settings.heading.is_empty() {
            let tag = settings.heading_level.tag();
            out.push_str(&format!(
                "<{tag} class=\"romeka-hero-heading\">{}</{tag}>",
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
        let buttons: String = settings
            .buttons
            .iter()
            .zip(BUTTON_CLASSES)
            .filter_map(|(button, class)| {
                let href = button.href(ctx)?;
                Some(hyperlink(
                    &button.label,
                    Some(&href),
                    &[Attr::text("class", class)],
                ))
            })
            .collect();
        if !buttons.is_empty() {
            out.push_str(&format!(
                "<div class=\"romeka-hero-actions\">{buttons}</div>"
            ));
        }
        out.push_str("</div></section>");
        out
    }

    fn plain(&self, settings: &JsonValue, refs: &PlainRefs) -> String {
        let settings = HeroSettings::parse(settings);
        let mut out = String::new();
        if !settings.heading.is_empty() {
            let tag = settings.heading_level.tag();
            out.push_str(&format!(
                "<{tag}>{}</{tag}>",
                escape_html(&settings.heading)
            ));
        }
        if !settings.subheading.is_empty() {
            out.push_str(&format!("<p>{}</p>", escape_html(&settings.subheading)));
        }
        if !settings.body.is_empty() {
            out.push_str(&omeka_core::purify::purify(&settings.body));
        }
        if let Some(image) = plain_image(settings.background, refs) {
            out.push_str(&open_tag(
                "img",
                &[Attr::text("src", &image.url), Attr::text("alt", &image.alt)],
            ));
        }
        for button in &settings.buttons {
            if let Some(href) = button.plain_href(refs) {
                out.push_str(&hyperlink(&button.label, Some(&href), &[]));
            }
        }
        out
    }

    fn asset_refs(&self, settings: &JsonValue) -> Vec<AssetId> {
        match HeroSettings::parse(settings).background {
            Background::Asset(id) => vec![id],
            Background::Media(_) | Background::None => Vec::new(),
        }
    }

    fn page_refs(&self, settings: &JsonValue) -> Vec<PageId> {
        HeroSettings::parse(settings).page_refs()
    }

    fn media_refs(&self, settings: &JsonValue) -> Vec<MediaId> {
        match HeroSettings::parse(settings).background {
            Background::Media(id) => vec![id],
            Background::Asset(_) | Background::None => Vec::new(),
        }
    }
}

/// The background on the rendered page, with the alt text its file carries.
fn background_image(background: Background, ctx: &RenderContext<'_>) -> Option<Thumbnail> {
    match background {
        Background::None => None,
        Background::Asset(id) => {
            let asset = ctx.resources.read_asset(id)?;
            let url = asset.url?;
            let alt = asset
                .thumbnail
                .map(|thumbnail| thumbnail.alt)
                .unwrap_or_default();
            Some(Thumbnail::new(url, alt))
        }
        Background::Media(id) => ctx.resources.read_image(id),
    }
}

/// The background at write time, from the resolved references.
fn plain_image(background: Background, refs: &PlainRefs) -> Option<&PlainImage> {
    match background {
        Background::None => None,
        Background::Asset(id) => refs.asset_of(id),
        Background::Media(id) => refs.media_of(id),
    }
}

#[cfg(test)]
mod tests;
