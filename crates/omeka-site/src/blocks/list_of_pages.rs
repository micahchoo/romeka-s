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

use crate::blocks::schema::{LayoutSchema, When, choice, option, page_list};
use crate::blocks::{BlockLayout, columns};
use crate::html::escape_html;
use crate::slug::page_url;
use crate::view::{PageCard, RenderContext, Thumbnail};

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
/// jsTree writes as a JSON string into one hidden member. The `romeka.`
/// fields are Romeka settings: links or cards, and the cards' columns.
const SCHEMA: LayoutSchema = LayoutSchema {
    fields: &[
        page_list(
            "pagelist",
            "blockField.listOfPages.pagelist",
            "fieldHelp.listOfPages.pagelist",
        ),
        choice(
            "romeka.display",
            "blockField.listOfPages.display",
            "fieldHelp.listOfPages.display",
            &[
                option("", "blockChoice.pages_links"),
                option("cards", "blockChoice.pages_cards"),
            ],
        )
        .default_text(""),
        choice(
            "romeka.columns",
            "blockField.listOfPages.columns",
            "fieldHelp.listOfPages.columns",
            columns::OPTIONS,
        )
        .default_text("")
        .when(When::equals("romeka.display", "cards")),
    ],
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
        let Ok(nodes) =
            serde_json::from_str::<Vec<PageNode>>(pagelist.as_deref().unwrap_or_default())
        else {
            return String::new();
        };
        if shows_cards(block) {
            render_cards(&nodes, columns::of(block), ctx)
        } else {
            format!("<div>{}</div>", render_nodes(&nodes, ctx))
        }
    }

    fn schema(&self) -> &'static LayoutSchema {
        &SCHEMA
    }
}

/// Whether the block draws its pages as cards (`romeka.settings.display`).
#[must_use]
pub fn shows_cards(block: &SiteBlock) -> bool {
    block
        .layout_data
        .romeka_setting("display")
        .and_then(JsonValue::as_str)
        == Some("cards")
}

/// The page ids a stored `pagelist` names, depth first, so the snapshot can
/// load their cards.
#[must_use]
pub fn page_ids(pagelist: &str) -> Vec<PageId> {
    fn walk(nodes: &[PageNode], out: &mut Vec<PageId>) {
        for node in nodes {
            out.extend(node.data.page_id());
            walk(&node.children, out);
        }
    }
    let mut out = Vec::new();
    if let Ok(nodes) = serde_json::from_str::<Vec<PageNode>>(pagelist) {
        walk(&nodes, &mut out);
    }
    out
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

/// The tree as one grid of cards, depth first: a card grid has no nesting,
/// so a child page follows its parent. A node with no URL has no card.
fn render_cards(nodes: &[PageNode], columns: Option<u8>, ctx: &RenderContext<'_>) -> String {
    fn walk<'n>(nodes: &'n [PageNode], out: &mut Vec<&'n PageNode>) {
        for node in nodes {
            out.push(node);
            walk(&node.children, out);
        }
    }
    let mut flat = Vec::new();
    walk(nodes, &mut flat);
    let mut out = String::from("<div class=\"page-cards");
    if let Some(count) = columns {
        out.push(' ');
        out.push_str(&columns::class("page-cards", count));
    }
    out.push_str("\"><ul>");
    for node in flat {
        let Some(url) = node_url(node, ctx) else {
            continue;
        };
        let card = node
            .data
            .page_id()
            .and_then(|id| ctx.resources.page_card(id))
            .unwrap_or_default();
        out.push_str(&page_card(&node_title(node, ctx), &url, &card));
    }
    out.push_str("</ul></div>");
    out
}

/// One card: the image or an empty tinted slot, the title and the summary,
/// all one link.
fn page_card(title: &str, url: &str, card: &PageCard) -> String {
    let thumb = match &card.thumbnail {
        Some(Thumbnail { url, .. }) => format!(
            "<span class=\"page-card-thumb\"><img src=\"{}\" alt=\"\" loading=\"lazy\"></span>",
            escape_html(url)
        ),
        None => "<span class=\"page-card-thumb page-card-thumb-empty\"></span>".to_string(),
    };
    let summary = card.summary.as_deref().map_or_else(String::new, |summary| {
        format!(
            "<span class=\"page-card-summary\">{}</span>",
            escape_html(summary)
        )
    });
    format!(
        "<li class=\"page-card\"><a href=\"{}\" class=\"page-card-link\">{thumb}\
         <span class=\"page-card-title\">{}</span>{summary}</a></li>",
        escape_html(url),
        escape_html(title)
    )
}

