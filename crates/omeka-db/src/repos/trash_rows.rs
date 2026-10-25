//! The rows of a trash snapshot on one transaction: read an item's rows into a [`TrashImage`],
//! delete them, and insert them again.
//!
//! Every read and insert goes through a table's column list (`rows::trash::TABLES`), so a column
//! is read and written the same way on both engines. Deletes lean on the schema's foreign keys
//! where they cascade: a media's `resource` row takes its `media` row, notes, extent and
//! derivative states with it, and the item's takes its `item` row and memberships, and empties
//! the page block attachments that named it.

use std::collections::{BTreeMap, BTreeSet};

use chrono::NaiveDateTime;
use serde_json::Value;
use sqlx::{ColumnIndex, Decode, Encode, Executor, IntoArguments, Type};

use super::{Statement, bind_id};
use crate::DbError;
use crate::backend::Backend;
use crate::ids::ItemId;
use crate::rows::trash::{
    Cell, ITEM, ITEM_ITEM_SET, ITEM_SITE, MEDIA, RESOURCE, TABLES, Table, VALUE, VALUE_ANNOTATION,
    int,
};
use crate::rows::{AttachmentLink, IncomingLink, TrashImage, TrashRow};
use crate::time;

/// Ids per `IN (...)` list.
const CHUNK: usize = 500;

/// The trash reads and writes on one connection.
pub(super) struct TrashRows<'t, DB: Backend> {
    pub(super) conn: &'t mut DB::Connection,
}

