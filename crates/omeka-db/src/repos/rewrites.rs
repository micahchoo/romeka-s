//! Bulk value rewrites: the census of a scope's text values, promoting tags into a term list, and
//! merging spellings.
//!
//! The caller decides which values change (it checks permissions and plans the merge); this
//! repository reads the rows that decision needs and writes it in one transaction, so a failing
//! row leaves no list, value or template row half changed. Each statement re-checks the type it
//! expects, so a value another writer changed after the read is left as that writer left it.

use serde_json::Value;
use sqlx::{ColumnIndex, Decode, Encode, Executor, IntoArguments, Pool, Sqlite, Type};

use super::resources::touch_modified;
use super::{bind_id, bind_opt_id, insert_id, json_text};
use crate::DbError;
use crate::backend::Backend;
use crate::ids::{CustomVocabId, PropertyId, TemplateId};
use crate::rows::{
    MergeWrite, PromoteList, PromoteWrite, PublicationTarget, TextValueRow, resource_type,
};

/// Bulk rewrites over `value`.
pub struct ValueRewriteRepo<'a, DB: Backend = Sqlite> {
    pool: &'a Pool<DB>,
}

impl<'a, DB: Backend> ValueRewriteRepo<'a, DB>
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

    /// Every text value of this type on an item, item set or media, optionally of one property,
    /// in value id order. A URI or resource value of the type is not text and is left out.
    pub async fn text_values(
        &self,
        of_type: &str,
        property: Option<PropertyId>,
    ) -> Result<Vec<TextValueRow>, DbError> {
        let sql = format!(
            "SELECT v.id, v.resource_id, v.property_id, v.type, v.lang, v.value, v.is_public,
                    v.value_annotation_id, r.resource_type, r.owner_id,
                    r.is_public AS resource_is_public, r.resource_template_id
             FROM {} v JOIN resource r ON r.id = v.resource_id
             WHERE v.type = ? AND (? IS NULL OR v.property_id = ?)
               AND v.value IS NOT NULL AND v.uri IS NULL AND v.value_resource_id IS NULL
               AND r.resource_type IN (?, ?, ?)
             ORDER BY v.id",
            DB::quote_identifier("value")
        );
        let property = bind_opt_id(property);
        let rows = sqlx::query(&sql)
            .bind(of_type)
            .bind(property)
            .bind(property)
            .bind(resource_type::ITEM)
            .bind(resource_type::ITEM_SET)
            .bind(resource_type::MEDIA)
            .fetch_all(self.pool)
            .await?;
        rows.iter().map(TextValueRow::from_db_row::<DB>).collect()
    }

    /// Write one promotion and return the list's id.
    pub async fn promote(&self, write: &PromoteWrite) -> Result<CustomVocabId, DbError> {
        let mut tx = self.pool.begin_with(DB::begin_write()).await?;
        let list = match &write.list {
            PromoteList::Create(new) => {
                let query = sqlx::query(
                    "INSERT INTO custom_vocab (item_set_id, owner_id, label, lang, terms, uris)
                     VALUES (?, ?, ?, ?, ?, ?)",
                )
                .bind(bind_opt_id(new.item_set_id))
                .bind(bind_opt_id(new.owner_id))
                .bind(&new.label)
                .bind(&new.lang)
                .bind(new.terms.as_ref().map(json_text).transpose()?)
                .bind(new.uris.as_ref().map(json_text).transpose()?);
                CustomVocabId::from(insert_id(query, &mut *tx).await?)
            }
            PromoteList::Terms(id, terms) => {
                if let Some(read) = &write.terms_read {
                    check_terms::<DB>(&mut tx, *id, read.as_ref()).await?;
                }
                set_terms(&mut tx, *id, terms).await?;
                *id
            }
        };
        let name = format!("customvocab:{list}");
        let retype = format!(
            "UPDATE {} SET type = ? WHERE id = ? AND type = ?",
            DB::quote_identifier("value")
        );
        for value in &write.values {
            sqlx::query(&retype)
                .bind(&name)
                .bind(bind_id(*value))
                .bind(&write.from_type)
                .execute(&mut *tx)
                .await?;
        }
        for template in &write.templates {
            offer_on_row(
                &mut tx,
                *template,
                write.property,
                &name,
                &write.unrestricted,
            )
            .await?;
        }
        touch_modified::<DB>(&mut tx, &write.resources).await?;
        tx.commit().await?;
        Ok(list)
    }

    /// Write one merge.
    pub async fn merge(&self, write: &MergeWrite) -> Result<(), DbError> {
        let table = DB::quote_identifier("value");
        let mut tx = self.pool.begin_with(DB::begin_write()).await?;
        if let (Some((list, _)), Some(read)) = (&write.list_terms, &write.terms_read) {
            check_terms::<DB>(&mut tx, *list, read.as_ref()).await?;
        }
        let rewrite = format!("UPDATE {table} SET value = ? WHERE id = ? AND type = ?");
        for value in &write.rewrite {
            sqlx::query(&rewrite)
                .bind(&write.to)
                .bind(bind_id(*value))
                .bind(&write.of_type)
                .execute(&mut *tx)
                .await?;
        }
        let delete = format!("DELETE FROM {table} WHERE id = ? AND type = ?");
        for value in &write.delete {
            sqlx::query(&delete)
                .bind(bind_id(*value))
                .bind(&write.of_type)
                .execute(&mut *tx)
                .await?;
        }
        if let Some((list, terms)) = &write.list_terms {
            set_terms(&mut tx, *list, terms).await?;
        }
        if let Some((list, states)) = &write.list_states {
            let column = PublicationTarget::TermList(*list).column();
            sqlx::query(&format!(
                "UPDATE romeka_publication SET terms = ? WHERE {column} = ?"
            ))
            .bind(json_text(states)?)
            .bind(bind_id(*list))
            .execute(&mut *tx)
            .await?;
        }
        // `ResourceTitleHydrator`: the first non-empty text of the title property.
        let retitle = format!(
            "UPDATE resource SET title = (
                 SELECT v.value FROM {table} v
                 WHERE v.resource_id = resource.id
                   AND v.property_id = COALESCE(
                       (SELECT t.title_property_id FROM resource_template t
                        WHERE t.id = resource.resource_template_id), ?)
                   AND v.value IS NOT NULL AND v.value <> '' AND v.value_resource_id IS NULL
                 ORDER BY v.id LIMIT 1)
             WHERE id = ?"
        );
        for resource in &write.resources {
            sqlx::query(&retitle)
                .bind(bind_opt_id(write.default_title_property))
                .bind(bind_id(*resource))
                .execute(&mut *tx)
                .await?;
        }
        touch_modified::<DB>(&mut tx, &write.resources).await?;
        tx.commit().await?;
        Ok(())
    }
}

