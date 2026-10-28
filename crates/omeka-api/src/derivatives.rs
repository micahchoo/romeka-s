//! Derivatives a media gets outside the request that created it.
//!
//! Upstream makes a file's thumbnails inside the create (`TempFile::storeThumbnails`), so a large
//! video holds the request for the length of `ffmpeg`. Romeka keeps that for the archive import,
//! which already batches its work, and moves it to a job for every other create: the write stores
//! the original, queues a `romeka_media_derivative` row, and answers at once with
//! `o-module-romeka:preview: {"state": "pending"}`. The worker (`Romeka\Job\MediaDerivatives`)
//! claims the row, makes the still, and settles it ready, failed or none.
//!
//! The same worker makes the audio, video and office document access copies D2 planned: an
//! `access_copy` row whose tool is `ffmpeg` is a transcode (H.264/AAC MP4 or AAC M4A), and one whose
//! tool is `soffice` is a PDF made from an office document. A transcode runs on its own thread,
//! writes its progress to the row and stops between progress updates when the job is asked to stop
//! or an import starts ([`settle_next`]); a document's PDF copy runs on the same thread with no
//! progress. A copy waiting for another tool is left for the worker that runs that tool.
//!
//! This module owns the decisions the create path, the worker and the CLI backfill must agree on:
//! which kind of derivative a media needs, when to queue one, what a media document shows for it,
//! and where its access copy is.

use std::sync::Arc;
use std::time::{Duration, Instant};

use omeka_db::DbPool;
use omeka_db::ids::JobId;
use omeka_db::repos::{ImportRepo, JobRepo, MediaDerivativeRepo, MediaRepo};
use omeka_db::rows::{
    DerivativeKind, DerivativeRow, DerivativeState, MediaRow, NewDerivative, NewJob,
};
use omeka_db::with_db;
use omeka_media::ThumbnailType;
use omeka_media::access::{self, Access, AccessTarget, Queue, StoredFile};
use omeka_media::access_master::{MasterSource, MasterWork, make_master};
use omeka_media::derivative::{DerivativeWork, PreviewSource, make_preview};
use omeka_media::store::{Store, StorePrefix, Thumbnailer, storage_path};
use omeka_media::thumbnailer::{SOURCE_MEDIA_TYPES, thumbnails_stored};
use omeka_media::tool::{Still, Tools};
use omeka_media::transcode::{Transcode, TranscodeSource, TranscodeWork, thumbnails_from_copy};

use crate::errors::ApiError;
use crate::read::db_error;
use crate::wire::media::{MediaPreview, PreviewState};

/// The `job.class` of the worker that drains the derivative queue.
pub const DERIVATIVES_JOB: &str = "Romeka\\Job\\MediaDerivatives";

/// The thumbnailer a create that defers passes to its ingester: it makes
/// nothing, because the worker reads the stored original later.
///
/// An empty list is what upstream's thumbnailer answers for a type it cannot
/// decode, so the ingester's own accounting (`media.has_thumbnails`) is
/// unchanged and a reader sees the pending state instead of a broken one.
pub struct DeferredThumbnails;

impl Thumbnailer for DeferredThumbnails {
    fn thumbnails(
        &self,
        _original: &[u8],
        _media_type: Option<&str>,
    ) -> Vec<(ThumbnailType, Vec<u8>)> {
        Vec::new()
    }
}

/// The preview kind a media row needs, from what it stored.
///
/// `None` for a row with no stored original (an embed, an HTML media, an IIIF reference) and for
/// a type no decoder or tool here reads: those have no derivative to queue and read as `none`.
#[must_use]
pub fn preview_kind(row: &MediaRow) -> Option<DerivativeKind> {
    let media_type = row.media_type.as_deref()?;
    if row.storage_id.is_none() || !row.has_original {
        return None;
    }
    let readable = SOURCE_MEDIA_TYPES.contains(&media_type) || Still::of(media_type).is_some();
    if !readable {
        return None;
    }
    Some(if media_type.starts_with("video/") {
        DerivativeKind::Poster
    } else if media_type == "application/pdf" {
        DerivativeKind::PdfPage
    } else {
        DerivativeKind::Thumbnail
    })
}

/// What one media's create queued.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Enqueued {
    /// The preview kind this media makes, when it makes one.
    pub preview: Option<DerivativeKind>,
    /// The access decision this media recorded, when its type has a rule.
    pub access: Option<AccessDecision>,
}

/// The access decision one create recorded in `romeka_media_derivative`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessDecision {
    pub access: Access,
    pub state: DerivativeState,
    /// The words a reader sees for `none`.
    pub reason: Option<String>,
    /// The command the copy waits for, or the one a missing install needs.
    pub tool: Option<String>,
}

