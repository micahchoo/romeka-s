//! Undo: put the install back as it was before one import, from what its
//! ledger says (`docs/import-v2.md`, "Lifecycle and the ledger", "Undo").
//!
//! [`undo_import`] is the body of the [`UNDO_JOB`] job. The route already
//! moved the import to `undoing`. The job reads the ledger once and deals
//! with each row in this order:
//!
//! 1. the changed resources, newest write first, restored from their
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
//! `undone` when its ledger is empty, else `undo_failed`.

mod remove;
mod report;

use std::collections::{HashMap, HashSet};

use omeka_db::Backend;
use omeka_db::DbError;
use omeka_db::ids::{ImportId, ItemId, JobId, MediaId};
use omeka_db::repos::{
    BeforeImageRepo, ImportRepo, ImportResourceRepo, ImportTouchRepo, ImportWrites, MediaRepo,
    Presence,
};
use omeka_db::rows::{
    BeforeImage, ImageMemberships, ImportChange, ImportEnding, ImportKind, ImportResource,
    ImportResourceRow, ImportStatus, ImportTarget,
};
use serde_json::{Map, Value};
use sqlx::{ColumnIndex, Decode, Encode, Executor, IntoArguments, Pool, Type};

pub use remove::{ApiRemoval, Removal, Removed};
pub use report::{Counts, Force, Kept, Named, Progress, Reason, Stopped, UndoReport};

use super::touched::{Own, Touch, touched};
use crate::messages::{self, ServerMessage};

/// The job class of an undo; its args are `{"importId": n, "force"?: ...}`.
pub const UNDO_JOB: &str = "Romeka\\Job\\ImportUndo";

/// The summary member that holds the undo's report, beside the run's.
pub const UNDO_MEMBER: &str = "undo";

/// How an undo ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UndoEnd {
    /// The ledger is empty.
    Undone(UndoReport),
    /// Something was kept, the undo stopped, or it failed; a new undo goes on.
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
    let rows = ImportResourceRepo::<DB>::new(pool)
        .list(import, None)
        .await?;
    let mut images = HashMap::new();
    for image in BeforeImageRepo::<DB>::new(pool).list(import).await? {
        images.insert(image.target, BeforeImage::from_json(&image.image)?);
    }
    let mut undo = Undo {
        pool,
        removal,
        import,
        job,
        force,
        own: Own::from_rows(&rows),
        images,
        verdicts: HashMap::new(),
        kept_items: HashSet::new(),
        base,
        report: UndoReport {
            progress: Progress {
                done: 0,
                total: rows.len(),
            },
            ..UndoReport::default()
        },
    };
    undo.go(order(rows)).await
}

