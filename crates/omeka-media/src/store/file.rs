//! [`FileStore`], the local filesystem store (`File/Store/Local.php`).

use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use super::{Access, ListedFile, Store, StoreError, checked_path};

/// The copy buffer of [`Store::put_stream`]: the most of a stream held at once.
const STREAM_CHUNK: usize = 256 * 1024;

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

    /// The absolute local path for a storage path, matching `Local::getLocalPath`.
    ///
    /// An illegal path is an error, as upstream rejects a `..` component before
    /// touching the filesystem.
    pub fn local_path(&self, storage_path: &str) -> Result<PathBuf, StoreError> {
        Ok(self.root.join(checked_path(storage_path)?))
    }

    /// Write through a unique sibling and rename it into place, so the target
    /// is either missing, the old file, or complete. A run that stops between
    /// the two steps leaves only the sibling, which `romeka media sweep` finds.
    fn write_atomic<T>(
        &self,
        storage_path: &str,
        write: impl FnOnce(&mut File) -> std::io::Result<T>,
    ) -> Result<T, StoreError> {
        let path = self.local_path(storage_path)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(io(parent))?;
        }
        let temp = temp_sibling(&path);
        let written = File::create(&temp)
            .and_then(|mut file| write(&mut file))
            .and_then(|written| std::fs::rename(&temp, &path).map(|()| written));
        written.map_err(|source| {
            let _ = std::fs::remove_file(&temp);
            StoreError::Io { path, source }
        })
    }
}

impl Store for FileStore {
    fn put(
        &self,
        bytes: &[u8],
        storage_path: &str,
        _media_type: Option<&str>,
    ) -> Result<(), StoreError> {
        self.write_atomic(storage_path, |file| file.write_all(bytes))
    }

    fn put_stream(
        &self,
        reader: &mut dyn Read,
        _size_hint: Option<u64>,
        storage_path: &str,
        _media_type: Option<&str>,
    ) -> Result<u64, StoreError> {
        self.write_atomic(storage_path, |file| {
            let mut buffer = vec![0; STREAM_CHUNK];
            let mut written = 0;
            loop {
                let read = match reader.read(&mut buffer) {
                    Ok(0) => return Ok(written),
                    Ok(read) => read,
                    Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(error) => return Err(error),
                };
                file.write_all(&buffer[..read])?;
                written += read as u64;
            }
        })
    }

    fn read(&self, storage_path: &str) -> Result<Vec<u8>, StoreError> {
        let path = self.local_path(storage_path)?;
        std::fs::read(&path).map_err(|source| StoreError::Io { path, source })
    }

    fn read_range(&self, storage_path: &str, range: Range<u64>) -> Result<Vec<u8>, StoreError> {
        let path = self.local_path(storage_path)?;
        let mut bytes = Vec::new();
        File::open(&path)
            .and_then(|mut file| {
                file.seek(SeekFrom::Start(range.start))?;
                file.take(range.end.saturating_sub(range.start))
                    .read_to_end(&mut bytes)
            })
            .map_err(|source| StoreError::Io { path, source })?;
        Ok(bytes)
    }

    fn delete(&self, storage_path: &str) -> Result<bool, StoreError> {
        let path = self.local_path(storage_path)?;
        match std::fs::remove_file(&path) {
            Ok(()) => Ok(true),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(source) => Err(StoreError::Io { path, source }),
        }
    }

    fn exists(&self, storage_path: &str) -> Result<bool, StoreError> {
        let path = self.local_path(storage_path)?;
        path.try_exists()
            .map_err(|source| StoreError::Io { path, source })
    }

    /// The file's metadata, so a page that links it prints its size without
    /// reading it.
    fn size(&self, storage_path: &str) -> Result<Option<u64>, StoreError> {
        let path = self.local_path(storage_path)?;
        match std::fs::metadata(&path) {
            Ok(metadata) => Ok(Some(metadata.len())),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(source) => Err(StoreError::Io { path, source }),
        }
    }