/// Queue everything one media's create needs: its preview kind and its access
/// decision. `None`/`None` when it needs neither.
///
/// The preview insert leaves an existing row alone, so a media whose row
/// already reached a state keeps it. The access decision is recomputed for
/// every create because it is about the file just stored; an access row that
/// already exists keeps its state too, so a retry starts from the queue.
///
/// # Errors
/// A database error writing a row, or a blocking task that did not finish.
pub async fn enqueue(
    pool: &DbPool,
    store: &Arc<dyn Store>,
    row: &MediaRow,
) -> Result<Enqueued, ApiError> {
    let preview = enqueue_preview(pool, row).await.map_err(db_error)?;
    let access = enqueue_access(pool, store, row).await?;
    Ok(Enqueued { preview, access })
}

/// Queue the preview kind `row` needs, or `None` when no decoder or tool here
/// makes one.
async fn enqueue_preview(
    pool: &DbPool,
    row: &MediaRow,
) -> Result<Option<DerivativeKind>, omeka_db::DbError> {
    let Some(kind) = preview_kind(row) else {
        return Ok(None);
    };
    let new = NewDerivative {
        media_id: row.id(),
        kind,
    };
    with_db!(pool, |db| MediaDerivativeRepo::new(db).enqueue(&new).await)?;
    Ok(Some(kind))
}

/// Decide what a browser needs to show this media's original, and record it as
/// one `access_copy` row: `none` when the original stands or the copy's tool is
/// not installed, `pending` when the copy is queued for the worker D3 and D4
/// add. The original is never changed.
///
/// The decision reads the stored file, so it runs on the blocking pool: a
/// video's codecs come from an `ffprobe` over a bounded sample of its ends, and
/// a TIFF's layout from its directory read.
///
/// # Errors
/// A database error writing the row, or a blocking task that did not finish.
pub async fn enqueue_access(
    pool: &DbPool,
    store: &Arc<dyn Store>,
    row: &MediaRow,
) -> Result<Option<AccessDecision>, ApiError> {
    let Some(source) = access_source(row) else {
        return Ok(None);
    };
    let store = Arc::clone(store);
    let decided = tokio::task::spawn_blocking(move || decide(store.as_ref(), &source))
        .await
        .map_err(|error| {
            ApiError::internal(format!("the access decision did not finish: {error}"))
        })?;
    let Some((access, queue)) = decided else {
        return Ok(None);
    };
    let (state, reason, tool) = recorded(access, queue);
    let new = NewDerivative {
        media_id: row.id(),
        kind: DerivativeKind::AccessCopy,
    };
    let planned = with_db!(pool, |db| MediaDerivativeRepo::new(db)
        .plan(&new, state, reason.as_deref(), tool.as_deref())
        .await)
    .map_err(db_error)?;
    // The row already existed, so it keeps its words. A row settled before the
    // words named the reader of the original can still be corrected: the
    // decision is about the same file, and only the words changed.
    if !planned && let Access::Original(reader) = access {
        with_db!(pool, |db| MediaDerivativeRepo::new(db)
            .refresh_original_reason(row.id(), reader.reason())
            .await)
        .map_err(db_error)?;
    }
    Ok(Some(AccessDecision {
        access,
        state,
        reason,
        tool,
    }))
}

/// The row one decision records: a copy waits `pending` for its command, and
/// anything else is settled `none` with the words a reader shows.
fn recorded(access: Access, queue: Queue) -> (DerivativeState, Option<String>, Option<String>) {
    match queue {
        Queue::None { reason } => {
            let tool = match access {
                Access::Copy { tools, .. } => tools.first().map(|tool| tool.command().to_string()),
                Access::Original(_) => None,
            };
            (DerivativeState::None, Some(reason), tool)
        }
        Queue::Pending { tool } => (DerivativeState::Pending, None, Some(tool.to_string())),
    }
}

/// The stored original a decision reads, from what the row says about it,
/// owned so it can travel to the blocking pool.
struct Source {
    storage_path: String,
    media_type: Option<String>,
    extension: Option<String>,
    size: Option<u64>,
}

impl Source {
    fn file(&self) -> StoredFile<'_> {
        StoredFile {
            storage_path: &self.storage_path,
            media_type: self.media_type.as_deref(),
            extension: self.extension.as_deref(),
            size: self.size,
        }
    }
}

/// The source one media row names, when it stored an original.
fn access_source(row: &MediaRow) -> Option<Source> {
    if !row.has_original {
        return None;
    }
    let storage_id = row.storage_id.as_deref()?;
    Some(Source {
        storage_path: storage_path(StorePrefix::Original, storage_id, row.extension.as_deref()),
        media_type: row.media_type.clone(),
        extension: row.extension.clone(),
        size: row.size.and_then(|size| u64::try_from(size).ok()),
    })
}

/// The decision and the queue row it records, or `None` when the type has no
/// access rule.
fn decide(store: &dyn Store, source: &Source) -> Option<(Access, Queue)> {
    let tools = Tools::on_path();
    let facts = access::facts(store, tools, &source.file());
    let access = Access::of(&facts)?;
    Some((access, access.queue(tools)))
}

