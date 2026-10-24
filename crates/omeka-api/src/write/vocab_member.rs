//! The columns `resource_class` and `property` share, and the two
//! repositories that own their tables.
//!
//! `ResourceClassAdapter` and `PropertyAdapter` hydrate and validate the same
//! five members (`o:owner`, `o:vocabulary`, `o:local_name`, `o:label`,
//! `o:comment`) and run the same uniqueness check on `(vocabulary, localName)`.
//! `VocabularyAdapter::hydrate` reuses both adapters for the `o:class` and
//! `o:property` lists it carries, so the code lives here once and [`Member`]
//! names which of the two a call is about.

use omeka_db::ids::{UserId, VocabularyId};
use omeka_db::repos::{
    ClassFilter, ClassRepo, Paging, PropertyFilter, PropertyRepo, VocabularyRepo,
};
use omeka_db::rows::{ClassRow, NewClass, NewProperty, PropertyRow};
use omeka_db::with_db;

use crate::errors::{ApiError, ValidationErrors};
use crate::messages;
use crate::read::{Api, db_error, not_found, taken_or_db_error};
use crate::write::body::{Payload, ref_row};
use crate::write::resource;
use crate::write::resource::not_found as ref_not_found;

/// Which vocabulary member a call is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Member {
    Class,
    Property,
}

impl Member {
    /// `getAdapter('resource_classes'|'properties')`'s entity class, the name a
    /// missing row's 404 carries (`AbstractEntityAdapter::findEntity`).
    pub fn php_class(self) -> &'static str {
        match self {
            Self::Class => "Omeka\\Entity\\ResourceClass",
            Self::Property => "Omeka\\Entity\\Property",
        }
    }

    /// The JSON-LD member a vocabulary carries a list of these under.
    pub fn list_key(self) -> &'static str {
        match self {
            Self::Class => "o:class",
            Self::Property => "o:property",
        }
    }

    /// The message `VocabularyAdapter::validateRequest` gives a non-array list
    /// member. Upstream files both lists under the same `o:item_set` key
    /// (`VocabularyAdapter.php:65,71`); the port reproduces the key.
    pub fn non_array_message(self) -> &'static str {
        match self {
            Self::Class => messages::CLASSES_NOT_ARRAY,
            Self::Property => messages::PROPERTIES_NOT_ARRAY,
        }
    }
}

/// The vocabulary a member's validation runs against.
///
/// A member created through a vocabulary's `o:class` / `o:property` list
/// belongs to the vocabulary being written: an existing row for an update, or
/// the row this request creates, whose id the database assigns at insert.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum VocabularyTarget {
    /// A stored vocabulary row.
    Stored(VocabularyId),
    /// The vocabulary this request is creating. Upstream's
    /// `if ($entity->getVocabulary()->getId())` skips the uniqueness check,
    /// because the row has no id yet.
    Pending,
    /// The body named no vocabulary, which a standalone create must.
    Missing,
}

/// One `resource_class` or `property` row in the shape both share.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MemberRow {
    pub id: u64,
    pub owner_id: Option<UserId>,
    pub vocabulary_id: VocabularyId,
    pub local_name: String,
    pub label: String,
    pub comment: Option<String>,
}

/// The hydratable columns of one member, before validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MemberInput {
    pub owner_id: Option<UserId>,
    /// The vocabulary the body named through `o:vocabulary[o:id]`, when it
    /// named one.
    pub vocabulary_id: Option<VocabularyId>,
    pub local_name: String,
    pub label: String,
    pub comment: Option<String>,
}

impl MemberInput {
    /// The vocabulary a write stores with: the body's override, or the
    /// vocabulary the request is about.
    pub fn vocabulary_or(&self, parent: VocabularyId) -> VocabularyId {
        self.vocabulary_id.unwrap_or(parent)
    }

    /// The vocabulary validation runs against.
    pub fn target(&self, parent: VocabularyTarget) -> VocabularyTarget {
        match self.vocabulary_id {
            Some(id) => VocabularyTarget::Stored(id),
            None => parent,
        }
    }
}

/// `ResourceClassAdapter::hydrate` and `PropertyAdapter::hydrate`.
///
/// `o:vocabulary` is read on create only, and only when it carries a numeric
/// `o:id` that names a stored vocabulary (`findEntity` throws otherwise); on
/// update the vocabulary is left alone.
pub(crate) async fn hydrate(
    api: &Api<'_>,
    payload: &Payload,
    existing: Option<&MemberRow>,
    partial: bool,
) -> Result<MemberInput, ApiError> {
    let create = existing.is_none();
    let owner_id =
        resource::owner_member(api, payload, create, existing.and_then(|row| row.owner_id)).await?;
    let local_name = member_or(
        payload,
        "o:local_name",
        partial,
        existing.map(|row| &row.local_name),
    );
    let label = member_or(payload, "o:label", partial, existing.map(|row| &row.label));
    let comment = match (partial && !payload.has("o:comment"), existing) {
        (true, Some(row)) => row.comment.clone(),
        _ => payload.str_member("o:comment").map(str::to_string),
    };
    let vocabulary_id = if create {
        vocabulary(api, payload).await?
    } else {
        existing.map(|row| row.vocabulary_id)
    };
    Ok(MemberInput {
        owner_id,
        vocabulary_id,
        local_name,
        label,
        comment,
    })
}

