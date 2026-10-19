//! Site navigation: the stored navigation tree, its boundary parse, and the
//! resolution of each link to a label and a URL.
//!
//! Upstream reference (`source/omeka-s/application`): `src/Site/Navigation/Link`
//! (`Page.php`, `Url.php`, `Browse.php`, `BrowseItemSets.php`, `Fallback.php`),
//! `src/Site/Navigation/Translator.php` and `src/Site/Navigation/Page/UriWithQuery.php`.
//!
//! `site.navigation` holds a nested list of nodes. Each node is
//! `{type, data, links}`; `data` depends on the type:
//!
//! | type | `data` keys |
//! | --- | --- |
//! | `page` | `id`, `label` |
//! | `url` | `label`, `url`, `target_blank` |
//! | `browse` / `browseItemSets` | `label`, `query` |
//!
//! Anything else resolves through the `Fallback` link, exactly as the link
//! manager's `get()` does. Core's `Site` omits `navigation` today, so the tree
//! arrives as JSON and [`parse_navigation`] validates it here, at the boundary.
//! Inside, [`NavNode`] is trusted.
//!
//! Validation and default labels come from the link classes: a `page` link
//! needs a numeric `id`; a `url` link needs a non-empty label and URL, rejects
//! C0 control characters and the `javascript` scheme. A label the user did not
//! set falls back to the link type's own name, translated by upstream's
//! `Translator::getLinkLabel`.

use serde_json::Value as JsonValue;

use omeka_core::ids::PageId;
use omeka_core::site::Slug;

use crate::i18n::Messages;
use crate::slug::{page_url, resource_browse_url, site_url};

/// Why the stored navigation JSON is not a valid tree.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NavError {
    /// The top level or a `links` child is not a JSON array.
    #[error("navigation{path} must be a list")]
    NotAList {
        /// The JSON path of the offending value.
        path: String,
    },
    /// A node is not a JSON object or has no string `type`.
    #[error("navigation{path} must be an object with a string type")]
    MissingType {
        /// The JSON path of the offending node.
        path: String,
    },
    /// A `page` link has no numeric `id`
    /// (`Link/Page.php::isValid`).
    #[error("navigation{path}.data.id must be a page id")]
    InvalidPageId {
        /// The JSON path of the offending node.
        path: String,
    },
    /// A `url` link has an empty label (`Link/Url.php::isValid`).
    #[error("navigation{path}.data.label must not be empty")]
    UrlMissingLabel {
        /// The JSON path of the offending node.
        path: String,
    },
    /// A `url` link has an empty URL after tab and newline stripping
    /// (`Link/Url.php::isValid`).
    #[error("navigation{path}.data.url must not be empty")]
    UrlMissingUrl {
        /// The JSON path of the offending node.
        path: String,
    },
    /// A `url` link URL holds a C0 control character
    /// (`Link/Url.php::isValid`).
    #[error("navigation{path}.data.url contains a control character")]
    UrlControlCharacter {
        /// The JSON path of the offending node.
        path: String,
    },
    /// A `url` link URL uses the `javascript` scheme
    /// (`Link/Url.php::isValid`).
    #[error("navigation{path}.data.url uses the javascript scheme")]
    UrlJavascriptScheme {
        /// The JSON path of the offending node.
        path: String,
    },
}

/// One node's link, the four registered types plus the fallback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NavLink {
    /// `page`: a link to a page of the same site.
    Page {
        /// The target page id.
        id: PageId,
        /// The user label, `None` when unset.
        label: Option<String>,
    },
    /// `url`: a link to any URL.
    Url {
        /// The user label, `None` when unset.
        label: Option<String>,
        /// The target URL, tab and newline stripped.
        url: String,
        /// Whether the link opens in a new tab.
        target_blank: bool,
    },
    /// `browse`: a link to the site's item browse.
    Browse {
        /// The user label, `None` when unset.
        label: Option<String>,
        /// The advanced-search query string, `None` when unset.
        query: Option<String>,
    },
    /// `browseItemSets`: a link to the site's item-set browse.
    BrowseItemSets {
        /// The user label, `None` when unset.
        label: Option<String>,
        /// The advanced-search query string, `None` when unset.
        query: Option<String>,
    },
    /// An unregistered type, kept by name.
    Fallback {
        /// The unregistered type name.
        name: String,
    },
}

