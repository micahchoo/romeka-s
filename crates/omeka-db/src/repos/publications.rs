//! `romeka_publication` repository: the Romeka state of a published term list or vocabulary.
//!
//! A version is written once and never changed: this repository has no statement that updates or
//! deletes a `romeka_publication_version` row. [`PublicationRepo::publish`] writes the record and
//! the next version in one transaction, so a version number is never taken twice and a record is
//! never left pointing at a version that was not written.

use chrono::NaiveDateTime;
use serde_json::Value;
use sqlx::{ColumnIndex, Decode, Encode, Executor, IntoArguments, Pool, Sqlite, Type};

use super::{bind_id, insert_id, json_text};
use crate::DbError;
use crate::backend::Backend;
use crate::ids::PublicationId;
use crate::rows::{PublicationRow, PublicationTarget, PublicationVersionRow};
use crate::time;

/// `romeka_publication` and its versions.
pub struct PublicationRepo<'a, DB: Backend = Sqlite> {
    pool: &'a Pool<DB>,
}

/// What one publish stores: the record's fixed address and term state, and the new snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewPublicationVersion<'a> {
    pub slug: &'a str,
    pub namespace_uri: &'a str,
    pub terms: &'a Value,
    pub snapshot: &'a Value,
}

impl<'a, DB: Backend> PublicationRepo<'a, DB>
where
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
    usize: ColumnIndex<DB::Row>,
    for<'x> i64: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Option<i64>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> String: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Option<String>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> &'x str: Encode<'x, DB> + Type<DB> + ColumnIndex<DB::Row>,
{
    pub fn new(pool: &'a Pool<DB>) -> Self {
        Self { pool }
    }

    /// The record of one term list or vocabulary, `None` when it has no Romeka state.
    pub async fn get(&self, target: PublicationTarget) -> Result<Option<PublicationRow>, DbError> {
        let sql = format!(
            "SELECT * FROM romeka_publication WHERE {} = ?",
            target.column()
        );
        let row = sqlx::query(&sql)
            .bind(target.raw_id())
            .fetch_optional(self.pool)
            .await?;
        row.as_ref()
            .map(PublicationRow::from_db_row::<DB>)
            .transpose()
    }

    /// The record published under this slug.
    pub async fn by_slug(&self, slug: &str) -> Result<Option<PublicationRow>, DbError> {
        let row = sqlx::query("SELECT * FROM romeka_publication WHERE slug = ?")
            .bind(slug)
            .fetch_optional(self.pool)
            .await?;
        row.as_ref()
            .map(PublicationRow::from_db_row::<DB>)
            .transpose()
    }

    /// Store the term state of a target, creating its record on first use.
    pub async fn save_terms(
        &self,
        target: PublicationTarget,
        terms: &Value,
    ) -> Result<PublicationRow, DbError> {
        let mut tx = self.pool.begin().await?;
        let id = record_id::<DB>(&mut tx, target).await?;
        let terms_text = json_text(terms)?;
        let id = match id {
            Some(id) => {
                sqlx::query("UPDATE romeka_publication SET terms = ? WHERE id = ?")
                    .bind(terms_text)
                    .bind(id)
                    .execute(&mut *tx)
                    .await?;
                id
            }
            None => insert_record::<DB>(&mut tx, target, &terms_text).await?,
        };
        tx.commit().await?;
        self.by_id(id).await
    }

    /// Record one publish: fix the slug and namespace, store the term state, and append the next
    /// version. Returns the version written.
    pub async fn publish(
        &self,
        target: PublicationTarget,
        new: &NewPublicationVersion<'_>,
        created: NaiveDateTime,
    ) -> Result<PublicationVersionRow, DbError> {
        let mut tx = self.pool.begin().await?;
        let terms_text = json_text(new.terms)?;
        let id = match record_id::<DB>(&mut tx, target).await? {
            Some(id) => id,
            None => insert_record::<DB>(&mut tx, target, &terms_text).await?,
        };
        sqlx::query(
            "UPDATE romeka_publication SET slug = ?, namespace_uri = ?, terms = ? WHERE id = ?",
        )
        .bind(new.slug)
        .bind(new.namespace_uri)
        .bind(terms_text)
        .bind(id)
        .execute(&mut *tx)
        .await?;
        let last: Option<i64> = sqlx::query_scalar(
            "SELECT MAX(version) FROM romeka_publication_version WHERE publication_id = ?",
        )
        .bind(id)
        .fetch_one(&mut *tx)
        .await?;
        let version = last.unwrap_or(0) + 1;
        sqlx::query(
            "INSERT INTO romeka_publication_version (publication_id, version, created, snapshot)
             VALUES (?, ?, ?, ?)",
        )
        .bind(id)
        .bind(version)
        .bind(time::format(created))
        .bind(json_text(new.snapshot)?)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        let id = PublicationId(super::new_id(id)?);
        self.version(id, u32::try_from(version).unwrap_or(u32::MAX))
            .await?
            .ok_or(DbError::Column {
                column: "version",
                reason: "the version just written is missing".to_string(),
            })
    }

    /// Every version of a record, oldest first.
    pub async fn versions(&self, id: PublicationId) -> Result<Vec<PublicationVersionRow>, DbError> {
        let rows = sqlx::query(
            "SELECT * FROM romeka_publication_version WHERE publication_id = ? ORDER BY version",
        )
        .bind(bind_id(id))
        .fetch_all(self.pool)
        .await?;
        rows.iter()
            .map(PublicationVersionRow::from_db_row::<DB>)
            .collect()
    }

    /// One version of a record.
    pub async fn version(
        &self,
        id: PublicationId,
        version: u32,
    ) -> Result<Option<PublicationVersionRow>, DbError> {
        let row = sqlx::query(
            "SELECT * FROM romeka_publication_version WHERE publication_id = ? AND version = ?",
        )
        .bind(bind_id(id))
        .bind(i64::from(version))
        .fetch_optional(self.pool)
        .await?;
        row.as_ref()
            .map(PublicationVersionRow::from_db_row::<DB>)
            .transpose()
    }

    /// The newest version of a record, `None` while it is a draft.
    pub async fn latest(
        &self,
        id: PublicationId,
    ) -> Result<Option<PublicationVersionRow>, DbError> {
        let row = sqlx::query(
            "SELECT * FROM romeka_publication_version WHERE publication_id = ?
             ORDER BY version DESC LIMIT 1",
        )
        .bind(bind_id(id))
        .fetch_optional(self.pool)
        .await?;
        row.as_ref()
            .map(PublicationVersionRow::from_db_row::<DB>)
            .transpose()
    }

    async fn by_id(&self, id: i64) -> Result<PublicationRow, DbError> {
        let row = sqlx::query("SELECT * FROM romeka_publication WHERE id = ?")
            .bind(id)
            .fetch_one(self.pool)
            .await?;
        PublicationRow::from_db_row::<DB>(&row)
    }
}

async fn record_id<DB: Backend>(
    conn: &mut DB::Connection,
    target: PublicationTarget,
) -> Result<Option<i64>, DbError>
where
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
    for<'x> i64: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    usize: ColumnIndex<DB::Row>,
{
    let sql = format!(
        "SELECT id FROM romeka_publication WHERE {} = ?",
        target.column()
    );
    let id: Option<i64> = sqlx::query_scalar(&sql)
        .bind(target.raw_id())
        .fetch_optional(&mut *conn)
        .await?;
    Ok(id)
}

async fn insert_record<DB: Backend>(
    conn: &mut DB::Connection,
    target: PublicationTarget,
    terms: &str,
) -> Result<i64, DbError>
where
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
    for<'x> i64: Encode<'x, DB> + Type<DB>,
    for<'x> &'x str: Encode<'x, DB> + Type<DB>,
{
    let sql = format!(
        "INSERT INTO romeka_publication ({}, terms) VALUES (?, ?)",
        target.column()
    );
    let query = sqlx::query(&sql).bind(target.raw_id()).bind(terms);
    let id = insert_id(query, &mut *conn).await?;
    i64::try_from(id).map_err(|_| DbError::Column {
        column: "id",
        reason: format!("insert returned id {id}"),
    })
}
