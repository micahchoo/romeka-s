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

use omeka_media::validation::{FileRefusal, FileValidation};

use super::plan::{ImportPlanV2, MatchKey, StarterId, Term};

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
    /// The fields of each starter a `newTypes` entry names, so a record of a
    /// type the run will create from a starter is checked like one of an
    /// existing template.
    pub starters: BTreeMap<StarterId, Vec<Field>>,
    /// The install's file-validation settings; a file they refuse flags
    /// `not-media`.
    pub files: FileValidation,
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

/// Why the install's file-validation settings refuse a file: its media type,
/// else `.` and its extension; `None` when it passes. Upstream's
/// `Omeka\File\Validator` checks both lists, media type first.
#[must_use]
pub fn refusal(files: &FileValidation, name: &str, media_type: &str) -> Option<String> {
    let extension = extension(name);
    files
        .refusals(name, media_type, extension.as_deref())
        .into_iter()
        .next()
        .map(|refused| match refused {
            FileRefusal::MediaType { media_type, .. } => media_type,
            FileRefusal::Extension { extension, .. } => format!(".{extension}"),
        })
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
    fn a_file_is_refused_by_media_type_then_extension() {
        let files = FileValidation::Allow {
            media_types: vec!["image/jpeg".to_string()],
            extensions: vec!["jpg".to_string()],
        };
        assert_eq!(refusal(&files, "a.JPG", "image/jpeg"), None);
        assert_eq!(
            refusal(&files, "Thumbs.db", "application/octet-stream").as_deref(),
            Some("application/octet-stream")
        );
        assert_eq!(
            refusal(&files, "a.jpeg", "image/jpeg").as_deref(),
            Some(".jpeg")
        );
        assert_eq!(refusal(&files, "noext", "image/jpeg").as_deref(), Some("."));
        assert_eq!(refusal(&FileValidation::Disabled, "x", "y"), None);
    }
}