/// One navigation node: a link plus its children.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavNode {
    /// The node's link.
    pub link: NavLink,
    /// Child nodes, in stored order.
    pub links: Vec<NavNode>,
}

/// A page of the site, as navigation resolution needs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageRef {
    /// `site_page.id`.
    pub id: PageId,
    /// `site_page.slug`.
    pub slug: Slug,
    /// `site_page.title`.
    pub title: String,
    /// `site_page.is_public`.
    pub is_public: bool,
}

/// A resolved navigation item: what a theme renders in a menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavItem {
    /// The link label, the user's label or the link type's default name.
    pub label: String,
    /// The link target. Empty when the node cannot resolve, as upstream's
    /// `Translator::getLinkUrl` returns an empty string for a `uri` link with a
    /// null URI.
    pub url: String,
    /// Whether the link opens in a new tab.
    pub target_blank: bool,
    /// The target page, when the link is a page link that resolves.
    pub page: Option<PageId>,
    /// Resolved child items.
    pub children: Vec<NavItem>,
}

/// Parse the stored `site.navigation` JSON.
///
/// # Errors
/// [`NavError`] naming the failing JSON path, e.g. `[0].links[1].data.url`.
pub fn parse_navigation(value: &JsonValue) -> Result<Vec<NavNode>, NavError> {
    match value {
        JsonValue::Null => Ok(Vec::new()),
        JsonValue::Array(nodes) => {
            let mut out = Vec::with_capacity(nodes.len());
            for (index, node) in nodes.iter().enumerate() {
                out.push(parse_node(node, &format!("[{index}]"))?);
            }
            Ok(out)
        }
        _ => Err(NavError::NotAList {
            path: String::new(),
        }),
    }
}

fn parse_node(node: &JsonValue, path: &str) -> Result<NavNode, NavError> {
    let JsonValue::Object(object) = node else {
        return Err(NavError::MissingType {
            path: path.to_string(),
        });
    };
    let Some(JsonValue::String(kind)) = object.get("type") else {
        return Err(NavError::MissingType {
            path: path.to_string(),
        });
    };
    let data = object.get("data").unwrap_or(&JsonValue::Null);
    let link = match kind.as_str() {
        "page" => NavLink::Page {
            id: parse_page_id(data, path)?,
            label: text_field(data, "label"),
        },
        "url" => parse_url_link(data, path)?,
        "browse" => NavLink::Browse {
            label: text_field(data, "label"),
            query: text_field(data, "query"),
        },
        "browseItemSets" => NavLink::BrowseItemSets {
            label: text_field(data, "label"),
            query: text_field(data, "query"),
        },
        other => NavLink::Fallback {
            name: other.to_string(),
        },
    };
    let mut links = Vec::new();
    match object.get("links") {
        None | Some(JsonValue::Null) => {}
        Some(JsonValue::Array(children)) => {
            for (index, child) in children.iter().enumerate() {
                links.push(parse_node(child, &format!("{path}.links[{index}]"))?);
            }
        }
        Some(_) => {
            return Err(NavError::NotAList {
                path: format!("{path}.links"),
            });
        }
    }
    Ok(NavNode { link, links })
}

fn parse_page_id(data: &JsonValue, path: &str) -> Result<PageId, NavError> {
    let raw = data.get("id");
    let id = match raw {
        Some(JsonValue::Number(number)) => number.as_u64(),
        Some(JsonValue::String(text)) => text.trim().parse::<u64>().ok(),
        _ => None,
    };
    id.map(PageId::new).ok_or_else(|| NavError::InvalidPageId {
        path: path.to_string(),
    })
}

