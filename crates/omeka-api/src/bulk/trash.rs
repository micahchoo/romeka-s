//! Bulk delete: items move to a trash an undo empties back into place, and a purge empties for
//! good once the trash is older than [`crate::settings::TRASH_DAYS`] or an administrator asks.
//! A delete of media or of item sets ([`TrashKind`]) moves each ticked media, or set, alone into
//! the same trash: the admin's media page and set place trash one that way.
//!
//! Each record is one transaction (`BulkRepo::trash_target`): its rows go, and its trash snapshot
//! is written with the target, so the record is in place or in the trash. Its files then move to
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
//! `batch_delete_all` (editor and up), both re-read before each chunk; each record needs the
//! delete right its own `DELETE` checks, under its owner. Every role that may delete a media or a
//! set holds `batch_delete`, so whoever may delete one may trash it. Media and sets are named by
//! id only. An undo acts as the person who asked for it, and puts a record back only when they
//! could delete it, so an author restores her own.

use std::collections::BTreeSet;

use chrono::{TimeDelta, Utc};
use omeka_core::permissions::Action;
use omeka_db::DbPool;
use omeka_db::ids::{BulkId, ItemId, ItemSetId, JobId, MediaId, ResourceId, UserId};
use omeka_db::repos::{BulkRepo, ItemSetRepo, MediaRepo, Trashed, Untrashed};
use omeka_db::rows::{
    BulkKind, BulkPhase, BulkRow, FailReason, HistorySource, HistoryStamp, NewBulk, TrashImage,
    TrashKind,
};
use omeka_db::with_db;
use omeka_media::store::{Store, StoreError};
use serde::Serialize;
use serde_json::Value;

use super::BulkError;
use super::scope::{Scope, select};
use super::work::{Ground, Step, assignable, chunks, permitted};
use crate::errors::ApiError;
use crate::messages;
use crate::read::Api;
use crate::write::access::resource_entity;
use crate::write::files::media_paths;
use crate::write::fulltext;

/// The entity class whose delete right a record of `kind` needs.
const fn class(kind: TrashKind) -> &'static str {
    match kind {
        TrashKind::Item => "Omeka\\Entity\\Item",
        TrashKind::Media => "Omeka\\Entity\\Media",
        TrashKind::ItemSet => "Omeka\\Entity\\ItemSet",
    }
}

/// How many records the delete preview names.
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
#[serde(rename_all = "camelCase")]
pub struct DeletePreview {
    pub id: u64,
    pub status: &'static str,
    pub kind: &'static str,
    /// The records the run visits: the selection, frozen.
    pub total: usize,
    /// Records the caller may not delete, which the run records as failed.
    pub denied: usize,
    /// The media that go: those of the items the caller may delete, or the media themselves.
    pub media: u64,
    /// Their originals' bytes, as each media row records its size.
    pub bytes: u64,
    /// How many days the trash keeps them before a purge empties it.
    pub trash_days: u64,
    pub samples: Vec<DeleteSample>,
}

#[derive(Debug, Serialize)]
pub struct DeleteSample {
    pub id: u64,
    pub title: Option<String>,
}

/// Whether `api`'s caller may delete a record of `kind` that `owner` owns: its `DELETE`'s right.
fn may_delete(api: &Api<'_>, kind: TrashKind, owner: Option<UserId>) -> bool {
    resource_entity(api.actor(), class(kind), owner).is_ok()
}

/// One selected record as the preview weighs it.
struct Selected {
    id: u64,
    title: Option<String>,
    owner: Option<UserId>,
    /// A media's original size in bytes.
    bytes: u64,
}

/// Freeze the selection of a delete of `kind` records and answer what it takes. Media and sets
/// are named by id; those the caller may not see are left out, as a missing one is.
pub(super) async fn preview(
    api: &Api<'_>,
    query: &Value,
    scope: &Scope,
    kind: TrashKind,
) -> Result<DeletePreview, BulkError> {
    let selected = match (kind, scope) {
        (TrashKind::Item, _) => items(api, query).await?,
        (_, Scope::Ticked(ids)) => named(api, kind, ids).await?,
        (_, Scope::Matching) => {
            return Err(BulkError::Api(ApiError::bad_request(
                messages::import_request_unreadable(
                    "a delete of media or item sets names them in \"query.id\"",
                ),
            )));
        }
    };
    let mut allowed = Vec::new();
    let mut preview = DeletePreview {
        id: 0,
        status: omeka_db::rows::BulkStatus::Previewed.as_str(),
        kind: BulkKind::Delete.as_str(),
        total: selected.len(),
        denied: 0,
        media: 0,
        bytes: 0,
        trash_days: crate::settings::trash_days(api.pool()).await?,
        samples: Vec::new(),
    };
    for record in &selected {
        if !may_delete(api, kind, record.owner) {
            preview.denied += 1;
            continue;
        }
        allowed.push(ItemId(record.id));
        if kind == TrashKind::Media {
            preview.media += 1;
            preview.bytes += record.bytes;
        }
        if preview.samples.len() < SAMPLES {
            preview.samples.push(DeleteSample {
                id: record.id,
                title: record.title.clone(),
            });
        }
    }
    if kind == TrashKind::Item {
        (preview.media, preview.bytes) = with_db!(api.pool(), |db| BulkRepo::new(db)
            .media_totals(&allowed)
            .await)?;
    }
    // A target's id sits in `item_id` whatever its kind; the run's change names the kind.
    let targets: Vec<ItemId> = selected.iter().map(|record| ItemId(record.id)).collect();
    let row = with_db!(api.pool(), |db| BulkRepo::new(db)
        .create(&NewBulk {
            owner_id: api.actor().user(),
            kind: BulkKind::Delete,
            query: query.clone(),
            change: super::BulkChange::Delete(kind).to_json(),
            items: targets,
        })
        .await)?;
    preview.id = row.id.0;
    Ok(preview)
}