/// Re-queue one media's access copy after its tool failed, or after a tool a
/// previous decision found missing was installed. `false` when the media has no
/// access row, or its row is waiting, ready, or settled because the original is
/// what the reader gets: there is nothing to make again.
///
/// # Errors
/// A database error reading or writing the row.
pub async fn retry_access(pool: &DbPool, media: omeka_db::ids::MediaId) -> Result<bool, ApiError> {
    with_db!(pool, |db| MediaDerivativeRepo::new(db)
        .retry(media, DerivativeKind::AccessCopy)
        .await)
    .map_err(db_error)
}

/// Queue a worker run when rows are waiting and no worker holds the queue.
///
/// One worker per install is the cap: a still is `ffmpeg` or `pdftoppm` on a file that may be
/// gigabytes, and the deployment is one box sharing its disk with the import. A composed job is
/// returned for the caller to start; `None` when nothing is queued or a worker is already
/// starting, running or stopping.
///
/// # Errors
/// A database error reading the queue or composing the job row.
pub async fn queue_worker(
    pool: &DbPool,
    owner_id: Option<omeka_db::ids::UserId>,
) -> Result<Option<JobId>, omeka_db::DbError> {
    let commands = Tools::on_path().derivative_commands();
    if with_db!(pool, |db| MediaDerivativeRepo::new(db)
        .pending(&commands)
        .await)?
        == 0
    {
        return Ok(None);
    }
    // A worker whose process died would hold the one-worker cap for good.
    crate::dead_jobs::settle(
        pool,
        Some(DERIVATIVES_JOB),
        crate::dead_jobs::Unstarted::Kept,
    )
    .await?;
    if with_db!(pool, |db| JobRepo::new(db)
        .active_of_class(DERIVATIVES_JOB)
        .await)?
    {
        return Ok(None);
    }
    let row = with_db!(pool, |db| JobRepo::new(db)
        .create(&NewJob {
            owner_id,
            pid: None,
            status: Some("starting".to_string()),
            class: DERIVATIVES_JOB.to_string(),
            args: Some(serde_json::json!({})),
            log: None,
            started: None,
        })
        .await)?;
    Ok(Some(row.id))
}

/// The `o-module-romeka:preview` member for one media.
///
/// `thumbnails_present` is what the store answers, so a media whose files exist reads ready even
/// when no row recorded it. Rows a build before this table wrote (the archive import's media)
/// therefore read the same way: from what the store holds.
#[must_use]
pub fn preview(rows: &[DerivativeRow], thumbnails_present: bool) -> MediaPreview {
    if thumbnails_present {
        return MediaPreview::ready();
    }
    match rows.iter().find(|row| row.kind.is_preview()) {
        None => MediaPreview::none(None),
        Some(row) => match row.state {
            DerivativeState::Pending => MediaPreview::pending(None),
            DerivativeState::Running => MediaPreview::pending(row.progress),
            DerivativeState::Ready => MediaPreview::ready(),
            DerivativeState::Failed => MediaPreview::failed(row.reason.clone()),
            DerivativeState::None => MediaPreview::none(row.reason.clone()),
        },
    }
}

/// The access copy one media has, as its document and the site read it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessCopy {
    pub state: DerivativeState,
    /// The words a reader shows for `none` or `failed`.
    pub reason: Option<String>,
    /// 0-100 while the transcode runs.
    pub progress: Option<u8>,
    /// The stored copy, when the row is ready and the type has a copy in the
    /// `access/` prefix.
    pub stored: Option<StoredCopy>,
}

/// Where an access copy is and what it answers with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredCopy {
    /// `access/<storage id>.<suffix>`, from [`omeka_media::access::copy_path`].
    pub path: String,
    pub media_type: &'static str,
}

/// The access decision one media recorded, or `None` when it stored none (an
/// embed, an import-made media before the backfill, a kind with no rule).
///
/// The row says where the work stands; the copy's own path and type come from
/// the stored type's target, which the row does not store.
#[must_use]
pub fn access_copy(
    rows: &[DerivativeRow],
    storage_id: Option<&str>,
    media_type: Option<&str>,
    extension: Option<&str>,
) -> Option<AccessCopy> {
    let row = rows
        .iter()
        .find(|row| row.kind == DerivativeKind::AccessCopy)?;
    let stored = storage_id
        .zip(media_type)
        .and_then(|(storage_id, media_type)| {
            let target = access::target_of(media_type, extension)?;
            (row.state == DerivativeState::Ready).then(|| StoredCopy {
                path: access::copy_path(storage_id, target),
                media_type: target.media_type(),
            })
        });
    Some(AccessCopy {
        state: row.state,
        reason: row.reason.clone(),
        progress: row.progress,
        stored,
    })
}

/// A derivative state as the media document reports it. `running` reads as
/// `pending`: from a reader's side both mean "not yet".
#[must_use]
pub const fn preview_state(state: DerivativeState) -> PreviewState {
    match state {
        DerivativeState::Pending | DerivativeState::Running => PreviewState::Pending,
        DerivativeState::Ready => PreviewState::Ready,
        DerivativeState::Failed => PreviewState::Failed,
        DerivativeState::None => PreviewState::None,
    }
}