fn parse_url_link(data: &JsonValue, path: &str) -> Result<NavLink, NavError> {
    let label = text_field(data, "label").unwrap_or_default();
    let raw_url = text_field(data, "url").unwrap_or_default();
    let url: String = raw_url
        .chars()
        .filter(|ch| !matches!(ch, '\t' | '\r' | '\n'))
        .collect();
    let url = url.trim().to_string();
    if label.trim().is_empty() {
        return Err(NavError::UrlMissingLabel {
            path: path.to_string(),
        });
    }
    if url.is_empty() {
        return Err(NavError::UrlMissingUrl {
            path: path.to_string(),
        });
    }
    if url.chars().any(|ch| ch.is_control()) {
        return Err(NavError::UrlControlCharacter {
            path: path.to_string(),
        });
    }
    if scheme(&url).is_some_and(|scheme| scheme == "javascript") {
        return Err(NavError::UrlJavascriptScheme {
            path: path.to_string(),
        });
    }
    let target_blank = data
        .get("target_blank")
        .and_then(JsonValue::as_bool)
        .unwrap_or(false);
    Ok(NavLink::Url {
        label: Some(label),
        url,
        target_blank,
    })
}

fn text_field(data: &JsonValue, key: &str) -> Option<String> {
    match data.get(key) {
        Some(JsonValue::String(value)) => Some(value.clone()),
        Some(JsonValue::Number(value)) => Some(value.to_string()),
        _ => None,
    }
}

/// The URL scheme, lowercased, as `parse_url(..., PHP_URL_SCHEME)` returns it.
fn scheme(url: &str) -> Option<String> {
    let (scheme, _) = url.split_once(':')?;
    if scheme.is_empty()
        || !scheme
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '+' | '-' | '.'))
        || !scheme.starts_with(|ch: char| ch.is_ascii_alphabetic())
    {
        return None;
    }
    Some(scheme.to_ascii_lowercase())
}

/// Resolve the stored tree into items a theme can render.
///
/// The label is the user's, else the link type's name
/// (`Translator::getLinkLabel`), translated. An empty tree resolves to the
/// single "Home" link upstream synthesises (`Translator::toZend`).
#[must_use]
pub fn resolve(
    nav: &[NavNode],
    site: &Slug,
    pages: &[PageRef],
    messages: &Messages,
) -> Vec<NavItem> {
    if nav.is_empty() {
        return vec![NavItem {
            label: messages.home.to_string(),
            url: site_url(site),
            target_blank: false,
            page: None,
            children: Vec::new(),
        }];
    }
    nav.iter()
        .map(|node| resolve_node(node, site, pages, messages))
        .collect()
}

fn resolve_node(node: &NavNode, site: &Slug, pages: &[PageRef], messages: &Messages) -> NavItem {
    let page = match &node.link {
        NavLink::Page { id, .. } if find_page_ref(pages, *id).is_some() => Some(*id),
        _ => None,
    };
    NavItem {
        label: label_for(&node.link, pages, messages),
        url: link_url(&node.link, site, pages),
        target_blank: matches!(
            &node.link,
            NavLink::Url {
                target_blank: true,
                ..
            }
        ),
        page,
        children: node
            .links
            .iter()
            .map(|child| resolve_node(child, site, pages, messages))
            .collect(),
    }
}

/// The link's label: the user's label as typed when set, else the link type's
/// name in the site's locale. Upstream translates only the fallback
/// (`Translator::getLinkLabel`).
#[must_use]
pub fn label_for(link: &NavLink, pages: &[PageRef], messages: &Messages) -> String {
    let user = match link {
        NavLink::Page { id, label } => {
            non_empty(label).or_else(|| find_page_ref(pages, *id).map(|page| page.title.clone()))
        }
        NavLink::Url { label, .. }
        | NavLink::Browse { label, .. }
        | NavLink::BrowseItemSets { label, .. } => non_empty(label),
        NavLink::Fallback { .. } => None,
    };
    user.unwrap_or_else(|| {
        match link {
            NavLink::Page { .. } | NavLink::Fallback { .. } => messages.missing_page,
            NavLink::Url { .. } => messages.custom_url,
            NavLink::Browse { .. } => messages.browse,
            NavLink::BrowseItemSets { .. } => messages.browse_item_sets,
        }
        .to_string()
    })
}

