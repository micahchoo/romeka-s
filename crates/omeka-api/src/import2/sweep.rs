//! The staging sweep: uploaded files removed once no import needs them
//! (`docs/import-v2.md`, "Arrival", "Retention").
//!
//! The rules, in [`due`]:
//!
//! - an import `done` or `undone` loses its staging area
//!   `import_staging_keep_days` (30) after it reached that state;
//! - an import whose last run or undo failed (`run_failed`, `undo_failed`,
//!   `partly_undone`) loses it the same number of days after that last
//!   attempt ended;
//! - a draft loses it `import_draft_staging_keep_days` (7) after it was last
//!   worked on;
//! - an upload still unfinished 24 hours after it began is removed, whatever
//!   its import.
//!
//! An import that is running or undoing keeps everything. The rows hold no
//! time a plan was saved, so a draft's last work is the newest of its
//! creation and its uploads; a draft whose plan is edited every day but that
//! gets no new file is swept after 7 days all the same.
//!
//! The clock is a parameter, so a test moves it.

use std::sync::Arc;

use chrono::{NaiveDateTime, TimeDelta};
use omeka_db::Backend;
use omeka_db::DbError;
use omeka_db::ids::ImportId;
use omeka_db::repos::{ImportRepo, UploadRepo};
use omeka_db::rows::{ImportRow, ImportStatus, UploadRow};
use omeka_media::store::Store;
use sqlx::{ColumnIndex, Decode, Encode, Executor, IntoArguments, Pool, Type};

use super::staging;

/// The job class of the sweep, `job.class`; it takes no args.
pub const SWEEP_JOB: &str = "Romeka\\Job\\ImportSweep";

/// How long an unfinished upload may stay.
pub const UNFINISHED_KEEP: TimeDelta = TimeDelta::hours(24);

/// How long staging stays, in days.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Retention {
    pub keep_days: u64,
    pub draft_keep_days: u64,
}

impl From<crate::settings::UploadSettings> for Retention {
    fn from(settings: crate::settings::UploadSettings) -> Self {
        Self {
            keep_days: settings.staging_keep_days,
            draft_keep_days: settings.draft_staging_keep_days,
        }
    }
}

/// What the sweep removes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Due {
    /// The whole staging area of an import.
    Staging(ImportId),
    /// One unfinished upload.
    Upload(ImportId, String),
}

fn days(count: u64) -> TimeDelta {
    TimeDelta::days(i64::try_from(count).unwrap_or(i64::MAX / 86_400 / 1_000))
}

/// When an import's staging area goes, or `None` while it is kept.
#[must_use]
pub fn kept_until(
    import: &ImportRow,
    uploads: &[UploadRow],
    retention: Retention,
) -> Option<NaiveDateTime> {
    match import.status {
        ImportStatus::Running | ImportStatus::Undoing => None,
        ImportStatus::Draft => {
            let last = uploads
                .iter()
                .map(|upload| upload.created)
                .chain([import.created])
                .max()?;
            last.checked_add_signed(days(retention.draft_keep_days))
        }
        ImportStatus::Done
        | ImportStatus::Undone
        | ImportStatus::RunFailed
        | ImportStatus::UndoFailed
        | ImportStatus::PartlyUndone => import
            .finished
            .unwrap_or(import.created)
            .checked_add_signed(days(retention.keep_days)),
    }
}

/// What the sweep removes at `now`, for the imports that hold uploads, each
/// with its uploads.
#[must_use]
pub fn due(
    imports: &[(ImportRow, Vec<UploadRow>)],
    now: NaiveDateTime,
    retention: Retention,
) -> Vec<Due> {
    let mut found = Vec::new();
    for (import, uploads) in imports {
        if kept_until(import, uploads, retention).is_some_and(|until| until <= now) {
            found.push(Due::Staging(import.id));
            continue;
        }
        if matches!(import.status, ImportStatus::Running | ImportStatus::Undoing) {
            continue;
        }
        for upload in uploads {
            if !upload.is_complete() && upload.created + UNFINISHED_KEEP <= now {
                found.push(Due::Upload(import.id, upload.id.clone()));
            }
        }
    }
    found
}

/// What one sweep removed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Swept {
    /// Imports whose whole staging area went.
    pub imports: usize,
    /// Unfinished uploads removed on their own.
    pub uploads: usize,
}

