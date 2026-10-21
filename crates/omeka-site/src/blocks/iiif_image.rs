//! `BlockLayout/IiifImage.php` and
//! `view/common/block-layout/iiif-image.phtml`.
//!
//! An OpenSeadragon viewer. The template appends the library to `headScript`
//! and writes the initialiser inline. The theme owns the document head, so the
//! port loads the library from the block itself, just before the initialiser.
//! A curator may pick the viewer height ([`super::iiif`]); unset, it is
//! upstream's `400px`. The editor's preview shows a poster instead.
//!
//! The image is a URL, as upstream has it, or a media of this site, a Romeka
//! setting (`romeka.source` and `romeka.media`). A media opens through this
//! server's Image API (`/iiif/image/3/<media>/info.json`), and the write also
//! stores that address as `image_url`, so upstream opens the same image.
//!
//! Upstream builds the options with `json_encode`, which escapes `/` as `\/`.
//! The port emits `serde_json`'s output, which does not; the two strings are the
//! same JSON object, so OpenSeadragon reads either.

use omeka_core::ids::MediaId;
use omeka_core::site::{BlockData, IiifData, SiteBlock};
use serde_json::Value as JsonValue;

use crate::blocks::BlockLayout;
use crate::blocks::iiif::{height, height_field, preview_poster};
use crate::blocks::schema::{
    Accepts, ChoiceOption, LayoutSchema, When, boolean, choice, media_ref, option, text,
};
use crate::html::escape_html;
use crate::theme::library_script;
use crate::view::RenderContext;

/// The `iiifImage` block.
#[derive(Debug, Clone, Copy, Default)]
pub struct IiifImage;

/// Where the image comes from. The empty value is upstream's URL, so a block
/// that never chose stores no Romeka setting.
const SOURCE: &[ChoiceOption] = &[
    option("", "blockChoice.imageUrl"),
    option("media", "blockChoice.siteMedia"),
];

/// `BlockLayout/IiifImage.php::form`: `image_url`, `title` and the `show_title`
/// checkbox, plus Romeka's source choice, site media and height.
const SCHEMA: LayoutSchema = LayoutSchema {
    fields: &[
        choice(
            "romeka.source",
            "blockField.iiifImage.source",
            "fieldHelp.iiifImage.source",
            SOURCE,
        )
        .default_text(""),
        text(
            "image_url",
            "blockField.iiifImage.image_url",
            "fieldHelp.iiifImage.image_url",
        )
        .placeholder("fieldExample.url")
        .when(When::equals("romeka.source", "")),
        media_ref(
            "romeka.media",
            "blockField.iiifImage.media",
            "fieldHelp.iiifImage.media",
            Accepts::ImageService,
        )
        .when(When::equals("romeka.source", "media")),
        text(
            "title",
            "blockField.iiifImage.title",
            "fieldHelp.iiif.title",
        ),
        boolean(
            "show_title",
            "blockField.iiifImage.show_title",
            "fieldHelp.iiif.show_title",
        )
        .default_bool(false),
        height_field("fieldHelp.iiifImage.height"),
    ],
};

impl BlockLayout for IiifImage {
    fn name(&self) -> &'static str {
        "iiifImage"
    }

    fn label(&self) -> &'static str {
        "IIIF image"
    }

    fn render(&self, block: &SiteBlock, ctx: &RenderContext<'_>) -> String {
        let BlockData::IiifImage(data) = &block.data else {
            return String::new();
        };
        render_viewer(block, data, ctx)
    }

    fn schema(&self) -> &'static LayoutSchema {
        &SCHEMA
    }

    fn media_refs(&self, settings: &JsonValue) -> Vec<MediaId> {
        site_media(settings).into_iter().collect()
    }
}

/// The site media the settings choose, `None` for an image URL.
fn site_media(settings: &JsonValue) -> Option<MediaId> {
    if settings.get("source").and_then(JsonValue::as_str) != Some("media") {
        return None;
    }
    match settings.get("media")? {
        JsonValue::Number(number) => number.as_u64(),
        JsonValue::String(text) => text.trim().parse().ok(),
        _ => None,
    }
    .map(MediaId::new)
}

/// The Image API address of a media of this server, relative to its origin.
#[must_use]
pub fn media_info_path(media: MediaId) -> String {
    format!("/iiif/image/3/{}/info.json", media.get())
}

/// The image the viewer opens: the site media's Image API address when the
/// visitor may see that media, else the stored URL. A site media the visitor
/// may not see opens nothing.
fn tile_source(block: &SiteBlock, data: &IiifData, ctx: &RenderContext<'_>) -> Option<String> {
    let settings = block
        .layout_data
        .romeka
        .as_ref()
        .map(|carrier| &carrier.settings);
    if let Some(media) = settings.and_then(site_media) {
        return ctx
            .resources
            .read_image(media)
            .map(|_| media_info_path(media));
    }
    data.url.clone().filter(|url| !url.is_empty())
}

