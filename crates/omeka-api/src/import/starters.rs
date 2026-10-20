//! The starter item types: the one table the run creates templates from and
//! the admin lists from `GET /api/romeka/starters`. The server is the one
//! validator, so a new type's required fields decide which cells are empty.

use serde::Serialize;

use super::plan::StarterId;

/// One field of a starter: the property, whether it is required, the label
/// the template shows in place of the property's own, and the data types it
/// takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StarterField {
    pub term: &'static str,
    pub required: bool,
    pub label: &'static str,
    pub data_types: &'static [&'static str],
}

/// An item type to start from. This table is the only copy: the run creates
/// templates from it and the admin lists the same rows from
/// `GET /api/romeka/starters`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Starter {
    pub id: StarterId,
    /// The key a UI translates the name by. `name` is its English text.
    pub name_key: &'static str,
    pub name: &'static str,
    /// What a person may type for this kind, in any shipped locale, lowercase
    /// and without accents.
    pub words: &'static [&'static str],
    pub class_term: &'static str,
    /// The first field is the required name, and the template's title property.
    pub fields: &'static [StarterField],
}

const TEXT: &[&str] = &["literal"];

const fn field(
    term: &'static str,
    label: &'static str,
    required: bool,
    data_types: &'static [&'static str],
) -> StarterField {
    StarterField {
        term,
        required,
        label,
        data_types,
    }
}

/// The union of the fields the server and the admin each listed before this
/// table existed, one date per kind, so a type holds what an archive keeps.
pub const STARTERS: [Starter; 6] = [
    Starter {
        id: StarterId::Letter,
        name_key: "starter.letter",
        name: "Letter",
        words: &["letter", "letters", "carta", "cartas"],
        class_term: "bibo:Letter",
        fields: &[
            field("dcterms:title", "Title", true, TEXT),
            field("dcterms:identifier", "Identifier", false, TEXT),
            field("dcterms:creator", "Sender", false, TEXT),
            field("bibo:recipient", "Recipient", false, TEXT),
            field("dcterms:created", "Date written", false, TEXT),
            field("dcterms:spatial", "Place", false, TEXT),
            field("dcterms:description", "Description", false, TEXT),
        ],
    },
    Starter {
        id: StarterId::Photograph,
        name_key: "starter.photograph",
        name: "Photograph",
        words: &[
            "photograph",
            "photographs",
            "photo",
            "photos",
            "picture",
            "pictures",
            "fotografia",
            "fotografias",
            "foto",
            "fotos",
        ],
        class_term: "dctype:StillImage",
        fields: &[
            field("dcterms:title", "Title", true, TEXT),
            field("dcterms:identifier", "Identifier", false, TEXT),
            field("dcterms:creator", "Photographer", false, TEXT),
            field("dcterms:created", "Date taken", false, TEXT),
            field("dcterms:spatial", "Place", false, TEXT),
            field(
                "dcterms:subject",
                "Who is in it",
                false,
                &["literal", "resource:item"],
            ),
            field("dcterms:rights", "Rights", false, TEXT),
        ],
    },
    Starter {
        id: StarterId::Postcard,
        name_key: "starter.postcard",
        name: "Postcard",
        words: &["postcard", "postcards", "postal", "postales"],
        class_term: "bibo:Letter",
        fields: &[
            field("dcterms:title", "Title", true, TEXT),
            field("dcterms:identifier", "Identifier", false, TEXT),
            field("dcterms:creator", "Sender", false, TEXT),
            field("bibo:recipient", "Recipient", false, TEXT),
            field("dcterms:date", "Date sent", false, TEXT),
            field("dcterms:spatial", "Postmark place", false, TEXT),
            field("dcterms:description", "Description", false, TEXT),
        ],
    },
    Starter {
        id: StarterId::Diary,
        name_key: "starter.diary",
        name: "Diary",
        words: &[
            "diary", "diaries", "journal", "journals", "diario", "diarios",
        ],
        class_term: "bibo:Document",
        fields: &[
            field("dcterms:title", "Title", true, TEXT),
            field("dcterms:identifier", "Identifier", false, TEXT),
            field("dcterms:creator", "Writer", false, TEXT),
            field("dcterms:date", "Dates covered", false, TEXT),
            field("bibo:numPages", "Number of pages", false, TEXT),
        ],
    },
    Starter {
        id: StarterId::Person,
        name_key: "starter.person",
        name: "Person",
        words: &["person", "people", "persons", "persona", "personas"],
        class_term: "foaf:Person",
        fields: &[
            field("foaf:name", "Name", true, TEXT),
            field("dcterms:identifier", "Identifier", false, TEXT),
            field("dcterms:description", "Relationship", false, TEXT),
            field("dcterms:date", "Born", false, TEXT),
        ],
    },
    Starter {
        id: StarterId::Other,
        name_key: "starter.other",
        name: "Physical object",
        words: &[],
        class_term: "dctype:PhysicalObject",
        fields: &[field("dcterms:title", "Title", true, TEXT)],
    },
];

/// The starter `id` names. The test below pins each position to its id.
#[must_use]
pub const fn starter(id: StarterId) -> &'static Starter {
    match id {
        StarterId::Letter => &STARTERS[0],
        StarterId::Photograph => &STARTERS[1],
        StarterId::Postcard => &STARTERS[2],
        StarterId::Diary => &STARTERS[3],
        StarterId::Person => &STARTERS[4],
        StarterId::Other => &STARTERS[5],
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;

    #[test]
    fn every_starter_has_a_required_name_field() {
        for starter in &STARTERS {
            assert!(
                starter.fields.first().is_some_and(|field| field.required),
                "{starter:?}"
            );
        }
        assert_eq!(starter(StarterId::Person).fields[0].term, "foaf:name");
        assert_eq!(starter(StarterId::Letter).fields[0].term, "dcterms:title");
    }

    #[test]
    fn every_starter_term_and_class_ships_with_the_default_vocabularies() {
        let mut properties = std::collections::BTreeSet::new();
        let mut classes = std::collections::BTreeSet::new();
        for vocabulary in &crate::vocab_import::defaults::DEFAULT_VOCABULARIES {
            let members = vocabulary.members().expect("bundled vocabulary parses");
            for term in members.properties {
                properties.insert(format!("{}:{}", vocabulary.prefix, term.local_name));
            }
            for term in members.classes {
                classes.insert(format!("{}:{}", vocabulary.prefix, term.local_name));
            }
        }
        for starter in &STARTERS {
            assert!(
                classes.contains(starter.class_term),
                "{}",
                starter.class_term
            );
            assert!(starter.words.iter().all(|w| *w == w.to_lowercase()));
            let mut seen = std::collections::BTreeSet::new();
            for field in starter.fields {
                assert!(properties.contains(field.term), "{}", field.term);
                assert!(seen.insert(field.term), "{} twice", field.term);
                assert!(!field.label.is_empty());
            }
            assert_eq!(super::starter(starter.id), starter);
        }
    }
}
