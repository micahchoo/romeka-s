//! `BlockLayout/TableOfContents.php` and
//! `view/common/block-layout/table-of-contents.phtml`.
//!
//! The active page's children, as a navigation menu, to `depth` levels:
//! `maxDepth` is `depth - 1` because the menu helper counts the levels below
//! the list itself. The class clamps a stored `depth` below 1 to 1 and returns
//! nothing when the page is not in the navigation.

use omeka_core::site::{BlockData, SiteBlock};

use crate::blocks::BlockLayout;
use crate::nav::{find_item_by_page, render_menu};
use crate::view::RenderContext;

/// The `tableOfContents` block.
#[derive(Debug, Clone, Copy, Default)]
pub struct TableOfContents;

impl BlockLayout for TableOfContents {
    fn name(&self) -> &'static str {
        "tableOfContents"
    }

    fn label(&self) -> &'static str {
        "Table of contents"
    }

    fn render(&self, block: &SiteBlock, ctx: &RenderContext<'_>) -> String {
        let BlockData::TableOfContents { depth } = &block.data else {
            return String::new();
        };
        render_block(*depth, ctx)
    }
}

fn render_block(depth: i64, ctx: &RenderContext<'_>) -> String {
    let Some(page) = ctx.page else {
        return String::new();
    };
    let Some(active) = find_item_by_page(&ctx.nav, page.id) else {
        return String::new();
    };
    let depth = depth.max(1);
    let max_depth = usize::try_from(depth - 1).unwrap_or(0);
    let menu = render_menu(&active.children, Some(max_depth), "navigation");
    format!("<div class=\"toc-block\">{menu}</div>")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks::test_support::{block, context, site};
    use crate::nav::{NavItem, PageRef};
    use omeka_core::ids::PageId;
    use omeka_core::site::{SitePage, Slug};
    use serde_json::json;

    fn item(label: &str, url: &str, id: u64, children: Vec<NavItem>) -> NavItem {
        NavItem {
            label: label.to_string(),
            url: url.to_string(),
            target_blank: false,
            page: Some(PageId::new(id)),
            children,
        }
    }

    /// `block-layout/table-of-contents.phtml`: the active page's children, one
    /// level for `depth` 1, nested under `toc-block`.
    #[test]
    fn table_of_contents_matches_upstream_block_layout() {
        let site = site();
        let page = SitePage::new(
            PageId::new(5),
            omeka_core::ids::SiteId::new(1),
            Slug::parse("guide").unwrap(),
            "Guide",
        );
        let mut ctx = context(&site, &crate::view::NoResources);
        ctx.page = Some(&page);
        ctx.pages = vec![PageRef {
            id: PageId::new(5),
            slug: Slug::parse("guide").unwrap(),
            title: "Guide".to_string(),
            is_public: true,
        }];
        ctx.nav = vec![
            item("Home", "/s/main", 1, vec![]),
            item(
                "Guide",
                "/s/main/page/guide",
                5,
                vec![
                    item(
                        "Chapter one",
                        "/s/main/page/one",
                        6,
                        vec![item("Section", "/s/main/page/section", 7, vec![])],
                    ),
                    item("Chapter two", "/s/main/page/two", 8, vec![]),
                ],
            ),
        ];
        let out = TableOfContents.render(&block("tableOfContents", json!({"depth": 1})), &ctx);
        assert!(
            out.starts_with("<div class=\"toc-block\"><ul class=\"navigation\">"),
            "{out}"
        );
        assert!(out.contains(">Chapter one</a>"), "{out}");
        assert!(out.contains(">Chapter two</a>"), "{out}");
        assert!(!out.contains(">Section</a>"), "{out}");
    }

    /// `depth` 2 shows the grandchildren, and a depth below 1 is clamped to 1
    /// (`BlockLayout/TableOfContents.php::render`).
    #[test]
    fn table_of_contents_depth_controls_levels() {
        let site = site();
        let page = SitePage::new(
            PageId::new(5),
            omeka_core::ids::SiteId::new(1),
            Slug::parse("guide").unwrap(),
            "Guide",
        );
        let mut ctx = context(&site, &crate::view::NoResources);
        ctx.page = Some(&page);
        ctx.nav = vec![item(
            "Guide",
            "/s/main/page/guide",
            5,
            vec![item(
                "Chapter",
                "/s/main/page/one",
                6,
                vec![item("Section", "/s/main/page/section", 7, vec![])],
            )],
        )];
        let deep = TableOfContents.render(&block("tableOfContents", json!({"depth": 2})), &ctx);
        assert!(deep.contains(">Section</a>"), "{deep}");
        let clamped = TableOfContents.render(&block("tableOfContents", json!({"depth": 0})), &ctx);
        assert!(clamped.contains(">Chapter</a>"), "{clamped}");
        assert!(!clamped.contains(">Section</a>"), "{clamped}");
    }

    /// A page that is not in the navigation renders nothing.
    #[test]
    fn table_of_contents_without_active_page_renders_nothing() {
        let site = site();
        let ctx = context(&site, &crate::view::NoResources);
        assert_eq!(
            TableOfContents.render(&block("tableOfContents", json!({"depth": 1})), &ctx),
            ""
        );
    }
}
