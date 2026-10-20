//! The site footer: up to three HTML columns, a list of links, a contact
//! block, a licence, and the "Powered by" credit, all stored under the
//! theme's own settings map.
//!
//! The links are stored in the navigation's own shape (`[{type, data}]`), so
//! the admin edits them with the navigation link editor and the footer
//! resolves them with [`crate::nav`]. A map stored before the footer had
//! structure carries one `footer_text`; it reads as the first column.

use serde_json::Value;

use crate::html::{escape_html, hyperlink};
use crate::i18n::Translator;
use crate::nav::{NavItem, NavLink, NavNode, parse_navigation, resolve};
use crate::theme::choices::choice;
use crate::theme::tokens::stored_text;
use crate::view::RenderContext;

/// The most HTML columns a footer holds.
pub const MAX_COLUMNS: usize = 3;

choice! {
    /// The licence a site states for its content.
    Licence {
        /// No licence line.
        Unstated = "none",
        /// Creative Commons Attribution 4.0.
        CcBy = "cc-by",
        /// Creative Commons Attribution-ShareAlike 4.0.
        CcBySa = "cc-by-sa",
        /// Creative Commons Attribution-NoDerivatives 4.0.
        CcByNd = "cc-by-nd",
        /// Creative Commons Attribution-NonCommercial 4.0.
        CcByNc = "cc-by-nc",
        /// Creative Commons Attribution-NonCommercial-ShareAlike 4.0.
        CcByNcSa = "cc-by-nc-sa",
        /// Creative Commons Attribution-NonCommercial-NoDerivatives 4.0.
        CcByNcNd = "cc-by-nc-nd",
        /// The CC0 1.0 public domain dedication.
        Cc0 = "cc0",
        /// All rights reserved.
        AllRightsReserved = "all-rights-reserved",
    }
}

impl Licence {
    /// The licence's short name and deed URL, `None` for no licence or all
    /// rights reserved. The name is the licence's own, so it is not
    /// translated.
    #[must_use]
    pub const fn deed(self) -> Option<(&'static str, &'static str)> {
        match self {
            Self::Unstated | Self::AllRightsReserved => None,
            Self::CcBy => Some(("CC BY 4.0", "https://creativecommons.org/licenses/by/4.0/")),
            Self::CcBySa => Some((
                "CC BY-SA 4.0",
                "https://creativecommons.org/licenses/by-sa/4.0/",
            )),
            Self::CcByNd => Some((
                "CC BY-ND 4.0",
                "https://creativecommons.org/licenses/by-nd/4.0/",
            )),
            Self::CcByNc => Some((
                "CC BY-NC 4.0",
                "https://creativecommons.org/licenses/by-nc/4.0/",
            )),
            Self::CcByNcSa => Some((
                "CC BY-NC-SA 4.0",
                "https://creativecommons.org/licenses/by-nc-sa/4.0/",
            )),
            Self::CcByNcNd => Some((
                "CC BY-NC-ND 4.0",
                "https://creativecommons.org/licenses/by-nc-nd/4.0/",
            )),
            Self::Cc0 => Some((
                "CC0 1.0",
                "https://creativecommons.org/publicdomain/zero/1.0/",
            )),
        }
    }
}

/// What the footer shows, read from the theme settings map.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Footer {
    /// The HTML columns, at most [`MAX_COLUMNS`], none blank. Purified when
    /// printed.
    pub columns: Vec<String>,
    /// The links, in the navigation's shape. `None` when the site has not set
    /// any, and the footer repeats the header menu's top level.
    pub links: Option<Vec<NavNode>>,
    /// The contact or address block, HTML purified when printed.
    pub contact: Option<String>,
    /// The licence line.
    pub licence: Licence,
    /// Whether the "Powered by Romeka S" credit shows.
    pub powered_by: bool,
}

