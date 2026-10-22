//! The run: an import written exactly as Check showed it (`docs/import-v2.md`,
//! "Actions", "Lifecycle and the ledger", "Write details" and "Values").
//!
//! [`run_import`] is the body of the [`RUN_JOB`] job. The route already
//! rescanned, derived, checked the digest and moved the import to
//! `running`, so the job derives the frozen plan again against that index
//! and writes, in this order:
//!
//! 1. what the records need first: the plan's new fields (`local`
//!    vocabulary), its new item types, and the sets records name by a title
//!    no set has ([`prelude`]);
//! 2. the item-set records, then each item record with its media, then the
//!    media records of items already in the install ([`units`]). Each
//!    record's bytes reach the store before its transaction opens; the
//!    transaction holds its rows, its ledger row and, for a changed
//!    resource, its before-image. A record that fails is rolled back, its
//!    bytes are removed, and it is listed in `summary.failures`;
//! 3. the links between the import's own records ([`links`]), which need
//!    every record to exist;
//! 4. `left.csv`, into the store.
//!
//! A `delete` writes none of this: it deletes each matched resource through
//! the API ([`delete`]), and nothing it does is undoable.
//!
//! A record whose key the ledger holds is skipped, so a retry converges.
//! After each record the job proves it is alive and reads the stop request;
//! a stop ends the run `run_failed` after the record in hand.

mod delete;
mod fields;
mod links;
mod prelude;
mod units;

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use omeka_db::Backend;
use omeka_db::DbError;
use omeka_db::ids::{ImportId, ItemId, JobId, UserId};
use omeka_db::repos::{ImportRepo, ImportResourceRepo};
use omeka_db::rows::{ImportAction, ImportEnding, ImportStatus, ImportTarget};
use omeka_media::HttpClient;
use omeka_media::store::Store;
use omeka_media::validation::FileValidation;
use sqlx::{ColumnIndex, Decode, Encode, Executor, IntoArguments, Pool, Type};

use super::archive::{ArchiveRoots, SourceDir};
use super::catalog_load::Owners;
use super::ledger::{Failure, Summary, Written};
use super::model::{Fate, Model, Parent, RecordId};
use super::plan::{Action, ImportPlan, PlanError, RecordKind, SourceId};
use super::preview::{Caches, Derived, PreviewError, derive_import};
use super::undo::Removal;
use super::{left, scan};

pub use fields::Lookup;

/// The job class of a run, `job.class`; its args are `{"importId": n}`.
pub const RUN_JOB: &str = "Romeka\\Job\\ImportRun";

/// What happens to an item the run created, after its record commits:
/// `site_items::attach_on_create`, which reads through the SQLite-only
/// read layer, so the caller supplies it.
pub trait SiteSync: Send + Sync {
    fn item_created<'a>(
        &'a self,
        item: ItemId,
    ) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send + 'a>>;
}

/// What the run reads besides the database.
pub struct RunEnv<'a> {
    pub store: Arc<dyn Store>,
    pub roots: &'a ArchiveRoots,
    pub caches: &'a Arc<Caches>,
    /// The install's file-validation settings.
    pub validation: FileValidation,
    /// The guarded client every URL ingester fetches through.
    pub http: HttpClient,
    pub sites: &'a dyn SiteSync,
    /// The API's delete paths, which a `delete` run deletes through.
    pub removal: &'a dyn Removal,
}

/// Who pressed Run: the owner of `runner` in the plan, and the owners the
/// derive may name, as the route derived with.
#[derive(Debug, Clone, Copy)]
pub struct Runner {
    pub user: Option<UserId>,
    pub owners: Owners,
}

/// How a run ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunEnd {
    /// At least one record is in the ledger.
    Done(Summary),
    /// Nothing committed, or a stop was asked for.
    Failed(Summary),
    /// Another worker owns the import now (a reader found this one dead).
    Lost,
}