/// What settling one queued row did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Settled {
    /// The row settled to this state, with the reason a reader shows.
    State {
        state: DerivativeState,
        reason: Option<String>,
    },
    /// The row is back in the queue: another tool makes this copy, or a running
    /// copy stopped and the next worker starts it again. Not a failure.
    Queued { reason: Option<String> },
    /// The media row is gone, so the cascade removed the derivative row too.
    Gone,
}

/// Totals of one drain.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DrainStats {
    pub ready: u32,
    pub skipped: u32,
    pub failed: u32,
    /// Rows left queued: a drain that stopped early, or a copy that went back
    /// to the queue, reports them.
    pub left: u32,
}

/// How often a running copy writes its percentage to the row. A transcode lasts
/// minutes and ffmpeg writes a record twice a second; one write every three
/// seconds keeps the row current without a write per record.
pub const COPY_PROGRESS_EVERY: Duration = Duration::from_secs(3);

/// How often a running copy looks for new progress, and how often (at most) it
/// asks the database whether it must stop.
pub const COPY_POLL: Duration = Duration::from_millis(100);

/// How often a running copy asks whether the job was stopped and whether an
/// import started. A transcode yields to both between progress updates, so a
/// reader waits a second at most.
pub const COPY_STATUS_EVERY: Duration = Duration::from_secs(1);

/// Claim one queued row and settle it: make the derivative, write the state,
/// and record `media.has_thumbnails` once the files are there.
///
/// `job` is the worker's own row, when the caller has one: a running copy stops
/// between progress updates when that row is asked to stop, and when an import
/// starts, and goes back to the queue. `None` (a test, the CLI backfill) never
/// stops a copy early.
///
/// `Ok(None)` when nothing is queued. The worker's yield checks between rows
/// live above this, so a caller that only wants the work done can drain the
/// queue without them.
///
/// # Errors
/// A database failure, or a blocking task that did not finish. A tool or a
/// decoder that refused is the row's `failed` state, not an error here.
pub async fn settle_next(
    pool: &DbPool,
    store: &Arc<dyn Store>,
    job: Option<JobId>,
) -> Result<Option<Settled>, ApiError> {
    settle_next_with(pool, store, Tools::on_path(), job).await
}

/// [`settle_next`] with the commands a caller names, instead of the ones this
/// process found on `PATH`. A test drives a long copy through a stand-in
/// `ffmpeg` this way; production passes [`Tools::on_path`].
///
/// # Errors
/// As [`settle_next`].
pub async fn settle_next_with(
    pool: &DbPool,
    store: &Arc<dyn Store>,
    tools: &Tools,
    job: Option<JobId>,
) -> Result<Option<Settled>, ApiError> {
    let commands = tools.derivative_commands();
    let Some(row) = with_db!(pool, |db| MediaDerivativeRepo::new(db)
        .claim_next(&commands)
        .await)
    .map_err(db_error)?
    else {
        return Ok(None);
    };
    let media =
        with_db!(pool, |db| MediaRepo::new(db).get(row.media_id).await).map_err(db_error)?;
    let Some(media) = media else {
        return Ok(Some(Settled::Gone));
    };
    if row.kind == DerivativeKind::AccessCopy {
        return settle_access(pool, store, tools, job, &row, &media).await;
    }
    settle_preview(pool, store, &row, &media).await
}

/// Settle one `access_copy` row with the worker its target belongs to: a video
/// or audio copy is a transcode, an office document's copy is the PDF conversion
/// on the same thread, and every other type's copy is a JPEG master.
async fn settle_access(
    pool: &DbPool,
    store: &Arc<dyn Store>,
    tools: &Tools,
    job: Option<JobId>,
    row: &DerivativeRow,
    media: &MediaRow,
) -> Result<Option<Settled>, ApiError> {
    let media_type = media.media_type.clone().unwrap_or_default();
    match access::target_of(&media_type, media.extension.as_deref()) {
        Some(target) => settle_copy(pool, store, tools, job, row, media, target).await,
        None => settle_image_master(pool, store, tools, row, media).await,
    }
}

