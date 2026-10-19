//! `custom_vocab` repository: the term lists.
//!
//! A value that uses a list stores the data type name `customvocab:<id>` in `value.type`, and a
//! template row names it in its `data_type` list. Neither is a foreign key, so [`delete`] clears
//! both by hand inside one transaction, the way the module's `entity.remove.pre` listener does
//! (`Module::setVocabTypeToDefaultState`).
//!
//! [`delete`]: CustomVocabRepo::delete

use serde_json::Value;
use sqlx::{ColumnIndex, Decode, Encode, Executor, IntoArguments, Pool, Sqlite, Type};

use super::{bind_id, bind_opt_id, insert_id, json_text};
use crate::DbError;
use crate::backend::Backend;
use crate::ids::CustomVocabId;
use crate::rows::{CustomVocabRow, NewCustomVocab};

/// `custom_vocab` rows.
pub struct CustomVocabRepo<'a, DB: Backend = Sqlite> {
    pool: &'a Pool<DB>,
}

impl<'a, DB: Backend> CustomVocabRepo<'a, DB>
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

    pub async fn create(&self, new: &NewCustomVocab) -> Result<CustomVocabRow, DbError> {
        let query = sqlx::query(
            "INSERT INTO custom_vocab (item_set_id, owner_id, label, lang, terms, uris)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(bind_opt_id(new.item_set_id))
        .bind(bind_opt_id(new.owner_id))
        .bind(&new.label)
        .bind(&new.lang)
        .bind(opt_json_text(new.terms.as_ref())?)
        .bind(opt_json_text(new.uris.as_ref())?);
        let id = insert_id(query, self.pool).await?;
        Ok(CustomVocabRow {
            id: CustomVocabId::from(id),
            item_set_id: new.item_set_id,
            owner_id: new.owner_id,
            label: new.label.clone(),
            lang: new.lang.clone(),
            terms: new.terms.clone(),
            uris: new.uris.clone(),
        })
    }

    pub async fn get(&self, id: CustomVocabId) -> Result<Option<CustomVocabRow>, DbError> {
        let row = sqlx::query("SELECT * FROM custom_vocab WHERE id = ?")
            .bind(bind_id(id))
            .fetch_optional(self.pool)
            .await?;
        row.as_ref()
            .map(CustomVocabRow::from_db_row::<DB>)
            .transpose()
    }

    /// The list with this label. The comparison follows the column's collation, as the module's
    /// unique key does.
    pub async fn find_by_label(&self, label: &str) -> Result<Option<CustomVocabRow>, DbError> {
        let row = sqlx::query("SELECT * FROM custom_vocab WHERE label = ?")
            .bind(label)
            .fetch_optional(self.pool)
            .await?;
        row.as_ref()
            .map(CustomVocabRow::from_db_row::<DB>)
            .transpose()
    }

    /// Every list, by id.
    pub async fn list(&self) -> Result<Vec<CustomVocabRow>, DbError> {
        let rows = sqlx::query("SELECT * FROM custom_vocab ORDER BY id")
            .fetch_all(self.pool)
            .await?;
        rows.iter().map(CustomVocabRow::from_db_row::<DB>).collect()
    }

    pub async fn update(&self, row: &CustomVocabRow) -> Result<bool, DbError> {
        let done = sqlx::query(
            "UPDATE custom_vocab
             SET item_set_id = ?, owner_id = ?, label = ?, lang = ?, terms = ?, uris = ?
             WHERE id = ?",
        )
        .bind(bind_opt_id(row.item_set_id))
        .bind(bind_opt_id(row.owner_id))
        .bind(&row.label)
        .bind(&row.lang)
        .bind(opt_json_text(row.terms.as_ref())?)
        .bind(opt_json_text(row.uris.as_ref())?)
        .bind(bind_id(row.id))
        .execute(self.pool)
        .await?;
        Ok(DB::rows_affected(&done) > 0)
    }

    /// Delete a list, and turn every reference to its data type name back into a plain one.
    ///
    /// The module sets each value of the list back to `literal`. It also tries to clear the template
    /// rows, but its `data_type = ?` never matches the JSON list the column holds since Omeka S 4,
    /// so upstream leaves a dead name there. This removes the name from each template row instead,
    /// and a row left with no data type stores `NULL`, which is what an unrestricted row stores.
    pub async fn delete(&self, id: CustomVocabId) -> Result<bool, DbError> {
        let name = format!("customvocab:{id}");
        let mut tx = self.pool.begin().await?;
        let sql = format!(
            "UPDATE {} SET type = 'literal' WHERE type = ?",
            DB::quote_identifier("value")
        );
        sqlx::query(&sql).bind(&name).execute(&mut *tx).await?;
        let rows = sqlx::query(
            "SELECT id, data_type FROM resource_template_property WHERE data_type LIKE ?",
        )
        .bind(format!("%\"{name}\"%"))
        .fetch_all(&mut *tx)
        .await?;
        for row in &rows {
            let row_id = DB::int(row, "id")?;
            let kept: Vec<String> = DB::opt_string_list(row, "data_type")?
                .unwrap_or_default()
                .into_iter()
                .filter(|data_type| *data_type != name)
                .collect();
            let stored = if kept.is_empty() {
                None
            } else {
                Some(json_text(&Value::from(kept))?)
            };
            sqlx::query("UPDATE resource_template_property SET data_type = ? WHERE id = ?")
                .bind(stored)
                .bind(row_id)
                .execute(&mut *tx)
                .await?;
        }
        let done = sqlx::query("DELETE FROM custom_vocab WHERE id = ?")
            .bind(bind_id(id))
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(DB::rows_affected(&done) > 0)
    }
}

fn opt_json_text(value: Option<&Value>) -> Result<Option<String>, DbError> {
    value.map(json_text).transpose()
}
