//! The built-in vocabularies' labels in the site's locale.
//!
//! Upstream prints a property label through the translator,
//! `$translate($property->label())` (`view/common/resource-values.phtml:28`),
//! and `application/language/template.pot` carries the label of every class
//! and property of the four built-in vocabularies. gettext looks the English
//! text up, so a label a user changed, or one of a vocabulary the user
//! imported, is no msgid and prints as stored.
//!
//! The port keys the translation by term instead (`dcterms:title`,
//! `bibo:Document`, or a vocabulary's prefix), and keeps gettext's effect with
//! one guard: a stored label is translated only while it is still the built-in
//! English for its term. Any other label passes through unchanged.
//!
//! The tables are the admin's files, `web/admin/src/i18n/locales/vocabulary/
//! labels/<tag>.json`, which the admin and the site both read. The image
//! builds this crate without `web/admin`, so `data/vocabulary-labels/` holds a
//! copy, and a test fails when the copy is stale. Each table is keyed by the
//! locale's tag, so a locale the site adds finds its table by `Locale::tag`.

use std::collections::HashMap;
use std::sync::LazyLock;

use serde::Deserialize;

use super::Translator;

/// Every shipped table, by locale tag. `en` is the built-in English the guard
/// compares a stored label against.
const TABLES: &[(&str, &str)] = &[
    ("en", include_str!("../../data/vocabulary-labels/en.json")),
    ("es", include_str!("../../data/vocabulary-labels/es.json")),
    ("hi", include_str!("../../data/vocabulary-labels/hi.json")),
    ("mr", include_str!("../../data/vocabulary-labels/mr.json")),
    ("ml", include_str!("../../data/vocabulary-labels/ml.json")),
    ("ta", include_str!("../../data/vocabulary-labels/ta.json")),
    ("kn", include_str!("../../data/vocabulary-labels/kn.json")),
    ("te", include_str!("../../data/vocabulary-labels/te.json")),
    ("ne", include_str!("../../data/vocabulary-labels/ne.json")),
];

/// The shape of one table file. `note` says where the text came from.
#[derive(Deserialize)]
struct TableFile {
    terms: HashMap<String, String>,
}

/// The parsed tables, by tag. A file that fails to parse is left out; the
/// tests parse every file, so a shipped table always parses.
static PARSED: LazyLock<HashMap<&'static str, HashMap<String, String>>> = LazyLock::new(|| {
    TABLES
        .iter()
        .filter_map(|(tag, text)| {
            serde_json::from_str::<TableFile>(text)
                .ok()
                .map(|file| (*tag, file.terms))
        })
        .collect()
});

/// A built-in term's label in the locale `tag` names, or `stored` as it is.
///
/// `stored` is translated only when it is the built-in English for `term`, so
/// a user's own vocabulary, an edited label and a locale without a table all
/// print what is stored.
#[must_use]
pub fn term_label<'a>(tag: &str, term: &str, stored: &'a str) -> &'a str {
    let english = PARSED.get("en").and_then(|table| table.get(term));
    if english.map(String::as_str) != Some(stored) {
        return stored;
    }
    PARSED
        .get(tag)
        .and_then(|table| table.get(term))
        .map_or(stored, String::as_str)
}

impl Translator {
    /// `$translate($label)` for the label of a vocabulary, a class or a
    /// property: [`term_label`] in this translator's locale.
    #[must_use]
    pub fn term_label<'a>(self, term: &str, stored: &'a str) -> &'a str {
        term_label(self.locale.tag(), term, stored)
    }
}

#[cfg(test)]
#[path = "vocabulary_tests.rs"]
mod tests;