/// Lock the list's row and refuse with [`DbError::Conflict`] when its `terms` are not `read`.
async fn check_terms<DB: Backend>(
    conn: &mut DB::Connection,
    list: CustomVocabId,
    read: Option<&Value>,
) -> Result<(), DbError>
where
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
    usize: ColumnIndex<DB::Row>,
    for<'x> i64: Encode<'x, DB> + Type<DB>,
{
    let sql = format!(
        "SELECT terms FROM custom_vocab WHERE id = ?{}",
        DB::for_update()
    );
    let row = sqlx::query(&sql)
        .bind(bind_id(list))
        .fetch_optional(&mut *conn)
        .await?
        .ok_or(DbError::Conflict("the term list is gone"))?;
    if DB::opt_json(&row, "terms")?.as_ref() != read {
        return Err(DbError::Conflict("the term list changed"));
    }
    Ok(())
}

async fn set_terms<DB: Backend>(
    conn: &mut DB::Connection,
    list: CustomVocabId,
    terms: &Value,
) -> Result<(), DbError>
where
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
    for<'x> i64: Encode<'x, DB> + Type<DB>,
    for<'x> String: Encode<'x, DB> + Type<DB>,
{
    sqlx::query("UPDATE custom_vocab SET terms = ? WHERE id = ?")
        .bind(json_text(terms)?)
        .bind(bind_id(list))
        .execute(conn)
        .await?;
    Ok(())
}

/// The template's row for `property` comes to offer `name`. A row with no data types offered
/// `unrestricted`, so it keeps those beside the list; a template with no row gets one, last.
async fn offer_on_row<DB: Backend>(
    conn: &mut DB::Connection,
    template: TemplateId,
    property: PropertyId,
    name: &str,
    unrestricted: &[String],
) -> Result<(), DbError>
where
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
    usize: ColumnIndex<DB::Row>,
    for<'x> i64: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Option<i64>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> String: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Option<String>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Vec<u8>: Decode<'x, DB> + Type<DB>,
    for<'x> Option<Vec<u8>>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> &'x str: Encode<'x, DB> + Type<DB> + ColumnIndex<DB::Row>,
    for<'x> bool: Encode<'x, DB> + Type<DB>,
{
    let row = sqlx::query(
        "SELECT id, data_type FROM resource_template_property
         WHERE resource_template_id = ? AND property_id = ?",
    )
    .bind(bind_id(template))
    .bind(bind_id(property))
    .fetch_optional(&mut *conn)
    .await?;
    if let Some(row) = row {
        let mut offered =
            DB::opt_string_list(&row, "data_type")?.unwrap_or_else(|| unrestricted.to_vec());
        if offered.iter().any(|offer| offer == name) {
            return Ok(());
        }
        offered.push(name.to_string());
        sqlx::query("UPDATE resource_template_property SET data_type = ? WHERE id = ?")
            .bind(json_text(&Value::from(offered))?)
            .bind(DB::int(&row, "id")?)
            .execute(&mut *conn)
            .await?;
        return Ok(());
    }
    let last = sqlx::query(
        "SELECT MAX(position) AS last FROM resource_template_property
         WHERE resource_template_id = ?",
    )
    .bind(bind_id(template))
    .fetch_one(&mut *conn)
    .await?;
    let position = DB::opt_int(&last, "last")?.unwrap_or(0) + 1;
    sqlx::query(
        "INSERT INTO resource_template_property
             (resource_template_id, property_id, alternate_label, alternate_comment, position,
              data_type, is_required, is_private, default_lang)
         VALUES (?, ?, NULL, NULL, ?, ?, ?, ?, NULL)",
    )
    .bind(bind_id(template))
    .bind(bind_id(property))
    .bind(position)
    .bind(json_text(&Value::from(vec![name.to_string()]))?)
    .bind(false)
    .bind(false)
    .execute(&mut *conn)
    .await?;
    Ok(())
}
