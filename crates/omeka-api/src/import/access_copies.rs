//! The playable-copy decision for the media an import created.
//!
//! An API create records it when the media is written
//! (`derivatives::enqueue`). A run makes its previews inline and records
//! nothing, so the run's job asks [`queue`] once the import has ended: each
//! created audio, video or stored image gets the same `access_copy` row an API
//! create would. A PDF or any other type gets none.
//!
//! The pass reads the whole ledger, so a run killed before it and retried
//! still reaches the first attempt's media. A media that already holds an
//! `access_copy` row is skipped before its file is probed, so a retry
//! decides nothing twice.

use std::sync::Arc;

use omeka_db::ids::{ImportId, MediaId};
use omeka_db::repos::{ImportResourceRepo, MediaDerivativeRepo, MediaRepo};
use omeka_db::rows::{DerivativeKind, ImportChange, ImportKind, ImportTarget, MediaRow};
use omeka_db::{DbError, DbPool, with_db};
use omeka_media::store::Store;

use crate::derivatives::enqueue_access;

/// What one pass decided.
#[derive(Debug, Default)]
pub struct Queued {
    /// Media that got an `access_copy` row in this pass.
    pub decided: usize,
    /// Media whose decision failed, with the reason; the run stands.
    pub failures: Vec<(MediaId, String)>,
}

/// Record the access decision of every media `import` created that has none.
///
/// # Errors
/// A database error reading the ledger. One media's failure is listed in
/// [`Queued::failures`] instead.
pub async fn queue(
    pool: &DbPool,
    store: &Arc<dyn Store>,
    import: ImportId,
) -> Result<Queued, DbError> {
    let ledger = with_db!(pool, |db| ImportResourceRepo::new(db)
        .list(import, Some(ImportKind::Media))
        .await)?;
    let mut queued = Queued::default();
    for row in &ledger {
        let ImportTarget::Media(media) = row.target else {
            continue;
        };
        if row.change != ImportChange::Created {
            continue;
        }
        match decide(pool, store, media).await {
            Ok(true) => queued.decided += 1,
            Ok(false) => {}
            Err(reason) => queued.failures.push((media, reason)),
        }
    }
    Ok(queued)
}

/// `true` when this call recorded the media's decision.
async fn decide(pool: &DbPool, store: &Arc<dyn Store>, media: MediaId) -> Result<bool, String> {
    let row = with_db!(pool, |db| MediaRepo::new(db).get(media).await)
        .map_err(|error| error.to_string())?;
    let Some(row) = row.filter(playable_kind) else {
        return Ok(false);
    };
    let held = with_db!(pool, |db| MediaDerivativeRepo::new(db)
        .for_media(media)
        .await)
    .map_err(|error| error.to_string())?;
    if held
        .iter()
        .any(|derivative| derivative.kind == DerivativeKind::AccessCopy)
    {
        return Ok(false);
    }
    let decision = enqueue_access(pool, store, &row)
        .await
        .map_err(|error| error.to_string())?;
    Ok(decision.is_some())
}

/// Audio, video, or an image whose original is in the store.
fn playable_kind(row: &MediaRow) -> bool {
    row.has_original
        && row.media_type.as_deref().is_some_and(|media_type| {
            media_type.starts_with("audio/")
                || media_type.starts_with("video/")
                || media_type.starts_with("image/")
                || media_type == "application/ogg"
        })
}