/// A node's label: its own text, else the page's title.
fn node_title(node: &PageNode, ctx: &RenderContext<'_>) -> String {
    node.text
        .clone()
        .filter(|text| !text.trim().is_empty())
        .or_else(|| {
            let id = node.data.page_id()?;
            ctx.pages
                .iter()
                .find(|page| page.id == id)
                .map(|page| page.title.clone())
        })
        .unwrap_or_default()
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

    /// Cards for two pages: one with an image and a summary, one with none.
    struct Cards;

    impl crate::view::ResourceProvider for Cards {
        fn search(
            &self,
            _kind: omeka_core::resource::ResourceKind,
            _query: &crate::view::BrowseQuery,
        ) -> crate::view::ResourcePage {
            crate::view::ResourcePage::default()
        }
        fn read(
            &self,
            _kind: omeka_core::resource::ResourceKind,
            _id: u64,
        ) -> Option<crate::view::ResourceView> {
            None
        }
        fn read_media(&self, _id: omeka_core::ids::MediaId) -> Option<crate::view::MediaRef> {
            None
        }
        fn read_asset(&self, _id: omeka_core::ids::AssetId) -> Option<crate::view::AssetView> {
            None
        }
        fn attachments(&self, _block: omeka_core::ids::BlockId) -> Vec<crate::view::AttachmentRef> {
            Vec::new()
        }
        fn search_sites(&self, _query: &crate::view::SiteQuery) -> crate::view::SiteSearch {
            crate::view::SiteSearch::default()
        }
        fn page_card(&self, page: PageId) -> Option<PageCard> {
            (page == PageId::new(7)).then(|| PageCard {
                thumbnail: Some(Thumbnail::new("/files/asset/9.jpg", "")),
                summary: Some("Who we are & why.".to_string()),
            })
        }
    }

    /// In card mode the tree becomes one grid of cards, a child after its
    /// parent: each card is one link with the page's image or an empty slot,
    /// its title and its summary. A node with no page has no card.
    #[test]
    fn list_of_pages_cards_show_each_page_image_and_summary() {
        let site = site();
        let mut ctx = context(&site, &Cards);
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
                is_public: true,
            },
        ];
        let pagelist = json!([
            {"text": "About us", "data": {"id": 7}, "children": [
                {"text": "", "data": {"id": 8}},
                {"text": "Gone", "data": {"id": 99}}
            ]}
        ]);
        let stored = crate::blocks::test_support::block_with_layout(
            "listOfPages",
            json!({"pagelist": pagelist.to_string()}),
            json!({"romeka": {"block": "listOfPages", "v": 2,
                "settings": {"display": "cards", "columns": "3"}}}),
        );
        assert_eq!(
            ListOfPages.render(&stored, &ctx),
            "<div class=\"page-cards page-cards-columns-3\"><ul>\
             <li class=\"page-card\"><a href=\"/s/main/page/about\" class=\"page-card-link\">\
             <span class=\"page-card-thumb\"><img src=\"/files/asset/9.jpg\" alt=\"\" loading=\"lazy\"></span>\
             <span class=\"page-card-title\">About us</span>\
             <span class=\"page-card-summary\">Who we are &amp; why.</span></a></li>\
             <li class=\"page-card\"><a href=\"/s/main/page/team\" class=\"page-card-link\">\
             <span class=\"page-card-thumb page-card-thumb-empty\"></span>\
             <span class=\"page-card-title\">Team</span></a></li>\
             </ul></div>"
        );
        assert_eq!(
            page_ids(&pagelist.to_string()),
            vec![PageId::new(7), PageId::new(8), PageId::new(99)]
        );
    }
}
