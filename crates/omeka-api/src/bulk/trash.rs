//! Bulk delete: items move to a trash an undo empties back into place, and a purge empties for
//! good once the trash is older than [`crate::settings::TRASH_DAYS`] or an administrator asks.
//!
//! Each item is one transaction (`BulkRepo::trash_target`): its rows go, and its trash snapshot
//! is written with the target, so the item is in place or in the trash. Its files then move to
//! `trash/<path>` in the store. The two cannot share a transaction, so each file move converges
//! when it is retried: a file is at its own path or under `trash/`, and a move that finds the
//! target already there removes the leftover source. A crash between the rows and the files
//! leaves the files at their own paths, which the undo and the purge both accept.
//!
//! An undo moves the files back first and then the rows (`BulkRepo::untrash_target`); if the
//! rows stay in the trash (an id another row took), the files move back to the trash. The purge
//! claims a run before it touches a file, so an undo cannot start on a half-purged run.
//!
//! Rights: a ticked list needs `batch_delete` (author and up), everything that matches
//! `batch_delete_all` (editor and up), both re-read before each chunk; each item needs the delete
//! right the item `DELETE` checks, under its owner. An undo acts as the person who asked for it,
//! and puts an item back only when they could delete it, so an author restores her own items.

use std::collections::BTreeSet;

use chrono::{TimeDelta, Utc};
use omeka_core::permissions::Action;
use omeka_db::DbPool;
use omeka_db::ids::{BulkId, ItemId, JobId, ResourceId, UserId};
use omeka_db::repos::{BulkRepo, Trashed, Untrashed};
use omeka_db::rows::{BulkKind, BulkPhase, BulkRow, FailReason, NewBulk, TrashImage};
use omeka_db::with_db;
use omeka_media::store::{Store, StoreError};
use serde::Serialize;
use serde_json::Value;

use super::BulkError;
use super::scope::select;
use super::work::{Ground, Step, assignable, chunks, permitted};
use crate::errors::ApiError;
use crate::messages;
use crate::read::Api;
use crate::write::access::resource_entity;
use crate::write::files::media_paths;
use crate::write::fulltext;

const ITEM_CLASS: &str = "Omeka\\Entity\\Item";

/// How many items the delete preview names.
const SAMPLES: usize = 5;

/// Targets a purge clears per read.
const PURGE_BATCH: i64 = 100;

/// Where the trash keeps a stored file.
#[must_use]
pub fn trash_path(path: &str) -> String {
    format!("trash/{path}")
}

/// A delete preview's answer.
#[derive(Debug, Serialize)]
pub struct DeletePreview {
    pub id: u64,
    pub status: &'static str,
    pub kind: &'static str,
    /// The items the run visits: the selection, frozen.
    pub total: usize,
    /// Items the caller may not delete, which the run records as failed.
    pub denied: usize,
    /// The media of the items the caller may delete, which go with them.
    pub media: u64,
    /// Their originals' bytes, as each media row records its size.
    pub bytes: u64,
    pub samples: Vec<DeleteSample>,
}

#[derive(Debug, Serialize)]
pub struct DeleteSample {
    pub id: u64,
    pub title: Option<String>,
}

/// Whether `api`'s caller may delete an item that `owner` owns: the item `DELETE`'s right.
fn may_delete(api: &Api<'_>, owner: Option<UserId>) -> bool {
    resource_entity(api.actor(), ITEM_CLASS, owner).is_ok()
}

