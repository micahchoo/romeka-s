//! `site` and its child tables (upstream `schema.sql` `site`, `site_page`, `site_page_block`,
//! `site_block_attachment`, `site_item_set`, `site_permission`, `site_setting`).

use crate::backend::Backend;
use chrono::NaiveDateTime;
use serde_json::Value;

use crate::error::DbError;
use crate::ids::{
    AssetId, AttachmentId, BlockId, ItemId, ItemSetId, MediaId, PageId, SiteId, SiteItemSetId,
    SitePermissionId, UserId,
};

/// One row of `site`.
#[derive(Debug, Clone, PartialEq)]
pub struct SiteRow {
    pub id: SiteId,
    pub thumbnail_id: Option<AssetId>,
    pub homepage_id: Option<PageId>,
    pub owner_id: Option<UserId>,
    pub slug: String,
    pub theme: String,
    pub title: String,
    pub summary: Option<String>,
    /// Upstream `json_array`, the site navigation tree.
    pub navigation: Value,
    /// Upstream `json_array`, the site item pool query.
    pub item_pool: Value,
    pub created: NaiveDateTime,
    pub modified: Option<NaiveDateTime>,
    pub is_public: bool,
    pub assign_new_items: bool,
}

impl SiteRow {
    pub fn from_db_row<DB: Backend>(row: &DB::Row) -> Result<Self, DbError> {
        Ok(Self {
            id: DB::id(row, "id")?,
            thumbnail_id: DB::opt_id(row, "thumbnail_id")?,
            homepage_id: DB::opt_id(row, "homepage_id")?,
            owner_id: DB::opt_id(row, "owner_id")?,
            slug: DB::text(row, "slug")?,
            theme: DB::text(row, "theme")?,
            title: DB::text(row, "title")?,
            summary: DB::opt_text(row, "summary")?,
            navigation: DB::json(row, "navigation")?,
            item_pool: DB::json(row, "item_pool")?,
            created: DB::datetime(row, "created")?,
            modified: DB::opt_datetime(row, "modified")?,
            is_public: DB::boolean(row, "is_public")?,
            assign_new_items: DB::boolean(row, "assign_new_items")?,
        })
    }
}

/// Insert input for a `site`.
#[derive(Debug, Clone, PartialEq)]
pub struct NewSite {
    pub thumbnail_id: Option<AssetId>,
    pub homepage_id: Option<PageId>,
    pub owner_id: Option<UserId>,
    pub slug: String,
    pub theme: String,
    pub title: String,
    pub summary: Option<String>,
    pub navigation: Value,
    pub item_pool: Value,
    pub is_public: bool,
    pub assign_new_items: bool,
    /// `None` means now.
    pub created: Option<NaiveDateTime>,
}

impl Default for NewSite {
    /// `Site` entity defaults: public, `assign_new_items` false, empty navigation and item pool.
    fn default() -> Self {
        Self {
            thumbnail_id: None,
            homepage_id: None,
            owner_id: None,
            slug: String::new(),
            theme: String::new(),
            title: String::new(),
            summary: None,
            navigation: serde_json::json!([]),
            item_pool: serde_json::json!([]),
            is_public: true,
            assign_new_items: false,
            created: None,
        }
    }
}

/// One row of `site_page`.
#[derive(Debug, Clone, PartialEq)]
pub struct SitePageRow {
    pub id: PageId,
    pub site_id: SiteId,
    pub slug: String,
    pub title: String,
    pub is_public: bool,
    pub layout: Option<String>,
    pub layout_data: Option<Value>,
    pub created: NaiveDateTime,
    pub modified: Option<NaiveDateTime>,
}

impl SitePageRow {
    pub fn from_db_row<DB: Backend>(row: &DB::Row) -> Result<Self, DbError> {
        Ok(Self {
            id: DB::id(row, "id")?,
            site_id: DB::id(row, "site_id")?,
            slug: DB::text(row, "slug")?,
            title: DB::text(row, "title")?,
            is_public: DB::boolean(row, "is_public")?,
            layout: DB::opt_text(row, "layout")?,
            layout_data: DB::opt_json(row, "layout_data")?,
            created: DB::datetime(row, "created")?,
            modified: DB::opt_datetime(row, "modified")?,
        })
    }
}

