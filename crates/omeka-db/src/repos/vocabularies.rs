//! `vocabulary`, `resource_class` and `property` repositories.
//!
//! Filters mirror upstream `VocabularyAdapter`, `ResourceClassAdapter` and `PropertyAdapter`
//! `buildQuery`: `owner_id`, `vocabulary_id` and `local_name`. The adapters' extra joins by
//! vocabulary namespace URI, prefix, term and site are not here; they are follow-ups listed in the
//! unit report.

use sqlx::{ColumnIndex, Decode, Encode, Executor, IntoArguments, Pool, Sqlite, Type};

use super::{Paging, Statement, bind_id, bind_opt_id, fetch_rows, insert_id, push_paging};
use crate::DbError;
use crate::backend::Backend;
use crate::ids::{ClassId, PropertyId, VocabularyId};
use crate::rows::{ClassRow, NewClass, NewProperty, NewVocabulary, PropertyRow, VocabularyRow};

/// A list filter for `vocabulary`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct VocabularyFilter {
    pub owner_id: Option<crate::ids::UserId>,
    pub namespace_uri: Option<String>,
    pub prefix: Option<String>,
}

/// `vocabulary` rows.
pub struct VocabularyRepo<'a, DB: Backend = Sqlite> {
    pool: &'a Pool<DB>,
}

impl<'a, DB: Backend> VocabularyRepo<'a, DB>
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

    pub async fn create(&self, new: &NewVocabulary) -> Result<VocabularyRow, DbError> {
        let query = sqlx::query(
            "INSERT INTO vocabulary (owner_id, namespace_uri, prefix, label, comment)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(bind_opt_id(new.owner_id))
        .bind(&new.namespace_uri)
        .bind(&new.prefix)
        .bind(&new.label)
        .bind(&new.comment);
        let id = insert_id(query, self.pool).await?;
        Ok(VocabularyRow {
            id: VocabularyId::from(id),
            owner_id: new.owner_id,
            namespace_uri: new.namespace_uri.clone(),
            prefix: new.prefix.clone(),
            label: new.label.clone(),
            comment: new.comment.clone(),
        })
    }

    pub async fn get(&self, id: VocabularyId) -> Result<Option<VocabularyRow>, DbError> {
        let row = sqlx::query("SELECT * FROM vocabulary WHERE id = ?")
            .bind(bind_id(id))
            .fetch_optional(self.pool)
            .await?;
        row.as_ref()
            .map(VocabularyRow::from_db_row::<DB>)
            .transpose()
    }

    /// The vocabulary with this prefix, compared the way upstream's `prefix` unique key compares.
    pub async fn get_by_prefix(&self, prefix: &str) -> Result<Option<VocabularyRow>, DbError> {
        let row = sqlx::query("SELECT * FROM vocabulary WHERE prefix = ?")
            .bind(prefix)
            .fetch_optional(self.pool)
            .await?;
        row.as_ref()
            .map(VocabularyRow::from_db_row::<DB>)
            .transpose()
    }

    /// Write every column of `row`, including `owner_id` and the namespace. Returns false when the
    /// row no longer exists.
    pub async fn update(&self, row: &VocabularyRow) -> Result<bool, DbError> {
        let done = sqlx::query(
            "UPDATE vocabulary
             SET owner_id = ?, namespace_uri = ?, prefix = ?, label = ?, comment = ?
             WHERE id = ?",
        )
        .bind(bind_opt_id(row.owner_id))
        .bind(&row.namespace_uri)
        .bind(&row.prefix)
        .bind(&row.label)
        .bind(&row.comment)
        .bind(bind_id(row.id))
        .execute(self.pool)
        .await?;
        Ok(DB::rows_affected(&done) > 0)
    }

    /// Delete a vocabulary, its properties and its classes.
    ///
    /// Upstream cascades the same way in Doctrine (`Entity/Vocabulary.php` has
    /// `cascade={"persist", "remove"}` on both collections), and the property delete cascades
    /// values and template property rows at the database level.
    pub async fn delete(&self, id: VocabularyId) -> Result<bool, DbError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM property WHERE vocabulary_id = ?")
            .bind(bind_id(id))
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM resource_class WHERE vocabulary_id = ?")
            .bind(bind_id(id))
            .execute(&mut *tx)
            .await?;
        let done = sqlx::query("DELETE FROM vocabulary WHERE id = ?")
            .bind(bind_id(id))
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(DB::rows_affected(&done) > 0)
    }

    pub async fn list(
        &self,
        filter: &VocabularyFilter,
        paging: &Paging,
    ) -> Result<Vec<VocabularyRow>, DbError> {
        let mut statement = Statement::<DB>::new("SELECT * FROM vocabulary WHERE 1 = 1");
        if let Some(owner_id) = filter.owner_id {
            statement.push(" AND owner_id = ");
            statement.bind(bind_id(owner_id))?;
        }
        if let Some(namespace_uri) = &filter.namespace_uri {
            statement.push(" AND namespace_uri = ");
            statement.bind(namespace_uri)?;
        }
        if let Some(prefix) = &filter.prefix {
            statement.push(" AND prefix = ");
            statement.bind(prefix)?;
        }
        statement.push(" ORDER BY id");
        push_paging(&mut statement, paging)?;
        let (sql, arguments) = statement.into_parts();
        let rows: Vec<DB::Row> = fetch_rows(&sql, arguments, self.pool).await?;
        rows.iter().map(VocabularyRow::from_db_row::<DB>).collect()
    }
}

