//! The install as the derive sees it: plain data the caller loads before a
//! derive (`docs/import-v2.md`, "The derive").
//!
//! The catalog holds maps, not a lookup the derive calls back into. Match
//! keys come from resolved slots, which is most of the derive, so a
//! two-phase API (derive, ask for keys, derive again) would double the cost
//! of every preview. Loaded maps keep the derive one pass and pure, keep the
//! digest a function of one fixed input, and put every comparison in Rust,
//! as the contract requires, so SQLite and MySQL answer the same.
//! [`Catalog::terms_needed`] names the terms whose values the caller loads.

use std::collections::{BTreeMap, BTreeSet};

use super::plan::{ImportPlanV2, MatchKey, Term};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Catalog {
    pub items: Vec<ExistingItem>,
    pub media: Vec<ExistingMedia>,
    pub item_sets: Vec<ExistingSet>,
    pub templates: Vec<Template>,
    /// The install's "Base Resource" template, found by the caller (its
    /// label may be renamed or translated).
    pub base_template: Option<i64>,
    /// Every property the install knows, term to label.
    pub properties: BTreeMap<Term, String>,
    /// Every resource class the install knows, term to label.
    pub classes: BTreeMap<Term, String>,
    pub users: Vec<User>,
    pub files: FileRules,
}

/// The values of the terms [`Catalog::terms_needed`] names, per resource.
pub type Keys = BTreeMap<Term, Vec<String>>;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExistingItem {
    pub id: i64,
    pub keys: Keys,
    /// The item sets the item is in.
    pub sets: Vec<i64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExistingMedia {
    pub id: i64,
    pub keys: Keys,
    pub item: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExistingSet {
    pub id: i64,
    pub title: String,
    pub keys: Keys,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Template {
    pub id: i64,
    pub label: String,
    pub fields: Vec<Field>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    pub term: Term,
    pub required: bool,
    /// The data types the field allows; empty allows any.
    pub data_types: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct User {
    pub id: i64,
    pub email: String,
}

/// The install's file-validation settings, with the meaning of upstream's
/// `Omeka\File\Validator`: a file passes when both lists hold its media type
/// and its extension.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum FileRules {
    /// `disable_file_validation` is on.
    #[default]
    Disabled,
    Allow {
        media_types: BTreeSet<String>,
        /// Lower case, no dot.
        extensions: BTreeSet<String>,
    },
}

impl FileRules {
    /// Why a file is refused, or `None` when it passes.
    #[must_use]
    pub fn refusal(&self, name: &str, media_type: &str) -> Option<String> {
        let Self::Allow {
            media_types,
            extensions,
        } = self
        else {
            return None;
        };
        if !media_types.contains(media_type) {
            return Some(media_type.to_string());
        }
        let extension = extension(name).unwrap_or_default();
        (!extensions.contains(&extension)).then(|| format!(".{extension}"))
    }
}

/// The lower-case extension of a file name, without its dot.
#[must_use]
pub fn extension(name: &str) -> Option<String> {
    let (stem, extension) = name.rsplit_once('.')?;
    (!stem.is_empty() && !extension.is_empty()).then(|| extension.to_lowercase())
}

impl Catalog {
    /// The terms whose values the caller loads into each resource's `keys`:
    /// every term the plan matches on, and `dcterms:identifier`, which
    /// resource links name.
    #[must_use]
    pub fn terms_needed(plan: &ImportPlanV2) -> BTreeSet<Term> {
        let rules = [
            &plan.action.match_by.item,
            &plan.action.match_by.media,
            &plan.action.match_by.item_set,
        ];
        let mut terms: BTreeSet<Term> = rules
            .into_iter()
            .filter_map(|rule| match &rule.key {
                MatchKey::Term(term) => Some(term.clone()),
                MatchKey::InternalId => None,
            })
            .collect();
        terms.insert(Term::dcterms_identifier());
        terms
    }

    #[must_use]
    pub fn template(&self, id: i64) -> Option<&Template> {
        self.templates.iter().find(|template| template.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_rules_refuse_by_media_type_then_extension() {
        let rules = FileRules::Allow {
            media_types: ["image/jpeg".to_string()].into(),
            extensions: ["jpg".to_string()].into(),
        };
        assert_eq!(rules.refusal("a.JPG", "image/jpeg"), None);
        assert_eq!(
            rules
                .refusal("Thumbs.db", "application/octet-stream")
                .as_deref(),
            Some("application/octet-stream")
        );
        assert_eq!(
            rules.refusal("a.jpeg", "image/jpeg").as_deref(),
            Some(".jpeg")
        );
        assert_eq!(rules.refusal("noext", "image/jpeg").as_deref(), Some("."));
        assert_eq!(FileRules::Disabled.refusal("x", "y"), None);
    }
}
