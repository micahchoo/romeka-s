//! `resource`, `item` and `item_set` repositories.
//!
//! An item and an item set are `resource` rows plus their own columns, joined on the shared id.
//! Creating one writes both rows in a transaction, so a crash never leaves a bare resource.
//!
//! Delete semantics follow Doctrine's cascade configuration, since upstream relies on it:
//! `Entity/Resource.php` cascades remove to `values` (MySQL has no `ON DELETE` action on
//! `value.resource_id`), while `item`, `item_set`, `media`, `item_item_set`, `item_site` and
//! `value.value_resource_id` are deleted by the database's own cascade.

use sqlx::{ColumnIndex, Decode, Encode, Executor, IntoArguments, Pool, Sqlite, Type};

use super::values::insert_value;
use super::{
    Paging, QueryFilter, Statement, bind_id, bind_opt_id, fetch_rows, insert_id, new_id,
    push_paging,
};
use crate::DbError;
use crate::backend::Backend;
use crate::ids::{ClassId, ItemId, ItemSetId, ResourceId, SiteId, TemplateId, UserId};
use crate::rows::{
    ItemRow, ItemSetRow, NewItem, NewItemSet, NewResource, NewWholeItem, ResourceRow,
    ValueAnnotationRow, resource_type,
};
use crate::time;

/// A list filter for `item`, matching `ItemAdapter::buildQuery` and its parent
/// `AbstractResourceEntityAdapter::buildQuery` (`owner_id`, `resource_class_id`,
/// `resource_template_id`, `item_set_id`, `is_public`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ItemFilter {
    pub owner_id: Option<UserId>,
    pub resource_class_id: Option<ClassId>,
    pub resource_template_id: Option<TemplateId>,
    /// Items attached to this item set.
    pub item_set_id: Option<ItemSetId>,
    pub is_public: Option<bool>,
}

/// A list filter for `item_set`, matching `ItemSetAdapter::buildQuery` plus its parent's fields.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ItemSetFilter {
    pub owner_id: Option<UserId>,
    pub resource_class_id: Option<ClassId>,
    pub resource_template_id: Option<TemplateId>,
    pub is_public: Option<bool>,
    /// `is_open` as a plain filter. Upstream also narrows this filter by ACL; the API layer owns
    /// that narrowing.
    pub is_open: Option<bool>,
}

/// `resource` rows: read, update and delete by the shared id.
pub struct ResourceRepo<'a, DB: Backend = Sqlite> {
    pool: &'a Pool<DB>,
}

impl<'a, DB: Backend> ResourceRepo<'a, DB>
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
    pub fn new(pool: &'a Pool<DB>) -> Self {
        Self { pool }
    }

    pub async fn get(&self, id: ResourceId) -> Result<Option<ResourceRow>, DbError> {
        let row = sqlx::query("SELECT * FROM resource WHERE id = ?")
            .bind(bind_id(id))
            .fetch_optional(self.pool)
            .await?;
        row.as_ref().map(ResourceRow::from_db_row::<DB>).transpose()
    }

    /// Write the shared resource columns. `created` and `resource_type` never change; `modified`
    /// becomes now (`AbstractEntityAdapter::updateTimestamps`).
    pub async fn update(&self, row: &ResourceRow) -> Result<bool, DbError> {
        let mut conn = self.pool.acquire().await?;
        update_resource_columns(&mut conn, row).await
    }

    /// Delete a resource and the values it carries, whatever its kind.
    ///
    /// This is the one delete path for items, item sets, media and value annotations: the kind rows
    /// go with the database cascade.
    pub async fn delete(&self, id: ResourceId) -> Result<bool, DbError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM value WHERE resource_id = ?")
            .bind(bind_id(id))
            .execute(&mut *tx)
            .await?;
        let done = sqlx::query("DELETE FROM resource WHERE id = ?")
            .bind(bind_id(id))
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(DB::rows_affected(&done) > 0)
    }

    /// Create a value annotation: a resource row of the annotation kind and its kind row.
    pub async fn create_annotation(
        &self,
        new: &NewResource,
    ) -> Result<ValueAnnotationRow, DbError> {
        let mut tx = self.pool.begin().await?;
        let resource = insert_resource(&mut tx, new, resource_type::VALUE_ANNOTATION).await?;
        sqlx::query("INSERT INTO value_annotation (id) VALUES (?)")
            .bind(bind_id(resource.id))
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(ValueAnnotationRow { resource })
    }
}

