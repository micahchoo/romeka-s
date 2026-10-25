//! The body of the run and undo jobs: the run's open targets, a chunk at a time.
//!
//! Each target is its own transaction (`BulkRepo::apply_target`, `revert_target`), which settles
//! it whatever happens to the next one. After each chunk the job counts the targets, proves it is
//! alive, and reads the state a stop request may have left. The chunks follow the item ids
//! upwards, so a target an undo keeps is passed once, not read again forever.
//!
//! A run acts as its owner and an undo as the person who asked for it, with the account as it
//! stands before each chunk: the batch right, and for a run the change's own checks, are read
//! again then, and the role decides, item by item, whether the item may be updated, as the item
//! `PATCH` decides it.

use std::collections::{BTreeSet, HashMap};
use std::time::{Duration, Instant};

use omeka_db::DbPool;
use omeka_db::ids::{BulkId, ItemId, JobId, PropertyId, SiteId, TemplateId, UserId};
use omeka_db::repos::{BulkRepo, JobRepo, Paging, SiteFilter, SiteRepo, TemplateRepo, UserRepo};
use omeka_db::rows::{
    BeforeImage, BulkEnding, BulkPhase, BulkRow, Described, FailReason, ImageMemberships,
    ItemMembers, Restore, ValueContent, Verdict, Written,
};
use omeka_db::with_db;

use super::BulkError;
use super::access::{denied, holds};
use super::change::{BulkChange, BulkEdit};
use super::preview::check;
use super::scope::Scope;
use super::values::ValueEdits;
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
        BulkPhase::Undo => undo(pool, base, &row, job, force).await,
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
    let scope = Scope::of(&row.query)?;
    let required = &required(pool).await?;
    let (change, scope) = (&change, &scope);
    chunks(pool, row, job, BulkPhase::Run, |items| async move {
        let api = permitted(pool, base, row.owner_id, scope).await?;
        let values = check(&api, change).await?;
        let rules = Rules {
            api: &api,
            edit,
            values: values.as_ref(),
            required,
        };
        for item in items {
            with_db!(pool, |db| BulkRepo::new(db)
                .apply_target(row.id, item, |image| rules.verdict(image))
                .await)?;
        }
        Ok(())
    })
    .await
}

/// An undo acts as the person who asked for it, the owner of its job, and restores an item only
/// as far as they may write it now.
async fn undo(
    pool: &DbPool,
    base: &BaseUrl,
    row: &BulkRow,
    job: JobId,
    force: bool,
) -> Result<Step, BulkError> {
    let scope = &Scope::of(&row.query)?;
    let BulkChange::Edit(edit) = &BulkChange::parse(&row.change).map_err(ApiError::internal)?;
    let asker = with_db!(pool, |db| JobRepo::new(db).get(job).await)?.and_then(|job| job.owner_id);
    chunks(pool, row, job, BulkPhase::Undo, |items| async move {
        let api = permitted(pool, base, asker, scope).await?;
        let restorer = Restorer {
            api: &api,
            edit,
            force,
            assignable: assignable(&api).await?,
        };
        for item in items {
            with_db!(pool, |db| BulkRepo::new(db)
                .revert_target(row.id, item, force, |now, kept| {
                    restorer.restore(now, kept)
                })
                .await)?;
        }
        Ok(())
    })
    .await
}

/// Visit the open targets of `phase` in id order, a chunk at a time, pulsing after each. `each`
/// checks the rights the chunk needs before it deals with the chunk's targets.
async fn chunks<F, Fut>(
    pool: &DbPool,
    row: &BulkRow,
    job: JobId,
    phase: BulkPhase,
    mut each: F,
) -> Result<Step, BulkError>
where
    F: FnMut(Vec<ItemId>) -> Fut,
    Fut: Future<Output = Result<(), BulkError>>,
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
        each(items).await?;
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

/// The read surface of `user` as their account stands now, once they hold the batch right the
/// run's scope needs. Called before each chunk, so a person deactivated or demoted while a job
/// works stops it at the next chunk; the error ends the run with its reason.
async fn permitted<'a>(
    pool: &'a DbPool,
    base: &BaseUrl,
    user: Option<UserId>,
    scope: &Scope,
) -> Result<Api<'a>, BulkError> {
    let account = match user {
        Some(user) => with_db!(pool, |db| UserRepo::new(db).get(user).await)?,
        None => None,
    };
    let Some(account) = account else {
        return Err(ApiError::internal("the person who asked for this change is gone").into());
    };
    if !account.is_active {
        return Err(
            ApiError::internal("the person who asked for this change is no longer active").into(),
        );
    }
    let actor = Actor::new(account.id, account.role);
    if !holds(&actor, scope.right()) {
        return Err(denied(scope.right()));
    }
    Ok(Api::load(pool, base.clone(), actor).await?)
}

