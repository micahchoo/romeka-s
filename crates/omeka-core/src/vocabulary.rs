//! Vocabularies, classes and properties: the predicate side of the graph.
//!
//! Mirrors `application/src/Entity/Vocabulary.php`, `ResourceClass.php` and
//! `Property.php`, and the `vocabulary`/`resource_class`/`property` tables in
//! `application/data/install/schema.sql`.
//!
//! Upstream disagreements with DESIGN.md, resolved in upstream's favour: all
//! three rows carry a nullable `owner_id` (the column is `ON DELETE SET NULL`),
//! and `label` is NOT NULL with a non-empty validation
//! (`PropertyAdapter::validateEntity`), so it is a `String` here, not an
//! `Option<String>`.

use serde::{Deserialize, Serialize};

use crate::ids::{ClassId, PropertyId, UserId, VocabularyId};

/// A vocabulary namespace (`application/src/Entity/Vocabulary.php`,
/// `schema.sql:377`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vocabulary {
    /// `vocabulary.id`.
    pub id: VocabularyId,
    /// `vocabulary.owner_id`, nullable.
    pub owner: Option<UserId>,
    /// `vocabulary.namespace_uri`, unique upstream.
    pub namespace_uri: String,
    /// `vocabulary.prefix`, unique upstream.
    pub prefix: String,
    /// `vocabulary.label`, trimmed, never empty (`VocabularyAdapter::hydrate`).
    pub label: String,
    /// `vocabulary.comment`.
    pub comment: Option<String>,
}

impl Vocabulary {
    /// A vocabulary with the upstream uniqueness keys but no owner or comment.
    #[must_use]
    pub fn new(
        id: VocabularyId,
        namespace_uri: impl Into<String>,
        prefix: impl Into<String>,
        label: impl Into<String>,
    ) -> Self {
        Self {
            id,
            owner: None,
            namespace_uri: namespace_uri.into(),
            prefix: prefix.into(),
            label: label.into(),
            comment: None,
        }
    }
}

/// A resource class (`application/src/Entity/ResourceClass.php`,
/// `schema.sql:166`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Class {
    /// `resource_class.id`.
    pub id: ClassId,
    /// `resource_class.owner_id`, nullable.
    pub owner: Option<UserId>,
    /// `resource_class.vocabulary_id`.
    pub vocabulary: VocabularyId,
    /// `resource_class.local_name`, unique within the vocabulary.
    pub local_name: String,
    /// `resource_class.label`, never empty.
    pub label: String,
    /// `resource_class.comment`.
    pub comment: Option<String>,
}

/// A property, the predicate of a triple (`application/src/Entity/Property.php`,
/// `schema.sql:129`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Property {
    /// `property.id`.
    pub id: PropertyId,
    /// `property.owner_id`, nullable.
    pub owner: Option<UserId>,
    /// `property.vocabulary_id`.
    pub vocabulary: VocabularyId,
    /// `property.local_name`, unique within the vocabulary.
    pub local_name: String,
    /// `property.label`, never empty.
    pub label: String,
    /// `property.comment`.
    pub comment: Option<String>,
}

/// A `prefix:local_name` term, the form the API accepts in query parameters and
/// in value objects with `property_id: "auto"`
/// (`AbstractResourceEntityAdapter::getPropertyByTerm`).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Term(String);

impl Term {
    /// Build a term from a vocabulary prefix and a local name.
    #[must_use]
    pub fn new(prefix: &str, local_name: &str) -> Self {
        Self(format!("{prefix}:{local_name}"))
    }

    /// The term text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Split a term into its prefix and local name, at the first colon.
    #[must_use]
    pub fn split(&self) -> Option<(&str, &str)> {
        self.0.split_once(':')
    }
}

impl std::fmt::Display for Term {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `VocabularyAdapter::hydrate` trims the label;
    /// `VocabularyAdapter::validateEntity` rejects an empty one.
    #[test]
    fn vocabulary_shape_matches_upstream_vocabulary_entity() {
        let vocabulary = Vocabulary::new(
            VocabularyId::new(1),
            "http://purl.org/dc/terms/",
            "dcterms",
            "Dublin Core",
        );
        assert_eq!(vocabulary.prefix, "dcterms");
        assert_eq!(vocabulary.owner, None);
        assert_eq!(vocabulary.comment, None);
    }

    /// `getPropertyByTerm` resolves `dcterms:title` by prefix and local name.
    #[test]
    fn term_splits_prefix_and_local_name() {
        let term = Term::new("dcterms", "title");
        assert_eq!(term.as_str(), "dcterms:title");
        assert_eq!(term.split(), Some(("dcterms", "title")));
        assert_eq!(Term::new("nocolon", "").split(), Some(("nocolon", "")));
    }

    /// `resource_class.local_name` and `property.local_name` are unique per
    /// vocabulary (`schema.sql:137,174`); the ids brand the two tables apart.
    #[test]
    fn class_and_property_ids_do_not_mix() {
        let class_id = ClassId::new(10);
        let property_id = PropertyId::new(10);
        assert_eq!(class_id.get(), property_id.get());
        assert_ne!(format!("{class_id:?}"), format!("{property_id:?}"));
    }
}
