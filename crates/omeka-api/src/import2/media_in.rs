//! A media record's bytes into the store, before its row commits
//! (`docs/import-v2.md`, "Per record", "Evidence" and "URL sources").
//!
//! A file of at most [`WHOLE_LIMIT`] bytes takes the upload ingester's path:
//! `omeka_media::ingest::ingest_file` sniffs, validates, stores the original
//! and makes the thumbnails. A larger file never sits in memory: it is
//! sniffed from its first [`SNIFF_BYTES`], validated, and streamed into the
//! store through [`Store::put_stream`] while its SHA-256 is computed. It
//! gets no thumbnails, because the image decoder needs the whole file in
//! memory. A URL record runs its ingester with the guarded HTTP client the
//! caller built from `settings::address_policy`.
//!
//! Everything here is blocking; the run calls it on the blocking pool. Each
//! function answers what it stored, so the run removes those bytes when the
//! record's transaction does not commit.

use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

use chrono::NaiveDateTime;
use omeka_core::ids::{ItemId as CoreItemId, MediaId as CoreMediaId};
use omeka_core::resource::{Media as CoreMedia, Timestamp};
use omeka_db::rows::StoredFile;
use omeka_media::IngesterRegistry;
use omeka_media::data::MediaData;
use omeka_media::file_render::ThumbnailType;
use omeka_media::http::{FetchError, HttpFetch, HttpResponse};
use omeka_media::ingest::{
    ErrorStore, HtmlPurifier, IngestEnv, IngestFile, IngestFlags, IngestRequest, MediaDraft,
    ingest_file,
};
use omeka_media::media_type::{detected_media_type, extension_for_media_type};
use omeka_media::oembed::{Oembed, OembedAllowList};
use omeka_media::store::{
    Store, StorePrefix, extension_from_source_name, new_storage_id, storage_path,
};
use omeka_media::thumbnailer::ImageThumbnailer;
use omeka_media::validation::FileValidation;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::evidence::SNIFF_BYTES;

/// The largest file read whole: "No reader loads a whole file over 64 MB
/// into memory".
pub const WHOLE_LIMIT: u64 = 64 * 1024 * 1024;

/// The ingester name a folder file is stored under, as upstream's upload
/// ingester names a file it stores.
pub const FILE_INGESTER: &str = "upload";

/// The media row facts the ingest made, and the stored paths that hold its
/// bytes.
#[derive(Debug, Clone, PartialEq)]
pub struct Arrived {
    pub file: Option<StoredFile>,
    pub ingester: String,
    pub renderer: String,
    pub data: Option<Value>,
    pub source: Option<String>,
    pub stored: Vec<String>,
}

/// Why a media record's bytes did not arrive. The run turns each into a
/// failure with a message key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArrivalError {
    /// The file could not be opened or read.
    Unreadable(String),
    /// The install's file-validation settings, or the ingester, refused it.
    Refused(String),
    /// The store did not take the bytes.
    Store(String),
}

/// Store one folder file. `name` is the file name `media.source` keeps.
///
/// # Errors
/// An [`ArrivalError`]; nothing stays in the store then.
pub fn arrive_file(
    store: &dyn Store,
    validation: &FileValidation,
    path: &Path,
    name: &str,
) -> Result<Arrived, ArrivalError> {
    let unreadable = |error: io::Error| ArrivalError::Unreadable(error.to_string());
    let size = std::fs::metadata(path).map_err(unreadable)?.len();
    if size > WHOLE_LIMIT {
        return stream_file(store, validation, path, name, size);
    }
    let bytes = std::fs::read(path).map_err(unreadable)?;
    let mut draft = draft(FILE_INGESTER, "file");
    draft.row.source = Some(name.to_string());
    let env = Env::new();
    let ingest = IngestFile::new(bytes).with_source_name(name.to_string());
    let outcome = ingest_file(
        &env.with(store, &NoNetwork, validation),
        &mut draft,
        ingest,
        IngestFlags::default(),
    );
    let stored = stored_paths(&draft);
    if let Err(error) = outcome {
        remove(store, &stored);
        return Err(match error {
            omeka_media::ingest::IngestFileError::Refused(_) => {
                ArrivalError::Refused(error.to_string())
            }
            omeka_media::ingest::IngestFileError::Store(error) => {
                ArrivalError::Store(error.to_string())
            }
        });
    }
    Ok(arrived(draft, stored))
}

fn stream_file(
    store: &dyn Store,
    validation: &FileValidation,
    path: &Path,
    name: &str,
    size: u64,
) -> Result<Arrived, ArrivalError> {
    let unreadable = |error: io::Error| ArrivalError::Unreadable(error.to_string());
    let mut head = Vec::with_capacity(SNIFF_BYTES);
    File::open(path)
        .map_err(unreadable)?
        .take(SNIFF_BYTES as u64)
        .read_to_end(&mut head)
        .map_err(unreadable)?;
    let media_type = detected_media_type(&head);
    let extension = extension_from_source_name(name)
        .or_else(|| extension_for_media_type(&media_type).map(str::to_string));
    let refusals = validation.refusals(name, &media_type, extension.as_deref());
    if !refusals.is_empty() {
        let reasons: Vec<String> = refusals.iter().map(ToString::to_string).collect();
        return Err(ArrivalError::Refused(reasons.join(" ")));
    }
    let storage_id = new_storage_id().map_err(|error| ArrivalError::Store(error.to_string()))?;
    let original = storage_path(StorePrefix::Original, &storage_id, extension.as_deref());
    let mut reader = Hashing {
        inner: File::open(path).map_err(unreadable)?,
        hasher: Sha256::new(),
    };
    let written = store
        .put_stream(&mut reader, Some(size), &original)
        .map_err(|error| ArrivalError::Store(error.to_string()))?;
    let sha256 = omeka_media::text::hex_encode(&reader.hasher.finalize());
    Ok(Arrived {
        file: Some(StoredFile {
            storage_id,
            extension,
            media_type: Some(media_type),
            sha256: Some(sha256),
            size: i64::try_from(written).ok(),
            has_thumbnails: false,
        }),
        ingester: FILE_INGESTER.to_string(),
        renderer: "file".to_string(),
        data: None,
        source: Some(name.to_string()),
        stored: vec![original],
    })
}

