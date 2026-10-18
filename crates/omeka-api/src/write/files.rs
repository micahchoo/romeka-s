//! The stored bytes one media row owns.
//!
//! `Module::deleteMediaFiles` listens on `Omeka\Entity\Media`
//! `entity.remove.post` and deletes `original/<filename>` when the media has an
//! original, then `<type>/<storage_id>.jpg` for every thumbnail type when it
//! has thumbnails (`application/Module.php:323`). The port deletes the bytes
//! before the row, so a filesystem failure answers 500 with the row intact
//! rather than leaving bytes behind a row that is already gone.

use omeka_db::rows::MediaRow;
use omeka_media::file_render::ThumbnailType;
use omeka_media::store::{FileStore, StorePrefix};

use crate::errors::ApiError;

/// Delete every stored file `row` names. A `None` store means the process
/// carries no file store, so there is nothing to reach.
pub fn delete_media_files(store: Option<&FileStore>, row: &MediaRow) -> Result<(), ApiError> {
    let Some(store) = store else {
        return Ok(());
    };
    if row.has_original
        && let Some(name) = filename(row.storage_id.as_deref(), row.extension.as_deref())
    {
        store
            .delete(&FileStore::storage_path(StorePrefix::Original, &name, None))
            .map_err(ApiError::internal)?;
    }
    if row.has_thumbnails
        && let Some(storage_id) = row.storage_id.as_deref()
    {
        for kind in ThumbnailType::ALL {
            store
                .delete(&FileStore::storage_path(
                    StorePrefix::Thumbnail(kind),
                    storage_id,
                    Some("jpg"),
                ))
                .map_err(ApiError::internal)?;
        }
    }
    Ok(())
}

/// `Entity\Media::getFilename`: the storage id plus the extension when there is
/// one. The result is the store's file name, so it already carries the
/// extension the storage path would add.
fn filename(storage_id: Option<&str>, extension: Option<&str>) -> Option<String> {
    let storage = storage_id?;
    Some(match extension {
        Some(extension) if !extension.is_empty() => format!("{storage}.{extension}"),
        _ => storage.to_string(),
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn the_filename_is_the_storage_id_plus_a_non_empty_extension() {
        assert_eq!(
            filename(Some("abc"), Some("jpg")).as_deref(),
            Some("abc.jpg")
        );
        assert_eq!(filename(Some("abc"), None).as_deref(), Some("abc"));
        assert_eq!(filename(Some("abc"), Some("")).as_deref(), Some("abc"));
        assert_eq!(filename(None, Some("jpg")), None);
    }
}
