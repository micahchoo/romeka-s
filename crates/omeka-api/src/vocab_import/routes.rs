//! The three Romeka routes behind the admin's vocabulary import and update.
//!
//! | Route | Body | Answer |
//! | --- | --- | --- |
//! | `POST /api/vocabularies/import` | the vocabulary's `o:` members and `source` | the created vocabulary |
//! | `POST /api/vocabularies/{id}/diff` | `source` | the [`Diff`] a file would apply |
//! | `POST /api/vocabularies/{id}/diff/apply` | a [`Diff`], as the first route answered it | the vocabulary |
//!
//! `source` is upstream's file and advanced fieldsets (`Form/VocabularyForm`):
//! `{import_type: "upload" | "url", file?, url?, format?, lang?, label_property?,
//! comment_property?}`. An upload carries the file's text in `file`.
//!
//! Upstream has no API for any of this: `Admin/VocabularyController` runs `RdfImporter` from
//! server-rendered forms and carries the reviewed diff in a hidden field. The import creates the
//! vocabulary through the `POST /api/vocabularies` write, members and all, as `RdfImporter::import`
//! calls `ApiManager::create`, so every vocabulary rule applies to it.

use omeka_db::ids::VocabularyId;
use omeka_db::repos::VocabularyRepo;
use omeka_db::rows::VocabularyRow;
use serde::Deserialize;
use serde_json::{Value, json};

use super::fetch::{self, FetchError, Reach};
use super::{Diff, ExtractError, Format, LabelPreference, MemberDiff, Members, Term};
use crate::errors::{ApiError, ValidationErrors};
use crate::messages;
use crate::read::response::serialize;
use crate::read::{Api, ApiResponse, db_error, not_found, parse_id};
use crate::routes::Operation;
use crate::write::access;
use crate::write::body::Payload;
use crate::write::vocab_member::{self, Member, MemberInput, MemberRow};

const VOCABULARY_CLASS: &str = "Omeka\\Entity\\Vocabulary";

/// The error key every source failure files under.
const SOURCE_KEY: &str = "source";

/// `VocabularyRepresentation::isPermanent`.
const PERMANENT_PREFIXES: [&str; 2] = ["dcterms", "dctype"];

/// Where the RDF comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Origin {
    /// The text of an uploaded file.
    Upload(String),
    Url(String),
}

