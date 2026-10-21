//! The file store: the [`Store`] trait every stored byte goes through, the
//! storage path rule, and [`FileStore`], the local implementation.
//!
//! Ports `Omeka\File\Store\StoreInterface` and `Omeka\File\Store\Local` plus the
//! storage path rule from `Omeka\File\TempFile::store()`.
//!
//! Upstream builds a storage path as
//! `sprintf('%s/%s%s', $prefix, $this->getStorageId(), $extension)`
//! (`File/TempFile.php:226`), where the prefix is `original`, `asset`, or a
//! thumbnail type (`File/TempFile.php:236,246,280`). The local store then
//! resolves it under `file_store.local.base_path` (which the shipped config
//! sets to `OMEKA_PATH . '/files'`) and serves it under
//! `file_store.local.base_uri` (`/files`). So the path relative to the Omeka
//! root is `files/<prefix>/<name>.<ext>`, and the URI is
//! `/files/<prefix>/<name>.<ext>` (`File/Store/Local.php:73,96`,
//! `Api/Representation/AbstractResourceRepresentation.php:getFileUrl`).
//!
//! # Sync, and no local path
//!
//! The trait is blocking. Every caller already runs off the async workers:
//! ingest and the import run on `spawn_blocking`, thumbnailing is CPU-bound,
//! and the CLI commands are sequential. An object store implements it with a
//! blocking client, as [`crate::http_client`] does for fetches.
//!
//! The trait has no `local_path`. Callers read bytes through [`Store::read`]
//! or [`Store::read_range`], so no caller can work on a local disk and break on
//! an object store. [`FileStore::local_path`] stays for code that is local by
//! nature, such as the migration's source install.

mod file;
mod s3;
mod s3_http;
mod urls;

pub mod conformance;

use std::io::Read;
use std::ops::Range;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use thiserror::Error;

use crate::file_render::ThumbnailType;

pub use file::FileStore;
pub use s3::{S3Config, S3ConfigError, S3Store};
pub use urls::{Download, FileKind, FileUrls, content_disposition, download_name};

/// How long a signed URL stays valid when the store sets nothing else: four
/// hours, the default of `ROMEKA_S3_URL_TTL_SECS`.
///
/// A browser plays a video by making ranged requests to the one URL the page
/// rendered, so the URL must outlast the longest single viewing, including a
/// pause. Four hours covers a long film or lecture with room to spare, where
/// the previous fifteen minutes cut a seek off mid-playback. It is still
/// bounded: a URL copied out of a document stops working the same day.
pub const DEFAULT_URL_TTL: Duration = Duration::from_secs(4 * 60 * 60);

/// Where stored bytes live. One instance serves the process, chosen at
/// startup, so callers hold an `Arc<dyn Store>`.
///
/// A storage path is relative and `/`-separated, as [`storage_path`] builds
/// it. A path with a `..` component, a leading `/` or no name is refused with
/// [`StoreError::IllegalPath`] before the store touches anything.
pub trait Store: Send + Sync + std::fmt::Debug {
    /// Write `bytes` at `storage_path`, replacing any file there. A reader sees
    /// the old file or the new one, never a partial write (`Local::put`).
    ///
    /// `media_type` is the object's type: an object store sends it as the
    /// object's `Content-Type`, so a fetch without an override answers with it.
    /// The local store keeps no metadata, so it ignores it.
    ///
    /// # Errors
    /// [`StoreError`] when the path is illegal or the write fails.
    fn put(
        &self,
        bytes: &[u8],
        storage_path: &str,
        media_type: Option<&str>,
    ) -> Result<(), StoreError>;

    /// Write everything `reader` yields at `storage_path`, in bounded chunks,
    /// and answer the byte count. `size_hint` is the expected length when the
    /// caller knows it. `media_type` is what [`Store::put`] takes. Replaces
    /// like [`Store::put`].
    ///
    /// # Errors
    /// [`StoreError`] when the path is illegal, the reader fails or the write
    /// fails. Nothing is left at the path then.
    fn put_stream(
        &self,
        reader: &mut dyn Read,
        size_hint: Option<u64>,
        storage_path: &str,
        media_type: Option<&str>,
    ) -> Result<u64, StoreError>;

    /// Read a whole stored file.
    ///
    /// # Errors
    /// [`StoreError`] when the path is illegal or the file cannot be read.
    fn read(&self, storage_path: &str) -> Result<Vec<u8>, StoreError>;

    /// Read the bytes of `range`, clipped at the end of the file. A range that
    /// starts at or after the end answers no bytes.
    ///
    /// # Errors
    /// [`StoreError`] when the path is illegal or the file cannot be read.
    fn read_range(&self, storage_path: &str, range: Range<u64>) -> Result<Vec<u8>, StoreError>;

    /// Delete a stored file. Answers `false` when it was already gone, which
    /// upstream logs and treats as success (`Local::delete`).
    ///
    /// # Errors
    /// [`StoreError`] when the path is illegal or the delete fails.
    fn delete(&self, storage_path: &str) -> Result<bool, StoreError>;

