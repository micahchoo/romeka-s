//! Term and label lookups, loaded once per request.
//!
//! A value object carries a property id and the wire document wants the
//! property's `prefix:localName` term and its label
//! (`ValueRepresentation::jsonSerialize`, `AbstractVocabularyMemberRepresentation`).
//! The resource class term in `@type` needs the same vocabulary prefix.
//!
//! Upstream resolves each term through the entity manager's identity map, so a
//! request touches the rows it prints and no others. The port loaded the whole
//! `property` and `resource_class` table on every request, which the perf
//! harness measured as 0.55 ms of the 0.99 ms `api_context` floor: 183 property
//! and 105 class rows fetched to print five terms on the list page.
//!
//! [`Catalog::load_for_read`] loads the rows a document can print instead: the
//! properties a `value` or `resource_template_property` row names, and the
//! resource classes a `resource` row names. Both sets are closed under what the
//! document builders ask for, because every id they pass to [`Catalog::property`]
//! or [`Catalog::class_term`] comes from one of those rows.
//!
//! Two callers need more than that. A write names properties by term before any
//! value exists, and a list request with `property[i][property]` resolves a term
//! to an id before the page is read. Both call [`Catalog::fill_all`], which
//! loads the whole table on demand. [`Catalog::load`] is the eager full load,
//! for a caller that resolves terms itself (the site loader).
//!
//! The reads are `omeka_db::repos::TermRepo`'s, whose projections are narrower
//! than the row structs.

use std::collections::HashMap;
use std::sync::OnceLock;

use omeka_db::DbPool;
use omeka_db::ids::{ClassId, PropertyId, VocabularyId};
use omeka_db::repos::{TermRepo, TermScope};
use omeka_db::with_db;

use super::db_error;
use crate::errors::ApiError;

/// One property's JSON-LD term and label.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PropertyInfo {
    pub term: String,
    pub label: String,
    /// The label of the vocabulary the property belongs to
    /// (`$property->vocabulary()->label()`).
    pub vocabulary_label: String,
}

/// Vocabulary prefixes plus the property and resource class terms a request can
/// print, and the whole table when a caller asked for it.
#[derive(Debug, Clone, Default)]
pub struct Catalog {
    prefixes: HashMap<u64, String>,
    properties: HashMap<u64, PropertyInfo>,
    classes: HashMap<u64, String>,
    terms: HashMap<String, PropertyId>,
    /// The full `property` and `resource_class` tables, loaded on demand by
    /// [`Catalog::fill_all`]. `OnceLock` keeps the accessors borrowing `&self`.
    all: OnceLock<FullCatalog>,
}

/// The full term table, for a caller that needs a term it did not print.
#[derive(Debug, Clone, Default)]
struct FullCatalog {
    properties: HashMap<u64, PropertyInfo>,
    classes: HashMap<u64, String>,
    terms: HashMap<String, PropertyId>,
}

impl Catalog {
    /// The whole term table, for a caller that resolves terms itself.
    pub async fn load(pool: &DbPool) -> Result<Self, ApiError> {
        let prefixes = prefixes(pool).await?;
        let properties = load_properties(pool, TermScope::All, &prefixes).await?;
        let classes = load_classes(pool, TermScope::All, &prefixes).await?;
        let terms = terms_of(&properties);
        Ok(Self {
            prefixes,
            properties,
            classes,
            terms,
            all: OnceLock::new(),
        })
    }

    /// The rows a request's documents can print, which is all the API read
    /// surface needs before it builds a page.
    pub async fn load_for_read(pool: &DbPool) -> Result<Self, ApiError> {
        let prefixes = prefixes(pool).await?;
        let properties = load_properties(pool, TermScope::Referenced, &prefixes).await?;
        let classes = load_classes(pool, TermScope::Referenced, &prefixes).await?;
        Ok(Self {
            prefixes,
            properties,
            classes,
            terms: HashMap::new(),
            all: OnceLock::new(),
        })
    }

    /// Load the whole `property` and `resource_class` table, once. A write and
    /// a request that parses `property[i][property]` need it; a plain read does
    /// not, and that is the point of this type.
    pub async fn fill_all(&self, pool: &DbPool) -> Result<(), ApiError> {
        if self.all.get().is_some() {
            return Ok(());
        }
        let properties = load_properties(pool, TermScope::All, &self.prefixes).await?;
        let classes = load_classes(pool, TermScope::All, &self.prefixes).await?;
        let terms = terms_of(&properties);
        // A second call races only if two threads share one `Api`, which they do
        // not; a lost set would still leave a complete catalog in place.
        let _ = self.all.set(FullCatalog {
            properties,
            classes,
            terms,
        });
        Ok(())
    }

