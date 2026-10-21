//! The kept caption edition, held beside the media files in the store (G6).
//!
//! The service in `omeka_api::annotation` writes and reads one document per
//! media and language; this module is the store seam under it. The documents
//! live under `caption-undo/`, a prefix no media or asset owns, so the proxy's
//! access check (`GET /api/romeka/files/allow`) refuses a path under it and no
//! reader of the annotations can reach it.
//!
//! Every call is blocking store I/O, so it runs on the blocking pool. A write or
//! a delete that fails is logged and costs the undo, never the import.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use omeka_api::annotation::{KeptEdition, KeptEditions, decode, encode, storage_path};
use omeka_db::ids::MediaId;
use omeka_media::store::Store;

/// The kept editions of every media, in the one store the process uses.
pub(crate) struct KeptFiles {
    store: Arc<dyn Store>,
}

impl KeptFiles {
    pub(crate) fn new(store: Arc<dyn Store>) -> Self {
        Self { store }
    }
}

impl KeptEditions for KeptFiles {
    fn kept<'a>(
        &'a self,
        media: MediaId,
        lang: Option<&'a str>,
    ) -> Pin<Box<dyn Future<Output = Option<KeptEdition>> + Send + 'a>> {
        let store = Arc::clone(&self.store);
        let lang = lang.map(str::to_string);
        Box::pin(async move {
            let path = storage_path(media, lang.as_deref());
            let read = tokio::task::spawn_blocking(move || store.read(&path))
                .await
                .map_err(|error| error.to_string())
                .and_then(|read| read.map_err(|error| error.to_string()));
            let bytes = match read {
                Ok(bytes) => bytes,
                Err(error) => {
                    tracing::debug!(error, media = media.0, "no kept caption edition");
                    return None;
                }
            };
            decode(media, lang.as_deref(), &bytes)
        })
    }

    fn keep<'a>(
        &'a self,
        media: MediaId,
        lang: Option<&'a str>,
        edition: &'a KeptEdition,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        let store = Arc::clone(&self.store);
        let path = storage_path(media, edition.tag());
        let bytes = encode(edition);
        let lang = lang.map(str::to_string);
        Box::pin(async move {
            if bytes.is_empty() {
                tracing::warn!(
                    media = media.0,
                    ?lang,
                    "a caption edition could not be written"
                );
                return;
            }
            let written = tokio::task::spawn_blocking(move || {
                store.put(&bytes, &path, Some("application/json"))
            })
            .await;
            if let Ok(Err(error)) = written {
                tracing::warn!(%error, media = media.0, ?lang, "a caption edition was not kept");
            }
        })
    }

    fn forget<'a>(
        &'a self,
        media: MediaId,
        lang: Option<&'a str>,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        let store = Arc::clone(&self.store);
        let lang = lang.map(str::to_string);
        Box::pin(async move {
            let path = storage_path(media, lang.as_deref());
            let removed = tokio::task::spawn_blocking(move || store.delete(&path)).await;
            match removed {
                Ok(Ok(_)) => {}
                Ok(Err(error)) => {
                    tracing::warn!(%error, media = media.0, ?lang, "a caption edition was not dropped");
                }
                Err(error) => {
                    tracing::warn!(%error, media = media.0, ?lang, "a caption edition was not dropped");
                }
            }
        })
    }
}
