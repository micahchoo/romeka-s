//! `romeka_site_page_version`: a page's saved states, and the save that makes one.
//!
//! [`PageVersionRepo::save`] is the whole of a page save in one serialized transaction: the
//! conflict check, the page row, the block and attachment rewrite, the new version and the
//! retention trim. A failing row rolls back all of it, so a save never leaves a page with only some
//! of its blocks. No statement here updates a version row.
//!
//! Each version holds a state as it went live. A page's first save also records the state it
//! replaces, with no user, so the content a page had before history began is kept too.

use chrono::NaiveDateTime;
use serde_json::Value;
use sqlx::{ColumnIndex, Decode, Encode, Executor, IntoArguments, Pool, Sqlite, Transaction, Type};

use super::{bind_id, bind_opt_id, insert_id, json_text};
use crate::DbError;
use crate::backend::Backend;
use crate::ids::{BlockId, PageId, UserId};
use crate::rows::{
    BlockChange, NewPageBlock, PageImage, PageImageBlock, PageSave, PageSaveOutcome,
    PageVersionRow, SiteBlockAttachmentRow, SiteBlockRow, SitePageRow,
};
use crate::time;

/// How many versions a page keeps. A save that makes the 101st deletes the oldest.
pub const KEPT_VERSIONS: u32 = 100;

/// Turns a stored page into the snapshot a version keeps. The caller owns the snapshot's shape.
pub type Snapshot<'s> = &'s (dyn Fn(&PageImage) -> Result<Value, DbError> + Send + Sync);

/// A page's versions, and the save that appends one.
pub struct PageVersionRepo<'a, DB: Backend = Sqlite> {
    pool: &'a Pool<DB>,
}

impl<'a, DB: Backend> PageVersionRepo<'a, DB>
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
    pub fn new(pool: &'a Pool<DB>) -> Self {
        Self { pool }
    }

    /// Save a page and record the state it now has as its next version.
    pub async fn save(
        &self,
        save: &PageSave<'_>,
        snapshot: Snapshot<'_>,
    ) -> Result<PageSaveOutcome, DbError> {
        let mut tx = self.pool.begin_with(DB::begin_write()).await?;
        let Some(current) = page_row::<DB>(&mut tx, save.page.id, DB::for_update()).await? else {
            return Ok(PageSaveOutcome::Missing);
        };
        let last = latest::<DB>(&mut tx, save.page.id).await?;
        if let Some(expected) = save.expected_modified
            && current.modified != Some(expected)
        {
            return Ok(PageSaveOutcome::Stale {
                modified: current.modified,
                saved_by: last.and_then(|(_, user)| user),
            });
        }
        let mut version = last.map_or(0, |(version, _)| version);
        if version == 0 {
            let before = image::<DB>(&mut tx, current).await?;
            let created = before.page.modified.unwrap_or(before.page.created);
            version = 1;
            insert_version::<DB>(
                &mut tx,
                save.page.id,
                version,
                created,
                None,
                &snapshot(&before)?,
            )
            .await?;
        }
        let now = time::now();
        write_page::<DB>(&mut tx, save.page, now).await?;
        match save.blocks {
            BlockChange::Keep => {}
            BlockChange::Replace(blocks) => {
                clear_blocks::<DB>(&mut tx, save.page.id).await?;
                insert_blocks::<DB>(&mut tx, save.page.id, blocks).await?;
            }
            BlockChange::Append(blocks) => {
                insert_blocks::<DB>(&mut tx, save.page.id, blocks).await?
            }
        }
        let Some(saved) = page_row::<DB>(&mut tx, save.page.id, "").await? else {
            return Ok(PageSaveOutcome::Missing);
        };
        let after = image::<DB>(&mut tx, saved).await?;
        version += 1;
        insert_version::<DB>(
            &mut tx,
            save.page.id,
            version,
            now,
            save.user,
            &snapshot(&after)?,
        )
        .await?;
        sqlx::query("DELETE FROM romeka_site_page_version WHERE page_id = ? AND version <= ?")
            .bind(bind_id(save.page.id))
            .bind(i64::from(version) - i64::from(KEPT_VERSIONS))
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(PageSaveOutcome::Saved {
            page: after.page,
            version,
        })
    }

    /// Every kept version of a page, newest first.
    pub async fn list(&self, page: PageId) -> Result<Vec<PageVersionRow>, DbError> {
        let rows = sqlx::query(
            "SELECT * FROM romeka_site_page_version WHERE page_id = ? ORDER BY version DESC",
        )
        .bind(bind_id(page))
        .fetch_all(self.pool)
        .await?;
        rows.iter().map(PageVersionRow::from_db_row::<DB>).collect()
    }

    /// One version of a page, `None` when it was never made or no longer kept.
    pub async fn get(&self, page: PageId, version: u32) -> Result<Option<PageVersionRow>, DbError> {
        let row =
            sqlx::query("SELECT * FROM romeka_site_page_version WHERE page_id = ? AND version = ?")
                .bind(bind_id(page))
                .bind(i64::from(version))
                .fetch_optional(self.pool)
                .await?;
        row.as_ref()
            .map(PageVersionRow::from_db_row::<DB>)
            .transpose()
    }
}

