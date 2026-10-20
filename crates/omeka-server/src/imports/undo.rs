//! Undo and its preview (`docs/import-v2.md`, "Lifecycle and the ledger",
//! "Undo details"). The preview runs the undo's own judge over the ledger
//! and writes nothing, so what it answers is what Undo would do now.

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode, Uri};
use axum::response::Response;
use omeka_api::import2::undo::{self, UNDO_JOB, UndoError, may_undo};
use omeka_api::messages;
use omeka_db::repos::{ImportRepo, StartOutcome};
use omeka_db::rows::ImportStatus;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::answer;
use super::run::{busy, compose_job, start_job};
use super::wire::{ImportError, admin, body, json, load};
use crate::AppState;

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct UndoBody {
    #[serde(default)]
    force: Option<Force>,
}

/// Which touched resources an undo restores or deletes anyway.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum Force {
    All(AllWord),
    Some(Vec<Forced>),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum AllWord {
    All,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Forced {
    kind: ForcedKind,
    id: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum ForcedKind {
    Item,
    Media,
    ItemSet,
    ResourceTemplate,
    Property,
}

fn cannot_undo(status: ImportStatus) -> ImportError {
    ImportError::conflict("cannotUndo", messages::IMPORT_CANNOT_UNDO)
        .with("status", json!(status.as_str()))
}

fn deleted(status: ImportStatus) -> ImportError {
    ImportError::conflict("cannotUndo", messages::IMPORT_DELETE_NOT_UNDONE)
        .with("status", json!(status.as_str()))
}

/// `POST /api/romeka/imports/{id}/undo {force?}`: undo an import that is
/// done, partly undone, or whose run or undo failed. An import that deleted
/// is never undone.
pub(super) async fn undo(
    State(state): State<AppState>,
    Path(id): Path<String>,
    uri: Uri,
    headers: HeaderMap,
    payload: crate::limits::Payload,
) -> Response {
    answer(
        async {
            let actor = admin(&state, &uri, &headers).await?;
            let request: UndoBody = if payload.0.is_empty() {
                UndoBody::default()
            } else {
                body(&headers, &payload.0)?
            };
            let row = load(&state, &id).await?;
            match ImportRepo::new(&state.pool).start_undo(row.id).await? {
                StartOutcome::Started => {}
                StartOutcome::Missing => return Err(super::wire::not_found(row.id.0)),
                StartOutcome::Refused(status) => return Err(cannot_undo(status)),
                StartOutcome::Deleted => return Err(deleted(row.status)),
                StartOutcome::Busy(other) => return Err(busy(other)),
                StartOutcome::Changed { .. } => {
                    return Err(ImportError::Internal(
                        "an undo start answered a revision change".to_string(),
                    ));
                }
            }
            let force = request
                .force
                .map(|force| serde_json::to_value(force).unwrap_or(Value::Null));
            let job = compose_job(&state, &actor, UNDO_JOB, row.id, force).await?;
            start_job(&state, job, StatusCode::OK).await
        }
        .await,
    )
}

/// Unknown members are let through: a client without a session sends its
/// API key in the query string (`key_identity`, `key_credential`).
#[derive(Deserialize, Default)]
pub(super) struct PreviewQuery {
    /// `all` previews `force: "all"`.
    #[serde(default)]
    force: Option<AllWord>,
}

/// `GET /api/romeka/imports/{id}/undo/preview[?force=all]`: what an undo
/// would remove, restore, keep with its reasons, and skip, read now and
/// written nowhere.
pub(super) async fn preview(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<PreviewQuery>,
    uri: Uri,
    headers: HeaderMap,
) -> Response {
    answer(
        async {
            admin(&state, &uri, &headers).await?;
            let row = load(&state, &id).await?;
            if !may_undo(row.status) {
                return Err(cannot_undo(row.status));
            }
            let force = match query.force {
                Some(AllWord::All) => undo::Force::All,
                None => undo::Force::Nothing,
            };
            let previewed = undo::preview_undo(&state.pool, row.id, force)
                .await
                .map_err(|error| match error {
                    UndoError::Deleted(_) => deleted(row.status),
                    other => ImportError::Internal(other.to_string()),
                })?;
            Ok(json(StatusCode::OK, &previewed))
        }
        .await,
    )
}
