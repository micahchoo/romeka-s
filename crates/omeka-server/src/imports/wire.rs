//! The import routes' boundary: who may call them, the error answers, and the
//! JSON an import is shown as.

use axum::body::Body;
use axum::http::{HeaderMap, StatusCode, Uri, header};
use axum::response::Response;
use omeka_api::auth::Actor;
use omeka_api::errors::{ApiError, ErrorDocument};
use omeka_api::import::archive::ArchiveError;
use omeka_db::DbError;
use omeka_db::repos::{DIED, HEARTBEAT};
use omeka_db::rows::ImportRow;
use serde::Serialize;
use serde_json::Value;

use crate::AppState;

/// A failure of an import route, with the status it answers.
#[derive(Debug)]
pub(crate) enum ImportError {
    Unauthenticated,
    Forbidden,
    BadRequest(String),
    NotFound(String),
    Conflict(String),
    Internal(String),
    Api(ApiError),
}

impl ImportError {
    pub(crate) fn into_response(self) -> Response {
        let (status, message) = match self {
            Self::Api(error) => return crate::error_response(error),
            Self::Unauthenticated => (
                StatusCode::UNAUTHORIZED,
                omeka_api::messages::AUTHENTICATION_REQUIRED.to_string(),
            ),
            Self::Forbidden => (
                StatusCode::FORBIDDEN,
                "Only a global or site administrator may import an archive.".to_string(),
            ),
            Self::BadRequest(message) => (StatusCode::BAD_REQUEST, message),
            Self::NotFound(message) => (StatusCode::NOT_FOUND, message),
            Self::Conflict(message) => (StatusCode::CONFLICT, message),
            Self::Internal(message) => (StatusCode::INTERNAL_SERVER_ERROR, message),
        };
        crate::document_response(status, &ErrorDocument::general(message))
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
            ArchiveError::Outside(_) => Self::BadRequest(error.to_string()),
            ArchiveError::NotFound(_) | ArchiveError::NotAFolder(_) | ArchiveError::NotAFile(_) => {
                Self::NotFound(error.to_string())
            }
            ArchiveError::NoRoot(_) | ArchiveError::Io(_) => Self::Internal(error.to_string()),
        }
    }
}

/// The caller, refused unless signed in with a role that may write items,
/// item sets, templates and vocabularies: the two administrator roles.
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

/// Refuse a body that is not JSON.
pub(crate) fn require_json(headers: &HeaderMap) -> Result<(), ImportError> {
    if crate::content_type_is_json(headers) {
        Ok(())
    } else {
        Err(ImportError::Api(
            omeka_api::write::body::content_type_error(crate::media_type(headers)),
        ))
    }
}

/// One import as the routes answer it, read after the dead were recorded;
/// `plan` is left out of the history list. Three stored summary members are
/// not the run's report: the undo job's report `undo`, shown as `undoReport`
/// (null until an undo ends); the time a reader found the job dead, shown as
/// `died` (null unless it died); and the worker's heartbeat, not shown.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ImportView {
    id: u64,
    folder: String,
    status: &'static str,
    undoable: bool,
    created: String,
    finished: Option<String>,
    summary: Option<Value>,
    undo_report: Value,
    died: Option<String>,
    job_id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    plan: Option<Value>,
}

impl ImportView {
    pub(crate) fn of(row: ImportRow, with_plan: bool) -> Self {
        let split = split_summary(row.summary);
        Self {
            id: row.id.0,
            folder: row.folder,
            status: row.status.as_str(),
            undoable: row.undoable,
            created: row.created.format(ISO).to_string(),
            finished: row.finished.map(|time| time.format(ISO).to_string()),
            summary: split.report,
            undo_report: split.undo,
            died: split.died,
            job_id: row.job_id.map(|job| job.0),
            plan: with_plan.then_some(row.plan),
        }
    }
}

struct Split {
    report: Option<Value>,
    undo: Value,
    died: Option<String>,
}

fn split_summary(summary: Option<Value>) -> Split {
    match summary {
        Some(Value::Object(mut members)) => {
            members.remove(HEARTBEAT);
            let undo = members.remove("undo").unwrap_or(Value::Null);
            let died = members.remove(DIED).and_then(|died| match died {
                Value::String(stored) => Some(stored_to_iso(&stored)),
                _ => None,
            });
            Split {
                report: (!members.is_empty()).then_some(Value::Object(members)),
                undo,
                died,
            }
        }
        report => Split {
            report,
            undo: Value::Null,
            died: None,
        },
    }
}

/// A time stored in a JSON member (`YYYY-MM-DD HH:MM:SS`, UTC) as the wire
/// writes the columns.
fn stored_to_iso(stored: &str) -> String {
    format!("{}+00:00", stored.replacen(' ', "T", 1))
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

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::split_summary;

    #[test]
    fn a_summary_with_only_the_heartbeat_is_no_report() {
        let split = split_summary(Some(json!({ "heartbeat": "2026-10-19 12:00:00" })));
        assert_eq!(split.report, None, "the admin parses null as no report yet");
    }

    #[test]
    fn the_report_keeps_its_members_without_the_internal_ones() {
        let split = split_summary(Some(json!({
            "heartbeat": "2026-10-19 12:00:00",
            "items": 2,
            "undo": { "items": 2 }
        })));
        assert_eq!(split.report, Some(json!({ "items": 2 })));
        assert_eq!(split.undo, json!({ "items": 2 }));
    }
}
