//! `site` and its children: pages, blocks, block attachments, item sets and permissions.
//!
//! Upstream cascades in Doctrine (`Entity/Site.php` removes pages, permissions and item sets;
//! `Entity/SitePage.php` removes blocks; `Entity/SitePageBlock.php` removes attachments), while the
//! database only cascades `site_item_set`, `site_permission` and `site_setting`. The repository
//! performs the Doctrine removals, so deleting a site leaves nothing behind.
//!
//! Filters mirror `SiteAdapter::buildQuery`: `owner_id`, `slug`, `exclude_id`, `assign_new_items`.

use sqlx::{ColumnIndex, Decode, Encode, Executor, IntoArguments, Pool, Sqlite, Type};

use super::{
    Paging, Statement, bind_id, bind_opt_id, fetch_rows, insert_id, json_text, push_paging,
};
use crate::DbError;
use crate::backend::Backend;
use crate::ids::{BlockId, ItemSetId, PageId, SiteId, UserId};
use crate::rows::{NewSite, NewSiteBlock, NewSitePage, SiteBlockRow, SitePageRow, SiteRow};
use crate::time;

/// A list filter for `site`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SiteFilter {
    pub owner_id: Option<UserId>,
    pub slug: Option<String>,
    /// Skip this site, for slug checks on an update (upstream `exclude_id`).
    pub exclude_id: Option<SiteId>,
    pub assign_new_items: Option<bool>,
}

/// `site` rows and their children.
pub struct SiteRepo<'a, DB: Backend = Sqlite> {
    pool: &'a Pool<DB>,
}

