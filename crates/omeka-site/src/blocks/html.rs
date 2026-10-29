//! `BlockLayout/Html.php` and `view/common/block-layout/html.phtml`.
//!
//! The template echoes the stored `html` with no escaping. That is safe only
//! because `onHydrate` runs the value through `Stdlib\HtmlPurifier` before it is
//! stored. `RenderContext` never injects raw HTML elsewhere, and the renderer
//! purifies again so a row stored before the write path purified, or copied by
//! the migrator, still cannot execute.

use omeka_core::site::{BlockData, SiteBlock};

use crate::blocks::BlockLayout;
use crate::blocks::schema::{LayoutSchema, html};
use crate::view::RenderContext;

/// The `html` block.
#[derive(Debug, Clone, Copy, Default)]
pub struct Html;

/// `BlockLayout/Html.php::form`: one `html` textarea
/// (`application/src/Site/BlockLayout/Html.php`).
const SCHEMA: LayoutSchema = LayoutSchema {
    fields: &[html("html", "blockField.html.html", "fieldHelp.html.html")
        .placeholder("fieldExample.html.html")
        .primary()],
};

impl BlockLayout for Html {
    fn name(&self) -> &'static str {
        "html"
    }

    fn label(&self) -> &'static str {
        "HTML"
    }

    fn render(&self, block: &SiteBlock, ctx: &RenderContext<'_>) -> String {
        if let Some(render) = crate::blocks::romeka::render_carrier(block, ctx) {
            return match render {
                crate::blocks::romeka::CarrierRender::Rich(html)
                | crate::blocks::romeka::CarrierRender::Edited(html) => html,
            };
        }
        match &block.data {
            BlockData::Html { html } => omeka_core::purify::purify(html),
            _ => String::new(),
        }
    }

    fn schema(&self) -> &'static LayoutSchema {
        &SCHEMA
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks::test_support::{block, context, site};
    use serde_json::json;

    /// `block-layout/html.phtml` echoes the stored HTML, purified at hydrate
    /// time (`BlockLayout/Html.php::onHydrate`).
    #[test]
    fn html_matches_upstream_block_layout() {
        let block = block("html", json!({"html": "<p>Hello <em>world</em></p>"}));
        let site = site();
        let ctx = context(&site, &crate::view::NoResources);
        assert_eq!(Html.render(&block, &ctx), "<p>Hello <em>world</em></p>");
    }

    /// A block whose parsed data is another layout's renders nothing.
    #[test]
    fn html_ignores_other_layout_data() {
        let block = block("lineBreak", json!({"break_type": "opaque"}));
        let site = site();
        let ctx = context(&site, &crate::view::NoResources);
        assert_eq!(Html.render(&block, &ctx), "");
    }

    /// A body stored before the write path purified (or copied by the migrator)
    /// is purified again at render, so it still cannot execute.
    #[test]
    fn html_render_purifies_a_dangerous_stored_body() {
        let block = block(
            "html",
            json!({"html": "<p>ok</p><script>alert(1)</script><img src=x onerror=alert(2)>"}),
        );
        let site = site();
        let ctx = context(&site, &crate::view::NoResources);
        let out = Html.render(&block, &ctx);
        assert!(!out.contains("<script"), "{out}");
        assert!(!out.contains("onerror"), "{out}");
        assert!(out.contains("<p>ok</p>"), "{out}");
    }
}
