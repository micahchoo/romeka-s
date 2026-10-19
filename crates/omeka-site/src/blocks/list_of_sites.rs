//! `BlockLayout/ListOfSites.php`, `common/site-list-entry.phtml` and
//! `view/common/block-layout/list-of-sites.phtml`.
//!
//! The block searches sites, orders them by `sort` (`alpha` by title, `oldest`
//! and `newest` by creation time), optionally excludes the current site, and
//! lists them with an optional summary and thumbnail. `pagination` turns the
//! limit into a page size and renders page links; without it the limit is
//! passed straight to the search.
//!
//! Upstream's `pagination` helper prints the navigation for the current URL.
//! The port builds `{current_path}?page=N` links, keeping the other query
//! pairs, and uses the default `per_page` from the limit. The exact markup of
//! the Laminas helper (its `<nav>` wrapper and per-link classes) is not
//! reproduced.

use omeka_core::site::{BlockData, ListOfSitesData, SiteBlock};

use crate::blocks::BlockLayout;
use crate::blocks::schema::{LayoutSchema, When, boolean, choice, integer_min, option};
use crate::html::escape_html;
use crate::view::{RenderContext, SiteQuery, SiteSort, SiteView};

/// The `listOfSites` block.
#[derive(Debug, Clone, Copy, Default)]
pub struct ListOfSites;

/// `BlockLayout/ListOfSites.php::form` and its `$defaults`.
const SCHEMA: LayoutSchema = LayoutSchema {
    fields: &[
        choice(
            "sort",
            "blockField.listOfSites.sort",
            "fieldHelp.listOfSites.sort",
            &[
                option("alpha", "blockChoice.alpha"),
                option("oldest", "blockChoice.oldest"),
                option("newest", "blockChoice.newest"),
            ],
        )
        .default_text("alpha"),
        integer_min(
            "limit",
            "blockField.listOfSites.limit",
            "fieldHelp.listOfSites.limit",
            0,
        ),
        boolean(
            "pagination",
            "blockField.listOfSites.pagination",
            "fieldHelp.listOfSites.pagination",
        )
        .default_bool(false)
        .when(When::set("limit")),
        boolean(
            "summaries",
            "blockField.listOfSites.summaries",
            "fieldHelp.listOfSites.summaries",
        )
        .default_bool(true),
        boolean(
            "thumbnails",
            "blockField.listOfSites.thumbnails",
            "fieldHelp.listOfSites.thumbnails",
        )
        .default_bool(true),
        boolean(
            "exclude_current",
            "blockField.listOfSites.exclude_current",
            "fieldHelp.listOfSites.exclude_current",
        )
        .default_bool(true),
    ],
};

impl BlockLayout for ListOfSites {
    fn name(&self) -> &'static str {
        "listOfSites"
    }

    fn label(&self) -> &'static str {
        "List of sites"
    }

    fn render(&self, block: &SiteBlock, ctx: &RenderContext<'_>) -> String {
        let BlockData::ListOfSites(data) = &block.data else {
            return String::new();
        };
        render_block(data, ctx)
    }

    fn schema(&self) -> &'static LayoutSchema {
        &SCHEMA
    }
}

fn render_block(data: &ListOfSitesData, ctx: &RenderContext<'_>) -> String {
    let sort = match data.sort.as_deref() {
        Some("oldest") => SiteSort::Oldest,
        Some("newest") => SiteSort::Newest,
        _ => SiteSort::Alpha,
    };
    let limit = data
        .limit
        .and_then(|limit| usize::try_from(limit).ok())
        .filter(|limit| *limit > 0);
    let exclude_current = data.exclude_current.unwrap_or(true);
    let paginate = limit.is_some() && data.pagination.unwrap_or(false);
    let page = if paginate { query_page(ctx).max(1) } else { 1 };

    let search = SiteQuery {
        sort,
        limit,
        exclude: if exclude_current {
            Some(ctx.site.id)
        } else {
            None
        },
        page: paginate.then_some(page),
    };
    let result = ctx.resources.search_sites(&search);

    let summaries = data.summaries.unwrap_or(true);
    let thumbnails = data.thumbnails.unwrap_or(true);
    let mut out = String::from("<div class=\"list-of-sites\"><div class=\"site-list\">");
    for site in &result.sites {
        out.push_str(&render_site(site, summaries, thumbnails));
    }
    out.push_str("</div>");
    if paginate && let Some(per_page) = limit {
        out.push_str(&crate::html::pagination(
            &ctx.current_path,
            &ctx.query,
            page,
            per_page,
            result.total,
        ));
    }
    out.push_str("</div>");
    out
}