/// Insert the `resource` half of any resource kind. Returns the row as stored.
pub(crate) async fn insert_resource<DB: Backend>(
    conn: &mut DB::Connection,
    new: &NewResource,
    kind: &str,
) -> Result<ResourceRow, DbError>
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
    let created = new.created.unwrap_or_else(time::now);
    let query = sqlx::query(
        "INSERT INTO resource
             (owner_id, resource_class_id, resource_template_id, thumbnail_id, title, is_public,
              created, modified, resource_type)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(bind_opt_id(new.owner_id))
    .bind(bind_opt_id(new.resource_class_id))
    .bind(bind_opt_id(new.resource_template_id))
    .bind(bind_opt_id(new.thumbnail_id))
    .bind(&new.title)
    .bind(new.is_public)
    .bind(time::format(created))
    .bind(time::format(created))
    .bind(kind);
    let id = insert_id(query, conn).await?;
    Ok(ResourceRow {
        id: ResourceId::from(id),
        owner_id: new.owner_id,
        resource_class_id: new.resource_class_id,
        resource_template_id: new.resource_template_id,
        thumbnail_id: new.thumbnail_id,
        title: new.title.clone(),
        is_public: new.is_public,
        created,
        modified: Some(created),
        resource_type: kind.to_string(),
    })
}

/// Write the shared resource columns, leaving `created` and `resource_type` alone.
pub(crate) async fn update_resource_columns<DB: Backend>(
    conn: &mut DB::Connection,
    row: &ResourceRow,
) -> Result<bool, DbError>
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
    let done = sqlx::query(
        "UPDATE resource
         SET owner_id = ?, resource_class_id = ?, resource_template_id = ?, thumbnail_id = ?,
             title = ?, is_public = ?, modified = ?
         WHERE id = ?",
    )
    .bind(bind_opt_id(row.owner_id))
    .bind(bind_opt_id(row.resource_class_id))
    .bind(bind_opt_id(row.resource_template_id))
    .bind(bind_opt_id(row.thumbnail_id))
    .bind(&row.title)
    .bind(row.is_public)
    .bind(time::format(time::now()))
    .bind(bind_id(row.id))
    .execute(conn)
    .await?;
    Ok(DB::rows_affected(&done) > 0)
}

const ITEM_COLUMNS: &str = "SELECT resource.*, item.primary_media_id FROM item
     JOIN resource ON resource.id = item.id";

/// `item` rows.
pub struct ItemRepo<'a, DB: Backend = Sqlite> {
    pool: &'a Pool<DB>,
}

