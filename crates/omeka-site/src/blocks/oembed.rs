//! `BlockLayout/Oembed.php`, `Stdlib/Oembed.php::renderOembed` and
//! `view/common/block-layout/oembed.phtml`.
//!
//! The template wraps the cached oEmbed response in a `<div>`. `renderOembed`
//! returns a provider's `html` field verbatim when the response has one, and
//! otherwise an `<img>` for a `photo` response. Both are deliberate: the HTML
//! comes from the oEmbed provider, which is the same trust upstream extends.
//! Everything the port builds itself is escaped.
//!
//! `renderOembed`'s `alt` reads an undefined `$data` and falls back to the
//! image URL (`Stdlib/Oembed.php:110`); the port uses the URL.

use omeka_core::site::{BlockData, OembedData, SiteBlock};

use crate::blocks::BlockLayout;
use crate::blocks::schema::{LayoutSchema, text};
use crate::html::escape_html;
use crate::view::RenderContext;

/// The `oembed` block.
#[derive(Debug, Clone, Copy, Default)]
pub struct Oembed;

/// `BlockLayout/Oembed.php::form`: the required `url`. Upstream's form also
/// has a `refresh` checkbox that fetches the embed again on save; the port has
/// no fetch, so the form leaves the box out. A stored `refresh` still parses
/// (`blocks/parse.rs`). `url_old` and `oembed` are hidden members the class
/// writes from the fetch, not fields the admin edits.
const SCHEMA: LayoutSchema = LayoutSchema {
    fields: &[text("url", "blockField.oembed.url", "fieldHelp.oembed.url")
        .placeholder("fieldExample.url")
        .required()],
};

impl BlockLayout for Oembed {
    fn name(&self) -> &'static str {
        "oembed"
    }

    fn label(&self) -> &'static str {
        "oEmbed"
    }

    fn render(&self, block: &SiteBlock, _ctx: &RenderContext<'_>) -> String {
        let BlockData::Oembed(data) = &block.data else {
            return String::new();
        };
        format!("<div>{}</div>", render_oembed(data))
    }

    fn schema(&self) -> &'static LayoutSchema {
        &SCHEMA
    }
}

/// `Stdlib/Oembed.php::renderOembed`, the fragment it returns.
#[must_use]
pub fn render_oembed(data: &OembedData) -> String {
    let Some(oembed) = data.oembed.as_ref() else {
        return String::new();
    };
    if let Some(html) = oembed.get("html").and_then(serde_json::Value::as_str) {
        return html.to_string();
    }
    if oembed.get("type").and_then(serde_json::Value::as_str) == Some("photo") {
        let url = oembed
            .get("url")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        return format!(
            "<img src=\"{}\" width=\"{}\" height=\"{}\" alt=\"{}\">",
            escape_html(url),
            escape_html(&scalar(oembed, "width")),
            escape_html(&scalar(oembed, "height")),
            escape_html(url)
        );
    }
    String::new()
}

/// A JSON scalar as the string `sprintf('%s')` would produce, empty when absent.
fn scalar(oembed: &serde_json::Value, key: &str) -> String {
    match oembed.get(key) {
        Some(serde_json::Value::String(value)) => value.clone(),
        Some(serde_json::Value::Number(value)) => value.to_string(),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks::test_support::{block, context, site};
    use serde_json::json;

    /// `Stdlib/Oembed.php:99-101`: a response with an `html` field is inserted
    /// as it arrived from the provider.
    #[test]
    fn oembed_matches_upstream_block_layout() {
        let site = site();
        let ctx = context(&site, &crate::view::NoResources);
        let stored = block(
            "oembed",
            json!({"url": "https://example.org/x", "oembed": {"type": "video", "html": "<iframe src=\"https://example.org/embed\"></iframe>"}}),
        );
        assert_eq!(
            Oembed.render(&stored, &ctx),
            "<div><iframe src=\"https://example.org/embed\"></iframe></div>"
        );
    }

    /// `Stdlib/Oembed.php:103-112`: a `photo` response becomes an `<img>` with
    /// escaped attribute values.
    #[test]
    fn oembed_photo_response_renders_an_image() {
        let site = site();
        let ctx = context(&site, &crate::view::NoResources);
        let stored = block(
            "oembed",
            json!({"oembed": {"type": "photo", "url": "https://example.org/p.jpg?a=1&b=2", "width": 640, "height": 480}}),
        );
        assert_eq!(
            Oembed.render(&stored, &ctx),
            "<div><img src=\"https://example.org/p.jpg?a=1&amp;b=2\" width=\"640\" height=\"480\" alt=\"https://example.org/p.jpg?a=1&amp;b=2\"></div>"
        );
    }

    /// A block with no cached response renders the empty wrapper, as the
    /// template does when `renderOembed` returns false.
    #[test]
    fn oembed_without_a_response_renders_an_empty_wrapper() {
        let site = site();
        let ctx = context(&site, &crate::view::NoResources);
        assert_eq!(
            Oembed.render(
                &block("oembed", json!({"url": "https://example.org/x"})),
                &ctx
            ),
            "<div></div>"
        );
    }
}
