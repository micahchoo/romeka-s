//! `GET /s/{site}/media/{id}/download`: the stored original, named by its
//! source and answered as an attachment.
//!
//! Upstream serves every file under `/files` with no `Content-Disposition`, so
//! a "save as" offers the storage id. This route names the file, and it keeps
//! the media's own visibility rule: a caller who may not see the media gets the
//! site's 404, as its page does.
//!
//! A local store's file is answered here, with the type and the name in
//! headers. An object store signs both into its URL, so this route answers a
//! 302 and the bucket sends the attachment.

use std::sync::Arc;

use axum::body::{Body, Bytes};
use axum::http::{StatusCode, header};
use axum::response::Response;
use futures_util::stream;

use omeka_db::repos::MediaRepo;
use omeka_db::with_db;
use omeka_media::store::{Download, FileKind, Store, StoreError};

use crate::AppState;
use crate::site::dispatch::Common;
use crate::site::load::media_display_title;
use crate::site::{Outcome, SiteError};

/// Serve one media's original under its source name.
pub(crate) async fn media(
    state: &AppState,
    common: &Common<'_>,
    id: u64,
) -> Result<Outcome, SiteError> {
    let Some(mut row) = with_db!(common.loader.pool(), |db| MediaRepo::new(db)
        .get(omeka_db::ids::MediaId::from(id))
        .await)?
    else {
        return Err(SiteError::NotFound);
    };
    if !common
        .loader
        .access()
        .sees_resource(row.resource.is_public, row.resource.owner_id)
    {
        return Err(SiteError::NotFound);
    }
    let Some(storage_id) = row.storage_id.as_deref().filter(|_| row.has_original) else {
        return Err(SiteError::NotFound);
    };
    let path = match row.extension.as_deref() {
        Some(extension) => format!("original/{storage_id}.{extension}"),
        None => format!("original/{storage_id}"),
    };
    // A source that names no file falls back to the media's display name plus
    // its extension, not to the storage id a reader cannot read.
    common
        .loader
        .show_public_titles([&mut row.resource])
        .await?;
    let name = media_display_title(&row, common.loader.messages());
    let fallback = match row.extension.as_deref() {
        Some(extension) => format!("{name}.{extension}"),
        None => name,
    };
    let download =
        Download::attachment(row.media_type.as_deref(), row.source.as_deref(), &fallback);
    let url = state.store.download_url(
        &path,
        FileKind::Original.access(state.store.url_ttl()),
        &download,
    );
    // An object store answers a whole URL that carries the signed attachment
    // name; a local store answers a path, and this route reads the bytes.
    if !url.starts_with('/') {
        return Ok(Outcome::Redirect(url));
    }
    let content_type = download.served_type().unwrap_or("application/octet-stream");
    let mut builder = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CONTENT_DISPOSITION, download.disposition())
        .header(header::CACHE_CONTROL, "private, max-age=0, must-revalidate")
        .header(header::X_CONTENT_TYPE_OPTIONS, "nosniff");
    // The row's `o:size` is the stored byte count, so a streamed answer can
    // still state its length and a client shows progress. A row with no size
    // answers chunked.
    if let Some(size) = row.size.and_then(|size| u64::try_from(size).ok()) {
        builder = builder.header(header::CONTENT_LENGTH, size.to_string());
    }
    builder
        .body(local_body(state.store.clone(), path))
        .map(Outcome::File)
        .map_err(|error| {
            SiteError::Storage(StoreError::Remote {
                path: "download".to_string(),
                message: format!("the download answer is invalid: {error}"),
            })
        })
}

/// How much of a local original one read takes. The bytes go out as they are
/// read, so a video is never held whole.
const CHUNK: u64 = 256 * 1024;

/// The local store's file as a body: one bounded read per frame, on a blocking
/// worker, so the async runtime never touches the disk itself.
fn local_body(store: Arc<dyn Store>, path: String) -> Body {
    let chunks = stream::unfold(
        (store, path, 0u64, false),
        |(store, path, offset, done)| async move {
            if done {
                return None;
            }
            let reader = store.clone();
            let read_path = path.clone();
            let read = tokio::task::spawn_blocking(move || {
                reader.read_range(&read_path, offset..offset + CHUNK)
            })
            .await;
            match read {
                Ok(Ok(chunk)) if chunk.is_empty() => None,
                Ok(Ok(chunk)) => {
                    let short = (chunk.len() as u64) < CHUNK;
                    Some((
                        Ok::<Bytes, std::io::Error>(Bytes::from(chunk)),
                        (store, path, offset + CHUNK, short),
                    ))
                }
                Ok(Err(error)) => Some((
                    Err(std::io::Error::other(error.to_string())),
                    (store, path, offset, true),
                )),
                Err(error) => Some((
                    Err(std::io::Error::other(error.to_string())),
                    (store, path, offset, true),
                )),
            }
        },
    );
    Body::from_stream(chunks)
}
