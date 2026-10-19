//! `BlockLayout/IiifImage.php` and
//! `view/common/block-layout/iiif-image.phtml`.
//!
//! An OpenSeadragon viewer. The template appends the library to `headScript`
//! and writes the initialiser inline; the port writes the initialiser only,
//! because the theme owns the document head and loads the library once.
//!
//! Upstream builds the options with `json_encode`, which escapes `/` as `\/`.
//! The port emits `serde_json`'s output, which does not; the two strings are the
//! same JSON object, so OpenSeadragon reads either.

use omeka_core::site::{BlockData, IiifData, SiteBlock};

use crate::blocks::BlockLayout;
use crate::blocks::schema::{LayoutSchema, boolean, text};
use crate::html::escape_html;
use crate::view::RenderContext;

/// The `iiifImage` block.
#[derive(Debug, Clone, Copy, Default)]
pub struct IiifImage;

/// `BlockLayout/IiifImage.php::form`: `image_url`, `title` and the `show_title`
/// checkbox.
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

    let mut out = String::new();
    if show_title && !title.is_empty() {
        out.push_str(&format!("<h3>{}</h3>", escape_html(&title)));
    }
    out.push_str(&format!(
        "<div id=\"{}\" style=\"height: 400px;\" role=\"group\" aria-label=\"{}\"></div>",
        escape_html(&id),
        escape_html(&title)
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

    /// `block-layout/iiif-image.phtml`: a container with the block id, the    /// OpenSeadragon assets under the theme's asset base, and an inline
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
}