async fn page_row<DB: Backend>(
    tx: &mut Transaction<'static, DB>,
    id: PageId,
    lock: &str,
) -> Result<Option<SitePageRow>, DbError>
where
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
    for<'x> i64: Encode<'x, DB> + Type<DB>,
{
    let sql = format!("SELECT * FROM site_page WHERE id = ?{lock}");
    let row = sqlx::query(&sql)
        .bind(bind_id(id))
        .fetch_optional(&mut **tx)
        .await?;
    row.as_ref().map(SitePageRow::from_db_row::<DB>).transpose()
}

/// The newest version number and its user, `None` before the first save.
async fn latest<DB: Backend>(
    tx: &mut Transaction<'static, DB>,
    page: PageId,
) -> Result<Option<(u32, Option<UserId>)>, DbError>
where
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
    usize: ColumnIndex<DB::Row>,
    for<'x> i64: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Option<i64>: Decode<'x, DB> + Type<DB>,
{
    let row: Option<(i64, Option<i64>)> = sqlx::query_as(
        "SELECT version, user_id FROM romeka_site_page_version WHERE page_id = ?
         ORDER BY version DESC LIMIT 1",
    )
    .bind(bind_id(page))
    .fetch_optional(&mut **tx)
    .await?;
    row.map(|(version, user)| {
        let version = u32::try_from(version).map_err(|_| DbError::Column {
            column: "version",
            reason: format!("not a version number: {version}"),
        })?;
        let user = user
            .map(|raw| super::new_id(raw).map(UserId::from))
            .transpose()?;
        Ok((version, user))
    })
    .transpose()
}

async fn image<DB: Backend>(
    tx: &mut Transaction<'static, DB>,
    page: SitePageRow,
) -> Result<PageImage, DbError>
where
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
    usize: ColumnIndex<DB::Row>,
    for<'x> i64: Encode<'x, DB> + Type<DB>,
    for<'x> &'x str: ColumnIndex<DB::Row>,
{
    let rows = sqlx::query("SELECT * FROM site_page_block WHERE page_id = ? ORDER BY position, id")
        .bind(bind_id(page.id))
        .fetch_all(&mut **tx)
        .await?;
    let mut blocks = Vec::with_capacity(rows.len());
    for row in &rows {
        let block = SiteBlockRow::from_db_row::<DB>(row)?;
        let attachments = sqlx::query(
            "SELECT * FROM site_block_attachment WHERE block_id = ? ORDER BY position, id",
        )
        .bind(bind_id(block.id))
        .fetch_all(&mut **tx)
        .await?
        .iter()
        .map(SiteBlockAttachmentRow::from_db_row::<DB>)
        .collect::<Result<_, _>>()?;
        blocks.push(PageImageBlock { block, attachments });
    }
    Ok(PageImage { page, blocks })
}

