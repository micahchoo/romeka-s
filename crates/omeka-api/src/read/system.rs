//! `assets`, `jobs` and `modules`, plus the two row reads `site_pages` and
//! `value_annotations` need for their listings. Each list reads every row
//! through its `omeka_db` repository and filters, sorts and pages in memory,
//! the way the adapters below describe.
//!
//! The representations are `AssetRepresentation`, `JobRepresentation` and
//! `ModuleRepresentation`. `ModuleRepresentation` reads `o:ini` from the
//! module's `config/module.ini` on disk, which the port does not have, so
//! `o:ini` carries the two keys the `module` table holds.

use std::cmp::Ordering;

use omeka_db::DbPool;
use omeka_db::ids::{AssetId, JobId, UserId};
use omeka_db::repos::{
    AssetFilter, AssetRepo, JobFilter, JobRepo, ModuleRepo, Paging, ResourceRepo, SiteRepo,
    compare_opt_text_ci, compare_text_ci,
};
use omeka_db::rows::{AssetRow, JobRow, ModuleRow, ResourceRow, SitePageRow, resource_type};
use omeka_db::with_db;
use omeka_media::store::FileKind;

use super::paging::{Limit, Window, ordered_page};
use super::params::{Params, Sort};
use super::query::{id_filter, id_matches};
use super::response::link_header;
use super::{Api, db_error, not_found, parse_id, resource_ref_opt};
use crate::errors::ApiError;
use crate::read::response::{ApiResponse, serialize};
use crate::wire::JsonLdType;

// ----------------------------------------------------------------- assets

const ASSET_CLASS: &str = "Omeka\\Entity\\Asset";

pub async fn search_assets(api: &Api<'_>, params: &Params) -> Result<ApiResponse, ApiError> {
    let window = Window::from_params(params);
    let limit = Limit::from_params(params);
    let ids = id_filter(params);
    let rows = asset_rows(api.pool()).await?;
    let rows: Vec<AssetRow> = rows
        .into_iter()
        .filter(|row| owner_matches(row.owner_id, params))
        .filter(|row| id_matches(&ids, row.id.0))
        .collect();
    let sort_by = params.get("sort_by").unwrap_or("id");
    let (total, page) = ordered_page(
        rows,
        Sort::from_params(params).descending,
        &limit,
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
        asset_url: api
            .files()
            .url(&format!("asset/{filename}"), FileKind::Asset),
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
        "media_type" => compare_text_ci(&a.media_type, &b.media_type).then(a.id.cmp(&b.id)),
        "name" => compare_text_ci(&a.name, &b.name).then(a.id.cmp(&b.id)),
        "extension" => compare_opt_text_ci(a.extension.as_deref(), b.extension.as_deref())
            .then(a.id.cmp(&b.id)),
        _ => a.id.cmp(&b.id),
    }
}

async fn asset_rows(pool: &DbPool) -> Result<Vec<AssetRow>, ApiError> {
    with_db!(pool, |db| AssetRepo::new(db)
        .list(&AssetFilter::default(), &Paging::default())
        .await)
    .map_err(db_error)
}

async fn asset_row(pool: &DbPool, id: AssetId) -> Result<Option<AssetRow>, ApiError> {
    with_db!(pool, |db| AssetRepo::new(db).get(id).await).map_err(db_error)
}

// ------------------------------------------------------------------- jobs

const JOB_CLASS: &str = "Omeka\\Entity\\Job";

pub async fn search_jobs(api: &Api<'_>, params: &Params) -> Result<ApiResponse, ApiError> {
    let window = Window::from_params(params);
    let limit = Limit::from_params(params);
    let ids = id_filter(params);
    let rows = job_rows(api.pool()).await?;
    let rows: Vec<JobRow> = rows
        .into_iter()
        .filter(|row| class_matches(&row.class, params))
        .filter(|row| status_matches(row.status.as_deref(), params))
        .filter(|row| id_matches(&ids, row.id.0))
        .collect();
    let sort_by = params.get("sort_by").unwrap_or("id");
    let (total, page) = ordered_page(
        rows,
        Sort::from_params(params).descending,
        &limit,
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
        "status" => {
            compare_opt_text_ci(a.status.as_deref(), b.status.as_deref()).then(a.id.cmp(&b.id))
        }
        "class" => compare_text_ci(&a.class, &b.class).then(a.id.cmp(&b.id)),
        "started" => a.started.cmp(&b.started).then(a.id.cmp(&b.id)),
        "ended" => a.ended.cmp(&b.ended).then(a.id.cmp(&b.id)),
        _ => a.id.cmp(&b.id),
    }
}

async fn job_rows(pool: &DbPool) -> Result<Vec<JobRow>, ApiError> {
    with_db!(pool, |db| JobRepo::new(db)
        .list(&JobFilter::default(), &Paging::default())
        .await)
    .map_err(db_error)
}

async fn job_row(pool: &DbPool, id: JobId) -> Result<Option<JobRow>, ApiError> {
    with_db!(pool, |db| JobRepo::new(db).get(id).await).map_err(db_error)
}

// ---------------------------------------------------------------- modules

pub async fn search_modules(api: &Api<'_>, params: &Params) -> Result<ApiResponse, ApiError> {
    let window = Window::from_params(params);
    let rows = module_rows(api.pool()).await?;
    // `ModuleAdapter::search` returns the registered modules and ignores the
    // query, so no window is applied and the plain `Response` it builds carries
    // no total (`ApiController::getList`).
    let (_, page) = ordered_page(rows, false, &Limit::ALL, |a, b| a.id.cmp(&b.id));
    let documents: Vec<_> = page.iter().map(|row| module_doc(api, row)).collect();
    let body = serialize(&documents)?;
    Ok(ApiResponse::collection(
        body,
        link_header(api.base(), "modules", params, &window, 0),
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

async fn module_rows(pool: &DbPool) -> Result<Vec<ModuleRow>, ApiError> {
    with_db!(pool, |db| ModuleRepo::new(db).list().await).map_err(db_error)
}

async fn module_row(pool: &DbPool, id: &str) -> Result<Option<ModuleRow>, ApiError> {
    with_db!(pool, |db| ModuleRepo::new(db).get(id).await).map_err(db_error)
}

// -------------------------------------------------- rows other reads list

/// Every `value_annotation` resource, for `ValueAnnotationAdapter::search`.
pub(crate) async fn annotation_rows(pool: &DbPool) -> Result<Vec<ResourceRow>, ApiError> {
    with_db!(pool, |db| ResourceRepo::new(db)
        .list_of_type(resource_type::VALUE_ANNOTATION)
        .await)
    .map_err(db_error)
}

/// Every `site_page` row, for `SitePageAdapter::search`.
pub(crate) async fn page_rows(pool: &DbPool) -> Result<Vec<SitePageRow>, ApiError> {
    with_db!(pool, |db| SiteRepo::new(db).list_all_pages().await).map_err(db_error)
}
