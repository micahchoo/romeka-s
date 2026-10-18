//! `omeka media thumbnails`: backfill the thumbnails a media row is missing.
//!
//! The import command makes thumbnails for the files it stores. Rows a later
//! run finds already imported are skipped there, so a store that predates the
//! thumbnailer, or a run interrupted between the original and its thumbnails,
//! needs a separate pass over the rows that already exist. This command is that
//! pass.
//!
//! It walks every `media` row and, for each, reads the original back from the
//! store (`original/<storage_id>.<extension>`), derives the three sizes through
//! [`generate_thumbnails`], and records the result. A row whose media type GD
//! does not decode is skipped, not failed, matching `TempFile::storeThumbnails`.
//! A row with no original on disk fails; the run continues to the next row.
//!
//! Idempotent. The decision for each row is read from the store: when all three
//! thumbnail files are present and `--force` is absent, the row is skipped and
//! nothing is written, so a second run reports zero generated. `--force` removes
//! the three files first and rebuilds them.

use std::path::PathBuf;

use sqlx::SqlitePool;

use omeka_db::open_file;
use omeka_db::repos::{MediaFilter, MediaRepo, Paging};
use omeka_db::rows::MediaRow;
use omeka_media::file_render::ThumbnailType;
use omeka_media::store::{FileStore, StorePrefix};
use omeka_media::thumbnailer::{ThumbnailOutcome, generate_thumbnails, thumbnail_path};

use crate::error::CliError;

/// Input for `omeka media thumbnails`.
#[derive(Debug, Clone)]
pub struct ThumbnailOptions {
    /// SQLite database file.
    pub database: PathBuf,
    /// Store root (`file_store.local.base_path`). Defaults to `files` beside the database.
    pub store: Option<PathBuf>,
    /// Rebuild thumbnails that already exist instead of leaving them.
    pub force: bool,
}

/// What happened to one media row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThumbnailStatus {
    /// The three thumbnails were written.
    Generated,
    /// Nothing was written: the files were present, or the row is not an image.
    Skipped,
    /// The row could not be backfilled; `message` says why.
    Failed,
}

/// The outcome of one media row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThumbnailResult {
    /// The media id.
    pub media: u64,
    /// The row's source file name, when it has one.
    pub file: Option<String>,
    /// What happened.
    pub status: ThumbnailStatus,
    /// The skip or failure reason, when there is one.
    pub message: Option<String>,
}

/// The outcome of a thumbnail backfill.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ThumbnailReport {
    /// One result per media row, in row order.
    pub results: Vec<ThumbnailResult>,
    /// Rows whose three thumbnails were written.
    pub generated: usize,
    /// Rows left alone.
    pub skipped: usize,
    /// Rows that failed.
    pub failed: usize,
}

/// Generate the thumbnails every media row is missing.
///
/// # Errors
/// A [`CliError`] when the database cannot be opened or the media rows cannot
/// be listed. A row that cannot be backfilled is reported in the
/// [`ThumbnailReport`], not here.
pub async fn run(options: &ThumbnailOptions) -> Result<ThumbnailReport, CliError> {
    let pool = open_file(&options.database).await?;
    let store = FileStore::new(
        crate::commands::media::store_root(&options.database, options.store.as_deref()),
        "/files",
    );
    let rows = MediaRepo::new(&pool)
        .list(&MediaFilter::default(), &Paging::default())
        .await?;

    let mut report = ThumbnailReport::default();
    for row in rows {
        let result = backfill_row(&pool, &store, &row, options.force).await;
        match result.status {
            ThumbnailStatus::Generated => report.generated += 1,
            ThumbnailStatus::Skipped => report.skipped += 1,
            ThumbnailStatus::Failed => report.failed += 1,
        }
        report.results.push(result);
    }
    Ok(report)
}

