//! Where a command finds the file store: `--store`, else `files` beside the
//! database.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use omeka_media::store::Store;

use crate::CliError;

/// The store a command reads and writes: `ROMEKA_STORE_BACKEND`'s choice, with the
/// local folder at [`store_root`].
pub(crate) fn open_store(
    database: &Path,
    store: Option<&Path>,
) -> Result<Arc<dyn Store>, CliError> {
    omeka_server::file_store(store_root(database, store))
        .map_err(|error| CliError::message(error.to_string()))
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