impl<'a, DB: Backend> SiteRepo<'a, DB>
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

    pub async fn create(&self, new: &NewSite) -> Result<SiteRow, DbError> {
        let created = new.created.unwrap_or_else(time::now);
        let query = sqlx::query(
            "INSERT INTO site
                 (thumbnail_id, homepage_id, owner_id, slug, theme, title, summary, navigation,
                  item_pool, created, modified, is_public, assign_new_items)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(bind_opt_id(new.thumbnail_id))
        .bind(bind_opt_id(new.homepage_id))
        .bind(bind_opt_id(new.owner_id))
        .bind(&new.slug)
        .bind(&new.theme)
        .bind(&new.title)
        .bind(&new.summary)
        .bind(json_text(&new.navigation)?)
        .bind(json_text(&new.item_pool)?)
        .bind(time::format(created))
        .bind(time::format(created))
        .bind(new.is_public)
        .bind(new.assign_new_items);
        let id = insert_id(query, self.pool).await?;
        Ok(SiteRow {
            id: SiteId::from(id),
            thumbnail_id: new.thumbnail_id,
            homepage_id: new.homepage_id,
            owner_id: new.owner_id,
            slug: new.slug.clone(),
            theme: new.theme.clone(),
            title: new.title.clone(),
            summary: new.summary.clone(),
            navigation: new.navigation.clone(),
            item_pool: new.item_pool.clone(),
            created,
            modified: Some(created),
            is_public: new.is_public,
            assign_new_items: new.assign_new_items,
        })
    }

    pub async fn get(&self, id: SiteId) -> Result<Option<SiteRow>, DbError> {
        let row = sqlx::query("SELECT * FROM site WHERE id = ?")
            .bind(bind_id(id))
            .fetch_optional(self.pool)
            .await?;
        row.as_ref().map(SiteRow::from_db_row::<DB>).transpose()
    }

    /// The public site with this slug. The comparison follows the column's collation, as upstream's
    /// unique key does.
    pub async fn get_by_slug(&self, slug: &str) -> Result<Option<SiteRow>, DbError> {
        let row = sqlx::query("SELECT * FROM site WHERE slug = ?")
            .bind(slug)
            .fetch_optional(self.pool)
            .await?;
        row.as_ref().map(SiteRow::from_db_row::<DB>).transpose()
    }

    /// Write every column except `created`, and set `modified` to now.
    pub async fn update(&self, row: &SiteRow) -> Result<bool, DbError> {
        let done = sqlx::query(
            "UPDATE site
             SET thumbnail_id = ?, homepage_id = ?, owner_id = ?, slug = ?, theme = ?, title = ?,
                 summary = ?, navigation = ?, item_pool = ?, modified = ?, is_public = ?,
                 assign_new_items = ?
             WHERE id = ?",
        )
        .bind(bind_opt_id(row.thumbnail_id))
        .bind(bind_opt_id(row.homepage_id))
        .bind(bind_opt_id(row.owner_id))
        .bind(&row.slug)
        .bind(&row.theme)
        .bind(&row.title)
        .bind(&row.summary)
        .bind(json_text(&row.navigation)?)
        .bind(json_text(&row.item_pool)?)
        .bind(time::format(time::now()))
        .bind(row.is_public)
        .bind(row.assign_new_items)
        .bind(bind_id(row.id))
        .execute(self.pool)
        .await?;
        Ok(DB::rows_affected(&done) > 0)
    }

    /// Delete a site with its pages, blocks, attachments and membership rows.
    pub async fn delete(&self, id: SiteId) -> Result<bool, DbError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "DELETE FROM site_block_attachment WHERE block_id IN (
                 SELECT site_page_block.id FROM site_page_block
                 JOIN site_page ON site_page.id = site_page_block.page_id
                 WHERE site_page.site_id = ?)",
        )
        .bind(bind_id(id))
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "DELETE FROM site_page_block WHERE page_id IN (
                 SELECT id FROM site_page WHERE site_id = ?)",
        )
        .bind(bind_id(id))
        .execute(&mut *tx)
        .await?;
        sqlx::query("DELETE FROM site_page WHERE site_id = ?")
            .bind(bind_id(id))
            .execute(&mut *tx)
            .await?;
        let done = sqlx::query("DELETE FROM site WHERE id = ?")
            .bind(bind_id(id))
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(DB::rows_affected(&done) > 0)
    }

    pub async fn list(
        &self,
        filter: &SiteFilter,
        paging: &Paging,
    ) -> Result<Vec<SiteRow>, DbError> {
        let mut statement = Statement::<DB>::new("SELECT * FROM site WHERE 1 = 1");
        if let Some(owner_id) = filter.owner_id {
            statement.push(" AND owner_id = ");
            statement.bind(bind_id(owner_id))?;
        }
        if let Some(slug) = &filter.slug {
            statement.push(" AND slug = ");
            statement.bind(slug)?;
        }
        if let Some(exclude_id) = filter.exclude_id {
            statement.push(" AND id <> ");
            statement.bind(bind_id(exclude_id))?;
        }
        if let Some(assign_new_items) = filter.assign_new_items {
            statement.push(" AND assign_new_items = ");
            statement.bind(assign_new_items)?;
        }
        statement.push(" ORDER BY id");
        push_paging(&mut statement, paging)?;
        let (sql, arguments) = statement.into_parts();
        let rows: Vec<DB::Row> = fetch_rows(&sql, arguments, self.pool).await?;
        rows.iter().map(SiteRow::from_db_row::<DB>).collect()
    }

    pub async fn add_page(&self, new: &NewSitePage) -> Result<SitePageRow, DbError> {
        let created = new.created.unwrap_or_else(time::now);
        let query = sqlx::query(
            "INSERT INTO site_page
                 (site_id, slug, title, is_public, layout, layout_data, created, modified)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(bind_id(new.site_id))
        .bind(&new.slug)
        .bind(&new.title)
        .bind(new.is_public)
        .bind(&new.layout)
        .bind(new.layout_data.as_ref().map(json_text).transpose()?)
        .bind(time::format(created))
        .bind(time::format(created));
        let id = insert_id(query, self.pool).await?;
        Ok(SitePageRow {
            id: PageId::from(id),
            site_id: new.site_id,
            slug: new.slug.clone(),
            title: new.title.clone(),
            is_public: new.is_public,
            layout: new.layout.clone(),
            layout_data: new.layout_data.clone(),
            created,
            modified: Some(created),
        })
    }

    pub async fn get_page(&self, id: PageId) -> Result<Option<SitePageRow>, DbError> {
        let row = sqlx::query("SELECT * FROM site_page WHERE id = ?")
            .bind(bind_id(id))
            .fetch_optional(self.pool)
            .await?;
        row.as_ref().map(SitePageRow::from_db_row::<DB>).transpose()
    }

    /// The site's pages, in creation order. Upstream orders navigation by `site.navigation`, not by
    /// a page column.
    pub async fn list_pages(&self, site_id: SiteId) -> Result<Vec<SitePageRow>, DbError> {
        let rows = sqlx::query("SELECT * FROM site_page WHERE site_id = ? ORDER BY id")
            .bind(bind_id(site_id))
            .fetch_all(self.pool)
            .await?;
        rows.iter().map(SitePageRow::from_db_row::<DB>).collect()
    }

    /// Write every page column except `created`, and set `modified` to now.
    pub async fn update_page(&self, row: &SitePageRow) -> Result<bool, DbError> {
        let done = sqlx::query(
            "UPDATE site_page
             SET site_id = ?, slug = ?, title = ?, is_public = ?, layout = ?, layout_data = ?,
                 modified = ?
             WHERE id = ?",
        )
        .bind(bind_id(row.site_id))
        .bind(&row.slug)
        .bind(&row.title)
        .bind(row.is_public)
        .bind(&row.layout)
        .bind(row.layout_data.as_ref().map(json_text).transpose()?)
        .bind(time::format(time::now()))
        .bind(bind_id(row.id))
        .execute(self.pool)
        .await?;
        Ok(DB::rows_affected(&done) > 0)
    }

    pub async fn delete_page(&self, id: PageId) -> Result<bool, DbError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "DELETE FROM site_block_attachment WHERE block_id IN (
                 SELECT id FROM site_page_block WHERE page_id = ?)",
        )
        .bind(bind_id(id))
        .execute(&mut *tx)
        .await?;
        sqlx::query("DELETE FROM site_page_block WHERE page_id = ?")
            .bind(bind_id(id))
            .execute(&mut *tx)
            .await?;
        let done = sqlx::query("DELETE FROM site_page WHERE id = ?")
            .bind(bind_id(id))
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(DB::rows_affected(&done) > 0)
    }

    pub async fn add_block(&self, new: &NewSiteBlock) -> Result<SiteBlockRow, DbError> {
        let query = sqlx::query(
            "INSERT INTO site_page_block (page_id, layout, data, layout_data, position)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(bind_id(new.page_id))
        .bind(&new.layout)
        .bind(json_text(&new.data)?)
        .bind(new.layout_data.as_ref().map(json_text).transpose()?)
        .bind(new.position);
        let id = insert_id(query, self.pool).await?;
        Ok(SiteBlockRow {
            id: BlockId::from(id),
            page_id: new.page_id,
            layout: new.layout.clone(),
            data: new.data.clone(),
            layout_data: new.layout_data.clone(),
            position: new.position,
        })
    }

    pub async fn get_block(&self, id: BlockId) -> Result<Option<SiteBlockRow>, DbError> {
        let row = sqlx::query("SELECT * FROM site_page_block WHERE id = ?")
            .bind(bind_id(id))
            .fetch_optional(self.pool)
            .await?;
        row.as_ref()
            .map(SiteBlockRow::from_db_row::<DB>)
            .transpose()
    }

    pub async fn list_blocks(&self, page_id: PageId) -> Result<Vec<SiteBlockRow>, DbError> {
        let rows =
            sqlx::query("SELECT * FROM site_page_block WHERE page_id = ? ORDER BY position, id")
                .bind(bind_id(page_id))
                .fetch_all(self.pool)
                .await?;
        rows.iter().map(SiteBlockRow::from_db_row::<DB>).collect()
    }

    pub async fn update_block(&self, row: &SiteBlockRow) -> Result<bool, DbError> {
        let done = sqlx::query(
            "UPDATE site_page_block
             SET page_id = ?, layout = ?, data = ?, layout_data = ?, position = ?
             WHERE id = ?",
        )
        .bind(bind_id(row.page_id))
        .bind(&row.layout)
        .bind(json_text(&row.data)?)
        .bind(row.layout_data.as_ref().map(json_text).transpose()?)
        .bind(row.position)
        .bind(bind_id(row.id))
        .execute(self.pool)
        .await?;
        Ok(DB::rows_affected(&done) > 0)
    }

    pub async fn delete_block(&self, id: BlockId) -> Result<bool, DbError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM site_block_attachment WHERE block_id = ?")
            .bind(bind_id(id))
            .execute(&mut *tx)
            .await?;
        let done = sqlx::query("DELETE FROM site_page_block WHERE id = ?")
            .bind(bind_id(id))
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(DB::rows_affected(&done) > 0)
    }

    /// Attach an item set to a site. Attaching twice replaces the position.
    pub async fn add_item_set(
        &self,
        site_id: SiteId,
        item_set_id: ItemSetId,
        position: Option<i64>,
    ) -> Result<(), DbError> {
        sqlx::query(DB::upsert_site_item_set())
            .bind(bind_id(site_id))
            .bind(bind_id(item_set_id))
            .bind(position)
            .execute(self.pool)
            .await?;
        Ok(())
    }

    pub async fn remove_item_set(
        &self,
        site_id: SiteId,
        item_set_id: ItemSetId,
    ) -> Result<bool, DbError> {
        let done = sqlx::query("DELETE FROM site_item_set WHERE site_id = ? AND item_set_id = ?")
            .bind(bind_id(site_id))
            .bind(bind_id(item_set_id))
            .execute(self.pool)
            .await?;
        Ok(DB::rows_affected(&done) > 0)
    }

    /// The site's item sets, ordered as upstream orders them (`@OrderBy({"position" = "ASC"})`).
    /// MySQL sorts NULL first ascending, so an unpositioned row leads.
    pub async fn list_item_sets(
        &self,
        site_id: SiteId,
    ) -> Result<Vec<crate::rows::SiteItemSetRow>, DbError> {
        let rows = sqlx::query(
            "SELECT * FROM site_item_set WHERE site_id = ?
             ORDER BY position, id",
        )
        .bind(bind_id(site_id))
        .fetch_all(self.pool)
        .await?;
        rows.iter()
            .map(crate::rows::SiteItemSetRow::from_db_row::<DB>)
            .collect()
    }

    /// Grant or change a user's role on a site. The pair (site, user) is unique.
    pub async fn set_permission(
        &self,
        site_id: SiteId,
        user_id: UserId,
        role: &str,
    ) -> Result<(), DbError> {
        sqlx::query(DB::upsert_site_permission())
            .bind(bind_id(site_id))
            .bind(bind_id(user_id))
            .bind(role)
            .execute(self.pool)
            .await?;
        Ok(())
    }

    pub async fn remove_permission(
        &self,
        site_id: SiteId,
        user_id: UserId,
    ) -> Result<bool, DbError> {
        let done = sqlx::query("DELETE FROM site_permission WHERE site_id = ? AND user_id = ?")
            .bind(bind_id(site_id))
            .bind(bind_id(user_id))
            .execute(self.pool)
            .await?;
        Ok(DB::rows_affected(&done) > 0)
    }

    pub async fn list_permissions(
        &self,
        site_id: SiteId,
    ) -> Result<Vec<crate::rows::SitePermissionRow>, DbError> {
        let rows = sqlx::query("SELECT * FROM site_permission WHERE site_id = ? ORDER BY id")
            .bind(bind_id(site_id))
            .fetch_all(self.pool)
            .await?;
        rows.iter()
            .map(crate::rows::SitePermissionRow::from_db_row::<DB>)
            .collect()
    }

    /// Whether `user_id` holds any permission on `site_id`
    /// (`HasSitePermissionAssertion`: any site role, `admin` through `viewer`).
    pub async fn has_permission(&self, site_id: SiteId, user_id: UserId) -> Result<bool, DbError> {
        let found: i64 = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM site_permission WHERE site_id = ? AND user_id = ?",
        )
        .bind(bind_id(site_id))
        .bind(bind_id(user_id))
        .fetch_one(self.pool)
        .await?;
        Ok(found > 0)
    }
}
