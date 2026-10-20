//! `BlockLayout/IiifPresentation.php` and
//! `view/common/block-layout/iiif-presentation.phtml`.
//!
//! The template builds two viewer query parameters and renders the IIIF viewer
//! helper, which is an iframe to the `iiif-viewer` route
//! (`View/Helper/IiifViewer.php`, `routes.config.php:370-379`). The port keeps
//! the query: `url` is the manifest, `mirador_config` carries the sidebar and
//! theme settings. The theme setting `iiif_viewer_theme` maps to Mirador's
//! `selectedTheme`, and anything but `dark` becomes `light`.
//!
//! Two departures, both Romeka's. The site setting `iiif_viewer_sidebar`
//! drives `window.sideBarOpen`, where upstream's block always sends `false`
//! and only its media renderer reads the setting. And a curator may pick the
//! viewer height ([`super::iiif`]); unset, it is upstream's `700px`.

use omeka_core::site::{BlockData, IiifData, SiteBlock};

use crate::blocks::BlockLayout;
use crate::blocks::iiif::{height, height_field, preview_poster};
use crate::blocks::schema::{LayoutSchema, boolean, manifest_source, text};
use crate::html::escape_html;
use crate::iiif_viewer::viewer_src;
use crate::view::RenderContext;

/// The `iiifPresentation` block.
#[derive(Debug, Clone, Copy, Default)]
pub struct IiifPresentation;

/// `BlockLayout/IiifPresentation.php::form`: `manifest_url`, `title` and the
/// `show_title` checkbox, plus Romeka's height. The manifest may be chosen as
/// an item of this site, which stores that item's manifest URL.
const SCHEMA: LayoutSchema = LayoutSchema {
    fields: &[
        manifest_source(
            "manifest_url",
            "blockField.iiifPresentation.manifest_url",
            "fieldHelp.iiifPresentation.manifest_url",
        ),
        text(
            "title",
            "blockField.iiifPresentation.title",
            "fieldHelp.iiif.title",
        ),
        boolean(
            "show_title",
            "blockField.iiifPresentation.show_title",
            "fieldHelp.iiif.show_title",
        )
        .default_bool(false),
        height_field("fieldHelp.iiifPresentation.height"),
    ],
};

impl BlockLayout for IiifPresentation {
    fn name(&self) -> &'static str {
        "iiifPresentation"
    }

    fn label(&self) -> &'static str {
        "IIIF presentation"
    }

    fn render(&self, block: &SiteBlock, ctx: &RenderContext<'_>) -> String {
        let BlockData::IiifPresentation(data) = &block.data else {
            return String::new();
        };
        render_viewer(block, data, ctx)
    }

    fn schema(&self) -> &'static LayoutSchema {
        &SCHEMA
    }
}

