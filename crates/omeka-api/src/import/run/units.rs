//! One record, or one item with its media, in one transaction.
//!
//! An item's media are stored first, each on its own: a file that cannot be
//! read or is refused fails that media record and no other. Then one
//! transaction writes the item (unless an earlier attempt did), every media
//! whose bytes arrived, the ledger rows, and each before-image. If it does
//! not commit, every record in it fails and the stored bytes are removed.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use omeka_db::Backend;
use omeka_db::DbError;
use omeka_db::ids::{ItemId, ItemSetId, MediaId};
use omeka_db::repos::{
    BeforeImageRepo, ImportResourceRepo, ImportWrites, MediaFilter, MediaRepo, Paging,
};
use omeka_db::rows::{
    ImportChange, ImportResource, ImportTarget, NewImportItem, NewImportItemSet, NewImportMedia,
    NewImportResource,
};
use sqlx::{ColumnIndex, Decode, Encode, Executor, IntoArguments, Transaction, Type};

use super::fields::{Named, id};
use super::{Run, RunError};
use crate::import::media_in::{self, ArrivalError, Arrived};
use crate::import::model::{Fate, Parent, Record, RecordId};
use crate::import::plan::RecordKind;
use crate::messages::{self, ServerMessage};

/// The registry names of every message a run failure carries; the test
/// below holds each to a registered message, so a misspelt name fails.
const FAILURE_MESSAGES: [&str; 11] = [
    "import_file_unreadable",
    "import_file_refused",
    "import_file_not_stored",
    "import_source_refused",
    "IMPORT_MEDIA_NO_SOURCE",
    "IMPORT_MEDIA_NO_ITEM",
    "IMPORT_RESOURCE_GONE",
    "import_record_not_saved",
    "import_links_not_saved",
    "import_sites_not_joined",
    "import_record_not_deleted",
];

/// The catalogue key of a registered message, by its registry name.
pub(super) fn catalogue_key(name: &'static str) -> String {
    debug_assert!(FAILURE_MESSAGES.contains(&name), "{name}");
    ServerMessage { name, pattern: "" }.key()
}

/// Why a transaction did not commit.
#[derive(Debug)]
enum UnitError {
    Db(DbError),
    /// The resource a matched record acts on is gone.
    Gone,
}

impl From<DbError> for UnitError {
    fn from(error: DbError) -> Self {
        Self::Db(error)
    }
}

impl From<sqlx::Error> for UnitError {
    fn from(error: sqlx::Error) -> Self {
        Self::Db(error.into())
    }
}

/// One media record of a unit and what the store holds for it.
struct Media<'m> {
    record: &'m Record,
    arrived: Option<Arrived>,
}

