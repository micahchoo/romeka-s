//! `BlockLayout/ListOfPages.php` and
//! `view/common/block-layout/list-of-pages.phtml`.
//!
//! The stored `pagelist` is a jsTree of pages: `{text, data, children}`. The
//! class walks it and gives each node a URL (`getPageNodeURLs`), which it takes
//! from the node's own `data.url` or resolves through the site's pages; the
//! template prints a nested list of links and marks a node without a URL
//! `[Missing Page]`.
//!
//! Upstream resolves the URL only for nodes that carry a navigation-link
//! `type`, which the page tree the admin writes does not. The port resolves a
//! node's page id directly, so a stored tree renders its links; the observable
//! template output is the same either way.

use omeka_core::ids::PageId;
use omeka_core::site::{BlockData, SiteBlock};
use serde::Deserialize;
use serde_json::Value as JsonValue;

use crate::blocks::BlockLayout;
use crate::blocks::schema::{LayoutSchema, page_list};
use crate::slug::page_url;
use crate::view::RenderContext;

/// One stored page-tree node.
#[derive(Debug, Clone, Default, Deserialize)]
struct PageNode {
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    data: PageNodeData,
    #[serde(default)]
    children: Vec<PageNode>,
}

/// A node's `data` object.
#[derive(Debug, Clone, Default, Deserialize)]
struct PageNodeData {
    #[serde(default)]
    id: Option<JsonValue>,
    #[serde(default)]
    url: Option<String>,
}

impl PageNodeData {
    fn page_id(&self) -> Option<PageId> {
        let id = match self.id.as_ref()? {
            JsonValue::Number(number) => number.as_u64(),
            JsonValue::String(text) => text.trim().parse::<u64>().ok(),
            _ => None,
        };
        id.map(PageId::new)
    }
}

/// The `listOfPages` block.
#[derive(Debug, Clone, Copy, Default)]
pub struct ListOfPages;

/// `BlockLayout/ListOfPages.php::form`: the `pagelist` tree, which the admin's
/// jsTree writes as a JSON string into one hidden member.
const SCHEMA: LayoutSchema = LayoutSchema {
    fields: &[page_list("pagelist", "blockField.listOfPages.pagelist")],
};

impl BlockLayout for ListOfPages {
    fn name(&self) -> &'static str {
        "listOfPages"
    }

    fn label(&self) -> &'static str {
        "List of pages"
    }

    fn render(&self, block: &SiteBlock, ctx: &RenderContext<'_>) -> String {
        let BlockData::ListOfPages { pagelist } = &block.data else {
            return String::new();
        };
        render_block(pagelist.as_deref().unwrap_or_default(), ctx)
    }

    fn schema(&self) -> &'static LayoutSchema {
        &SCHEMA
    }
}

fn render_block(pagelist: &str, ctx: &RenderContext<'_>) -> String {
    let Ok(nodes) = serde_json::from_str::<Vec<PageNode>>(pagelist) else {
        return String::new();
    };
    format!("<div>{}</div>", render_nodes(&nodes, ctx))
}

fn render_nodes(nodes: &[PageNode], ctx: &RenderContext<'_>) -> String {
    let mut out = String::from("<ul>");
    for node in nodes {
        out.push_str("<li>");
        let text = node.text.clone().unwrap_or_default();
        match node_url(node, ctx) {
            Some(url) => out.push_str(&crate::html::hyperlink(
                &text,
                Some(&url),
                &[crate::html::Attr::text("class", "list-of-pages-link")],
            )),
            None => out.push_str(
                "<span class=\"list-of-pages-link list-of-pages-link-missing\">[Missing Page]</span>",
            ),
        }
        if !node.children.is_empty() {
            out.push_str(&render_nodes(&node.children, ctx));
        }
        out.push_str("</li>");
    }
    out.push_str("</ul>");
    out
}

fn node_url(node: &PageNode, ctx: &RenderContext<'_>) -> Option<String> {
    if let Some(url) = node.data.url.as_deref().filter(|url| !url.is_empty()) {
        return Some(url.to_string());
    }
    let id = node.data.page_id()?;
    let page = ctx.pages.iter().find(|page| page.id == id)?;
    Some(page_url(&ctx.site.slug, &page.slug))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks::test_support::{block, context, site};
    use crate::nav::PageRef;
    use omeka_core::site::Slug;
    use serde_json::json;

    /// `block-layout/list-of-pages.phtml`: a nested list of links, and a node
    /// with no resolvable page marked `[Missing Page]`.
    #[test]
    fn list_of_pages_matches_upstream_block_layout() {
        let site = site();
        let mut ctx = context(&site, &crate::view::NoResources);
        ctx.pages = vec![
            PageRef {
                id: PageId::new(7),
                slug: Slug::parse("about").unwrap(),
                title: "About".to_string(),
                is_public: true,
            },
            PageRef {
                id: PageId::new(8),
                slug: Slug::parse("team").unwrap(),
                title: "Team".to_string(),
                is_public: false,
            },
        ];
        let pagelist = json!([
            {"text": "About", "data": {"id": 7}, "children": [
                {"text": "Team", "data": {"id": "8"}},
                {"text": "Gone", "data": {"id": 99}}
            ]}
        ]);
        let stored = block("listOfPages", json!({"pagelist": pagelist.to_string()}));
        assert_eq!(
            ListOfPages.render(&stored, &ctx),
            "<div><ul><li><a href=\"/s/main/page/about\" class=\"list-of-pages-link\">About</a><ul><li><a href=\"/s/main/page/team\" class=\"list-of-pages-link\">Team</a></li><li><span class=\"list-of-pages-link list-of-pages-link-missing\">[Missing Page]</span></li></ul></li></ul></div>"
        );
    }

    /// A node's own `data.url` wins over the page lookup.
    #[test]
    fn list_of_pages_prefers_a_stored_url() {
        let site = site();
        let ctx = context(&site, &crate::view::NoResources);
        let pagelist = json!([{"text": "Home", "data": {"url": "/s/main"}}]);
        let stored = block("listOfPages", json!({"pagelist": pagelist.to_string()}));
        let out = ListOfPages.render(&stored, &ctx);
        assert!(
            out.contains("<a href=\"/s/main\" class=\"list-of-pages-link\">Home</a>"),
            "{out}"
        );
    }

    /// A `pagelist` that is not a JSON list renders nothing
    /// (`BlockLayout/ListOfPages.php::render`).
    #[test]
    fn list_of_pages_without_a_tree_renders_nothing() {
        let site = site();
        let ctx = context(&site, &crate::view::NoResources);
        assert_eq!(
            ListOfPages.render(&block("listOfPages", json!({"pagelist": "nope"})), &ctx),
            ""
        );
        assert_eq!(
            ListOfPages.render(&block("listOfPages", json!({})), &ctx),
            ""
        );
    }
}
