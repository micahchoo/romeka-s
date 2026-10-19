//! `value` repository.
//!
//! Upstream has no `ValueAdapter`; values are read and written through their resource. The
//! ordering below matches the one upstream guarantees: `Entity/Resource.php` orders values by id
//! ascending.

use sqlx::{ColumnIndex, Decode, Encode, Executor, IntoArguments, Pool, Sqlite, Type};

use super::{Paging, Statement, bind_id, bind_opt_id, fetch_rows, insert_id, push_paging};
use crate::DbError;
use crate::backend::Backend;
use crate::ids::{PropertyId, ResourceId, ValueId};
use crate::rows::{NewValue, ValueContent, ValueRow};

/// `value` rows.
pub struct ValueRepo<'a, DB: Backend = Sqlite> {
    pool: &'a Pool<DB>,
}

impl<'a, DB: Backend> ValueRepo<'a, DB>
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

    pub async fn create(&self, new: &NewValue) -> Result<ValueRow, DbError> {
        let mut conn = self.pool.acquire().await?;
        let id = insert_value(&mut conn, new).await?;
        Ok(ValueRow {
            id,
            resource_id: new.resource_id,
            property_id: new.property_id,
            value_resource_id: new.value_resource_id,
            value_annotation_id: new.value_annotation_id,
            r#type: new.r#type.clone(),
            lang: new.lang.clone(),
            value: new.value.clone(),
            uri: new.uri.clone(),
            is_public: new.is_public,
        })
    }

    /// Delete some of a resource's values and insert new ones, all in one transaction: a failed
    /// insert keeps the values it would have replaced. An id that is not a value of this
    /// resource deletes nothing.
    pub async fn rewrite(
        &self,
        resource_id: ResourceId,
        delete: &[ValueId],
        insert: &[ValueContent],
    ) -> Result<(), DbError> {
        let mut tx = self.pool.begin().await?;
        for id in delete {
            sqlx::query("DELETE FROM value WHERE id = ? AND resource_id = ?")
                .bind(bind_id(*id))
                .bind(bind_id(resource_id))
                .execute(&mut *tx)
                .await?;
        }
        for content in insert {
            insert_value(&mut tx, &content.on(resource_id)).await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn get(&self, id: ValueId) -> Result<Option<ValueRow>, DbError> {
        let row = sqlx::query("SELECT * FROM value WHERE id = ?")
            .bind(bind_id(id))
            .fetch_optional(self.pool)
            .await?;
        row.as_ref().map(ValueRow::from_db_row::<DB>).transpose()
    }

    /// Write the whole value except its id. Values carry no timestamps.
    pub async fn update(&self, row: &ValueRow) -> Result<bool, DbError> {
        let done = sqlx::query(
            "UPDATE value
             SET resource_id = ?, property_id = ?, value_resource_id = ?, value_annotation_id = ?,
                 type = ?, lang = ?, value = ?, uri = ?, is_public = ?
             WHERE id = ?",
        )
        .bind(bind_id(row.resource_id))
        .bind(bind_id(row.property_id))
        .bind(bind_opt_id(row.value_resource_id))
        .bind(bind_opt_id(row.value_annotation_id))
        .bind(&row.r#type)
        .bind(&row.lang)
        .bind(&row.value)
        .bind(&row.uri)
        .bind(row.is_public)
        .bind(bind_id(row.id))
        .execute(self.pool)
        .await?;
        Ok(DB::rows_affected(&done) > 0)
    }

    pub async fn delete(&self, id: ValueId) -> Result<bool, DbError> {
        let done = sqlx::query("DELETE FROM value WHERE id = ?")
            .bind(bind_id(id))
            .execute(self.pool)
            .await?;
        Ok(DB::rows_affected(&done) > 0)
    }

    /// Every value on a resource, in upstream's order.
    pub async fn list_for_resource(
        &self,
        resource_id: ResourceId,
    ) -> Result<Vec<ValueRow>, DbError> {
        let rows = sqlx::query("SELECT * FROM value WHERE resource_id = ? ORDER BY id")
            .bind(bind_id(resource_id))
            .fetch_all(self.pool)
            .await?;
        rows.iter().map(ValueRow::from_db_row::<DB>).collect()
    }

    /// Every value on a resource with one property, in upstream's order.
    pub async fn list_for_property(
        &self,
        resource_id: ResourceId,
        property_id: PropertyId,
    ) -> Result<Vec<ValueRow>, DbError> {
        let rows = sqlx::query(
            "SELECT * FROM value WHERE resource_id = ? AND property_id = ? ORDER BY id",
        )
        .bind(bind_id(resource_id))
        .bind(bind_id(property_id))
        .fetch_all(self.pool)
        .await?;
        rows.iter().map(ValueRow::from_db_row::<DB>).collect()
    }

    /// Values whose `value_resource_id` points at this resource.
    pub async fn list_pointing_at(
        &self,
        resource_id: ResourceId,
        paging: &Paging,
    ) -> Result<Vec<ValueRow>, DbError> {
        let mut statement = Statement::<DB>::new("SELECT * FROM value WHERE value_resource_id = ");
        statement.bind(bind_id(resource_id))?;
        statement.push(" ORDER BY id");
        push_paging(&mut statement, paging)?;
        let (sql, arguments) = statement.into_parts();
        let rows: Vec<DB::Row> = fetch_rows(&sql, arguments, self.pool).await?;
        rows.iter().map(ValueRow::from_db_row::<DB>).collect()
    }
}

/// Insert one `value` row on this connection, so a caller can make it part of a transaction.
pub(crate) async fn insert_value<DB: Backend>(
    conn: &mut DB::Connection,
    new: &NewValue,
) -> Result<ValueId, DbError>
where
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
    for<'x> i64: Encode<'x, DB> + Type<DB>,
    for<'x> Option<i64>: Encode<'x, DB> + Type<DB>,
    for<'x> bool: Encode<'x, DB> + Type<DB>,
    for<'x> String: Encode<'x, DB> + Type<DB>,
    for<'x> Option<String>: Encode<'x, DB> + Type<DB>,
{
    // MariaDB reserves `VALUE`, so the table name after `INSERT INTO` must be quoted there.
    let table = DB::quote_identifier("value");
    let sql = format!(
        "INSERT INTO {table}
             (resource_id, property_id, value_resource_id, value_annotation_id, type, lang,
              value, uri, is_public)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"
    );
    let query = sqlx::query(&sql)
        .bind(bind_id(new.resource_id))
        .bind(bind_id(new.property_id))
        .bind(bind_opt_id(new.value_resource_id))
        .bind(bind_opt_id(new.value_annotation_id))
        .bind(&new.r#type)
        .bind(&new.lang)
        .bind(&new.value)
        .bind(&new.uri)
        .bind(new.is_public);
    Ok(ValueId::from(insert_id(query, conn).await?))
}