    /// Whether a file is stored at `storage_path`.
    ///
    /// # Errors
    /// [`StoreError`] when the path is illegal or the store cannot answer.
    fn exists(&self, storage_path: &str) -> Result<bool, StoreError>;

    /// The number of bytes stored at `storage_path`, `None` when nothing is.
    /// A site page prints the size of a file it links, so this reads the
    /// object's metadata and never its body.
    ///
    /// # Errors
    /// [`StoreError`] when the path is illegal or the store cannot answer.
    fn size(&self, storage_path: &str) -> Result<Option<u64>, StoreError>;

    /// The files directly inside the folder `prefix` (such as `original`),
    /// sorted by storage path. Files of nested folders are not listed, and a
    /// folder that does not exist lists nothing.
    ///
    /// # Errors
    /// [`StoreError`] when the prefix is illegal or the store cannot list it.
    fn list(&self, prefix: &str) -> Result<Vec<ListedFile>, StoreError>;

    /// The URL a client fetches the file from (`Local::getUri`).
    fn url(&self, storage_path: &str, access: Access) -> String;

    /// The URL of an original, which answers with `download`'s type and name.
    ///
    /// An object store signs both into the URL, so a bucket stops answering
    /// every original as `binary/octet-stream`. The local store answers a path
    /// and the origin sets the two headers from its own lookup, so the default
    /// is the plain URL.
    fn download_url(&self, storage_path: &str, access: Access, download: &Download) -> String {
        let _ = download;
        self.url(storage_path, access)
    }

    /// How long the signed URLs the API and the site render stay valid.
    fn url_ttl(&self) -> Duration {
        DEFAULT_URL_TTL
    }
}

/// Who may fetch a URL from [`Store::url`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    /// Anyone with the URL.
    Public,
    /// Only for `ttl` from now. A store that cannot sign answers its public URL.
    Signed {
        /// How long the URL stays valid.
        ttl: Duration,
    },
}

/// The type of every stored thumbnail: `TempFile::storeThumbnails` always
/// writes a JPEG, whatever the original.
pub const THUMBNAIL_MEDIA_TYPE: &str = "image/jpeg";

/// Which part of the tree a stored file belongs to
/// (`TempFile::storeOriginal`, `storeAsset`, `storeThumbnails`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorePrefix {
    /// `original`, the ingested file.
    Original,
    /// `asset`, an uploaded asset such as a site logo.
    Asset,
    /// A thumbnail type, stored under its own prefix.
    Thumbnail(ThumbnailType),
}

impl StorePrefix {
    /// Every prefix a media or asset row's files live under.
    pub const ALL: [Self; 5] = [
        Self::Original,
        Self::Asset,
        Self::Thumbnail(ThumbnailType::ALL[0]),
        Self::Thumbnail(ThumbnailType::ALL[1]),
        Self::Thumbnail(ThumbnailType::ALL[2]),
    ];

    /// The path segment upstream passes to `TempFile::store`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Original => "original",
            Self::Asset => "asset",
            Self::Thumbnail(kind) => kind.as_str(),
        }
    }
}

/// The storage path for a file, matching `TempFile::store`:
/// `<prefix>/<name>` plus `.` plus the extension when there is one.
#[must_use]
pub fn storage_path(prefix: StorePrefix, name: &str, extension: Option<&str>) -> String {
    match extension {
        Some(extension) if !extension.is_empty() => {
            format!("{}/{name}.{extension}", prefix.as_str())
        }
        _ => format!("{}/{name}", prefix.as_str()),
    }
}

/// A store failure.
#[derive(Debug, Error)]
pub enum StoreError {
    /// The underlying filesystem call failed.
    #[error("file store I/O error at {path}: {source}")]
    Io {
        /// The path involved.
        path: PathBuf,
        /// The underlying error.
        source: std::io::Error,
    },
    /// The storage path escapes the store root.
    #[error("illegal relative component in path \"{0}\"")]
    IllegalPath(String),
    /// The operating system would not supply random bytes for a storage id.
    #[error("could not generate a storage id: {0}")]
    Random(String),
    /// The object store refused a request or could not be reached. The message
    /// names the action and the status, never a URL or a credential.
    #[error("object store error at {path}: {message}")]
    Remote {
        /// The storage path involved.
        path: String,
        /// What failed.
        message: String,
    },
}

impl StoreError {
    pub(crate) fn remote(path: &str, message: impl Into<String>) -> Self {
        Self::Remote {
            path: path.to_string(),
            message: message.into(),
        }
    }

    /// The caller's reader failed mid-stream.
    pub(crate) fn reader(path: &str, source: std::io::Error) -> Self {
        Self::Io {
            path: PathBuf::from(path),
            source,
        }
    }
}

/// The facts a stored file contributes to a media row, matching the columns
/// `TempFile::mediaIngestFile` sets (`File/TempFile.php:420`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredFile {
    /// `media.storage_id`, the file name on disk without extension.
    pub storage_id: String,
    /// `media.extension`.
    pub extension: Option<String>,
    /// `media.sha256`, a 64 character hex digest.
    pub sha256: String,
    /// `media.size` in bytes.
    pub size: u64,
}

