//! An item's files into the store, before its row commits.
//!
//! This is the upload ingester's path without a request: `media.source` is the
//! file name (`Ingester/Upload.php:61`) and `TempFile::mediaIngestFile` stores
//! the original and the thumbnails (`omeka_media::ingest::ingest_file`). The
//! media type is sniffed from the bytes, as upstream's `finfo` does. A bad
//! image keeps its original and gets no thumbnails, as upstream's thumbnailer
//! failure does. Everything here is blocking; the run calls it on the blocking
//! pool.

use chrono::NaiveDateTime;
use omeka_core::ids::{ItemId as CoreItemId, MediaId as CoreMediaId};
use omeka_core::resource::{Media as CoreMedia, Timestamp};
use omeka_db::ids::{ItemId, UserId};
use omeka_db::rows::{NewMedia, NewResource};
use omeka_media::IngesterRegistry;
use omeka_media::data::MediaData;
use omeka_media::file_render::ThumbnailType;
use omeka_media::http::{FetchError, HttpFetch, HttpResponse};
use omeka_media::ingest::{
    HtmlPurifier, IngestEnv, IngestFile, IngestFlags, MediaDraft, ingest_file,
};
use omeka_media::oembed::{Oembed, OembedAllowList};
use omeka_media::store::{FileStore, StorePrefix};
use omeka_media::thumbnailer::ImageThumbnailer;
use omeka_media::validation::FileValidation;

use super::work::FileWork;

const INGESTER: &str = "upload";

/// Store every file of one item, in order, as the media rows they become.
/// Each row's `item_id` is a placeholder: the item is created with the rows.
/// On a failure the files already stored for this item are removed again, and
/// the message names the file.
///
/// # Errors
/// A message for the first file that could not be read, that `validation`
/// refused, or that could not be stored.
pub fn store_all(
    store: &FileStore,
    validation: &FileValidation,
    files: &[FileWork],
    owner: Option<UserId>,
) -> Result<Vec<NewMedia>, String> {
    let mut stored: Vec<NewMedia> = Vec::with_capacity(files.len());
    for (index, file) in files.iter().enumerate() {
        match store_one(store, validation, file, owner, index) {
            Ok(one) => stored.push(one),
            Err((partial, message)) => {
                discard(store, &stored);
                if let Some(partial) = partial {
                    remove_storage(store, &partial.0, partial.1.as_deref());
                }
                return Err(message);
            }
        }
    }
    Ok(stored)
}

/// Remove the bytes of files whose row did not commit. A file already gone is
/// fine; a file that cannot be removed stays, and nothing names it.
pub fn discard(store: &FileStore, stored: &[NewMedia]) {
    for media in stored {
        if let Some(id) = media.storage_id.as_deref() {
            remove_storage(store, id, media.extension.as_deref());
        }
    }
}

fn remove_storage(store: &FileStore, storage_id: &str, extension: Option<&str>) {
    let _ = store.delete(&FileStore::storage_path(
        StorePrefix::Original,
        storage_id,
        extension,
    ));
    for kind in ThumbnailType::ALL {
        let _ = store.delete(&FileStore::storage_path(
            StorePrefix::Thumbnail(kind),
            storage_id,
            Some("jpg"),
        ));
    }
}

type Partial = Option<(String, Option<String>)>;

fn store_one(
    store: &FileStore,
    validation: &FileValidation,
    file: &FileWork,
    owner: Option<UserId>,
    index: usize,
) -> Result<NewMedia, (Partial, String)> {
    let bytes = std::fs::read(&file.absolute)
        .map_err(|error| (None, format!("cannot read {}: {error}", file.path)))?;
    let renderer = IngesterRegistry::with_defaults()
        .get(INGESTER)
        .renderer()
        .to_string();
    let mut draft = MediaDraft::new(CoreMedia::new(
        CoreMediaId::new(0),
        CoreItemId::new(0),
        INGESTER.to_string(),
        renderer.clone(),
        Timestamp::from_naive(NaiveDateTime::default()),
    ));
    draft.row.source = Some(file.source().to_string());
    let purifier = HtmlPurifier;
    let thumbnailer = ImageThumbnailer;
    let oembed = Oembed::new(OembedAllowList::shipped());
    let env = IngestEnv {
        store,
        http: &NoNetwork,
        purifier: &purifier,
        thumbnailer: &thumbnailer,
        oembed: &oembed,
        validation,
    };
    let ingest = IngestFile::new(bytes).with_source_name(file.source().to_string());
    if let Err(error) = ingest_file(&env, &mut draft, ingest, IngestFlags::default()) {
        let partial = draft
            .row
            .storage_id
            .clone()
            .map(|id| (id, draft.row.extension.clone()));
        return Err((partial, format!("cannot store {}: {error}", file.path)));
    }
    let MediaDraft { row, data } = draft;
    Ok(NewMedia {
        resource: NewResource {
            owner_id: owner,
            ..NewResource::default()
        },
        item_id: ItemId(0),
        ingester: INGESTER.to_string(),
        renderer,
        data: match data {
            MediaData::None => None,
            other => Some(other.to_value()),
        },
        source: row.source,
        media_type: row.media_type,
        storage_id: row.storage_id,
        extension: row.extension,
        sha256: row.sha256,
        size: row.size,
        has_original: row.has_original,
        has_thumbnails: row.has_thumbnails,
        position: i64::try_from(index + 1).ok(),
        lang: None,
        alt_text: None,
    })
}

/// The upload path reads a local file and never fetches.
struct NoNetwork;

impl HttpFetch for NoNetwork {
    fn fetch(&self, _url: &str) -> Result<HttpResponse, FetchError> {
        Err(FetchError::new(
            "the archive import reads local files and does not fetch over the network",
        ))
    }
}
