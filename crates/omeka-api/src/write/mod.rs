//! The write half of the API: `POST`, `PUT` and `DELETE` over the same
//! resources the read layer serves.
//!
//! `ApiController::create` and `::update` hand the decoded body to
//! `Api\Manager`, which checks general adapter access, hydrates the entity
//! through the adapter's `hydrate`, validates it, and answers with the same
//! representation a read would return. This module mirrors that order at the
//! boundary: parse the body first, check access, hydrate and persist, then
//! answer through [`crate::read::Api::read_member`] so the create and update
//! responses are byte-identical to a read of the row just written.
//!
//! `DELETE` shares the boundary shape: parse the id at the route, check
//! adapter access, find the row, check the entity privilege, remove it, and
//! answer with the removed row's representation (`Api\Manager::finalize`
//! renders the entity the adapter returns). `PATCH` is routed as a partial
//! update by the server.

pub mod access;
pub mod assets;
pub mod body;
pub mod detached;
pub mod files;
pub mod fulltext;
pub mod item_sets;
pub mod items;
pub mod jobs;
pub mod media;
pub mod multipart;
pub mod navigation;
pub mod resource;
pub mod site_defaults;
pub mod site_pages;
pub mod sites;
pub mod templates;
pub mod users;
pub mod values;
pub mod vocab;
pub mod vocab_member;

use crate::errors::ApiError;
use crate::read::{Api, ApiResponse};
use crate::routes::Operation;
use crate::write::multipart::UploadedFile;

/// Dispatch a create to the resource handler.
///
/// `file` carries the upload of a multipart `POST`, which only `assets`
/// reads. Every other create ignores it. `jobs` still has no arm: a job's
/// required `class` comes from the dispatcher, not a body, so it stays a 501;
/// see [`jobs`].
pub async fn create(
    api: &Api<'_>,
    resource: &str,
    payload: &body::Payload,
    file: Option<UploadedFile>,
) -> Result<ApiResponse, ApiError> {
    match resource {
        "items" => items::create(api, payload).await,
        "item_sets" => item_sets::create(api, payload).await,
        "media" => media::create(api, payload).await,
        "sites" => sites::create(api, payload).await,
        "site_pages" => site_pages::create(api, payload).await,
        "users" => users::create(api, payload).await,
        "vocabularies" => vocab::create_vocabulary(api, payload).await,
        "resource_classes" => vocab::create_class(api, payload).await,
        "properties" => vocab::create_property(api, payload).await,
        "resource_templates" => templates::create(api, payload).await,
        "assets" => assets::create(api, payload, file).await,
        other => Err(not_implemented(other, Operation::Create)),
    }
}

/// Dispatch an update to the resource handler.
pub async fn update(
    api: &Api<'_>,
    resource: &str,
    raw_id: &str,
    payload: &body::Payload,
    partial: bool,
) -> Result<ApiResponse, ApiError> {
    match resource {
        "items" => items::update(api, raw_id, payload, partial).await,
        "item_sets" => item_sets::update(api, raw_id, payload, partial).await,
        "media" => media::update(api, raw_id, payload, partial).await,
        "sites" => sites::update(api, raw_id, payload, partial).await,
        "site_pages" => site_pages::update(api, raw_id, payload, partial).await,
        "users" => users::update(api, raw_id, payload, partial).await,
        "vocabularies" => vocab::update_vocabulary(api, raw_id, payload, partial).await,
        "resource_classes" => vocab::update_class(api, raw_id, payload, partial).await,
        "properties" => vocab::update_property(api, raw_id, payload, partial).await,
        "resource_templates" => templates::update(api, raw_id, payload, partial).await,
        "assets" => assets::update(api, raw_id, payload, partial).await,
        "jobs" => jobs::update(api, raw_id, payload, partial).await,
        other => Err(not_implemented(other, Operation::Update)),
    }
}

/// Dispatch a delete to the resource handler, then detach the document.
///
/// The adapter-level grant is checked by [`crate::read::Api::delete`] before
/// this runs, as `Api\Manager::execute` checks it before the adapter's own
/// `delete`. Every handler answers the row's live document (the values and
/// pages are gone after the removal), so [`detached`] rewrites it into the
/// entity `Api\Manager::finalize` renders once the flush has detached it.
pub async fn delete(api: &Api<'_>, resource: &str, raw_id: &str) -> Result<ApiResponse, ApiError> {
    let answer = match resource {
        "items" => items::delete(api, raw_id).await,
        "item_sets" => item_sets::delete(api, raw_id).await,
        "media" => media::delete(api, raw_id).await,
        "sites" => sites::delete(api, raw_id).await,
        "site_pages" => site_pages::delete(api, raw_id).await,
        "users" => users::delete(api, raw_id).await,
        "vocabularies" => vocab::delete_vocabulary(api, raw_id).await,
        "resource_classes" => vocab::delete_class(api, raw_id).await,
        "properties" => vocab::delete_property(api, raw_id).await,
        "resource_templates" => templates::delete(api, raw_id).await,
        "assets" => assets::delete(api, raw_id).await,
        "jobs" => jobs::delete(api, raw_id).await,
        other => Err(not_implemented(other, Operation::Delete)),
    }?;
    detached::detach(answer, resource, api.base())
}

fn not_implemented(resource: &str, operation: Operation) -> ApiError {
    ApiError::NotImplemented(crate::errors::NotImplemented::adapter(resource, operation))
}