/// Run a URL ingester (`url`, `html`, `youtube`, `iiif`,
/// `iiif_presentation`, `oembed`) on `source`. `http` is the guarded client.
///
/// # Errors
/// [`ArrivalError::Refused`] with the ingester's messages; nothing stays in
/// the store then.
pub fn arrive_url(
    store: &dyn Store,
    validation: &FileValidation,
    http: &dyn HttpFetch,
    ingester: &str,
    source: &str,
) -> Result<Arrived, ArrivalError> {
    let registry = IngesterRegistry::with_defaults();
    let Some(implementation) = registry.find(ingester) else {
        return Err(ArrivalError::Refused(format!(
            "no media ingester is named \"{ingester}\""
        )));
    };
    let request = IngestRequest::from_value(match ingester {
        "url" => json!({ "o:ingester": ingester, "ingest_url": source }),
        "html" => json!({ "o:ingester": ingester, "html": source }),
        _ => json!({ "o:ingester": ingester, "o:source": source }),
    });
    let mut draft = draft(ingester, implementation.renderer());
    let env = Env::new();
    let mut errors = ErrorStore::default();
    implementation.ingest(
        &mut draft,
        &request,
        &env.with(store, http, validation),
        &mut errors,
    );
    let stored = stored_paths(&draft);
    if errors.has_errors() {
        remove(store, &stored);
        let messages: Vec<&str> = errors
            .errors()
            .iter()
            .map(|error| error.message.as_str())
            .collect();
        return Err(ArrivalError::Refused(messages.join(" ")));
    }
    let mut arrived = arrived(draft, stored);
    if ingester == "html" {
        arrived.data = Some(json!({ "html": omeka_core::purify::purify(source) }));
    }
    Ok(arrived)
}

/// Remove stored bytes whose row did not commit. A path already gone is
/// fine; one that cannot be removed stays, and nothing else names it.
pub fn remove(store: &dyn Store, stored: &[String]) {
    for path in stored {
        let _ = store.delete(path);
    }
}

fn draft(ingester: &str, renderer: &str) -> MediaDraft {
    MediaDraft::new(CoreMedia::new(
        CoreMediaId::new(0),
        CoreItemId::new(0),
        ingester.to_string(),
        renderer.to_string(),
        Timestamp::from_naive(NaiveDateTime::default()),
    ))
}

fn stored_paths(draft: &MediaDraft) -> Vec<String> {
    let Some(id) = draft.row.storage_id.as_deref() else {
        return Vec::new();
    };
    let mut paths = Vec::new();
    if draft.row.has_original {
        paths.push(storage_path(
            StorePrefix::Original,
            id,
            draft.row.extension.as_deref(),
        ));
    }
    if draft.row.has_thumbnails {
        for kind in ThumbnailType::ALL {
            paths.push(storage_path(StorePrefix::Thumbnail(kind), id, Some("jpg")));
        }
    }
    paths
}

fn arrived(draft: MediaDraft, stored: Vec<String>) -> Arrived {
    let MediaDraft { row, data } = draft;
    let file = match (row.has_original, row.storage_id) {
        (true, Some(storage_id)) => Some(StoredFile {
            storage_id,
            extension: row.extension,
            media_type: row.media_type,
            sha256: row.sha256,
            size: row.size,
            has_thumbnails: row.has_thumbnails,
        }),
        _ => None,
    };
    Arrived {
        file,
        ingester: row.ingester,
        renderer: row.renderer,
        data: match data {
            MediaData::None => None,
            other => Some(other.to_value()),
        },
        source: row.source,
        stored,
    }
}

/// The parts of an [`IngestEnv`] the run owns.
struct Env {
    purifier: HtmlPurifier,
    thumbnailer: ImageThumbnailer,
    oembed: Oembed,
}

impl Env {
    fn new() -> Self {
        Self {
            purifier: HtmlPurifier,
            thumbnailer: ImageThumbnailer,
            oembed: Oembed::new(OembedAllowList::shipped()),
        }
    }

    fn with<'a>(
        &'a self,
        store: &'a dyn Store,
        http: &'a dyn HttpFetch,
        validation: &'a FileValidation,
    ) -> IngestEnv<'a> {
        IngestEnv {
            store,
            http,
            purifier: &self.purifier,
            thumbnailer: &self.thumbnailer,
            oembed: &self.oembed,
            validation,
        }
    }
}

/// A folder file is read from disk; its ingest never fetches.
struct NoNetwork;

impl HttpFetch for NoNetwork {
    fn fetch(&self, _url: &str) -> Result<HttpResponse, FetchError> {
        Err(FetchError::new(
            "a file from an archive folder is read from disk, never fetched",
        ))
    }
}

/// A reader that hashes what passes through it.
struct Hashing<R> {
    inner: R,
    hasher: Sha256,
}

impl<R: Read> Read for Hashing<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let read = self.inner.read(buf)?;
        self.hasher.update(&buf[..read]);
        Ok(read)
    }
}