impl<DB: Backend> TrashRows<'_, DB>
where
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
    usize: ColumnIndex<DB::Row>,
    for<'x> i64: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Option<i64>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> bool: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> String: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Option<String>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> &'x str: Encode<'x, DB> + Type<DB> + ColumnIndex<DB::Row>,
{
    /// Every row the item owns, and the rows of others that name it. `None` when no item has
    /// the id.
    pub(super) async fn snapshot(&mut self, item: ItemId) -> Result<Option<TrashImage>, DbError> {
        let id = item.0;
        let items = self.select(&ITEM, "id", &[id]).await?;
        if items.is_empty() {
            return Ok(None);
        }
        let media = self.select(&MEDIA, "item_id", &[id]).await?;
        let media_ids: Vec<u64> = media.iter().filter_map(|row| int(row, "id")).collect();
        let mut owners: Vec<u64> = [id].into_iter().chain(media_ids.iter().copied()).collect();
        let mut values = self.select(&VALUE, "resource_id", &owners).await?;
        let annotations: Vec<u64> = values
            .iter()
            .filter_map(|row| int(row, "value_annotation_id"))
            .collect();
        values.extend(self.select(&VALUE, "resource_id", &annotations).await?);
        let linkable = owners.clone();
        owners.extend(&annotations);
        let owned: BTreeSet<u64> = owners.iter().copied().collect();
        let mut tables = Vec::with_capacity(TABLES.len());
        for table in TABLES {
            let rows = match table.name {
                "resource" => self.select(&RESOURCE, "id", &owners).await?,
                "item" => items.clone(),
                "media" => media.clone(),
                "value_annotation" => self.select(&VALUE_ANNOTATION, "id", &annotations).await?,
                "value" => values.clone(),
                "item_item_set" => self.select(&ITEM_ITEM_SET, "item_id", &[id]).await?,
                "item_site" => self.select(&ITEM_SITE, "item_id", &[id]).await?,
                _ => self.select(table, "media_id", &media_ids).await?,
            };
            tables.push((table.name, rows));
        }
        let attachments = self.attachments(id, &media_ids).await?;
        let mut incoming = Vec::new();
        for value in self.select(&VALUE, "value_resource_id", &linkable).await? {
            let Some(resource) = int(&value, "resource_id").filter(|id| !owned.contains(id)) else {
                continue;
            };
            let modified = self.modified(resource).await?.flatten();
            incoming.push(IncomingLink { value, modified });
        }
        Ok(Some(TrashImage {
            tables,
            attachments,
            incoming,
        }))
    }

    /// Delete what `image` holds. The values that link in go first, then each resource's own
    /// values, then the resources, media before the item, whose `media.item_id` has no cascade.
    pub(super) async fn remove(&mut self, item: ItemId, image: &TrashImage) -> Result<(), DbError> {
        let incoming: Vec<u64> = image
            .incoming
            .iter()
            .filter_map(|link| int(&link.value, "id"))
            .collect();
        self.delete_in("value", "id", &incoming).await?;
        let resources: Vec<u64> = image.resources().into_iter().collect();
        self.delete_in("value", "resource_id", &resources).await?;
        let media = image.media_ids();
        self.delete_in("resource", "id", &media).await?;
        let annotations: Vec<u64> = image
            .rows(VALUE_ANNOTATION.name)
            .iter()
            .filter_map(|row| int(row, "id"))
            .collect();
        self.delete_in("resource", "id", &annotations).await?;
        self.delete_in("resource", "id", &[item.0]).await?;
        sqlx::query("DELETE FROM fulltext_search WHERE resource = 'items' AND id = ?")
            .bind(bind_id(item))
            .execute(&mut *self.conn)
            .await?;
        for chunk in media.chunks(CHUNK) {
            let mut statement =
                Statement::<DB>::new("DELETE FROM fulltext_search WHERE resource = 'media'");
            push_in(&mut statement, " AND id", chunk)?;
            let (sql, arguments) = statement.into_parts();
            sqlx::query_with::<DB, _>(&sql, arguments)
                .execute(&mut *self.conn)
                .await?;
        }
        Ok(())
    }

    /// The rows of `table` whose `key` is one of `ids`, in the order of the table's first
    /// column.
    pub(super) async fn select(
        &mut self,
        table: &Table,
        key: &'static str,
        ids: &[u64],
    ) -> Result<Vec<TrashRow>, DbError> {
        let columns: Vec<String> = table
            .columns
            .iter()
            .map(|(name, _)| DB::quote_identifier(name).into_owned())
            .collect();
        let mut out = Vec::new();
        for chunk in ids.chunks(CHUNK) {
            let mut statement = Statement::<DB>::new(&format!(
                "SELECT {} FROM {} WHERE 1 = 1",
                columns.join(", "),
                DB::quote_identifier(table.name)
            ));
            push_in(&mut statement, &format!(" AND {key}"), chunk)?;
            statement.push(&format!(" ORDER BY {}", columns[0]));
            let (sql, arguments) = statement.into_parts();
            let rows = sqlx::query_with::<DB, _>(&sql, arguments)
                .fetch_all(&mut *self.conn)
                .await?;
            for row in &rows {
                let mut copied = TrashRow::new();
                for (name, cell) in table.columns {
                    copied.insert((*name).to_string(), cell_value::<DB>(row, name, *cell)?);
                }
                out.push(copied);
            }
        }
        Ok(out)
    }

    /// Insert one copied row with every column the table lists.
    pub(super) async fn insert(&mut self, table: &Table, row: &TrashRow) -> Result<(), DbError> {
        let columns: Vec<String> = table
            .columns
            .iter()
            .map(|(name, _)| DB::quote_identifier(name).into_owned())
            .collect();
        let mut statement = Statement::<DB>::new(&format!(
            "INSERT INTO {} ({}) VALUES (",
            DB::quote_identifier(table.name),
            columns.join(", ")
        ));
        for (at, (name, cell)) in table.columns.iter().enumerate() {
            if at > 0 {
                statement.push(", ");
            }
            let value = row.get(*name);
            match cell {
                Cell::Int => statement.bind(value.and_then(Value::as_i64))?,
                Cell::Text | Cell::Time => {
                    statement.bind(value.and_then(Value::as_str).map(str::to_string))?
                }
                Cell::Flag => statement.bind(value.and_then(Value::as_bool).unwrap_or(false))?,
            };
        }
        statement.push(")");
        let (sql, arguments) = statement.into_parts();
        sqlx::query_with::<DB, _>(&sql, arguments)
            .execute(&mut *self.conn)
            .await?;
        Ok(())
    }

    /// The ids among `ids` that a row of `table` holds in `column`.
    pub(super) async fn existing(
        &mut self,
        table: &'static str,
        column: &'static str,
        ids: &BTreeSet<u64>,
    ) -> Result<BTreeSet<u64>, DbError> {
        let ids: Vec<u64> = ids.iter().copied().collect();
        let mut found = BTreeSet::new();
        for chunk in ids.chunks(CHUNK) {
            let mut statement = Statement::<DB>::new(&format!(
                "SELECT {column} FROM {} WHERE 1 = 1",
                DB::quote_identifier(table)
            ));
            push_in(&mut statement, &format!(" AND {column}"), chunk)?;
            let (sql, arguments) = statement.into_parts();
            let raw: Vec<i64> = sqlx::query_scalar_with::<DB, i64, _>(&sql, arguments)
                .fetch_all(&mut *self.conn)
                .await?;
            found.extend(raw.into_iter().filter_map(|id| u64::try_from(id).ok()));
        }
        Ok(found)
    }

    /// Whether a media row holds one of `storage_ids`; the column is unique.
    pub(super) async fn storage_taken(&mut self, storage_ids: &[String]) -> Result<bool, DbError> {
        for storage_id in storage_ids {
            let row = sqlx::query("SELECT id FROM media WHERE storage_id = ?")
                .bind(storage_id.clone())
                .fetch_optional(&mut *self.conn)
                .await?;
            if row.is_some() {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// A resource's `modified`: `None` when no resource has the id, `Some(None)` when it was
    /// never modified.
    pub(super) async fn modified(
        &mut self,
        resource: u64,
    ) -> Result<Option<Option<NaiveDateTime>>, DbError> {
        let row = sqlx::query("SELECT modified FROM resource WHERE id = ?")
            .bind(bind_id(resource))
            .fetch_optional(&mut *self.conn)
            .await?;
        row.map(|row| DB::opt_datetime(&row, "modified"))
            .transpose()
    }

    async fn attachments(
        &mut self,
        item: u64,
        media: &[u64],
    ) -> Result<Vec<AttachmentLink>, DbError> {
        let mut statement = Statement::<DB>::new(
            "SELECT id, item_id, media_id FROM site_block_attachment WHERE item_id = ",
        );
        statement.bind(bind_id(item))?;
        if !media.is_empty() {
            push_in(&mut statement, " OR media_id", media)?;
        }
        statement.push(" ORDER BY id");
        let (sql, arguments) = statement.into_parts();
        let rows = sqlx::query_with::<DB, _>(&sql, arguments)
            .fetch_all(&mut *self.conn)
            .await?;
        let mine = |id: Option<u64>, ours: &dyn Fn(u64) -> bool| id.filter(|id| ours(*id));
        rows.iter()
            .map(|row| {
                Ok(AttachmentLink {
                    attachment: DB::id(row, "id")?,
                    item: mine(DB::opt_id(row, "item_id")?, &|id| id == item),
                    media: mine(DB::opt_id(row, "media_id")?, &|id| media.contains(&id)),
                })
            })
            .collect()
    }

    async fn delete_in(
        &mut self,
        table: &'static str,
        column: &'static str,
        ids: &[u64],
    ) -> Result<(), DbError> {
        for chunk in ids.chunks(CHUNK) {
            let mut statement = Statement::<DB>::new(&format!("DELETE FROM {table} WHERE 1 = 1"));
            push_in(&mut statement, &format!(" AND {column}"), chunk)?;
            let (sql, arguments) = statement.into_parts();
            sqlx::query_with::<DB, _>(&sql, arguments)
                .execute(&mut *self.conn)
                .await?;
        }
        Ok(())
    }
}

/// `{column} IN (?, ?)` after `open`; `{column} IN (NULL)` for no ids, which matches nothing.
fn push_in<DB: Backend>(
    statement: &mut Statement<'_, DB>,
    open: &str,
    ids: &[u64],
) -> Result<(), DbError>
where
    for<'x> i64: Encode<'x, DB> + Type<DB>,
{
    statement.push(open);
    statement.push(" IN (");
    if ids.is_empty() {
        statement.push("NULL");
    }
    for (at, id) in ids.iter().enumerate() {
        if at > 0 {
            statement.push(", ");
        }
        statement.bind(bind_id(*id))?;
    }
    statement.push(")");
    Ok(())
}

/// One column of a row as the snapshot stores it.
fn cell_value<DB: Backend>(
    row: &DB::Row,
    column: &'static str,
    cell: Cell,
) -> Result<Value, DbError>
where
    for<'x> Option<i64>: Decode<'x, DB> + Type<DB>,
    for<'x> bool: Decode<'x, DB> + Type<DB>,
    for<'x> Option<String>: Decode<'x, DB> + Type<DB>,
    for<'x> &'x str: ColumnIndex<DB::Row>,
{
    Ok(match cell {
        Cell::Int => DB::opt_int(row, column)?.map_or(Value::Null, Value::from),
        Cell::Text => DB::opt_text(row, column)?.map_or(Value::Null, Value::from),
        Cell::Flag => Value::Bool(DB::boolean(row, column)?),
        Cell::Time => {
            DB::opt_datetime(row, column)?.map_or(Value::Null, |at| Value::from(time::format(at)))
        }
    })
}

/// Set `column` to null on each row whose id there is not among `existing`.
pub(super) fn null_missing(rows: &mut [TrashRow], column: &str, existing: &BTreeSet<u64>) {
    for row in rows {
        if int(row, column).is_some_and(|id| !existing.contains(&id)) {
            row.insert(column.to_string(), Value::Null);
        }
    }
}

/// The ids `column` names across `rows`.
pub(super) fn named(rows: &[TrashRow], column: &str) -> BTreeSet<u64> {
    rows.iter().filter_map(|row| int(row, column)).collect()
}

/// The rows per table, as a map an undo edits before it inserts them.
pub(super) fn by_table(image: &TrashImage) -> BTreeMap<&'static str, Vec<TrashRow>> {
    image
        .tables
        .iter()
        .map(|(name, rows)| (*name, rows.clone()))
        .collect()
}
