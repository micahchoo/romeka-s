//! `site_block_attachment`: the items, media and assets a page block shows.
//!
//! `SitePageBlock` owns its attachments and removes them in Doctrine
//! (`Entity/SitePageBlock.php`); the database gives `block_id` no delete action, so a block delete
//! must remove its attachments first, which `SiteRepo::delete_block` does. `item_id` and
//! `media_id` are `ON DELETE SET NULL`, so deleting an item or a media leaves the attachment row
//! with a null reference, which the block layout renders as an empty slot.
//!
//! `SitePageBlock::getAttachments` orders by `position` (`@OrderBy({"position" = "ASC"})`).

use sqlx::{ColumnIndex, Decode, Encode, Executor, IntoArguments, Pool, Sqlite, Type};

use super::{bind_id, bind_opt_id, insert_id};
use crate::DbError;
use crate::backend::Backend;
use crate::ids::{AttachmentId, BlockId};
use crate::rows::{NewSiteBlockAttachment, SiteBlockAttachmentRow};

/// `site_block_attachment` rows.
pub struct SiteBlockAttachmentRepo<'a, DB: Backend = Sqlite> {
    pool: &'a Pool<DB>,
}

impl<'a, DB: Backend> SiteBlockAttachmentRepo<'a, DB>
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

    pub async fn create(
        &self,
        new: &NewSiteBlockAttachment,
    ) -> Result<SiteBlockAttachmentRow, DbError> {
        let query = sqlx::query(
            "INSERT INTO site_block_attachment (block_id, item_id, media_id, caption, position)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(bind_id(new.block_id))
        .bind(bind_opt_id(new.item_id))
        .bind(bind_opt_id(new.media_id))
        .bind(&new.caption)
        .bind(new.position);
        let id = insert_id(query, self.pool).await?;
        Ok(SiteBlockAttachmentRow {
            id: AttachmentId::from(id),
            block_id: new.block_id,
            item_id: new.item_id,
            media_id: new.media_id,
            caption: new.caption.clone(),
            position: new.position,
        })
    }

    pub async fn get(&self, id: AttachmentId) -> Result<Option<SiteBlockAttachmentRow>, DbError> {
        let row = sqlx::query("SELECT * FROM site_block_attachment WHERE id = ?")
            .bind(bind_id(id))
            .fetch_optional(self.pool)
            .await?;
        row.as_ref()
            .map(SiteBlockAttachmentRow::from_db_row::<DB>)
            .transpose()
    }

    /// The block's attachments, ordered as upstream orders them
    /// (`@OrderBy({"position" = "ASC"})`). MySQL sorts NULL first ascending, so an unpositioned
    /// row leads.
    pub async fn list_for_block(
        &self,
        block_id: BlockId,
    ) -> Result<Vec<SiteBlockAttachmentRow>, DbError> {
        let rows = sqlx::query(
            "SELECT * FROM site_block_attachment WHERE block_id = ? ORDER BY position, id",
        )
        .bind(bind_id(block_id))
        .fetch_all(self.pool)
        .await?;
        rows.iter()
            .map(SiteBlockAttachmentRow::from_db_row::<DB>)
            .collect()
    }

    /// Write an attachment's columns. A block layout uses this to reorder or recaption a slot.
    pub async fn update(&self, row: &SiteBlockAttachmentRow) -> Result<bool, DbError> {
        let done = sqlx::query(
            "UPDATE site_block_attachment
             SET block_id = ?, item_id = ?, media_id = ?, caption = ?, position = ?
             WHERE id = ?",
        )
        .bind(bind_id(row.block_id))
        .bind(bind_opt_id(row.item_id))
        .bind(bind_opt_id(row.media_id))
        .bind(&row.caption)
        .bind(row.position)
        .bind(bind_id(row.id))
        .execute(self.pool)
        .await?;
        Ok(DB::rows_affected(&done) > 0)
    }

    pub async fn delete(&self, id: AttachmentId) -> Result<bool, DbError> {
        let done = sqlx::query("DELETE FROM site_block_attachment WHERE id = ?")
            .bind(bind_id(id))
            .execute(self.pool)
            .await?;
        Ok(DB::rows_affected(&done) > 0)
    }
}
