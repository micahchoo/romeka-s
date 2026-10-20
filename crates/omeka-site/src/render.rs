//! Render a site page into a full HTML document.
//!
//! The entry point the server calls. It wires the bundled theme to the page
//! view through the [`Theme`] seam, so a second theme needs no change here.

use omeka_core::site::SitePage;

use crate::theme::Theme;
use crate::view::{RenderContext, View};

/// Render a page's blocks, in `position` order, into a full HTML document.
///
/// The context must carry the site, the settings, the navigation and the
/// resource provider; this call sets [`RenderContext::page`] to `page`.
/// `theme` renders the document, so a site's own theme is what the caller
/// resolves from `site.theme`.
#[must_use]
pub fn render_page<'a>(
    theme: &dyn Theme,
    page: &'a SitePage,
    ctx: &mut RenderContext<'a>,
) -> String {
    ctx.page = Some(page);
    theme.render(&View::Page, ctx)
}

/// Render the site index, the view a site with no pages shows.
#[must_use]
pub fn render_index(theme: &dyn Theme, ctx: &RenderContext<'_>) -> String {
    theme.render(&View::Index, ctx)
}

/// Render any view through a caller-chosen theme.
#[must_use]
pub fn render(theme: &dyn Theme, view: &View<'_>, ctx: &RenderContext<'_>) -> String {
    theme.render(view, ctx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks::test_support::{block, context, site};
    use crate::nav::PageRef;
    use crate::theme::DefaultTheme;
    use crate::view::NoResources;
    use omeka_core::ids::{PageId, SiteId};
    use omeka_core::site::{BlockLayout, SitePage, Slug};
    use serde_json::json;

    fn page_with_blocks(blocks: Vec<omeka_core::site::SiteBlock>) -> SitePage {
        let mut page = SitePage::new(
            PageId::new(2),
            SiteId::new(1),
            Slug::parse("welcome").unwrap(),
            "Welcome",
        );
        page.blocks = blocks;
        page
    }

    /// A site and a page with three blocks render a document that carries each
    /// block's own fragment, in position order.
    #[test]
    fn a_page_with_three_blocks_renders_each_fragment() {
        let mut html_block = block("html", json!({"html": "<p>Body copy</p>"}));
        html_block.position = 2;
        let mut title_block = block("pageTitle", json!({}));
        title_block.position = 1;
        let mut break_block = block("lineBreak", json!({"break_type": "opaque"}));
        break_block.position = 3;
        let page = page_with_blocks(vec![break_block, html_block, title_block]);

        let site = site();
        let mut ctx = context(&site, &NoResources);
        ctx.pages = vec![PageRef {
            id: PageId::new(2),
            slug: Slug::parse("welcome").unwrap(),
            title: "Welcome".to_string(),
            is_public: true,
        }];
        let document = render_page(&DefaultTheme, &page, &mut ctx);

        let title_at = document.find("<h2>Welcome</h2>").expect("title fragment");
        let html_at = document.find("<p>Body copy</p>").expect("html fragment");
        let break_at = document
            .find("<div class='break opaque'></div>")
            .expect("line break fragment");
        assert!(title_at < html_at && html_at < break_at, "{document}");
        assert!(document.starts_with("<!DOCTYPE html>"), "{document}");
        assert!(document.ends_with("</html>"), "{document}");
    }

    /// A value carrying markup is escaped wherever the theme prints it.
    #[test]
    fn markup_in_a_value_renders_escaped() {
        let mut title_block = block("pageTitle", json!({}));
        title_block.position = 1;
        let page = page_with_blocks(vec![title_block]);
        let site = site();
        let hostile = SitePage {
            title: "<script>alert('x')</script>".to_string(),
            ..page.clone()
        };
        let mut ctx = context(&site, &NoResources);
        let document = render_page(&DefaultTheme, &hostile, &mut ctx);

        assert!(
            document.contains("<h2>&lt;script&gt;alert(&#039;x&#039;)&lt;/script&gt;</h2>"),
            "{document}"
        );
        assert!(!document.contains("<script>alert"), "{document}");
        assert!(
            document.contains(
                "<title>&lt;script&gt;alert(&#039;x&#039;)&lt;/script&gt; · Romeka S</title>"
            ),
            "{document}"
        );
        // An unregistered layout renders through the fallback and adds nothing.
        assert_eq!(
            BlockLayout::from_wire_name("other").wire_name(),
            None,
            "the fallback carries no wire name"
        );
    }

    /// The index view renders without a page.
    #[test]
    fn index_renders_without_a_page() {
        let site = site();
        let ctx = context(&site, &NoResources);
        let document = render_index(&DefaultTheme, &ctx);
        assert!(
            document.contains("<p>This site has no pages.</p>"),
            "{document}"
        );
        assert!(!document.contains("<div class=\"blocks\">"), "{document}");
    }
}
