//! Tags and the step from tag to term: the census of a scope's spellings, merging near-duplicate
//! spellings, remembering spellings someone said are different, and promoting tags into a term
//! list ([`promote`]).
//!
//! A **tag** is a plain-text `dcterms:subject` value: type `literal`, text in `value`. A URI or
//! list-typed subject value is an expert heading on the same property and is never a tag, so no
//! operation here reads or writes it. A **scope** is the set of spellings one operation works
//! over: every tag, or the words stored with one term list's data type (`customvocab:<id>`) on
//! any property.
//!
//! Every write touches only values on resources the caller may update
//! (`AbstractEntityAdapter::authorize`), and answers how many resources it changed and how many it
//! left because the caller may not edit them. Each runs in one transaction
//! (`ValueRewriteRepo`), takes a `dry_run` that answers the same counts and writes nothing, and
//! converges: running it again changes nothing more.
//!
//! The routes are Romeka's own (`omeka-server/src/tags.rs`). Upstream has no tag concept and no
//! bulk rewrite; what is written is ordinary `value`, `custom_vocab` and template rows, which an
//! Omeka S install with the Custom Vocab module reads the same way. The spellings judged different
//! are one Romeka-only `setting` row, [`DISTINCT_SETTING`].

mod plan;
pub mod promote;

use std::collections::{BTreeMap, BTreeSet};

use omeka_core::term_list::{self, TermList, TermListKind, php_trim};
use omeka_db::ids::{CustomVocabId, MediaId, PropertyId, ResourceId, TemplateId};
use omeka_db::repos::{CustomVocabRepo, MediaRepo, SettingRepo, TemplateRepo, ValueRewriteRepo};
use omeka_db::rows::{CustomVocabRow, MergeWrite, TextValueRow, resource_type};
use serde::de::{self, Deserializer};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::errors::ApiError;
use crate::publish;
use crate::read::response::serialize;
use crate::read::{Api, ApiResponse, db_error, not_found, parse_id};
use crate::routes::Operation;
use crate::write::fulltext;

/// The property a tag is stored on.
pub const TAG_PROPERTY: &str = "dcterms:subject";
/// The data type of a tag.
pub const TAG_TYPE: &str = "literal";
/// The `setting` row holding the spellings judged different, per scope.
pub const DISTINCT_SETTING: &str = "romeka_distinct_values";

const LIST_CLASS: &str = "CustomVocab\\Entity\\CustomVocab";

/// The spellings one operation works over.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// Every plain-text `dcterms:subject` value.
    Tags,
    /// The text values stored with this list's data type, on any property.
    List(CustomVocabId),
}

impl Scope {
    fn of_type(self) -> String {
        match self {
            Self::Tags => TAG_TYPE.to_string(),
            Self::List(id) => term_list::data_type_name(id.0),
        }
    }

    /// The key the scope's "different" answers live under in [`DISTINCT_SETTING`].
    fn key(self) -> String {
        match self {
            Self::Tags => "tags".to_string(),
            Self::List(id) => term_list::data_type_name(id.0),
        }
    }
}

/// One spelling and how many resources carry it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Count {
    pub value: String,
    /// The resources the caller can see that carry the spelling.
    pub resources: usize,
    /// Of those, the ones the caller may edit.
    pub editable: usize,
}

/// A scope's spellings, most used first, and the groups of spellings judged different.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Census {
    pub values: Vec<Count>,
    pub distinct: Vec<Vec<String>>,
}

/// What a write did, or would do.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Rewrite {
    /// The resources whose values change.
    pub changed: usize,
    /// The resources carrying a spelling the write names that the caller may not edit; they keep
    /// their values.
    pub skipped: usize,
    /// On a published list, the merged spellings that stay in it deprecated, because a version
    /// carries them (Romeka's own member).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub deprecated: Vec<String>,
}

/// A value the caller may see, and whether they may edit its resource.
pub(crate) struct Seen<'r> {
    pub row: &'r TextValueRow,
    pub editable: bool,
}

/// The scope's values the caller can see. A resource is visible as a read shows it, and a private
/// value only to whoever sees the resource's private values.
pub(crate) fn seen<'r>(api: &Api<'_>, rows: &'r [TextValueRow]) -> Vec<Seen<'r>> {
    rows.iter()
        .filter(|row| {
            api.sees(row.resource_is_public, row.owner_id)
                && (row.is_public || api.sees(false, row.owner_id))
        })
        .map(|row| Seen {
            row,
            editable: may_edit(api, row),
        })
        .collect()
}

