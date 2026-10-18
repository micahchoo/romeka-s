//! The content a new site is born with: `SiteAdapter::getDefaultNavigation`
//! and the welcome page `SiteAdapter::hydrate` adds on create.

use omeka_db::ids::SiteId;
use omeka_db::repos::SiteRepo;
use omeka_db::rows::{NewSiteBlock, NewSitePage};
use serde_json::{Value, json};

use crate::errors::ApiError;
use crate::read::{Api, db_error};

/// `SiteAdapter::getDefaultNavigation`: one link to the browse page.
pub fn default_navigation() -> Value {
    json!([{
        "type": "browse",
        "data": { "label": "Browse", "query": "" },
        "links": [],
    }])
}

/// Build the welcome page `SiteAdapter::hydrate` adds on create.
pub async fn welcome_page(api: &Api<'_>, site: SiteId) -> Result<(), ApiError> {
    let repo = SiteRepo::new(api.pool());
    let page = repo
        .add_page(&NewSitePage {
            site_id: site,
            slug: "welcome".to_string(),
            title: "Welcome".to_string(),
            is_public: true,
            ..NewSitePage::default()
        })
        .await
        .map_err(db_error)?;
    for (position, layout, data) in [
        (1_i64, "html", json!({ "html": FIRST_PAGE_CONTENT })),
        (2, "lineBreak", json!({ "break_type": "opaque" })),
        (3, "html", json!({ "html": SECOND_PAGE_CONTENT })),
    ] {
        repo.add_block(&NewSiteBlock {
            page_id: page.id,
            layout: layout.to_string(),
            data,
            layout_data: None,
            position,
        })
        .await
        .map_err(db_error)?;
    }
    Ok(())
}

/// `SiteAdapter::getFirstPageContent`.
const FIRST_PAGE_CONTENT: &str = "<p>Welcome to your new site. This is an example page.</p>\n<ul>\n<li>This is a bullet list.</li>\n<li>Second entry.</li>\n</ul>\n<p>Back to normal again.</p>\n<ol>\n<li>This is an ordered list.</li>\n<li>Second entry.</li>\n</ol>\n<p>Back to normal again.</p>\n<blockquote>This is a blockquote.</blockquote>\n<p>Back to normal again.</p>\n<div><a href=\"#\">This text is a link, which currently points to nothing.</a></div>\n<p><strong>This text is bold, in a &lt;strong&gt; tag.</strong></p>\n<p><em>This text is italicized, in an &lt;em&gt; tag.</em></p>\n<p><u>This text is underlined, in a &lt;u&gt; tag.</u></p>\n<p><s>This text has a strikethrough, in a &lt;s&gt; tag.</s></p>\n<p>This text is <sub>subscript</sub> and <sup>superscript</sup> using &lt;sub&gt; and &lt;sup&gt;, which can be used for adding notes and citations.</p>\n<hr>\n<h1>This is an H1 title. It is bigger than the Page Title, which is rendered in H2.</h1>\n<h2>This is an H2 header, the same size as the Page Title.</h2>\n<h3>This is an H3 subheader.</h3>\n<h4>This is an H4 subheader.</h4>\n<h5>This is an H5 subheader.</h5>\n<h6>This is an H6 subheader.</h6>";

/// `SiteAdapter::getSecondPageContent`.
const SECOND_PAGE_CONTENT: &str = "<h2 style=\"font-style:italic;\">This is the &quot;Italic Title&quot; block style.</h2>\n<div style=\"background:#eeeeee;border:1px solid #cccccc;padding:5px 10px;\">This is the &quot;Special Container&quot; block style.</div>\n<p><span class=\"marker\">This is the &quot;Marker&quot; inline style. </span></p>\n<p><small>This text is inside a &quot;small&quot; inline style.</small> This is normal text.</p>\n<p><code>This is the &quot;Computer Code&quot; inline style.</code></p>";