/// Freeze the selection of a delete and answer what it takes.
pub(super) async fn preview(api: &Api<'_>, query: &Value) -> Result<DeletePreview, BulkError> {
    let items = select(api, query).await?;
    let described = with_db!(api.pool(), |db| BulkRepo::new(db).described(&items).await)?;
    let mut allowed = Vec::new();
    let mut preview = DeletePreview {
        id: 0,
        status: omeka_db::rows::BulkStatus::Previewed.as_str(),
        kind: BulkKind::Delete.as_str(),
        total: items.len(),
        denied: 0,
        media: 0,
        bytes: 0,
        samples: Vec::new(),
    };
    for item in described {
        if !may_delete(api, item.fields.owner) {
            preview.denied += 1;
            continue;
        }
        allowed.push(item.item);
        if preview.samples.len() < SAMPLES {
            preview.samples.push(DeleteSample {
                id: item.item.0,
                title: item.title,
            });
        }
    }
    (preview.media, preview.bytes) = with_db!(api.pool(), |db| BulkRepo::new(db)
        .media_totals(&allowed)
        .await)?;
    let row = with_db!(api.pool(), |db| BulkRepo::new(db)
        .create(&NewBulk {
            owner_id: api.actor().user(),
            kind: BulkKind::Delete,
            query: query.clone(),
            change: super::BulkChange::Delete.to_json(),
            items: items.clone(),
        })
        .await)?;
    preview.id = row.id.0;
    Ok(preview)
}

/// The run of a delete: each pending item to the trash, as its owner, a chunk at a time.
pub(super) async fn run(
    ground: &Ground<'_>,
    row: &BulkRow,
    job: JobId,
    right: Action,
) -> Result<Step, BulkError> {
    let pool = ground.pool;
    chunks(pool, row, job, BulkPhase::Run, |items| async move {
        let api = permitted(ground, row.owner_id, right).await?;
        for item in items {
            let outcome = with_db!(pool, |db| BulkRepo::new(db)
                .trash_target(row.id, item, |image| {
                    (!may_delete(&api, image.owner())).then_some(FailReason::DeleteDenied)
                })
                .await)?;
            if let Trashed::Trashed(image) = outcome {
                move_files(ground.store.as_ref(), &image, Toward::Trash)?;
            }
        }
        Ok(())
    })
    .await
}

/// The undo of a delete: each item in the trash back in place, as the person who asked.
pub(super) async fn undo(
    ground: &Ground<'_>,
    row: &BulkRow,
    job: JobId,
    asker: Option<UserId>,
    right: Action,
) -> Result<Step, BulkError> {
    let pool = ground.pool;
    chunks(pool, row, job, BulkPhase::Undo, |items| async move {
        let api = permitted(ground, asker, right).await?;
        let sites = assignable(&api).await?;
        for item in items {
            put_back(ground, &api, row.id, item, &sites).await?;
        }
        Ok(())
    })
    .await
}

async fn put_back(
    ground: &Ground<'_>,
    api: &Api<'_>,
    bulk: BulkId,
    item: ItemId,
    sites: &BTreeSet<omeka_db::ids::SiteId>,
) -> Result<(), BulkError> {
    let pool = ground.pool;
    let Some(image) = with_db!(pool, |db| BulkRepo::new(db).trash_image(bulk, item).await)? else {
        return Ok(());
    };
    if !may_delete(api, image.owner()) {
        with_db!(pool, |db| BulkRepo::new(db)
            .note_refusal(bulk, item, FailReason::UndoDenied)
            .await)?;
        return Ok(());
    }
    let store = ground.store.as_ref();
    move_files(store, &image, Toward::Place)?;
    match with_db!(pool, |db| BulkRepo::new(db)
        .untrash_target(bulk, item, sites)
        .await)?
    {
        Untrashed::Restored { image, .. } => {
            for media in image.media_ids() {
                fulltext::reindex(api, "media", ResourceId(media)).await?;
            }
            fulltext::reindex(api, "items", item.as_resource()).await?;
        }
        Untrashed::Kept => move_files(store, &image, Toward::Trash)?,
        Untrashed::Settled | Untrashed::Purged => {}
    }
    Ok(())
}

/// Which way a file moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Toward {
    Trash,
    Place,
}

