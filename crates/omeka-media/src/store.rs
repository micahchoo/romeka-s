//! The file store.
//!
//! Ports `Omeka\File\Store\Local` plus the storage path rule from
//! `Omeka\File\TempFile::store()`.
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
//! The ingesters reach this module only through [`crate::ingest`]; the store is
//! the one place that writes bytes.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use thiserror::Error;

use crate::file_render::ThumbnailType;

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

/// A local filesystem file store (`File/Store/Local.php`).
#[derive(Debug, Clone)]
pub struct FileStore {
    root: PathBuf,
    base_uri: String,
}

impl FileStore {
    /// Build a store rooted at `root` (upstream `file_store.local.base_path`)
    /// and served under `base_uri` (upstream `file_store.local.base_uri`).
    #[must_use]
    pub fn new(root: impl Into<PathBuf>, base_uri: impl Into<String>) -> Self {
        Self {
            root: root.into(),
            base_uri: base_uri.into(),
        }
    }

    /// The store root.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The base URI.
    #[must_use]
    pub fn base_uri(&self) -> &str {
        &self.base_uri
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

    /// The URL for a storage path, matching `Local::getUri`.
    #[must_use]
    pub fn uri(&self, storage_path: &str) -> String {
        format!("{}/{}", self.base_uri, storage_path)
    }

    /// The absolute local path for a storage path, matching `Local::getLocalPath`.
    ///
    /// A `..` component anywhere in the path is an error, as upstream rejects it
    /// before touching the filesystem.
    pub fn local_path(&self, storage_path: &str) -> Result<PathBuf, StoreError> {
        if has_parent_component(storage_path) {
            return Err(StoreError::IllegalPath(storage_path.to_string()));
        }
        Ok(self.root.join(storage_path))
    }

    /// Write bytes to a storage path, creating parent directories, matching
    /// `Local::put`.
    pub fn put(&self, bytes: &[u8], storage_path: &str) -> Result<(), StoreError> {
        let path = self.local_path(storage_path)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| StoreError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        std::fs::write(&path, bytes).map_err(|source| StoreError::Io { path, source })
    }

    /// Write bytes to a storage path without ever leaving a partial file in
    /// place: write to a unique sibling, then rename it into the target.
    ///
    /// A run that stops between the two steps leaves only the temporary file,
    /// which the next write for the same target overwrites, so the target is
    /// either missing or complete. The thumbnailer uses this so a crash cannot
    /// turn an existence check into a corrupt thumbnail.
    pub fn put_atomic(&self, bytes: &[u8], storage_path: &str) -> Result<(), StoreError> {
        let path = self.local_path(storage_path)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| StoreError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        let temp = temp_sibling(&path);
        std::fs::write(&temp, bytes).map_err(|source| StoreError::Io {
            path: temp.clone(),
            source,
        })?;
        std::fs::rename(&temp, &path).map_err(|source| {
            let _ = std::fs::remove_file(&temp);
            StoreError::Io { path, source }
        })
    }

    /// Read bytes back from a storage path.
    pub fn read(&self, storage_path: &str) -> Result<Vec<u8>, StoreError> {
        let path = self.local_path(storage_path)?;
        std::fs::read(&path).map_err(|source| StoreError::Io { path, source })
    }

    /// Delete a stored file. Returns `false` when it was already gone, which
    /// upstream logs and treats as success (`Local::delete`).
    pub fn delete(&self, storage_path: &str) -> Result<bool, StoreError> {
        let path = self.local_path(storage_path)?;
        if !path.exists() {
            return Ok(false);
        }
        std::fs::remove_file(&path).map_err(|source| StoreError::Io { path, source })?;
        Ok(true)
    }

    /// Whether a storage path exists on disk.
    #[must_use]
    pub fn exists(&self, storage_path: &str) -> bool {
        self.local_path(storage_path)
            .map(|path| path.exists())
            .unwrap_or(false)
    }
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

fn has_parent_component(storage_path: &str) -> bool {
    storage_path.split('/').any(|part| part == "..")
}

/// A unique temporary path beside `path`, on the same filesystem so the rename
/// into place is atomic. The process id and a process-local counter keep two
/// writers for one target from sharing a temporary file.
fn temp_sibling(path: &Path) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let serial = COUNTER.fetch_add(1, Ordering::Relaxed);
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("file");
    path.with_file_name(format!(".{name}.{}.{serial}.tmp", std::process::id()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> FileStore {
        FileStore::new("/srv/romeka/files", "/files")
    }

    /// `TempFile::store` with `storeOriginal`:
    /// `sprintf('%s/%s%s', 'original', $storageId, '.jpg')`.
    #[test]
    fn storage_path_matches_upstream_temp_file_store() {
        assert_eq!(
            FileStore::storage_path(StorePrefix::Original, "abc123", Some("jpg")),
            "original/abc123.jpg"
        );
        assert_eq!(
            FileStore::storage_path(StorePrefix::Original, "abc123", None),
            "original/abc123"
        );
    }

    /// `TempFile::storeThumbnails` stores each type under its own prefix with a
    /// forced `jpg` extension (`File/TempFile.php:280`).
    #[test]
    fn thumbnail_storage_paths_use_type_prefixes() {
        for kind in ThumbnailType::ALL {
            assert_eq!(
                FileStore::storage_path(StorePrefix::Thumbnail(kind), "abc123", Some("jpg")),
                format!("{}/abc123.jpg", kind.as_str())
            );
        }
    }

    /// `Local::getLocalPath` and `Local::getUri`: the path relative to the
    /// Omeka root is `files/<prefix>/<name>.<ext>` and the URI is
    /// `/files/<prefix>/<name>.<ext>`.
    #[test]
    fn local_path_and_uri_match_upstream_local_store() {
        let store = store();
        let storage_path = FileStore::storage_path(StorePrefix::Original, "abc123", Some("jpg"));
        assert_eq!(
            store.local_path(&storage_path).expect("path"),
            PathBuf::from("/srv/romeka/files/original/abc123.jpg")
        );
        assert_eq!(store.uri(&storage_path), "/files/original/abc123.jpg");
    }

    /// `Local::getLocalPath` rejects a `..` component before touching the disk.
    #[test]
    fn local_path_rejects_parent_component() {
        let store = store();
        assert!(matches!(
            store.local_path("../secrets.txt"),
            Err(StoreError::IllegalPath(_))
        ));
        assert!(matches!(
            store.local_path("original/../../etc/passwd"),
            Err(StoreError::IllegalPath(_))
        ));
    }

    /// `Local::put` then `getUri`: the bytes land where the URI says they do.
    #[test]
    fn put_writes_bytes_under_the_store_root() {
        let root = std::env::temp_dir().join(format!("romeka-media-store-{}", std::process::id()));
        let store = FileStore::new(&root, "/files");
        let storage_path = FileStore::storage_path(StorePrefix::Original, "abc123", Some("txt"));
        store.put(b"hello", &storage_path).expect("put");
        assert_eq!(store.read(&storage_path).expect("read"), b"hello");
        assert!(store.exists(&storage_path));
        assert!(store.delete(&storage_path).expect("delete"));
        assert!(!store.exists(&storage_path));
        assert!(!store.delete(&storage_path).expect("second delete"));
        let _ = std::fs::remove_dir_all(&root);
    }

    /// `put_atomic` leaves the target complete and no temporary sibling, so an
    /// existence check on the target never sees a partial write.
    #[test]
    fn put_atomic_writes_the_target_and_no_temporary_file() {
        let root = std::env::temp_dir().join(format!("romeka-media-atomic-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let store = FileStore::new(&root, "/files");
        let storage_path = FileStore::storage_path(
            StorePrefix::Thumbnail(ThumbnailType::Large),
            "abc123",
            Some("jpg"),
        );
        store
            .put_atomic(b"thumb", &storage_path)
            .expect("put_atomic");
        assert_eq!(store.read(&storage_path).expect("read"), b"thumb");
        let names: Vec<String> = std::fs::read_dir(root.join("large"))
            .expect("directory")
            .map(|entry| {
                entry
                    .expect("entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        assert_eq!(names, vec!["abc123.jpg".to_string()]);
        let _ = std::fs::remove_dir_all(&root);
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
