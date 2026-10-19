//! `POST`, `PUT`, `PATCH` and `DELETE` for `vocabularies`, `resource_classes`
//! and `properties`.
//!
//! `VocabularyAdapter::hydrate` writes `o:namespace_uri`, `o:prefix` and
//! `o:label` through `trim`, and carries two lists: `o:class` and `o:property`.
//! An entry that names an `o:id` is kept as it is; an entry that names none is
//! created through the member adapter; and every member of the vocabulary the
//! list does not name is removed (`orphanRemoval` deletes the row).
//!
//! `ResourceClassAdapter::hydrate` and `PropertyAdapter::hydrate` live in
//! [`super::vocab_member`], which this module reuses for the nested lists.
//!
//! `validateEntity` runs after hydration, so a nested entry's failure is
//! reported before the vocabulary's own fields are checked, and both lists file
//! their duplicate local names under the one key `o:resource_class`.

use omeka_db::ids::VocabularyId;
use omeka_db::repos::{VocabularyFilter, VocabularyRepo};
use omeka_db::rows::{NewVocabulary, VocabularyRow};

use crate::errors::{ApiError, ValidationErrors};
use crate::messages;
use crate::publish::guard::{self, VocabularyChange};
use crate::read::{Api, ApiResponse, db_error, not_found, parse_id};
use crate::write::access;
use crate::write::body::{Payload, ref_row};
use crate::write::resource;
use crate::write::vocab_member::{self, Member, MemberInput, MemberRow, VocabularyTarget};

const VOCABULARY_CLASS: &str = "Omeka\\Entity\\Vocabulary";

/// The hydratable columns of one vocabulary, before validation.
struct VocabularyInput {
    owner_id: Option<omeka_db::ids::UserId>,
    namespace_uri: String,
    prefix: String,
    label: String,
    comment: Option<String>,
}

/// What a vocabulary's `o:class` / `o:property` list asks for.
#[derive(Debug, Default)]
struct MemberPlan {
    /// The new members to insert, hydrated and validated.
    create: Vec<MemberInput>,
    /// The stored members to remove.
    remove: Vec<u64>,
    /// The local names of the vocabulary's members after the list is applied,
    /// for `validateEntity`'s duplicate check.
    local_names: Vec<String>,
}

// ----------------------------------------------------------- vocabularies

pub async fn create_vocabulary(api: &Api<'_>, payload: &Payload) -> Result<ApiResponse, ApiError> {
    validate_request(payload)?;
    let input = hydrate_vocabulary(api, payload, None, false).await?;
    let classes = plan_members(
        api,
        Member::Class,
        VocabularyTarget::Pending,
        payload,
        true,
        false,
        &[],
    )
    .await?;
    let properties = plan_members(
        api,
        Member::Property,
        VocabularyTarget::Pending,
        payload,
        true,
        false,
        &[],
    )
    .await?;
    validate_vocabulary(api, &input, None, classes.as_ref(), properties.as_ref()).await?;
    let row = VocabularyRepo::new(api.pool())
        .create(&NewVocabulary {
            owner_id: input.owner_id,
            namespace_uri: input.namespace_uri,
            prefix: input.prefix,
            label: input.label,
            comment: input.comment,
        })
        .await
        .map_err(db_error)?;
    apply_members(api, Member::Class, row.id, classes.as_ref()).await?;
    apply_members(api, Member::Property, row.id, properties.as_ref()).await?;
    api.read_member("vocabularies", &row.id.0.to_string()).await
}