/// A list filter for `resource_class`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ClassFilter {
    pub owner_id: Option<crate::ids::UserId>,
    pub vocabulary_id: Option<VocabularyId>,
    pub local_name: Option<String>,
}

/// `resource_class` rows.
pub struct ClassRepo<'a, DB: Backend = Sqlite> {
    pool: &'a Pool<DB>,
}

impl<'a, DB: Backend> ClassRepo<'a, DB>
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

    pub async fn create(&self, new: &NewClass) -> Result<ClassRow, DbError> {
        let query = sqlx::query(
            "INSERT INTO resource_class (owner_id, vocabulary_id, local_name, label, comment)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(bind_opt_id(new.owner_id))
        .bind(bind_id(new.vocabulary_id))
        .bind(&new.local_name)
        .bind(&new.label)
        .bind(&new.comment);
        let id = insert_id(query, self.pool).await?;
        Ok(ClassRow {
            id: ClassId::from(id),
            owner_id: new.owner_id,
            vocabulary_id: new.vocabulary_id,
            local_name: new.local_name.clone(),
            label: new.label.clone(),
            comment: new.comment.clone(),
        })
    }

    pub async fn get(&self, id: ClassId) -> Result<Option<ClassRow>, DbError> {
        let row = sqlx::query("SELECT * FROM resource_class WHERE id = ?")
            .bind(bind_id(id))
            .fetch_optional(self.pool)
            .await?;
        row.as_ref().map(ClassRow::from_db_row::<DB>).transpose()
    }

    /// The class with this local name in this vocabulary, the pair upstream's unique key covers.
    pub async fn find(
        &self,
        vocabulary_id: VocabularyId,
        local_name: &str,
    ) -> Result<Option<ClassRow>, DbError> {
        let row =
            sqlx::query("SELECT * FROM resource_class WHERE vocabulary_id = ? AND local_name = ?")
                .bind(bind_id(vocabulary_id))
                .bind(local_name)
                .fetch_optional(self.pool)
                .await?;
        row.as_ref().map(ClassRow::from_db_row::<DB>).transpose()
    }

    pub async fn update(&self, row: &ClassRow) -> Result<bool, DbError> {
        let done = sqlx::query(
            "UPDATE resource_class
             SET owner_id = ?, vocabulary_id = ?, local_name = ?, label = ?, comment = ?
             WHERE id = ?",
        )
        .bind(bind_opt_id(row.owner_id))
        .bind(bind_id(row.vocabulary_id))
        .bind(&row.local_name)
        .bind(&row.label)
        .bind(&row.comment)
        .bind(bind_id(row.id))
        .execute(self.pool)
        .await?;
        Ok(DB::rows_affected(&done) > 0)
    }

    /// Resources that use this class keep their class cleared (`resource_class_id` is
    /// `ON DELETE SET NULL` upstream).
    pub async fn delete(&self, id: ClassId) -> Result<bool, DbError> {
        let done = sqlx::query("DELETE FROM resource_class WHERE id = ?")
            .bind(bind_id(id))
            .execute(self.pool)
            .await?;
        Ok(DB::rows_affected(&done) > 0)
    }

    pub async fn list(
        &self,
        filter: &ClassFilter,
        paging: &Paging,
    ) -> Result<Vec<ClassRow>, DbError> {
        let mut statement = Statement::<DB>::new("SELECT * FROM resource_class WHERE 1 = 1");
        if let Some(owner_id) = filter.owner_id {
            statement.push(" AND owner_id = ");
            statement.bind(bind_id(owner_id))?;
        }
        if let Some(vocabulary_id) = filter.vocabulary_id {
            statement.push(" AND vocabulary_id = ");
            statement.bind(bind_id(vocabulary_id))?;
        }
        if let Some(local_name) = &filter.local_name {
            statement.push(" AND local_name = ");
            statement.bind(local_name)?;
        }
        statement.push(" ORDER BY id");
        push_paging(&mut statement, paging)?;
        let (sql, arguments) = statement.into_parts();
        let rows: Vec<DB::Row> = fetch_rows(&sql, arguments, self.pool).await?;
        rows.iter().map(ClassRow::from_db_row::<DB>).collect()
    }
}