/// `common/site-list-entry.phtml`.
fn render_site(site: &SiteView, summaries: bool, thumbnails: bool) -> String {
    let mut out = String::from("<div class=\"site\">");
    if thumbnails && let Some(thumbnail) = &site.thumbnail {
        out.push_str(&format!(
            "<div class=\"site-thumbnail\"><img src=\"{}\" alt=\"{}\" class=\"site-thumbnail-image\"></div>",
            escape_html(&thumbnail.url),
            escape_html(&thumbnail.alt)
        ));
    }
    out.push_str("<div class=\"site-text\">");
    out.push_str(&crate::html::hyperlink(
        &site.title,
        Some(&site.url),
        &[crate::html::Attr::text("class", "site-link")],
    ));
    if summaries && let Some(summary) = site.summary.as_deref().filter(|s| !s.is_empty()) {
        out.push_str(&format!(
            "<p class=\"site-summary\">{}</p>",
            escape_html(summary).replace('\n', "<br />\n")
        ));
    }
    out.push_str("</div></div>");
    out
}

fn query_page(ctx: &RenderContext<'_>) -> usize {
    ctx.query
        .iter()
        .find(|(key, _)| key == "page")
        .and_then(|(_, value)| value.parse::<usize>().ok())
        .unwrap_or(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks::test_support::{TestResources, block, context, site};
    use crate::view::Thumbnail;
    use omeka_core::ids::SiteId;
    use serde_json::json;

    fn sites() -> Vec<SiteView> {
        vec![
            SiteView {
                id: SiteId::new(2),
                title: "Alpha".to_string(),
                url: "/s/alpha".to_string(),
                summary: Some("First\nSecond".to_string()),
                thumbnail: Some(Thumbnail::new("/files/square/2.jpg", "Alpha")),
            },
            SiteView {
                id: SiteId::new(3),
                title: "Beta".to_string(),
                url: "/s/beta".to_string(),
                summary: None,
                thumbnail: None,
            },
        ]
    }

    /// `block-layout/list-of-sites.phtml` and `common/site-list-entry.phtml`:
    /// each site is a link with an escaped, line-broken summary.
    #[test]
    fn list_of_sites_matches_upstream_block_layout() {
        let site = site();
        let resources = TestResources {
            sites: sites(),
            ..TestResources::default()
        };
        let ctx = context(&site, &resources);
        let stored = block(
            "listOfSites",
            json!({"summaries": true, "thumbnails": true}),
        );
        assert_eq!(
            ListOfSites.render(&stored, &ctx),
            "<div class=\"list-of-sites\"><div class=\"site-list\"><div class=\"site\"><div class=\"site-thumbnail\"><img src=\"/files/square/2.jpg\" alt=\"Alpha\" class=\"site-thumbnail-image\"></div><div class=\"site-text\"><a href=\"/s/alpha\" class=\"site-link\">Alpha</a><p class=\"site-summary\">First<br />\nSecond</p></div></div><div class=\"site\"><div class=\"site-text\"><a href=\"/s/beta\" class=\"site-link\">Beta</a></div></div></div></div>"
        );
    }

    /// `exclude_current` defaults to true and drops the current site.
    #[test]
    fn list_of_sites_excludes_the_current_site_by_default() {
        let site = site();
        let resources = TestResources {
            sites: vec![SiteView {
                id: SiteId::new(1),
                title: "Main".to_string(),
                url: "/s/main".to_string(),
                summary: None,
                thumbnail: None,
            }],
            ..TestResources::default()
        };
        let ctx = context(&site, &resources);
        let out = ListOfSites.render(&block("listOfSites", json!({})), &ctx);
        assert_eq!(
            out,
            "<div class=\"list-of-sites\"><div class=\"site-list\"></div></div>"
        );

        let included = ListOfSites.render(
            &block("listOfSites", json!({"exclude_current": false})),
            &ctx,
        );
        assert!(included.contains(">Main</a>"), "{included}");
    }

    /// A limit with pagination renders page links; without pagination the limit
    /// simply caps the list.
    #[test]
    fn list_of_sites_pagination_matches_upstream() {
        let site = site();
        let resources = TestResources {
            sites: sites(),
            ..TestResources::default()
        };
        let mut ctx = context(&site, &resources);
        ctx.current_path = "/s/main".to_string();
        let paged = ListOfSites.render(
            &block("listOfSites", json!({"limit": 1, "pagination": true})),
            &ctx,
        );
        assert!(paged.contains("<nav class=\"pagination\">"), "{paged}");
        assert!(paged.contains("href=\"/s/main?page=2\""), "{paged}");

        let capped = ListOfSites.render(&block("listOfSites", json!({"limit": 1})), &ctx);
        assert!(!capped.contains("pagination"), "{capped}");
        assert!(capped.contains("Alpha"), "{capped}");
        assert!(!capped.contains("Beta"), "{capped}");
    }
}