/// `Api\Manager` and `AbstractEntityAdapter::authorize($entity, 'update')` for the resource.
fn may_edit(api: &Api<'_>, row: &TextValueRow) -> bool {
    let Some(adapter) = adapter_of(&row.resource_type) else {
        return false;
    };
    api.allow_write(adapter, Operation::Update).is_ok()
        && api
            .authorize_resource(&row.resource_type, row.owner_id)
            .is_ok()
}

fn adapter_of(kind: &str) -> Option<&'static str> {
    match kind {
        resource_type::ITEM => Some("items"),
        resource_type::ITEM_SET => Some("item_sets"),
        resource_type::MEDIA => Some("media"),
        _ => None,
    }
}

/// The scope's stored rows.
pub(crate) async fn rows(api: &Api<'_>, scope: Scope) -> Result<Vec<TextValueRow>, ApiError> {
    let property = match scope {
        Scope::Tags => Some(tag_property(api).await?),
        Scope::List(_) => None,
    };
    ValueRewriteRepo::new(api.pool())
        .text_values(&scope.of_type(), property)
        .await
        .map_err(db_error)
}

/// The id of `dcterms:subject`. An install without it has no tags; it is a 404 for the property.
pub(crate) async fn tag_property(api: &Api<'_>) -> Result<PropertyId, ApiError> {
    api.fill_catalog().await?;
    api.catalog()
        .property_id(TAG_PROPERTY)
        .ok_or_else(|| not_found("Omeka\\Entity\\Property", TAG_PROPERTY))
}

/// `GET /api/tags` and `GET /api/custom_vocabs/{id}/values`.
pub async fn census(api: &Api<'_>, scope: Scope) -> Result<ApiResponse, ApiError> {
    if let Scope::List(id) = scope {
        list_row(api, id).await?;
    }
    let rows = rows(api, scope).await?;
    let mut counts: BTreeMap<&str, (BTreeSet<ResourceId>, BTreeSet<ResourceId>)> = BTreeMap::new();
    for seen in seen(api, &rows) {
        let entry = counts.entry(seen.row.text.as_str()).or_default();
        entry.0.insert(seen.row.resource_id);
        if seen.editable {
            entry.1.insert(seen.row.resource_id);
        }
    }
    let mut values: Vec<Count> = counts
        .into_iter()
        .map(|(value, (resources, editable))| Count {
            value: value.to_string(),
            resources: resources.len(),
            editable: editable.len(),
        })
        .collect();
    values.sort_by(|a, b| b.resources.cmp(&a.resources).then(a.value.cmp(&b.value)));
    let distinct = distinct_groups(api, scope).await?;
    Ok(ApiResponse::document(serialize(&Census {
        values,
        distinct,
    })?))
}

/// A spelling a body names: text that is not empty once trimmed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Spelling(pub String);

impl<'de> Deserialize<'de> for Spelling {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        if php_trim(&text).is_empty() {
            return Err(de::Error::invalid_value(
                de::Unexpected::Str(&text),
                &"a spelling with a letter in it",
            ));
        }
        Ok(Self(text))
    }
}

/// One or more spellings.
pub(crate) fn spellings<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<String>, D::Error> {
    let list = Vec::<Spelling>::deserialize(deserializer)?;
    if list.is_empty() {
        return Err(de::Error::invalid_length(0, &"at least one spelling"));
    }
    Ok(list.into_iter().map(|spelling| spelling.0).collect())
}

pub(crate) fn parse<T: for<'de> Deserialize<'de>>(bytes: &[u8]) -> Result<T, ApiError> {
    serde_json::from_slice(bytes).map_err(|error| ApiError::bad_request(error.to_string()))
}

/// The body of a merge: the spellings that go and the one they become.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MergeBody {
    #[serde(deserialize_with = "spellings")]
    from: Vec<String>,
    to: Spelling,
    #[serde(default)]
    dry_run: bool,
}

