//! What the import looks up in the database, gathered before the derive so
//! the derive stays pure: existing item sets by title, templates by id, field
//! labels by term, the items that already hold an identifier, and the open
//! undo window. [`load`] fills it from the
//! database for the preview route and the run job; tests write it by hand.
//!
//! The starter item types live here too, because the server is the one
//! validator: a new type's required fields decide which cells are empty.

use std::collections::BTreeMap;

use omeka_db::DbError;
use omeka_db::ids::{ImportId, PropertyId};
use omeka_db::repos::{
    ImportRepo, ImportResourceRepo, ItemSetFilter, ItemSetRepo, Paging, PropertyFilter,
    PropertyRepo, TemplateRepo, VocabularyFilter, VocabularyRepo,
};
use omeka_db::rows::ImportKind;
use serde::Serialize;
use sqlx::{Row, SqlitePool};

use super::plan::StarterId;

/// The template an item gets when its set names no type: upstream's
/// installer creates it (`Installation\Task\AddDefaultTemplateTask`).
pub const DEFAULT_TEMPLATE_LABEL: &str = "Base Resource";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Catalog {
    /// Existing item set ids by title.
    pub item_sets: BTreeMap<String, i64>,
    pub templates: BTreeMap<i64, Template>,
    /// The template of an item whose set has no `typeFor` entry, normally
    /// Base Resource. `None` gives such items no template.
    pub default_template: Option<i64>,
    /// The label of every property by term. A term not here is unknown, and
    /// a plan that writes one cannot run.
    pub labels: BTreeMap<String, String>,
    /// The item that holds each `dcterms:identifier` literal, trimmed. Over
    /// every item, because an identifier names one thing in the whole
    /// archive. A row whose identifier is here is never created again.
    pub existing: BTreeMap<String, i64>,
    pub open_undo: Option<OpenUndo>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Template {
    pub label: String,
    pub fields: Vec<TemplateField>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateField {
    pub term: String,
    pub required: bool,
}

/// The import whose undo window the next run closes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OpenUndo {
    pub id: i64,
    pub folder: String,
    pub items: u64,
}

/// Read the catalog from the database.
///
/// # Errors
/// A [`DbError`] when a read fails.
pub async fn load(pool: &SqlitePool) -> Result<Catalog, DbError> {
    load_with(pool, None).await
}

/// The catalog as import `import` sees it: the items it made itself do not
/// already exist. For a draft this is [`load`]; after a run it keeps the
/// rows the run created out of the left-out rows.
///
/// # Errors
/// A [`DbError`] when a read fails.
pub async fn load_for_import(pool: &SqlitePool, import: ImportId) -> Result<Catalog, DbError> {
    load_with(pool, Some(import)).await
}

async fn load_with(pool: &SqlitePool, own: Option<ImportId>) -> Result<Catalog, DbError> {
    let terms = property_terms(pool).await?;
    let mut item_sets = BTreeMap::new();
    for set in ItemSetRepo::new(pool)
        .list(&ItemSetFilter::default(), &Paging::default())
        .await?
    {
        if let Some(title) = set.resource.title.as_deref().map(str::trim) {
            item_sets
                .entry(title.to_string())
                .or_insert(set.id().0 as i64);
        }
    }
    let repo = TemplateRepo::new(pool);
    let mut templates = BTreeMap::new();
    let mut default_template = None;
    for row in repo.list().await? {
        let fields = repo
            .list_properties(row.id)
            .await?
            .into_iter()
            .filter_map(|field| {
                terms
                    .get(&field.property_id)
                    .map(|(term, _)| TemplateField {
                        term: term.clone(),
                        required: field.is_required,
                    })
            })
            .collect();
        let id = row.id.0 as i64;
        if default_template.is_none() && row.label == DEFAULT_TEMPLATE_LABEL {
            default_template = Some(id);
        }
        templates.insert(
            id,
            Template {
                label: row.label,
                fields,
            },
        );
    }
    let open_undo = match ImportRepo::new(pool).undoable().await? {
        Some(row) => Some(OpenUndo {
            id: row.id.0 as i64,
            items: ImportResourceRepo::new(pool)
                .list(row.id, Some(ImportKind::Item))
                .await?
                .len() as u64,
            folder: row.folder,
        }),
        None => None,
    };
    Ok(Catalog {
        item_sets,
        templates,
        default_template,
        labels: terms.into_values().collect(),
        existing: existing_identifiers(pool, own).await?,
        open_undo,
    })
}

/// Every item's identifier literals with an import's own items left out.
/// The lowest item id wins when two items share an identifier.
const EXISTING: &str = "SELECT v.value, v.resource_id FROM value v
     JOIN item i ON i.id = v.resource_id
     JOIN property p ON p.id = v.property_id
     JOIN vocabulary w ON w.id = p.vocabulary_id
     WHERE w.prefix = 'dcterms' AND p.local_name = 'identifier'
       AND v.type = 'literal' AND v.value IS NOT NULL
       AND v.resource_id NOT IN (SELECT target_id FROM romeka_import_resource
                                 WHERE import_id = ? AND kind = 'item')
     ORDER BY v.resource_id";

/// One query for the whole archive, not one per row.
async fn existing_identifiers(
    pool: &SqlitePool,
    own: Option<ImportId>,
) -> Result<BTreeMap<String, i64>, DbError> {
    let own = own.map_or(-1, |id| id.0 as i64);
    let mut existing = BTreeMap::new();
    for row in sqlx::query(EXISTING).bind(own).fetch_all(pool).await? {
        let value: String = row.try_get(0)?;
        let item: i64 = row.try_get(1)?;
        let value = value.trim();
        if !value.is_empty() {
            existing.entry(value.to_string()).or_insert(item);
        }
    }
    Ok(existing)
}

/// Every property's term and label, by property id.
async fn property_terms(
    pool: &SqlitePool,
) -> Result<BTreeMap<PropertyId, (String, String)>, DbError> {
    let prefixes: BTreeMap<_, _> = VocabularyRepo::new(pool)
        .list(&VocabularyFilter::default(), &Paging::default())
        .await?
        .into_iter()
        .map(|row| (row.id, row.prefix))
        .collect();
    Ok(PropertyRepo::new(pool)
        .list(&PropertyFilter::default(), &Paging::default())
        .await?
        .into_iter()
        .filter_map(|row| {
            let prefix = prefixes.get(&row.vocabulary_id)?;
            Some((row.id, (format!("{prefix}:{}", row.local_name), row.label)))
        })
        .collect())
}

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

/// The fields a new type made from `starter` gets.
#[must_use]
pub fn starter_fields(id: StarterId) -> Vec<TemplateField> {
    starter(id)
        .fields
        .iter()
        .map(|field| TemplateField {
            term: field.term.to_string(),
            required: field.required,
        })
        .collect()
}

/// The field a new type made from `starter` takes its items' titles from:
/// its first field, the required name. The run sets it as the template's
/// title property, as an admin choosing "Title" on the template form would.
#[must_use]
pub fn starter_title(id: StarterId) -> Option<String> {
    starter(id)
        .fields
        .first()
        .map(|field| field.term.to_string())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;

    #[test]
    fn every_starter_has_a_required_name_field() {
        for starter in &STARTERS {
            let fields = starter_fields(starter.id);
            assert!(fields.first().is_some_and(|f| f.required), "{starter:?}");
        }
        assert_eq!(starter_fields(StarterId::Person)[0].term, "foaf:name");
        assert_eq!(
            starter_title(StarterId::Person).as_deref(),
            Some("foaf:name")
        );
        assert_eq!(
            starter_title(StarterId::Letter).as_deref(),
            Some("dcterms:title")
        );
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
