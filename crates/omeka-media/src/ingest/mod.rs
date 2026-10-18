//! Media ingesters.
//!
//! One module per upstream class under `application/src/Media/Ingester/`:
//! [`upload`], [`url`], [`html`], [`iiif`], [`iiif_presentation`], [`oembed`],
//! [`youtube`] and [`fallback`]. [`IngesterRegistry`] mirrors `Manager.php`,
//! including its fallback behaviour, and [`Ingester`] / [`MutableIngester`]
//! mirror the two upstream interfaces.
//!
//! An ingester reads the API request, validates it, fills a [`MediaDraft`], and
//! stores files through [`crate::store::FileStore`]. Every external dependency
//! is a trait in [`IngestEnv`], so a test drives an ingester with fakes and no
//! network, no database and no temporary server.

pub mod fallback;
pub mod html;
pub mod iiif;
pub mod iiif_presentation;
pub mod oembed;
pub mod upload;
pub mod url;
pub mod youtube;

use std::collections::BTreeMap;
use std::sync::Arc;

use omeka_core::resource::Media;
use serde_json::{Map, Value};

use crate::data::MediaData;
use crate::http::HttpFetch;
use crate::oembed::Oembed;
use crate::store::{
    FileStore, StoreError, StorePrefix, Thumbnailer, extension_from_source_name, new_storage_id,
    sha256_hex,
};

/// A key and message pair, matching one `ErrorStore::addError` call
/// (`Stdlib/ErrorStore.php:23`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IngestError {
    /// The error key, e.g. `error` or `o:source`.
    pub key: String,
    /// The message.
    pub message: String,
}

/// The error store upstream threads through every ingest
/// (`Stdlib/ErrorStore.php`).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ErrorStore {
    errors: Vec<IngestError>,
}

impl ErrorStore {
    /// Add an error under a key.
    pub fn add(&mut self, key: impl Into<String>, message: impl Into<String>) {
        self.errors.push(IngestError {
            key: key.into(),
            message: message.into(),
        });
    }

    /// Every error, in the order it was added.
    #[must_use]
    pub fn errors(&self) -> &[IngestError] {
        &self.errors
    }

    /// The messages recorded under one key.
    #[must_use]
    pub fn messages_for(&self, key: &str) -> Vec<&str> {
        self.errors
            .iter()
            .filter(|error| error.key == key)
            .map(|error| error.message.as_str())
            .collect()
    }

    /// Whether any error was recorded.
    #[must_use]
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    /// Whether no error was recorded.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.errors.is_empty()
    }

    /// Merge another store's errors, matching `ErrorStore::mergeErrors`.
    pub fn merge(&mut self, other: &Self) {
        self.errors.extend(other.errors.iter().cloned());
    }
}

/// One uploaded file, the port's shape for an entry of `$_FILES['file']`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UploadedFile {
    /// The client file name, which becomes the source name.
    pub name: String,
    /// The file bytes.
    pub bytes: Vec<u8>,
}

impl UploadedFile {
    /// Build an uploaded file.
    #[must_use]
    pub fn new(name: impl Into<String>, bytes: impl Into<Vec<u8>>) -> Self {
        Self {
            name: name.into(),
            bytes: bytes.into(),
        }
    }
}

/// An ingest or update request (`Api/Request.php`).
#[derive(Debug, Clone, Default)]
pub struct IngestRequest {
    content: Map<String, Value>,
    files: Vec<UploadedFile>,
}

impl IngestRequest {
    /// Build from the request content.
    #[must_use]
    pub fn new(content: Map<String, Value>) -> Self {
        Self {
            content,
            files: Vec::new(),
        }
    }

    /// Build from a JSON object literal.
    #[must_use]
    pub fn from_value(content: Value) -> Self {
        match content {
            Value::Object(map) => Self::new(map),
            _ => Self::default(),
        }
    }

    /// Attach uploaded files, indexed like `$fileData['file']`.
    #[must_use]
    pub fn with_files(mut self, files: Vec<UploadedFile>) -> Self {
        self.files = files;
        self
    }

    /// The request content (`Request::getContent`).
    #[must_use]
    pub fn content(&self) -> &Map<String, Value> {
        &self.content
    }

    /// The uploaded files (`Request::getFileData()['file']`).
    #[must_use]
    pub fn files(&self) -> &[UploadedFile] {
        &self.files
    }