/// What a committed unit wrote, by record.
type Wrote = Vec<(RecordId, String, ImportTarget)>;

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
    fn named(&self) -> Named<'_> {
        Named {
            lookup: &self.lookup,
            plan: &self.derived.plan,
            runner: self.runner,
            set_records: &self.set_ids,
        }
    }

    /// Every item-set record's set, for the items that name it.
    pub(super) fn set_records(&self) -> HashMap<RecordId, ItemSetId> {
        self.model()
            .kept()
            .filter(|record| record.kind == RecordKind::ItemSet)
            .filter_map(|record| match self.target_of(record.id)? {
                ImportTarget::ItemSet(id) => Some((record.id, id)),
                _ => None,
            })
            .collect()
    }

    pub(super) async fn item_set(&mut self, record: &'r Record) -> Result<(), RunError> {
        let key = record.origin.key();
        if self.written.holds(&key) {
            return Ok(());
        }
        self.done();
        let mut tx = self.pool.begin().await?;
        let wrote = self.write_item_set(&mut tx, record).await;
        self.settle(tx, wrote, Vec::new(), &key).await
    }

    async fn write_item_set(
        &self,
        tx: &mut Transaction<'static, DB>,
        record: &Record,
    ) -> Result<Wrote, UnitError> {
        let named = self.named();
        let sites = Named::sites(record);
        let target = match record.fate {
            Fate::Act(raw) => {
                let set: ItemSetId = id(raw).ok_or(UnitError::Gone)?;
                self.before(tx, ImportResource::ItemSet(set)).await?;
                let fields = named.changed(record, RecordKind::ItemSet);
                let slot = named.item_slots(record).sites;
                if !ImportWrites::<DB>::new(tx)
                    .apply_item_set(set, self.action, &fields, &slot)
                    .await?
                {
                    return Err(UnitError::Gone);
                }
                ImportTarget::ItemSet(set)
            }
            _ => {
                let new = NewImportItemSet {
                    fields: named.created(record),
                    is_open: false,
                    sites,
                };
                ImportTarget::ItemSet(ImportWrites::<DB>::new(tx).create_item_set(&new).await?)
            }
        };
        self.ledger(tx, record, target).await?;
        Ok(vec![(record.id, record.origin.key(), target)])
    }

    pub(super) async fn item(&mut self, record: &'r Record) -> Result<(), RunError> {
        let key = record.origin.key();
        let derived: &'r crate::import::preview::Derived = self.derived;
        let model = &derived.model;
        let mut media = Vec::new();
        for at in &record.media {
            let one = model.record(*at);
            if self.written.holds(&one.origin.key()) {
                continue;
            }
            let arrived = if one.fate == Fate::Create {
                match self.arrive(one).await? {
                    Ok(arrived) => Some(arrived),
                    Err((name, detail)) => {
                        self.done();
                        self.fail(one.origin.key(), &catalogue_key(name), detail);
                        continue;
                    }
                }
            } else {
                None
            };
            media.push(Media {
                record: one,
                arrived,
            });
        }
        if self.written.holds(&key) {
            if media.is_empty() {
                return Ok(());
            }
        } else {
            self.done();
        }
        let mut tx = self.pool.begin().await?;
        let wrote = self.write_item(&mut tx, record, &media).await;
        let stored = media
            .iter()
            .filter_map(|one| one.arrived.as_ref())
            .flat_map(|arrived| arrived.stored.clone())
            .collect();
        let created = wrote.as_ref().ok().and_then(|wrote| {
            wrote.iter().find_map(|(at, _, target)| match target {
                ImportTarget::Item(item) if *at == record.id && record.fate == Fate::Create => {
                    Some(*item)
                }
                _ => None,
            })
        });
        let media_count = media.len();
        self.settle(tx, wrote, stored, &key).await?;
        self.summary.done += media_count;
        if let Some(item) = created
            && self.written.holds(&key)
            && let Err(error) = self.env.sites.item_created(item).await
        {
            self.fail(
                key,
                &catalogue_key("import_sites_not_joined"),
                messages::import_sites_not_joined(error),
            );
        }
        Ok(())
    }

    async fn write_item(
        &self,
        tx: &mut Transaction<'static, DB>,
        record: &Record,
        media: &[Media<'_>],
    ) -> Result<Wrote, UnitError> {
        let named = self.named();
        let mut wrote = Vec::new();
        let (item, mut position) = match (self.target_of(record.id), record.fate) {
            (Some(ImportTarget::Item(item)), Fate::Create) => (item, 0),
            (_, Fate::Act(raw)) if !self.written.holds(&record.origin.key()) => {
                let item: ItemId = id(raw).ok_or(UnitError::Gone)?;
                let held = self.before(tx, ImportResource::Item(item)).await?;
                let fields = named.changed(record, RecordKind::Item);
                let slots = named.item_slots(record);
                if !ImportWrites::<DB>::new(tx)
                    .apply_item(item, self.action, &fields, &slots)
                    .await?
                {
                    return Err(UnitError::Gone);
                }
                self.ledger(tx, record, ImportTarget::Item(item)).await?;
                wrote.push((record.id, record.origin.key(), ImportTarget::Item(item)));
                (item, held)
            }
            (_, Fate::Act(raw)) => {
                let item: ItemId = id(raw).ok_or(UnitError::Gone)?;
                (item, self.media_count(item).await?)
            }
            _ => {
                let new = NewImportItem {
                    fields: named.created(record),
                    item_sets: named.sets(record),
                    sites: Named::sites(record),
                };
                let item = ImportWrites::<DB>::new(tx).create_item(&new).await?;
                self.ledger(tx, record, ImportTarget::Item(item)).await?;
                wrote.push((record.id, record.origin.key(), ImportTarget::Item(item)));
                (item, 0)
            }
        };
        for one in media {
            let target = self
                .write_media(tx, item, one, &mut position, record)
                .await?;
            wrote.push((one.record.id, one.record.origin.key(), target));
        }
        if record.fate == Fate::Create {
            let first = record.media.first().and_then(|first| {
                wrote
                    .iter()
                    .find(|(at, _, _)| at == first)
                    .map(|(_, _, target)| *target)
                    .or_else(|| self.target_of(*first))
            });
            if let Some(ImportTarget::Media(primary)) = first {
                ImportWrites::<DB>::new(tx)
                    .set_primary_media(item, Some(primary))
                    .await?;
            }
        }
        Ok(wrote)
    }

    /// One media under `item`. A created item numbers its media by their
    /// place in media order; an item in the install gets them after its own.
    async fn write_media(
        &self,
        tx: &mut Transaction<'static, DB>,
        item: ItemId,
        one: &Media<'_>,
        position: &mut i64,
        parent: &Record,
    ) -> Result<ImportTarget, UnitError> {
        let record = one.record;
        let named = self.named();
        let target = match (record.fate, &one.arrived) {
            (Fate::Act(raw), _) => {
                let media: MediaId = id(raw).ok_or(UnitError::Gone)?;
                self.before(tx, ImportResource::Media(media)).await?;
                let fields = named.changed(record, RecordKind::Media);
                if !ImportWrites::<DB>::new(tx)
                    .apply_media(media, self.action, &fields)
                    .await?
                {
                    return Err(UnitError::Gone);
                }
                ImportTarget::Media(media)
            }
            (_, Some(arrived)) => {
                let place = if parent.fate == Fate::Create {
                    parent
                        .media
                        .iter()
                        .position(|at| *at == record.id)
                        .map_or(*position + 1, |at| at as i64 + 1)
                } else {
                    *position += 1;
                    *position
                };
                let new = NewImportMedia {
                    item,
                    fields: named.created(record),
                    file: arrived.file.clone(),
                    ingester: arrived.ingester.clone(),
                    renderer: arrived.renderer.clone(),
                    data: arrived.data.clone(),
                    source: arrived.source.clone(),
                    position: place,
                };
                ImportTarget::Media(ImportWrites::<DB>::new(tx).create_media(&new).await?)
            }
            (_, None) => return Err(UnitError::Gone),
        };
        self.ledger(tx, record, target).await?;
        Ok(target)
    }

    /// A media record of an item already in the install.
    pub(super) async fn loose_media(&mut self, record: &'r Record) -> Result<(), RunError> {
        let key = record.origin.key();
        if self.written.holds(&key) {
            return Ok(());
        }
        self.done();
        let Some(Parent::Existing(raw)) = record.parent else {
            self.fail(
                key,
                &catalogue_key("IMPORT_MEDIA_NO_ITEM"),
                messages::IMPORT_MEDIA_NO_ITEM.to_string(),
            );
            return Ok(());
        };
        let Some(item) = id::<ItemId>(raw) else {
            return Ok(());
        };
        let arrived = if record.fate == Fate::Create {
            match self.arrive(record).await? {
                Ok(arrived) => Some(arrived),
                Err((name, detail)) => {
                    self.fail(key, &catalogue_key(name), detail);
                    return Ok(());
                }
            }
        } else {
            None
        };
        let stored = arrived
            .as_ref()
            .map(|arrived| arrived.stored.clone())
            .unwrap_or_default();
        let one = Media { record, arrived };
        let mut position = self.media_count(item).await?;
        let mut tx = self.pool.begin().await?;
        let wrote = self
            .write_media(&mut tx, item, &one, &mut position, record)
            .await
            .map(|target| vec![(record.id, key.clone(), target)]);
        self.settle(tx, wrote, stored, &key).await
    }

    async fn media_count(&self, item: ItemId) -> Result<i64, DbError> {
        let filter = MediaFilter {
            item_id: Some(item),
            ..MediaFilter::default()
        };
        let all = MediaRepo::<DB>::new(self.pool)
            .list(&filter, &Paging::default())
            .await?;
        Ok(all.len() as i64)
    }

    /// Store the before-image of a resource the record changes, and answer
    /// how many media an item has, which new media follow.
    async fn before(
        &self,
        tx: &mut Transaction<'static, DB>,
        target: ImportResource,
    ) -> Result<i64, UnitError> {
        let image = ImportWrites::<DB>::new(tx)
            .before_image(target)
            .await?
            .ok_or(UnitError::Gone)?;
        let count = match &image.memberships {
            omeka_db::rows::ImageMemberships::Item { media_order, .. } => media_order.len() as i64,
            _ => 0,
        };
        BeforeImageRepo::<DB>::put(&mut **tx, self.import, target.target(), &image.to_json())
            .await?;
        Ok(count)
    }

    async fn ledger(
        &self,
        tx: &mut Transaction<'static, DB>,
        record: &Record,
        target: ImportTarget,
    ) -> Result<(), DbError> {
        let change = match record.fate {
            Fate::Act(_) => ImportChange::Changed,
            _ => ImportChange::Created,
        };
        ImportResourceRepo::<DB>::record(
            &mut **tx,
            self.import,
            &NewImportResource {
                target,
                record: record.origin.key(),
                change,
            },
        )
        .await?;
        Ok(())
    }

    /// Commit a unit, or fail it: roll back, remove its bytes, list it.
    async fn settle(
        &mut self,
        tx: Transaction<'static, DB>,
        wrote: Result<Wrote, UnitError>,
        stored: Vec<String>,
        key: &str,
    ) -> Result<(), RunError> {
        let outcome = match wrote {
            Ok(wrote) => tx.commit().await.map(|()| wrote).map_err(UnitError::from),
            Err(error) => {
                drop(tx);
                Err(error)
            }
        };
        match outcome {
            Ok(wrote) => {
                for (at, record, target) in wrote {
                    self.ids.insert(at, target);
                    self.written.add(record, target);
                }
            }
            Err(error) => {
                let store = Arc::clone(&self.env.store);
                tokio::task::spawn_blocking(move || media_in::remove(store.as_ref(), &stored))
                    .await
                    .map_err(|error| RunError::State(error.to_string()))?;
                let (name, detail) = match error {
                    UnitError::Gone => (
                        "IMPORT_RESOURCE_GONE",
                        messages::IMPORT_RESOURCE_GONE.to_string(),
                    ),
                    UnitError::Db(error) => (
                        "import_record_not_saved",
                        messages::import_record_not_saved(error),
                    ),
                };
                self.fail(key.to_string(), &catalogue_key(name), detail);
            }
        }
        Ok(())
    }

    /// A media record's bytes into the store, or the message why not.
    async fn arrive(
        &self,
        record: &Record,
    ) -> Result<Result<Arrived, (&'static str, String)>, RunError> {
        let store = Arc::clone(&self.env.store);
        let validation = self.env.validation.clone();
        if let Some(file) = record.files.first() {
            let shown = format!("{}/{}", file.source.0, file.path.as_str());
            let entry = self
                .derived
                .stored
                .index
                .sources
                .iter()
                .find(|source| source.id == file.source)
                .and_then(|source| {
                    source
                        .walk
                        .entries
                        .binary_search_by(|entry| entry.path.cmp(&file.path))
                        .ok()
                        .map(|at| source.walk.entries[at].disk_path())
                });
            let name = Path::new(file.path.as_str())
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            if let Some((stored, size)) = self.staged_file(&file.source, file.path.as_str()) {
                let arrived = tokio::task::spawn_blocking(move || {
                    media_in::arrive_stored(store.as_ref(), &validation, &stored, size, &name)
                })
                .await
                .map_err(|error| RunError::State(error.to_string()))?;
                return Ok(arrived.map_err(|error| error.failure(&shown)));
            }
            let located = match (self.dirs.get(&file.source), entry) {
                (Some(dir), Some(disk)) => dir.locate(&disk).map_err(|error| error.to_string()),
                _ => Err("the source folder is not available".to_string()),
            };
            let path = match located {
                Ok(path) => path,
                Err(reason) => {
                    return Ok(Err((
                        "import_file_unreadable",
                        messages::import_file_unreadable(&shown, reason),
                    )));
                }
            };
            let arrived = tokio::task::spawn_blocking(move || {
                media_in::arrive_file(store.as_ref(), &validation, &path, &name)
            })
            .await
            .map_err(|error| RunError::State(error.to_string()))?;
            return Ok(arrived.map_err(|error| error.failure(&shown)));
        }
        let (Some(ingester), Some(source)) = (
            record.resolved.ingester.clone(),
            record.resolved.media_source.clone(),
        ) else {
            return Ok(Err((
                "IMPORT_MEDIA_NO_SOURCE",
                messages::IMPORT_MEDIA_NO_SOURCE.to_string(),
            )));
        };
        let http = self.env.http.clone();
        let shown = source.clone();
        let arrived = tokio::task::spawn_blocking(move || {
            media_in::arrive_url(store.as_ref(), &validation, &http, &ingester, &source)
        })
        .await
        .map_err(|error| RunError::State(error.to_string()))?;
        Ok(arrived.map_err(|error| match error {
            ArrivalError::Store(reason) => (
                "import_file_not_stored",
                messages::import_file_not_stored(&shown, reason),
            ),
            ArrivalError::Refused(reason) | ArrivalError::Unreadable(reason) => (
                "import_source_refused",
                messages::import_source_refused(&shown, reason),
            ),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_failure_names_a_registered_message() {
        for name in FAILURE_MESSAGES {
            let registered = messages::REGISTRY
                .iter()
                .find(|message| message.name.eq_ignore_ascii_case(name))
                .unwrap_or_else(|| panic!("{name} is not registered"));
            assert_eq!(catalogue_key(name), registered.key());
        }
    }
}
