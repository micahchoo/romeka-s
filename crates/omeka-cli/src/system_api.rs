//! The read surface `romeka` uses when it writes on no one's behalf: a job, or an import.
//!
//! Upstream grants a job `view-all` so its searches see private items, because a job may have no
//! owner. `romeka` reads as a global administrator for the same reason.

use std::sync::Arc;

use omeka_media::store::Store;

use omeka_api::auth::Actor;
use omeka_api::errors::ApiError;
use omeka_api::jsonld::BaseUrl;
use omeka_api::read::Api;
use omeka_db::DbPool;
use omeka_db::ids::UserId;

/// The base URL behind every `@id`. These callers write rows and serve no document, so no `@id`
/// built from it leaves the process.
const BASE_URL: &str = "http://localhost";

/// Load the read surface as a global administrator.
///
/// # Errors
/// The read layer's error when the catalogue of vocabularies cannot load.
pub async fn load(pool: &DbPool) -> Result<Api<'_>, ApiError> {
    let base = BaseUrl::parse(BASE_URL).map_err(ApiError::internal)?;
    Api::load(pool, base, Actor::new(UserId(0), "global_admin")).await
}

/// [`load`] with the file store, for a caller whose deletes must remove a media's files.
///
/// # Errors
/// As [`load`].
pub async fn load_with_store(pool: &DbPool, store: Arc<dyn Store>) -> Result<Api<'_>, ApiError> {
    let base = BaseUrl::parse(BASE_URL).map_err(ApiError::internal)?;
    Api::load_with_store(
        pool,
        base,
        Actor::new(UserId(0), "global_admin"),
        Some(store),
    )
    .await
}
