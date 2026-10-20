//! `POST`, `PUT`, `PATCH` and `DELETE` for `custom_vocabs`, the term lists.
//!
//! `CustomVocabAdapter::hydrate` writes `o:owner`, `o:label`, `o:lang`,
//! `o:item_set`, `o:terms` and `o:uris`; a `PATCH` writes only the members the
//! body carries, and a `PUT` writes all six, so an absent member clears.
//! `validateEntity` then requires a label that is not empty and not taken, and
//! at least one of the three sources.

use omeka_core::term_list::{self, php_trim};
use omeka_db::ids::{CustomVocabId, ItemSetId};
use omeka_db::repos::{CustomVocabRepo, ItemSetRepo};
use omeka_db::rows::{CustomVocabRow, NewCustomVocab};
use omeka_db::with_db;
use serde_json::Value;

use crate::errors::{ApiError, ValidationErrors};
use crate::read::{Api, ApiResponse, db_error, entity_denied, not_found, parse_id};
use crate::routes::Operation;
use crate::write::body::{Payload, ref_row};
use crate::write::resource::{not_found as ref_not_found, owner_member};

const CLASS: &str = "CustomVocab\\Entity\\CustomVocab";
const ITEM_SET_CLASS: &str = "Omeka\\Entity\\ItemSet";
const RESOURCE: &str = "custom_vocabs";

pub async fn create(api: &Api<'_>, payload: &Payload) -> Result<ApiResponse, ApiError> {
    let input = hydrate(api, payload, None, false).await?;
    validate(api, &input, None).await?;
    let row = with_db!(api.pool(), |db| CustomVocabRepo::new(db)
        .create(&input)
        .await)
    .map_err(db_error)?;
    api.read_member(RESOURCE, &row.id.0.to_string()).await
}

pub async fn update(
    api: &Api<'_>,
    raw_id: &str,
    payload: &Payload,
    partial: bool,
) -> Result<ApiResponse, ApiError> {
    let stored = stored(api, raw_id).await?;
    owner_only(api, &stored, Operation::Update)?;
    let input = hydrate(api, payload, Some(&stored), partial).await?;
    validate(api, &input, Some(stored.id)).await?;
    let row = CustomVocabRow {
        id: stored.id,
        item_set_id: input.item_set_id,
        owner_id: input.owner_id,
        label: input.label,
        lang: input.lang,
        terms: input.terms,
        uris: input.uris,
    };
    crate::publish::guard::term_list(api, stored.id, Some(&row)).await?;
    with_db!(api.pool(), |db| CustomVocabRepo::new(db).update(&row).await).map_err(db_error)?;
    api.read_member(RESOURCE, raw_id).await
}

/// Delete a list. Its values become `literal` and its name leaves every
/// template row (`CustomVocabRepo::delete`). A published list is not deleted
/// (`crate::publish::guard`).
pub async fn delete(api: &Api<'_>, raw_id: &str) -> Result<ApiResponse, ApiError> {
    let stored = stored(api, raw_id).await?;
    owner_only(api, &stored, Operation::Delete)?;
    crate::publish::guard::term_list(api, stored.id, None).await?;
    let answer = api.read_member(RESOURCE, raw_id).await?;
    with_db!(api.pool(), |db| CustomVocabRepo::new(db)
        .delete(stored.id)
        .await)
    .map_err(db_error)?;
    Ok(answer)
}

async fn stored(api: &Api<'_>, raw_id: &str) -> Result<CustomVocabRow, ApiError> {
    let id = parse_id(CLASS, raw_id)?;
    with_db!(api.pool(), |db| CustomVocabRepo::new(db)
        .get(CustomVocabId(id))
        .await)
    .map_err(db_error)?
    .ok_or_else(|| not_found(CLASS, raw_id))
}

/// `Module::onBootstrap`: an editor may update or delete a list they own
/// (`OwnsEntityAssertion`), and both admin roles may change any. The adapter
/// grant has already refused every other role.
pub(crate) fn owner_only(
    api: &Api<'_>,
    row: &CustomVocabRow,
    operation: Operation,
) -> Result<(), ApiError> {
    let actor = api.actor();
    if crate::write::access::is_admin_role(actor.role()) {
        return Ok(());
    }
    match (actor.user(), row.owner_id) {
        (Some(me), Some(owner)) if me == owner => Ok(()),
        _ => Err(entity_denied(CLASS, operation)),
    }
}

