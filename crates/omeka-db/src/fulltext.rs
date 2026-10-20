//! The text rules of the `fulltext_search` index, shared by the two writers of
//! its rows: `omeka-api`'s `write/fulltext.rs` after an api write, and the
//! import's `repos/import_index.rs` on the record's transaction.
//!
//! Each writer reads the rows its own way; these functions turn what it read
//! into the indexed text, so both writers index the same words.

use crate::ids::ResourceId;
use crate::rows::ValueRow;

/// `omeka_core::term_list::DATA_TYPE_PREFIX`; this crate does not depend on the core.
const CUSTOM_VOCAB: &str = "customvocab:";

/// One value's fulltext text, as the data type registry gives it.
///
/// `Literal::getFulltextText` is the stored value, `Uri::getFulltextText` is the URI and the label
/// space separated, and `AbstractResource::getFulltextText` is the referenced resource's title,
/// the title of [`linked_resource`]. `CustomVocab::getFulltextText` is the
/// stored value whatever the shape: a term, a URI's label, nothing for an item.
pub fn value_text(value: &ValueRow, linked_title: Option<String>) -> String {
    let custom = value
        .r#type
        .strip_prefix(CUSTOM_VOCAB)
        .is_some_and(|digits| !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()));
    if custom {
        return value.value.clone().unwrap_or_default();
    }
    if value.r#type == "uri" {
        return format!(
            "{} {}",
            value.uri.as_deref().unwrap_or_default(),
            value.value.as_deref().unwrap_or_default()
        )
        .trim()
        .to_string();
    }
    if value.r#type.starts_with("resource") {
        return linked_title.unwrap_or_default();
    }
    value.value.clone().unwrap_or_default()
}

/// The resource whose title is the value's text: the linked resource of a resource-typed value.
/// The caller reads its title and passes it to [`value_text`].
pub fn linked_resource(value: &ValueRow) -> Option<ResourceId> {
    value
        .value_resource_id
        .filter(|_| value.r#type.starts_with("resource"))
}

/// PHP `strip_tags` plus `html_entity_decode` for the entities a body carries. The full HTML5
/// named-entity table is not ported; the numeric forms and the five XML entities are.
pub fn strip_tags(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    decode_entities(&out)
}

fn decode_entities(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find('&') {
        out.push_str(&rest[..start]);
        let Some(end) = rest[start..].find(';').map(|end| start + end) else {
            out.push_str(&rest[start..]);
            return out;
        };
        match decode_entity(&rest[start + 1..end]) {
            Some(decoded) => out.push(decoded),
            None => out.push_str(&rest[start..=end]),
        }
        rest = &rest[end + 1..];
    }
    out.push_str(rest);
    out
}

fn decode_entity(entity: &str) -> Option<char> {
    match entity {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" | "#39" => Some('\''),
        "nbsp" => Some('\u{a0}'),
        _ => {
            let digits = entity.strip_prefix('#')?;
            let code = match digits.strip_prefix(['x', 'X']) {
                Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                None => digits.parse().ok()?,
            };
            char::from_u32(code)
        }
    }
}