/// A list filter for `property`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PropertyFilter {
    pub owner_id: Option<crate::ids::UserId>,
    pub vocabulary_id: Option<VocabularyId>,
    pub local_name: Option<String>,
}

/// `property` rows.
pub struct PropertyRepo<'a, DB: Backend = Sqlite> {
    pool: &'a Pool<DB>,
}

impl<'a, DB: Backend> PropertyRepo<'a, DB>
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

    pub async fn create(&self, new: &NewProperty) -> Result<PropertyRow, DbError> {
        let mut conn = self.pool.acquire().await?;
        Self::insert(&mut conn, new).await
    }

    /// Insert a property on the caller's connection, so it can join a larger transaction.
    pub(crate) async fn insert(
        conn: &mut DB::Connection,
        new: &NewProperty,
    ) -> Result<PropertyRow, DbError> {
        let query = sqlx::query(
            "INSERT INTO property (owner_id, vocabulary_id, local_name, label, comment)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(bind_opt_id(new.owner_id))
        .bind(bind_id(new.vocabulary_id))
        .bind(&new.local_name)
        .bind(&new.label)
        .bind(&new.comment);
        let id = insert_id(query, &mut *conn).await?;
        Ok(PropertyRow {
            id: PropertyId::from(id),
            owner_id: new.owner_id,
            vocabulary_id: new.vocabulary_id,
            local_name: new.local_name.clone(),
            label: new.label.clone(),
            comment: new.comment.clone(),
        })
    }

    pub async fn get(&self, id: PropertyId) -> Result<Option<PropertyRow>, DbError> {
        let row = sqlx::query("SELECT * FROM property WHERE id = ?")
            .bind(bind_id(id))
            .fetch_optional(self.pool)
            .await?;
        row.as_ref().map(PropertyRow::from_db_row::<DB>).transpose()
    }

    /// The property with this local name in this vocabulary, the pair imports look up by.
    pub async fn find(
        &self,
        vocabulary_id: VocabularyId,
        local_name: &str,
    ) -> Result<Option<PropertyRow>, DbError> {
        let row = sqlx::query("SELECT * FROM property WHERE vocabulary_id = ? AND local_name = ?")
            .bind(bind_id(vocabulary_id))
            .bind(local_name)
            .fetch_optional(self.pool)
            .await?;
        row.as_ref().map(PropertyRow::from_db_row::<DB>).transpose()
    }

    pub async fn update(&self, row: &PropertyRow) -> Result<bool, DbError> {
        let done = sqlx::query(
            "UPDATE property
             SET owner_id = ?, vocabulary_id = ?, local_name = ?, label = ?, comment = ?
             WHERE id = ?",
        )
        .bind(bind_opt_id(row.owner_id))
        .bind(bind_id(row.vocabulary_id))
        .bind(&row.local_name)
        .bind(&row.label)
        .bind(&row.comment)
        .bind(bind_id(row.id))
        .execute(self.pool)
        .await?;
        Ok(DB::rows_affected(&done) > 0)
    }

    /// Deleting a property deletes the values that use it and the template rows that list it, as
    /// the upstream foreign keys do (`value.property_id` and `resource_template_property.property_id`
    /// are `ON DELETE CASCADE`).
    pub async fn delete(&self, id: PropertyId) -> Result<bool, DbError> {
        let done = sqlx::query("DELETE FROM property WHERE id = ?")
            .bind(bind_id(id))
            .execute(self.pool)
            .await?;
        Ok(DB::rows_affected(&done) > 0)
    }

    pub async fn list(
        &self,
        filter: &PropertyFilter,
        paging: &Paging,
    ) -> Result<Vec<PropertyRow>, DbError> {
        let mut statement = Statement::<DB>::new("SELECT * FROM property WHERE 1 = 1");
        if let Some(owner_id) = filter.owner_id {
            statement.push(" AND owner_id = ");
            statement.bind(bind_id(owner_id))?;
        }
        if let Some(vocabulary_id) = filter.vocabulary_id {
            statement.push(" AND vocabulary_id = ");
            statement.bind(bind_id(vocabulary_id))?;
        }
        if let Some(local_name) = &filter.local_name {
            statement.push(" AND local_name = ");
            statement.bind(local_name)?;
        }
        statement.push(" ORDER BY id");
        push_paging(&mut statement, paging)?;
        let (sql, arguments) = statement.into_parts();
        let rows: Vec<DB::Row> = fetch_rows(&sql, arguments, self.pool).await?;
        rows.iter().map(PropertyRow::from_db_row::<DB>).collect()
    }
}