/// A parsed `source` member.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Source {
    origin: Origin,
    format: Format,
    preference: LabelPreference,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ImportType {
    Upload,
    Url,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceBody {
    import_type: ImportType,
    file: Option<String>,
    url: Option<String>,
    format: Option<String>,
    lang: Option<String>,
    label_property: Option<String>,
    comment_property: Option<String>,
}

/// The `source` member, parsed at the boundary.
fn parse_source(value: Option<Value>) -> Result<Source, ApiError> {
    let value = value.ok_or_else(|| source_error(vec![messages::VOCABULARY_NO_FILE.into()]))?;
    let body: SourceBody =
        serde_json::from_value(value).map_err(|error| ApiError::bad_request(error.to_string()))?;
    let text = |field: Option<String>| field.filter(|text| !text.trim().is_empty());
    let origin = match body.import_type {
        ImportType::Upload => Origin::Upload(
            text(body.file)
                .ok_or_else(|| source_error(vec![messages::VOCABULARY_NO_FILE.into()]))?,
        ),
        ImportType::Url => Origin::Url(
            text(body.url)
                .map(|url| url.trim().to_string())
                .ok_or_else(|| source_error(vec![messages::VOCABULARY_NO_URL.into()]))?,
        ),
    };
    let format_text = body.format.unwrap_or_default();
    let format = Format::from_form(&format_text)
        .ok_or_else(|| source_error(vec![messages::vocabulary_format_invalid(&format_text)]))?;
    Ok(Source {
        origin,
        format,
        preference: LabelPreference::new(
            body.lang.as_deref(),
            body.label_property.as_deref(),
            body.comment_property.as_deref(),
        ),
    })
}

fn source_error(messages: Vec<String>) -> ApiError {
    ApiError::Validation(ValidationErrors::from([(SOURCE_KEY.to_string(), messages)]))
}

/// `RdfImporter::getMembers` for a parsed source: read the file or fetch the URL, then extract.
async fn read_members(source: &Source, namespace_uri: &str) -> Result<Members, ApiError> {
    let (bytes, content_type, base_iri, failure) = match &source.origin {
        Origin::Upload(text) => (
            text.clone().into_bytes(),
            None,
            namespace_uri.to_string(),
            messages::VOCABULARY_NOT_PARSED,
        ),
        Origin::Url(url) => {
            let target = url.clone();
            let format = source.format;
            let fetched =
                tokio::task::spawn_blocking(move || fetch::fetch(&target, format, Reach::Public))
                    .await
                    .map_err(ApiError::internal)?
                    .map_err(|error| {
                        source_error(vec![
                            messages::VOCABULARY_NOT_LOADED.into(),
                            fetch_reason(&error),
                        ])
                    })?;
            (
                fetched.bytes,
                fetched.content_type,
                url.clone(),
                messages::VOCABULARY_NOT_LOADED,
            )
        }
    };
    super::extract(
        &bytes,
        source.format,
        content_type.as_deref(),
        &base_iri,
        namespace_uri,
        &source.preference,
    )
    .map_err(|error| {
        let reason = match error {
            ExtractError::UnknownFormat => messages::VOCABULARY_FORMAT_UNKNOWN.to_string(),
            ExtractError::Syntax(detail) => detail,
        };
        source_error(vec![failure.into(), reason])
    })
}

fn fetch_reason(error: &FetchError) -> String {
    match error {
        FetchError::Scheme => messages::VOCABULARY_URL_SCHEME.into(),
        FetchError::PrivateAddress => messages::VOCABULARY_URL_PRIVATE.into(),
        FetchError::TooLarge => {
            messages::vocabulary_url_too_large(format!("{} MiB", fetch::MAX_BYTES >> 20))
        }
        FetchError::Timeout => messages::VOCABULARY_URL_TIMEOUT.into(),
        FetchError::Status(status) => messages::vocabulary_url_status(status),
        FetchError::Transport(detail) => detail.clone(),
    }
}

/// `POST /api/vocabularies/import`: `VocabularyController::importAction`.
///
/// # Errors
/// 403 for a caller who may not create vocabularies, 422 for a source that yields no members or a
/// vocabulary the create refuses, 400 for a malformed body.
pub async fn import(api: &Api<'_>, bytes: &[u8]) -> Result<ApiResponse, ApiError> {
    api.allow_write("vocabularies", Operation::Create)?;
    let mut body = Payload::parse(bytes)?.object().clone();
    let source = parse_source(body.remove(SOURCE_KEY))?;
    let namespace_uri = body
        .get("o:namespace_uri")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();
    let members = read_members(&source, &namespace_uri).await?;
    if members.is_empty() {
        return Err(source_error(vec![messages::VOCABULARY_NO_MEMBERS.into()]));
    }
    body.insert("o:class".into(), member_list(&members.classes));
    body.insert("o:property".into(), member_list(&members.properties));
    crate::write::vocab::create_vocabulary(api, &Payload::from_object(body)).await
}

/// The `o:class` / `o:property` list `RdfImporter::import` hands the create.
fn member_list(terms: &[Term]) -> Value {
    Value::Array(
        terms
            .iter()
            .map(|term| {
                json!({
                    "o:local_name": term.local_name,
                    "o:label": term.label,
                    "o:comment": term.comment,
                })
            })
            .collect(),
    )
}

/// `POST /api/vocabularies/{id}/diff`: `VocabularyController::editAction` with a file.
///
/// # Errors
/// 404 for an unknown vocabulary, 403 for a caller who may not change it or a permanent one, 422
/// for a source that yields no members.
pub async fn diff(api: &Api<'_>, raw_id: &str, bytes: &[u8]) -> Result<ApiResponse, ApiError> {
    let vocabulary = editable(api, raw_id).await?;
    let mut body = Payload::parse(bytes)?.object().clone();
    let source = parse_source(body.remove(SOURCE_KEY))?;
    let file = read_members(&source, &vocabulary.namespace_uri).await?;
    // Upstream compares an empty file too and shows every member unchanged; a file with no
    // member in the namespace is the wrong file or the wrong namespace, so it is refused as the
    // import refuses it.
    if file.is_empty() {
        return Err(source_error(vec![messages::VOCABULARY_NO_MEMBERS.into()]));
    }
    let stored = Members {
        classes: stored_terms(api, Member::Class, vocabulary.id).await?,
        properties: stored_terms(api, Member::Property, vocabulary.id).await?,
    };
    Ok(ApiResponse::document(serialize(&super::diff(
        &stored, &file,
    ))?))
}

/// `POST /api/vocabularies/{id}/diff/apply`: `VocabularyController::updateAction`, which runs
/// `RdfImporter::update`: insert the new members, rewrite the changed labels and comments, and
/// leave every other member and every value alone.
///
/// Upstream applies whatever diff the form posts back. Here every change must still match the
/// stored text it was computed from, so a diff reviewed before someone else edited a member is
/// refused rather than applied over that edit.
///
/// # Errors
/// 404, 403 as for the diff; 422 for a change that no longer applies; 400 for a malformed body.
pub async fn apply(api: &Api<'_>, raw_id: &str, bytes: &[u8]) -> Result<ApiResponse, ApiError> {
    let vocabulary = editable(api, raw_id).await?;
    let diff: Diff =
        serde_json::from_slice(bytes).map_err(|error| ApiError::bad_request(error.to_string()))?;
    let mut errors = ValidationErrors::new();
    let mut plans = Vec::new();
    for (member, changes) in [
        (Member::Class, &diff.classes),
        (Member::Property, &diff.properties),
    ] {
        let stored = vocab_member::list(api, member, vocabulary.id).await?;
        let plan = plan(&stored, changes);
        if !plan.problems.is_empty() {
            errors.insert(member.list_key().to_string(), plan.problems.clone());
        }
        plans.push((member, plan));
    }
    if !errors.is_empty() {
        return Err(ApiError::Validation(errors));
    }
    for (member, plan) in plans {
        for row in &plan.update {
            vocab_member::update(api, member, row).await?;
        }
        for input in &plan.create {
            vocab_member::create(api, member, vocabulary.id, input).await?;
        }
    }
    api.read_member("vocabularies", raw_id).await
}

/// The writes one kind's diff makes, or what keeps it from applying.
#[derive(Debug, Default)]
struct Plan {
    create: Vec<MemberInput>,
    update: Vec<MemberRow>,
    problems: Vec<String>,
}

/// Check one kind's diff against the stored members and turn it into writes.
fn plan(stored: &[MemberRow], changes: &MemberDiff) -> Plan {
    let mut plan = Plan::default();
    let mut rows: Vec<MemberRow> = stored.to_vec();
    for change in &changes.label {
        match rows
            .iter_mut()
            .find(|row| row.local_name == change.local_name)
        {
            None => plan
                .problems
                .push(messages::vocabulary_member_missing(&change.local_name)),
            Some(row) if row.label != change.from => plan
                .problems
                .push(messages::vocabulary_change_stale(&change.local_name)),
            Some(_) if change.to.is_empty() => plan.problems.push(messages::LABEL_EMPTY.into()),
            Some(row) => row.label.clone_from(&change.to),
        }
    }
    for change in &changes.comment {
        match rows
            .iter_mut()
            .find(|row| row.local_name == change.local_name)
        {
            None => plan
                .problems
                .push(messages::vocabulary_member_missing(&change.local_name)),
            Some(row) if row.comment != change.from => plan
                .problems
                .push(messages::vocabulary_change_stale(&change.local_name)),
            Some(row) => row.comment.clone_from(&change.to),
        }
    }
    for term in &changes.new {
        if term.local_name.is_empty() {
            plan.problems.push(messages::LOCAL_NAME_EMPTY.into());
        } else if term.label.is_empty() {
            plan.problems.push(messages::LABEL_EMPTY.into());
        } else if stored
            .iter()
            .map(|row| &row.local_name)
            .chain(plan.create.iter().map(|input| &input.local_name))
            .any(|name| *name == term.local_name)
        {
            plan.problems
                .push(messages::local_name_taken(&term.local_name));
        } else {
            plan.create.push(MemberInput {
                // `RdfImporter::update` builds the entity without an owner.
                owner_id: None,
                vocabulary_id: None,
                local_name: term.local_name.clone(),
                label: term.label.clone(),
                comment: term.comment.clone(),
            });
        }
    }
    plan.update = rows
        .into_iter()
        .zip(stored)
        .filter(|(row, before)| row != *before)
        .map(|(row, _)| row)
        .collect();
    plan
}

/// The vocabulary a diff or an apply changes, when the caller may change it.
async fn editable(api: &Api<'_>, raw_id: &str) -> Result<VocabularyRow, ApiError> {
    api.allow_write("vocabularies", Operation::Update)?;
    let id = parse_id(VOCABULARY_CLASS, raw_id)?;
    let Some(row) = VocabularyRepo::new(api.pool())
        .get(VocabularyId(id))
        .await
        .map_err(db_error)?
    else {
        return Err(not_found(VOCABULARY_CLASS, raw_id));
    };
    access::vocabulary_entity(api.actor(), VOCABULARY_CLASS, row.owner_id)?;
    if PERMANENT_PREFIXES.contains(&row.prefix.as_str()) {
        return Err(ApiError::PermissionDenied {
            message: messages::VOCABULARY_PERMANENT.into(),
        });
    }
    Ok(row)
}

async fn stored_terms(
    api: &Api<'_>,
    member: Member,
    vocabulary: VocabularyId,
) -> Result<Vec<Term>, ApiError> {
    Ok(vocab_member::list(api, member, vocabulary)
        .await?
        .into_iter()
        .map(|row| Term {
            local_name: row.local_name,
            label: row.label,
            comment: row.comment,
        })
        .collect())
}