    /// A content value (`Request::getValue`).
    #[must_use]
    pub fn value(&self, key: &str) -> Option<&Value> {
        self.content.get(key)
    }

    /// A content value as a string.
    #[must_use]
    pub fn string(&self, key: &str) -> Option<&str> {
        self.content.get(key).and_then(Value::as_str)
    }

    /// Whether the content has a key, matching `array_key_exists`.
    #[must_use]
    pub fn has(&self, key: &str) -> bool {
        self.content.contains_key(key)
    }

    /// Whether a content value is set and not null, matching `isset`.
    #[must_use]
    pub fn is_set(&self, key: &str) -> bool {
        matches!(self.content.get(key), Some(value) if !value.is_null())
    }
}

/// A media row being built by an ingester.
///
/// The port keeps the core [`Media`] row for identity and file facts, and adds
/// the typed payload core deliberately omits (DESIGN.md "Media").
#[derive(Debug, Clone, PartialEq)]
pub struct MediaDraft {
    /// The `media` row.
    pub row: Media,
    /// The typed `media.data` value.
    pub data: MediaData,
}

impl MediaDraft {
    /// Wrap a media row with no payload yet.
    #[must_use]
    pub fn new(row: Media) -> Self {
        Self {
            row,
            data: MediaData::None,
        }
    }
}

/// The HTML purifier seam.
///
/// Upstream purifies ingester HTML through `Omeka\Stdlib\HtmlPurifier`
/// (`Ingester/Html.php:65`). The port keeps the call behind a trait so the
/// crate carries no HTML sanitizer of its own.
pub trait Purifier {
    /// Purify an HTML fragment.
    fn purify(&self, html: &str) -> String;
}

/// The purifier the port runs in production: [`omeka_core::purify::purify`],
/// the port of `Stdlib\HtmlPurifier`.
#[derive(Debug, Clone, Copy, Default)]
pub struct HtmlPurifier;

impl Purifier for HtmlPurifier {
    fn purify(&self, html: &str) -> String {
        omeka_core::purify::purify(html)
    }
}

/// Every external dependency an ingester may use.
pub struct IngestEnv<'a> {
    /// The file store.
    pub store: &'a FileStore,
    /// The network seam.
    pub http: &'a dyn HttpFetch,
    /// The HTML purifier.
    pub purifier: &'a dyn Purifier,
    /// The thumbnailer.
    pub thumbnailer: &'a dyn Thumbnailer,
    /// The oEmbed service.
    pub oembed: &'a Oembed,
}

/// What `TempFile::mediaIngestFile` should do with a file
/// (`File/TempFile.php:405`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IngestFlags {
    /// Store the original file.
    pub store_original: bool,
    /// Create and store thumbnails.
    pub store_thumbnails: bool,
    /// Set extension, media type, checksum and size even when the original is
    /// not stored.
    pub hydrate_metadata: bool,
}

impl Default for IngestFlags {
    fn default() -> Self {
        Self {
            store_original: true,
            store_thumbnails: true,
            hydrate_metadata: false,
        }
    }
}

/// A file handed to the store, the port's shape for a `TempFile`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IngestFile {
    /// The file bytes.
    pub bytes: Vec<u8>,
    /// The source name, used for the extension and stored as `media.source`.
    pub source_name: Option<String>,
    /// A fixed storage id, or `None` to generate one.
    pub storage_id: Option<String>,
    /// The detected media type, or `None` when the boundary did not detect one.
    pub media_type: Option<String>,
}

impl IngestFile {
    /// A file with only bytes; the storage id is generated.
    #[must_use]
    pub fn new(bytes: impl Into<Vec<u8>>) -> Self {
        Self {
            bytes: bytes.into(),
            source_name: None,
            storage_id: None,
            media_type: None,
        }
    }

    /// Set the source name.
    #[must_use]
    pub fn with_source_name(mut self, source_name: impl Into<String>) -> Self {
        self.source_name = Some(source_name.into());
        self
    }

    /// Set a fixed storage id, which tests use to make paths predictable.
    #[must_use]
    pub fn with_storage_id(mut self, storage_id: impl Into<String>) -> Self {
        self.storage_id = Some(storage_id.into());
        self
    }

