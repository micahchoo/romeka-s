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

/// The fields of a starter type: term and whether it is required.
const STARTERS: [(StarterId, &[(&str, bool)]); 6] = [
    (
        StarterId::Letter,
        &[
            ("dcterms:title", true),
            ("dcterms:identifier", false),
            ("dcterms:date", false),
            ("dcterms:creator", false),
            ("bibo:recipient", false),
        ],
    ),
    (
        StarterId::Photograph,
        &[
            ("dcterms:title", true),
            ("dcterms:identifier", false),
            ("dcterms:created", false),
            ("dcterms:creator", false),
            ("dcterms:rights", false),
            ("dcterms:spatial", false),
        ],
    ),
    (
        StarterId::Postcard,
        &[
            ("dcterms:title", true),
            ("dcterms:identifier", false),
            ("dcterms:date", false),
            ("dcterms:spatial", false),
            ("dcterms:description", false),
        ],
    ),
    (
        StarterId::Diary,
        &[
            ("dcterms:title", true),
            ("dcterms:identifier", false),
            ("dcterms:date", false),
            ("dcterms:creator", false),
        ],
    ),
    (
        StarterId::Person,
        &[("foaf:name", true), ("dcterms:identifier", false)],
    ),
    (StarterId::Other, &[("dcterms:title", true)]),
];

/// The fields a new type made from `starter` gets.
#[must_use]
pub fn starter_fields(starter: StarterId) -> Vec<TemplateField> {
    STARTERS
        .iter()
        .find(|(id, _)| *id == starter)
        .map(|(_, fields)| {
            fields
                .iter()
                .map(|(term, required)| TemplateField {
                    term: (*term).to_string(),
                    required: *required,
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The field a new type made from `starter` takes its items' titles from:
/// its first field, the required name. The run sets it as the template's
/// title property, as an admin choosing "Title" on the template form would.
#[must_use]
pub fn starter_title(starter: StarterId) -> Option<String> {
    starter_fields(starter)
        .into_iter()
        .next()
        .map(|field| field.term)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;

    #[test]
    fn every_starter_has_a_required_name_field() {
        for (starter, _) in STARTERS {
            let fields = starter_fields(starter);
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
}
