//! `romeka media thumbnails`: backfill the thumbnails a media row is missing.
//!
//! The import command makes thumbnails for the files it stores. Rows a later
//! run finds already imported are skipped there, so a store that predates the
//! thumbnailer, or a run interrupted between the original and its thumbnails,
//! needs a separate pass over the rows that already exist. This command is that
//! pass.
//!
//! It walks every `media` row and hands each to the worker's own
//! [`make_preview`], so this pass and `Romeka\Job\MediaDerivatives` make the
//! same derivative with the same reason. An image type is read whole and
//! decoded. A type no decoder here reads (a video, a PDF, an AVIF, a HEIC) is
//! streamed out of the store into a temporary file in [`COPY_CHUNK`] pieces and
//! handed to `ffmpeg` or `pdftoppm`, so a 2 GB video is never in memory, and an
//! original with no local path is served the same way. Any other type is skipped
//! without a read. A type whose tool is absent is skipped with the tool's name in
//! the message, matching `TempFile::storeThumbnails`: the tools are optional at
//! runtime.
//!
//! A row with no original on disk fails; the run continues to the next row.
//!
//! Idempotent. The decision for each row is read from the store: when all three
//! thumbnail files are present and `--force` is absent, the row is skipped and
//! nothing is written, so a second run reports zero generated. `--force` removes
//! the three files first and rebuilds them.

use std::path::PathBuf;

use omeka_db::DbPool;
use omeka_db::Location;
use omeka_db::repos::{MediaFilter, MediaRepo, Paging};
use omeka_db::rows::MediaRow;
use omeka_db::with_db;
use omeka_media::derivative::{DerivativeWork, PreviewSource, make_preview};
use omeka_media::store::Store;

use crate::error::CliError;

/// How much of an original is copied out of the store at a time on its way to a
/// temporary file. Re-exported from the media crate, which owns the copy.
pub use omeka_media::derivative::COPY_CHUNK;

/// Input for `romeka media thumbnails`.
#[derive(Debug, Clone)]
pub struct ThumbnailOptions {
    /// The database.
    pub database: Location,
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
    let pool = options.database.open().await?;
    let store =
        crate::commands::file_store::open_store(&options.database, options.store.as_deref())?;
    let rows = with_db!(&pool, |db| MediaRepo::new(db)
        .list(&MediaFilter::default(), &Paging::default())
        .await)?;

    let mut report = ThumbnailReport::default();
    for row in rows {
        let result = backfill_row(&pool, store.as_ref(), &row, options.force).await;
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
    pool: &DbPool,
    store: &dyn Store,
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
    let source = PreviewSource {
        storage_id: row.storage_id.clone().unwrap_or_default(),
        extension: row.extension.clone(),
        media_type: row.media_type.clone(),
        has_original: row.has_original,
    };
    let work = make_preview(store, &source, force);
    match work {
        DerivativeWork::Ready { made, .. } => {
            if let Err(error) = mark_has_thumbnails(pool, row).await {
                return failed(format!(
                    "thumbnails are on disk but the row was not updated: {error}"
                ));
            }
            if made {
                ThumbnailResult {
                    media,
                    file,
                    status: ThumbnailStatus::Generated,
                    message: None,
                }
            } else {
                ThumbnailResult {
                    media,
                    file,
                    status: ThumbnailStatus::Skipped,
                    message: Some("the three thumbnails are already present".to_string()),
                }
            }
        }
        DerivativeWork::Skipped { reason, .. } => ThumbnailResult {
            media,
            file,
            status: ThumbnailStatus::Skipped,
            message: Some(reason),
        },
        DerivativeWork::Failed { reason, .. } => failed(reason),
    }
}

/// Set `media.has_thumbnails` when the files exist but the flag does not.
///
/// A no-op for a row that already carries the flag, so a repeated run writes
/// nothing.
async fn mark_has_thumbnails(pool: &DbPool, row: &MediaRow) -> Result<(), CliError> {
    if row.has_thumbnails {
        return Ok(());
    }
    let mut updated = row.clone();
    updated.has_thumbnails = true;
    if with_db!(pool, |db| MediaRepo::new(db).update(&updated).await)? {
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
    use omeka_media::file_render::ThumbnailType;
    use omeka_media::store::{FileStore, StorePrefix, storage_path};
    use omeka_media::thumbnailer::thumbnail_path;

    use crate::testing::{TempDb, png_rgb, seeded_db};

    /// Store an original and insert a media row that points at it, without
    /// thumbnails. This is the shape `romeka import media` leaves when the
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
        let original = storage_path(StorePrefix::Original, storage_id, Some(extension));
        store.put(bytes, &original, None).expect("store original");
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
            database: db.path.clone().into(),
            store: Some(store.to_path_buf()),
            force,
        }
    }

    fn thumbnail_files(store: &FileStore, storage_id: &str) -> Vec<bool> {
        ThumbnailType::ALL
            .iter()
            .map(|kind| store.exists(&thumbnail_path(storage_id, *kind)).unwrap())
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

    /// A row whose type no decoder or tool reads is skipped, not failed, before
    /// its original is read, and no file is written.
    #[tokio::test]
    async fn a_type_no_tool_reads_is_skipped() {
        let db = seeded_db().await;
        let store_dir = tempfile::tempdir().expect("store dir");
        let store = FileStore::new(store_dir.path(), "/files");
        row_without_thumbnails(
            &db,
            &store,
            "dddd4444",
            "docx",
            "application/msword",
            b"not a thumbnail source",
        )
        .await;

        let report = run(&options(&db, store_dir.path(), false))
            .await
            .expect("backfill");

        assert_eq!(report.skipped, 1, "{report:?}");
        assert_eq!(report.generated, 0);
        assert_eq!(report.failed, 0);
        let message = report.results[0].message.as_deref().unwrap_or_default();
        assert!(message.contains("application/msword"), "{message}");
        assert_eq!(
            thumbnail_files(&store, "dddd4444"),
            vec![false, false, false]
        );
    }

    /// A video row is streamed out of the store and its poster becomes the three
    /// derivatives, so the flag flips as it does for an image.
    #[tokio::test]
    async fn a_video_row_gets_a_poster() {
        let db = seeded_db().await;
        let store_dir = tempfile::tempdir().expect("store dir");
        let store = FileStore::new(store_dir.path(), "/files");
        let video = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/media-kinds/video.mp4"
        ))
        .expect("the corpus video");
        let media =
            row_without_thumbnails(&db, &store, "gggg7777", "mp4", "video/mp4", &video).await;

        let report = run(&options(&db, store_dir.path(), false))
            .await
            .expect("backfill");

        assert_eq!(report.generated, 1, "{report:?}");
        assert_eq!(report.failed, 0, "{report:?}");
        assert_eq!(thumbnail_files(&store, "gggg7777"), vec![true, true, true]);
        let row = MediaRepo::new(&db.pool)
            .get(omeka_db::ids::MediaId::from(media))
            .await
            .expect("get")
            .expect("row");
        assert!(row.has_thumbnails);
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
            .delete(&storage_path(
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
