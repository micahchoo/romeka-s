//! `BlockLayout/Html.php` and `view/common/block-layout/html.phtml`.
//!
//! The template echoes the stored `html` with no escaping. That is safe only
//! because `onHydrate` runs the value through `Stdlib\HtmlPurifier` before it is
//! stored, so the site layer emits what the storage boundary already cleansed.
//! A caller that stores block data without purifying it reopens that hole; the
//! port has no purifier in `omeka-site`, and `RenderContext` never injects raw
//! HTML elsewhere.

use omeka_core::site::{BlockData, SiteBlock};

use crate::blocks::BlockLayout;
use crate::view::RenderContext;

/// The `html` block.
#[derive(Debug, Clone, Copy, Default)]
pub struct Html;

impl BlockLayout for Html {
    fn name(&self) -> &'static str {
        "html"
    }

    fn label(&self) -> &'static str {
        "HTML"
    }

    fn render(&self, block: &SiteBlock, _ctx: &RenderContext<'_>) -> String {
        match &block.data {
            BlockData::Html { html } => html.clone(),
            _ => String::new(),
        }
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
}