pub async fn update_vocabulary(
    api: &Api<'_>,
    raw_id: &str,
    payload: &Payload,
    partial: bool,
) -> Result<ApiResponse, ApiError> {
    validate_request(payload)?;
    let id = parse_id(VOCABULARY_CLASS, raw_id)?;
    let Some(row) = VocabularyRepo::new(api.pool())
        .get(VocabularyId(id))
        .await
        .map_err(db_error)?
    else {
        return Err(not_found(VOCABULARY_CLASS, raw_id));
    };
    access::vocabulary_entity(api.actor(), VOCABULARY_CLASS, row.owner_id)?;
    let input = hydrate_vocabulary(api, payload, Some(&row), partial).await?;
    let existing_classes = vocab_member::list(api, Member::Class, row.id).await?;
    let existing_properties = vocab_member::list(api, Member::Property, row.id).await?;
    let classes = plan_members(
        api,
        Member::Class,
        VocabularyTarget::Stored(row.id),
        payload,
        false,
        partial,
        &existing_classes,
    )
    .await?;
    let properties = plan_members(
        api,
        Member::Property,
        VocabularyTarget::Stored(row.id),
        payload,
        false,
        partial,
        &existing_properties,
    )
    .await?;
    validate_vocabulary(api, &input, Some(id), classes.as_ref(), properties.as_ref()).await?;
    guard::vocabulary(
        api,
        row.id,
        VocabularyChange::Update {
            namespace: &input.namespace_uri,
            classes: classes.as_ref().map(|plan| plan.local_names.as_slice()),
            properties: properties.as_ref().map(|plan| plan.local_names.as_slice()),
        },
    )
    .await?;
    VocabularyRepo::new(api.pool())
        .update(&VocabularyRow {
            id: row.id,
            owner_id: input.owner_id,
            namespace_uri: input.namespace_uri,
            prefix: input.prefix,
            label: input.label,
            comment: input.comment,
        })
        .await
        .map_err(db_error)?;
    apply_members(api, Member::Class, row.id, classes.as_ref()).await?;
    apply_members(api, Member::Property, row.id, properties.as_ref()).await?;
    api.read_member("vocabularies", raw_id).await
}

/// Delete a vocabulary with its classes and properties.
///
/// `Vocabulary` cascades both collections (`Entity/Vocabulary.php:54,65`), and
/// a property delete cascades the values that used it and the template rows
/// that listed it (`value.property_id` and
/// `resource_template_property.property_id`). `VocabularyRepo::delete` runs the
/// three statements in one transaction.
///
/// The adapter grant keeps this to the two admin roles, and
/// `AclFactory::addRulesForSiteAdmin` then refuses a site admin a vocabulary
/// they do not own.
pub async fn delete_vocabulary(api: &Api<'_>, raw_id: &str) -> Result<ApiResponse, ApiError> {
    let id = parse_id(VOCABULARY_CLASS, raw_id)?;
    let repo = VocabularyRepo::new(api.pool());
    let Some(row) = repo.get(VocabularyId(id)).await.map_err(db_error)? else {
        return Err(not_found(VOCABULARY_CLASS, raw_id));
    };
    access::vocabulary_entity(api.actor(), VOCABULARY_CLASS, row.owner_id)?;
    guard::vocabulary(api, row.id, VocabularyChange::Delete).await?;
    let answer = api.read_member("vocabularies", raw_id).await?;
    repo.delete(row.id).await.map_err(db_error)?;
    Ok(answer)
}