async fn insert_version<DB: Backend>(
    tx: &mut Transaction<'static, DB>,
    page: PageId,
    version: u32,
    created: NaiveDateTime,
    user: Option<UserId>,
    snapshot: &Value,
) -> Result<(), DbError>
where
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
    for<'x> i64: Encode<'x, DB> + Type<DB>,
    for<'x> Option<i64>: Encode<'x, DB> + Type<DB>,
    for<'x> String: Encode<'x, DB> + Type<DB>,
{
    sqlx::query(
        "INSERT INTO romeka_site_page_version (page_id, version, created, user_id, snapshot)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(bind_id(page))
    .bind(i64::from(version))
    .bind(time::format(created))
    .bind(bind_opt_id(user))
    .bind(json_text(snapshot)?)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Write every page column except `created`, with `modified` at `now`.
async fn write_page<DB: Backend>(
    tx: &mut Transaction<'static, DB>,
    page: &SitePageRow,
    now: NaiveDateTime,
) -> Result<(), DbError>
where
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
    for<'x> i64: Encode<'x, DB> + Type<DB>,
    for<'x> bool: Encode<'x, DB> + Type<DB>,
    for<'x> String: Encode<'x, DB> + Type<DB>,
    for<'x> Option<String>: Encode<'x, DB> + Type<DB>,
    for<'x> &'x str: Encode<'x, DB> + Type<DB>,
{
    sqlx::query(
        "UPDATE site_page
         SET slug = ?, title = ?, is_public = ?, layout = ?, layout_data = ?, modified = ?
         WHERE id = ?",
    )
    .bind(page.slug.as_str())
    .bind(page.title.as_str())
    .bind(page.is_public)
    .bind(page.layout.clone())
    .bind(page.layout_data.as_ref().map(json_text).transpose()?)
    .bind(time::format(now))
    .bind(bind_id(page.id))
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn clear_blocks<DB: Backend>(
    tx: &mut Transaction<'static, DB>,
    page: PageId,
) -> Result<(), DbError>
where
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
    for<'x> i64: Encode<'x, DB> + Type<DB>,
{
    sqlx::query(
        "DELETE FROM site_block_attachment WHERE block_id IN (
             SELECT id FROM site_page_block WHERE page_id = ?)",
    )
    .bind(bind_id(page))
    .execute(&mut **tx)
    .await?;
    sqlx::query("DELETE FROM site_page_block WHERE page_id = ?")
        .bind(bind_id(page))
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// Insert blocks at positions 1, 2, ... and their attachments likewise, as
/// `SitePageAdapter::hydrateBlocks` numbers them.
async fn insert_blocks<DB: Backend>(
    tx: &mut Transaction<'static, DB>,
    page: PageId,
    blocks: &[NewPageBlock],
) -> Result<(), DbError>
where
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
    for<'x> i64: Encode<'x, DB> + Type<DB>,
    for<'x> Option<i64>: Encode<'x, DB> + Type<DB>,
    for<'x> String: Encode<'x, DB> + Type<DB>,
    for<'x> Option<String>: Encode<'x, DB> + Type<DB>,
    for<'x> &'x str: Encode<'x, DB> + Type<DB>,
{
    for (position, block) in (1_i64..).zip(blocks) {
        let query = sqlx::query(
            "INSERT INTO site_page_block (page_id, layout, data, layout_data, position)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(bind_id(page))
        .bind(block.layout.as_str())
        .bind(json_text(&block.data)?)
        .bind(block.layout_data.as_ref().map(json_text).transpose()?)
        .bind(position);
        let id = BlockId::from(insert_id(query, &mut **tx).await?);
        for (place, attachment) in (1_i64..).zip(&block.attachments) {
            sqlx::query(
                "INSERT INTO site_block_attachment (block_id, item_id, media_id, caption, position)
                 VALUES (?, ?, ?, ?, ?)",
            )
            .bind(bind_id(id))
            .bind(bind_opt_id(attachment.item_id))
            .bind(bind_opt_id(attachment.media_id))
            .bind(attachment.caption.as_str())
            .bind(place)
            .execute(&mut **tx)
            .await?;
        }
    }
    Ok(())
}