/// `POST /api/tags/merge` and `POST /api/custom_vocabs/{id}/merge`: every value spelled one of
/// `from` becomes `to`. On a list of words the list's own entries follow, so the list offers the
/// kept spelling and not the merged ones; that needs the right to edit the list. On a published
/// list a spelling a version carries stays, deprecated and replaced by `to`, so its URI keeps
/// resolving (`publish::guard::retire_merged`).
pub async fn merge(api: &Api<'_>, scope: Scope, bytes: &[u8]) -> Result<ApiResponse, ApiError> {
    let list = match scope {
        Scope::List(id) => {
            let row = list_row(api, id).await?;
            authorize_list(api, &row, Operation::Update)?;
            Some(row)
        }
        Scope::Tags => None,
    };
    let body: MergeBody = parse(bytes)?;
    let to = php_trim(&body.to.0).to_string();
    let rows = rows(api, scope).await?;
    let seen = seen(api, &rows);
    let named = |seen: &&Seen<'_>| body.from.contains(&seen.row.text) && seen.row.text != to;
    let skipped: BTreeSet<ResourceId> = seen
        .iter()
        .filter(named)
        .filter(|seen| !seen.editable)
        .map(|seen| seen.row.resource_id)
        .collect();
    let editable: Vec<&TextValueRow> = seen
        .iter()
        .filter(|seen| seen.editable)
        .map(|seen| seen.row)
        .collect();
    let plan = plan::plan_merge(&editable, &body.from, &to);
    let retired = match &list {
        Some(row) => publish::guard::retire_merged(api, row.id, &body.from, &to).await?,
        None => publish::guard::Retired::default(),
    };
    let list_terms = match &list {
        Some(row) => list_words(api, row, &body.from, &retired.keys, &to).await?,
        None => None,
    };
    let list_states = list
        .as_ref()
        .zip(retired.states)
        .map(|(row, states)| (row.id, states));
    let answer = Rewrite {
        changed: plan.changed.len(),
        skipped: skipped.len(),
        deprecated: retired.keys,
    };
    if body.dry_run || (plan.changed.is_empty() && list_terms.is_none() && list_states.is_none()) {
        return Ok(ApiResponse::document(serialize(&answer)?));
    }
    let resources: Vec<ResourceId> = plan.changed.iter().copied().collect();
    ValueRewriteRepo::new(api.pool())
        .merge(&MergeWrite {
            rewrite: plan.rewrite,
            to,
            of_type: scope.of_type(),
            delete: plan.delete,
            list_terms,
            list_states,
            resources: resources.clone(),
            default_title_property: api.catalog().property_id("dcterms:title"),
        })
        .await
        .map_err(db_error)?;
    reindex(api, &rows, &resources).await?;
    Ok(ApiResponse::document(serialize(&answer)?))
}

/// The list's words after the merge when they change, refused like any other write that would
/// drop a published term (`publish::guard::term_list`).
async fn list_words(
    api: &Api<'_>,
    row: &CustomVocabRow,
    from: &[String],
    retained: &[String],
    to: &str,
) -> Result<Option<(CustomVocabId, Value)>, ApiError> {
    let Some(words) = words_of(row) else {
        return Ok(None);
    };
    let merged = plan::merged_terms(&words, from, retained, to);
    if merged == words {
        return Ok(None);
    }
    let terms = Value::from(merged);
    let proposed = CustomVocabRow {
        terms: Some(terms.clone()),
        ..row.clone()
    };
    publish::guard::term_list(api, row.id, Some(&proposed)).await?;
    Ok(Some((row.id, terms)))
}

/// The words of a list that holds words, in list order; `None` for a list of links or items.
fn words_of(row: &CustomVocabRow) -> Option<Vec<String>> {
    let list = TermList::from_columns(
        row.label.clone(),
        row.lang.clone(),
        row.item_set_id.map(|id| id.0),
        row.terms.as_ref(),
        row.uris.as_ref(),
    );
    match list.kind() {
        Some(TermListKind::Terms) => Some(list.terms),
        None => Some(Vec::new()),
        Some(TermListKind::Uris | TermListKind::Items) => None,
    }
}

