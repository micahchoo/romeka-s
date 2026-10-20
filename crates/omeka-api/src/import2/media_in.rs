//! A media record's bytes into the store, before its row commits
//! (`docs/import-v2.md`, "Per record", "Evidence" and "URL sources").
//!
//! A file of at most [`WHOLE_LIMIT`] bytes takes the upload ingester's path:
//! `omeka_media::ingest::ingest_file` sniffs, validates, stores the original
//! and makes the thumbnails. A larger file never sits in memory: it is
//! sniffed from its first [`SNIFF_BYTES`], validated, and streamed into the
//! store through [`Store::put_stream`] while its SHA-256 is computed. It
//! gets no thumbnails, because the image decoder needs the whole file in
//! memory. A staged file (an upload) takes the same two paths, read
//! through the store instead of the disk.
//!
//! A URL record runs its ingester with the guarded HTTP client the caller
//! built from `settings::address_policy`. The `url` ingester's download is
//! the one that can be large, so the run fetches it here: a body that
//! declares at most [`WHOLE_LIMIT`] bytes goes to the ingester whole, and
//! any other body streams into the store like a large file, capped at the
//! client's `import_upload_max_file`.
//!
//! Everything here is blocking; the run calls it on the blocking pool. Each
//! function answers what it stored, so the run removes those bytes when the
//! record's transaction does not commit.

use std::fs::File;
use std::io::{self, Cursor, Read};
use std::path::Path;
use std::sync::Mutex;

use chrono::NaiveDateTime;
use omeka_core::ids::{ItemId as CoreItemId, MediaId as CoreMediaId};
use omeka_core::resource::{Media as CoreMedia, Timestamp};
use omeka_db::rows::StoredFile;
use omeka_media::HttpClient;
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
use omeka_media::uri::HttpUri;
use omeka_media::validation::FileValidation;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::evidence::SNIFF_BYTES;
use super::staging::StoredReader;

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

impl ArrivalError {
    /// The run failure for a file shown as `shown`: its message key and text.
    #[must_use]
    pub fn failure(self, shown: &str) -> (&'static str, String) {
        match self {
            Self::Unreadable(reason) => (
                "import_file_unreadable",
                crate::messages::import_file_unreadable(shown, reason),
            ),
            Self::Refused(reason) => (
                "import_file_refused",
                crate::messages::import_file_refused(shown, reason),
            ),
            Self::Store(reason) => (
                "import_file_not_stored",
                crate::messages::import_file_not_stored(shown, reason),
            ),
        }
    }
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
        let mut head = Vec::with_capacity(SNIFF_BYTES);
        File::open(path)
            .map_err(unreadable)?
            .take(SNIFF_BYTES as u64)
            .read_to_end(&mut head)
            .map_err(unreadable)?;
        let file = File::open(path).map_err(unreadable)?;
        return stream_file(store, validation, &head, Box::new(file), Some(size), name)
            .map(|arrived| arrived.with(FILE_INGESTER, Some(name.to_string())));
    }
    let bytes = std::fs::read(path).map_err(unreadable)?;
    arrive_whole(store, validation, bytes, name)
}

/// Store one staged file, read through the store from `stored`, its path
/// there. `name` is the file name `media.source` keeps.
///
/// # Errors
/// An [`ArrivalError`]; nothing new stays in the store then.
pub fn arrive_stored(
    store: &dyn Store,
    validation: &FileValidation,
    stored: &str,
    size: u64,
    name: &str,
) -> Result<Arrived, ArrivalError> {
    let unreadable =
        |error: omeka_media::store::StoreError| ArrivalError::Unreadable(error.to_string());
    if size > WHOLE_LIMIT {
        let head = store
            .read_range(stored, 0..SNIFF_BYTES as u64)
            .map_err(unreadable)?;
        let reader = StoredReader::new(store, stored.to_string(), 0..size);
        return stream_file(store, validation, &head, Box::new(reader), Some(size), name)
            .map(|arrived| arrived.with(FILE_INGESTER, Some(name.to_string())));
    }
    let bytes = store.read(stored).map_err(unreadable)?;
    arrive_whole(store, validation, bytes, name)
}

