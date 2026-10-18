//! `assets`, `jobs` and `modules`, plus the two row queries `site_pages` and
//! `value_annotations` need for their listings.
//!
//! `omeka-db` keeps a row struct for every one of these tables but no
//! repository walks them, and the readgaps brief forbids editing that crate.
//! `DESIGN.md` says SQL strings live in `omeka-db` only, so the two rules
//! cannot both hold; the brief wins and the queries sit here, one per table.
//! Every row still parses through the `omeka_db::rows` struct for its table, so
//! the boundary is unchanged: raw columns in, typed rows out. Reported as a
//! deviation from `DESIGN.md`.
//!
//! The representations are `AssetRepresentation`, `JobRepresentation` and
//! `ModuleRepresentation`. `ModuleRepresentation` reads `o:ini` from the
//! module's `config/module.ini` on disk, which the port does not have, so
//! `o:ini` carries the two keys the `module` table holds.

use std::cmp::Ordering;

use omeka_db::ids::{AssetId, JobId, UserId};
use omeka_db::rows::{AssetRow, JobRow, ModuleRow, ResourceRow, SitePageRow, resource_type};
use sqlx::SqlitePool;

use super::params::{Params, Sort, Window, ordered_page};
use super::response::link_header;
use super::{Api, db_error, not_found, parse_id, resource_ref_opt};
use crate::errors::ApiError;
use crate::read::response::{ApiResponse, serialize};
use crate::wire::JsonLdType;

// ----------------------------------------------------------------- assets

const ASSET_CLASS: &str = "Omeka\\Entity\\Asset";

pub async fn search_assets(api: &Api<'_>, params: &Params) -> Result<ApiResponse, ApiError> {
    let window = Window::from_params(params);
    let rows = asset_rows(api.pool()).await?;
    let rows: Vec<AssetRow> = rows
        .into_iter()
        .filter(|row| owner_matches(row.owner_id, params))
        .collect();
    let sort_by = params.get("sort_by").unwrap_or("id");
    let (total, page) = ordered_page(
        rows,
        Sort::from_params(params).descending,
        &window,
        cmp_asset(sort_by),
    );
    let documents: Vec<_> = page.iter().map(|row| asset_doc(api, row)).collect();
    let body = serialize(&documents)?;
    Ok(ApiResponse::listing(
        body,
        total,
        link_header(api.base(), "assets", params, &window, total),
    ))
}

pub async fn read_asset(api: &Api<'_>, raw_id: &str) -> Result<ApiResponse, ApiError> {
    let id = parse_id(ASSET_CLASS, raw_id)?;
    let Some(row) = asset_row(api.pool(), AssetId(id)).await? else {
        return Err(not_found(ASSET_CLASS, raw_id));
    };
    Ok(ApiResponse::document(serialize(&asset_doc(api, &row))?))
}

/// `AssetRepresentation::getJsonLd`.
fn asset_doc(api: &Api<'_>, row: &AssetRow) -> crate::wire::asset::Asset {
    let base = api.base();
    let filename = asset_filename(row);
    crate::wire::asset::Asset {
        context: base.api_context(),
        id: base.api_resource("assets", row.id.0),
        jsonld_type: JsonLdType::single("o:Asset"),
        o_id: row.id.0,
        owner: resource_ref_opt(base, "users", row.owner_id.map(|id| id.0)),
        name: row.name.clone(),
        asset_url: base.file_url(&format!("asset/{filename}")),
        filename,
        media_type: row.media_type.clone(),
        alt_text: row.alt_text.clone(),
    }
}

/// `Entity\Asset::getFilename`: the storage id, plus the extension when there
/// is one.
fn asset_filename(row: &AssetRow) -> String {
    match row.extension.as_deref() {
        Some(extension) if !extension.is_empty() => format!("{}.{extension}", row.storage_id),
        _ => row.storage_id.clone(),
    }
}

/// `AssetAdapter::buildQuery`: `owner_id=0` selects assets with no owner, any
/// other number selects that owner, and anything else is ignored.
fn owner_matches(owner: Option<UserId>, params: &Params) -> bool {
    match params.int("owner_id") {
        Some(0) => owner.is_none(),
        Some(id) => owner.map(|row| row.0 as i64 == id).unwrap_or(false),
        None => true,
    }
}

/// The `AssetAdapter::$sortFields` map, with the `id` tiebreak
/// `AbstractEntityAdapter::search` always appends. An unmapped field falls back
/// to `id`.
fn cmp_asset(field: &str) -> impl Fn(&AssetRow, &AssetRow) -> Ordering + '_ {
    move |a, b| match field {
        "media_type" => a.media_type.cmp(&b.media_type).then(a.id.cmp(&b.id)),
        "name" => a.name.cmp(&b.name).then(a.id.cmp(&b.id)),
        "extension" => a.extension.cmp(&b.extension).then(a.id.cmp(&b.id)),
        _ => a.id.cmp(&b.id),
    }
}

