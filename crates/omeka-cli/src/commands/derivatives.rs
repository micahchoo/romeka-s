//! `romeka media derivatives`: queue the derivatives a media is missing, then
//! run the worker once in this process.
//!
//! The operator's version of what a create does: every media whose preview
//! kinds a decoder or a tool here can make gets a `romeka_media_derivative` row,
//! and one `Romeka\Job\MediaDerivatives` job drains the queue. An original in an
//! object store has no local path above the streaming limit; the worker streams
//! it out of the store, so this command makes its poster or first page where the
//! import could not.
//!
//! Unlike `romeka media thumbnails`, which stays for a one-off inline pass, this
//! command records its work in the queue and the job log and can retry a row a
//! tool failed on (`--retry`).

use std::path::PathBuf;
use std::sync::Arc;

use omeka_api::derivatives::{DERIVATIVES_JOB, enqueue_access, preview_kind};
use omeka_api::jsonld::BaseUrl;
use omeka_db::DbPool;
use omeka_db::Location;
use omeka_db::ids::MediaId;
use omeka_db::repos::{JobRepo, MediaDerivativeRepo, MediaFilter, MediaRepo, Paging};
use omeka_db::rows::{
    DerivativeKind, DerivativeRow, DerivativeState, MediaRow, NewDerivative, NewJob,
};
use omeka_db::with_db;
use omeka_media::store::Store;
use omeka_media::thumbnailer::thumbnails_stored;

use crate::CliError;
use crate::jobs::{JobEnv, Runner};

/// Input for `romeka media derivatives`.
#[derive(Debug, Clone)]
pub struct DerivativeOptions {
    /// The database.
    pub database: Location,
    /// Store root (`file_store.local.base_path`). Defaults to `files` beside the database.
    pub store: Option<PathBuf>,
    /// Re-queue rows a tool failed on or skipped, instead of leaving them.
    pub retry: bool,
    /// Only this media.
    pub media: Option<u64>,
}

/// What the command found and did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DerivativeReport {
    /// Rows this run queued.
    pub queued: usize,
    /// Failed or skipped rows this run put back in the queue.
    pub retried: usize,
    /// Rows already waiting for a worker.
    pub waiting: usize,
    /// Media whose derivative was already in the store.
    pub ready: usize,
    /// Media with no derivative to make, or a row this run left settled.
    pub skipped: usize,
    /// Every derivative row of the selected media after the run.
    pub rows: Vec<DerivativeResultRow>,
}

/// One row the command reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DerivativeResultRow {
    pub media: u64,
    pub file: Option<String>,
    pub kind: DerivativeKind,
    pub state: DerivativeState,
    pub reason: Option<String>,
}

impl DerivativeResultRow {
    fn of(media: &MediaRow, row: &DerivativeRow) -> Self {
        Self {
            media: media.id().0,
            file: media.source.clone(),
            kind: row.kind,
            state: row.state,
            reason: row.reason.clone(),
        }
    }
}

/// Queue every missing derivative, then run the worker once.
///
/// # Errors
/// A [`CliError`] when the database or the store cannot be opened, or when the
/// job row cannot be written or run. A media that cannot be backfilled is
/// reported in the [`DerivativeReport`], not here.
pub async fn run(options: &DerivativeOptions) -> Result<DerivativeReport, CliError> {
    let pool = options.database.open().await?;
    let store =
        crate::commands::file_store::open_store(&options.database, options.store.as_deref())?;
    let media = selected(&pool, options.media).await?;
    let mut report = DerivativeReport::default();
    for row in &media {
        queue(&pool, &store, row, options.retry, &mut report).await?;
    }
    if report.queued + report.retried > 0 {
        run_worker(&pool, Arc::clone(&store)).await?;
    }
    report.rows = finished(&pool, &media).await?;
    Ok(report)
}

/// The media rows the command works: one, or every row.
async fn selected(pool: &DbPool, media: Option<u64>) -> Result<Vec<MediaRow>, CliError> {
    let rows = match media {
        Some(id) => with_db!(pool, |db| MediaRepo::new(db).get(MediaId(id)).await)?
            .into_iter()
            .collect(),
        None => with_db!(pool, |db| MediaRepo::new(db)
            .list(&MediaFilter::default(), &Paging::default())
            .await)?,
    };
    Ok(rows)
}

/// Queue one media's preview, or record why nothing was queued, and record its
/// access decision either way.
async fn queue(
    pool: &DbPool,
    store: &Arc<dyn Store>,
    media: &MediaRow,
    retry: bool,
    report: &mut DerivativeReport,
) -> Result<(), CliError> {
    // An audio file has no preview kind and still has an access rule, so the
    // decision is made before the preview returns early.
    enqueue_access(pool, store, media).await?;
    let Some(kind) = preview_kind(media) else {
        report.skipped += 1;
        return Ok(());
    };
    let id = media.id();
    let existing = with_db!(pool, |db| MediaDerivativeRepo::new(db).get(id, kind).await)?;
    let Some(row) = existing else {
        // No row: a media the import made, or one older than this table. The
        // store decides whether it needs anything.
        if files_stored(store.as_ref(), media) {
            report.ready += 1;
            return Ok(());
        }
        with_db!(pool, |db| MediaDerivativeRepo::new(db)
            .enqueue(&NewDerivative { media_id: id, kind })
            .await)?;
        report.queued += 1;
        return Ok(());
    };
    match row.state {
        DerivativeState::Ready => report.ready += 1,
        DerivativeState::Pending | DerivativeState::Running => report.waiting += 1,
        DerivativeState::Failed | DerivativeState::None if retry => {
            if with_db!(pool, |db| MediaDerivativeRepo::new(db)
                .retry(id, kind)
                .await)?
            {
                report.retried += 1;
            } else {
                report.waiting += 1;
            }
        }
        DerivativeState::Failed | DerivativeState::None => report.skipped += 1,
    }
    Ok(())
}

/// Whether the three thumbnail files are in the store for this media.
fn files_stored(store: &dyn Store, media: &MediaRow) -> bool {
    let Some(storage_id) = media.storage_id.as_deref() else {
        return false;
    };
    thumbnails_stored(store, storage_id).unwrap_or(false)
}

/// Compose a worker job and run it in this process, as `romeka import run` runs
/// a run job.
async fn run_worker(pool: &DbPool, store: Arc<dyn Store>) -> Result<(), CliError> {
    let job = with_db!(pool, |db| JobRepo::new(db)
        .create(&NewJob {
            owner_id: None,
            pid: None,
            status: Some("starting".to_string()),
            class: DERIVATIVES_JOB.to_string(),
            args: Some(serde_json::json!({})),
            log: None,
            started: None,
        })
        .await)?
    .id;
    let env = JobEnv {
        store,
        base_url: BaseUrl::parse(crate::jobs::DEFAULT_BASE_URL)
            .map_err(|error| CliError::message(error.to_string()))?,
        archive_roots: Default::default(),
        jobs: None,
    };
    Runner::with_env(pool, env).run(job).await?;
    Ok(())
}

/// Every derivative row of the selected media after the run.
async fn finished(pool: &DbPool, media: &[MediaRow]) -> Result<Vec<DerivativeResultRow>, CliError> {
    let mut rows = Vec::new();
    for row in media {
        let derivatives = with_db!(pool, |db| MediaDerivativeRepo::new(db)
            .for_media(row.id())
            .await)?;
        for derivative in &derivatives {
            rows.push(DerivativeResultRow::of(row, derivative));
        }
    }
    Ok(rows)
}
