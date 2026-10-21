//! `romeka_media_extent`: what a IIIF canvas needs of an audio or video file.
//!
//! `media.data` is upstream's, so a duration and a frame size live here instead. One row per media,
//! written by the server's extent source after ffprobe answers, and read by every later manifest.
//! `storage_path` is the original the probe read; a row whose path no longer matches the media's is
//! stale, and the caller probes again rather than painting a canvas from another file.

use sqlx::{ColumnIndex, Decode, Encode, Executor, IntoArguments, Pool, Sqlite, Type};

use crate::DbError;
use crate::backend::Backend;
use crate::ids::MediaId;
use crate::time;

/// The extent of one media: a duration in milliseconds, and a frame size in pixels. `None` on a
/// field means the probe did not answer it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ExtentRow {
    pub duration_ms: Option<u64>,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

/// `romeka_media_extent` rows.
pub struct MediaExtentRepo<'a, DB: Backend = Sqlite> {
    pool: &'a Pool<DB>,
}

impl<'a, DB: Backend> MediaExtentRepo<'a, DB>
where
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
    usize: ColumnIndex<DB::Row>,
    for<'x> i64: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Option<i64>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> chrono::NaiveDateTime: Encode<'x, DB> + Type<DB>,
    for<'x> String: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> &'x str: Encode<'x, DB> + Type<DB> + ColumnIndex<DB::Row>,
{
    pub fn new(pool: &'a Pool<DB>) -> Self {
        Self { pool }
    }

    /// The extent recorded for `media` at `storage_path`; `None` when unprobed or stale.
    pub async fn get(
        &self,
        media: MediaId,
        storage_path: &str,
    ) -> Result<Option<ExtentRow>, DbError> {
        let row = sqlx::query::<DB>(
            "SELECT duration_ms, width, height FROM romeka_media_extent
             WHERE media_id = ? AND storage_path = ?",
        )
        .bind(super::bind_id(media))
        .bind(storage_path)
        .fetch_optional(self.pool)
        .await?;
        let Some(row) = row else {
            return Ok(None);
        };
        Ok(Some(ExtentRow {
            duration_ms: duration(DB::opt_int(&row, "duration_ms")?)?,
            width: dimension(DB::opt_int(&row, "width")?, "width")?,
            height: dimension(DB::opt_int(&row, "height")?, "height")?,
        }))
    }

    /// Record the extent of the original at `storage_path`, replacing an older row for the media.
    ///
    /// An update first and an insert only when no row exists, so two probes of one unprobed file
    /// both finish: the loser of the insert race updates the winner's row.
    pub async fn put(
        &self,
        media: MediaId,
        storage_path: &str,
        extent: &ExtentRow,
    ) -> Result<(), DbError> {
        let duration = signed(extent.duration_ms, "duration_ms")?;
        let width = signed(extent.width.map(u64::from), "width")?;
        let height = signed(extent.height.map(u64::from), "height")?;
        let now = time::now();
        let updated = sqlx::query::<DB>(
            "UPDATE romeka_media_extent
             SET storage_path = ?, probed = ?, duration_ms = ?, width = ?, height = ?
             WHERE media_id = ?",
        )
        .bind(storage_path)
        .bind(now)
        .bind(duration)
        .bind(width)
        .bind(height)
        .bind(super::bind_id(media))
        .execute(self.pool)
        .await?;
        if DB::rows_affected(&updated) > 0 {
            return Ok(());
        }
        let inserted = sqlx::query::<DB>(
            "INSERT INTO romeka_media_extent
                 (media_id, storage_path, probed, duration_ms, width, height)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(super::bind_id(media))
        .bind(storage_path)
        .bind(now)
        .bind(duration)
        .bind(width)
        .bind(height)
        .execute(self.pool)
        .await
        .map_err(DbError::from);
        match inserted {
            Ok(_) => Ok(()),
            Err(error) if error.is_unique_violation() => {
                sqlx::query::<DB>(
                    "UPDATE romeka_media_extent
                     SET storage_path = ?, probed = ?, duration_ms = ?, width = ?, height = ?
                     WHERE media_id = ?",
                )
                .bind(storage_path)
                .bind(now)
                .bind(duration)
                .bind(width)
                .bind(height)
                .bind(super::bind_id(media))
                .execute(self.pool)
                .await?;
                Ok(())
            }
            Err(error) => Err(error),
        }
    }
}

fn duration(value: Option<i64>) -> Result<Option<u64>, DbError> {
    value
        .map(|raw| {
            u64::try_from(raw).map_err(|_| DbError::Column {
                column: "duration_ms",
                reason: format!("{raw} is negative"),
            })
        })
        .transpose()
}

fn dimension(value: Option<i64>, column: &'static str) -> Result<Option<u32>, DbError> {
    value
        .map(|raw| {
            u32::try_from(raw).map_err(|_| DbError::Column {
                column,
                reason: format!("{raw} is not a pixel count"),
            })
        })
        .transpose()
}

fn signed(value: Option<u64>, column: &'static str) -> Result<Option<i64>, DbError> {
    value
        .map(|raw| {
            i64::try_from(raw).map_err(|_| DbError::Bind {
                reason: format!("{column} {raw} is beyond the signed range"),
            })
        })
        .transpose()
}
