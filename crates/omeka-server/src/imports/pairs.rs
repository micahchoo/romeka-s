//! The pairs list round trip (`docs/import-v2.md`, "Pairs list round trip"):
//! `GET …/pairs.csv` answers the unpaired rows as a list to fill in, and
//! `POST …/pairs` reads the filled list back as `pair` decisions. The rules
//! live in `omeka_api::import::preview::pairs`; these handlers read the
//! request, derive the import, and save the plan.

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode, Uri, header};
use axum::response::Response;
use omeka_api::import::draft;
use omeka_api::import::preview::pairs::{PairsError, Unmatched, pairs_csv, read_pairs};
use omeka_api::messages;
use omeka_db::with_db;
use serde::Serialize;

use super::answer;
use super::lists::{read_upload, require_draft};
use super::run::{derive, stored_plan};
use super::wire::{ImportError, ImportView, admin, blocking, draft_error, json, load};
use crate::AppState;

/// `GET /api/romeka/imports/{id}/pairs.csv`.
pub(super) async fn list(
    State(state): State<AppState>,
    Path(id): Path<String>,
    uri: Uri,
    headers: HeaderMap,
) -> Response {
    answer(
        async {
            let actor = admin(&state, &uri, &headers).await?;
            let row = load(&state, &id).await?;
            let plan = stored_plan(&row)?;
            let derived = derive(&state, &actor, row.id, plan).await?;
            let id = row.id.0;
            let bytes = blocking(move || Ok(pairs_csv(&derived))).await?;
            Ok(Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "text/csv; charset=utf-8")
                .header(
                    header::CONTENT_DISPOSITION,
                    format!("attachment; filename=\"import-{id}-pairs.csv\""),
                )
                .body(Body::from(bytes))
                .unwrap_or_else(|_| crate::empty_internal_error()))
        }
        .await,
    )
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PairsAdded {
    /// Pair decisions this list added.
    added: usize,
    /// Cells whose pair the plan already held.
    already: usize,
    unmatched: Vec<Unmatched>,
    unmatched_total: usize,
    /// The import after the list, with its new plan and revision.
    import: ImportView,
}

/// `POST /api/romeka/imports/{id}/pairs` (multipart, one file): add one
/// `pair` decision per `Files` cell that names exactly one file.
pub(super) async fn upload(
    State(state): State<AppState>,
    Path(id): Path<String>,
    uri: Uri,
    headers: HeaderMap,
    payload: crate::limits::Payload,
) -> Response {
    answer(
        async {
            let actor = admin(&state, &uri, &headers).await?;
            let row = load(&state, &id).await?;
            require_draft(&row)?;
            let upload = read_upload(&headers, &payload)?;
            let mut plan = stored_plan(&row)?;
            let derived = derive(&state, &actor, row.id, plan.clone()).await?;
            let list = upload.list;
            let read = blocking(move || {
                read_pairs(&derived, &list).map_err(|error| match error {
                    PairsError::NoColumns => {
                        ImportError::BadRequest(messages::IMPORT_PAIRS_COLUMNS.into())
                    }
                })
            })
            .await?;
            let added = read.decisions.len();
            let row = if added == 0 {
                row
            } else {
                plan.decisions.extend(read.decisions);
                let value: serde_json::Value = serde_json::from_str(&plan.to_json())
                    .map_err(|error| ImportError::Internal(error.to_string()))?;
                with_db!(&state.pool, |db| draft::save(
                    db,
                    row.id,
                    &value,
                    row.revision
                )
                .await)
                .map_err(|error| draft_error(error, row.id, false))?;
                load(&state, &id).await?
            };
            Ok(json(
                StatusCode::OK,
                &PairsAdded {
                    added,
                    already: read.already,
                    unmatched: read.unmatched,
                    unmatched_total: read.unmatched_total,
                    import: ImportView::of(row, true),
                },
            ))
        }
        .await,
    )
}