    /// Set the media type.
    #[must_use]
    pub fn with_media_type(mut self, media_type: impl Into<String>) -> Self {
        self.media_type = Some(media_type.into());
        self
    }
}

/// Store a file and record its facts on the draft, matching
/// `TempFile::mediaIngestFile`.
pub fn ingest_file(
    env: &IngestEnv<'_>,
    draft: &mut MediaDraft,
    file: IngestFile,
    flags: IngestFlags,
) -> Result<(), StoreError> {
    let storage_id = match file.storage_id {
        Some(id) => id,
        None => new_storage_id()?,
    };
    let extension = file
        .source_name
        .as_deref()
        .and_then(extension_from_source_name);
    let media_type = file.media_type.clone();
    draft.row.storage_id = Some(storage_id.clone());

    if flags.store_original || flags.hydrate_metadata {
        draft.row.extension = extension.clone();
        draft.row.media_type = media_type.clone();
        draft.row.sha256 = Some(sha256_hex(&file.bytes));
        draft.row.size = Some(file.bytes.len() as i64);
    }

    if flags.store_original {
        let path =
            FileStore::storage_path(StorePrefix::Original, &storage_id, extension.as_deref());
        env.store.put(&file.bytes, &path)?;
        draft.row.has_original = true;
    }

    if flags.store_thumbnails {
        let thumbnails = env
            .thumbnailer
            .thumbnails(&file.bytes, media_type.as_deref());
        let mut made = false;
        for (kind, bytes) in thumbnails {
            let path =
                FileStore::storage_path(StorePrefix::Thumbnail(kind), &storage_id, Some("jpg"));
            env.store.put(&bytes, &path)?;
            made = true;
        }
        draft.row.has_thumbnails = made;
    }

    Ok(())
}

/// Whether a JSON value is truthy under PHP's rules.
///
/// Upstream tests options and payload members with bare `if ($value)`, so a
/// JSON `false`, a `0`, and the empty string are false; any other scalar and
/// any non-empty array or object are true.
#[must_use]
pub(crate) fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(flag) => *flag,
        Value::Number(number) => number.as_f64().is_some_and(|value| value != 0.0),
        Value::String(text) => !text.is_empty() && text != "0",
        Value::Array(items) => !items.is_empty(),
        Value::Object(map) => !map.is_empty(),
    }
}

/// Read a file index the way PHP coerces an array key: an integer, or a numeric
/// string.
#[must_use]
pub(crate) fn index_value(value: &Value) -> Option<usize> {
    match value {
        Value::Number(number) => number.as_u64().map(|index| index as usize),
        Value::String(text) => text.trim().parse::<usize>().ok(),
        _ => None,
    }
}

/// `(string) $value` followed by `trim()`, for the YouTube time parameters.
#[must_use]
pub(crate) fn trim_scalar(value: &Value) -> String {
    let text = match value {
        Value::String(text) => text.clone(),
        Value::Number(number) => number.to_string(),
        Value::Bool(true) => "1".to_string(),
        _ => String::new(),
    };
    text.trim().to_string()
}

/// PHP's `is_numeric` for the time parameters: an integer or float string.
///
/// The character guard keeps Rust's `f64` parser from accepting `inf` and `nan`,
/// which PHP rejects.
#[must_use]
pub(crate) fn php_is_numeric(text: &str) -> bool {
    if text.is_empty()
        || !text
            .chars()
            .all(|c| c.is_ascii_digit() || "+-.eE".contains(c))
    {
        return false;
    }
    text.parse::<i64>().is_ok() || text.parse::<f64>().is_ok()
}

/// A media ingester (`Media/Ingester/IngesterInterface.php`).
pub trait Ingester {
    /// The registry name, e.g. `upload`.
    fn name(&self) -> &str;

    /// The human-readable label (`getLabel`).
    fn label(&self) -> String;

    /// The renderer name (`getRenderer`).
    fn renderer(&self) -> &'static str;

    /// Process an ingest request (`ingest`).
    fn ingest(
        &self,
        draft: &mut MediaDraft,
        request: &IngestRequest,
        env: &IngestEnv<'_>,
        errors: &mut ErrorStore,
    );
}

/// An ingester that also accepts updates
/// (`Media/Ingester/MutableIngesterInterface.php`).
pub trait MutableIngester: Ingester {
    /// Process an update request (`update`).
    fn update(
        &self,
        draft: &mut MediaDraft,
        request: &IngestRequest,
        env: &IngestEnv<'_>,
        errors: &mut ErrorStore,
    );
}

