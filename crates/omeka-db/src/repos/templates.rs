//! `resource_template` and `resource_template_property` repositories.
//!
//! A template lists properties with a position and a data type list. The list is `json_array` in
//! the schema, so it round-trips as JSON text. Deleting a template deletes its property rows first:
//! `resource_template_property.resource_template_id` has no database delete action, while resources
//! that used the template keep their rows and lose the reference (`resource.resource_template_id`
//! is `ON DELETE SET NULL`).

use sqlx::{ColumnIndex, Decode, Encode, Executor, IntoArguments, Pool, Sqlite, Type};

use super::{bind_id, bind_opt_id, insert_id};
use crate::DbError;
use crate::backend::Backend;
use crate::ids::{TemplateId, TemplatePropertyId};
use crate::rows::{NewTemplate, NewTemplateProperty, TemplatePropertyRow, TemplateRow};

/// `resource_template` rows.
pub struct TemplateRepo<'a, DB: Backend = Sqlite> {
    pool: &'a Pool<DB>,
}

impl<'a, DB: Backend> TemplateRepo<'a, DB>
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

    pub async fn create(&self, new: &NewTemplate) -> Result<TemplateRow, DbError> {
        let mut conn = self.pool.acquire().await?;
        Self::insert(&mut conn, new).await
    }

    /// Insert a template on the caller's connection, so it can join a larger transaction.
    pub(crate) async fn insert(
        conn: &mut DB::Connection,
        new: &NewTemplate,
    ) -> Result<TemplateRow, DbError> {
        let query = sqlx::query(
            "INSERT INTO resource_template
                 (owner_id, resource_class_id, title_property_id, description_property_id, label)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(bind_opt_id(new.owner_id))
        .bind(bind_opt_id(new.resource_class_id))
        .bind(bind_opt_id(new.title_property_id))
        .bind(bind_opt_id(new.description_property_id))
        .bind(&new.label);
        let id = insert_id(query, &mut *conn).await?;
        Ok(TemplateRow {
            id: TemplateId::from(id),
            owner_id: new.owner_id,
            resource_class_id: new.resource_class_id,
            title_property_id: new.title_property_id,
            description_property_id: new.description_property_id,
            label: new.label.clone(),
        })
    }

    pub async fn get(&self, id: TemplateId) -> Result<Option<TemplateRow>, DbError> {
        let row = sqlx::query("SELECT * FROM resource_template WHERE id = ?")
            .bind(bind_id(id))
            .fetch_optional(self.pool)
            .await?;
        row.as_ref().map(TemplateRow::from_db_row::<DB>).transpose()
    }

    /// The template with this label. The comparison follows the column's collation, as upstream's
    /// unique key does.
    pub async fn find_by_label(&self, label: &str) -> Result<Option<TemplateRow>, DbError> {
        let row = sqlx::query("SELECT * FROM resource_template WHERE label = ?")
            .bind(label)
            .fetch_optional(self.pool)
            .await?;
        row.as_ref().map(TemplateRow::from_db_row::<DB>).transpose()
    }

    pub async fn update(&self, row: &TemplateRow) -> Result<bool, DbError> {
        let done = sqlx::query(
            "UPDATE resource_template
             SET owner_id = ?, resource_class_id = ?, title_property_id = ?,
                 description_property_id = ?, label = ?
             WHERE id = ?",
        )
        .bind(bind_opt_id(row.owner_id))
        .bind(bind_opt_id(row.resource_class_id))
        .bind(bind_opt_id(row.title_property_id))
        .bind(bind_opt_id(row.description_property_id))
        .bind(&row.label)
        .bind(bind_id(row.id))
        .execute(self.pool)
        .await?;
        Ok(DB::rows_affected(&done) > 0)
    }

    pub async fn delete(&self, id: TemplateId) -> Result<bool, DbError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM resource_template_property WHERE resource_template_id = ?")
            .bind(bind_id(id))
            .execute(&mut *tx)
            .await?;
        let done = sqlx::query("DELETE FROM resource_template WHERE id = ?")
            .bind(bind_id(id))
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(DB::rows_affected(&done) > 0)
    }

    pub async fn list(&self) -> Result<Vec<TemplateRow>, DbError> {
        let rows = sqlx::query("SELECT * FROM resource_template ORDER BY id")
            .fetch_all(self.pool)
            .await?;
        rows.iter().map(TemplateRow::from_db_row::<DB>).collect()
    }

    /// Add a property to a template. The pair (template, property) is unique.
    pub async fn add_property(
        &self,
        new: &NewTemplateProperty,
    ) -> Result<TemplatePropertyRow, DbError> {
        let mut conn = self.pool.acquire().await?;
        Self::insert_property(&mut conn, new).await
    }

    /// [`Self::add_property`] on the caller's connection, so it can join a larger transaction.
    pub(crate) async fn insert_property(
        conn: &mut DB::Connection,
        new: &NewTemplateProperty,
    ) -> Result<TemplatePropertyRow, DbError> {
        let query = sqlx::query(
            "INSERT INTO resource_template_property
                 (resource_template_id, property_id, alternate_label, alternate_comment, position,
                  data_type, is_required, is_private, default_lang)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(bind_id(new.resource_template_id))
        .bind(bind_id(new.property_id))
        .bind(&new.alternate_label)
        .bind(&new.alternate_comment)
        .bind(new.position)
        .bind(database_json(&new.data_type)?)
        .bind(new.is_required)
        .bind(new.is_private)
        .bind(&new.default_lang);
        let id = insert_id(query, &mut *conn).await?;
        Ok(TemplatePropertyRow {
            id: TemplatePropertyId::from(id),
            resource_template_id: new.resource_template_id,
            property_id: new.property_id,
            alternate_label: new.alternate_label.clone(),
            alternate_comment: new.alternate_comment.clone(),
            position: new.position,
            data_type: new.data_type.clone(),
            is_required: new.is_required,
            is_private: new.is_private,
            default_lang: new.default_lang.clone(),
        })
    }

    /// The template's properties, ordered as upstream orders them
    /// (`@OrderBy({"position" = "ASC"})`): MySQL sorts NULL first ascending, so an unpositioned
    /// property leads.
    pub async fn list_properties(
        &self,
        template_id: TemplateId,
    ) -> Result<Vec<TemplatePropertyRow>, DbError> {
        let rows = sqlx::query(
            "SELECT * FROM resource_template_property
             WHERE resource_template_id = ?
             ORDER BY position, id",
        )
        .bind(bind_id(template_id))
        .fetch_all(self.pool)
        .await?;
        rows.iter()
            .map(TemplatePropertyRow::from_db_row::<DB>)
            .collect()
    }

    pub async fn update_property(&self, row: &TemplatePropertyRow) -> Result<bool, DbError> {
        let done = sqlx::query(
            "UPDATE resource_template_property
             SET resource_template_id = ?, property_id = ?, alternate_label = ?,
                 alternate_comment = ?, position = ?, data_type = ?, is_required = ?,
                 is_private = ?, default_lang = ?
             WHERE id = ?",
        )
        .bind(bind_id(row.resource_template_id))
        .bind(bind_id(row.property_id))
        .bind(&row.alternate_label)
        .bind(&row.alternate_comment)
        .bind(row.position)
        .bind(database_json(&row.data_type)?)
        .bind(row.is_required)
        .bind(row.is_private)
        .bind(&row.default_lang)
        .bind(bind_id(row.id))
        .execute(self.pool)
        .await?;
        Ok(DB::rows_affected(&done) > 0)
    }

    pub async fn delete_property(&self, id: TemplatePropertyId) -> Result<bool, DbError> {
        let done = sqlx::query("DELETE FROM resource_template_property WHERE id = ?")
            .bind(bind_id(id))
            .execute(self.pool)
            .await?;
        Ok(DB::rows_affected(&done) > 0)
    }
}

fn database_json(data_type: &Option<Vec<String>>) -> Result<Option<String>, DbError> {
    data_type
        .as_ref()
        .map(serde_json::to_string)
        .transpose()
        .map_err(|error| DbError::Column {
            column: "data_type",
            reason: format!("cannot store as JSON: {error}"),
        })
}