/// A file small enough to read whole, through the upload ingester's path.
fn arrive_whole(
    store: &dyn Store,
    validation: &FileValidation,
    bytes: Vec<u8>,
    name: &str,
) -> Result<Arrived, ArrivalError> {
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

/// Stream a file into the store: sniffed from `head` (its first bytes),
/// validated, then `body` (the whole file, from its first byte) written
/// while its SHA-256 is computed. No thumbnails.
fn stream_file(
    store: &dyn Store,
    validation: &FileValidation,
    head: &[u8],
    body: Box<dyn Read + '_>,
    size: Option<u64>,
    name: &str,
) -> Result<Arrived, ArrivalError> {
    let media_type = detected_media_type(head);
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
        inner: body,
        hasher: Sha256::new(),
    };
    let written = store
        .put_stream(&mut reader, size, &original)
        .map_err(|error| match error {
            omeka_media::store::StoreError::Io { source, .. } if source.get_ref().is_some() => {
                ArrivalError::Unreadable(FetchError::from_body(&source).message)
            }
            other => ArrivalError::Store(other.to_string()),
        })?;
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
        source: None,
        stored: vec![original],
    })
}

impl Arrived {
    fn with(mut self, ingester: &str, source: Option<String>) -> Self {
        self.ingester = ingester.to_string();
        self.source = source;
        self
    }
}

/// The `url` ingester's download (`Ingester/Url.php`), streamed when it may
/// be large: a body that declares at most [`WHOLE_LIMIT`] bytes is handed to
/// the ingester whole, so it keeps its thumbnails; any other body streams
/// into the store.
fn arrive_download(
    store: &dyn Store,
    validation: &FileValidation,
    http: &HttpClient,
    source: &str,
) -> Result<Arrived, ArrivalError> {
    let Some(uri) = HttpUri::parse(source) else {
        return Err(ArrivalError::Refused("Invalid ingest URL".to_string()));
    };
    let failed =
        |reason: String| ArrivalError::Refused(format!("Error downloading {source}: {reason}"));
    let mut response = http
        .fetch_stream(uri.raw())
        .map_err(|error| failed(error.to_string()))?;
    if !response.is_ok() {
        return Err(failed(format!("{} {}", response.status, response.reason)));
    }
    if response
        .content_length
        .is_some_and(|length| length <= WHOLE_LIMIT)
    {
        let whole = response
            .into_response()
            .map_err(|error| failed(error.to_string()))?;
        let fetched = Fetched(Mutex::new(Some(whole)));
        return run_ingester(store, validation, &fetched, "url", source);
    }
    let mut head = Vec::with_capacity(SNIFF_BYTES);
    (&mut response)
        .take(SNIFF_BYTES as u64)
        .read_to_end(&mut head)
        .map_err(|error| failed(FetchError::from_body(&error).message))?;
    let size = response.content_length;
    let name = uri
        .path()
        .rsplit('/')
        .next()
        .unwrap_or_default()
        .to_string();
    let body = Cursor::new(head.clone()).chain(response);
    stream_file(store, validation, &head, Box::new(body), size, &name)
        .map(|arrived| arrived.with("url", Some(uri.raw().to_string())))
        .map_err(|error| match error {
            ArrivalError::Unreadable(reason) => failed(reason),
            other => other,
        })
}

/// One response, fetched before the ingester asked for it.
struct Fetched(Mutex<Option<HttpResponse>>);

impl HttpFetch for Fetched {
    fn fetch(&self, _url: &str) -> Result<HttpResponse, FetchError> {
        self.0
            .lock()
            .ok()
            .and_then(|mut held| held.take())
            .ok_or_else(|| FetchError::new("the response was already read"))
    }
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
    http: &HttpClient,
    ingester: &str,
    source: &str,
) -> Result<Arrived, ArrivalError> {
    if ingester == "url" {
        return arrive_download(store, validation, http, source);
    }
    run_ingester(store, validation, http, ingester, source)
}

fn run_ingester(
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
