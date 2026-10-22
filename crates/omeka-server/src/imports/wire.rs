//! The import routes' boundary: who may call them, the error answers, and
//! the JSON an import is shown as.
//!
//! An error answers the usual document, `{"errors": {"error": message}}`.
//! A 409, and a 400 the admin can act on, also carries `code`, the name the
//! contract gives it (`stale`, `v1Plan`, `seriesNeeded`, ...), and the
//! members the admin needs to act on it, so the admin branches on the code
//! and shows the message.

use axum::body::Body;
use axum::http::{HeaderMap, StatusCode, Uri, header};
use axum::response::Response;
use omeka_api::auth::Actor;
use omeka_api::errors::ApiError;
use omeka_api::import::archive::ArchiveError;
use omeka_api::import::catalog_load::Owners;
use omeka_api::import::draft::DraftError;
use omeka_api::import::preview::PreviewError;
use omeka_api::import::scan::ScanError;
use omeka_api::import::start::AssessError;
use omeka_api::messages;
use omeka_db::DbError;
use omeka_db::ids::ImportId;
use omeka_db::repos::{DIED, HEARTBEAT, ImportRepo, RESUMED};
use omeka_db::rows::ImportRow;
use omeka_db::with_db;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value, json};

use crate::AppState;

/// A failure of an import route, with the status it answers.
#[derive(Debug)]
pub(crate) enum ImportError {
    Unauthenticated,
    Forbidden,
    BadRequest(String),
    /// 413: a file past the route's own limit.
    TooLarge(String),
    NotFound(String),
    /// A 409, or a 400 the admin acts on, with its code and the members
    /// that explain it.
    Coded {
        status: StatusCode,
        code: &'static str,
        message: String,
        members: Map<String, Value>,
    },
    Internal(String),
    Api(ApiError),
}

impl ImportError {
    pub(crate) fn conflict(code: &'static str, message: impl Into<String>) -> Self {
        Self::Coded {
            status: StatusCode::CONFLICT,
            code,
            message: message.into(),
            members: Map::new(),
        }
    }

    /// A 400 with a code, for a request the admin can correct.
    pub(crate) fn refused(code: &'static str, message: impl Into<String>) -> Self {
        Self::Coded {
            status: StatusCode::BAD_REQUEST,
            code,
            message: message.into(),
            members: Map::new(),
        }
    }

    /// A coded error with one more member.
    pub(crate) fn with(mut self, key: &str, value: Value) -> Self {
        if let Self::Coded { members, .. } = &mut self {
            members.insert(key.to_string(), value);
        }
        self
    }

    pub(crate) fn into_response(self) -> Response {
        let (status, message, code, members) = match self {
            Self::Api(error) => return crate::error_response(error),
            Self::Unauthenticated => (
                StatusCode::UNAUTHORIZED,
                messages::AUTHENTICATION_REQUIRED.to_string(),
                None,
                Map::new(),
            ),
            Self::Forbidden => (
                StatusCode::FORBIDDEN,
                messages::IMPORT_ADMINS_ONLY.to_string(),
                None,
                Map::new(),
            ),
            Self::BadRequest(message) => (StatusCode::BAD_REQUEST, message, None, Map::new()),
            Self::TooLarge(message) => (StatusCode::PAYLOAD_TOO_LARGE, message, None, Map::new()),
            Self::NotFound(message) => (StatusCode::NOT_FOUND, message, None, Map::new()),
            Self::Coded {
                status,
                code,
                message,
                members,
            } => (status, message, Some(code), members),
            Self::Internal(message) => {
                (StatusCode::INTERNAL_SERVER_ERROR, message, None, Map::new())
            }
        };
        let mut body = Map::new();
        body.insert("errors".to_string(), json!({ "error": message }));
        if let Some(code) = code {
            body.insert("code".to_string(), Value::from(code));
        }
        body.extend(members);
        match serde_json::to_vec(&Value::Object(body)) {
            Ok(bytes) => Response::builder()
                .status(status)
                .header(header::CONTENT_TYPE, crate::JSONLD_CONTENT_TYPE)
                .body(Body::from(bytes))
                .unwrap_or_else(|_| crate::empty_internal_error()),
            Err(_) => crate::empty_internal_error(),
        }
    }
}

impl From<DbError> for ImportError {
    fn from(error: DbError) -> Self {
        Self::Internal(error.to_string())
    }
}