/// The ledger in undo order: changed resources newest first, then what the
/// import made, media before items before sets before types before fields.
fn order(mut rows: Vec<ImportResourceRow>) -> Vec<ImportResourceRow> {
    let rank = |row: &ImportResourceRow| match (row.change, row.target.kind()) {
        (ImportChange::Changed, _) => 0,
        (_, ImportKind::Media) => 1,
        (_, ImportKind::Item) => 2,
        (_, ImportKind::ItemSet) => 3,
        (_, ImportKind::ResourceTemplate) => 4,
        (_, ImportKind::Property) => 5,
    };
    rows.sort_by(|a, b| {
        rank(a)
            .cmp(&rank(b))
            .then_with(|| b.written.cmp(&a.written))
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
    force: Force,
    own: Own,
    images: HashMap<ImportTarget, BeforeImage>,
    /// What touched each item the import made, read once: its media ask
    /// before the item's own turn.
    verdicts: HashMap<ItemId, Vec<Touch>>,
    /// Items the undo keeps, whose media it keeps too.
    kept_items: HashSet<ItemId>,
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
            Outcome::Kept(touches) => self.report.kept.push(Kept {
                resource: Named::of(row.target),
                reasons: touches
                    .into_iter()
                    .map(|touch| Reason {
                        touch,
                        key: touch.key(),
                        detail: touch.detail(),
                    })
                    .collect(),
            }),
        }
    }

    /// Deal with one row: keep, restore, delete or skip its resource.
    async fn one(&mut self, row: &ImportResourceRow) -> Result<Outcome, UndoError> {
        let target = row.target;
        let reads = ImportTouchRepo::<DB>::new(self.pool);
        let Presence::Here { modified } = reads.presence(target).await? else {
            self.forget(target).await?;
            return Ok(Outcome::Skipped);
        };
        let touches = match target {
            ImportTarget::ResourceTemplate(id) if reads.template_in_use(id).await? => {
                vec![Touch::InUse]
            }
            ImportTarget::Property(id) if reads.property_in_use(id).await? => vec![Touch::InUse],
            ImportTarget::ResourceTemplate(_) | ImportTarget::Property(_) => Vec::new(),
            ImportTarget::Item(item) if row.change == ImportChange::Created => {
                self.verdict(row, item, modified).await?
            }
            ImportTarget::Media(media) if row.change == ImportChange::Created => {
                let mut touches = self.read_touched(row, modified).await?;
                if self.item_is_kept(media).await? {
                    touches.push(Touch::ItemKept);
                }
                touches
            }
            ImportTarget::ItemSet(set) if row.change == ImportChange::Created => {
                let mut touches = self.read_touched(row, modified).await?;
                let holds_kept = reads
                    .set_members(set)
                    .await?
                    .iter()
                    .any(|member| self.kept_items.contains(&ItemId(member.resource.0)));
                if holds_kept {
                    touches.push(Touch::MemberKept);
                }
                touches
            }
            _ => self.read_touched(row, modified).await?,
        };
        if !touches.is_empty() {
            if !self.force.covers(target) {
                if let ImportTarget::Item(item) = target {
                    self.kept_items.insert(item);
                }
                return Ok(Outcome::Kept(touches));
            }
            self.report.forced.push(Named::of(target));
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
        }
    }

    async fn read_touched(
        &self,
        row: &ImportResourceRow,
        modified: Option<chrono::NaiveDateTime>,
    ) -> Result<Vec<Touch>, UndoError> {
        let before = match self.images.get(&row.target).map(|image| &image.memberships) {
            Some(ImageMemberships::Item { media_order, .. }) => media_order.as_slice(),
            _ => &[],
        };
        Ok(touched(self.pool, self.import, row, modified, &self.own, before).await?)
    }

    async fn verdict(
        &mut self,
        row: &ImportResourceRow,
        item: ItemId,
        modified: Option<chrono::NaiveDateTime>,
    ) -> Result<Vec<Touch>, UndoError> {
        if let Some(touches) = self.verdicts.get(&item) {
            return Ok(touches.clone());
        }
        let touches = self.read_touched(row, modified).await?;
        self.verdicts.insert(item, touches.clone());
        Ok(touches)
    }

    /// Whether the undo keeps the media's item: a changed item kept in the
    /// first pass, or an item this import made that is touched and not forced.
    async fn item_is_kept(&mut self, media: MediaId) -> Result<bool, UndoError> {
        let Some(row) = MediaRepo::<DB>::new(self.pool).get(media).await? else {
            return Ok(false);
        };
        let item = row.item_id;
        if self.kept_items.contains(&item) {
            return Ok(true);
        }
        let target = ImportTarget::Item(item);
        if !self.own.holds(target) || self.force.covers(target) {
            return Ok(false);
        }
        let ledger = ImportResourceRepo::<DB>::new(self.pool)
            .list(self.import, Some(ImportKind::Item))
            .await?;
        let Some(item_row) = ledger
            .iter()
            .find(|row| row.target == target && row.change == ImportChange::Created)
        else {
            return Ok(false);
        };
        let Presence::Here { modified } = ImportTouchRepo::<DB>::new(self.pool)
            .presence(target)
            .await?
        else {
            return Ok(false);
        };
        Ok(!self.verdict(item_row, item, modified).await?.is_empty())
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
        let image = self.images.get(&target).ok_or_else(|| {
            UndoError::State(format!("the changed {target:?} has no before-image"))
        })?;
        let mut tx = self.pool.begin().await?;
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
        let ending = if left.is_empty() && self.report.error.is_none() && !self.report.stopped {
            ImportEnding::Undone
        } else {
            ImportEnding::UndoFailed
        };
        if !imports.finish(self.import, ending).await? {
            return Ok(UndoEnd::Lost);
        }
        let report = std::mem::take(&mut self.report);
        Ok(match ending {
            ImportEnding::Undone => UndoEnd::Undone(report),
            _ => UndoEnd::Failed(report),
        })
    }
}
