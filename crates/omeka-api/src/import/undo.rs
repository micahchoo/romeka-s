//! Undo: put the install back as it was before one import, from what its
//! ledger says (`docs/import-v2.md`, "Lifecycle and the ledger", "Undo").
//!
//! [`undo_import`] is the body of the [`UNDO_JOB`] job. The route already
//! moved the import to `undoing`. The job reads the ledger once and deals
//! with each row in this order:
//!
//! 1. the changed resources, restored from their
//!    before-images ([`ImportWrites::restore`]);
//! 2. the created media, then items, then item sets, deleted through the
//!    API's delete paths ([`remove`]), so a media's files leave the store;
//! 3. the created item types and fields, deleted when nothing uses them.
//!
//! A resource [`touched`] since the import wrote it is kept, and listed with
//! what touched it, unless the job's `force` names it or says `all`. A media
//! whose item is kept is kept too, and so is a set that holds a kept item. A
//! resource already gone is skipped and
//! listed. Each row leaves the ledger when its resource is dealt with: a
//! restore forgets its row and image in the restore's own transaction, so a
//! rerun of an interrupted undo never sees the restore as a later change.
//! What stays in the ledger is what the undo kept, so a second undo with
//! `force` can finish it.
//!
//! After each resource the job proves it is alive, stores its report in the
//! summary's `undo` member, and reads the stop request. The import ends
//! `undone` when its ledger is empty, `partly_undone` when the undo kept
//! touched resources, and `undo_failed` when it stopped on request or on an
//! error. [`preview_undo`] answers what an undo would do, by the same
//! [`judge`], without writing.

mod judge;
mod preview;
mod remove;
mod report;

use std::collections::HashMap;

use omeka_db::Backend;
use omeka_db::DbError;
use omeka_db::ids::{ImportId, JobId};
use omeka_db::repos::{BeforeImageRepo, ImportRepo, ImportResourceRepo, ImportWrites};
use omeka_db::rows::{
    BeforeImage, ImportChange, ImportEnding, ImportKind, ImportResource, ImportResourceRow,
    ImportStatus, ImportTarget,
};
use serde_json::{Map, Value};
use sqlx::{ColumnIndex, Decode, Encode, Executor, IntoArguments, Pool, Type};

pub use preview::{UndoPreview, preview_undo};
pub use remove::{ApiRemoval, Removal, Removed};
pub use report::{Counts, Force, Kept, Named, Progress, Reason, Stopped, UndoReport};

use self::judge::{Judge, Verdict};
use super::touched::Touch;
use crate::messages::{self, ServerMessage};

/// The job class of an undo; its args are `{"importId": n, "force"?: ...}`.
pub const UNDO_JOB: &str = "Romeka\\Job\\ImportUndo";

/// The summary member that holds the undo's report, beside the run's.
pub const UNDO_MEMBER: &str = "undo";

/// The states an undo may start from, as `ImportRepo::start_undo` reads them.
#[must_use]
pub fn may_undo(status: ImportStatus) -> bool {
    matches!(
        status,
        ImportStatus::Done
            | ImportStatus::RunFailed
            | ImportStatus::UndoFailed
            | ImportStatus::PartlyUndone
    )
}

/// How an undo ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UndoEnd {
    /// The ledger is empty.
    Undone(UndoReport),
    /// Touched resources were kept; a new undo with `force` can finish it.
    PartlyUndone(UndoReport),
    /// The undo stopped on request or on an error; a new undo goes on.
    Failed(UndoReport),
    /// Another worker owns the import now.
    Lost,
}

