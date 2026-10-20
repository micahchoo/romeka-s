//! The list round trip (`docs/import-v2.md`, "The list round trip"):
//! `GET …/template.csv?columns=` answers the list to fill in, and
//! `POST …/sheets` stores a filled list (CSV, TSV, .xlsx, .xls or .ods) in
//! the import's staging area and adds it to the draft's plan. The rules live in
//! `omeka_api::import2::template`; these handlers read the request, move
//! the bytes through the store, and save the plan.

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode, Uri, header};
use axum::response::Response;
use omeka_api::import2::catalog_load;
use omeka_api::import2::draft;
use omeka_api::import2::plan::RelPath;
use omeka_api::import2::preview::staging_path;
use omeka_api::import2::sheet;
use omeka_api::import2::template::{
    ListColumn, ListError, MAX_LIST_BYTES, add_list, list_read, template_csv,
};
use omeka_api::messages;
use omeka_db::repos::SettingRepo;
use omeka_db::rows::ImportStatus;
use serde_json::json;

use super::answer;
use super::multipart::one_file;
use super::run::{derive, stored_plan};
use super::wire::{ImportError, ImportView, admin, blocking, draft_error, json, load, owners};
use crate::AppState;

/// `GET /api/romeka/imports/{id}/template.csv?columns=<term>,<term>…`.
pub(super) async fn template(
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
            let params = omeka_api::read::Params::parse(uri.query().unwrap_or(""));
            let columns = params
                .get("columns")
                .unwrap_or_default()
                .split(',')
                .filter(|text| !text.trim().is_empty())
                .map(|text| {
                    ListColumn::parse(text, &derived.catalog).map_err(|_| {
                        ImportError::BadRequest(messages::import_list_column_unknown(text.trim()))
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            let id = row.id.0;
            let bytes =
                blocking(move || Ok(template_csv(&derived.model, &derived.catalog, &columns)))
                    .await?;
            Ok(Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "text/csv; charset=utf-8")
                .header(
                    header::CONTENT_DISPOSITION,
                    format!("attachment; filename=\"import-{id}-list.csv\""),
                )
                .body(Body::from(bytes))
                .unwrap_or_else(|_| crate::empty_internal_error()))
        }
        .await,
    )
}

/// `POST /api/romeka/imports/{id}/sheets` (multipart, one file): store a
/// filled list in staging and add it to the draft's plan. Answers the
/// import with its new plan and revision.
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
            if row.status != ImportStatus::Draft {
                return Err(
                    ImportError::conflict("notDraft", messages::IMPORT_PLAN_FROZEN)
                        .with("status", json!(row.status.as_str())),
                );
            }
            let (name, bytes) = {
                let (name, part) = one_file(&headers, &payload.0)?;
                (name, payload.0.slice_ref(part))
            };
            if bytes.len() > MAX_LIST_BYTES {
                return Err(ImportError::TooLarge(
                    messages::IMPORT_LIST_TOO_LARGE.into(),
                ));
            }
            let extension = name
                .rsplit_once('.')
                .map(|(_, extension)| extension.to_ascii_lowercase());
            let (extension, tab) = match extension.as_deref() {
                Some("csv") => ("csv", false),
                Some("tsv") => ("tsv", true),
                Some("xlsx") => ("xlsx", false),
                Some("xls") => ("xls", false),
                Some("ods") => ("ods", false),
                _ => return Err(ImportError::BadRequest(messages::import_list_type(&name))),
            };
            // A workbook named so is read as one, never as text by accident.
            if !matches!(extension, "csv" | "tsv") && sheet::WorkbookKind::sniff(&bytes).is_none() {
                return Err(ImportError::BadRequest(messages::import_list_unread(
                    format!("\"{name}\" is not a {extension} workbook"),
                )));
            }
            let read = list_read(tab);
            let list = sheet::read(&bytes, &read, None)
                .map_err(|error| ImportError::BadRequest(messages::import_list_unread(error)))?;
            let mut plan = stored_plan(&row)?;
            let files =
                omeka_api::settings::file_validation(&SettingRepo::new(&state.pool)).await?;
            let catalog =
                catalog_load::load(&state.pool, row.id, &plan, owners(&actor), files).await?;

            let store = state.store.clone();
            let import = row.id;
            let stored = blocking(move || {
                let folder = staging_path(import, &RelPath::parse("lists").map_err(internal)?);
                let next = store
                    .list(&folder)
                    .map_err(internal)?
                    .iter()
                    .filter_map(|file| {
                        let name = file.storage_path.rsplit_once('/')?.1;
                        name.split_once('.')?.0.parse::<u64>().ok()
                    })
                    .max()
                    .unwrap_or(0)
                    + 1;
                let file =
                    RelPath::parse(&format!("lists/{next}.{extension}")).map_err(internal)?;
                store
                    .put(&bytes, &staging_path(import, &file))
                    .map_err(internal)?;
                Ok(file)
            })
            .await?;
            let added = add_list(&mut plan, stored.clone(), read, &list, &catalog);
            let saved = match added {
                Ok(_) => {
                    let value: serde_json::Value = serde_json::from_str(&plan.to_json())
                        .map_err(|error| ImportError::Internal(error.to_string()))?;
                    draft::save(&state.pool, row.id, &value, row.revision)
                        .await
                        .map_err(|error| draft_error(error, row.id, false))
                }
                Err(ListError::NoIdentifier) => Err(ImportError::BadRequest(
                    messages::IMPORT_LIST_NO_IDENTIFIER.into(),
                )),
                Err(ListError::UnknownColumn(column)) => Err(ImportError::BadRequest(
                    messages::import_list_column_unknown(column),
                )),
            };
            if let Err(error) = saved {
                let store = state.store.clone();
                let path = staging_path(import, &stored);
                blocking(move || store.delete(&path).map(|_| ()).map_err(internal)).await?;
                return Err(error);
            }
            let row = load(&state, &id).await?;
            Ok(json(StatusCode::CREATED, &ImportView::of(row, true)))
        }
        .await,
    )
}

fn internal(error: impl std::fmt::Display) -> ImportError {
    ImportError::Internal(error.to_string())
}