impl<'a, DB: Backend> ItemRepo<'a, DB>
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
    pub fn new(pool: &'a Pool<DB>) -> Self {
        Self { pool }
    }

    pub async fn create(&self, new: &NewItem) -> Result<ItemRow, DbError> {
        let mut tx = self.pool.begin().await?;
        let row = Self::insert_item(&mut tx, new).await?;
        tx.commit().await?;
        Ok(row)
    }

    /// Create an item with its item-set links and its values in one transaction, so a failed
    /// value leaves no item, no link and no value behind.
    pub async fn create_whole(&self, new: &NewWholeItem) -> Result<ItemRow, DbError> {
        let mut tx = self.pool.begin().await?;
        let row = Self::insert_whole(&mut tx, new).await?;
        tx.commit().await?;
        Ok(row)
    }

    /// [`Self::create_whole`] on the caller's connection, so it can join a larger transaction.
    pub(crate) async fn insert_whole(
        conn: &mut DB::Connection,
        new: &NewWholeItem,
    ) -> Result<ItemRow, DbError> {
        let row = Self::insert_item(conn, &new.item).await?;
        for item_set_id in &new.item_sets {
            sqlx::query(DB::insert_item_item_set())
                .bind(bind_id(row.id()))
                .bind(bind_id(*item_set_id))
                .execute(&mut *conn)
                .await?;
        }
        for content in &new.values {
            insert_value(conn, &content.on(row.resource.id)).await?;
        }
        Ok(row)
    }

    async fn insert_item(conn: &mut DB::Connection, new: &NewItem) -> Result<ItemRow, DbError> {
        let resource = insert_resource(conn, &new.resource, resource_type::ITEM).await?;
        sqlx::query("INSERT INTO item (id, primary_media_id) VALUES (?, ?)")
            .bind(bind_id(resource.id))
            .bind(bind_opt_id(new.primary_media_id))
            .execute(&mut *conn)
            .await?;
        Ok(ItemRow {
            resource,
            primary_media_id: new.primary_media_id,
        })
    }

    pub async fn get(&self, id: ItemId) -> Result<Option<ItemRow>, DbError> {
        let sql = format!("{ITEM_COLUMNS} WHERE item.id = ?");
        let row = sqlx::query(&sql)
            .bind(bind_id(id))
            .fetch_optional(self.pool)
            .await?;
        row.as_ref().map(ItemRow::from_db_row::<DB>).transpose()
    }

    pub async fn update(&self, row: &ItemRow) -> Result<bool, DbError> {
        let mut tx = self.pool.begin().await?;
        if !update_resource_columns(&mut tx, &row.resource).await? {
            return Ok(false);
        }
        sqlx::query("UPDATE item SET primary_media_id = ? WHERE id = ?")
            .bind(bind_opt_id(row.primary_media_id))
            .bind(bind_id(row.resource.id))
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(true)
    }

    /// Delete an item, its values and every row that cascades from it.
    pub async fn delete(&self, id: ItemId) -> Result<bool, DbError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM value WHERE resource_id = ?")
            .bind(bind_id(id))
            .execute(&mut *tx)
            .await?;
        let done = sqlx::query("DELETE FROM resource WHERE id = ?")
            .bind(bind_id(id))
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(DB::rows_affected(&done) > 0)
    }

    pub async fn list(
        &self,
        filter: &ItemFilter,
        paging: &Paging,
    ) -> Result<Vec<ItemRow>, DbError> {
        self.list_filtered(filter, &QueryFilter::default(), paging)
            .await
    }

    /// `list` with the advanced search parameters of `ItemAdapter::buildQuery`
    /// on top ([`QueryFilter`]).
    pub async fn list_filtered(
        &self,
        filter: &ItemFilter,
        query: &QueryFilter,
        paging: &Paging,
    ) -> Result<Vec<ItemRow>, DbError> {
        let mut statement = Statement::<DB>::new(ITEM_COLUMNS);
        statement.push(" WHERE 1 = 1");
        if let Some(owner_id) = filter.owner_id {
            statement.push(" AND resource.owner_id = ");
            statement.bind(bind_id(owner_id))?;
        }
        if let Some(class_id) = filter.resource_class_id {
            statement.push(" AND resource.resource_class_id = ");
            statement.bind(bind_id(class_id))?;
        }
        if let Some(template_id) = filter.resource_template_id {
            statement.push(" AND resource.resource_template_id = ");
            statement.bind(bind_id(template_id))?;
        }
        if let Some(item_set_id) = filter.item_set_id {
            statement.push(
                " AND EXISTS (SELECT 1 FROM item_item_set
                       WHERE item_item_set.item_id = item.id
                         AND item_item_set.item_set_id = ",
            );
            statement.bind(bind_id(item_set_id))?;
            statement.push(")");
        }
        if let Some(is_public) = filter.is_public {
            statement.push(" AND resource.is_public = ");
            statement.bind(is_public)?;
        }
        query.push_item(&mut statement)?;
        statement.push(" ORDER BY resource.id");
        push_paging(&mut statement, paging)?;
        let (sql, arguments) = statement.into_parts();
        let rows: Vec<DB::Row> = fetch_rows(&sql, arguments, self.pool).await?;
        rows.iter().map(ItemRow::from_db_row::<DB>).collect()
    }

    /// Attach an item to an item set. Attaching twice is a no-op, as adding to a Doctrine
    /// collection is.
    pub async fn add_to_item_set(
        &self,
        item_id: ItemId,
        item_set_id: ItemSetId,
    ) -> Result<(), DbError> {
        sqlx::query(DB::insert_item_item_set())
            .bind(bind_id(item_id))
            .bind(bind_id(item_set_id))
            .execute(self.pool)
            .await?;
        Ok(())
    }

    pub async fn remove_from_item_set(
        &self,
        item_id: ItemId,
        item_set_id: ItemSetId,
    ) -> Result<bool, DbError> {
        let done = sqlx::query("DELETE FROM item_item_set WHERE item_id = ? AND item_set_id = ?")
            .bind(bind_id(item_id))
            .bind(bind_id(item_set_id))
            .execute(self.pool)
            .await?;
        Ok(DB::rows_affected(&done) > 0)
    }

    pub async fn list_item_sets(&self, item_id: ItemId) -> Result<Vec<ItemSetId>, DbError> {
        let ids: Vec<i64> = sqlx::query_scalar::<DB, i64>(
            "SELECT item_set_id FROM item_item_set WHERE item_id = ? ORDER BY item_set_id",
        )
        .bind(bind_id(item_id))
        .fetch_all(self.pool)
        .await?;
        ids.into_iter()
            .map(|raw| Ok(ItemSetId::from(new_id(raw)?)))
            .collect()
    }

    /// Attach an item to a site (`Item::$sites`, `Entity/Item.php:48`).
    /// Attaching twice is a no-op.
    pub async fn add_to_site(&self, item_id: ItemId, site_id: SiteId) -> Result<(), DbError> {
        let sql = format!("{}(?, ?)", DB::insert_item_site());
        sqlx::query(&sql)
            .bind(bind_id(item_id))
            .bind(bind_id(site_id))
            .execute(self.pool)
            .await?;
        Ok(())
    }

    pub async fn remove_from_site(
        &self,
        item_id: ItemId,
        site_id: SiteId,
    ) -> Result<bool, DbError> {
        let done = sqlx::query("DELETE FROM item_site WHERE item_id = ? AND site_id = ?")
            .bind(bind_id(item_id))
            .bind(bind_id(site_id))
            .execute(self.pool)
            .await?;
        Ok(DB::rows_affected(&done) > 0)
    }

    pub async fn list_sites(&self, item_id: ItemId) -> Result<Vec<SiteId>, DbError> {
        let ids: Vec<i64> = sqlx::query_scalar::<DB, i64>(
            "SELECT site_id FROM item_site WHERE item_id = ? ORDER BY site_id",
        )
        .bind(bind_id(item_id))
        .fetch_all(self.pool)
        .await?;
        ids.into_iter()
            .map(|raw| Ok(SiteId::from(new_id(raw)?)))
            .collect()
    }

    /// Whether the item is attached to the site (`item_site`). Upstream's
    /// `noindexItem` reads `$resource->sites()` for the same question
    /// (`Module.php:925-931`).
    pub async fn in_site(&self, item_id: ItemId, site_id: SiteId) -> Result<bool, DbError> {
        let count: i64 = sqlx::query_scalar::<DB, i64>(
            "SELECT COUNT(*) FROM item_site WHERE item_id = ? AND site_id = ?",
        )
        .bind(bind_id(item_id))
        .bind(bind_id(site_id))
        .fetch_one(self.pool)
        .await?;
        Ok(count > 0)
    }
}

