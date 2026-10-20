//! Storing one file for a media row, the port of `TempFile::mediaIngestFile`.

use crate::ingest::{ErrorStore, IngestEnv, MediaDraft};
use crate::media_type::{detected_media_type, extension_for_media_type};
use crate::store::{
    FileStore, StoreError, StorePrefix, extension_from_source_name, new_storage_id, sha256_hex,
};
use crate::validation::FileRefusal;

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

/// Why [`ingest_file`] stored nothing, or stopped part way.
#[derive(Debug, thiserror::Error)]
pub enum IngestFileError {
    /// [`IngestEnv::validation`] refused the file; nothing was stored.
    #[error("{}", .0.iter().map(ToString::to_string).collect::<Vec<_>>().join(" "))]
    Refused(Vec<FileRefusal>),
    /// The store failed.
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl IngestFileError {
    /// Record the error the way the upload and URL ingesters do: each
    /// refusal under `file`, as `Validator::validate` adds it, and a store
    /// failure under `error` after `context`.
    pub fn record(self, errors: &mut ErrorStore, context: &str) {
        match self {
            Self::Refused(refusals) => {
                for refusal in refusals {
                    errors.add("file", refusal.to_string());
                }
            }
            Self::Store(error) => errors.add("error", format!("{context}: {error}")),
        }
    }
}

/// Store a file and record its facts on the draft, matching
/// `TempFile::mediaIngestFile`.
///
/// A file whose facts are kept (`store_original` or `hydrate_metadata`) is
/// validated first, as upstream validates it; a thumbnail-only ingest is not.
///
/// # Errors
/// [`IngestFileError::Refused`] before anything is stored, or
/// [`IngestFileError::Store`] when the store fails.
pub fn ingest_file(
    env: &IngestEnv<'_>,
    draft: &mut MediaDraft,
    file: IngestFile,
    flags: IngestFlags,
) -> Result<(), IngestFileError> {
    // `TempFile::getMediaType` sniffs the bytes with `finfo`; a boundary that
    // already knows the type (a IIIF or oEmbed thumbnail) supplies it.
    let media_type = file
        .media_type
        .clone()
        .unwrap_or_else(|| detected_media_type(&file.bytes));
    // `TempFile::getExtension`: the source name's, else the media type's.
    let extension = file
        .source_name
        .as_deref()
        .and_then(extension_from_source_name)
        .or_else(|| extension_for_media_type(&media_type).map(str::to_string));
    let keeps_facts = flags.store_original || flags.hydrate_metadata;
    if keeps_facts {
        let refusals = env.validation.refusals(
            file.source_name.as_deref().unwrap_or_default(),
            &media_type,
            extension.as_deref(),
        );
        if !refusals.is_empty() {
            return Err(IngestFileError::Refused(refusals));
        }
    }
    let storage_id = match file.storage_id {
        Some(id) => id,
        None => new_storage_id()?,
    };
    draft.row.storage_id = Some(storage_id.clone());

    if keeps_facts {
        draft.row.extension = extension.clone();
        draft.row.media_type = Some(media_type.clone());
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
        let thumbnails = env.thumbnailer.thumbnails(&file.bytes, Some(&media_type));
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
