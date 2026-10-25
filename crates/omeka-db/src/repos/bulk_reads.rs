//! `romeka_bulk` reads over the items a run selected: what a preview shows of them, and what an
//! undo would find.

use std::collections::BTreeMap;

use sqlx::{ColumnIndex, Decode, Encode, Executor, IntoArguments, Type};

use super::bulk::{members_of, push_ids};
use super::{BulkRepo, DescribedItem, Presence, Statement, UndoCandidate, bind_id};
use crate::DbError;
use crate::backend::Backend;
use crate::ids::{BulkId, ItemId, PropertyId};
use crate::rows::{
    BeforeImage, Described, ItemMembers, TargetState, ValueContent, ValueRow, resource_type,
};

/// Rows per `IN (...)` list.
const CHUNK: usize = 500;

impl<DB: Backend> BulkRepo<'_, DB>
where
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
    usize: ColumnIndex<DB::Row>,
    for<'x> i64: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Option<i64>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> bool: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> String: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Option<String>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Vec<u8>: Decode<'x, DB> + Type<DB>,
    for<'x> Option<Vec<u8>>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> &'x str: Encode<'x, DB> + Type<DB> + ColumnIndex<DB::Row>,
    for<'x> f64: Decode<'x, DB> + Type<DB>,
{
    /// Every target an undo would deal with, with its item's state now.
    pub async fn undo_candidates(&self, id: BulkId) -> Result<Vec<UndoCandidate>, DbError> {
        let rows = sqlx::query(
            "SELECT t.item_id, t.state, t.written, t.image, r.modified, r.resource_type
             FROM romeka_bulk_target t LEFT JOIN resource r ON r.id = t.item_id
             WHERE t.bulk_id = ? AND t.state IN ('done', 'kept')
             ORDER BY t.item_id",
        )
        .bind(bind_id(id))
        .fetch_all(self.pool)
        .await?;
        let mut candidates = Vec::with_capacity(rows.len());
        for chunk in rows.chunks(CHUNK) {
            let items = chunk
                .iter()
                .map(|row| DB::id(row, "item_id"))
                .collect::<Result<Vec<ItemId>, _>>()?;
            let sets = self.members(&items, "item_item_set", "item_set_id").await?;
            let sites = self.members(&items, "item_site", "site_id").await?;
            for (row, item) in chunk.iter().zip(items) {
                let here =
                    DB::opt_text(row, "resource_type")?.as_deref() == Some(resource_type::ITEM);
                let image = DB::opt_json(row, "image")?.ok_or_else(|| DbError::Column {
                    column: "image",
                    reason: format!("the target {item} of bulk run {id} has no before-image"),
                })?;
                candidates.push(UndoCandidate {
                    item,
                    state: TargetState::parse(&DB::text(row, "state")?)?,
                    written: DB::opt_datetime(row, "written")?,
                    now: if here {
                        Presence::Here {
                            modified: DB::opt_datetime(row, "modified")?,
                        }
                    } else {
                        Presence::Gone
                    },
                    image: BeforeImage::from_json(&image)?,
                    members: ItemMembers {
                        item_sets: members_of(&sets, item),
                        sites: members_of(&sites, item),
                    },
                });
            }
        }
        Ok(candidates)
    }

    /// The values of `items` in the given properties, each item's in id order.
    pub async fn values(
        &self,
        items: &[ItemId],
        properties: &[PropertyId],
    ) -> Result<BTreeMap<ItemId, Vec<ValueContent>>, DbError> {
        let mut found: BTreeMap<ItemId, Vec<ValueContent>> = BTreeMap::new();
        if properties.is_empty() {
            return Ok(found);
        }
        for chunk in items.chunks(CHUNK) {
            let mut statement = Statement::<DB>::new("SELECT * FROM value");
            push_ids(&mut statement, " WHERE resource_id IN (", chunk)?;
            statement.push(" AND property_id IN (");
            for (index, property) in properties.iter().enumerate() {
                if index > 0 {
                    statement.push(", ");
                }
                statement.bind(bind_id(*property))?;
            }
            statement.push(") ORDER BY id");
            let (sql, arguments) = statement.into_parts();
            let rows = sqlx::query_with::<DB, _>(&sql, arguments)
                .fetch_all(self.pool)
                .await?;
            for row in &rows {
                let value = ValueRow::from_db_row::<DB>(row)?;
                found
                    .entry(ItemId(value.resource_id.0))
                    .or_default()
                    .push(ValueContent {
                        property_id: value.property_id,
                        value_resource_id: value.value_resource_id,
                        value_annotation_id: value.value_annotation_id,
                        r#type: value.r#type,
                        lang: value.lang,
                        value: value.value,
                        uri: value.uri,
                        is_public: value.is_public,
                    });
            }
        }
        Ok(found)
    }

    /// The describing fields of `items` that still exist, in id order.
    pub async fn described(&self, items: &[ItemId]) -> Result<Vec<DescribedItem>, DbError> {
        let mut found = Vec::new();
        for chunk in items.chunks(CHUNK) {
            let mut statement = Statement::<DB>::new(
                "SELECT r.id, r.title, r.is_public, r.owner_id, r.resource_class_id,
                        r.resource_template_id
                 FROM resource r JOIN item i ON i.id = r.id",
            );
            push_ids(&mut statement, " WHERE r.id IN (", chunk)?;
            statement.push(" ORDER BY r.id");
            let (sql, arguments) = statement.into_parts();
            let rows = sqlx::query_with::<DB, _>(&sql, arguments)
                .fetch_all(self.pool)
                .await?;
            let sets = self.members(chunk, "item_item_set", "item_set_id").await?;
            let sites = self.members(chunk, "item_site", "site_id").await?;
            for row in &rows {
                let item: ItemId = DB::id(row, "id")?;
                found.push(DescribedItem {
                    item,
                    title: DB::opt_text(row, "title")?,
                    fields: Described {
                        is_public: DB::boolean(row, "is_public")?,
                        owner: DB::opt_id(row, "owner_id")?,
                        class: DB::opt_id(row, "resource_class_id")?,
                        template: DB::opt_id(row, "resource_template_id")?,
                        item_sets: members_of(&sets, item),
                        sites: members_of(&sites, item),
                    },
                });
            }
        }
        Ok(found)
    }

    /// `(item_id, member)` of a membership table, for the items of one chunk.
    async fn members(
        &self,
        items: &[ItemId],
        table: &'static str,
        member: &'static str,
    ) -> Result<BTreeMap<ItemId, Vec<u64>>, DbError> {
        let mut statement =
            Statement::<DB>::new(&format!("SELECT item_id, {member} AS member FROM {table}"));
        push_ids(&mut statement, " WHERE item_id IN (", items)?;
        let (sql, arguments) = statement.into_parts();
        let rows = sqlx::query_with::<DB, _>(&sql, arguments)
            .fetch_all(self.pool)
            .await?;
        let mut members: BTreeMap<ItemId, Vec<u64>> = BTreeMap::new();
        for row in &rows {
            members
                .entry(DB::id(row, "item_id")?)
                .or_default()
                .push(DB::id(row, "member")?);
        }
        Ok(members)
    }
}
