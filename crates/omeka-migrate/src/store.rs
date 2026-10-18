//! The asset file copy: the bytes behind an `asset` row.
//!
//! An upstream `asset` row names a file under `files/asset/<storage_id>[.<extension>]`
//! (`File/TempFile.php::storeAsset`, `File/Store/Local.php`). The row moves through
//! `AssetRepo::create`; this module moves the file, from the source install's `files` directory
//! into the target store, so a migrated thumbnail points at a file the port serves.

use std::path::{Path, PathBuf};

use omeka_db::rows::AssetRow;
use omeka_media::store::{FileStore, StorePrefix};

use crate::error::MigrateError;

/// Copy every asset row's stored file from the source store into the target store.
///
/// The storage path is `asset/<storage_id>[.<extension>]`, which [`FileStore::storage_path`]
/// builds. A source asset whose file is not in the source store stops the run by name, because a
/// silent miss would leave a migrated thumbnail pointing at nothing.
pub(crate) fn copy_asset_files(
    assets: &[AssetRow],
    source_root: &Path,
    target_root: &Path,
) -> Result<(), MigrateError> {
    let source = FileStore::new(source_root, "/files");
    let target = FileStore::new(target_root, "/files");
    for asset in assets {
        let storage_path = FileStore::storage_path(
            StorePrefix::Asset,
            &asset.storage_id,
            asset.extension.as_deref(),
        );
        if !source.exists(&storage_path) {
            return Err(MigrateError::MissingAssetFile {
                storage_id: asset.storage_id.clone(),
                path: source.local_path(&storage_path)?,
            });
        }
        target.put_atomic(&source.read(&storage_path)?, &storage_path)?;
    }
    Ok(())
}

/// The target store root: `target_files` when the caller names one, else `files` beside the target
/// database, the same default `romeka import media` uses.
pub(crate) fn target_store_root(target_path: &Path, target_files: Option<&Path>) -> PathBuf {
    match target_files {
        Some(root) => root.to_path_buf(),
        None => target_path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or(Path::new("."))
            .join("files"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use omeka_db::ids::{AssetId, UserId};

    fn asset(id: u64, storage_id: &str) -> AssetRow {
        AssetRow {
            id: AssetId(id),
            owner_id: Some(UserId(2)),
            name: "logo.png".to_string(),
            media_type: "image/png".to_string(),
            storage_id: storage_id.to_string(),
            extension: Some("png".to_string()),
            alt_text: None,
        }
    }

    fn temp_root(tag: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_nanos());
        std::env::temp_dir().join(format!(
            "romeka-migrate-{}-{tag}-{nanos}",
            std::process::id()
        ))
    }

    #[test]
    fn asset_files_copy_from_the_source_store_into_the_target_store() {
        let root = temp_root("assets");
        let source = root.join("source");
        let target = root.join("target");
        let storage = "a1b2c3d4e5f60718293a4b5c6d7e8f9012345678";
        std::fs::create_dir_all(source.join("asset")).expect("create the source store");
        std::fs::write(source.join(format!("asset/{storage}.png")), b"png bytes")
            .expect("write the source asset");

        copy_asset_files(&[asset(77, storage)], &source, &target).expect("copy the asset files");

        assert_eq!(
            std::fs::read(target.join(format!("asset/{storage}.png"))).expect("read the copy"),
            b"png bytes",
            "the asset file is byte-for-byte in the target store"
        );
        std::fs::remove_dir_all(root).expect("clean up");
    }

    #[test]
    fn a_missing_asset_file_names_the_asset_it_could_not_find() {
        let root = temp_root("missing-asset");
        let storage = "a1b2c3d4e5f60718293a4b5c6d7e8f9012345678";

        let error = copy_asset_files(
            &[asset(77, storage)],
            &root.join("source"),
            &root.join("target"),
        )
        .expect_err("a missing source file stops the copy");

        assert!(
            matches!(error, MigrateError::MissingAssetFile { ref storage_id, .. } if storage_id == storage),
            "the error names the asset: {error:?}"
        );
        assert!(
            !root.join("target").exists(),
            "the target store was not created"
        );
        let _ = std::fs::remove_dir_all(root);
    }
}