/// The stored vocabulary a create names, or `None` when it names none.
async fn vocabulary(api: &Api<'_>, payload: &Payload) -> Result<Option<VocabularyId>, ApiError> {
    let Some(raw) = payload.numeric_ref("o:vocabulary") else {
        return Ok(None);
    };
    let Some(id) = ref_row(raw) else {
        return Err(ref_not_found("Omeka\\Entity\\Vocabulary", raw));
    };
    let id = VocabularyId(id);
    if with_db!(api.pool(), |db| VocabularyRepo::new(db).get(id).await)
        .map_err(db_error)?
        .is_none()
    {
        return Err(ref_not_found("Omeka\\Entity\\Vocabulary", raw));
    }
    Ok(Some(id))
}

/// `ResourceClassAdapter::validateEntity` / `PropertyAdapter::validateEntity`.
///
/// `self_id` is the row being updated, which the uniqueness check excludes;
/// `None` on a create.
pub(crate) async fn validate(
    api: &Api<'_>,
    member: Member,
    input: &MemberInput,
    self_id: Option<u64>,
    target: VocabularyTarget,
) -> Result<(), ApiError> {
    let mut errors = ValidationErrors::new();
    if input.local_name.is_empty() {
        push(&mut errors, "o:local_name", messages::LOCAL_NAME_EMPTY);
    }
    if input.label.is_empty() {
        push(&mut errors, "o:label", messages::LABEL_EMPTY);
    }
    match target {
        VocabularyTarget::Missing => {
            push(&mut errors, "o:vocabulary", messages::VOCABULARY_REQUIRED);
        }
        VocabularyTarget::Pending => {}
        VocabularyTarget::Stored(vocabulary_id) => {
            let taken = find(api, member, vocabulary_id, &input.local_name).await?;
            if taken.is_some_and(|id| Some(id) != self_id) {
                push(
                    &mut errors,
                    "o:local_name",
                    messages::local_name_taken(&input.local_name),
                );
            }
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(ApiError::Validation(errors))
    }
}

/// One row by id.
pub(crate) async fn get(
    api: &Api<'_>,
    member: Member,
    id: u64,
) -> Result<Option<MemberRow>, ApiError> {
    Ok(match member {
        Member::Class => with_db!(api.pool(), |db| ClassRepo::new(db)
            .get(omeka_db::ids::ClassId(id))
            .await)
        .map_err(db_error)?
        .map(class_row),
        Member::Property => with_db!(api.pool(), |db| PropertyRepo::new(db)
            .get(omeka_db::ids::PropertyId(id))
            .await)
        .map_err(db_error)?
        .map(property_row),
    })
}

/// Every member of one vocabulary, in id order.
pub(crate) async fn list(
    api: &Api<'_>,
    member: Member,
    vocabulary_id: VocabularyId,
) -> Result<Vec<MemberRow>, ApiError> {
    Ok(match member {
        Member::Class => with_db!(api.pool(), |db| ClassRepo::new(db)
            .list(
                &ClassFilter {
                    vocabulary_id: Some(vocabulary_id),
                    ..ClassFilter::default()
                },
                &Paging::default()
            )
            .await)
        .map_err(db_error)?
        .into_iter()
        .map(class_row)
        .collect(),
        Member::Property => with_db!(api.pool(), |db| PropertyRepo::new(db)
            .list(
                &PropertyFilter {
                    vocabulary_id: Some(vocabulary_id),
                    ..PropertyFilter::default()
                },
                &Paging::default()
            )
            .await)
        .map_err(db_error)?
        .into_iter()
        .map(property_row)
        .collect(),
    })
}

/// The id of the member with this local name in this vocabulary, if any.
async fn find(
    api: &Api<'_>,
    member: Member,
    vocabulary_id: VocabularyId,
    local_name: &str,
) -> Result<Option<u64>, ApiError> {
    Ok(match member {
        Member::Class => with_db!(api.pool(), |db| ClassRepo::new(db)
            .find(vocabulary_id, local_name)
            .await)
        .map_err(db_error)?
        .map(|row| row.id.0),
        Member::Property => with_db!(api.pool(), |db| PropertyRepo::new(db)
            .find(vocabulary_id, local_name)
            .await)
        .map_err(db_error)?
        .map(|row| row.id.0),
    })
}

/// Insert one member.
pub(crate) async fn create(
    api: &Api<'_>,
    member: Member,
    vocabulary_id: VocabularyId,
    input: &MemberInput,
) -> Result<MemberRow, ApiError> {
    Ok(match member {
        Member::Class => class_row(
            with_db!(api.pool(), |db| ClassRepo::new(db)
                .create(&NewClass {
                    owner_id: input.owner_id,
                    vocabulary_id,
                    local_name: input.local_name.clone(),
                    label: input.label.clone(),
                    comment: input.comment.clone(),
                })
                .await)
            .map_err(taken_or_db_error(
                "o:local_name",
                messages::local_name_taken(&input.local_name),
            ))?,
        ),
        Member::Property => property_row(
            with_db!(api.pool(), |db| PropertyRepo::new(db)
                .create(&NewProperty {
                    owner_id: input.owner_id,
                    vocabulary_id,
                    local_name: input.local_name.clone(),
                    label: input.label.clone(),
                    comment: input.comment.clone(),
                })
                .await)
            .map_err(taken_or_db_error(
                "o:local_name",
                messages::local_name_taken(&input.local_name),
            ))?,
        ),
    })
}

/// Write every writable column of a member row.
pub(crate) async fn update(api: &Api<'_>, member: Member, row: &MemberRow) -> Result<(), ApiError> {
    let done = match member {
        Member::Class => with_db!(api.pool(), |db| ClassRepo::new(db)
            .update(&ClassRow {
                id: omeka_db::ids::ClassId(row.id),
                owner_id: row.owner_id,
                vocabulary_id: row.vocabulary_id,
                local_name: row.local_name.clone(),
                label: row.label.clone(),
                comment: row.comment.clone(),
            })
            .await)
        .map_err(taken_or_db_error(
            "o:local_name",
            messages::local_name_taken(&row.local_name),
        ))?,
        Member::Property => with_db!(api.pool(), |db| PropertyRepo::new(db)
            .update(&PropertyRow {
                id: omeka_db::ids::PropertyId(row.id),
                owner_id: row.owner_id,
                vocabulary_id: row.vocabulary_id,
                local_name: row.local_name.clone(),
                label: row.label.clone(),
                comment: row.comment.clone(),
            })
            .await)
        .map_err(taken_or_db_error(
            "o:local_name",
            messages::local_name_taken(&row.local_name),
        ))?,
    };
    if done {
        Ok(())
    } else {
        Err(not_found(member.php_class(), &row.id.to_string()))
    }
}

/// Remove one member.
///
/// The database carries the upstream cascades: the values that used the
/// property and the template rows that listed it go too (`value.property_id`
/// and `resource_template_property.property_id` are `ON DELETE CASCADE`), and
/// a class delete clears `resource.resource_class_id` (`ON DELETE SET NULL`).
pub(crate) async fn delete(api: &Api<'_>, member: Member, id: u64) -> Result<(), ApiError> {
    let done = match member {
        Member::Class => with_db!(api.pool(), |db| ClassRepo::new(db)
            .delete(omeka_db::ids::ClassId(id))
            .await)
        .map_err(db_error)?,
        Member::Property => with_db!(api.pool(), |db| PropertyRepo::new(db)
            .delete(omeka_db::ids::PropertyId(id))
            .await)
        .map_err(db_error)?,
    };
    if done {
        Ok(())
    } else {
        Err(not_found(member.php_class(), &id.to_string()))
    }
}

/// A member's value on a full write, or the stored one on a partial write that
/// omits it. An absent member on a full write hydrates to the empty string,
/// which `setLocalName(getValue(...))` does.
fn member_or(payload: &Payload, key: &str, partial: bool, existing: Option<&String>) -> String {
    match (partial && !payload.has(key), payload.str_member(key)) {
        (true, _) => existing.cloned().unwrap_or_default(),
        (false, Some(value)) => value.to_string(),
        (false, None) => String::new(),
    }
}

/// `ErrorStore::addError`: a message is appended to the key's list.
pub(crate) fn push(errors: &mut ValidationErrors, key: &str, message: impl Into<String>) {
    errors
        .entry(key.to_string())
        .or_default()
        .push(message.into());
}

fn class_row(row: ClassRow) -> MemberRow {
    MemberRow {
        id: row.id.0,
        owner_id: row.owner_id,
        vocabulary_id: row.vocabulary_id,
        local_name: row.local_name,
        label: row.label,
        comment: row.comment,
    }
}

fn property_row(row: PropertyRow) -> MemberRow {
    MemberRow {
        id: row.id.0,
        owner_id: row.owner_id,
        vocabulary_id: row.vocabulary_id,
        local_name: row.local_name,
        label: row.label,
        comment: row.comment,
    }
}