fn render_viewer(block: &SiteBlock, data: &IiifData, ctx: &RenderContext<'_>) -> String {
    let Some(manifest_url) = data.url.as_deref().filter(|url| !url.is_empty()) else {
        return String::new();
    };
    let title = data.title.clone().unwrap_or_default();
    let show_title = data.show_title.unwrap_or(false);

    let src = viewer_src(&ctx.iiif_viewer_url, manifest_url, &ctx.settings);
    let height = height(block, "700px");
    let mut out = String::new();
    if show_title && !title.is_empty() {
        out.push_str(&format!("<h3>{}</h3>", escape_html(&title)));
    }
    let name = if title.is_empty() {
        manifest_url
    } else {
        &title
    };
    match preview_poster(name, height, ctx) {
        Some(poster) => out.push_str(&poster),
        None => out.push_str(&format!(
            "<iframe style=\"width: 100%; height: {height};\" aria-label=\"{}\" src=\"{}\"></iframe>",
            escape_html(&title),
            escape_html(&src)
        )),
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks::test_support::{block, block_with_layout, context, site};
    use crate::settings::SiteSettings;
    use serde_json::json;

    /// `block-layout/iiif-presentation.phtml` and `IiifViewer`: an iframe to the
    /// viewer route, carrying the manifest URL and the Mirador config.
    #[test]
    fn iiif_presentation_matches_upstream_block_layout() {
        let site = site();
        let ctx = context(&site, &crate::view::NoResources);
        let stored = block(
            "iiifPresentation",
            json!({"manifest_url": "https://example.org/manifest", "title": "Album", "show_title": true}),
        );
        let out = IiifPresentation.render(&stored, &ctx);
        assert!(out.starts_with("<h3>Album</h3>"), "{out}");
        assert!(out.contains("width: 100%; height: 700px;"), "{out}");
        assert!(
            out.contains("src=\"/iiif-viewer?url=https%3A%2F%2Fexample.org%2Fmanifest&amp;mirador_config=%7B%22window.sideBarOpen%22%3Afalse%2C%22selectedTheme%22%3A%22light%22%7D\""),
            "{out}"
        );
    }

    /// `iiif_viewer_theme` maps to Mirador's `selectedTheme`; a theme other than
    /// `dark` is `light`.
    #[test]
    fn iiif_presentation_reads_the_viewer_theme() {
        let site = site();
        let mut ctx = context(&site, &crate::view::NoResources);
        ctx.settings = SiteSettings {
            iiif_viewer_theme: Some("dark".to_string()),
            ..SiteSettings::default()
        };
        let stored = block(
            "iiifPresentation",
            json!({"manifest_url": "https://example.org/m"}),
        );
        let out = IiifPresentation.render(&stored, &ctx);
        assert!(out.contains("selectedTheme%22%3A%22dark"), "{out}");
        assert!(!out.contains("<h3>"), "{out} is untitled");
    }

    /// `BlockLayout/IiifPresentation.php::render` returns nothing without a
    /// manifest URL.
    #[test]
    fn iiif_presentation_without_url_renders_nothing() {
        let site = site();
        let ctx = context(&site, &crate::view::NoResources);
        assert_eq!(
            IiifPresentation.render(&block("iiifPresentation", json!({})), &ctx),
            ""
        );
    }

    /// Romeka: the site's `iiif_viewer_sidebar` opens Mirador's sidebar, where
    /// upstream's block always closes it.
    #[test]
    fn iiif_presentation_reads_the_sidebar_setting() {
        let site = site();
        let mut ctx = context(&site, &crate::view::NoResources);
        ctx.settings = SiteSettings {
            iiif_viewer_sidebar: Some(true),
            ..SiteSettings::default()
        };
        let stored = block(
            "iiifPresentation",
            json!({"manifest_url": "https://example.org/m"}),
        );
        let out = IiifPresentation.render(&stored, &ctx);
        assert!(out.contains("window.sideBarOpen%22%3Atrue"), "{out}");
    }

    /// The height a curator picks in `o:layout_data.romeka` sizes the viewer.
    #[test]
    fn iiif_presentation_takes_the_chosen_height() {
        let site = site();
        let ctx = context(&site, &crate::view::NoResources);
        let stored = block_with_layout(
            "iiifPresentation",
            json!({"manifest_url": "https://example.org/m"}),
            json!({"romeka": {"block": "iiifPresentation", "v": 1, "settings": {"height": "full"}}}),
        );
        let out = IiifPresentation.render(&stored, &ctx);
        assert!(out.contains("width: 100%; height: 100vh;"), "{out}");
    }

    /// The editor's preview runs no script, so it shows a poster at the
    /// viewer's height in place of the iframe.
    #[test]
    fn iiif_presentation_previews_as_a_poster() {
        let site = site();
        let mut ctx = context(&site, &crate::view::NoResources);
        ctx.block_markers = crate::view::BlockMarkers::Preview;
        let stored = block(
            "iiifPresentation",
            json!({"manifest_url": "https://example.org/m", "title": "Album"}),
        );
        let out = IiifPresentation.render(&stored, &ctx);
        assert!(!out.contains("<iframe"), "{out}");
        assert!(
            out.contains("<div class=\"iiif-poster\" style=\"height: 700px;\"><p>IIIF viewer: Album. It opens on the public page.</p></div>"),
            "{out}"
        );
        let untitled = block(
            "iiifPresentation",
            json!({"manifest_url": "https://example.org/m"}),
        );
        assert!(
            IiifPresentation
                .render(&untitled, &ctx)
                .contains("IIIF viewer: https://example.org/m."),
            "an untitled block names its manifest"
        );
    }
}