/// Settle one image `access_copy` row: the JPEG master a PSD, a camera RAW, an
/// odd TIFF, a HEIC or a PostScript page needs, and the three thumbnails made
/// from it. The master runs on the blocking pool; the original is never changed.
async fn settle_image_master(
    pool: &DbPool,
    store: &Arc<dyn Store>,
    tools: &Tools,
    row: &DerivativeRow,
    media: &MediaRow,
) -> Result<Option<Settled>, ApiError> {
    let source = MasterSource {
        storage_id: media.storage_id.clone().unwrap_or_default(),
        extension: media.extension.clone(),
        media_type: media.media_type.clone(),
        has_original: media.has_original,
        size: media.size.and_then(|size| u64::try_from(size).ok()),
    };
    let storage_id = source.storage_id.clone();
    let worker_store = Arc::clone(store);
    let tools = tools.clone();
    let work =
        tokio::task::spawn_blocking(move || make_master(worker_store.as_ref(), &tools, &source))
            .await
            .map_err(|error| {
                ApiError::internal(format!("the access master did not finish: {error}"))
            })?;
    let state = match &work {
        MasterWork::Ready { .. } => DerivativeState::Ready,
        MasterWork::Skipped { .. } => DerivativeState::None,
        MasterWork::Failed { .. } => DerivativeState::Failed,
    };
    // The same undo race a copy has: the master and its thumbnails are written
    // before the row is settled, and a media removed meanwhile leaves files
    // nothing names.
    if state == DerivativeState::Ready && media_gone(pool, row.media_id).await? {
        remove_image_master(store, &storage_id);
        return Ok(Some(Settled::Gone));
    }
    with_db!(pool, |db| MediaDerivativeRepo::new(db)
        .finish(
            row.media_id,
            row.kind,
            state,
            work.reason().map(str::to_string),
            work.tool().map(str::to_string),
        )
        .await)
    .map_err(db_error)?;
    if state == DerivativeState::Ready {
        mark_thumbnails(pool, row.media_id).await?;
    }
    Ok(Some(Settled::State {
        state,
        reason: work.reason().map(str::to_string),
    }))
}

/// Settle one preview row: make the still and record its state.
async fn settle_preview(
    pool: &DbPool,
    store: &Arc<dyn Store>,
    row: &DerivativeRow,
    media: &MediaRow,
) -> Result<Option<Settled>, ApiError> {
    let source = PreviewSource {
        storage_id: media.storage_id.clone().unwrap_or_default(),
        extension: media.extension.clone(),
        media_type: media.media_type.clone(),
        has_original: media.has_original,
    };
    let store = Arc::clone(store);
    let work = tokio::task::spawn_blocking(move || make_preview(store.as_ref(), &source, false))
        .await
        .map_err(|error| {
            ApiError::internal(format!("the derivative worker did not finish: {error}"))
        })?;
    let state = match &work {
        DerivativeWork::Ready { .. } => DerivativeState::Ready,
        DerivativeWork::Skipped { .. } => DerivativeState::None,
        DerivativeWork::Failed { .. } => DerivativeState::Failed,
    };
    with_db!(pool, |db| MediaDerivativeRepo::new(db)
        .finish(
            row.media_id,
            row.kind,
            state,
            work.reason().map(str::to_string),
            work.tool().map(str::to_string),
        )
        .await)
    .map_err(db_error)?;
    if state == DerivativeState::Ready {
        mark_thumbnails(pool, row.media_id).await?;
    }
    Ok(Some(Settled::State {
        state,
        reason: work.reason().map(str::to_string),
    }))
}

/// Settle one `access_copy` row: decide the target from the stored type,
/// re-probe a video whose create-time probe failed, make the copy, and record
/// what happened.
///
/// The original is never changed. A copy that stops because the worker was
/// asked to or an import started goes back to the queue with nothing stored.
async fn settle_copy(
    pool: &DbPool,
    store: &Arc<dyn Store>,
    tools: &Tools,
    job: Option<JobId>,
    row: &DerivativeRow,
    media: &MediaRow,
    target: AccessTarget,
) -> Result<Option<Settled>, ApiError> {
    let media_type = media.media_type.clone().unwrap_or_default();
    let storage_id = media.storage_id.clone().unwrap_or_default();
    if target == AccessTarget::Mp4 && replayed_as_original(store, tools, media, &storage_id).await {
        // D2's open item: the create-time probe may have failed on a file a
        // browser plays. Re-probing costs a bounded sample; a transcode costs
        // the length of the file.
        let reason = access::OriginalReader::Browser.reason().to_string();
        settle_row(pool, row, DerivativeState::None, Some(reason.clone()), None).await?;
        return Ok(Some(Settled::State {
            state: DerivativeState::None,
            reason: Some(reason),
        }));
    }
    let source = TranscodeSource {
        storage_id: storage_id.clone(),
        extension: media.extension.clone(),
        media_type: Some(media_type),
        has_original: media.has_original,
    };
    let run = Transcode::start(Arc::clone(store), tools.clone(), source, target)
        .map_err(ApiError::internal)?;
    let work = wait_for_copy(pool, job, row, run).await?;
    let (state, reason) = match &work {
        TranscodeWork::Ready { .. } => (DerivativeState::Ready, None),
        TranscodeWork::Skipped { reason, .. } => (DerivativeState::None, Some(reason.clone())),
        TranscodeWork::Failed { reason, .. } => (DerivativeState::Failed, Some(reason.clone())),
        TranscodeWork::Stopped { .. } => {
            requeue(pool, row).await?;
            return Ok(Some(Settled::Queued {
                reason: Some("the copy stopped and waits for the next worker".to_string()),
            }));
        }
    };
    if state == DerivativeState::Ready {
        // An undo can remove the media while the copy runs. The copy is
        // written before this check, so a row that is gone leaves bytes that
        // nothing names: remove them here rather than waiting for a sweep.
        if media_gone(pool, row.media_id).await? {
            remove_copy(store, &storage_id, target);
            return Ok(Some(Settled::Gone));
        }
        // The first page a document's preview paints comes from this copy, so
        // the row is settled after those thumbnails exist. A reader between
        // the two then sees the copy running, not a media that reads `none`.
        poster_after_copy(pool, store, row, target).await?;
    }
    settle_row(pool, row, state, reason.clone(), work.tool()).await?;
    Ok(Some(Settled::State { state, reason }))
}

