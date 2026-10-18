//! `BlockLayout/Fallback.php` and `Site\BlockLayout\Manager::get`.
//!
//! The manager substitutes `Fallback` for a name no module registered.
//! `Fallback::render` returns an empty string, and the class keeps the original
//! name so a round trip preserves the block; core's `BlockLayout::Fallback`
//! carries that name, so this layout holds no state.

use omeka_core::site::SiteBlock;

use crate::blocks::BlockLayout;
use crate::blocks::schema::{EMPTY, LayoutSchema};
use crate::view::RenderContext;

/// The layout for an unregistered name.
#[derive(Debug, Clone, Copy, Default)]
pub struct Fallback;

impl BlockLayout for Fallback {
    fn name(&self) -> &'static str {
        "fallback"
    }

    fn label(&self) -> &'static str {
        "Unknown []"
    }

    fn render(&self, _block: &SiteBlock, _ctx: &RenderContext<'_>) -> String {
        String::new()
    }

    /// `BlockLayout/Fallback.php::form` shows no field the admin can edit; the
    /// stored data travels through as one hidden value.
    fn schema(&self) -> &'static LayoutSchema {
        &EMPTY
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks::test_support::{block, context, site};
    use serde_json::json;

    /// `BlockLayout/Fallback.php::render` returns an empty string.
    #[test]
    fn fallback_matches_upstream_block_layout() {
        let site = site();
        let ctx = context(&site, &crate::view::NoResources);
        assert_eq!(
            Fallback.render(&block("myModuleLayout", json!({"x": 1})), &ctx),
            ""
        );
        let stored = block("myModuleLayout", json!({"x": 1}));
        assert_eq!(
            stored.layout,
            omeka_core::site::BlockLayout::Fallback("myModuleLayout".to_string())
        );
    }
}