/// Remove what [`due`] names at `now`.
///
/// # Errors
/// [`DbError`] when the rows cannot be read or removed.
pub async fn sweep<DB>(
    pool: &Pool<DB>,
    store: &Arc<dyn Store>,
    now: NaiveDateTime,
    retention: Retention,
) -> Result<Swept, DbError>
where
    DB: Backend,
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
    usize: ColumnIndex<DB::Row>,
    for<'x> i64: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Option<i64>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> String: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Option<String>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> &'x str: Encode<'x, DB> + Type<DB> + ColumnIndex<DB::Row>,
{
    let uploads = UploadRepo::new(pool);
    let imports = ImportRepo::new(pool);
    let mut held = Vec::new();
    for id in uploads.imports().await? {
        if let Some(import) = imports.get(id).await? {
            held.push((import, uploads.list(id).await?));
        }
    }
    let mut swept = Swept::default();
    for due in due(&held, now, retention) {
        match due {
            Due::Staging(import) => {
                staging::remove_all(pool, store, import).await?;
                swept.imports += 1;
            }
            Due::Upload(import, upload) => {
                staging::remove(pool, store, import, &upload).await?;
                swept.uploads += 1;
            }
        }
    }
    Ok(swept)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]

    use chrono::NaiveDate;
    use omeka_db::repos::UploadRepo;
    use serde_json::json;

    use super::*;
    use crate::import2::staging::tests::{LIMITS, body, site};

    const RETENTION: Retention = Retention {
        keep_days: 30,
        draft_keep_days: 7,
    };

    fn at(day: u32, hour: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 10, day)
            .and_then(|date| date.and_hms_opt(hour, 0, 0))
            .unwrap()
    }

    fn import(
        id: u64,
        status: ImportStatus,
        created: NaiveDateTime,
        finished: Option<NaiveDateTime>,
    ) -> ImportRow {
        ImportRow {
            id: ImportId(id),
            owner_id: None,
            sources: json!([]),
            plan: json!({}),
            ran_plan: None,
            revision: 0,
            status,
            job_id: None,
            summary: None,
            stop_requested: false,
            copied_from: None,
            created,
            finished,
        }
    }

    fn upload(import: u64, id: &str, created: NaiveDateTime, complete: bool) -> UploadRow {
        UploadRow {
            import_id: ImportId(import),
            id: id.to_string(),
            path: format!("{id}.jpg"),
            length: 4,
            received: if complete { 4 } else { 1 },
            created,
        }
    }

    /// Each rule at its edge, with the clock where the test puts it.
    #[test]
    fn the_rules_remove_exactly_what_is_due() {
        let now = at(31, 12);
        let held = vec![
            // done on the 1st at 12:00: 30 days are up at 12:00 on the 31st.
            (
                import(1, ImportStatus::Done, at(1, 9), Some(at(1, 12))),
                vec![upload(1, "a", at(1, 9), true)],
            ),
            // undone on the 1st at 13:00: one hour still to go.
            (
                import(2, ImportStatus::Undone, at(1, 9), Some(at(1, 13))),
                vec![upload(2, "b", at(1, 9), true)],
            ),
            // the last failed attempt ended on the 1st at 11:00.
            (
                import(3, ImportStatus::RunFailed, at(1, 9), Some(at(1, 11))),
                vec![upload(3, "c", at(1, 9), true)],
            ),
            // a draft last worked on the 24th at 12:00: 7 days are up.
            (
                import(4, ImportStatus::Draft, at(20, 9), None),
                vec![upload(4, "d", at(24, 12), true)],
            ),
            // a draft with a file from the 25th: kept, but its upload that
            // began 25 hours ago and never finished goes.
            (
                import(5, ImportStatus::Draft, at(20, 9), None),
                vec![
                    upload(5, "e", at(25, 12), true),
                    upload(5, "f", at(30, 11), false),
                    upload(5, "g", at(30, 13), false),
                ],
            ),
            // running: nothing goes, not even an old unfinished upload.
            (
                import(6, ImportStatus::Running, at(1, 9), None),
                vec![upload(6, "h", at(1, 9), false)],
            ),
            (
                import(7, ImportStatus::PartlyUndone, at(1, 9), Some(at(2, 9))),
                vec![upload(7, "i", at(1, 9), true)],
            ),
        ];
        assert_eq!(
            due(&held, now, RETENTION),
            [
                Due::Staging(ImportId(1)),
                Due::Staging(ImportId(3)),
                Due::Staging(ImportId(4)),
                Due::Upload(ImportId(5), "f".into()),
            ]
        );
        assert_eq!(
            kept_until(&held[4].0, &held[4].1, RETENTION),
            Some(at(30, 13) + TimeDelta::days(7)),
            "a draft's date moves with its newest upload"
        );
        assert_eq!(kept_until(&held[5].0, &held[5].1, RETENTION), None);
    }

    /// The sweep on a real staging area, with the clock moved forward.
    #[tokio::test]
    async fn a_sweep_removes_the_old_draft_files_and_the_unfinished_upload() {
        let site = site().await;
        let finished = site.upload("a.jpg", b"aaaa").await;
        let open = crate::import2::staging::create(&site.pool, site.import, "b.jpg", 4, LIMITS)
            .await
            .unwrap();
        crate::import2::staging::append(
            &site.pool,
            &site.store,
            site.import,
            &open.id,
            0,
            body(b"b"),
        )
        .await
        .unwrap();
        let now = chrono::Utc::now().naive_utc();

        let early = sweep(
            &site.pool,
            &site.store,
            now + TimeDelta::hours(23),
            RETENTION,
        )
        .await
        .unwrap();
        assert_eq!(early, Swept::default(), "nothing is due yet");

        let next_day = sweep(
            &site.pool,
            &site.store,
            now + TimeDelta::hours(25),
            RETENTION,
        )
        .await
        .unwrap();
        assert_eq!(
            next_day,
            Swept {
                imports: 0,
                uploads: 1
            }
        );
        let rows = UploadRepo::new(&site.pool).list(site.import).await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, finished.id);

        let week = sweep(&site.pool, &site.store, now + TimeDelta::days(8), RETENTION)
            .await
            .unwrap();
        assert_eq!(
            week,
            Swept {
                imports: 1,
                uploads: 0
            }
        );
        assert!(
            UploadRepo::new(&site.pool)
                .list(site.import)
                .await
                .unwrap()
                .is_empty()
        );
        assert!(
            !site
                .store
                .exists(&crate::import2::staging::file_path(
                    site.import,
                    &finished.id
                ))
                .unwrap()
        );
    }
}
