//! Deleting what an import made, through the API's own delete paths, so a
//! media's files leave the store and the index rows go with the rows.

use std::future::Future;
use std::pin::Pin;

use omeka_db::rows::ImportTarget;

use crate::errors::ApiError;
use crate::read::Api;
use crate::write::{item_sets, items, media, templates, vocab};

/// What a delete found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Removed {
    Removed,
    /// Nothing had the id any more.
    AlreadyGone,
}

/// The deletes undo makes. The API delete paths read through the SQLite-only
/// read layer, so the caller supplies them, as a run's caller supplies
/// [`SiteSync`](crate::import2::run::SiteSync).
pub trait Removal: Send + Sync {
    fn remove<'a>(
        &'a self,
        target: ImportTarget,
    ) -> Pin<Box<dyn Future<Output = Result<Removed, String>> + Send + 'a>>;
}

/// The API's delete paths. `api` must carry the store, or a media's files
/// stay behind.
pub struct ApiRemoval<'a> {
    pub api: &'a Api<'a>,
}

impl Removal for ApiRemoval<'_> {
    fn remove<'a>(
        &'a self,
        target: ImportTarget,
    ) -> Pin<Box<dyn Future<Output = Result<Removed, String>> + Send + 'a>> {
        Box::pin(async move {
            let api = self.api;
            let done = match target {
                ImportTarget::Item(id) => items::delete(api, &id.0.to_string()).await.map(drop),
                ImportTarget::Media(id) => media::delete(api, &id.0.to_string()).await.map(drop),
                ImportTarget::ItemSet(id) => {
                    item_sets::delete(api, &id.0.to_string()).await.map(drop)
                }
                ImportTarget::ResourceTemplate(id) => {
                    templates::delete(api, &id.0.to_string()).await.map(drop)
                }
                ImportTarget::Property(id) => vocab::delete_property(api, &id.0.to_string())
                    .await
                    .map(drop),
            };
            match done {
                Ok(()) => Ok(Removed::Removed),
                Err(ApiError::NotFound { .. }) => Ok(Removed::AlreadyGone),
                Err(error) => Err(error.to_string()),
            }
        })
    }
}