impl From<ArchiveError> for ImportError {
    fn from(error: ArchiveError) -> Self {
        match error {
            ArchiveError::UnknownRoot(root) => Self::NotFound(messages::import_no_root(root)),
            ArchiveError::Outside(path) => Self::BadRequest(messages::import_outside_root(path)),
            ArchiveError::NotFound(path) | ArchiveError::NotAFile(path) => {
                Self::NotFound(messages::import_folder_missing(path))
            }
            ArchiveError::NotAFolder(path) => Self::BadRequest(messages::import_not_a_folder(path)),
            ArchiveError::NoRoot(_) | ArchiveError::Io(_) => Self::Internal(error.to_string()),
        }
    }
}

impl From<AssessError> for ImportError {
    fn from(error: AssessError) -> Self {
        Self::Internal(error.to_string())
    }
}

impl From<PreviewError> for ImportError {
    fn from(error: PreviewError) -> Self {
        Self::Internal(error.to_string())
    }
}

impl From<ScanError> for ImportError {
    fn from(error: ScanError) -> Self {
        Self::Internal(error.to_string())
    }
}

/// A draft operation's refusal as the route answers it. `id` names the
/// import; `deleting` picks the message for a non-draft.
pub(crate) fn draft_error(error: DraftError, id: ImportId, deleting: bool) -> ImportError {
    match error {
        DraftError::Missing => not_found(id.0),
        DraftError::Stale(revision) => {
            ImportError::conflict("stale", messages::IMPORT_STALE).with("revision", json!(revision))
        }
        DraftError::NotDraft(status) => ImportError::conflict(
            "notDraft",
            if deleting {
                messages::IMPORT_DELETE_NOT_DRAFT
            } else {
                messages::IMPORT_PLAN_FROZEN
            },
        )
        .with("status", json!(status.as_str())),
        DraftError::V1Plan => ImportError::conflict("v1Plan", messages::IMPORT_V1_PLAN),
        DraftError::TypeExists(label) => {
            ImportError::conflict("typeExists", messages::import_type_exists(&label))
                .with("label", json!(label))
        }
        DraftError::Plan(error) => {
            ImportError::BadRequest(messages::import_plan_refused(error.to_string()))
        }
        DraftError::Db(error) => error.into(),
    }
}

pub(crate) fn not_found(id: impl std::fmt::Display) -> ImportError {
    ImportError::NotFound(messages::import_not_found(id))
}

/// The caller, refused unless signed in with one of the two administrator
/// roles, which may write items, item sets, templates and vocabularies.
pub(crate) async fn admin(
    state: &AppState,
    uri: &Uri,
    headers: &HeaderMap,
) -> Result<Actor, ImportError> {
    let actor = crate::actor_for(state, uri, headers)
        .await
        .map_err(ImportError::Api)?;
    if actor.user().is_none() {
        return Err(ImportError::Unauthenticated);
    }
    if !actor.is_admin() {
        return Err(ImportError::Forbidden);
    }
    Ok(actor)
}

/// The owners `actor` may name in a plan.
pub(crate) fn owners(actor: &Actor) -> Owners {
    Owners::of(actor.role(), actor.user())
}

/// Refuse a body that is not JSON, then read it.
pub(crate) fn body<T: DeserializeOwned>(
    headers: &HeaderMap,
    bytes: &[u8],
) -> Result<T, ImportError> {
    if !crate::content_type_is_json(headers) {
        return Err(ImportError::Api(
            omeka_api::write::body::content_type_error(crate::media_type(headers)),
        ));
    }
    serde_json::from_slice(bytes)
        .map_err(|error| ImportError::BadRequest(messages::import_request_unreadable(error)))
}

pub(crate) fn parse_id(raw: &str) -> Result<ImportId, ImportError> {
    raw.parse().map(ImportId).map_err(|_| not_found(raw))
}

/// The import `raw` names, read after the dead have been recorded, so its
/// status is the one the wire shows.
pub(crate) async fn load(state: &AppState, raw: &str) -> Result<ImportRow, ImportError> {
    let id = parse_id(raw)?;
    with_db!(&state.pool, |db| ImportRepo::new(db).settle_dead().await)?;
    with_db!(&state.pool, |db| ImportRepo::new(db).get(id).await)?.ok_or_else(|| not_found(id.0))
}

/// One import as the routes answer it. `plan` and `ranPlan` are left out of
/// the history list. Four stored summary members are not the run's report:
/// the undo job's report `undo`, shown as `undoReport`; the time a reader
/// found the job dead, shown as `died`; how often the server resumed the run
/// by itself, shown as `resumed` once it did; and the worker's heartbeat, not
/// shown.
/// A stop request is the row's own `stopRequested`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ImportView {
    id: u64,
    owner_id: Option<u64>,
    status: &'static str,
    revision: u64,
    sources: Value,
    created: String,
    finished: Option<String>,
    summary: Option<Value>,
    undo_report: Value,
    died: Option<String>,
    #[serde(skip_serializing_if = "is_zero")]
    resumed: u64,
    stop_requested: bool,
    job_id: Option<u64>,
    copied_from: Option<u64>,
    /// Records done, records total and seconds left, while the run writes
    /// its progress.
    #[serde(skip_serializing_if = "Option::is_none")]
    progress: Option<super::progress::Progress>,
    #[serde(skip_serializing_if = "Option::is_none")]
    plan: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ran_plan: Option<Value>,
}