async fn asset_rows(pool: &SqlitePool) -> Result<Vec<AssetRow>, ApiError> {
    let rows = sqlx::query(
        "SELECT id, owner_id, name, media_type, storage_id, extension, alt_text FROM asset",
    )
    .fetch_all(pool)
    .await
    .map_err(ApiError::internal)?;
    rows.iter()
        .map(AssetRow::from_row)
        .collect::<Result<_, _>>()
        .map_err(db_error)
}

async fn asset_row(pool: &SqlitePool, id: AssetId) -> Result<Option<AssetRow>, ApiError> {
    let row = sqlx::query(
        "SELECT id, owner_id, name, media_type, storage_id, extension, alt_text \
         FROM asset WHERE id = ?",
    )
    .bind(id.0 as i64)
    .fetch_optional(pool)
    .await
    .map_err(ApiError::internal)?;
    row.map(|row| AssetRow::from_row(&row))
        .transpose()
        .map_err(db_error)
}

// ------------------------------------------------------------------- jobs

const JOB_CLASS: &str = "Omeka\\Entity\\Job";

pub async fn search_jobs(api: &Api<'_>, params: &Params) -> Result<ApiResponse, ApiError> {
    let window = Window::from_params(params);
    let rows = job_rows(api.pool()).await?;
    let rows: Vec<JobRow> = rows
        .into_iter()
        .filter(|row| class_matches(&row.class, params))
        .filter(|row| status_matches(row.status.as_deref(), params))
        .collect();
    let sort_by = params.get("sort_by").unwrap_or("id");
    let (total, page) = ordered_page(
        rows,
        Sort::from_params(params).descending,
        &window,
        cmp_job(sort_by),
    );
    let documents: Vec<_> = page.iter().map(|row| job_doc(api, row)).collect();
    let body = serialize(&documents)?;
    Ok(ApiResponse::listing(
        body,
        total,
        link_header(api.base(), "jobs", params, &window, total),
    ))
}

pub async fn read_job(api: &Api<'_>, raw_id: &str) -> Result<ApiResponse, ApiError> {
    let id = parse_id(JOB_CLASS, raw_id)?;
    let Some(row) = job_row(api.pool(), JobId(id)).await? else {
        return Err(not_found(JOB_CLASS, raw_id));
    };
    Ok(ApiResponse::document(serialize(&job_doc(api, &row))?))
}

/// `JobRepresentation::getJsonLd`.
fn job_doc(api: &Api<'_>, row: &JobRow) -> crate::wire::job::Job {
    let base = api.base();
    crate::wire::job::Job {
        context: base.api_context(),
        id: base.api_resource("jobs", row.id.0),
        jsonld_type: JsonLdType::single("o:Job"),
        o_id: row.id.0,
        status: row.status.clone(),
        job_class: row.class.clone(),
        args: row.args.clone().unwrap_or(serde_json::Value::Null),
        owner: resource_ref_opt(base, "users", row.owner_id.map(|id| id.0)),
        started: Some(super::jsonld_date(row.started)),
        ended: row.ended.map(super::jsonld_date),
    }
}

/// `JobAdapter::buildQuery`'s `class` filter: an exact match, ignored when the
/// caller sends nothing.
fn class_matches(class: &str, params: &Params) -> bool {
    params.get("class").is_none_or(|want| want == class)
}

/// `JobAdapter::buildQuery`'s `status` filter.
fn status_matches(status: Option<&str>, params: &Params) -> bool {
    params.get("status").is_none_or(|want| status == Some(want))
}

/// The `JobAdapter::$sortFields` map, with the `id` tiebreak
/// `AbstractEntityAdapter::search` always appends. `owner_email` sorts through
/// the owner's email upstream; the port keeps `id`, which is the fallback
/// `AbstractEntityAdapter::sortQuery` uses for an unmapped field.
fn cmp_job(field: &str) -> impl Fn(&JobRow, &JobRow) -> Ordering + '_ {
    move |a, b| match field {
        "status" => a.status.cmp(&b.status).then(a.id.cmp(&b.id)),
        "class" => a.class.cmp(&b.class).then(a.id.cmp(&b.id)),
        "started" => a.started.cmp(&b.started).then(a.id.cmp(&b.id)),
        "ended" => a.ended.cmp(&b.ended).then(a.id.cmp(&b.id)),
        _ => a.id.cmp(&b.id),
    }
}

async fn job_rows(pool: &SqlitePool) -> Result<Vec<JobRow>, ApiError> {
    let rows =
        sqlx::query("SELECT id, owner_id, pid, status, class, args, log, started, ended FROM job")
            .fetch_all(pool)
            .await
            .map_err(ApiError::internal)?;
    rows.iter()
        .map(JobRow::from_row)
        .collect::<Result<_, _>>()
        .map_err(db_error)
}

