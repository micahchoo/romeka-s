//! `BlockLayout/PageTitle.php` and `view/common/block-layout/page-title.phtml`.
//!
//! `<h2>` around the escaped page title. The class takes the title from
//! `$block->page()->title()`, so the block carries no data of its own.

use omeka_core::site::SiteBlock;

use crate::blocks::BlockLayout;
use crate::blocks::schema::{EMPTY, LayoutSchema};
use crate::html::escape_html;
use crate::view::RenderContext;

/// The `pageTitle` block.
#[derive(Debug, Clone, Copy, Default)]
pub struct PageTitle;

impl BlockLayout for PageTitle {
    fn name(&self) -> &'static str {
        "pageTitle"
    }

    fn label(&self) -> &'static str {
        "Page title"
    }

    fn render(&self, _block: &SiteBlock, ctx: &RenderContext<'_>) -> String {
        match ctx.page {
            Some(page) => format!("<h2>{}</h2>", escape_html(&page.title)),
            None => String::new(),
        }
    }

    /// `BlockLayout/PageTitle.php::form` shows the page title as text; the
    /// block stores nothing.
    fn schema(&self) -> &'static LayoutSchema {
        &EMPTY
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks::test_support::{context, site};
    use omeka_core::ids::{PageId, SiteId};
    use omeka_core::site::SitePage;

    /// `block-layout/page-title.phtml`: an escaped title in an `<h2>`.
    #[test]
    fn page_title_matches_upstream_block_layout() {
        let site = site();
        let page = SitePage::new(
            PageId::new(2),
            SiteId::new(1),
            omeka_core::site::Slug::parse("welcome").unwrap(),
            "Welcome & Co",
        );
        let mut ctx = context(&site, &crate::view::NoResources);
        ctx.page = Some(&page);
        let block = crate::blocks::test_support::block("pageTitle", serde_json::json!({}));
        assert_eq!(PageTitle.render(&block, &ctx), "<h2>Welcome &amp; Co</h2>");
    }

    /// With no page there is no title to render.
    #[test]
    fn page_title_is_empty_without_a_page() {
        let site = site();
        let ctx = context(&site, &crate::view::NoResources);
        let block = crate::blocks::test_support::block("pageTitle", serde_json::json!({}));
        assert_eq!(PageTitle.render(&block, &ctx), "");
    }
}