/// Insert input for a `site_page`.
#[derive(Debug, Clone, PartialEq)]
pub struct NewSitePage {
    pub site_id: SiteId,
    pub slug: String,
    pub title: String,
    pub is_public: bool,
    pub layout: Option<String>,
    pub layout_data: Option<Value>,
    /// `None` means now.
    pub created: Option<NaiveDateTime>,
}

impl Default for NewSitePage {
    fn default() -> Self {
        Self {
            site_id: SiteId(0),
            slug: String::new(),
            title: String::new(),
            is_public: false,
            layout: None,
            layout_data: None,
            created: None,
        }
    }
}

/// One row of `site_page_block`.
#[derive(Debug, Clone, PartialEq)]
pub struct SiteBlockRow {
    pub id: BlockId,
    pub page_id: PageId,
    pub layout: String,
    pub data: Value,
    pub layout_data: Option<Value>,
    pub position: i64,
}

impl SiteBlockRow {
    pub fn from_db_row<DB: Backend>(row: &DB::Row) -> Result<Self, DbError> {
        Ok(Self {
            id: DB::id(row, "id")?,
            page_id: DB::id(row, "page_id")?,
            layout: DB::text(row, "layout")?,
            data: DB::json(row, "data")?,
            layout_data: DB::opt_json(row, "layout_data")?,
            position: DB::int(row, "position")?,
        })
    }
}

/// Insert input for a `site_page_block`.
#[derive(Debug, Clone, PartialEq)]
pub struct NewSiteBlock {
    pub page_id: PageId,
    pub layout: String,
    pub data: Value,
    pub layout_data: Option<Value>,
    pub position: i64,
}

/// One row of `site_block_attachment`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SiteBlockAttachmentRow {
    pub id: AttachmentId,
    pub block_id: BlockId,
    pub item_id: Option<ItemId>,
    pub media_id: Option<MediaId>,
    pub caption: String,
    pub position: i64,
}

impl SiteBlockAttachmentRow {
    pub fn from_db_row<DB: Backend>(row: &DB::Row) -> Result<Self, DbError> {
        Ok(Self {
            id: DB::id(row, "id")?,
            block_id: DB::id(row, "block_id")?,
            item_id: DB::opt_id(row, "item_id")?,
            media_id: DB::opt_id(row, "media_id")?,
            caption: DB::text(row, "caption")?,
            position: DB::int(row, "position")?,
        })
    }
}

/// Insert input for a `site_block_attachment`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewSiteBlockAttachment {
    pub block_id: BlockId,
    pub item_id: Option<ItemId>,
    pub media_id: Option<MediaId>,
    pub caption: String,
    pub position: i64,
}

/// One row of `site_item_set`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SiteItemSetRow {
    pub id: SiteItemSetId,
    pub site_id: SiteId,
    pub item_set_id: ItemSetId,
    pub position: Option<i64>,
}

impl SiteItemSetRow {
    pub fn from_db_row<DB: Backend>(row: &DB::Row) -> Result<Self, DbError> {
        Ok(Self {
            id: DB::id(row, "id")?,
            site_id: DB::id(row, "site_id")?,
            item_set_id: DB::id(row, "item_set_id")?,
            position: DB::opt_int(row, "position")?,
        })
    }
}

/// One row of `site_permission`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SitePermissionRow {
    pub id: SitePermissionId,
    pub site_id: SiteId,
    pub user_id: UserId,
    pub role: String,
}

impl SitePermissionRow {
    pub fn from_db_row<DB: Backend>(row: &DB::Row) -> Result<Self, DbError> {
        Ok(Self {
            id: DB::id(row, "id")?,
            site_id: DB::id(row, "site_id")?,
            user_id: DB::id(row, "user_id")?,
            role: DB::text(row, "role")?,
        })
    }
}

/// One row of `site_setting`.
#[derive(Debug, Clone, PartialEq)]
pub struct SiteSettingRow {
    pub id: String,
    pub site_id: SiteId,
    pub value: Value,
}

impl SiteSettingRow {
    pub fn from_db_row<DB: Backend>(row: &DB::Row) -> Result<Self, DbError> {
        Ok(Self {
            id: DB::text(row, "id")?,
            site_id: DB::id(row, "site_id")?,
            value: DB::json(row, "value")?,
        })
    }
}