async fn job_row(pool: &SqlitePool, id: JobId) -> Result<Option<JobRow>, ApiError> {
    let row = sqlx::query(
        "SELECT id, owner_id, pid, status, class, args, log, started, ended \
         FROM job WHERE id = ?",
    )
    .bind(id.0 as i64)
    .fetch_optional(pool)
    .await
    .map_err(ApiError::internal)?;
    row.map(|row| JobRow::from_row(&row))
        .transpose()
        .map_err(db_error)
}

// ---------------------------------------------------------------- modules

pub async fn search_modules(api: &Api<'_>, params: &Params) -> Result<ApiResponse, ApiError> {
    let window = Window::from_params(params);
    let rows = module_rows(api.pool()).await?;
    let (total, page) = ordered_page(rows, false, &window, |a, b| a.id.cmp(&b.id));
    let documents: Vec<_> = page.iter().map(|row| module_doc(api, row)).collect();
    let body = serialize(&documents)?;
    Ok(ApiResponse::listing(
        body,
        total,
        link_header(api.base(), "modules", params, &window, total),
    ))
}

pub async fn read_module(api: &Api<'_>, id: &str) -> Result<ApiResponse, ApiError> {
    let Some(row) = module_row(api.pool(), id).await? else {
        // Upstream `ModuleAdapter::read` returns `false` for an unregistered
        // module, which the response validator turns into a 500. The port
        // answers 404, the same shape every other member read uses.
        return Err(not_found(MODULE_CLASS, id));
    };
    Ok(ApiResponse::document(serialize(&module_doc(api, &row))?))
}

/// `Module\Module`'s entity class, the name the port reports for a missing
/// module.
const MODULE_CLASS: &str = "Omeka\\Entity\\Module";

/// `ModuleRepresentation::getJsonLd`.
///
/// `o:state` is `Module\Manager::STATE_ACTIVE` or `STATE_NOT_ACTIVE`, the two
/// states the `module` table can express: the port scans no module directory,
/// so `not_installed`, `not_found` and the `invalid_*` states cannot occur.
/// `o:ini` carries the `name` and `version` keys `Manager::iniIsValid` requires
/// and the `module` table stores; the remaining INI keys live in a
/// `config/module.ini` the port does not ship.
fn module_doc(api: &Api<'_>, row: &ModuleRow) -> crate::wire::module::Module {
    let base = api.base();
    crate::wire::module::Module {
        context: base.api_context(),
        id: base.api_resource("modules", &row.id),
        jsonld_type: JsonLdType::single("o:Module"),
        o_id: row.id.clone(),
        state: if row.is_active {
            "active"
        } else {
            "not_active"
        }
        .to_string(),
        ini: serde_json::json!({"name": row.id, "version": row.version}),
    }
}

async fn module_rows(pool: &SqlitePool) -> Result<Vec<ModuleRow>, ApiError> {
    let rows = sqlx::query("SELECT id, is_active, version FROM module")
        .fetch_all(pool)
        .await
        .map_err(ApiError::internal)?;
    rows.iter()
        .map(ModuleRow::from_row)
        .collect::<Result<_, _>>()
        .map_err(db_error)
}

async fn module_row(pool: &SqlitePool, id: &str) -> Result<Option<ModuleRow>, ApiError> {
    let row = sqlx::query("SELECT id, is_active, version FROM module WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(ApiError::internal)?;
    row.map(|row| ModuleRow::from_row(&row))
        .transpose()
        .map_err(db_error)
}

// -------------------------------------------------- rows other reads list

/// Every `value_annotation` resource, for `ValueAnnotationAdapter::search`.
pub(crate) async fn annotation_rows(pool: &SqlitePool) -> Result<Vec<ResourceRow>, ApiError> {
    let rows = sqlx::query(
        "SELECT id, owner_id, resource_class_id, resource_template_id, thumbnail_id, \
         title, is_public, created, modified, resource_type FROM resource \
         WHERE resource_type = ?",
    )
    .bind(resource_type::VALUE_ANNOTATION)
    .fetch_all(pool)
    .await
    .map_err(ApiError::internal)?;
    rows.iter()
        .map(ResourceRow::from_row)
        .collect::<Result<_, _>>()
        .map_err(db_error)
}

/// Every `site_page` row, for `SitePageAdapter::search`.
pub(crate) async fn page_rows(pool: &SqlitePool) -> Result<Vec<SitePageRow>, ApiError> {
    let rows = sqlx::query(
        "SELECT id, site_id, slug, title, is_public, layout, layout_data, created, modified \
         FROM site_page",
    )
    .fetch_all(pool)
    .await
    .map_err(ApiError::internal)?;
    rows.iter()
        .map(SitePageRow::from_row)
        .collect::<Result<_, _>>()
        .map_err(db_error)
}
