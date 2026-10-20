//! Reading a Romeka block's stored settings: the scalar shapes a form and a
//! stored document both write, and the link a button or a call to action
//! names.
//!
//! The settings are the block's own JSON in `o:layout_data.romeka.settings`.
//! Every reader takes an absent or malformed member as the empty value, so a
//! block parses whatever an older form or an outside edit stored.

use omeka_core::ids::{ItemId, PageId};
use omeka_core::resource::ResourceKind;
use serde_json::Value as JsonValue;

use super::romeka::{ItemSelection, PlainRefs};
use crate::slug::page_url;
use crate::view::{RenderContext, ResourceView};

/// The string under `key`, empty when absent or not a string.
pub(crate) fn text(settings: &JsonValue, key: &str) -> String {
    settings
        .get(key)
        .and_then(JsonValue::as_str)
        .unwrap_or("")
        .to_string()
}

/// A whole number from a JSON number or a numeric string, the two shapes a
/// form and a stored document use.
pub(crate) fn whole(value: &JsonValue) -> Option<u64> {
    match value {
        JsonValue::Number(number) => number.as_u64(),
        JsonValue::String(text) => text.trim().parse::<u64>().ok(),
        _ => None,
    }
}

/// The whole number under `key`.
pub(crate) fn whole_at(settings: &JsonValue, key: &str) -> Option<u64> {
    settings.get(key).and_then(whole)
}

/// The item id under `key`.
pub(crate) fn item_at(settings: &JsonValue, key: &str) -> Option<ItemId> {
    whole_at(settings, key)
        .filter(|id| *id > 0)
        .map(ItemId::new)
}

/// The checkbox under `key`, `default` when absent.
pub(crate) fn flag(settings: &JsonValue, key: &str, default: bool) -> bool {
    settings
        .get(key)
        .and_then(JsonValue::as_bool)
        .unwrap_or(default)
}

/// The records of the list under `key`, skipping anything that is not an
/// object.
pub(crate) fn records<'a>(
    settings: &'a JsonValue,
    key: &str,
) -> impl Iterator<Item = &'a JsonValue> + 'a {
    settings
        .get(key)
        .and_then(JsonValue::as_array)
        .into_iter()
        .flatten()
        .filter(|record| record.is_object())
}

/// A URL safe to put in an `href`: relative, or an absolute http, https or
/// mailto URL. A stored `javascript:` or `data:` target renders nothing.
pub(crate) fn safe_url(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    match trimmed.split_once(':') {
        Some((scheme, _)) if !scheme.contains('/') => {
            let scheme = scheme.to_ascii_lowercase();
            matches!(scheme.as_str(), "http" | "https" | "mailto").then(|| trimmed.to_string())
        }
        _ => Some(trimmed.to_string()),
    }
}

/// One link with its text and its target, a page of the site or a URL: a
/// hero's call to action or a button.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Button {
    pub(crate) label: String,
    pub(crate) page: Option<PageId>,
    pub(crate) url: String,
}

impl Button {
    /// The button whose keys start with `prefix`: `<prefix>_label`,
    /// `<prefix>_page` and `<prefix>_url`.
    pub(crate) fn parse(settings: &JsonValue, prefix: &str) -> Self {
        Self {
            label: text(settings, &format!("{prefix}_label")),
            page: whole_at(settings, &format!("{prefix}_page")).map(PageId::new),
            url: text(settings, &format!("{prefix}_url")),
        }
    }

    /// The target on the rendered page: the stored page when it is still in
    /// the site, else the URL when it is a safe one. A button with no text
    /// has no target, because it would render as an empty link.
    pub(crate) fn href(&self, ctx: &RenderContext<'_>) -> Option<String> {
        if self.label.is_empty() {
            return None;
        }
        if let Some(id) = self.page
            && let Some(page) = ctx.pages.iter().find(|page| page.id == id)
        {
            return Some(page_url(&ctx.site.slug, &page.slug));
        }
        safe_url(&self.url)
    }

    /// The same target at write time, where only the resolved page URLs are
    /// available.
    pub(crate) fn plain_href(&self, refs: &PlainRefs) -> Option<String> {
        if self.label.is_empty() {
            return None;
        }
        if let Some(id) = self.page
            && let Some(url) = refs.page_of(id)
        {
            return Some(url.to_string());
        }
        safe_url(&self.url)
    }
}

/// The items a block selects with a `source` choice: the `query` search when
/// the source is `query`, else the ids picked under `items`, `None` before
/// any is picked.
pub(crate) fn selection(settings: &JsonValue) -> Option<ItemSelection> {
    if text(settings, "source") == "query" {
        return Some(ItemSelection::Query(text(settings, "query")));
    }
    let picked: Vec<ItemId> = settings
        .get("items")
        .and_then(JsonValue::as_array)
        .map(|ids| {
            ids.iter()
                .filter_map(whole)
                .filter(|id| *id > 0)
                .map(ItemId::new)
                .collect()
        })
        .unwrap_or_default();
    (!picked.is_empty()).then_some(ItemSelection::Picked(picked))
}

/// The selected items the visitor may see, in selection order: each picked
/// item, or what the search finds on the site, at most
/// [`ItemSelection::LIMIT`].
pub(crate) fn selected(
    selection: Option<&ItemSelection>,
    ctx: &RenderContext<'_>,
) -> Vec<ResourceView> {
    match selection {
        None => Vec::new(),
        Some(ItemSelection::Picked(ids)) => ids
            .iter()
            .filter_map(|id| ctx.resources.read(ResourceKind::Item, id.get()))
            .collect(),
        Some(ItemSelection::Query(query)) => {
            let search = ItemSelection::browse_query(query, ctx.site.id, ItemSelection::LIMIT);
            ctx.resources.search(ResourceKind::Item, &search).resources
        }
    }
}
