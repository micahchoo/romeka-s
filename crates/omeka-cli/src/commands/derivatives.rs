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
//! command records its work in the queue and the job log, can retry a row a
//! tool failed on (`--retry`), and can make ready copies again (`--redo`).

use std::path::PathBuf;
use std::sync::Arc;

use omeka_api::derivatives::{enqueue_access, preview_kind, queue_worker};
use omeka_api::jsonld::BaseUrl;
use omeka_db::DbPool;
use omeka_db::Location;
use omeka_db::ids::MediaId;
use omeka_db::repos::{MediaDerivativeRepo, MediaFilter, MediaRepo, Paging};
use omeka_db::rows::{DerivativeKind, DerivativeRow, DerivativeState, MediaRow, NewDerivative};
use omeka_db::with_db;
use omeka_media::access::AccessTarget;
use omeka_media::store::Store;
use omeka_media::thumbnailer::thumbnails_stored;

use crate::CliError;
use crate::jobs::{JobEnv, Runner};

/// Which ready copies `--redo` makes again, by the copy's target. A bare
/// `--redo` takes every kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Redo {
    All,
    Kind(RedoKind),
}

/// The target kind a `--redo` value names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RedoKind {
    Video,
    Audio,
    Image,
    Document,
}

impl RedoKind {
    /// The word the command line uses, in the order `--help` lists them.
    pub const ALL: [Self; 4] = [Self::Video, Self::Audio, Self::Image, Self::Document];

    /// The word a reader typed.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Video => "video",
            Self::Audio => "audio",
            Self::Image => "image",
            Self::Document => "document",
        }
    }

    /// The word a reader typed, or `None` for another word.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.name() == value)
    }

    /// Whether a copy at `target` is this kind.
    #[must_use]
    pub const fn matches(self, target: AccessTarget) -> bool {
        matches!(
            (self, target),
            (Self::Video, AccessTarget::Mp4)
                | (Self::Audio, AccessTarget::M4a)
                | (Self::Image, AccessTarget::Jpeg)
                | (Self::Document, AccessTarget::Pdf)
        )
    }
}

/// Input for `romeka media derivatives`.
#[derive(Debug, Clone)]
pub struct DerivativeOptions {
    /// The database.
    pub database: Location,
    /// Store root (`file_store.local.base_path`). Defaults to `files` beside the database.
    pub store: Option<PathBuf>,
    /// Re-queue rows a tool failed on or skipped, instead of leaving them.
    pub retry: bool,
    /// Make ready copies again, of every kind or of one target kind. The old
    /// copy stays until the worker writes the new one over it.
    pub redo: Option<Redo>,
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
    /// Ready copies this run asked a worker to make again.
    pub redone: usize,
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
        queue(&pool, &store, row, options, &mut report).await?;
    }
    if report.queued + report.retried + report.redone > 0 {
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

/// Record one media's access decision, retry or redo it when asked, and count
/// both its access row and its preview row.
///
/// An audio file has no preview kind and still has an access rule, so the
/// access row is handled before the preview count returns.
async fn queue(
    pool: &DbPool,
    store: &Arc<dyn Store>,
    media: &MediaRow,
    options: &DerivativeOptions,
    report: &mut DerivativeReport,
) -> Result<(), CliError> {
    access(pool, store, media, options, report).await?;
    preview(pool, store, media, options.retry, report).await
}

/// Decide what a browser needs for this original, record it, and count the row
/// this run left.
async fn access(
    pool: &DbPool,
    store: &Arc<dyn Store>,
    media: &MediaRow,
    options: &DerivativeOptions,
    report: &mut DerivativeReport,
) -> Result<(), CliError> {
    let id = media.id();
    let before = with_db!(pool, |db| MediaDerivativeRepo::new(db)
        .get(id, DerivativeKind::AccessCopy)
        .await)?;
    let decided = enqueue_access(pool, store, media).await?;
    let row = match before {
        Some(row) => row,
        None => {
            // A new row is queued when it waits for a command, and settled when
            // the original stands or the command is missing.
            match decided {
                Some(decision) if decision.state == DerivativeState::Pending => {
                    report.queued += 1;
                }
                Some(_) => report.skipped += 1,
                None => {}
            }
            return Ok(());
        }
    };
    if options.retry
        && with_db!(pool, |db| MediaDerivativeRepo::new(db)
            .retry(id, DerivativeKind::AccessCopy)
            .await)?
    {
        report.retried += 1;
        return Ok(());
    }
    if let Some(redo) = options.redo
        && redo_matches(media, redo)
        && with_db!(pool, |db| MediaDerivativeRepo::new(db)
            .redo(id, DerivativeKind::AccessCopy)
            .await)?
    {
        report.redone += 1;
        return Ok(());
    }
    count(report, row.state);
    Ok(())
}

/// Queue one media's preview, or count the row it has.
async fn preview(
    pool: &DbPool,
    store: &Arc<dyn Store>,
    media: &MediaRow,
    retry: bool,
    report: &mut DerivativeReport,
) -> Result<(), CliError> {
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
    if retry && row.state == DerivativeState::Failed {
        if with_db!(pool, |db| MediaDerivativeRepo::new(db)
            .retry(id, kind)
            .await)?
        {
            report.retried += 1;
        } else {
            report.waiting += 1;
        }
        return Ok(());
    }
    count(report, row.state);
    Ok(())
}

/// Whether this media's copy is the kind a `--redo` value names.
fn redo_matches(media: &MediaRow, redo: Redo) -> bool {
    let Redo::Kind(kind) = redo else {
        return true;
    };
    let target = omeka_media::access::target_of(
        media.media_type.as_deref().unwrap_or_default(),
        media.extension.as_deref(),
    )
    .unwrap_or(AccessTarget::Jpeg);
    kind.matches(target)
}

/// Count one row's state in the report's totals.
fn count(report: &mut DerivativeReport, state: DerivativeState) {
    match state {
        DerivativeState::Ready => report.ready += 1,
        DerivativeState::Pending | DerivativeState::Running => report.waiting += 1,
        DerivativeState::Failed | DerivativeState::None => report.skipped += 1,
    }
}

/// Whether the three thumbnail files are in the store for this media.
fn files_stored(store: &dyn Store, media: &MediaRow) -> bool {
    let Some(storage_id) = media.storage_id.as_deref() else {
        return false;
    };
    thumbnails_stored(store, storage_id).unwrap_or(false)
}

/// Start the queue's one worker and run it in this process, as `romeka import
/// run` runs a run job. `queue_worker` composes the row only when a row this
/// install can claim is waiting and no worker already holds the queue, so the
/// command shares D1's cap with a server: when another worker holds it, this
/// run leaves the rows to that worker and its report says they are waiting.
async fn run_worker(pool: &DbPool, store: Arc<dyn Store>) -> Result<(), CliError> {
    let Some(job) = queue_worker(pool, None).await? else {
        return Ok(());
    };
    let env = JobEnv {
        store,
        base_url: BaseUrl::parse(crate::jobs::DEFAULT_BASE_URL)
            .map_err(|error| CliError::message(error.to_string()))?,
        archive_roots: Default::default(),
        // The backfill drains its queue in this process on purpose: the operator
        // is waiting for the report.
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
