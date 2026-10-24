//! What an undo does with each ledger row, decided by reads alone, so the
//! undo and its preview answer the same.
//!
//! The judge keeps what earlier rows decided: an item it keeps keeps its
//! media and the sets that hold it. Rows must come in undo order
//! ([`super::order`]).
//!
//! A template or field is in use only by what the undo leaves behind: the
//! judge remembers every row it acts on, so the preview, which deletes
//! nothing, counts the import's own items as gone, as the undo does.

use std::collections::{HashMap, HashSet};

use omeka_db::Backend;
use omeka_db::ids::{ImportId, ItemId, MediaId, ResourceId, TemplateId};
use omeka_db::repos::{ImportResourceRepo, ImportTouchRepo, MediaRepo, Presence};
use omeka_db::rows::{
    BeforeImage, ImageMemberships, ImportChange, ImportKind, ImportResourceRow, ImportTarget,
};
use sqlx::{ColumnIndex, Decode, Encode, Executor, IntoArguments, Pool, Type};

use super::UndoError;
use super::report::Force;
use crate::import::touched::{Own, Touch, resource, touched};

/// What the undo does with one row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Verdict {
    /// The resource is gone already.
    Gone,
    /// Something touched it and `force` does not name it.
    Keep(Vec<Touch>),
    /// Restore or delete it; `forced` when it was touched. `modified` is the
    /// resource's as judged: a restore that finds another one keeps the resource,
    /// because something edited it after the judge looked.
    Act {
        forced: bool,
        modified: Option<chrono::NaiveDateTime>,
    },
}

pub(super) struct Judge<'u, DB: Backend> {
    pub pool: &'u Pool<DB>,
    pub import: ImportId,
    pub force: Force,
    pub own: Own,
    pub images: HashMap<ImportTarget, BeforeImage>,
    /// What touched each item the import made, read once: its media ask
    /// before the item's own turn.
    verdicts: HashMap<ItemId, Vec<Touch>>,
    /// Items the undo keeps, whose media it keeps too.
    kept_items: HashSet<ItemId>,
    /// Resources the undo restores or deletes before it reaches a template or field.
    leaving_resources: HashSet<ResourceId>,
    /// Templates the undo deletes before it reaches a field.
    leaving_templates: HashSet<TemplateId>,
}

impl<'u, DB> Judge<'u, DB>
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
    pub fn new(
        pool: &'u Pool<DB>,
        import: ImportId,
        force: Force,
        rows: &[ImportResourceRow],
        images: HashMap<ImportTarget, BeforeImage>,
    ) -> Self {
        Self {
            pool,
            import,
            force,
            own: Own::from_rows(rows),
            images,
            verdicts: HashMap::new(),
            kept_items: HashSet::new(),
            leaving_resources: HashSet::new(),
            leaving_templates: HashSet::new(),
        }
    }

    pub async fn judge(&mut self, row: &ImportResourceRow) -> Result<Verdict, UndoError> {
        let target = row.target;
        let reads = ImportTouchRepo::<DB>::new(self.pool);
        let Presence::Here { modified } = reads.presence(target).await? else {
            return Ok(Verdict::Gone);
        };
        let touches = match target {
            ImportTarget::ResourceTemplate(id) => {
                let users = reads.template_users(id).await?;
                self.in_use(
                    users
                        .iter()
                        .any(|user| !self.leaving_resources.contains(user)),
                )
            }
            ImportTarget::Property(id) => {
                let users = reads.property_users(id).await?;
                self.in_use(
                    users
                        .resources
                        .iter()
                        .any(|user| !self.leaving_resources.contains(user))
                        || users
                            .templates
                            .iter()
                            .any(|user| !self.leaving_templates.contains(user)),
                )
            }
            ImportTarget::Item(item) if row.change == ImportChange::Created => {
                self.verdict(row, item, modified).await?
            }
            ImportTarget::Media(media) if row.change == ImportChange::Created => {
                let mut touches = self.read_touched(row, modified).await?;
                if self.item_is_kept(media).await? {
                    touches.push(Touch::ItemKept);
                }
                touches
            }
            ImportTarget::ItemSet(set) if row.change == ImportChange::Created => {
                let mut touches = self.read_touched(row, modified).await?;
                let holds_kept = reads
                    .set_members(set)
                    .await?
                    .iter()
                    .any(|member| self.kept_items.contains(&ItemId(member.resource.0)));
                if holds_kept {
                    touches.push(Touch::MemberKept);
                }
                touches
            }
            _ => self.read_touched(row, modified).await?,
        };
        if touches.is_empty() {
            self.leaves(target);
            return Ok(Verdict::Act {
                forced: false,
                modified,
            });
        }
        if self.force.covers(target) {
            self.leaves(target);
            return Ok(Verdict::Act {
                forced: true,
                modified,
            });
        }
        if let ImportTarget::Item(item) = target {
            self.kept_items.insert(item);
        }
        Ok(Verdict::Keep(touches))
    }

    fn in_use(&self, used: bool) -> Vec<Touch> {
        if used { vec![Touch::InUse] } else { Vec::new() }
    }

    fn leaves(&mut self, target: ImportTarget) {
        if let Some(id) = resource(target) {
            self.leaving_resources.insert(id);
        }
        if let ImportTarget::ResourceTemplate(id) = target {
            self.leaving_templates.insert(id);
        }
    }

    async fn read_touched(
        &self,
        row: &ImportResourceRow,
        modified: Option<chrono::NaiveDateTime>,
    ) -> Result<Vec<Touch>, UndoError> {
        let before = match self.images.get(&row.target).map(|image| &image.memberships) {
            Some(ImageMemberships::Item { media_order, .. }) => media_order.as_slice(),
            _ => &[],
        };
        Ok(touched(self.pool, self.import, row, modified, &self.own, before).await?)
    }

    async fn verdict(
        &mut self,
        row: &ImportResourceRow,
        item: ItemId,
        modified: Option<chrono::NaiveDateTime>,
    ) -> Result<Vec<Touch>, UndoError> {
        if let Some(touches) = self.verdicts.get(&item) {
            return Ok(touches.clone());
        }
        let touches = self.read_touched(row, modified).await?;
        self.verdicts.insert(item, touches.clone());
        Ok(touches)
    }

    /// Whether the undo keeps the media's item: a changed item kept in the
    /// first pass, or an item this import made that is touched and not forced.
    async fn item_is_kept(&mut self, media: MediaId) -> Result<bool, UndoError> {
        let Some(row) = MediaRepo::<DB>::new(self.pool).get(media).await? else {
            return Ok(false);
        };
        let item = row.item_id;
        if self.kept_items.contains(&item) {
            return Ok(true);
        }
        let target = ImportTarget::Item(item);
        if !self.own.holds(target) || self.force.covers(target) {
            return Ok(false);
        }
        let ledger = ImportResourceRepo::<DB>::new(self.pool)
            .list(self.import, Some(ImportKind::Item))
            .await?;
        let Some(item_row) = ledger
            .iter()
            .find(|row| row.target == target && row.change == ImportChange::Created)
        else {
            return Ok(false);
        };
        let Presence::Here { modified } = ImportTouchRepo::<DB>::new(self.pool)
            .presence(target)
            .await?
        else {
            return Ok(false);
        };
        Ok(!self.verdict(item_row, item, modified).await?.is_empty())
    }
}
