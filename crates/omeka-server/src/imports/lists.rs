//! The list round trip (`docs/import-v2.md`, "The list round trip"):
//! `GET …/template.csv?columns=&series=` answers the list of one series'
//! items to fill in, and `POST …/sheets?series=` stores a filled list (CSV,
//! TSV, .xlsx, .xls or .ods) in the import's staging area and adds it to
//! that series of the draft's plan, or with `&replace=<sheet id>` puts it
//! in that list's place. A plan of one series may leave `series` out.
//! `POST …/sheets/header` reads a list's header row and stores nothing. The
//! rules live in `omeka_api::import::template`; these handlers read the
//! request, move the bytes through the store, and save the plan.

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode, Uri, header};
use axum::response::Response;
use omeka_api::import::catalog_load;
use omeka_api::import::draft;
use omeka_api::import::plan::{ImportPlan, ReadOptions, RelPath, SeriesIx, SheetId};
use omeka_api::import::preview::staging_path;
use omeka_api::import::sheet;
use omeka_api::import::template::{
    ListColumn, ListError, MAX_LIST_BYTES, add_list, list_read, list_series, replace_list,
    template_csv,
};
use omeka_api::messages;
use omeka_db::rows::{ImportRow, ImportStatus};
use omeka_db::with_db;
use serde_json::json;

use super::answer;
use super::multipart::one_file;
use super::run::{derive, stored_plan};
use super::wire::{ImportError, ImportView, admin, blocking, draft_error, json, load, owners};
use crate::AppState;

/// `GET /api/romeka/imports/{id}/template.csv?columns=<term>,<term>…&series=<level id>`.
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
            let series = series_of(&plan, &uri)?;
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
            let bytes = blocking(move || {
                Ok(template_csv(
                    &derived.model,
                    &derived.catalog,
                    &columns,
                    series,
                ))
            })
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

/// `POST /api/romeka/imports/{id}/sheets?series=<level id>&replace=<sheet id>`
/// (multipart, one file): store a filled list in staging and add it to that
/// series of the draft's plan, or put it in the place of the list `replace`
/// names. Only `replace` makes a replacement: two lists of one template have
/// the same header row, so the bytes cannot say which list they are. Answers the import with its new plan and revision, in
/// the version the plan was stored in.
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
            let (extension, read, bytes, list) =
                (upload.extension, upload.read, upload.bytes, upload.list);
            let mut plan = stored_plan(&row)?;
            let series = series_of(&plan, &uri)?;
            let replace = omeka_api::read::Params::parse(uri.query().unwrap_or(""))
                .get("replace")
                .map(|named| SheetId(named.to_string()));
            let files = omeka_api::settings::file_validation(&state.pool).await?;
            let catalog = with_db!(&state.pool, |db| catalog_load::load(
                db,
                row.id,
                &plan,
                owners(&actor),
                files
            )
            .await)?;

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
                    .put(&bytes, &staging_path(import, &file), None)
                    .map_err(internal)?;
                Ok(file)
            })
            .await?;
            let old = match &replace {
                Some(replace) => {
                    replace_list(&mut plan, series, replace, stored.clone(), read, &list).map(Some)
                }
                None => {
                    add_list(&mut plan, series, stored.clone(), read, &list, &catalog).map(|_| None)
                }
            };
            let saved = match old {
                Ok(old) => {
                    let value: serde_json::Value = serde_json::from_str(&plan.to_json())
                        .map_err(|error| ImportError::Internal(error.to_string()))?;
                    with_db!(&state.pool, |db| draft::save(
                        db,
                        row.id,
                        &value,
                        row.revision
                    )
                    .await)
                    .map(|_| old)
                    .map_err(|error| draft_error(error, row.id, false))
                }
                Err(error) => Err(list_error(error, &plan)),
            };
            match saved {
                Err(error) => {
                    let store = state.store.clone();
                    let path = staging_path(import, &stored);
                    blocking(move || store.delete(&path).map(|_| ()).map_err(internal)).await?;
                    return Err(error);
                }
                Ok(Some(old)) => {
                    // The plan is saved and no longer reads the old file: one
                    // left behind is never read, so failing here would only
                    // report a saved change as failed.
                    let store = state.store.clone();
                    let path = staging_path(import, &old);
                    let _ =
                        blocking(move || store.delete(&path).map(|_| ()).map_err(internal)).await;
                }
                Ok(None) => {}
            }
            let row = load(&state, &id).await?;
            Ok(json(StatusCode::CREATED, &ImportView::of(row, true)))
        }
        .await,
    )
}