#[derive(Debug, thiserror::Error)]
pub enum UndoError {
    #[error(transparent)]
    Db(#[from] DbError),
    #[error(transparent)]
    Sql(#[from] sqlx::Error),
    /// The ledger records a delete, which is not undone.
    #[error("the ledger records that the import deleted {0:?}; a delete is not undone")]
    Deleted(ImportTarget),
    #[error("{0}")]
    State(String),
}

/// Undo import `import` as job `job`.
///
/// # Errors
/// [`UndoError`] when the import is not undoing under this job, or the
/// database fails while the undo records its ending. A failure while it deals
/// with a resource is not an error: it ends the import `undo_failed` with the
/// reason in the report.
pub async fn undo_import<DB>(
    pool: &Pool<DB>,
    removal: &dyn Removal,
    import: ImportId,
    job: JobId,
    force: Force,
) -> Result<UndoEnd, UndoError>
where
    DB: Backend,
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
    usize: ColumnIndex<DB::Row>,
    for<'x> bool: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> f64: Decode<'x, DB> + Type<DB>,
    for<'x> i64: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Option<i64>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> String: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Option<String>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Vec<u8>: Decode<'x, DB> + Type<DB>,
    for<'x> Option<Vec<u8>>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> &'x str: Encode<'x, DB> + Type<DB> + ColumnIndex<DB::Row>,
{
    let row = ImportRepo::<DB>::new(pool)
        .get(import)
        .await?
        .ok_or_else(|| UndoError::State(format!("no import has the id {}", import.0)))?;
    if row.status != ImportStatus::Undoing || row.job_id != Some(job) {
        return Err(UndoError::State(format!(
            "import {} is {} and not undone by job {}",
            import.0,
            row.status.as_str(),
            job.0
        )));
    }
    let mut base = match row.summary {
        Some(Value::Object(members)) => members,
        _ => Map::new(),
    };
    base.remove(UNDO_MEMBER);
    let (rows, images) = ledger(pool, import).await?;
    let mut undo = Undo {
        pool,
        removal,
        import,
        job,
        judge: Judge::new(pool, import, force, &rows, images),
        base,
        report: UndoReport {
            progress: Progress {
                done: 0,
                total: rows.len(),
            },
            ..UndoReport::default()
        },
    };
    undo.go(rows).await
}

/// The ledger rows and before-images of `import`. A row that records a
/// delete is refused: `start_undo` never lets an undo begin on one.
async fn ledger<DB>(
    pool: &Pool<DB>,
    import: ImportId,
) -> Result<(Vec<ImportResourceRow>, HashMap<ImportTarget, BeforeImage>), UndoError>
where
    DB: Backend,
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
    usize: ColumnIndex<DB::Row>,
    for<'x> i64: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> String: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> &'x str: Encode<'x, DB> + Type<DB> + ColumnIndex<DB::Row>,
{
    let rows = ImportResourceRepo::<DB>::new(pool)
        .list(import, None)
        .await?;
    if let Some(row) = rows.iter().find(|row| row.change == ImportChange::Deleted) {
        return Err(UndoError::Deleted(row.target));
    }
    let mut images = HashMap::new();
    for image in BeforeImageRepo::<DB>::new(pool).list(import).await? {
        images.insert(image.target, BeforeImage::from_json(&image.image)?);
    }
    Ok((order(rows), images))
}

/// The ledger in undo order: changed resources, then what the import made,
/// media before items before sets before types before fields, each by kind
/// and id. One import writes a resource once, so no two rows depend on each
/// other's order, and an order by write time would follow the whole-second
/// clock: the same import read back in another order when its writes
/// crossed a second.
fn order(mut rows: Vec<ImportResourceRow>) -> Vec<ImportResourceRow> {
    let rank = |row: &ImportResourceRow| match (row.change, row.target.kind()) {
        (ImportChange::Changed, _) => 0,
        (_, ImportKind::Media) => 1,
        (_, ImportKind::Item) => 2,
        (_, ImportKind::ItemSet) => 3,
        (_, ImportKind::ResourceTemplate) => 4,
        (_, ImportKind::Property) => 5,
    };
    rows.sort_by_key(|row| {
        let named = Named::of(row.target);
        (rank(row), named.kind, named.id)
    });
    rows
}

/// What happened to one row.
enum Outcome {
    Restored,
    Removed,
    Skipped,
    Kept(Vec<Touch>),
}

/// Whether the undo goes on after a resource.
enum Next {
    Go,
    Stop,
    Lost,
}

struct Undo<'u, DB: Backend> {
    pool: &'u Pool<DB>,
    removal: &'u dyn Removal,
    import: ImportId,
    job: JobId,
    judge: Judge<'u, DB>,
    /// The summary as the run left it.
    base: Map<String, Value>,
    report: UndoReport,
}

impl<DB> Undo<'_, DB>
where
    DB: Backend,
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
    usize: ColumnIndex<DB::Row>,
    for<'x> bool: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> f64: Decode<'x, DB> + Type<DB>,
    for<'x> i64: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Option<i64>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> String: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Option<String>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Vec<u8>: Decode<'x, DB> + Type<DB>,
    for<'x> Option<Vec<u8>>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> &'x str: Encode<'x, DB> + Type<DB> + ColumnIndex<DB::Row>,
{
    async fn go(&mut self, rows: Vec<ImportResourceRow>) -> Result<UndoEnd, UndoError> {
        for row in &rows {
            match self.one(row).await {
                Ok(outcome) => self.count(row, outcome),
                Err(error) => {
                    self.report.error = Some(Stopped {
                        key: ServerMessage {
                            name: "import_undo_stopped",
                            pattern: "",
                        }
                        .key(),
                        detail: messages::import_undo_stopped(error),
                    });
                    break;
                }
            }
            self.report.progress.done += 1;
            match self.pulse().await? {
                Next::Go => {}
                Next::Stop => {
                    self.report.stopped = true;
                    break;
                }
                Next::Lost => return Ok(UndoEnd::Lost),
            }
        }
        self.end().await
    }

    fn count(&mut self, row: &ImportResourceRow, outcome: Outcome) {
        let kind = row.target.kind();
        match outcome {
            Outcome::Restored => self.report.restored.add(kind),
            Outcome::Removed => self.report.removed.add(kind),
            Outcome::Skipped => self.report.skipped.push(Named::of(row.target)),
            Outcome::Kept(touches) => self.report.kept.push(Kept::of(row.target, touches)),
        }
    }

    /// Deal with one row: keep, restore, delete or skip its resource.
    async fn one(&mut self, row: &ImportResourceRow) -> Result<Outcome, UndoError> {
        let target = row.target;
        match self.judge.judge(row).await? {
            Verdict::Gone => {
                self.forget(target).await?;
                return Ok(Outcome::Skipped);
            }
            Verdict::Keep(touches) => return Ok(Outcome::Kept(touches)),
            Verdict::Act { forced: true } => self.report.forced.push(Named::of(target)),
            Verdict::Act { forced: false } => {}
        }
        match row.change {
            ImportChange::Changed => self.restore(target).await,
            ImportChange::Created => {
                let removed = self
                    .removal
                    .remove(target)
                    .await
                    .map_err(UndoError::State)?;
                self.forget(target).await?;
                Ok(match removed {
                    Removed::Removed => Outcome::Removed,
                    Removed::AlreadyGone => Outcome::Skipped,
                })
            }
            ImportChange::Deleted => Err(UndoError::Deleted(target)),
        }
    }

    /// Put a changed resource back, forgetting its row and image in the same
    /// transaction.
    async fn restore(&self, target: ImportTarget) -> Result<Outcome, UndoError> {
        let resource = match target {
            ImportTarget::Item(id) => ImportResource::Item(id),
            ImportTarget::Media(id) => ImportResource::Media(id),
            ImportTarget::ItemSet(id) => ImportResource::ItemSet(id),
            other => {
                return Err(UndoError::State(format!(
                    "the ledger records {other:?} as changed; only items, media and item sets change"
                )));
            }
        };
        let image = self.judge.images.get(&target).ok_or_else(|| {
            UndoError::State(format!("the changed {target:?} has no before-image"))
        })?;
        let mut tx = self.pool.begin_with(DB::begin_write()).await?;
        let restored = ImportWrites::<DB>::new(&mut tx)
            .restore(resource, image)
            .await?;
        ImportResourceRepo::<DB>::delete(&mut *tx, self.import, target).await?;
        BeforeImageRepo::<DB>::delete(&mut *tx, self.import, target).await?;
        tx.commit().await?;
        Ok(if restored {
            Outcome::Restored
        } else {
            Outcome::Skipped
        })
    }

    async fn forget(&self, target: ImportTarget) -> Result<(), UndoError> {
        let mut tx = self.pool.begin().await?;
        ImportResourceRepo::<DB>::delete(&mut *tx, self.import, target).await?;
        BeforeImageRepo::<DB>::delete(&mut *tx, self.import, target).await?;
        tx.commit().await?;
        Ok(())
    }

    fn summary(&self) -> Value {
        let mut summary = self.base.clone();
        summary.insert(
            UNDO_MEMBER.to_string(),
            serde_json::to_value(&self.report).unwrap_or(Value::Null),
        );
        Value::Object(summary)
    }

    async fn pulse(&self) -> Result<Next, UndoError> {
        let imports = ImportRepo::<DB>::new(self.pool);
        if !imports.heartbeat(self.import, self.job).await? {
            return Ok(Next::Lost);
        }
        imports.set_summary(self.import, &self.summary()).await?;
        Ok(if imports.stop_requested(self.import).await? {
            Next::Stop
        } else {
            Next::Go
        })
    }

    async fn end(&mut self) -> Result<UndoEnd, UndoError> {
        let imports = ImportRepo::<DB>::new(self.pool);
        if !imports.heartbeat(self.import, self.job).await? {
            return Ok(UndoEnd::Lost);
        }
        imports.set_summary(self.import, &self.summary()).await?;
        let left = ImportResourceRepo::<DB>::new(self.pool)
            .list(self.import, None)
            .await?;
        let ending = if self.report.error.is_some() || self.report.stopped {
            ImportEnding::UndoFailed
        } else if left.is_empty() {
            ImportEnding::Undone
        } else {
            ImportEnding::PartlyUndone
        };
        if !imports.finish(self.import, ending).await? {
            return Ok(UndoEnd::Lost);
        }
        let report = std::mem::take(&mut self.report);
        Ok(match ending {
            ImportEnding::Undone => UndoEnd::Undone(report),
            ImportEnding::PartlyUndone => UndoEnd::PartlyUndone(report),
            _ => UndoEnd::Failed(report),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use omeka_db::ids::MediaId;

    fn row(id: u64, written: &str) -> ImportResourceRow {
        ImportResourceRow {
            import_id: ImportId(1),
            target: ImportTarget::Media(MediaId(id)),
            record: format!("f:s1/{id}.jpg"),
            change: ImportChange::Created,
            written: chrono::NaiveDateTime::parse_from_str(written, "%Y-%m-%d %H:%M:%S")
                .unwrap_or_default(),
        }
    }

    #[test]
    fn the_undo_order_does_not_depend_on_the_second_a_row_was_written_in() {
        let rows = vec![row(6, "2026-10-20 10:00:01"), row(5, "2026-10-20 10:00:00")];
        let ids: Vec<u64> = order(rows)
            .iter()
            .map(|row| Named::of(row.target).id)
            .collect();
        assert_eq!(ids, [5, 6]);
    }
}