    /// Symlinks are never followed and never listed: neither a linked folder
    /// nor a linked file is reported, so a caller that deletes what this
    /// returns cannot reach outside the root.
    fn list(&self, prefix: &str) -> Result<Vec<ListedFile>, StoreError> {
        let prefix = prefix.trim_end_matches('/');
        let dir = self.local_path(prefix)?;
        match std::fs::symlink_metadata(&dir) {
            Ok(meta) if meta.file_type().is_dir() => {}
            Ok(_) => return Ok(Vec::new()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(io(&dir)(error)),
        }
        let mut listed = Vec::new();
        for entry in std::fs::read_dir(&dir).map_err(io(&dir))? {
            let entry = entry.map_err(io(&dir))?;
            let Ok(name) = entry.file_name().into_string() else {
                continue;
            };
            // `DirEntry::file_type` does not follow a symlink.
            let kind = entry.file_type().map_err(io(&entry.path()))?;
            if !kind.is_file() {
                continue;
            }
            let modified = entry
                .metadata()
                .and_then(|meta| meta.modified())
                .map_err(io(&entry.path()))?;
            listed.push(ListedFile {
                storage_path: format!("{prefix}/{name}"),
                modified,
            });
        }
        listed.sort_by(|a, b| a.storage_path.cmp(&b.storage_path));
        Ok(listed)
    }

    /// `Local::getUri`. The local store cannot sign, so a signed URL is the
    /// public one; `/files` access control is the web server's.
    fn url(&self, storage_path: &str, _access: Access) -> String {
        format!("{}/{}", self.base_uri, storage_path)
    }
}

fn io(path: &Path) -> impl Fn(std::io::Error) -> StoreError {
    let path = path.to_path_buf();
    move |source| StoreError::Io {
        path: path.clone(),
        source,
    }
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
    use crate::file_render::ThumbnailType;
    use crate::store::{StorePrefix, storage_path};

    /// `Local::getLocalPath` and `Local::getUri`: the path relative to the
    /// Omeka root is `files/<prefix>/<name>.<ext>` and the URI is
    /// `/files/<prefix>/<name>.<ext>`.
    #[test]
    fn local_path_and_url_match_upstream_local_store() {
        let store = FileStore::new("/srv/romeka/files", "/files");
        let path = storage_path(StorePrefix::Original, "abc123", Some("jpg"));
        assert_eq!(
            store.local_path(&path).expect("path"),
            PathBuf::from("/srv/romeka/files/original/abc123.jpg")
        );
        assert_eq!(
            store.url(&path, Access::Public),
            "/files/original/abc123.jpg"
        );
    }

    /// A put leaves the target complete and no temporary sibling, so an
    /// existence check on the target never sees a partial write.
    #[test]
    fn put_writes_the_target_and_no_temporary_file() {
        let root = tempfile::tempdir().expect("tempdir");
        let store = FileStore::new(root.path(), "/files");
        let path = storage_path(
            StorePrefix::Thumbnail(ThumbnailType::Large),
            "abc123",
            Some("jpg"),
        );
        store.put(b"thumb", &path, None).expect("put");
        assert_eq!(store.read(&path).expect("read"), b"thumb");
        let names: Vec<String> = std::fs::read_dir(root.path().join("large"))
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
    }

    /// A reader that fails part way leaves nothing at the target.
    #[test]
    fn a_failed_stream_leaves_no_file() {
        struct Failing(bool);
        impl Read for Failing {
            fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
                if std::mem::replace(&mut self.0, true) {
                    return Err(std::io::Error::other("source went away"));
                }
                buffer[0] = b'x';
                Ok(1)
            }
        }
        let root = tempfile::tempdir().expect("tempdir");
        let store = FileStore::new(root.path(), "/files");
        let result = store.put_stream(&mut Failing(false), None, "original/partial.bin", None);
        assert!(matches!(result, Err(StoreError::Io { .. })), "{result:?}");
        assert!(!store.exists("original/partial.bin").expect("exists"));
        assert_eq!(
            std::fs::read_dir(root.path().join("original"))
                .expect("directory")
                .count(),
            0,
            "no temporary sibling is left"
        );
    }

    /// A symlinked file or folder under a prefix is never listed.
    #[cfg(unix)]
    #[test]
    fn list_skips_symlinks() {
        let root = tempfile::tempdir().expect("tempdir");
        let outside = tempfile::tempdir().expect("outside");
        std::fs::write(outside.path().join("secret"), b"x").expect("secret");
        let store = FileStore::new(root.path(), "/files");
        store.put(b"x", "original/real.jpg", None).expect("put");
        let original = root.path().join("original");
        std::os::unix::fs::symlink(outside.path().join("secret"), original.join("link.jpg"))
            .expect("file link");
        std::os::unix::fs::symlink(outside.path(), root.path().join("asset")).expect("dir link");
        let listed: Vec<String> = store
            .list("original")
            .expect("list")
            .into_iter()
            .map(|file| file.storage_path)
            .collect();
        assert_eq!(listed, ["original/real.jpg"]);
        assert!(store.list("asset").expect("list asset").is_empty());
    }
}
