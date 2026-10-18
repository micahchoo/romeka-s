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
use crate::rows::{NewValue, ValueRow};

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
        let query = sqlx::query(
            "INSERT INTO value
                 (resource_id, property_id, value_resource_id, value_annotation_id, type, lang,
                  value, uri, is_public)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(bind_id(new.resource_id))
        .bind(bind_id(new.property_id))
        .bind(bind_opt_id(new.value_resource_id))
        .bind(bind_opt_id(new.value_annotation_id))
        .bind(&new.r#type)
        .bind(&new.lang)
        .bind(&new.value)
        .bind(&new.uri)
        .bind(new.is_public);
        let id = insert_id(query, self.pool).await?;
        Ok(ValueRow {
            id: ValueId::from(id),
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
