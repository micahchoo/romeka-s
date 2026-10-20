//! `POST /api/tags/promote`: tags become the words of a term list.
//!
//! The list with the body's label takes the tags as words; it is created when no list has that
//! label. Each tag value on a resource the caller may edit keeps its text and its place on
//! `dcterms:subject`, and its type becomes `customvocab:<id>`. The item type (resource template)
//! of each changed resource then offers the list on its `dcterms:subject` field, adding the field
//! when the type has none, so no promoted list is left without a field that uses it. A type the
//! caller may not edit is left as it is and named in the answer.
//!
//! Running the same promotion again finds no tag left to retype, the words already in the list
//! and the fields already offering it, so it changes nothing.

use std::collections::BTreeSet;

use omeka_core::term_list::{self, php_trim};
use omeka_db::ids::{ResourceId, TemplateId, ValueId};
use omeka_db::repos::{CustomVocabRepo, ValueRewriteRepo};
use omeka_db::rows::{CustomVocabRow, NewCustomVocab, PromoteList, PromoteWrite};
use omeka_db::with_db;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{
    Scope, Spelling, authorize_list, parse, rows, seen, spellings, tag_property, words_of,
};
use crate::errors::{ApiError, ValidationErrors};
use crate::messages;
use crate::read::response::serialize;
use crate::read::{Api, ApiResponse, db_error};
use crate::routes::Operation;
use crate::write::access;

/// What a field with no data types offers in the admin (`makeNewField`'s default): such a field
/// keeps offering these beside the list.
const UNRESTRICTED: [&str; 3] = ["literal", "uri", "resource"];
const TEMPLATE_CLASS: &str = "Omeka\\Entity\\ResourceTemplate";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Body {
    #[serde(deserialize_with = "spellings")]
    tags: Vec<String>,
    /// The label of the list the tags join.
    list: Spelling,
    #[serde(default)]
    dry_run: bool,
}

/// A list or an item type, as the answer names it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Named {
    #[serde(rename = "o:id")]
    pub id: u64,
    #[serde(rename = "o:label")]
    pub label: String,
}

/// The answer of a promotion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Promotion {
    /// The list, or `null` on a dry run that would create it.
    pub list: Option<Named>,
    /// Whether the promotion creates the list.
    pub created: bool,
    pub changed: usize,
    pub skipped: usize,
    /// The item types whose subject field offers the list afterwards.
    pub templates: Vec<Named>,
    /// The item types of changed resources that the caller may not edit.
    pub templates_skipped: Vec<Named>,
}

pub async fn promote(api: &Api<'_>, bytes: &[u8]) -> Result<ApiResponse, ApiError> {
    let body: Body = parse(bytes)?;
    let label = php_trim(&body.list.0).to_string();
    let target = target(api, &label, &body.tags).await?;
    let property = tag_property(api).await?;
    let rows = rows(api, Scope::Tags).await?;
    let mut values: Vec<ValueId> = Vec::new();
    let mut changed: BTreeSet<ResourceId> = BTreeSet::new();
    let mut skipped: BTreeSet<ResourceId> = BTreeSet::new();
    let mut used: BTreeSet<TemplateId> = BTreeSet::new();
    for seen in seen(api, &rows) {
        if !body.tags.contains(&seen.row.text) {
            continue;
        }
        if !seen.editable {
            skipped.insert(seen.row.resource_id);
            continue;
        }
        values.push(seen.row.id);
        changed.insert(seen.row.resource_id);
        used.extend(seen.row.template_id);
    }
    let (templates, templates_skipped) = templates(api, &used).await?;
    let (list, created) = match &target {
        PromoteList::Create(_) => (None, true),
        PromoteList::Terms(id, _) => (Some(*id), false),
    };
    let mut answer = Promotion {
        list: list.map(|id| Named {
            id: id.0,
            label: label.clone(),
        }),
        created,
        changed: changed.len(),
        skipped: skipped.len(),
        templates: templates.iter().map(|(_, named)| named.clone()).collect(),
        templates_skipped,
    };
    if body.dry_run {
        return Ok(ApiResponse::document(serialize(&answer)?));
    }
    let id = with_db!(api.pool(), |db| ValueRewriteRepo::new(db)
        .promote(&PromoteWrite {
            list: target,
            values,
            from_type: super::TAG_TYPE.to_string(),
            property,
            templates: templates.iter().map(|(id, _)| *id).collect(),
            unrestricted: UNRESTRICTED
                .iter()
                .map(|name| (*name).to_string())
                .collect(),
            resources: changed.into_iter().collect(),
        })
        .await)
    .map_err(db_error)?;
    answer.list = Some(Named { id: id.0, label });
    Ok(ApiResponse::document(serialize(&answer)?))
}

/// The list the tags join: the stored list with this label, with the tags added to its words, or
/// a new list of the tags. A stored list of links or items cannot take words, so its label counts
/// as taken.
async fn target(api: &Api<'_>, label: &str, tags: &[String]) -> Result<PromoteList, ApiError> {
    let stored = with_db!(api.pool(), |db| CustomVocabRepo::new(db)
        .find_by_label(label)
        .await)
    .map_err(db_error)?;
    let Some(row) = stored else {
        api.allow_write("custom_vocabs", Operation::Create)?;
        let terms = term_list::sanitize_terms(&Value::from(tags.to_vec())).map(Value::from);
        return Ok(PromoteList::Create(NewCustomVocab {
            item_set_id: None,
            owner_id: api.actor().user(),
            label: label.to_string(),
            lang: None,
            terms,
            uris: None,
        }));
    };
    authorize_list(api, &row, Operation::Update)?;
    let Some(mut words) = words_of(&row) else {
        let mut errors = ValidationErrors::new();
        errors
            .entry("o:label".to_string())
            .or_default()
            .push(messages::LABEL_TAKEN.to_string());
        return Err(ApiError::Validation(errors));
    };
    words.extend(tags.iter().cloned());
    let terms = term_list::sanitize_terms(&Value::from(words)).map(Value::from);
    let proposed = CustomVocabRow {
        terms: terms.clone(),
        ..row.clone()
    };
    crate::publish::guard::term_list(api, row.id, Some(&proposed)).await?;
    Ok(PromoteList::Terms(row.id, terms.unwrap_or(Value::Null)))
}

/// The item types the list joins, and those the caller may not edit
/// (`ResourceTemplateAdapter`'s grant, then `authorize($entity, 'update')`).
async fn templates(
    api: &Api<'_>,
    used: &BTreeSet<TemplateId>,
) -> Result<(Vec<(TemplateId, Named)>, Vec<Named>), ApiError> {
    let mut offered = Vec::new();
    let mut skipped = Vec::new();
    for id in used {
        let Some((label, owner)) = super::template_label(api, *id).await? else {
            continue;
        };
        let named = Named { id: id.0, label };
        let may = api
            .allow_write("resource_templates", Operation::Update)
            .is_ok()
            && access::resource_entity_update(api.actor(), TEMPLATE_CLASS, owner).is_ok();
        if may {
            offered.push((*id, named));
        } else {
            skipped.push(named);
        }
    }
    Ok((offered, skipped))
}
