//! Term lists: the Custom Vocab module's `custom_vocab` entity and the
//! `customvocab:<id>` data type each list registers.
//!
//! Upstream source: `omeka-s-modules/CustomVocab` v2.1.0. The admin calls a
//! list a "term list"; the data model keeps the module's names so an Omeka S
//! install with the module reads the same rows and values.
//!
//! A list holds one of three things, and `CustomVocabRepresentation::type`
//! decides which by precedence: an item set (its items are the choices), a map
//! of URI to label, or a plain list of terms. The other two columns may still
//! hold data; the precedence ignores it.

use serde_json::Value;

/// The prefix of every term list data type name (`DataType\CustomVocab::getName`).
pub const DATA_TYPE_PREFIX: &str = "customvocab:";

/// The data type name a list registers: `customvocab:<id>`.
#[must_use]
pub fn data_type_name(id: u64) -> String {
    format!("{DATA_TYPE_PREFIX}{id}")
}

/// The list id a data type name names, when it matches
/// `CustomVocabFactory::canCreate`'s `^customvocab:(\d+)$`.
#[must_use]
pub fn list_id(data_type: &str) -> Option<u64> {
    let digits = data_type.strip_prefix(DATA_TYPE_PREFIX)?;
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}

/// What a list offers (`CustomVocabRepresentation::type`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TermListKind {
    /// `literal`: a list of words.
    Terms,
    /// `uri`: links with labels.
    Uris,
    /// `resource`: the items of one item set.
    Items,
}

/// One list, parsed from its row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TermList {
    pub label: String,
    pub lang: Option<String>,
    pub item_set: Option<u64>,
    pub terms: Vec<String>,
    /// URI and label pairs in stored order. An empty label means "show the URI".
    pub uris: Vec<(String, String)>,
}

impl TermList {
    /// Parse the stored columns. `terms` is a JSON list and `uris` a JSON map;
    /// any other shape an older module release left behind reads as empty,
    /// which is how the module's own representation treats it.
    #[must_use]
    pub fn from_columns(
        label: String,
        lang: Option<String>,
        item_set: Option<u64>,
        terms: Option<&Value>,
        uris: Option<&Value>,
    ) -> Self {
        let terms = match terms {
            Some(Value::Array(entries)) => entries.iter().filter_map(php_string).collect(),
            _ => Vec::new(),
        };
        let uris = match uris {
            Some(Value::Object(map)) => map
                .iter()
                .map(|(uri, label)| (uri.clone(), php_string(label).unwrap_or_default()))
                .collect(),
            _ => Vec::new(),
        };
        Self {
            label,
            lang,
            item_set,
            terms,
            uris,
        }
    }

    /// `CustomVocabRepresentation::type`; `None` when the list holds nothing.
    #[must_use]
    pub fn kind(&self) -> Option<TermListKind> {
        if self.item_set.is_some() {
            Some(TermListKind::Items)
        } else if !self.uris.is_empty() {
            Some(TermListKind::Uris)
        } else if !self.terms.is_empty() {
            Some(TermListKind::Terms)
        } else {
            None
        }
    }

    /// The label a URI value of this list stores (`listUriLabels()[$uri]`): the
    /// list's label for the URI, the URI itself when that label is empty, and
    /// `None` when the URI is not in the list.
    #[must_use]
    pub fn uri_label(&self, uri: &str) -> Option<String> {
        self.uris
            .iter()
            .find(|(listed, _)| listed == uri)
            .map(|(listed, label)| {
                if label.is_empty() {
                    listed.clone()
                } else {
                    label.clone()
                }
            })
    }
}

/// `CustomVocabAdapter::sanitizeTerms`: a string splits at line ends, every
/// entry is trimmed, empty entries go, and a repeat keeps its first place.
/// `None` when nothing is left (`$terms ? array_values($terms) : null`).
///
/// `array_filter` with no callback also drops the string `"0"`, so a list
/// cannot hold the term `0`. That is upstream behaviour and it is kept.
#[must_use]
pub fn sanitize_terms(raw: &Value) -> Option<Vec<String>> {
    let mut terms: Vec<String> = Vec::new();
    for entry in entries(raw) {
        let term = php_trim(&entry).to_string();
        if term.is_empty() || term == "0" || terms.contains(&term) {
            continue;
        }
        terms.push(term);
    }
    if terms.is_empty() { None } else { Some(terms) }
}

/// `CustomVocabAdapter::sanitizeUris` over the map the admin form sends: each
/// label is trimmed, and an empty map is `None`.
///
/// Upstream splits a string or a list into lines and stores the lines as a JSON
/// list, which its own representation then cannot read as URIs. The port reads
/// each line the way the module's form does (`ArrayTextarea` with a space
/// separator): the URI, a space, then the label.
#[must_use]
pub fn sanitize_uris(raw: &Value) -> Option<Vec<(String, String)>> {
    let mut uris: Vec<(String, String)> = Vec::new();
    let mut put = |uri: String, label: String| match uris.iter_mut().find(|(key, _)| *key == uri) {
        Some(entry) => entry.1 = label,
        None => uris.push((uri, label)),
    };
    match raw {
        Value::Null => return None,
        Value::Object(map) => {
            for (uri, label) in map {
                let label = php_string(label).unwrap_or_default();
                put(uri.clone(), php_trim(&label).to_string());
            }
        }
        _ => {
            for line in entries(raw) {
                let line = php_trim(&line);
                if line.is_empty() {
                    continue;
                }
                match line.split_once(' ') {
                    Some((uri, label)) => {
                        put(php_trim(uri).to_string(), php_trim(label).to_string())
                    }
                    None => put(line.to_string(), String::new()),
                }
            }
        }
    }
    if uris.is_empty() { None } else { Some(uris) }
}