/// `VocabularyAdapter::validateRequest`: both lists must be arrays.
fn validate_request(payload: &Payload) -> Result<(), ApiError> {
    let mut errors = ValidationErrors::new();
    for member in [Member::Class, Member::Property] {
        if payload.has(member.list_key()) && !payload.is_array_like(member.list_key()) {
            // Upstream files both under `o:item_set` (`VocabularyAdapter.php:65,71`).
            vocab_member::push(&mut errors, "o:item_set", member.non_array_message());
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(ApiError::Validation(errors))
    }
}

/// `VocabularyAdapter::hydrate`'s five scalar members.
async fn hydrate_vocabulary(
    api: &Api<'_>,
    payload: &Payload,
    existing: Option<&VocabularyRow>,
    partial: bool,
) -> Result<VocabularyInput, ApiError> {
    let owner_id = resource::owner_member(
        api,
        payload,
        existing.is_none(),
        existing.and_then(|row| row.owner_id),
    )
    .await?;
    let trimmed = |key: &str, stored: Option<&str>| match (
        partial && !payload.has(key),
        payload.str_member(key),
    ) {
        (true, _) => stored.unwrap_or_default().to_string(),
        (false, Some(value)) => value.trim().to_string(),
        (false, None) => String::new(),
    };
    let comment = match (partial && !payload.has("o:comment"), existing) {
        (true, Some(row)) => row.comment.clone(),
        _ => payload.str_member("o:comment").map(str::to_string),
    };
    Ok(VocabularyInput {
        owner_id,
        namespace_uri: trimmed(
            "o:namespace_uri",
            existing.map(|row| row.namespace_uri.as_str()),
        ),
        prefix: trimmed("o:prefix", existing.map(|row| row.prefix.as_str())),
        label: trimmed("o:label", existing.map(|row| row.label.as_str())),
        comment,
    })
}

/// `VocabularyAdapter::validateEntity`, plus the two duplicate local-name loops
/// the vocabulary applies to its member lists (both file under
/// `o:resource_class`).
async fn validate_vocabulary(
    api: &Api<'_>,
    input: &VocabularyInput,
    self_id: Option<u64>,
    classes: Option<&MemberPlan>,
    properties: Option<&MemberPlan>,
) -> Result<(), ApiError> {
    let mut errors = ValidationErrors::new();
    if input.namespace_uri.is_empty() {
        vocab_member::push(
            &mut errors,
            "o:namespace_uri",
            messages::NAMESPACE_URI_EMPTY,
        );
    }
    if namespace_taken(api, &input.namespace_uri, self_id).await? {
        vocab_member::push(
            &mut errors,
            "o:namespace_uri",
            messages::namespace_uri_taken(&input.namespace_uri),
        );
    }
    if input.prefix.is_empty() {
        vocab_member::push(&mut errors, "o:prefix", messages::PREFIX_EMPTY);
    }
    if !input.prefix.is_empty() && !prefix_is_valid(&input.prefix) {
        vocab_member::push(&mut errors, "o:prefix", messages::PREFIX_INVALID);
    }
    if prefix_taken(api, &input.prefix, self_id).await? {
        vocab_member::push(
            &mut errors,
            "o:prefix",
            messages::prefix_taken(&input.prefix),
        );
    }
    for reserved in ["^o$", "^o-"] {
        if reserved_prefix(reserved, &input.prefix) {
            vocab_member::push(
                &mut errors,
                "o:prefix",
                messages::prefix_reserved(&input.prefix),
            );
            break;
        }
    }
    if input.label.is_empty() {
        vocab_member::push(&mut errors, "o:label", messages::LABEL_EMPTY);
    }
    for plan in [classes, properties].into_iter().flatten() {
        push_duplicate_local_names(&mut errors, &plan.local_names);
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(ApiError::Validation(errors))
    }
}

/// `VocabularyAdapter::validateEntity`'s two loops over the member
/// collections: a local name that appears twice is refused.
fn push_duplicate_local_names(errors: &mut ValidationErrors, local_names: &[String]) {
    let mut seen: Vec<&str> = Vec::new();
    for name in local_names {
        if seen.contains(&name.as_str()) {
            vocab_member::push(errors, "o:resource_class", messages::local_name_taken(name));
        } else {
            seen.push(name);
        }
    }
}

/// `AbstractEntityAdapter::isUnique` for `vocabulary.namespaceUri`.
async fn namespace_taken(
    api: &Api<'_>,
    namespace_uri: &str,
    self_id: Option<u64>,
) -> Result<bool, ApiError> {
    let rows = VocabularyRepo::new(api.pool())
        .list(
            &VocabularyFilter {
                namespace_uri: Some(namespace_uri.to_string()),
                ..VocabularyFilter::default()
            },
            &omeka_db::repos::Paging::default(),
        )
        .await
        .map_err(db_error)?;
    Ok(rows.iter().any(|row| Some(row.id.0) != self_id))
}

/// `AbstractEntityAdapter::isUnique` for `vocabulary.prefix`.
async fn prefix_taken(api: &Api<'_>, prefix: &str, self_id: Option<u64>) -> Result<bool, ApiError> {
    let Some(row) = VocabularyRepo::new(api.pool())
        .get_by_prefix(prefix)
        .await
        .map_err(db_error)?
    else {
        return Ok(false);
    };
    Ok(Some(row.id.0) != self_id)
}

/// `Vocabulary::PREFIX_REGEX`, `/^[a-z0-9_-]+$/i`.
fn prefix_is_valid(prefix: &str) -> bool {
    prefix
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// `VocabularyAdapter::$reservedPrefixes`, matched with `preg_match("/$p/", $prefix)`.
fn reserved_prefix(pattern: &str, prefix: &str) -> bool {
    match pattern {
        // `^o$` inside the delimiters: the whole prefix is `o`.
        "^o$" => prefix == "o",
        // `^o-`: the prefix starts with `o-`.
        _ => prefix.starts_with("o-"),
    }
}

// ------------------------------------------------- class and property routes

pub async fn create_class(api: &Api<'_>, payload: &Payload) -> Result<ApiResponse, ApiError> {
    create_member(api, Member::Class, payload).await
}

pub async fn create_property(api: &Api<'_>, payload: &Payload) -> Result<ApiResponse, ApiError> {
    create_member(api, Member::Property, payload).await
}

pub async fn update_class(
    api: &Api<'_>,
    raw_id: &str,
    payload: &Payload,
    partial: bool,
) -> Result<ApiResponse, ApiError> {
    update_member(api, Member::Class, raw_id, payload, partial).await
}

pub async fn update_property(
    api: &Api<'_>,
    raw_id: &str,
    payload: &Payload,
    partial: bool,
) -> Result<ApiResponse, ApiError> {
    update_member(api, Member::Property, raw_id, payload, partial).await
}

pub async fn delete_class(api: &Api<'_>, raw_id: &str) -> Result<ApiResponse, ApiError> {
    delete_member(api, Member::Class, raw_id).await
}

pub async fn delete_property(api: &Api<'_>, raw_id: &str) -> Result<ApiResponse, ApiError> {
    delete_member(api, Member::Property, raw_id).await
}

/// One `resource_classes` or `properties` create.
async fn create_member(
    api: &Api<'_>,
    member: Member,
    payload: &Payload,
) -> Result<ApiResponse, ApiError> {
    let input = vocab_member::hydrate(api, payload, None, false).await?;
    vocab_member::validate(
        api,
        member,
        &input,
        None,
        input.target(VocabularyTarget::Missing),
    )
    .await?;
    // `validate` refuses a body with no vocabulary, so this is set.
    let Some(vocabulary_id) = input.vocabulary_id else {
        return Err(ApiError::internal(
            "a validated resource class or property has no vocabulary",
        ));
    };
    let row = vocab_member::create(api, member, vocabulary_id, &input).await?;
    api.read_member(resource_name(member), &row.id.to_string())
        .await
}

/// One `resource_classes` or `properties` update.
async fn update_member(
    api: &Api<'_>,
    member: Member,
    raw_id: &str,
    payload: &Payload,
    partial: bool,
) -> Result<ApiResponse, ApiError> {
    let id = parse_id(member.php_class(), raw_id)?;
    let Some(stored) = vocab_member::get(api, member, id).await? else {
        return Err(not_found(member.php_class(), raw_id));
    };
    let input = vocab_member::hydrate(api, payload, Some(&stored), partial).await?;
    vocab_member::validate(
        api,
        member,
        &input,
        Some(id),
        input.target(VocabularyTarget::Stored(stored.vocabulary_id)),
    )
    .await?;
    let vocabulary_id = input.vocabulary_or(stored.vocabulary_id);
    if input.local_name != stored.local_name || vocabulary_id != stored.vocabulary_id {
        let stays = vocabulary_id == stored.vocabulary_id;
        guard::vocabulary(
            api,
            stored.vocabulary_id,
            VocabularyChange::Member {
                from: &stored.local_name,
                to: stays.then_some(input.local_name.as_str()),
            },
        )
        .await?;
    }
    vocab_member::update(
        api,
        member,
        &MemberRow {
            id: stored.id,
            owner_id: input.owner_id,
            vocabulary_id: input.vocabulary_or(stored.vocabulary_id),
            local_name: input.local_name,
            label: input.label,
            comment: input.comment,
        },
    )
    .await?;
    api.read_member(resource_name(member), raw_id).await
}

/// One `resource_classes` or `properties` delete.
///
/// The adapter grant keeps this to the two admin roles; `Entity\ResourceClass`
/// and `Entity\Property` carry no extra assertion for them.
async fn delete_member(
    api: &Api<'_>,
    member: Member,
    raw_id: &str,
) -> Result<ApiResponse, ApiError> {
    let id = parse_id(member.php_class(), raw_id)?;
    let Some(row) = vocab_member::get(api, member, id).await? else {
        return Err(not_found(member.php_class(), raw_id));
    };
    guard::vocabulary(
        api,
        row.vocabulary_id,
        VocabularyChange::Member {
            from: &row.local_name,
            to: None,
        },
    )
    .await?;
    let answer = api.read_member(resource_name(member), raw_id).await?;
    vocab_member::delete(api, member, row.id).await?;
    Ok(answer)
}

/// The resource name a member's documents are read back through.
fn resource_name(member: Member) -> &'static str {
    match member {
        Member::Class => "resource_classes",
        Member::Property => "properties",
    }
}

// ------------------------------------------------------ the member lists

/// Apply one of a vocabulary's member lists, or `None` when the request does not
/// hydrate it (a partial update that omits the key).
///
/// `VocabularyAdapter::hydrate`: an entry with an `o:id` keeps that member, an
/// entry without one creates a member, and a member of the vocabulary the list
/// does not name is removed.
async fn plan_members(
    api: &Api<'_>,
    member: Member,
    parent: VocabularyTarget,
    payload: &Payload,
    create: bool,
    partial: bool,
    existing: &[MemberRow],
) -> Result<Option<MemberPlan>, ApiError> {
    let key = member.list_key();
    if !create && partial && !payload.has(key) {
        return Ok(None);
    }
    let mut plan = MemberPlan::default();
    let mut retained: Vec<u64> = Vec::new();
    for entry in payload.entries(key) {
        // A vocabulary list entry carries its id at the top level
        // (`VocabularyAdapter::hydrate`: `$classData['o:id']`), read with
        // `isset` and then compared against the stored ids.
        let id = entry.id().and_then(ref_row);
        match id {
            Some(id) => {
                retained.push(id);
                if let Some(row) = existing.iter().find(|row| row.id == id) {
                    plan.local_names.push(row.local_name.clone());
                }
            }
            None => {
                // A nested entry hydrates with no stored row, so a partial flag
                // would be meaningless; upstream passes a CREATE subrequest.
                let input = vocab_member::hydrate(api, &entry, None, false).await?;
                vocab_member::validate(api, member, &input, None, input.target(parent)).await?;
                plan.local_names.push(input.local_name.clone());
                plan.create.push(input);
            }
        }
    }
    for row in existing {
        if !retained.contains(&row.id) {
            plan.remove.push(row.id);
        }
    }
    Ok(Some(plan))
}

/// Insert the plan's new members and remove the ones it drops.
async fn apply_members(
    api: &Api<'_>,
    member: Member,
    vocabulary_id: VocabularyId,
    plan: Option<&MemberPlan>,
) -> Result<(), ApiError> {
    let Some(plan) = plan else {
        return Ok(());
    };
    for id in &plan.remove {
        vocab_member::delete(api, member, *id).await?;
    }
    for input in &plan.create {
        vocab_member::create(api, member, input.vocabulary_or(vocabulary_id), input).await?;
    }
    Ok(())
}