/// The sites the caller may assign items to (`can-assign-items`).
async fn assignable(api: &Api<'_>) -> Result<BTreeSet<SiteId>, BulkError> {
    let sites = with_db!(api.pool(), |db| SiteRepo::new(db)
        .list(&SiteFilter::default(), &Paging::default())
        .await)?;
    let mut assignable = BTreeSet::new();
    for site in &sites {
        if crate::write::items::can_assign(api, site).await? {
            assignable.insert(site.id);
        }
    }
    Ok(assignable)
}

/// What an undo puts back on one item, from the item as it is and the image the run kept.
struct Restorer<'r, 'a> {
    api: &'r Api<'a>,
    edit: &'r BulkEdit,
    force: bool,
    assignable: BTreeSet<SiteId>,
}

/// Whether an item's sets or sites are no longer those the run left: a site's item pool and an
/// item set's removal change them without a new `modified`, which the undo's own test reads.
pub(super) fn members_moved(edit: &BulkEdit, kept: &BeforeImage, now: &ItemMembers) -> bool {
    let Some(before) = Described::of_image(kept) else {
        return false;
    };
    let left = edit.apply(&before);
    left.item_sets != now.item_sets || left.sites != now.sites
}

impl Restorer<'_, '_> {
    /// The image to restore: kept unless forced when the item's memberships moved since the
    /// run; refused unless the caller may update the item under its owner now and under the
    /// owner the image gives back, as the item `PATCH` checks the owner. A site the caller may
    /// not assign items to keeps the item's membership as it is now, as an item write leaves
    /// such a site alone.
    fn restore(&self, now: &BeforeImage, mut kept: BeforeImage) -> Restore {
        let moved = ItemMembers::of_image(now)
            .is_some_and(|members| members_moved(self.edit, &kept, &members));
        if moved && !self.force {
            return Restore::Keep;
        }
        for owner in [now.owner, kept.owner] {
            if self.api.authorize_resource(ITEM_CLASS, owner).is_err() {
                return Restore::Refuse(FailReason::UndoDenied);
            }
        }
        if let (
            ImageMemberships::Item { sites: current, .. },
            ImageMemberships::Item { sites, .. },
        ) = (&now.memberships, &mut kept.memberships)
        {
            sites.retain(|site| self.assignable.contains(site));
            for site in current {
                if !self.assignable.contains(site) && !sites.contains(site) {
                    sites.push(*site);
                }
            }
        }
        Restore::Image(kept)
    }
}

/// The properties each item type requires.
async fn required(pool: &DbPool) -> Result<HashMap<TemplateId, BTreeSet<PropertyId>>, BulkError> {
    let templates = with_db!(pool, |db| TemplateRepo::new(db).list().await)?;
    let mut required = HashMap::new();
    for template in templates {
        let properties = with_db!(pool, |db| TemplateRepo::new(db)
            .list_properties(template.id)
            .await)?;
        required.insert(
            template.id,
            properties
                .into_iter()
                .filter(|property| property.is_required)
                .map(|property| property.property_id)
                .collect(),
        );
    }
    Ok(required)
}

/// What a run decides for one item, from its before-image alone.
struct Rules<'r, 'a> {
    api: &'r Api<'a>,
    edit: &'r BulkEdit,
    values: Option<&'r ValueEdits<ValueContent>>,
    required: &'r HashMap<TemplateId, BTreeSet<PropertyId>>,
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
        let held: Vec<ValueContent> = image
            .values
            .iter()
            .map(|value| value.content.clone())
            .collect();
        let values = self
            .values
            .map(|values| values.plan(&held))
            .unwrap_or_default();
        if after == before && values.is_empty() {
            return Verdict::Unchanged;
        }
        let fields = |values: &[ValueContent]| -> BTreeSet<PropertyId> {
            values.iter().map(|value| value.property_id).collect()
        };
        let (had, has) = (fields(&held), fields(&values.applied(&held)));
        let required = after
            .template
            .and_then(|template| self.required.get(&template))
            .cloned()
            .unwrap_or_default();
        if after.template != before.template && !required.is_subset(&has) {
            return Verdict::Refuse(FailReason::TemplateRequires);
        }
        if required
            .iter()
            .any(|property| had.contains(property) && !has.contains(property))
        {
            return Verdict::Refuse(FailReason::RemovesRequired);
        }
        Verdict::Write(Written {
            fields: after,
            values,
        })
    }
}