impl Footer {
    /// Read the footer keys. A key the map omits or holds in another shape
    /// reads as empty; the credit shows unless the map turns it off.
    #[must_use]
    pub fn from_value(value: &Value) -> Self {
        let columns: Vec<String> = match value.get("footer_columns") {
            Some(Value::Array(columns)) => columns
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|column| !column.is_empty())
                .take(MAX_COLUMNS)
                .map(str::to_string)
                .collect(),
            _ => stored_text(value, "footer_text").into_iter().collect(),
        };
        Self {
            columns,
            links: value
                .get("footer_links")
                .and_then(|links| parse_navigation(links).ok()),
            contact: stored_text(value, "footer_contact"),
            licence: value
                .get("footer_licence")
                .and_then(Value::as_str)
                .and_then(Licence::parse)
                .unwrap_or(Licence::Unstated),
            powered_by: value
                .get("show_powered_by")
                .and_then(Value::as_bool)
                .unwrap_or(true),
        }
    }

    /// The footer element: the site name with the columns, the links, the
    /// contact block, the licence and the credit, each only when it has
    /// content.
    #[must_use]
    pub fn render(&self, ctx: &RenderContext<'_>, tr: Translator) -> String {
        let messages = tr.messages();
        let mut out =
            String::from("<footer class=\"site-footer\">\n<div class=\"footer-inner\">\n");
        out.push_str(&format!(
            "<div class=\"footer-site\"><p class=\"footer-title\">{}</p>",
            escape_html(&ctx.site.title)
        ));
        if !self.columns.is_empty() {
            out.push_str("<div class=\"footer-texts\">");
            for column in &self.columns {
                out.push_str(&format!(
                    "<div class=\"footer-text\">{}</div>",
                    omeka_core::purify::purify(column)
                ));
            }
            out.push_str("</div>");
        }
        out.push_str("</div>\n");
        out.push_str(&self.links_markup(ctx, tr));
        if let Some(contact) = &self.contact {
            out.push_str(&format!(
                "<address class=\"footer-contact\">{}</address>\n",
                omeka_core::purify::purify(contact)
            ));
        }
        match (self.licence, self.licence.deed()) {
            (_, Some((name, url))) => out.push_str(&format!(
                "<p class=\"footer-licence\"><a rel=\"license\" class=\"licence-badge\" href=\"{url}\">\
                 <span class=\"licence-mark\" aria-hidden=\"true\">cc</span>{name}</a></p>\n"
            )),
            (Licence::AllRightsReserved, None) => out.push_str(&format!(
                "<p class=\"footer-licence\">© {}</p>\n",
                escape_html(messages.all_rights_reserved)
            )),
            _ => {}
        }
        if self.powered_by {
            out.push_str(&format!(
                "<p class=\"powered-by\">{}</p>\n",
                escape_html(messages.powered_by)
            ));
        }
        out.push_str("</div>\n</footer>");
        out
    }

    /// The links list: the site's own, or the header menu's top level. A page
    /// link to a page the reader cannot see is left out, as the header menu
    /// leaves it out; children are not shown.
    fn links_markup(&self, ctx: &RenderContext<'_>, tr: Translator) -> String {
        let items = match &self.links {
            Some(links) => self.resolved(links, ctx, tr),
            None => ctx.nav.clone(),
        };
        let mut out = String::new();
        for item in items.iter().filter(|item| !item.url.is_empty()) {
            out.push_str("<li>");
            out.push_str(&hyperlink(&item.label, Some(&item.url), &[]));
            out.push_str("</li>");
        }
        if out.is_empty() {
            return out;
        }
        format!(
            "<nav class=\"footer-links\" aria-label=\"{}\"><ul>{out}</ul></nav>\n",
            escape_html(tr.messages().footer_links)
        )
    }

    fn resolved(&self, links: &[NavNode], ctx: &RenderContext<'_>, tr: Translator) -> Vec<NavItem> {
        let visible: Vec<NavNode> = links
            .iter()
            .filter(|node| match &node.link {
                NavLink::Page { id, .. } => ctx.pages.iter().any(|page| page.id == *id),
                _ => true,
            })
            .map(|node| NavNode {
                link: node.link.clone(),
                links: Vec::new(),
            })
            .collect();
        if visible.is_empty() {
            return Vec::new();
        }
        resolve(&visible, &ctx.site.slug, &ctx.pages, tr.messages())
    }
}