/// `CustomVocabAdapter::hydrate` plus `hydrateOwner`. `stored` is the row an
/// update starts from; a `PATCH` keeps every column whose member is absent.
async fn hydrate(
    api: &Api<'_>,
    payload: &Payload,
    stored: Option<&CustomVocabRow>,
    partial: bool,
) -> Result<NewCustomVocab, ApiError> {
    let writes = |key: &str| !partial || payload.has(key);
    let owner_id = match stored {
        Some(row) if !writes("o:owner") => row.owner_id,
        _ => {
            owner_member(
                api,
                payload,
                stored.is_none(),
                stored.and_then(|row| row.owner_id),
            )
            .await?
        }
    };
    let label = match stored {
        Some(row) if !writes("o:label") => row.label.clone(),
        _ => text(payload.get("o:label")),
    };
    let lang = match stored {
        Some(row) if !writes("o:lang") => row.lang.clone(),
        _ => {
            Some(php_trim(&text(payload.get("o:lang"))).to_string()).filter(|lang| !lang.is_empty())
        }
    };
    let item_set_id = match stored {
        Some(row) if !writes("o:item_set") => row.item_set_id,
        _ => item_set(api, payload).await?,
    };
    let terms = match stored {
        Some(row) if !writes("o:terms") => row.terms.clone(),
        _ => payload
            .get("o:terms")
            .and_then(term_list::sanitize_terms)
            .map(Value::from),
    };
    let uris = match stored {
        Some(row) if !writes("o:uris") => row.uris.clone(),
        _ => payload
            .get("o:uris")
            .and_then(term_list::sanitize_uris)
            .map(|uris| term_list::uris_json(&uris)),
    };
    Ok(NewCustomVocab {
        item_set_id,
        owner_id,
        label,
        lang,
        terms,
        uris,
    })
}

/// `o:item_set`: a numeric `o:id` names a stored item set (`findEntity` throws
/// when it does not exist), anything else clears the member.
async fn item_set(api: &Api<'_>, payload: &Payload) -> Result<Option<ItemSetId>, ApiError> {
    let Some(raw) = payload.numeric_ref("o:item_set") else {
        return Ok(None);
    };
    let Some(id) = ref_row(raw) else {
        return Err(ref_not_found(ITEM_SET_CLASS, raw));
    };
    let id = ItemSetId(id);
    if with_db!(api.pool(), |db| ItemSetRepo::new(db).get(id).await)
        .map_err(db_error)?
        .is_none()
    {
        return Err(ref_not_found(ITEM_SET_CLASS, raw));
    }
    Ok(Some(id))
}

/// `CustomVocabAdapter::validateEntity`.
async fn validate(
    api: &Api<'_>,
    input: &NewCustomVocab,
    own: Option<CustomVocabId>,
) -> Result<(), ApiError> {
    let mut errors = ValidationErrors::new();
    // `false == trim($label)` is true for the string "0" as well as for "".
    let trimmed = php_trim(&input.label);
    if trimmed.is_empty() || trimmed == "0" {
        errors
            .entry("o:label".to_string())
            .or_default()
            .push("The label cannot be empty.".to_string());
    }
    let taken = with_db!(api.pool(), |db| CustomVocabRepo::new(db)
        .find_by_label(&input.label)
        .await)
    .map_err(db_error)?
    .is_some_and(|row| Some(row.id) != own);
    if taken {
        errors
            .entry("o:label".to_string())
            .or_default()
            .push("The label is already taken.".to_string());
    }
    if input.item_set_id.is_none() && input.terms.is_none() && input.uris.is_none() {
        errors
            .entry("o:terms".to_string())
            .or_default()
            .push("The item set, terms, and URIs cannot all be empty.".to_string());
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(ApiError::Validation(errors))
    }
}

/// A member as PHP's string cast reads it; an absent or non-scalar member is
/// the empty string.
fn text(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Number(number)) => number.to_string(),
        Some(Value::Bool(true)) => "1".to_string(),
        _ => String::new(),
    }
}