/// Wait for a running copy, writing its progress and stopping it when the job
/// is asked to stop or an import starts.
async fn wait_for_copy(
    pool: &DbPool,
    job: Option<JobId>,
    row: &DerivativeRow,
    run: Transcode,
) -> Result<TranscodeWork, ApiError> {
    let mut wrote = Instant::now()
        .checked_sub(COPY_PROGRESS_EVERY)
        .unwrap_or_else(Instant::now);
    let mut checked = Instant::now();
    loop {
        while let Some(percent) = run.try_progress() {
            if wrote.elapsed() >= COPY_PROGRESS_EVERY {
                with_db!(pool, |db| MediaDerivativeRepo::new(db)
                    .set_progress(row.media_id, row.kind, percent)
                    .await)
                .map_err(db_error)?;
                wrote = Instant::now();
            }
        }
        if run.is_finished() {
            return Ok(run.finish());
        }
        if checked.elapsed() >= COPY_STATUS_EVERY {
            checked = Instant::now();
            if stopping(pool, job).await?
                || with_db!(pool, |db| ImportRepo::new(db).busy_id().await)
                    .map_err(db_error)?
                    .is_some()
            {
                run.request_stop();
            }
        }
        tokio::time::sleep(COPY_POLL).await;
    }
}

/// Whether the worker's own job row was asked to stop.
async fn stopping(pool: &DbPool, job: Option<JobId>) -> Result<bool, ApiError> {
    let Some(job) = job else {
        return Ok(false);
    };
    Ok(with_db!(pool, |db| JobRepo::new(db).status(job).await)
        .map_err(db_error)?
        .as_deref()
        == Some("stopping"))
}

/// Whether a video the table planned a copy for is in fact one a browser plays,
/// re-probed from the stored file. Only video: the video table reads the
/// container first, so a re-probe can only answer "the original stands" for a
/// container a browser opens. The audio table reads the codec alone, and an
/// ASF container with an MP3 stream is still an ASF file.
async fn replayed_as_original(
    store: &Arc<dyn Store>,
    tools: &Tools,
    media: &MediaRow,
    storage_id: &str,
) -> bool {
    let file = StoredFile {
        storage_path: &storage_path(
            StorePrefix::Original,
            storage_id,
            media.extension.as_deref(),
        ),
        media_type: media.media_type.as_deref(),
        extension: media.extension.as_deref(),
        size: media.size.and_then(|size| u64::try_from(size).ok()),
    };
    let owned = (
        file.storage_path.to_string(),
        file.media_type.map(str::to_string),
        file.extension.map(str::to_string),
        file.size,
    );
    let store = Arc::clone(store);
    let tools = tools.clone();
    let probed = tokio::task::spawn_blocking(move || {
        let (path, media_type, extension, size) = &owned;
        let file = StoredFile {
            storage_path: path,
            media_type: media_type.as_deref(),
            extension: extension.as_deref(),
            size: *size,
        };
        access::facts(store.as_ref(), &tools, &file)
    })
    .await;
    match probed {
        Ok(facts) => matches!(Access::of(&facts), Some(Access::Original(_))),
        Err(_) => false,
    }
}

/// The three thumbnails a copy makes when the original's own still failed or was
/// never queued: the copy's frame, or its first page for a document, becomes the
/// thumbnails.
///
/// A preview row that is queued, running or ready already answers the poster, so
/// the work is not done twice; the copy's own still is the fallback for a media
/// whose preview is absent or failed.
async fn poster_after_copy(
    pool: &DbPool,
    store: &Arc<dyn Store>,
    row: &DerivativeRow,
    target: AccessTarget,
) -> Result<(), ApiError> {
    let Some(media) =
        with_db!(pool, |db| MediaRepo::new(db).get(row.media_id).await).map_err(db_error)?
    else {
        return Ok(());
    };
    let rows = with_db!(pool, |db| MediaDerivativeRepo::new(db)
        .for_media(row.media_id)
        .await)
    .map_err(db_error)?;
    let answered = rows.iter().any(|row| {
        row.kind.is_preview()
            && matches!(
                row.state,
                DerivativeState::Pending | DerivativeState::Running | DerivativeState::Ready
            )
    });
    if answered {
        return Ok(());
    }
    let Some(storage_id) = media.storage_id.clone() else {
        return Ok(());
    };
    let store = Arc::clone(store);
    let has_poster = thumbnails_stored(store.as_ref(), &storage_id).unwrap_or(false);
    if has_poster {
        return Ok(());
    }
    let copy = access::copy_path(&storage_id, target);
    let made = tokio::task::spawn_blocking(move || {
        thumbnails_from_copy(store.as_ref(), &storage_id, &copy, target)
    })
    .await
    .map_err(|error| ApiError::internal(format!("the poster did not finish: {error}")))?;
    if made {
        mark_thumbnails(pool, row.media_id).await?;
    }
    Ok(())
}