#[derive(Debug, thiserror::Error)]
pub enum RunError {
    #[error(transparent)]
    Db(#[from] DbError),
    #[error(transparent)]
    Sql(#[from] sqlx::Error),
    #[error(transparent)]
    Derive(#[from] PreviewError),
    #[error("the frozen plan does not parse: {0}")]
    Plan(#[from] PlanError),
    #[error("{0}")]
    State(String),
}

/// Run import `import` as job `job`.
///
/// # Errors
/// [`RunError`] when the import is not running under this job, its plan does
/// not parse, or the database fails outside a record. A record's own failure
/// is not an error: it is listed in the summary.
pub async fn run_import<DB>(
    pool: &Pool<DB>,
    env: &RunEnv<'_>,
    import: ImportId,
    job: JobId,
    runner: Runner,
) -> Result<RunEnd, RunError>
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
    let imports = ImportRepo::<DB>::new(pool);
    let row = imports
        .get(import)
        .await?
        .ok_or_else(|| RunError::State(format!("no import has the id {}", import.0)))?;
    if row.status != ImportStatus::Running || row.job_id != Some(job) {
        return Err(RunError::State(format!(
            "import {} is {} and not run by job {}",
            import.0,
            row.status.as_str(),
            job.0
        )));
    }
    let frozen = row.ran_plan.as_ref().unwrap_or(&row.plan);
    let plan = ImportPlan::from_json(&frozen.to_string())?;
    // A delete writes no record, so the value it gets here is never read.
    let action = match plan.action.action {
        Action::Create | Action::Append | Action::Delete => ImportAction::Append,
        Action::Revise => ImportAction::Revise,
        Action::Update => ImportAction::Update,
        Action::Replace => ImportAction::Replace,
    };
    let derived = derive_import(
        pool,
        env.roots,
        &env.store,
        env.caches,
        import,
        Arc::new(plan),
        runner.owners,
        env.validation.clone(),
    )
    .await?;
    let written = Written::from_rows(
        &ImportResourceRepo::<DB>::new(pool)
            .list(import, None)
            .await?,
    );
    let dirs = scan::source_dirs(env.roots, &derived.plan.sources);
    let staged = super::staging::stored_paths(pool, import).await?;
    let mut run = Run {
        pool,
        env,
        import,
        job,
        runner: runner.user,
        action,
        derived: &derived,
        dirs,
        staged,
        written,
        ids: HashMap::new(),
        set_ids: HashMap::new(),
        lookup: Lookup::default(),
        summary: Summary::default(),
    };
    // A retry starts where the last attempt stopped: each record the ledger
    // already holds is done, and the record loop counts only the others.
    run.summary = Summary {
        done: run
            .model()
            .kept()
            .filter(|record| run.written.holds(&record.origin.key()))
            .count(),
        total: run.model().kept().count(),
        resumed: omeka_db::repos::resumed_of(row.summary.as_ref()),
        ..Summary::default()
    };
    run.go().await
}

/// One run in progress.
struct Run<'r, DB: Backend> {
    pool: &'r Pool<DB>,
    env: &'r RunEnv<'r>,
    import: ImportId,
    job: JobId,
    runner: Option<UserId>,
    /// What a matched record does; a created record ignores it.
    action: ImportAction,
    derived: &'r Derived,
    dirs: std::collections::BTreeMap<SourceId, SourceDir>,
    /// Each staged file's store path, by its path in staging.
    staged: std::collections::BTreeMap<String, String>,
    /// The ledger as it stood, plus what this run wrote.
    written: Written,
    /// What each record of this run's model wrote.
    ids: HashMap<RecordId, ImportTarget>,
    /// The set each item-set record wrote, once those records are done.
    set_ids: HashMap<RecordId, omeka_db::ids::ItemSetId>,
    lookup: Lookup,
    summary: Summary,
}

/// Whether the run goes on after a record.
enum Next {
    Go,
    Stop,
    Lost,
}

impl<'r, DB> Run<'r, DB>
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
    fn model(&self) -> &Model {
        &self.derived.model
    }

    async fn go(&mut self) -> Result<RunEnd, RunError> {
        if self.derived.plan.action.action == Action::Delete {
            return self.delete_all().await;
        }
        self.lookup = Lookup::load(self.pool, self.model()).await?;
        self.prelude().await?;
        if let Next::Lost = self.pulse().await? {
            return Ok(RunEnd::Lost);
        }
        let derived: &'r Derived = self.derived;
        let model = &derived.model;
        let sets = model
            .kept()
            .filter(|record| record.kind == RecordKind::ItemSet);
        let items = model
            .kept()
            .filter(|record| record.kind == RecordKind::Item);
        let loose = model.kept().filter(|record| {
            record.kind == RecordKind::Media && !matches!(record.parent, Some(Parent::Record(_)))
        });
        let order: Vec<RecordId> = sets
            .chain(items)
            .chain(loose)
            .map(|record| record.id)
            .collect();
        let mut sets_done = false;
        for id in order {
            let record = derived.model.record(id);
            if !sets_done && record.kind != RecordKind::ItemSet {
                self.set_ids = self.set_records();
                sets_done = true;
            }
            match record.kind {
                RecordKind::ItemSet => self.item_set(record).await?,
                RecordKind::Item => self.item(record).await?,
                RecordKind::Media => self.loose_media(record).await?,
            }
            match self.pulse().await? {
                Next::Go => {}
                Next::Stop => {
                    self.summary.stopped = true;
                    break;
                }
                Next::Lost => return Ok(RunEnd::Lost),
            }
        }
        if !self.summary.stopped {
            self.links().await?;
        }
        self.end().await
    }

    /// Prove the worker is alive, store the summary, and read the stop
    /// request.
    async fn pulse(&self) -> Result<Next, RunError> {
        let imports = ImportRepo::<DB>::new(self.pool);
        if !imports.heartbeat(self.import, self.job).await? {
            return Ok(Next::Lost);
        }
        imports
            .set_summary(self.import, &self.summary.to_json())
            .await?;
        Ok(if imports.stop_requested(self.import).await? {
            Next::Stop
        } else {
            Next::Go
        })
    }

    /// Count a record this attempt wrote or failed as done.
    fn done(&mut self) {
        self.summary.done += 1;
    }

    fn fail(&mut self, record: String, key: &str, detail: String) {
        self.summary.failures.push(Failure {
            record,
            key: key.to_string(),
            detail,
        });
    }

    /// Store `left.csv` and the ending. A worker another reader found dead
    /// writes nothing, so the stored ending stays the reader's.
    async fn end(&mut self) -> Result<RunEnd, RunError> {
        let imports = ImportRepo::<DB>::new(self.pool);
        if !imports.heartbeat(self.import, self.job).await? {
            return Ok(RunEnd::Lost);
        }
        let text = left::render(self.model(), &self.summary.failures, &self.written);
        let store = Arc::clone(&self.env.store);
        let path = left::path(self.import);
        tokio::task::spawn_blocking(move || store.put(text.as_bytes(), &path, None))
            .await
            .map_err(|error| RunError::State(error.to_string()))?
            .map_err(|error| RunError::State(format!("left.csv was not stored: {error}")))?;
        imports
            .set_summary(self.import, &self.summary.to_json())
            .await?;
        // An earlier attempt's records are left out as `exists` on a retry,
        // so every record counts, kept or not.
        let committed = self
            .model()
            .records
            .iter()
            .any(|record| self.written.holds(&record.origin.key()));
        let ending = if committed && !self.summary.stopped {
            ImportEnding::Done
        } else {
            ImportEnding::RunFailed
        };
        if !imports.finish(self.import, ending).await? {
            return Ok(RunEnd::Lost);
        }
        let summary = std::mem::take(&mut self.summary);
        Ok(match ending {
            ImportEnding::Done => RunEnd::Done(summary),
            _ => RunEnd::Failed(summary),
        })
    }

    /// The created or matched resource of a record, from this run or the
    /// ledger of an earlier attempt.
    fn target_of(&self, id: RecordId) -> Option<ImportTarget> {
        if let Some(target) = self.ids.get(&id) {
            return Some(*target);
        }
        let record = self.model().record(id);
        if let Fate::Act(_) = record.fate {
            return fields::matched(record);
        }
        self.written.target(&record.origin.key())
    }
}

impl<DB: Backend> Run<'_, DB> {
    /// Where a file of a staging source is in the store, and its size.
    fn staged_file(&self, source: &SourceId, path: &str) -> Option<(String, u64)> {
        let staging = self.derived.plan.sources.iter().any(
            |plan| matches!(plan, crate::import::plan::SourcePlan::Staging { id } if id == source),
        );
        if !staging {
            return None;
        }
        let stored = self.staged.get(path)?.clone();
        let size = self
            .derived
            .stored
            .index
            .sources
            .iter()
            .find(|indexed| indexed.id == *source)?
            .walk
            .entries
            .iter()
            .find(|entry| entry.path.as_str() == path)?
            .size;
        Some((stored, size))
    }
}
