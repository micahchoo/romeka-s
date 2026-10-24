//! The body of the run and undo jobs: the run's open targets, a chunk at a time.
//!
//! Each target is its own transaction (`BulkRepo::apply_target`, `revert_target`), which settles
//! it whatever happens to the next one. After each chunk the job counts the targets, proves it is
//! alive, and reads the state a stop request may have left. The chunks follow the item ids
//! upwards, so a target an undo keeps is passed once, not read again forever.
//!
//! A run acts as its owner: the owner's role decides, item by item, whether the item may be
//! updated, as the item `PATCH` decides it.

use std::collections::BTreeSet;
use std::time::{Duration, Instant};

use omeka_db::DbPool;
use omeka_db::ids::{BulkId, ItemId, JobId, PropertyId};
use omeka_db::repos::{BulkRepo, TemplateRepo, UserRepo};
use omeka_db::rows::{BeforeImage, BulkEnding, BulkPhase, BulkRow, Described, FailReason, Verdict};
use omeka_db::with_db;

use super::BulkError;
use super::change::{BulkChange, BulkEdit};
use crate::auth::Actor;
use crate::errors::ApiError;
use crate::jsonld::BaseUrl;
use crate::read::Api;

/// Targets per chunk: one heartbeat and one stop check each.
const CHUNK: i64 = 50;

/// The longest rest between two chunks.
const MAX_REST: Duration = Duration::from_millis(250);

const ITEM_CLASS: &str = "Omeka\\Entity\\Item";

/// How a job's work on a run ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkEnd {
    Ended(BulkEnding),
    /// Another job owns the run now, or a reader settled it dead; this one wrote nothing more.
    Lost,
}

/// Do `phase` on run `bulk` as job `job`. `force` restores items edited since (undo only).
///
/// # Errors
/// [`BulkError`] when the run is not working under this job. A failure while it deals with the
/// targets is not an error: it ends the run `failed` (or `partly_undone`) with the reason.
pub async fn work(
    pool: &DbPool,
    base: &BaseUrl,
    bulk: BulkId,
    job: JobId,
    phase: BulkPhase,
    force: bool,
) -> Result<WorkEnd, BulkError> {
    let row = with_db!(pool, |db| BulkRepo::new(db).get(bulk).await)?
        .filter(|row| row.job_id == Some(job) && phase.live().contains(&row.status))
        .ok_or_else(|| {
            ApiError::internal(format!("bulk change {bulk} is not worked on by job {job}"))
        })?;
    let outcome = match phase {
        BulkPhase::Run => run(pool, base, &row, job).await,
        BulkPhase::Undo => undo(pool, &row, job, force).await,
    };
    let (ending, error) = match outcome {
        Ok(Step::Lost) => return Ok(WorkEnd::Lost),
        Ok(Step::Stopped) => (stopped(phase), None),
        Ok(Step::Finished) => (finished(pool, bulk, phase).await?, None),
        Err(error) => (failed(phase), Some(error.to_string())),
    };
    let ended = with_db!(pool, |db| BulkRepo::new(db)
        .finish(bulk, job, ending, error.as_deref())
        .await)?;
    Ok(if ended {
        WorkEnd::Ended(ending)
    } else {
        WorkEnd::Lost
    })
}

/// Where the chunk loop left the run.
enum Step {
    Finished,
    Stopped,
    Lost,
}

fn stopped(phase: BulkPhase) -> BulkEnding {
    match phase {
        BulkPhase::Run => BulkEnding::Stopped,
        BulkPhase::Undo => BulkEnding::PartlyUndone,
    }
}

fn failed(phase: BulkPhase) -> BulkEnding {
    match phase {
        BulkPhase::Run => BulkEnding::Failed,
        BulkPhase::Undo => BulkEnding::PartlyUndone,
    }
}

/// A run that visited every target is done; an undo is undone when nothing it changed is left.
async fn finished(pool: &DbPool, bulk: BulkId, phase: BulkPhase) -> Result<BulkEnding, BulkError> {
    if phase == BulkPhase::Run {
        return Ok(BulkEnding::Done);
    }
    let counts = with_db!(pool, |db| BulkRepo::new(db).counts(bulk).await)?;
    Ok(if counts.done + counts.kept == 0 {
        BulkEnding::Undone
    } else {
        BulkEnding::PartlyUndone
    })
}