/// `POST /api/romeka/imports/{id}/sheets/header` (multipart, one file): the
/// header row of a list, read as `sheets` would read it, and nothing stored.
/// The admin matches a list sent again to the old list's columns before it
/// sends it, and a browser cannot read a workbook.
pub(super) async fn header(
    State(state): State<AppState>,
    Path(id): Path<String>,
    uri: Uri,
    headers: HeaderMap,
    payload: crate::limits::Payload,
) -> Response {
    answer(
        async {
            admin(&state, &uri, &headers).await?;
            load(&state, &id).await?;
            let upload = read_upload(&headers, &payload)?;
            let header: Vec<&str> = upload
                .list
                .columns
                .iter()
                .map(|column| column.header.as_str())
                .collect();
            Ok(json(StatusCode::OK, &json!({ "header": header })))
        }
        .await,
    )
}

/// The series `?series=` names, or the plan's only one.
fn series_of(plan: &ImportPlan, uri: &Uri) -> Result<SeriesIx, ImportError> {
    let params = omeka_api::read::Params::parse(uri.query().unwrap_or(""));
    let named = params.get("series").filter(|named| !named.is_empty());
    list_series(plan, named).map_err(|error| list_error(error, plan))
}

/// A refused list as the admin reads it. A series or `replace` error is
/// coded, with the plan's series ids, so the admin can ask which series or
/// list the upload is for.
fn list_error(error: ListError, plan: &ImportPlan) -> ImportError {
    let ids = || {
        json!(
            plan.structure
                .all_series()
                .map(|(_, series)| series.id().0.clone())
                .collect::<Vec<_>>()
        )
    };
    match error {
        ListError::NoIdentifier => {
            ImportError::BadRequest(messages::IMPORT_LIST_NO_IDENTIFIER.into())
        }
        ListError::UnknownColumn(column) => {
            ImportError::BadRequest(messages::import_list_column_unknown(column))
        }
        ListError::SeriesNeeded => {
            ImportError::refused("seriesNeeded", messages::IMPORT_LIST_SERIES_NEEDED)
                .with("series", ids())
        }
        ListError::UnknownSeries(named) => {
            ImportError::refused("unknownSeries", messages::import_list_series_unknown(named))
                .with("series", ids())
        }
        ListError::UnknownList(named) => {
            ImportError::refused("unknownList", messages::import_list_unknown(named))
                .with("series", ids())
        }
    }
}

/// A list a person sent, read and ready to use.
pub(super) struct Upload {
    pub(super) bytes: axum::body::Bytes,
    /// `csv`, `tsv`, `xlsx`, `xls` or `ods`.
    pub(super) extension: &'static str,
    pub(super) read: ReadOptions,
    pub(super) list: sheet::Sheet,
}

/// Only a draft's plan may change; a started import's plan is its record.
pub(super) fn require_draft(row: &ImportRow) -> Result<(), ImportError> {
    if row.status == ImportStatus::Draft {
        return Ok(());
    }
    Err(
        ImportError::conflict("notDraft", messages::IMPORT_PLAN_FROZEN)
            .with("status", json!(row.status.as_str())),
    )
}

/// The one file of a list upload: its kind from the name, the size limit,
/// and the read. A workbook named so is read as one, never as text by
/// accident.
pub(super) fn read_upload(
    headers: &HeaderMap,
    payload: &crate::limits::Payload,
) -> Result<Upload, ImportError> {
    let (name, bytes) = {
        let (name, part) = one_file(headers, &payload.0)?;
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
    if !matches!(extension, "csv" | "tsv") && sheet::WorkbookKind::sniff(&bytes).is_none() {
        return Err(ImportError::BadRequest(messages::import_list_unread(
            format!("\"{name}\" is not a {extension} workbook"),
        )));
    }
    let read = list_read(tab);
    let list = sheet::read(&bytes, &read, None)
        .map_err(|error| ImportError::BadRequest(messages::import_list_unread(error)))?;
    Ok(Upload {
        bytes,
        extension,
        read,
        list,
    })
}

pub(super) fn internal(error: impl std::fmt::Display) -> ImportError {
    ImportError::Internal(error.to_string())
}
