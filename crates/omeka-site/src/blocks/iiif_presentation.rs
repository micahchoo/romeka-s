//! `BlockLayout/IiifPresentation.php` and
//! `view/common/block-layout/iiif-presentation.phtml`.
//!
//! The template builds two viewer query parameters and renders the IIIF viewer
//! helper, which is an iframe to the `iiif-viewer` route
//! (`View/Helper/IiifViewer.php`, `routes.config.php:370-379`). The port keeps
//! the query: `url` is the manifest, `mirador_config` carries the sidebar and
//! theme settings. The theme setting `iiif_viewer_theme` maps to Mirador's
//! `selectedTheme`, and anything but `dark` becomes `light`.

use omeka_core::site::{BlockData, IiifData, SiteBlock};

use crate::blocks::BlockLayout;
use crate::blocks::schema::{LayoutSchema, boolean, text};
use crate::html::{escape_html, query_string};
use crate::view::RenderContext;

/// The `iiifPresentation` block.
#[derive(Debug, Clone, Copy, Default)]
pub struct IiifPresentation;

/// `BlockLayout/IiifPresentation.php::form`: `manifest_url`, `title` and the
/// `show_title` checkbox.
const SCHEMA: LayoutSchema = LayoutSchema {
    fields: &[
        text("manifest_url", "blockField.iiifPresentation.manifest_url"),
        text("title", "blockField.iiifPresentation.title"),
        boolean("show_title", "blockField.iiifPresentation.show_title").default_bool(false),
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
        render_viewer(data, ctx)
    }

    fn schema(&self) -> &'static LayoutSchema {
        &SCHEMA
    }
}

fn render_viewer(data: &IiifData, ctx: &RenderContext<'_>) -> String {
    let Some(manifest_url) = data.url.as_deref().filter(|url| !url.is_empty()) else {
        return String::new();
    };
    let title = data.title.clone().unwrap_or_default();
    let show_title = data.show_title.unwrap_or(false);

    let theme = if ctx.settings.iiif_viewer_theme() == "dark" {
        "dark"
    } else {
        "light"
    };
    // Built as a string rather than through a map, so the key order matches
    // `json_encode`'s insertion order whatever serde_json's map type is.
    let mirador = format!(
        "{{\"window.sideBarOpen\":false,\"selectedTheme\":\"{}\"}}",
        theme
    );
    let query = query_string(&[
        ("url".to_string(), manifest_url.to_string()),
        ("mirador_config".to_string(), mirador),
    ])
    .unwrap_or_default();
    let src = format!("{}?{query}", ctx.iiif_viewer_url.trim_end_matches('?'));

    let mut out = String::new();
    if show_title && !title.is_empty() {
        out.push_str(&format!("<h3>{}</h3>", escape_html(&title)));
    }
    out.push_str(&format!(
        "<iframe style=\"width: 100%; height: 700px;\" aria-label=\"{}\" src=\"{}\"></iframe>",
        escape_html(&title),
        escape_html(&src)
    ));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks::test_support::{block, context, site};
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
}