/// Backfill one row. Failures come back in the result, so one bad row does not
/// stop the run.
async fn backfill_row(
    pool: &SqlitePool,
    store: &FileStore,
    row: &MediaRow,
    force: bool,
) -> ThumbnailResult {
    let media = row.id().0;
    let file = row.source.clone();
    let failed = |message: String| ThumbnailResult {
        media,
        file: file.clone(),
        status: ThumbnailStatus::Failed,
        message: Some(message),
    };

    let Some(storage_id) = row.storage_id.clone() else {
        return failed("the row has no storage id, so it has no original to read".to_string());
    };
    let original_path =
        FileStore::storage_path(StorePrefix::Original, &storage_id, row.extension.as_deref());
    let original = match store.read(&original_path) {
        Ok(bytes) => bytes,
        Err(error) => return failed(format!("cannot read {original_path}: {error}")),
    };

    if force {
        for kind in ThumbnailType::ALL {
            if let Err(error) = store.delete(&thumbnail_path(&storage_id, kind)) {
                return failed(format!("cannot remove an existing thumbnail: {error}"));
            }
        }
    }

    match generate_thumbnails(store, &storage_id, &original, row.media_type.as_deref()) {
        Ok(ThumbnailOutcome::Generated) => {
            if let Err(error) = mark_has_thumbnails(pool, row).await {
                return failed(format!(
                    "thumbnails are on disk but the row was not updated: {error}"
                ));
            }
            ThumbnailResult {
                media,
                file,
                status: ThumbnailStatus::Generated,
                message: None,
            }
        }
        Ok(ThumbnailOutcome::Existing) => {
            if let Err(error) = mark_has_thumbnails(pool, row).await {
                return failed(format!(
                    "thumbnails are on disk but the row was not updated: {error}"
                ));
            }
            ThumbnailResult {
                media,
                file,
                status: ThumbnailStatus::Skipped,
                message: Some("the three thumbnails are already present".to_string()),
            }
        }
        Ok(ThumbnailOutcome::Skipped(skip)) => ThumbnailResult {
            media,
            file,
            status: ThumbnailStatus::Skipped,
            message: Some(skip.to_string()),
        },
        Err(error) => failed(format!("cannot write a thumbnail: {error}")),
    }
}

