//! `BlockLayout/BlockGroup.php` and the group branch of
//! `View/Helper/PageLayout.php::render`.
//!
//! A group is a container. Its own `render` returns an empty string; the
//! grouping markup belongs to the page content, which
//! [`render_page_blocks`](crate::blocks::render_page_blocks) reproduces: a
//! `blockGroup` opens a `block block-blockGroup` wrapper around the `span`
//! blocks that follow it on the same page. The admin form carries the `span`,
//! which is the only thing this layout stores.

use omeka_core::site::SiteBlock;

use crate::blocks::BlockLayout;
use crate::view::RenderContext;

/// The `blockGroup` block.
#[derive(Debug, Clone, Copy, Default)]
pub struct BlockGroup;

impl BlockLayout for BlockGroup {
    fn name(&self) -> &'static str {
        "blockGroup"
    }

    fn label(&self) -> &'static str {
        "Block group"
    }

    fn render(&self, _block: &SiteBlock, _ctx: &RenderContext<'_>) -> String {
        String::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks::test_support::{block, context, site};
    use serde_json::json;

    /// `BlockLayout/BlockGroup.php::render` returns an empty string; the group
    /// span is admin-only data.
    #[test]
    fn block_group_matches_upstream_block_layout() {
        let site = site();
        let ctx = context(&site, &crate::view::NoResources);
        assert_eq!(
            BlockGroup.render(&block("blockGroup", json!({"span": "6"})), &ctx),
            ""
        );
    }
}
