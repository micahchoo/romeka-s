//! `BlockLayout/LineBreak.php` and `view/common/block-layout/line-break.phtml`.
//!
//! `<div class='break transparent'></div>`. The template uses single quotes and
//! echoes `break_type` unescaped; the port escapes the value, because a stored
//! value is data even when the admin form offers only two options.

use omeka_core::site::{BlockData, SiteBlock};

use crate::blocks::BlockLayout;
use crate::blocks::schema::{LayoutSchema, choice, option};
use crate::html::escape_html;
use crate::view::RenderContext;

/// The `lineBreak` block.
#[derive(Debug, Clone, Copy, Default)]
pub struct LineBreak;

/// `BlockLayout/LineBreak.php::form`: one `break_type` select.
const SCHEMA: LayoutSchema = LayoutSchema {
    fields: &[choice(
        "break_type",
        "blockField.lineBreak.break_type",
        &[
            option("transparent", "blockChoice.transparent"),
            option("opaque", "blockChoice.opaque"),
        ],
    )
    .default_text("transparent")],
};

impl BlockLayout for LineBreak {
    fn name(&self) -> &'static str {
        "lineBreak"
    }

    fn label(&self) -> &'static str {
        "Line break"
    }

    fn render(&self, block: &SiteBlock, _ctx: &RenderContext<'_>) -> String {
        match &block.data {
            BlockData::LineBreak { break_type } => {
                format!("<div class='break {}'></div>", escape_html(break_type))
            }
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

    /// `block-layout/line-break.phtml`: the break type becomes a class.
    #[test]
    fn line_break_matches_upstream_block_layout() {
        let site = site();
        let ctx = context(&site, &crate::view::NoResources);
        assert_eq!(
            LineBreak.render(&block("lineBreak", json!({"break_type": "opaque"})), &ctx),
            "<div class='break opaque'></div>"
        );
    }

    /// `BlockLayout/LineBreak.php` and core both default the type to
    /// `transparent`.
    #[test]
    fn line_break_defaults_to_transparent() {
        let site = site();
        let ctx = context(&site, &crate::view::NoResources);
        assert_eq!(
            LineBreak.render(&block("lineBreak", json!({})), &ctx),
            "<div class='break transparent'></div>"
        );
    }
}