/// Set `media.has_thumbnails` when the files exist but the flag does not.
///
/// A no-op for a row that already carries the flag, so a repeated run writes
/// nothing.
async fn mark_has_thumbnails(pool: &SqlitePool, row: &MediaRow) -> Result<(), CliError> {
    if row.has_thumbnails {
        return Ok(());
    }
    let mut updated = row.clone();
    updated.has_thumbnails = true;
    if MediaRepo::new(pool).update(&updated).await? {
        Ok(())
    } else {
        Err(CliError::message(
            "the media row was removed before it could be updated",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use omeka_db::repos::ItemRepo;
    use omeka_db::rows::{NewItem, NewMedia, NewResource};

    use crate::testing::{TempDb, png_rgb, seeded_db};

    /// Store an original and insert a media row that points at it, without
    /// thumbnails. This is the shape `omeka import media` leaves when the
    /// thumbnailer did not run.
    async fn row_without_thumbnails(
        db: &TempDb,
        store: &FileStore,
        storage_id: &str,
        extension: &str,
        media_type: &str,
        bytes: &[u8],
    ) -> u64 {
        let item = ItemRepo::new(&db.pool)
            .create(&NewItem::default())
            .await
            .expect("item")
            .id();
        let original = FileStore::storage_path(StorePrefix::Original, storage_id, Some(extension));
        store.put(bytes, &original).expect("store original");
        let row = MediaRepo::new(&db.pool)
            .create(&NewMedia {
                resource: NewResource::default(),
                item_id: item,
                ingester: "upload".to_string(),
                renderer: "file".to_string(),
                data: None,
                source: Some(format!("{storage_id}.{extension}")),
                media_type: Some(media_type.to_string()),
                storage_id: Some(storage_id.to_string()),
                extension: Some(extension.to_string()),
                sha256: Some(String::new()),
                size: Some(bytes.len() as i64),
                has_original: true,
                has_thumbnails: false,
                position: None,
                lang: None,
                alt_text: None,
            })
            .await
            .expect("media row");
        row.id().0
    }

    fn options(db: &TempDb, store: &std::path::Path, force: bool) -> ThumbnailOptions {
        ThumbnailOptions {
            database: db.path.clone(),
            store: Some(store.to_path_buf()),
            force,
        }
    }

    fn thumbnail_files(store: &FileStore, storage_id: &str) -> Vec<bool> {
        ThumbnailType::ALL
            .iter()
            .map(|kind| store.exists(&thumbnail_path(storage_id, *kind)))
            .collect()
    }

    /// A row without thumbnails gets all three, and the row's flag flips.
    #[tokio::test]
    async fn backfill_creates_the_missing_thumbnails() {
        let db = seeded_db().await;
        let store_dir = tempfile::tempdir().expect("store dir");
        let store = FileStore::new(store_dir.path(), "/files");
        let media = row_without_thumbnails(
            &db,
            &store,
            "aaaa1111",
            "png",
            "image/png",
            &png_rgb(64, 48),
        )
        .await;

        let report = run(&options(&db, store_dir.path(), false))
            .await
            .expect("backfill");

        assert_eq!(report.generated, 1, "{report:?}");
        assert_eq!(report.skipped, 0);
        assert_eq!(report.failed, 0, "{report:?}");
        assert_eq!(thumbnail_files(&store, "aaaa1111"), vec![true, true, true]);
        let row = MediaRepo::new(&db.pool)
            .get(omeka_db::ids::MediaId::from(media))
            .await
            .expect("get")
            .expect("row");
        assert!(row.has_thumbnails);
    }

    /// The second run finds all three present and writes nothing.
    #[tokio::test]
    async fn a_second_run_generates_nothing() {
        let db = seeded_db().await;
        let store_dir = tempfile::tempdir().expect("store dir");
        let store = FileStore::new(store_dir.path(), "/files");
        row_without_thumbnails(
            &db,
            &store,
            "bbbb2222",
            "png",
            "image/png",
            &png_rgb(32, 32),
        )
        .await;
        let opts = options(&db, store_dir.path(), false);

        let first = run(&opts).await.expect("first run");
        assert_eq!(first.generated, 1);
        let second = run(&opts).await.expect("second run");

        assert_eq!(second.generated, 0, "{second:?}");
        assert_eq!(second.skipped, 1);
        assert_eq!(second.failed, 0);
        assert_eq!(second.results[0].status, ThumbnailStatus::Skipped);
    }

    /// `--force` rebuilds thumbnails that already exist.
    #[tokio::test]
    async fn force_rebuilds_existing_thumbnails() {
        let db = seeded_db().await;
        let store_dir = tempfile::tempdir().expect("store dir");
        let store = FileStore::new(store_dir.path(), "/files");
        row_without_thumbnails(
            &db,
            &store,
            "cccc3333",
            "png",
            "image/png",
            &png_rgb(20, 20),
        )
        .await;
        run(&options(&db, store_dir.path(), false))
            .await
            .expect("first run");

        let forced = run(&options(&db, store_dir.path(), true))
            .await
            .expect("forced run");

        assert_eq!(forced.generated, 1, "{forced:?}");
        assert_eq!(thumbnail_files(&store, "cccc3333"), vec![true, true, true]);
    }

    /// A row whose bytes are not an image is skipped, not failed, and no file is
    /// written.
    #[tokio::test]
    async fn a_non_image_is_skipped() {
        let db = seeded_db().await;
        let store_dir = tempfile::tempdir().expect("store dir");
        let store = FileStore::new(store_dir.path(), "/files");
        row_without_thumbnails(
            &db,
            &store,
            "dddd4444",
            "pdf",
            "application/pdf",
            b"%PDF-1.4 not an image",
        )
        .await;

        let report = run(&options(&db, store_dir.path(), false))
            .await
            .expect("backfill");

        assert_eq!(report.skipped, 1, "{report:?}");
        assert_eq!(report.generated, 0);
        assert_eq!(report.failed, 0);
        assert_eq!(
            thumbnail_files(&store, "dddd4444"),
            vec![false, false, false]
        );
    }

    /// A row whose original is missing fails, and the run still backfills the
    /// rows after it.
    #[tokio::test]
    async fn a_missing_original_fails_and_the_run_continues() {
        let db = seeded_db().await;
        let store_dir = tempfile::tempdir().expect("store dir");
        let store = FileStore::new(store_dir.path(), "/files");
        row_without_thumbnails(
            &db,
            &store,
            "eeee5555",
            "png",
            "image/png",
            &png_rgb(16, 16),
        )
        .await;
        store
            .delete(&FileStore::storage_path(
                StorePrefix::Original,
                "eeee5555",
                Some("png"),
            ))
            .expect("remove original");
        row_without_thumbnails(
            &db,
            &store,
            "ffff6666",
            "png",
            "image/png",
            &png_rgb(16, 16),
        )
        .await;

        let report = run(&options(&db, store_dir.path(), false))
            .await
            .expect("backfill");

        assert_eq!(report.failed, 1, "{report:?}");
        assert_eq!(report.generated, 1);
        assert_eq!(thumbnail_files(&store, "ffff6666"), vec![true, true, true]);
    }
}
