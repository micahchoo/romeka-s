//! `BlockLayout/IiifImage.php` and
//! `view/common/block-layout/iiif-image.phtml`.
//!
//! An OpenSeadragon viewer. The template appends the library to `headScript`
//! and writes the initialiser inline. The theme owns the document head, so the
//! port loads the library from the block itself, just before the initialiser.
//! A curator may pick the viewer height ([`super::iiif`]); unset, it is
//! upstream's `400px`. The editor's preview shows a poster instead.
//!
//! Upstream builds the options with `json_encode`, which escapes `/` as `\/`.
//! The port emits `serde_json`'s output, which does not; the two strings are the
//! same JSON object, so OpenSeadragon reads either.

use omeka_core::site::{BlockData, IiifData, SiteBlock};

use crate::blocks::BlockLayout;
use crate::blocks::iiif::{height, height_field, preview_poster};
use crate::blocks::schema::{LayoutSchema, boolean, text};
use crate::html::escape_html;
use crate::theme::library_script;
use crate::view::RenderContext;

/// The `iiifImage` block.
#[derive(Debug, Clone, Copy, Default)]
pub struct IiifImage;

/// `BlockLayout/IiifImage.php::form`: `image_url`, `title` and the `show_title`
/// checkbox, plus Romeka's height.
const SCHEMA: LayoutSchema = LayoutSchema {
    fields: &[
        text(
            "image_url",
            "blockField.iiifImage.image_url",
            "fieldHelp.iiifImage.image_url",
        )
        .placeholder("fieldExample.url"),
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
}

fn render_viewer(block: &SiteBlock, data: &IiifData, ctx: &RenderContext<'_>) -> String {
    let Some(image_url) = data.url.as_deref().filter(|url| !url.is_empty()) else {
        return String::new();
    };
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
}