const ITEM_SET_COLUMNS: &str = "SELECT resource.*, item_set.is_open FROM item_set
     JOIN resource ON resource.id = item_set.id";

/// `item_set` rows.
pub struct ItemSetRepo<'a, DB: Backend = Sqlite> {
    pool: &'a Pool<DB>,
}

impl<'a, DB: Backend> ItemSetRepo<'a, DB>
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
    pub fn new(pool: &'a Pool<DB>) -> Self {
        Self { pool }
    }

    pub async fn create(&self, new: &NewItemSet) -> Result<ItemSetRow, DbError> {
        let mut tx = self.pool.begin().await?;
        let row = Self::insert(&mut tx, new).await?;
        tx.commit().await?;
        Ok(row)
    }

    /// Insert an item set on the caller's connection, so it can join a larger transaction.
    pub(crate) async fn insert(
        conn: &mut DB::Connection,
        new: &NewItemSet,
    ) -> Result<ItemSetRow, DbError> {
        let resource = insert_resource(conn, &new.resource, resource_type::ITEM_SET).await?;
        sqlx::query("INSERT INTO item_set (id, is_open) VALUES (?, ?)")
            .bind(bind_id(resource.id))
            .bind(new.is_open)
            .execute(&mut *conn)
            .await?;
        Ok(ItemSetRow {
            resource,
            is_open: new.is_open,
        })
    }

    pub async fn get(&self, id: ItemSetId) -> Result<Option<ItemSetRow>, DbError> {
        let sql = format!("{ITEM_SET_COLUMNS} WHERE item_set.id = ?");
        let row = sqlx::query(&sql)
            .bind(bind_id(id))
            .fetch_optional(self.pool)
            .await?;
        row.as_ref().map(ItemSetRow::from_db_row::<DB>).transpose()
    }

    pub async fn update(&self, row: &ItemSetRow) -> Result<bool, DbError> {
        let mut tx = self.pool.begin().await?;
        if !update_resource_columns(&mut tx, &row.resource).await? {
            return Ok(false);
        }
        sqlx::query("UPDATE item_set SET is_open = ? WHERE id = ?")
            .bind(row.is_open)
            .bind(bind_id(row.resource.id))
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(true)
    }

    /// Delete an item set: the membership rows go with the database cascade.
    pub async fn delete(&self, id: ItemSetId) -> Result<bool, DbError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM value WHERE resource_id = ?")
            .bind(bind_id(id))
            .execute(&mut *tx)
            .await?;
        let done = sqlx::query("DELETE FROM resource WHERE id = ?")
            .bind(bind_id(id))
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(DB::rows_affected(&done) > 0)
    }

    pub async fn list(
        &self,
        filter: &ItemSetFilter,
        paging: &Paging,
    ) -> Result<Vec<ItemSetRow>, DbError> {
        self.list_filtered(filter, &QueryFilter::default(), paging)
            .await
    }

    /// `list` with `ItemSetAdapter::buildQuery`'s `site_id` join on top.
    pub async fn list_filtered(
        &self,
        filter: &ItemSetFilter,
        query: &QueryFilter,
        paging: &Paging,
    ) -> Result<Vec<ItemSetRow>, DbError> {
        let mut statement = Statement::<DB>::new(ITEM_SET_COLUMNS);
        statement.push(" WHERE 1 = 1");
        if let Some(owner_id) = filter.owner_id {
            statement.push(" AND resource.owner_id = ");
            statement.bind(bind_id(owner_id))?;
        }
        if let Some(class_id) = filter.resource_class_id {
            statement.push(" AND resource.resource_class_id = ");
            statement.bind(bind_id(class_id))?;
        }
        if let Some(template_id) = filter.resource_template_id {
            statement.push(" AND resource.resource_template_id = ");
            statement.bind(bind_id(template_id))?;
        }
        if let Some(is_public) = filter.is_public {
            statement.push(" AND resource.is_public = ");
            statement.bind(is_public)?;
        }
        if let Some(is_open) = filter.is_open {
            statement.push(" AND item_set.is_open = ");
            statement.bind(is_open)?;
        }
        query.push_item_set(&mut statement)?;
        statement.push(" ORDER BY resource.id");
        push_paging(&mut statement, paging)?;
        let (sql, arguments) = statement.into_parts();
        let rows: Vec<DB::Row> = fetch_rows(&sql, arguments, self.pool).await?;
        rows.iter().map(ItemSetRow::from_db_row::<DB>).collect()
    }
}
