//! `SiteAdapter::validateNavigation` (`Api/Adapter/SiteAdapter.php:369-411`).
//!
//! The stored navigation tree is whatever JSON a client sent, so the write
//! boundary proves it is a tree before the row is written. Each registered
//! link type also validates its `data` (`Site/Navigation/Link/*.php`); an
//! unregistered type is the `Fallback` link and always valid.
//!
//! The messages are upstream's, and a failing link adds its own message before
//! the generic one, in that order, because `isValid` writes to the same error
//! store the caller found false from.

use serde_json::Value;

use crate::errors::ValidationErrors;

const KEY: &str = "o:navigation";

/// Validate one navigation tree, appending to `errors` under `o:navigation`.
///
/// Stops each list at its first failure, as the upstream closure's `return`
/// does; a failure in a child list does not stop its parent.
pub fn validate(navigation: &Value, errors: &mut ValidationErrors) {
    let Value::Array(nodes) = navigation else {
        push(errors, "Invalid navigation: navigation must be an array");
        return;
    };
    let mut pages = Vec::new();
    validate_links(nodes, &mut pages, errors);
}

fn validate_links(links: &[Value], pages: &mut Vec<f64>, errors: &mut ValidationErrors) {
    for node in links {
        let Some(object) = node.as_object() else {
            push(errors, "Invalid navigation: link missing type");
            return;
        };
        let Some(kind) = object.get("type").and_then(Value::as_str) else {
            push(errors, "Invalid navigation: link missing type");
            return;
        };
        let Some(data) = object.get("data") else {
            push(errors, "Invalid navigation: link missing data");
            return;
        };
        if !link_is_valid(kind, data, errors) {
            push(errors, "Invalid navigation: invalid link data");
            return;
        }
        if kind == "page" {
            let Some(id) = page_id(data) else {
                // `link_is_valid` already refused a non-numeric id.
                return;
            };
            if pages.contains(&id) {
                push(errors, "Invalid navigation: page links must be unique");
                return;
            }
            pages.push(id);
        }
        if let Some(children) = object.get("links") {
            let Some(children) = children.as_array() else {
                push(errors, "Invalid navigation: links must be an array");
                return;
            };
            validate_links(children, pages, errors);
        }
    }
}

/// `LinkInterface::isValid` for one link type.
fn link_is_valid(kind: &str, data: &Value, errors: &mut ValidationErrors) -> bool {
    match kind {
        "page" => {
            if page_id(data).is_none() {
                push(errors, "Invalid navigation: page link missing page ID");
                return false;
            }
            true
        }
        "url" => url_is_valid(data, errors),
        // `Browse`, `BrowseItemSets` and the `Fallback` for an unregistered
        // type accept any data.
        _ => true,
    }
}

/// `Link/Url.php::isValid`. The checks run in the order the file writes them,
/// so the reported message is the first one that fails.
fn url_is_valid(data: &Value, errors: &mut ValidationErrors) -> bool {
    let label = data
        .get("label")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("");
    if label.is_empty() {
        push(errors, "Invalid navigation: URL link missing label");
        return false;
    }
    let raw = data.get("url").and_then(Value::as_str).unwrap_or("");
    let url = raw.trim().replace(['\t', '\r', '\n'], "");
    if url.is_empty() {
        push(errors, "Invalid navigation: URL link missing URL");
        return false;
    }
    if url.chars().any(|ch| (ch as u32) < 0x20) {
        push(
            errors,
            "Invalid navigation: URL link contains invalid character",
        );
        return false;
    }
    if scheme(&url.to_lowercase()).is_some_and(|scheme| scheme == "javascript") {
        push(errors, "Invalid navigation: URL link invalid scheme");
        return false;
    }
    true
}

/// A `page` link's `data.id` as `is_numeric` reads it, for the uniqueness test.
///
/// Upstream's `in_array` is loose, so `1` and `"1"` are the same page; a
/// `f64` comparison keeps that reading.
fn page_id(data: &Value) -> Option<f64> {
    let id = data.get("id")?;
    if let Some(number) = id.as_f64() {
        return Some(number);
    }
    id.as_str()?.trim().parse::<f64>().ok()
}