async fn run(pool: &DbPool, base: &BaseUrl, row: &BulkRow, job: JobId) -> Result<Step, BulkError> {
    let change = BulkChange::parse(&row.change).map_err(ApiError::internal)?;
    let BulkChange::Edit(edit) = &change;
    let api = Api::load(pool, base.clone(), owner(pool, row).await?).await?;
    let rules = &Rules {
        api: &api,
        edit,
        required: required(pool, edit).await?,
    };
    chunks(pool, row, job, BulkPhase::Run, |item| async move {
        with_db!(pool, |db| BulkRepo::new(db)
            .apply_target(row.id, item, |image| rules.verdict(image))
            .await)
        .map(|_| ())
    })
    .await
}

async fn undo(pool: &DbPool, row: &BulkRow, job: JobId, force: bool) -> Result<Step, BulkError> {
    chunks(pool, row, job, BulkPhase::Undo, |item| async move {
        with_db!(pool, |db| BulkRepo::new(db)
            .revert_target(row.id, item, force)
            .await)
        .map(|_| ())
    })
    .await
}

/// Visit the open targets of `phase` in id order, a chunk at a time, pulsing after each.
async fn chunks<F, Fut>(
    pool: &DbPool,
    row: &BulkRow,
    job: JobId,
    phase: BulkPhase,
    mut each: F,
) -> Result<Step, BulkError>
where
    F: FnMut(ItemId) -> Fut,
    Fut: Future<Output = Result<(), omeka_db::DbError>>,
{
    let mut after = None;
    loop {
        let items = with_db!(pool, |db| BulkRepo::new(db)
            .open_targets(row.id, phase, after, CHUNK)
            .await)?;
        let Some(last) = items.last().copied() else {
            return Ok(Step::Finished);
        };
        let began = Instant::now();
        for item in items {
            each(item).await?;
        }
        after = Some(last);
        let counts = with_db!(pool, |db| BulkRepo::new(db).counts(row.id).await)?;
        match with_db!(pool, |db| BulkRepo::new(db)
            .pulse(row.id, job, phase, &counts)
            .await)?
        {
            None => return Ok(Step::Lost),
            Some(status) if status.is_stopping() => return Ok(Step::Stopped),
            Some(_) => {}
        }
        // SQLite has one writer, and a writer that waits sleeps between its tries: a job that
        // starts its next transaction at once starves every other write, a stop request
        // included, until it ends. Resting as long as the chunk worked lets them in.
        tokio::time::sleep(began.elapsed().min(MAX_REST)).await;
    }
}

/// The run's owner, with the role they hold now.
async fn owner(pool: &DbPool, row: &BulkRow) -> Result<Actor, BulkError> {
    let user = match row.owner_id {
        Some(owner) => with_db!(pool, |db| UserRepo::new(db).get(owner).await)?,
        None => None,
    };
    user.map(|user| Actor::new(user.id, user.role))
        .ok_or_else(|| ApiError::internal("the person who previewed this change is gone").into())
}

/// The properties the item type an edit sets requires; none when it sets none.
async fn required(pool: &DbPool, edit: &BulkEdit) -> Result<BTreeSet<PropertyId>, BulkError> {
    let Some(super::Assign::Set(template)) = edit.template else {
        return Ok(BTreeSet::new());
    };
    let properties = with_db!(pool, |db| TemplateRepo::new(db)
        .list_properties(template)
        .await)?;
    Ok(properties
        .into_iter()
        .filter(|property| property.is_required)
        .map(|property| property.property_id)
        .collect())
}

/// What a run decides for one item, from its before-image alone.
struct Rules<'r, 'a> {
    api: &'r Api<'a>,
    edit: &'r BulkEdit,
    required: BTreeSet<PropertyId>,
}

impl Rules<'_, '_> {
    fn verdict(&self, image: &BeforeImage) -> Verdict {
        let Some(before) = Described::of_image(image) else {
            return Verdict::Refuse(FailReason::Gone);
        };
        if self
            .api
            .authorize_resource(ITEM_CLASS, before.owner)
            .is_err()
        {
            return Verdict::Refuse(FailReason::Denied);
        }
        let after = self.edit.apply(&before);
        if after == before {
            return Verdict::Unchanged;
        }
        let present: BTreeSet<PropertyId> = image
            .values
            .iter()
            .map(|value| value.content.property_id)
            .collect();
        if after.template != before.template && !self.required.is_subset(&present) {
            return Verdict::Refuse(FailReason::TemplateRequires);
        }
        Verdict::Write(after)
    }
}