/// Bring the search index up to date for each changed resource (`Module::saveFulltext`); a
/// media's text is part of its item's, so the item follows.
async fn reindex(
    api: &Api<'_>,
    rows: &[TextValueRow],
    changed: &[ResourceId],
) -> Result<(), ApiError> {
    let kinds: BTreeMap<ResourceId, &str> = rows
        .iter()
        .map(|row| (row.resource_id, row.resource_type.as_str()))
        .collect();
    for id in changed {
        let Some(kind) = kinds.get(id).copied().and_then(adapter_of) else {
            continue;
        };
        fulltext::reindex(api, kind, *id).await?;
        if kind == "media"
            && let Some(media) = MediaRepo::new(api.pool())
                .get(MediaId(id.0))
                .await
                .map_err(db_error)?
        {
            fulltext::reindex(api, "items", ResourceId(media.item_id.0)).await?;
        }
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DistinctBody {
    #[serde(deserialize_with = "spellings")]
    values: Vec<String>,
}

/// `POST /api/tags/distinct` and `POST /api/custom_vocabs/{id}/distinct`: remember that these
/// spellings name different things, so the admin stops offering to merge them. It needs the right
/// to merge in the scope. A group inside a remembered group adds nothing; a group that holds
/// remembered groups replaces them. The answer is the scope's census.
pub async fn distinct(api: &Api<'_>, scope: Scope, bytes: &[u8]) -> Result<ApiResponse, ApiError> {
    match scope {
        Scope::Tags => api.allow_write("items", Operation::Update)?,
        Scope::List(id) => authorize_list(api, &list_row(api, id).await?, Operation::Update)?,
    }
    let body: DistinctBody = parse(bytes)?;
    let group: BTreeSet<String> = body.values.into_iter().collect();
    let repo = SettingRepo::new(api.pool());
    let mut stored = match repo.get(DISTINCT_SETTING).await.map_err(db_error)? {
        Some(Value::Object(map)) => map,
        _ => serde_json::Map::new(),
    };
    let key = scope.key();
    let mut groups = groups_in(stored.get(&key));
    if group.len() > 1 && !groups.iter().any(|known| group.is_subset(known)) {
        groups.retain(|known| !known.is_subset(&group));
        groups.push(group);
        stored.insert(
            key,
            serde_json::to_value(&groups).map_err(ApiError::internal)?,
        );
        repo.set(DISTINCT_SETTING, &Value::Object(stored))
            .await
            .map_err(db_error)?;
    }
    census(api, scope).await
}

async fn distinct_groups(api: &Api<'_>, scope: Scope) -> Result<Vec<Vec<String>>, ApiError> {
    let stored = SettingRepo::new(api.pool())
        .get(DISTINCT_SETTING)
        .await
        .map_err(db_error)?;
    let groups = groups_in(stored.as_ref().and_then(|map| map.get(scope.key())));
    Ok(groups
        .into_iter()
        .map(|group| group.into_iter().collect())
        .collect())
}

/// The groups a stored scope entry holds; anything not a list of lists of text is none.
fn groups_in(stored: Option<&Value>) -> Vec<BTreeSet<String>> {
    let Some(Value::Array(groups)) = stored else {
        return Vec::new();
    };
    groups
        .iter()
        .filter_map(|group| serde_json::from_value::<BTreeSet<String>>(group.clone()).ok())
        .collect()
}

pub(crate) async fn list_row(api: &Api<'_>, id: CustomVocabId) -> Result<CustomVocabRow, ApiError> {
    CustomVocabRepo::new(api.pool())
        .get(id)
        .await
        .map_err(db_error)?
        .ok_or_else(|| not_found(LIST_CLASS, &id.0.to_string()))
}

/// Parse a list id from the path.
pub fn list_scope(raw_id: &str) -> Result<Scope, ApiError> {
    Ok(Scope::List(CustomVocabId(parse_id(LIST_CLASS, raw_id)?)))
}

/// The Custom Vocab module's grants: `editor` and the admin roles may write lists, and an editor
/// only a list they own (`Module::onBootstrap`).
pub(crate) fn authorize_list(
    api: &Api<'_>,
    row: &CustomVocabRow,
    operation: Operation,
) -> Result<(), ApiError> {
    api.allow_write("custom_vocabs", operation)?;
    crate::write::term_lists::owner_only(api, row, operation)
}

/// The template a changed resource uses, for the fields a promotion reports.
pub(crate) async fn template_label(
    api: &Api<'_>,
    id: TemplateId,
) -> Result<Option<(String, Option<omeka_db::ids::UserId>)>, ApiError> {
    Ok(TemplateRepo::new(api.pool())
        .get(id)
        .await
        .map_err(db_error)?
        .map(|row| (row.label, row.owner_id)))
}