/// The ingester registry, mirroring `Media/Ingester/Manager.php`.
pub struct IngesterRegistry {
    ingesters: BTreeMap<String, Arc<dyn Ingester>>,
}

impl Default for IngesterRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl IngesterRegistry {
    /// An empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            ingesters: BTreeMap::new(),
        }
    }

    /// The seven ingesters from `media_ingesters`
    /// (`application/config/module.config.php:860`).
    #[must_use]
    pub fn with_defaults() -> Self {
        let mut registry = Self::new();
        registry.register(Box::new(upload::Upload));
        registry.register(Box::new(url::Url));
        registry.register(Box::new(html::Html));
        registry.register(Box::new(iiif::Iiif::new(800)));
        registry.register(Box::new(iiif_presentation::IiifPresentation));
        registry.register(Box::new(oembed::OEmbed));
        registry.register(Box::new(youtube::Youtube));
        registry
    }

    /// Add an ingester under its own name.
    pub fn register(&mut self, ingester: Box<dyn Ingester>) {
        let name = ingester.name().to_string();
        self.ingesters.insert(name, Arc::from(ingester));
    }

    /// Get an ingester by name, falling back to [`fallback::Fallback`] for an
    /// unknown name, exactly as `Manager::get` does (`Ingester/Manager.php:18`).
    #[must_use]
    pub fn get(&self, name: &str) -> Arc<dyn Ingester> {
        match self.ingesters.get(name) {
            Some(ingester) => Arc::clone(ingester),
            None => Arc::new(fallback::Fallback::new(name)),
        }
    }

    /// The registered names, sorted.
    #[must_use]
    pub fn names(&self) -> Vec<String> {
        self.ingesters.keys().cloned().collect()
    }

    /// Borrow the registered ingester for a name, or `None` when the name is
    /// unknown. Unlike [`IngesterRegistry::get`] this does not invent a
    /// fallback.
    #[must_use]
    pub fn find(&self, name: &str) -> Option<&dyn Ingester> {
        self.ingesters.get(name).map(AsRef::as_ref)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `Manager::get` returns a `Fallback` for a name it does not know
    /// (`Ingester/Manager.php:18`, `Ingester/Fallback.php:35`).
    #[test]
    fn registry_falls_back_for_an_unknown_name() {
        let registry = IngesterRegistry::with_defaults();
        let unknown = registry.get("does_not_exist");
        assert_eq!(unknown.name(), "does_not_exist");
        assert_eq!(unknown.label(), "Unknown [does_not_exist]");
        assert_eq!(unknown.renderer(), "fallback");
    }

    /// The seven upstream ingesters are registered under their configuration
    /// keys (`application/config/module.config.php:860`).
    #[test]
    fn registry_holds_the_upstream_ingesters() {
        let registry = IngesterRegistry::with_defaults();
        let names = registry.names();
        assert_eq!(
            names,
            vec![
                "html",
                "iiif",
                "iiif_presentation",
                "oembed",
                "upload",
                "url",
                "youtube"
            ]
        );
        assert_eq!(
            registry.find("upload").map(Ingester::renderer),
            Some("file")
        );
    }

    /// `ErrorStore::mergeErrors` keeps every key and message.
    #[test]
    fn error_store_merges_and_lists_by_key() {
        let mut store = ErrorStore::default();
        store.add("error", "first");
        let mut other = ErrorStore::default();
        other.add("o:source", "second");
        store.merge(&other);
        assert_eq!(store.errors().len(), 2);
        assert_eq!(store.messages_for("error"), vec!["first"]);
        assert_eq!(store.messages_for("o:source"), vec!["second"]);
        assert!(store.has_errors());
    }

    /// `Request::getValue` returns the content value, and `array_key_exists`
    /// distinguishes a null value from a missing key.
    #[test]
    fn request_reads_content_like_upstream_request() {
        let request = IngestRequest::from_value(serde_json::json!({"a": 1, "b": null}));
        assert_eq!(request.value("a"), Some(&serde_json::json!(1)));
        assert!(request.has("b"));
        assert!(!request.is_set("b"));
        assert!(!request.has("c"));
        assert_eq!(request.string("a"), None);
    }
}