/// `parse_url($url, PHP_URL_SCHEME)`: the text before the first colon, and only
/// when that colon comes before any `/`, `?` or `#` and the text is a valid
/// scheme.
fn scheme(url: &str) -> Option<&str> {
    let end = url.find([':', '/', '?', '#'])?;
    if !url[end..].starts_with(':') {
        return None;
    }
    let candidate = &url[..end];
    let mut chars = candidate.chars();
    let first = chars.next()?;
    if !first.is_ascii_alphabetic() {
        return None;
    }
    if !chars.all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '+' | '-' | '.')) {
        return None;
    }
    Some(candidate)
}

fn push(errors: &mut ValidationErrors, message: &str) {
    errors
        .entry(KEY.to_string())
        .or_default()
        .push(message.to_string());
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use serde_json::json;

    use super::*;

    fn errors_for(navigation: &Value) -> Vec<String> {
        let mut errors = ValidationErrors::new();
        validate(navigation, &mut errors);
        errors.get(KEY).cloned().unwrap_or_default()
    }

    #[test]
    fn a_tree_that_is_not_a_list_is_refused() {
        assert_eq!(
            errors_for(&json!("browse")),
            ["Invalid navigation: navigation must be an array"]
        );
    }

    #[test]
    fn a_link_without_a_type_is_refused() {
        assert_eq!(
            errors_for(&json!([{"data": {"label": "x"}}])),
            ["Invalid navigation: link missing type"]
        );
    }

    #[test]
    fn a_link_without_data_is_refused() {
        assert_eq!(
            errors_for(&json!([{"type": "browse"}])),
            ["Invalid navigation: link missing data"]
        );
    }

    #[test]
    fn a_url_link_reports_its_own_error_before_the_generic_one() {
        assert_eq!(
            errors_for(&json!([{"type": "url", "data": {"label": "x"}}])),
            [
                "Invalid navigation: URL link missing URL",
                "Invalid navigation: invalid link data"
            ]
        );
    }

    #[test]
    fn a_url_link_rejects_the_javascript_scheme() {
        assert_eq!(
            errors_for(&json!([{
                "type": "url",
                "data": {"label": "x", "url": "JavaScript:alert(1)"}
            }])),
            [
                "Invalid navigation: URL link invalid scheme",
                "Invalid navigation: invalid link data"
            ]
        );
    }

    #[test]
    fn a_url_link_rejects_a_control_character_after_stripping_tabs_and_newlines() {
        assert_eq!(
            errors_for(&json!([{
                "type": "url",
                "data": {"label": "x", "url": "https://x.test/\u{1}"}
            }])),
            [
                "Invalid navigation: URL link contains invalid character",
                "Invalid navigation: invalid link data"
            ]
        );
    }

    #[test]
    fn a_page_link_needs_a_numeric_id() {
        assert_eq!(
            errors_for(&json!([{"type": "page", "data": {"label": "x"}}])),
            [
                "Invalid navigation: page link missing page ID",
                "Invalid navigation: invalid link data"
            ]
        );
    }

    #[test]
    fn page_links_must_be_unique() {
        assert_eq!(
            errors_for(&json!([
                {"type": "page", "data": {"id": 1}},
                {"type": "page", "data": {"id": "1"}}
            ])),
            ["Invalid navigation: page links must be unique"]
        );
    }

    #[test]
    fn a_child_list_that_is_not_a_list_is_refused() {
        assert_eq!(
            errors_for(&json!([
                {"type": "browse", "data": {"label": "x"}, "links": "no"}
            ])),
            ["Invalid navigation: links must be an array"]
        );
    }

    #[test]
    fn an_unregistered_type_passes_as_the_fallback_link() {
        assert!(errors_for(&json!([{"type": "wat", "data": {}}])).is_empty());
    }
}