fn render_viewer(block: &SiteBlock, data: &IiifData, ctx: &RenderContext<'_>) -> String {
    let Some(image_url) = tile_source(block, data, ctx) else {
        return String::new();
    };
    let image_url = image_url.as_str();
    let title = data.title.clone().unwrap_or_default();
    let show_title = data.show_title.unwrap_or(false);
    let id = format!("iiif-image-{}", block.id);
    let prefix_url = format!(
        "{}/vendor/openseadragon/images/",
        ctx.asset_base.trim_end_matches('/')
    );
    let options = format!(
        "{{\"id\":{},\"prefixUrl\":{},\"tileSources\":{}}}",
        json_string(&id),
        json_string(&prefix_url),
        json_string(image_url)
    );

    let height = height(block, "400px");
    let mut out = String::new();
    if show_title && !title.is_empty() {
        out.push_str(&format!("<h3>{}</h3>", escape_html(&title)));
    }
    let name = if title.is_empty() { image_url } else { &title };
    if let Some(poster) = preview_poster(name, height, ctx) {
        out.push_str(&poster);
        return out;
    }
    out.push_str(&format!(
        "<div id=\"{}\" style=\"height: {height};\" role=\"group\" aria-label=\"{}\"></div>",
        escape_html(&id),
        escape_html(&title)
    ));
    out.push_str(&library_script(
        &ctx.asset_base,
        "vendor/openseadragon/openseadragon.min.js",
    ));
    out.push_str(&format!(
        "<script type=\"text/javascript\">OpenSeadragon({options});</script>"
    ));
    out
}

/// A JSON string literal, as `json_encode` writes it for the option values.
fn json_string(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks::test_support::{block, context, site};
    use serde_json::json;

    /// `block-layout/iiif-image.phtml`: a container with the block id, the
    /// OpenSeadragon assets under the theme's asset base, and an inline
    /// initialiser carrying the image URL.
    #[test]
    fn iiif_image_matches_upstream_block_layout() {
        let site = site();
        let ctx = context(&site, &crate::view::NoResources);
        let stored = block(
            "iiifImage",
            json!({"image_url": "https://example.org/iiif/info.json", "title": "Map", "show_title": true}),
        );
        let out = IiifImage.render(&stored, &ctx);
        assert!(out.starts_with("<h3>Map</h3>"), "{out}");
        assert!(out.contains("id=\"iiif-image-1\""), "{out}");
        assert!(out.contains("style=\"height: 400px;\""), "{out}");
        assert!(
            out.contains("\"prefixUrl\":\"/assets/vendor/openseadragon/images/\""),
            "{out}"
        );
        assert!(
            out.contains("\"tileSources\":\"https://example.org/iiif/info.json\""),
            "{out}"
        );
        assert!(
            out.contains("<script src=\"/assets/vendor/openseadragon/openseadragon.min.js?v="),
            "the block loads the library it calls: {out}"
        );
        assert!(out.ends_with("</script>"), "{out}");
    }

    /// `BlockLayout/IiifImage.php::render` returns nothing without an image URL.
    #[test]
    fn iiif_image_without_url_renders_nothing() {
        let site = site();
        let ctx = context(&site, &crate::view::NoResources);
        assert_eq!(IiifImage.render(&block("iiifImage", json!({})), &ctx), "");
    }

    /// The heading appears only when `show_title` is set and a title exists.
    #[test]
    fn iiif_image_heading_follows_show_title() {
        let site = site();
        let ctx = context(&site, &crate::view::NoResources);
        let stored = block(
            "iiifImage",
            json!({"image_url": "https://example.org/i", "title": "Map"}),
        );
        assert!(!IiifImage.render(&stored, &ctx).contains("<h3>"));
    }

    /// The preview shows the poster and loads no library; a chosen height
    /// sizes both.
    #[test]
    fn iiif_image_previews_as_a_poster_at_its_height() {
        let site = site();
        let mut ctx = context(&site, &crate::view::NoResources);
        ctx.block_markers = crate::view::BlockMarkers::Preview;
        let stored = crate::blocks::test_support::block_with_layout(
            "iiifImage",
            json!({"image_url": "https://example.org/info.json", "title": "Map"}),
            json!({"romeka": {"block": "iiifImage", "v": 1, "settings": {"height": "medium"}}}),
        );
        let out = IiifImage.render(&stored, &ctx);
        assert_eq!(
            out,
            "<div class=\"iiif-poster\" style=\"height: 600px;\"><p>IIIF viewer: Map. It opens on the public page.</p></div>"
        );
    }

    /// A site media opens through this server's Image API when the visitor
    /// may see it, whatever URL is stored beside it; one the visitor may not
    /// see opens nothing.
    #[test]
    fn iiif_image_opens_a_visible_site_media_through_the_image_api() {
        let site = site();
        let stored = crate::blocks::test_support::block_with_layout(
            "iiifImage",
            json!({"image_url": "https://example.org/stale/info.json"}),
            json!({"romeka": {"block": "iiifImage", "v": 2, "settings": {"source": "media", "media": 9}}}),
        );
        let mut resources = crate::blocks::test_support::TestResources::default();
        resources.images.insert(
            9,
            crate::view::Thumbnail::new("/files/large/abc.jpg", "A map"),
        );
        let out = IiifImage.render(&stored, &context(&site, &resources));
        assert!(
            out.contains("\"tileSources\":\"/iiif/image/3/9/info.json\""),
            "{out}"
        );
        assert_eq!(
            IiifImage.render(&stored, &context(&site, &crate::view::NoResources)),
            ""
        );
        assert_eq!(
            IiifImage.media_refs(&json!({"source": "media", "media": "9"})),
            vec![MediaId::new(9)]
        );
        assert!(
            IiifImage.media_refs(&json!({"media": 9})).is_empty(),
            "a hidden media under the URL source is not read"
        );
    }
}
