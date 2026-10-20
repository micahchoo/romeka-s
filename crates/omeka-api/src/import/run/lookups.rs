//! What the run looks up before it writes, and how it turns the work's terms
//! into values: terms to properties, the `local` vocabulary, the templates
//! the items use, and the denormalised title.

use std::collections::BTreeMap;

use omeka_db::DbError;
use omeka_db::ids::{ClassId, PropertyId, TemplateId};
use omeka_db::repos::{ClassRepo, PropertyRepo, TemplateRepo, VocabularyRepo};
use omeka_db::rows::{NewVocabulary, ValueContent};
use sqlx::SqlitePool;

use super::{LOCAL_PREFIX, RunError};
use crate::import::work::{Literal, TemplateRef, Work};
use crate::jsonld::BaseUrl;

/// What an item takes from its template: the id, the resource class (as the
/// admin's item form does) and the title property.
#[derive(Debug, Clone, Copy)]
pub(super) struct TemplateUse {
    pub(super) id: TemplateId,
    pub(super) class: Option<ClassId>,
    pub(super) title: Option<PropertyId>,
}

pub(super) fn property(
    terms: &BTreeMap<String, PropertyId>,
    term: &str,
) -> Result<PropertyId, RunError> {
    terms
        .get(term)
        .copied()
        .ok_or_else(|| RunError::UnknownTerm(term.to_string()))
}

pub(super) fn literals(
    terms: &BTreeMap<String, PropertyId>,
    values: &[Literal],
) -> Result<Vec<ValueContent>, RunError> {
    values
        .iter()
        .map(|literal| {
            Ok(ValueContent::literal(
                property(terms, &literal.term)?,
                literal.value.clone(),
            ))
        })
        .collect()
}

/// Every term the work names that is not a new field, refusing an unknown one.
/// `dcterms:title` is always there: a new set's title is in it.
pub(super) async fn resolve_terms(
    pool: &SqlitePool,
    work: &Work,
) -> Result<BTreeMap<String, PropertyId>, RunError> {
    let mut terms = BTreeMap::new();
    let mut wanted = work.existing_terms();
    wanted.insert("dcterms:title");
    for term in wanted {
        let id = find_property(pool, term)
            .await?
            .ok_or_else(|| RunError::UnknownTerm(term.to_string()))?;
        terms.insert(term.to_string(), id);
    }
    Ok(terms)
}

/// The property `prefix:local_name` names, if any.
pub(in crate::import) async fn find_property(
    pool: &SqlitePool,
    term: &str,
) -> Result<Option<PropertyId>, DbError> {
    let Some((prefix, local_name)) = term.split_once(':') else {
        return Ok(None);
    };
    let Some(vocabulary) = VocabularyRepo::new(pool).get_by_prefix(prefix).await? else {
        return Ok(None);
    };
    Ok(PropertyRepo::new(pool)
        .find(vocabulary.id, local_name)
        .await?
        .map(|row| row.id))
}

/// The resource class `prefix:local_name` names, if any.
pub(super) async fn find_class(pool: &SqlitePool, term: &str) -> Result<Option<ClassId>, DbError> {
    let Some((prefix, local_name)) = term.split_once(':') else {
        return Ok(None);
    };
    let Some(vocabulary) = VocabularyRepo::new(pool).get_by_prefix(prefix).await? else {
        return Ok(None);
    };
    Ok(ClassRepo::new(pool)
        .find(vocabulary.id, local_name)
        .await?
        .map(|row| row.id))
}

/// The `local` vocabulary, created with the admin's label and namespace when missing.
pub(super) async fn local_vocabulary(
    pool: &SqlitePool,
    base_url: &BaseUrl,
) -> Result<omeka_db::ids::VocabularyId, DbError> {
    let repo = VocabularyRepo::new(pool);
    if let Some(row) = repo.get_by_prefix(LOCAL_PREFIX).await? {
        return Ok(row.id);
    }
    let row = repo
        .create(&NewVocabulary {
            owner_id: None,
            namespace_uri: format!("{}/vocab/local/", base_url.as_str()),
            prefix: LOCAL_PREFIX.to_string(),
            label: "Local fields".to_string(),
            comment: Some("Fields this archive made for itself.".to_string()),
        })
        .await?;
    Ok(row.id)
}

/// Every existing template the items use.
pub(super) async fn existing_templates(
    pool: &SqlitePool,
    work: &Work,
) -> Result<BTreeMap<i64, TemplateUse>, DbError> {
    let mut templates = BTreeMap::new();
    for item in &work.items {
        if let TemplateRef::Existing(id) = item.template
            && !templates.contains_key(&id)
        {
            let row = TemplateRepo::new(pool).get(TemplateId(id as u64)).await?;
            templates.insert(
                id,
                TemplateUse {
                    id: TemplateId(id as u64),
                    class: row.as_ref().and_then(|row| row.resource_class_id),
                    title: row.and_then(|row| row.title_property_id),
                },
            );
        }
    }
    Ok(templates)
}

/// The denormalised title, as `write::values::title_for` (upstream's
/// `ResourceTitleHydrator`) sets it: the first non-empty value of the
/// template's title property, or of `dcterms:title` when the template names
/// none.
pub(super) fn title(
    template: Option<TemplateUse>,
    terms: &BTreeMap<String, PropertyId>,
    values: &[ValueContent],
) -> Option<String> {
    let property = template
        .and_then(|t| t.title)
        .or_else(|| terms.get("dcterms:title").copied())?;
    values
        .iter()
        .filter(|value| value.property_id == property)
        .find_map(|value| value.value.clone().filter(|text| !text.is_empty()))
}