impl ImportView {
    pub(crate) fn of(row: ImportRow, with_plan: bool) -> Self {
        Self::running_since(row, with_plan, None)
    }

    /// The view with seconds left in its progress, counted from `started`,
    /// the start of the job running the import, in Unix seconds.
    pub(crate) fn running_since(row: ImportRow, with_plan: bool, started: Option<i64>) -> Self {
        let progress =
            super::progress::progress(row.summary.as_ref(), started, super::progress::now());
        let split = split_summary(row.summary);
        Self {
            progress,
            id: row.id.0,
            owner_id: row.owner_id.map(|owner| owner.0),
            status: row.status.as_str(),
            revision: row.revision,
            sources: row.sources,
            created: row.created.format(ISO).to_string(),
            finished: row.finished.map(|time| time.format(ISO).to_string()),
            summary: split.report,
            undo_report: split.undo,
            died: split.died,
            resumed: split.resumed,
            stop_requested: row.stop_requested,
            job_id: row.job_id.map(|job| job.0),
            copied_from: row.copied_from.map(|from| from.0),
            plan: with_plan.then(|| as_v3(row.plan)),
            ran_plan: if with_plan {
                row.ran_plan.map(as_v3)
            } else {
                None
            },
        }
    }
}

/// A stored plan as the admin reads it: version 3, a v2 plan lifted. A plan
/// the boundary refuses, such as a v1 plan, goes out as stored, so the admin
/// can say why it cannot open it.
fn as_v3(stored: Value) -> Value {
    omeka_api::import::plan::ImportPlan::from_json(&stored.to_string())
        .ok()
        .and_then(|plan| serde_json::from_str(&plan.to_json()).ok())
        .unwrap_or(stored)
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_zero(count: &u64) -> bool {
    *count == 0
}

struct Split {
    report: Option<Value>,
    undo: Value,
    died: Option<String>,
    resumed: u64,
}

fn split_summary(summary: Option<Value>) -> Split {
    match summary {
        Some(Value::Object(mut members)) => {
            members.remove(HEARTBEAT);
            let undo = members.remove("undo").unwrap_or(Value::Null);
            let resumed = members
                .remove(RESUMED)
                .and_then(|resumed| resumed.as_u64())
                .unwrap_or(0);
            let died = members.remove(DIED).and_then(|died| match died {
                Value::String(stored) => Some(format!("{}+00:00", stored.replacen(' ', "T", 1))),
                _ => None,
            });
            Split {
                report: (!members.is_empty()).then_some(Value::Object(members)),
                undo,
                died,
                resumed,
            }
        }
        report => Split {
            report,
            undo: Value::Null,
            died: None,
            resumed: 0,
        },
    }
}

/// Stored times are UTC; the wire writes them the way upstream's JSON-LD dates read.
const ISO: &str = "%Y-%m-%dT%H:%M:%S+00:00";

pub(crate) fn json<T: Serialize>(status: StatusCode, value: &T) -> Response {
    match serde_json::to_vec(value) {
        Ok(body) => Response::builder()
            .status(status)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body))
            .unwrap_or_else(|_| crate::empty_internal_error()),
        Err(error) => ImportError::Internal(error.to_string()).into_response(),
    }
}

/// Work on the blocking pool: walking, reading sheets and deriving.
pub(crate) async fn blocking<T, F>(task: F) -> Result<T, ImportError>
where
    F: FnOnce() -> Result<T, ImportError> + Send + 'static,
    T: Send + 'static,
{
    tokio::task::spawn_blocking(task)
        .await
        .map_err(|error| ImportError::Internal(error.to_string()))?
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::split_summary;

    #[test]
    fn the_report_keeps_its_members_without_the_internal_ones() {
        let split = split_summary(Some(json!({
            "heartbeat": "2026-10-19 12:00:00",
            "items": 2,
            "undo": { "items": 2 }
        })));
        assert_eq!(split.report, Some(json!({ "items": 2 })));
        assert_eq!(split.undo, json!({ "items": 2 }));
        let only_heartbeat = split_summary(Some(json!({ "heartbeat": "2026-10-19 12:00:00" })));
        assert_eq!(
            only_heartbeat.report, None,
            "the admin reads null as no report yet"
        );
    }
}