fn non_empty(label: &Option<String>) -> Option<String> {
    label
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn find_page_ref(pages: &[PageRef], id: PageId) -> Option<&PageRef> {
    pages.iter().find(|page| page.id == id)
}

/// The URL a link resolves to (`Translator::getLinkUrl` over each
/// `toZend`). An unresolvable link yields an empty string.
#[must_use]
pub fn link_url(link: &NavLink, site: &Slug, pages: &[PageRef]) -> String {
    match link {
        NavLink::Page { id, .. } => find_page_ref(pages, *id)
            .map(|page| page_url(site, &page.slug))
            .unwrap_or_default(),
        NavLink::Url { url, .. } => url.clone(),
        NavLink::Browse { query, .. } => {
            resource_browse_url(site, "item", query_string_option(query).as_deref())
        }
        NavLink::BrowseItemSets { query, .. } => {
            resource_browse_url(site, "item-set", query_string_option(query).as_deref())
        }
        NavLink::Fallback { .. } => String::new(),
    }
}

/// A browse link's query string, `None` when absent or empty.
fn query_string_option(query: &Option<String>) -> Option<String> {
    query.as_ref().filter(|value| !value.is_empty()).cloned()
}

/// Render a resolved navigation tree as a menu, the markup
/// `Laminas\View\Helper\Navigation\Menu::renderMenu` produces for the classes
/// the port uses.
///
/// `max_depth` counts the levels of children below the list itself: `Some(0)`
/// renders the immediate items and stops, `None` renders every level. This is
/// the reading the table-of-contents block depends on, where `depth` 1 shows
/// one level of child pages. The exact Laminas markup (per-item classes,
/// `aria` attributes, translated `title`) is not reproduced.
#[must_use]
pub fn render_menu(items: &[NavItem], max_depth: Option<usize>, class: &str) -> String {
    render_menu_with_id(items, max_depth, class, None)
}

/// [`render_menu`] with the `id` the default theme passes for its top
/// navigation (`renderMenu(null, ['id' => 'top-nav-contents'])`).
#[must_use]
pub fn render_menu_with_id(
    items: &[NavItem],
    max_depth: Option<usize>,
    class: &str,
    id: Option<&str>,
) -> String {
    if items.is_empty() {
        return String::new();
    }
    render_list(items, 0, max_depth, class, id)
}

fn render_list(
    items: &[NavItem],
    level: usize,
    max_depth: Option<usize>,
    class: &str,
    id: Option<&str>,
) -> String {
    let mut out = match id {
        Some(id) => format!(
            "<ul class=\"{class}\" id=\"{}\">",
            crate::html::escape_html(id)
        ),
        None => format!("<ul class=\"{class}\">"),
    };
    for item in items {
        out.push_str("<li>");
        // `Link/Url.php::toZend` sets `target` to `_blank` for a URL link the
        // user marked, and the menu helper renders every page option as an
        // attribute.
        let mut attributes = Vec::new();
        if item.target_blank {
            attributes.push(crate::html::Attr::text("target", "_blank"));
        }
        out.push_str(&crate::html::hyperlink(
            &item.label,
            Some(&item.url),
            &attributes,
        ));
        if !item.children.is_empty() && max_depth.is_none_or(|depth| level < depth) {
            out.push_str(&render_list(
                &item.children,
                level + 1,
                max_depth,
                "navigation",
                None,
            ));
        }
        out.push_str("</li>");
    }
    out.push_str("</ul>");
    out
}

/// The chain of items from the root to a page, for a breadcrumb trail:
/// `[Home, Guide, Chapter]`. `None` when the page is not in the tree.
#[must_use]
pub fn nav_path(items: &[NavItem], page: PageId) -> Option<Vec<&NavItem>> {
    for item in items {
        if item.page == Some(page) {
            return Some(vec![item]);
        }
        if let Some(mut tail) = nav_path(&item.children, page) {
            let mut path = vec![item];
            path.append(&mut tail);
            return Some(path);
        }
    }
    None
}

/// Find the item for a page, for a sub-menu: the node whose page id matches.
#[must_use]
pub fn find_item_by_page(items: &[NavItem], page: PageId) -> Option<&NavItem> {
    fn search(items: &[NavItem], page: PageId) -> Option<&NavItem> {
        for item in items {
            if item.page == Some(page) {
                return Some(item);
            }
            if let Some(found) = search(&item.children, page) {
                return Some(found);
            }
        }
        None
    }
    search(items, page)
}

#[cfg(test)]
#[path = "nav_tests.rs"]
mod tests;