/// Whether the media the row names is still there. An undo removes the media
/// and the derivative rows cascade with it.
async fn media_gone(pool: &DbPool, media: omeka_db::ids::MediaId) -> Result<bool, ApiError> {
    Ok(with_db!(pool, |db| MediaRepo::new(db).get(media).await)
        .map_err(db_error)?
        .is_none())
}

/// Delete a copy whose media no longer exists. A missing file is already the
/// state wanted.
fn remove_copy(store: &Arc<dyn Store>, storage_id: &str, target: AccessTarget) {
    let path = access::copy_path(storage_id, target);
    // Nothing names the file any more, and no reader can reach it: a store that
    // refuses the delete leaves bytes a later sweep reports, not a broken read.
    let _ = store.delete(&path);
}

/// Delete the JPEG master and the three thumbnails whose media no longer
/// exists. A missing file is already the state wanted.
fn remove_image_master(store: &Arc<dyn Store>, storage_id: &str) {
    let _ = store.delete(&access::copy_path(storage_id, AccessTarget::Jpeg));
    for kind in ThumbnailType::ALL {
        let _ = store.delete(&omeka_media::thumbnailer::thumbnail_path(storage_id, kind));
    }
}

/// Put a claimed row back in the queue.
async fn requeue(pool: &DbPool, row: &DerivativeRow) -> Result<(), ApiError> {
    with_db!(pool, |db| MediaDerivativeRepo::new(db)
        .requeue(row.media_id, row.kind)
        .await)
    .map_err(db_error)?;
    Ok(())
}

/// Settle a claimed row to one state, recording the tool the work used.
async fn settle_row(
    pool: &DbPool,
    row: &DerivativeRow,
    state: DerivativeState,
    reason: Option<String>,
    tool: Option<&'static str>,
) -> Result<(), ApiError> {
    with_db!(pool, |db| MediaDerivativeRepo::new(db)
        .finish(
            row.media_id,
            row.kind,
            state,
            reason,
            tool.map(str::to_string),
        )
        .await)
    .map_err(db_error)?;
    Ok(())
}

/// Drain every queued row, counting what each settled to.
///
/// # Errors
/// Whatever [`settle_next`] returns.
pub async fn drain(pool: &DbPool, store: &Arc<dyn Store>) -> Result<DrainStats, ApiError> {
    let mut stats = DrainStats::default();
    while let Some(settled) = settle_next(pool, store, None).await? {
        match settled {
            Settled::State {
                state: DerivativeState::Ready,
                ..
            } => stats.ready += 1,
            Settled::State {
                state: DerivativeState::None,
                ..
            } => stats.skipped += 1,
            Settled::State { .. } => stats.failed += 1,
            Settled::Queued { .. } => {
                // A copy that went back to the queue stopped because the machine
                // was not free (an import started) or it was asked to. Claiming
                // it again would start the same copy at once, so the drain ends
                // and the job's own loop waits for the import.
                stats.left += 1;
                break;
            }
            Settled::Gone => {}
        }
    }
    Ok(stats)
}