/// The items `query` selects for the caller.
async fn items(api: &Api<'_>, query: &Value) -> Result<Vec<Selected>, BulkError> {
    let items = select(api, query).await?;
    let described = with_db!(api.pool(), |db| BulkRepo::new(db).described(&items).await)?;
    Ok(described
        .into_iter()
        .map(|item| Selected {
            id: item.item.0,
            title: item.title,
            owner: item.fields.owner,
            bytes: 0,
        })
        .collect())
}

/// The media or sets among `ids` that the caller sees, ascending.
async fn named(api: &Api<'_>, kind: TrashKind, ids: &[ItemId]) -> Result<Vec<Selected>, BulkError> {
    let ids: BTreeSet<u64> = ids.iter().map(|id| id.0).collect();
    let mut selected = Vec::new();
    for id in ids {
        let found = match kind {
            TrashKind::Media => {
                with_db!(api.pool(), |db| MediaRepo::new(db).get(MediaId(id)).await)?.map(|row| {
                    let bytes = row.size.and_then(|size| u64::try_from(size).ok());
                    (row.resource, bytes.unwrap_or(0))
                })
            }
            TrashKind::ItemSet | TrashKind::Item => {
                with_db!(api.pool(), |db| ItemSetRepo::new(db)
                    .get(ItemSetId(id))
                    .await)?
                .map(|row| (row.resource, 0))
            }
        };
        let Some((resource, bytes)) = found else {
            continue;
        };
        if api.sees(resource.is_public, resource.owner_id) {
            selected.push(Selected {
                id,
                title: resource.title,
                owner: resource.owner_id,
                bytes,
            });
        }
    }
    Ok(selected)
}

/// The kind of record a delete run's targets are.
fn kind_of(row: &BulkRow) -> Result<TrashKind, BulkError> {
    match super::BulkChange::parse(&row.change).map_err(ApiError::internal)? {
        super::BulkChange::Delete(kind) => Ok(kind),
        super::BulkChange::Edit(_) => Err(ApiError::internal("an edit is not a delete").into()),
    }
}

/// The run of a delete: each pending record to the trash, as the run's owner, a chunk at a time.
pub(super) async fn run(
    ground: &Ground<'_>,
    row: &BulkRow,
    job: JobId,
    right: Action,
) -> Result<Step, BulkError> {
    let pool = ground.pool;
    let kind = kind_of(row)?;
    chunks(pool, row, job, BulkPhase::Run, |targets| async move {
        let api = permitted(ground, row.owner_id, right).await?;
        for target in targets {
            let outcome = with_db!(pool, |db| BulkRepo::new(db)
                .trash_target(row.id, kind, target, |image| {
                    (!may_delete(&api, kind, image.owner())).then_some(FailReason::DeleteDenied)
                })
                .await)?;
            if let Trashed::Trashed(image) = outcome {
                move_files(ground.store.as_ref(), &image, Toward::Trash)?;
                // An item's full text holds its media's.
                if let Some(item) = image.item_of_media() {
                    fulltext::reindex(&api, "items", item.as_resource()).await?;
                }
            }
        }
        Ok(())
    })
    .await
}

/// The undo of a delete: each record in the trash back in place, as the person who asked.
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
    if !may_delete(api, image.kind, image.owner()) {
        with_db!(pool, |db| BulkRepo::new(db)
            .note_refusal(bulk, item, FailReason::UndoDenied)
            .await)?;
        return Ok(());
    }
    let store = ground.store.as_ref();
    move_files(store, &image, Toward::Place)?;
    let stamp = HistoryStamp {
        user: api.actor().user(),
        source: HistorySource::BulkUndo(bulk),
    };
    match with_db!(pool, |db| BulkRepo::new(db)
        .untrash_target(bulk, item, sites, &stamp)
        .await)?
    {
        Untrashed::Restored { image, .. } => {
            for media in image.media_ids() {
                fulltext::reindex(api, "media", ResourceId(media)).await?;
            }
            let indexed = match image.item_of_media() {
                Some(owner) => ("items", owner.as_resource()),
                None => (image.kind.resource_name(), item.as_resource()),
            };
            fulltext::reindex(api, indexed.0, indexed.1).await?;
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

/// What a purge emptied. Each count is of the records whose snapshot and files it removed, a
/// purge cut short included.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Purged {
    /// The runs it claimed.
    pub runs: usize,
    pub items: usize,
    /// Media trashed alone; an item's media count with the item.
    pub media: usize,
    pub item_sets: usize,
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
    let mut purged = Purged {
        runs: runs.len(),
        ..Purged::default()
    };
    loop {
        let batch = with_db!(pool, |db| BulkRepo::new(db).purged_trash(PURGE_BATCH).await)?;
        if batch.is_empty() {
            break;
        }
        for (bulk, item, image) in batch {
            empty_files(pool, store, &image).await?;
            with_db!(pool, |db| BulkRepo::new(db).clear_trash(bulk, item).await)?;
            *match image.kind {
                TrashKind::Item => &mut purged.items,
                TrashKind::Media => &mut purged.media,
                TrashKind::ItemSet => &mut purged.item_sets,
            } += 1;
        }
    }
    Ok(purged)
}

/// Delete a record's files from the trash, and from their own paths when no media holds them:
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