/// Move every file the snapshot's media own, each one converging when retried.
fn move_files(store: &dyn Store, image: &TrashImage, toward: Toward) -> Result<(), BulkError> {
    for media in image.media_files() {
        let paths = media_paths(
            Some(&media.storage_id),
            media.extension.as_deref(),
            media.has_original,
            media.has_thumbnails,
        );
        for path in paths {
            let trash = trash_path(&path);
            let (from, to) = match toward {
                Toward::Trash => (path.as_str(), trash.as_str()),
                Toward::Place => (trash.as_str(), path.as_str()),
            };
            settle_move(store, from, to).map_err(ApiError::internal)?;
        }
    }
    Ok(())
}

/// Move `from` to `to` unless `to` already holds the file: storage ids are unique, so a file at
/// `to` is this one, left by a move cut short between its copy and its delete.
fn settle_move(store: &dyn Store, from: &str, to: &str) -> Result<(), StoreError> {
    if store.exists(to)? {
        store.delete(from)?;
        return Ok(());
    }
    store.rename(from, to)?;
    Ok(())
}

/// Which trash a purge empties.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PurgeScope {
    /// The runs that ended more than the trash days ago.
    Due,
    /// Every run at rest, whatever its age.
    All,
}

/// What a purge emptied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Purged {
    /// The runs it claimed.
    pub runs: usize,
    /// The items whose snapshot and files it removed, a purge cut short included.
    pub items: usize,
}

/// Empty the trash `scope` names: claim the runs, then delete each item's files (a file already
/// gone is no error) and its snapshot. A purge cut short goes on at the next one.
///
/// # Errors
/// A storage failure or a store that refuses a delete; what was emptied stays emptied.
pub async fn purge(
    pool: &DbPool,
    store: &dyn Store,
    scope: PurgeScope,
) -> Result<Purged, BulkError> {
    let before = match scope {
        PurgeScope::All => None,
        PurgeScope::Due => {
            let days = crate::settings::trash_days(pool).await?;
            let days = i64::try_from(days).unwrap_or(i64::MAX);
            Some(Utc::now().naive_utc() - TimeDelta::try_days(days).unwrap_or(TimeDelta::MAX))
        }
    };
    let runs = with_db!(pool, |db| BulkRepo::new(db).claim_purge(before).await)?;
    let mut items = 0;
    loop {
        let batch = with_db!(pool, |db| BulkRepo::new(db).purged_trash(PURGE_BATCH).await)?;
        if batch.is_empty() {
            break;
        }
        for (bulk, item, image) in batch {
            empty_files(pool, store, &image).await?;
            with_db!(pool, |db| BulkRepo::new(db).clear_trash(bulk, item).await)?;
            items += 1;
        }
    }
    Ok(Purged {
        runs: runs.len(),
        items,
    })
}

/// Delete an item's files from the trash, and from their own paths when no media holds them:
/// a delete cut short before its files moved left them there.
async fn empty_files(
    pool: &DbPool,
    store: &dyn Store,
    image: &TrashImage,
) -> Result<(), BulkError> {
    for media in image.media_files() {
        let in_use = with_db!(pool, |db| BulkRepo::new(db)
            .storage_in_use(&media.storage_id)
            .await)?;
        let paths = media_paths(
            Some(&media.storage_id),
            media.extension.as_deref(),
            media.has_original,
            media.has_thumbnails,
        );
        for path in paths {
            store
                .delete(&trash_path(&path))
                .map_err(ApiError::internal)?;
            if !in_use {
                store.delete(&path).map_err(ApiError::internal)?;
            }
        }
    }
    Ok(())
}

/// `POST /api/romeka/bulk/trash/purge`: an administrator empties every trash at rest now.
///
/// # Errors
/// 403 for any other caller; a storage failure.
pub async fn empty_trash(api: &Api<'_>) -> Result<Purged, BulkError> {
    if !api.actor().is_admin() {
        return Err(BulkError::Api(ApiError::PermissionDenied {
            message: messages::permission_denied("purge", "Romeka\\Entity\\Bulk"),
        }));
    }
    let store = api
        .store()
        .ok_or_else(|| ApiError::internal("the trash cannot be emptied without a file store"))?;
    purge(api.pool(), store.as_ref(), PurgeScope::All).await
}