/// Set `media.has_thumbnails` once the files are in the store.
async fn mark_thumbnails(pool: &DbPool, media: omeka_db::ids::MediaId) -> Result<(), ApiError> {
    let Some(row) = with_db!(pool, |db| omeka_db::repos::MediaRepo::new(db)
        .get(media)
        .await)
    .map_err(db_error)?
    else {
        return Ok(());
    };
    if row.has_thumbnails {
        return Ok(());
    }
    let mut updated = row;
    updated.has_thumbnails = true;
    with_db!(pool, |db| omeka_db::repos::MediaRepo::new(db)
        .update(&updated)
        .await)
    .map_err(db_error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDateTime;
    use omeka_db::ids::{ItemId, MediaId, ResourceId};
    use omeka_db::rows::{
        DerivativeKind, DerivativeRow, DerivativeState, ResourceRow, resource_type,
    };

    use crate::wire::media::PreviewState;

    fn derivative(
        state: DerivativeState,
        reason: Option<&str>,
        progress: Option<u8>,
    ) -> DerivativeRow {
        DerivativeRow {
            media_id: MediaId(7),
            kind: DerivativeKind::Poster,
            state,
            reason: reason.map(str::to_string),
            progress,
            tool: None,
            queued: NaiveDateTime::default(),
            started: None,
            finished: None,
            attempts: 1,
        }
    }

    fn media(media_type: &str, has_original: bool) -> MediaRow {
        MediaRow {
            resource: ResourceRow {
                id: ResourceId(7),
                owner_id: None,
                resource_class_id: None,
                resource_template_id: None,
                thumbnail_id: None,
                title: None,
                is_public: true,
                created: NaiveDateTime::default(),
                modified: None,
                resource_type: resource_type::MEDIA.to_string(),
            },
            item_id: ItemId(1),
            ingester: "upload".to_string(),
            renderer: "file".to_string(),
            data: None,
            source: Some("file".to_string()),
            media_type: Some(media_type.to_string()),
            storage_id: has_original.then(|| "stored".to_string()),
            extension: None,
            sha256: None,
            size: None,
            has_original,
            has_thumbnails: false,
            position: None,
            lang: None,
            alt_text: None,
        }
    }

    /// The archive import writes no derivative row: its document reads what the
    /// store holds, so every media reads the same way.
    #[test]
    fn a_media_with_files_and_no_row_reads_ready() {
        let preview = preview(&[], true);
        assert_eq!(preview.state, PreviewState::Ready);
        assert!(preview.reason.is_none());
    }

    #[test]
    fn a_media_with_no_files_and_no_row_reads_none() {
        assert_eq!(preview(&[], false).state, PreviewState::None);
    }

    #[test]
    fn a_queued_row_reads_pending() {
        let preview = preview(&[derivative(DerivativeState::Pending, None, None)], false);
        assert_eq!(preview.state, PreviewState::Pending);
        assert!(preview.progress.is_none());
    }

    #[test]
    fn a_claimed_row_reads_pending_with_its_progress() {
        let preview = preview(
            &[derivative(DerivativeState::Running, None, Some(40))],
            false,
        );
        assert_eq!(preview.state, PreviewState::Pending);
        assert_eq!(preview.progress, Some(40));
    }

    #[test]
    fn a_failed_row_carries_the_tool_reason() {
        let preview = preview(
            &[derivative(
                DerivativeState::Failed,
                Some("the pdftoppm command failed: exit status 1"),
                None,
            )],
            false,
        );
        assert_eq!(preview.state, PreviewState::Failed);
        assert_eq!(
            preview.reason.as_deref(),
            Some("the pdftoppm command failed: exit status 1")
        );
    }

    #[test]
    fn a_skipped_row_reads_none_with_its_reason() {
        let preview = preview(
            &[derivative(
                DerivativeState::None,
                Some("toolMissing: pdftoppm"),
                None,
            )],
            false,
        );
        assert_eq!(preview.state, PreviewState::None);
        assert_eq!(preview.reason.as_deref(), Some("toolMissing: pdftoppm"));
    }

    #[test]
    fn the_kind_follows_the_media_type() {
        assert_eq!(
            preview_kind(&media("video/mp4", true)),
            Some(DerivativeKind::Poster)
        );
        assert_eq!(
            preview_kind(&media("application/pdf", true)),
            Some(DerivativeKind::PdfPage)
        );
        assert_eq!(
            preview_kind(&media("image/jpeg", true)),
            Some(DerivativeKind::Thumbnail)
        );
        assert_eq!(
            preview_kind(&media("image/avif", true)),
            Some(DerivativeKind::Thumbnail)
        );
        assert_eq!(
            preview_kind(&media("image/x-nikon-nef", true)),
            Some(DerivativeKind::Thumbnail)
        );
        assert_eq!(preview_kind(&media("audio/mpeg", true)), None);
        assert_eq!(preview_kind(&media("video/mp4", false)), None);
    }

    /// A copy whose command is installed waits `pending` and names the command.
    #[test]
    fn a_pending_copy_records_the_command_it_waits_for() {
        let (state, reason, tool) = recorded(
            Access::Copy {
                target: omeka_media::access::AccessTarget::Mp4,
                tools: &[omeka_media::tool::AccessTool::Ffmpeg],
            },
            Queue::Pending { tool: "ffmpeg" },
        );
        assert_eq!(state, DerivativeState::Pending);
        assert!(reason.is_none());
        assert_eq!(tool.as_deref(), Some("ffmpeg"));
    }

    /// A copy whose command is missing is settled `none`, names the command it
    /// needs, and carries the words a reader shows.
    #[test]
    fn a_copy_with_no_command_records_none_and_the_reason() {
        let access = Access::Copy {
            target: omeka_media::access::AccessTarget::Jpeg,
            tools: &[omeka_media::tool::AccessTool::Vips],
        };
        let (state, reason, tool) = recorded(access, access.queue(&Tools::none()));
        assert_eq!(state, DerivativeState::None);
        assert_eq!(reason.as_deref(), Some("needs vips, not installed"));
        assert_eq!(tool.as_deref(), Some("vips"));
    }

    /// The original stands, and the row says so in the words the admin shows.
    #[test]
    fn an_original_records_none_with_its_words() {
        let (state, reason, tool) = recorded(
            Access::Original(access::OriginalReader::Browser),
            Access::Original(access::OriginalReader::Browser).queue(&Tools::none()),
        );
        assert_eq!(state, DerivativeState::None);
        assert_eq!(reason.as_deref(), Some("a browser shows the original"));
        assert!(tool.is_none());
    }
}