    /// The vocabulary prefix, or an empty string when the row is missing.
    pub fn prefix(&self, vocabulary: VocabularyId) -> String {
        self.prefixes
            .get(&vocabulary.0)
            .cloned()
            .unwrap_or_default()
    }

    /// The term and label of a property the request's rows name.
    pub fn property(&self, property: PropertyId) -> Option<&PropertyInfo> {
        self.properties.get(&property.0).or_else(|| {
            self.all
                .get()
                .and_then(|all| all.properties.get(&property.0))
        })
    }

    /// The property a `prefix:localName` term names, for a write body key or a
    /// `property[i][property]` row.
    pub fn property_id(&self, term: &str) -> Option<PropertyId> {
        self.terms
            .get(term)
            .copied()
            .or_else(|| self.all.get().and_then(|all| all.terms.get(term).copied()))
    }

    /// Every known property term, for the unknown-property check.
    pub fn knows_term(&self, term: &str) -> bool {
        self.property_id(term).is_some()
    }

    /// The `prefix:localName` term of a resource class a resource names.
    pub fn class_term(&self, class: ClassId) -> Option<&str> {
        self.classes.get(&class.0).map(String::as_str).or_else(|| {
            self.all
                .get()
                .and_then(|all| all.classes.get(&class.0).map(String::as_str))
        })
    }
}

/// The vocabulary prefixes by id.
async fn prefixes(pool: &DbPool) -> Result<HashMap<u64, String>, ApiError> {
    let rows = with_db!(pool, |db| TermRepo::new(db).prefixes().await).map_err(db_error)?;
    Ok(rows
        .into_iter()
        .map(|(id, prefix)| (id.0, prefix))
        .collect())
}

/// The properties in `scope`, as `id -> term and label`. The referenced scope
/// holds every property a document can print, so an id outside it is never
/// passed to [`Catalog::property`].
async fn load_properties(
    pool: &DbPool,
    scope: TermScope,
    prefixes: &HashMap<u64, String>,
) -> Result<HashMap<u64, PropertyInfo>, ApiError> {
    let rows = with_db!(pool, |db| TermRepo::new(db).properties(scope).await).map_err(db_error)?;
    Ok(rows
        .into_iter()
        .map(|row| {
            let info = PropertyInfo {
                term: term_of(prefixes, row.vocabulary_id.0, &row.local_name),
                label: row.label,
                vocabulary_label: row.vocabulary_label,
            };
            (row.id.0, info)
        })
        .collect())
}

/// The resource classes in `scope`, as `id -> term`.
async fn load_classes(
    pool: &DbPool,
    scope: TermScope,
    prefixes: &HashMap<u64, String>,
) -> Result<HashMap<u64, String>, ApiError> {
    let rows = with_db!(pool, |db| TermRepo::new(db).classes(scope).await).map_err(db_error)?;
    Ok(rows
        .into_iter()
        .map(|row| {
            (
                row.id.0,
                term_of(prefixes, row.vocabulary_id.0, &row.local_name),
            )
        })
        .collect())
}

fn term_of(prefixes: &HashMap<u64, String>, vocabulary: u64, local_name: &str) -> String {
    let prefix = prefixes.get(&vocabulary).map(String::as_str).unwrap_or("");
    format!("{prefix}:{local_name}")
}

/// The term lookup a write body key resolves against.
fn terms_of(properties: &HashMap<u64, PropertyInfo>) -> HashMap<String, PropertyId> {
    properties
        .iter()
        .map(|(id, info)| (info.term.clone(), PropertyId(*id)))
        .collect()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn an_empty_catalog_knows_no_term() {
        let catalog = Catalog::default();
        assert_eq!(catalog.property(PropertyId(1)), None);
        assert_eq!(catalog.property_id("dcterms:title"), None);
        assert!(!catalog.knows_term("dcterms:title"));
        assert_eq!(catalog.class_term(ClassId(1)), None);
        assert_eq!(catalog.prefix(VocabularyId(1)), "");
    }

    #[test]
    fn a_term_joins_the_prefix_and_the_local_name() {
        let mut prefixes = HashMap::new();
        prefixes.insert(1, "dcterms".to_string());
        assert_eq!(term_of(&prefixes, 1, "title"), "dcterms:title");
        assert_eq!(term_of(&prefixes, 9, "title"), ":title");
    }
}
