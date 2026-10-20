//! Where a command finds the file store: `--store`, else `files` beside the
//! database.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use omeka_media::store::Store;

/// The store a command reads and writes, rooted at [`store_root`].
pub(crate) fn open_store(database: &Path, store: Option<&Path>) -> Arc<dyn Store> {
    omeka_server::file_store(store_root(database, store))
}

/// The store root: the explicit `--store`, else `files` beside the database.
pub(crate) fn store_root(database: &Path, store: Option<&Path>) -> PathBuf {
    store.map_or_else(
        || {
            let parent = database
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
                .unwrap_or(Path::new("."));
            parent.join("files")
        },
        Path::to_path_buf,
    )
}
