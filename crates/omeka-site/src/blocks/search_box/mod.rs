//! The `searchBox` Romeka block: a full-text search form for the site, its
//! items or its item sets, with the curator's placeholder.
//!
//! The purifier allows no `form`, so stored html cannot carry one. The block
//! is stored as an upstream `html` block whose plain rendering is a link to
//! the same search page, relative to the page so it holds after a slug change
//! and on an upstream installation.

use serde_json::Value as JsonValue;

use super::romeka::{PlainRefs, RomkaBlock};
use super::schema::{ChoiceOption, LayoutSchema, choice, option, text};
use super::setting::text as text_of;
use crate::html::{Attr, escape_html, open_tag};
use crate::slug::{resource_browse_url, site_url};
use crate::view::RenderContext;

/// The `searchBox` block.
#[derive(Debug, Clone, Copy, Default)]
pub struct SearchBox;

const RESOURCE: &[ChoiceOption] = &[
    option("all", "romekaChoice.searchBox.all"),
    option("items", "romekaChoice.searchBox.items"),
    option("item_sets", "romekaChoice.searchBox.item_sets"),
];

const SCHEMA: LayoutSchema = LayoutSchema {
    fields: &[
        text(
            "placeholder",
            "romekaField.searchBox.placeholder",
            "fieldHelp.searchBox.placeholder",
        )
        .placeholder("fieldExample.searchBox.placeholder"),
        choice(
            "resource",
            "romekaField.searchBox.resource",
            "fieldHelp.searchBox.resource",
            RESOURCE,
        )
        .default_text("all"),
    ],
};

/// What the form searches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Scope {
    /// The site search page, every resource and page.
    All,
    Items,
    ItemSets,
}

impl Scope {
    fn parse(settings: &JsonValue) -> Self {
        match text_of(settings, "resource").as_str() {
            "items" => Self::Items,
            "item_sets" => Self::ItemSets,
            _ => Self::All,
        }
    }

    /// The page's path under the site, `index/search` or a browse.
    fn path(self) -> &'static str {
        match self {
            Self::All => "index/search",
            Self::Items => "item",
            Self::ItemSets => "item-set",
        }
    }
}

/// The curator's placeholder, else the site's "Search the collection".
fn placeholder<'a>(settings: &'a JsonValue, fallback: &'a str) -> std::borrow::Cow<'a, str> {
    let stored = text_of(settings, "placeholder");
    if stored.trim().is_empty() {
        fallback.into()
    } else {
        stored.into()
    }
}

impl RomkaBlock for SearchBox {
    fn name(&self) -> &'static str {
        "searchBox"
    }

    fn label(&self) -> &'static str {
        "romeka.searchBox"
    }

    fn schema(&self) -> &'static LayoutSchema {
        &SCHEMA
    }

    fn render(&self, settings: &JsonValue, ctx: &RenderContext<'_>) -> String {
        let scope = Scope::parse(settings);
        let labels = ctx.translator().search_labels();
        let action = match scope {
            Scope::All => format!("{}/{}", site_url(&ctx.site.slug), scope.path()),
            _ => resource_browse_url(&ctx.site.slug, scope.path(), None),
        };
        let hint = placeholder(settings, ctx.translator().messages().search_placeholder);
        let mut out = open_tag(
            "form",
            &[
                Attr::text("class", "romeka-search"),
                Attr::text("action", &action),
                Attr::text("method", "get"),
                Attr::text("role", "search"),
                Attr::text("aria-label", labels.search),
            ],
        );
        out.push_str(&format!(
            "<label class=\"romeka-search-field\"><span class=\"visually-hidden\">{}</span>",
            escape_html(labels.query)
        ));
        out.push_str(&open_tag(
            "input",
            &[
                Attr::text("class", "romeka-search-input"),
                Attr::text("type", "search"),
                Attr::text("name", "fulltext_search"),
                Attr::text("placeholder", &hint),
            ],
        ));
        out.push_str(&format!(
            "</label><button class=\"romeka-search-submit\" type=\"submit\">{}</button></form>",
            escape_html(labels.search)
        ));
        out
    }

    fn plain(&self, settings: &JsonValue, refs: &PlainRefs) -> String {
        let scope = Scope::parse(settings);
        format!(
            "<p><a href=\"../{}\">{}</a></p>",
            scope.path(),
            escape_html(&placeholder(settings, refs.messages().search_placeholder))
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks::test_support::{context, site};
    use crate::i18n::Locale;
    use crate::view::NoResources;
    use serde_json::json;

    /// The purifier drops a `form`, which is why the plain rendering is a link.
    #[test]
    fn the_purifier_drops_a_form() {
        let purified = omeka_core::purify::purify(
            "<form action=\"/s/main/index/search\"><input name=\"q\"></form>",
        );
        assert!(!purified.contains("<form"), "{purified}");
        assert!(!purified.contains("<input"), "{purified}");
    }

    /// The plain rendering links the search page relative to the page, with
    /// the curator's words or the site's own.
    #[test]
    fn plain_links_the_search_page() {
        assert_eq!(
            SearchBox.plain(&json!({"placeholder": "Find a ship"}), &PlainRefs::new()),
            "<p><a href=\"../index/search\">Find a ship</a></p>"
        );
        assert_eq!(
            SearchBox.plain(
                &json!({"resource": "items"}),
                &PlainRefs::new().locale(Locale::Es)
            ),
            format!(
                "<p><a href=\"../item\">{}</a></p>",
                crate::i18n::ES.search_placeholder
            )
        );
    }

    /// The rich rendering is a full-text search form for the chosen scope.
    #[test]
    fn rich_renders_a_search_form() {
        let site = site();
        let ctx = context(&site, &NoResources);
        assert_eq!(
            SearchBox.render(&json!({"placeholder": "Find a ship"}), &ctx),
            "<form class=\"romeka-search\" action=\"/s/main/index/search\" method=\"get\" role=\"search\" \
             aria-label=\"Search\"><label class=\"romeka-search-field\"><span class=\"visually-hidden\">Query</span>\
             <input class=\"romeka-search-input\" type=\"search\" name=\"fulltext_search\" placeholder=\"Find a ship\">\
             </label><button class=\"romeka-search-submit\" type=\"submit\">Search</button></form>"
        );
        let sets = SearchBox.render(&json!({"resource": "item_sets"}), &ctx);
        assert!(sets.contains("action=\"/s/main/item-set\""), "{sets}");
        assert!(
            sets.contains("placeholder=\"Search the collection\""),
            "{sets}"
        );
    }
}