/// A file found by [`Store::list`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListedFile {
    /// The storage path, as [`storage_path`] builds it.
    pub storage_path: String,
    /// The file's last modification time.
    pub modified: SystemTime,
}

/// Generate a storage id the way `TempFile::getStorageId` does:
/// `bin2hex(random_bytes(20))`, 40 lowercase hex characters.
pub fn new_storage_id() -> Result<String, StoreError> {
    let mut bytes = [0u8; 20];
    getrandom::getrandom(&mut bytes).map_err(|error| StoreError::Random(error.to_string()))?;
    Ok(crate::text::hex_encode(&bytes))
}

/// The SHA-256 of a byte slice, matching `TempFile::getSha256` (`hash_file`).
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    crate::text::hex_encode(&hasher.finalize())
}

/// Resolve a file extension the way `TempFile::getExtension` does from the
/// source name.
///
/// The name must contain a dot, must not end with one, and the extension after
/// the last dot must be at most 12 characters. The result is lowercased.
/// Upstream falls back to a media-type map when the name has no usable
/// extension; the port leaves that to the caller, because the map is 400 lines
/// of configuration (`application/data/media-types.php`).
#[must_use]
pub fn extension_from_source_name(source_name: &str) -> Option<String> {
    let dot = source_name.rfind('.')?;
    let extension = &source_name[dot + 1..];
    if extension.is_empty() || extension.len() > 12 {
        return None;
    }
    Some(extension.to_lowercase())
}

/// The thumbnailer seam.
///
/// Upstream builds thumbnails through `Omeka\File\Thumbnailer` and stores them
/// with `TempFile::storeThumbnails` (`File/TempFile.php:275`). Image processing
/// is a separate concern, so the port takes a [`Thumbnailer`] and stores
/// whatever it returns. Tests use a fake; the production implementation belongs
/// to whichever crate owns image decoding.
pub trait Thumbnailer {
    /// Produce thumbnail bytes per type for an original file. Returning an empty
    /// list means no thumbnails were made, which is a normal outcome upstream.
    fn thumbnails(
        &self,
        original: &[u8],
        media_type: Option<&str>,
    ) -> Vec<(ThumbnailType, Vec<u8>)>;
}

/// Refuse a storage path that could leave the store: a `..` component, a
/// leading `/`, or an empty path. Upstream's `Local::getLocalPath` rejects the
/// first; the other two would make a local join replace the root.
pub(crate) fn checked_path(storage_path: &str) -> Result<&str, StoreError> {
    let illegal = storage_path.is_empty()
        || storage_path.starts_with('/')
        || storage_path.split('/').any(|part| part == "..");
    if illegal {
        return Err(StoreError::IllegalPath(storage_path.to_string()));
    }
    Ok(storage_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `TempFile::store` with `storeOriginal`:
    /// `sprintf('%s/%s%s', 'original', $storageId, '.jpg')`.
    #[test]
    fn storage_path_matches_upstream_temp_file_store() {
        assert_eq!(
            storage_path(StorePrefix::Original, "abc123", Some("jpg")),
            "original/abc123.jpg"
        );
        assert_eq!(
            storage_path(StorePrefix::Original, "abc123", None),
            "original/abc123"
        );
    }

    /// `TempFile::storeThumbnails` stores each type under its own prefix with a
    /// forced `jpg` extension (`File/TempFile.php:280`).
    #[test]
    fn thumbnail_storage_paths_use_type_prefixes() {
        for kind in ThumbnailType::ALL {
            assert_eq!(
                storage_path(StorePrefix::Thumbnail(kind), "abc123", Some("jpg")),
                format!("{}/abc123.jpg", kind.as_str())
            );
        }
    }

    /// The five prefixes, in the order upstream's store writes them.
    #[test]
    fn all_prefixes_are_original_asset_and_the_thumbnail_types() {
        let names: Vec<&str> = StorePrefix::ALL
            .iter()
            .map(|prefix| prefix.as_str())
            .collect();
        assert_eq!(names, ["original", "asset", "large", "medium", "square"]);
    }

    /// `TempFile::getStorageId`: 40 lowercase hex characters.
    #[test]
    fn storage_ids_are_40_hex_characters() {
        let id = new_storage_id().expect("random");
        assert_eq!(id.len(), 40);
        assert!(
            id.chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        );
        assert_ne!(id, new_storage_id().expect("random"));
    }

    /// `TempFile::getSha256` for a known input.
    #[test]
    fn sha256_hex_matches_hash_file() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    /// `TempFile::getExtension` from the source name: a dot, a non-empty tail,
    /// at most 12 characters, lowercased.
    #[test]
    fn extension_from_source_name_matches_upstream_temp_file() {
        assert_eq!(extension_from_source_name("Photo.JPG"), Some("jpg".into()));
        assert_eq!(
            extension_from_source_name("archive.tar.gz"),
            Some("gz".into())
        );
        assert_eq!(extension_from_source_name("noext"), None);
        assert_eq!(extension_from_source_name("trailing."), None);
        assert_eq!(extension_from_source_name("too.longextension123"), None);
    }
}