/// The URI map as the column stores it: a JSON object in list order.
#[must_use]
pub fn uris_json(uris: &[(String, String)]) -> Value {
    Value::Object(
        uris.iter()
            .map(|(uri, label)| (uri.clone(), Value::String(label.clone())))
            .collect(),
    )
}

/// The entries of a terms or URIs member: the lines of a string, or the
/// elements of a list or map. A string is split after the module's
/// `str_replace(["\r\n", "\n\r", "\r"], "\n")`.
fn entries(raw: &Value) -> Vec<String> {
    match raw {
        Value::String(text) => text
            .replace("\r\n", "\n")
            .replace("\n\r", "\n")
            .replace('\r', "\n")
            .split('\n')
            .map(str::to_string)
            .collect(),
        Value::Array(items) => items.iter().filter_map(php_string).collect(),
        Value::Object(map) => map.values().filter_map(php_string).collect(),
        other => php_string(other).into_iter().collect(),
    }
}

/// A scalar as PHP's string cast writes it; `None` for a list or a map.
fn php_string(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(true) => Some("1".to_string()),
        Value::Bool(false) | Value::Null => Some(String::new()),
        Value::Array(_) | Value::Object(_) => None,
    }
}

/// PHP `trim` with its default character list: space, tab, line feed, carriage
/// return, NUL and vertical tab. Rust's `trim` also strips Unicode spaces,
/// which PHP keeps.
#[must_use]
pub fn php_trim(text: &str) -> &str {
    text.trim_matches([' ', '\t', '\n', '\r', '\0', '\x0B'])
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_data_type_name_names_a_list_only_with_digits() {
        assert_eq!(data_type_name(7), "customvocab:7");
        assert_eq!(list_id("customvocab:7"), Some(7));
        assert_eq!(list_id("customvocab:"), None);
        assert_eq!(list_id("customvocab:7a"), None);
        assert_eq!(list_id("customvocab:-1"), None);
        assert_eq!(list_id("literal"), None);
    }

    /// `CustomVocabRepresentation::type`: an item set wins over URIs, URIs
    /// over terms.
    #[test]
    fn the_kind_follows_the_module_precedence() {
        let list = |item_set, terms: Value, uris: Value| {
            TermList::from_columns(
                "Places".to_string(),
                None,
                item_set,
                Some(&terms),
                Some(&uris),
            )
            .kind()
        };
        let terms = json!(["Cork"]);
        let uris = json!({"http://example.org/cork": "Cork"});
        assert_eq!(
            list(Some(3), terms.clone(), uris.clone()),
            Some(TermListKind::Items)
        );
        assert_eq!(list(None, terms.clone(), uris), Some(TermListKind::Uris));
        assert_eq!(list(None, terms, Value::Null), Some(TermListKind::Terms));
        assert_eq!(list(None, Value::Null, json!(["http://legacy"])), None);
    }

    #[test]
    fn a_uri_label_falls_back_to_the_uri_and_is_none_outside_the_list() {
        let list = TermList::from_columns(
            "Subjects".to_string(),
            None,
            None,
            None,
            Some(&json!({"http://example.org/a": "Bogs", "http://example.org/b": ""})),
        );
        assert_eq!(
            list.uri_label("http://example.org/a").as_deref(),
            Some("Bogs")
        );
        assert_eq!(
            list.uri_label("http://example.org/b").as_deref(),
            Some("http://example.org/b")
        );
        assert_eq!(list.uri_label("http://example.org/c"), None);
    }

    /// `sanitizeTerms`: split, trim, drop empty and `"0"`, keep the first of a
    /// repeat.
    #[test]
    fn terms_are_split_trimmed_and_unique() {
        assert_eq!(
            sanitize_terms(&json!(" Cork\r\nDublin\n\n0\rCork\n\u{a0}Galway ")),
            Some(vec![
                "Cork".to_string(),
                "Dublin".to_string(),
                "\u{a0}Galway".to_string()
            ])
        );
        assert_eq!(
            sanitize_terms(&json!(["Cork", " Cork ", 12])),
            Some(vec!["Cork".to_string(), "12".to_string()])
        );
        assert_eq!(sanitize_terms(&json!("  \n ")), None);
        assert_eq!(sanitize_terms(&Value::Null), None);
    }

    #[test]
    fn uris_keep_order_and_read_lines_the_way_the_form_does() {
        assert_eq!(
            sanitize_uris(&json!(
                "http://example.org/a  Bogs \nhttp://example.org/b\n"
            )),
            Some(vec![
                ("http://example.org/a".to_string(), "Bogs".to_string()),
                ("http://example.org/b".to_string(), String::new()),
            ])
        );
        assert_eq!(
            sanitize_uris(&json!({"http://example.org/a": " Bogs "})),
            Some(vec![(
                "http://example.org/a".to_string(),
                "Bogs".to_string()
            )])
        );
        assert_eq!(sanitize_uris(&json!({})), None);
        assert_eq!(sanitize_uris(&Value::Null), None);
    }
}
