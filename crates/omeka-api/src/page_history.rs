//! A site page's version history and its save conflict check (Romeka additions).
//!
//! Every create and save stores the page document it makes as a version
//! ([`omeka_db::repos::PageVersionRepo`]). `GET /api/site_pages/{id}/versions` lists them,
//! newest first, each with what made it, who, and what it changed against the version before it
//! ([`crate::page_changes`]), and `GET /api/site_pages/{id}/versions/{version}` answers one
//! version's document.
//! Both need a signed-in caller who may read the page.
//!
//! A save that sends the `o:modified` it loaded is refused with 409 when the page was saved since;
//! the answer names when and by whom. A save without `o:modified` is never refused, so an upstream
//! client behaves as it does upstream.

use chrono::{DateTime, NaiveDateTime};
use omeka_db::ids::{PageId, UserId};
use omeka_db::repos::{PageVersionRepo, SiteRepo, UserRepo};
use omeka_db::rows::{PageVersionOrigin, PageVersionRow, VersionAuthor};
use omeka_db::with_db;
use serde::Serialize;
use serde_json::Value;

use crate::errors::{ApiError, SavedBy, StalePageError};
use crate::messages;
use crate::page_changes::{self, VersionChanges};
use crate::read::{Api, ApiResponse, db_error, jsonld_date, not_found, parse_id, serialize};
use crate::routes::Operation;
use crate::wire::common::JsonLdDate;
use crate::write::body::Payload;

const PAGE_CLASS: &str = "Omeka\\Entity\\SitePage";

/// The `o:modified` a save body carries, as a read answers it (`{"@value": ...}`) or as the bare
/// ISO 8601 text. Anything else is no claim, so the save is not checked.
pub(crate) fn loaded_modified(payload: &Payload) -> Option<NaiveDateTime> {
    let text = match payload.get("o:modified")? {
        Value::String(text) => text.as_str(),
        Value::Object(object) => object.get("@value")?.as_str()?,
        _ => return None,
    };
    DateTime::parse_from_rfc3339(text)
        .ok()
        .map(|at| at.naive_utc())
}

/// The 409 for a save that lost the race: when the stored page was saved, and who saved it.
pub(crate) async fn stale(
    api: &Api<'_>,
    modified: Option<NaiveDateTime>,
    saved_by: Option<UserId>,
) -> ApiError {
    let user = match saved_by {
        Some(id) => match user_ref(api, id).await {
            Ok(user) => user,
            Err(error) => return error,
        },
        None => None,
    };
    ApiError::StalePage(Box::new(StalePageError {
        error: messages::PAGE_SAVED_SINCE_OPENED.to_string(),
        modified: modified.map(jsonld_date),
        user,
    }))
}

async fn user_ref(api: &Api<'_>, id: UserId) -> Result<Option<SavedBy>, ApiError> {
    Ok(with_db!(api.pool(), |db| UserRepo::new(db).get(id).await)
        .map_err(db_error)?
        .map(|row| SavedBy {
            id: row.id.0,
            name: row.name,
        }))
}

/// What made a version, as the list names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum Origin {
    /// The state the page had before its first recorded save.
    BeforeHistory,
    /// The page create.
    Create,
    Save,
}

impl From<PageVersionOrigin> for Origin {
    fn from(origin: PageVersionOrigin) -> Self {
        match origin {
            PageVersionOrigin::BeforeHistory => Self::BeforeHistory,
            PageVersionOrigin::Create => Self::Create,
            PageVersionOrigin::Save => Self::Save,
        }
    }
}

/// Whether a version names its author, and why not when it does not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum AuthorState {
    /// `o:user` names who made it.
    User,
    /// The state the page had before its first recorded save.
    NotRecorded,
    /// The user who made it was deleted.
    DeletedUser,
}

/// One entry of the version list.
#[derive(Debug, Serialize)]
struct VersionEntry {
    #[serde(rename = "o:version")]
    version: u32,
    #[serde(rename = "o:created")]
    created: JsonLdDate,
    #[serde(rename = "o:origin")]
    origin: Origin,
    #[serde(rename = "o:user")]
    user: Option<SavedBy>,
    #[serde(rename = "o:author")]
    author: AuthorState,
    #[serde(rename = "o:block_count")]
    block_count: usize,
    /// What this version changed against the one listed after it; `null` for the oldest kept.
    #[serde(rename = "o:changes")]
    changes: Option<VersionChanges>,
}

/// `GET /api/site_pages/{id}/versions`: every kept version, newest first, each with what it
/// changed. The changes are computed here so the list never ships a snapshot.
pub async fn list(api: &Api<'_>, raw_id: &str) -> Result<ApiResponse, ApiError> {
    let page = readable_page(api, raw_id).await?;
    let rows =
        with_db!(api.pool(), |db| PageVersionRepo::new(db).list(page).await).map_err(db_error)?;
    let mut entries = Vec::with_capacity(rows.len());
    for (index, row) in rows.iter().enumerate() {
        let older = rows.get(index + 1).map(|older| &older.snapshot);
        entries.push(entry(api, row, older).await?);
    }
    Ok(ApiResponse::document(serialize(&entries)?))
}

async fn entry(
    api: &Api<'_>,
    row: &PageVersionRow,
    older: Option<&Value>,
) -> Result<VersionEntry, ApiError> {
    let (user, author) = match row.author() {
        VersionAuthor::User(id) => match user_ref(api, id).await? {
            Some(user) => (Some(user), AuthorState::User),
            None => (None, AuthorState::DeletedUser),
        },
        VersionAuthor::NotRecorded => (None, AuthorState::NotRecorded),
        VersionAuthor::Deleted => (None, AuthorState::DeletedUser),
    };
    Ok(VersionEntry {
        version: row.version,
        created: jsonld_date(row.created),
        origin: row.origin.into(),
        user,
        author,
        block_count: row
            .snapshot
            .get("o:block")
            .and_then(Value::as_array)
            .map_or(0, Vec::len),
        changes: older.map(|older| page_changes::changes(older, &row.snapshot)),
    })
}

/// `GET /api/site_pages/{id}/versions/{version}`: the page document as that version stored it.
pub async fn version(
    api: &Api<'_>,
    raw_id: &str,
    raw_version: &str,
) -> Result<ApiResponse, ApiError> {
    let page = readable_page(api, raw_id).await?;
    let missing = || ApiError::not_found(messages::NOT_FOUND);
    let number: u32 = raw_version.parse().map_err(|_| missing())?;
    let row = with_db!(api.pool(), |db| PageVersionRepo::new(db)
        .get(page, number)
        .await)
    .map_err(db_error)?
    .ok_or_else(missing)?;
    Ok(ApiResponse::document(serialize(&row.snapshot)?))
}

/// The page, when the caller is signed in and may read it. Anonymous callers are refused even for
/// a public page: a version can hold content the page no longer shows.
async fn readable_page(api: &Api<'_>, raw_id: &str) -> Result<PageId, ApiError> {
    if api.actor().user().is_none() {
        return Err(crate::read::permission_denied(
            "site_pages",
            Operation::Read,
        ));
    }
    let id = PageId(parse_id(PAGE_CLASS, raw_id)?);
    let Some(row) =
        with_db!(api.pool(), |db| SiteRepo::new(db).get_page(id).await).map_err(db_error)?
    else {
        return Err(not_found(PAGE_CLASS, raw_id));
    };
    if !crate::read::site_page_visible(api, &row).await? {
        return Err(not_found(PAGE_CLASS, raw_id));
    }
    Ok(id)
}
