//! The remaining upstream tables: settings, modules, jobs, assets, and the small join and index
//! tables (upstream `schema.sql`).

use crate::backend::Backend;
use chrono::NaiveDateTime;
use serde_json::Value;

use crate::error::DbError;
use crate::ids::{AssetId, ItemId, ItemSetId, JobId, ResourceId, SiteId, UserId};

/// One row of `setting`, a global setting as JSON.
#[derive(Debug, Clone, PartialEq)]
pub struct SettingRow {
    pub id: String,
    pub value: Value,
}

impl SettingRow {
    pub fn from_db_row<DB: Backend>(row: &DB::Row) -> Result<Self, DbError> {
        Ok(Self {
            id: DB::text(row, "id")?,
            value: DB::json(row, "value")?,
        })
    }
}

/// One row of `module`, the installed module list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleRow {
    pub id: String,
    pub is_active: bool,
    pub version: String,
}

impl ModuleRow {
    pub fn from_db_row<DB: Backend>(row: &DB::Row) -> Result<Self, DbError> {
        Ok(Self {
            id: DB::text(row, "id")?,
            is_active: DB::boolean(row, "is_active")?,
            version: DB::text(row, "version")?,
        })
    }
}

/// One row of `job`.
#[derive(Debug, Clone, PartialEq)]
pub struct JobRow {
    pub id: JobId,
    pub owner_id: Option<UserId>,
    pub pid: Option<String>,
    /// `starting`, `in_progress`, `completed`, `stopped` or `error`.
    pub status: Option<String>,
    pub class: String,
    pub args: Option<Value>,
    pub log: Option<String>,
    pub started: NaiveDateTime,
    pub ended: Option<NaiveDateTime>,
}

impl JobRow {
    pub fn from_db_row<DB: Backend>(row: &DB::Row) -> Result<Self, DbError> {
        Ok(Self {
            id: DB::id(row, "id")?,
            owner_id: DB::opt_id(row, "owner_id")?,
            pid: DB::opt_text(row, "pid")?,
            status: DB::opt_text(row, "status")?,
            class: DB::text(row, "class")?,
            args: DB::opt_json(row, "args")?,
            log: DB::opt_text(row, "log")?,
            started: DB::datetime(row, "started")?,
            ended: DB::opt_datetime(row, "ended")?,
        })
    }
}

/// Insert input for a `job`.
#[derive(Debug, Clone, PartialEq)]
pub struct NewJob {
    pub owner_id: Option<UserId>,
    pub pid: Option<String>,
    pub status: Option<String>,
    pub class: String,
    pub args: Option<Value>,
    pub log: Option<String>,
    /// `None` means now.
    pub started: Option<NaiveDateTime>,
}

/// One row of `asset`, an uploaded file that is not attached to an item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetRow {
    pub id: AssetId,
    pub owner_id: Option<UserId>,
    pub name: String,
    pub media_type: String,
    pub storage_id: String,
    pub extension: Option<String>,
    pub alt_text: Option<String>,
}

impl AssetRow {
    pub fn from_db_row<DB: Backend>(row: &DB::Row) -> Result<Self, DbError> {
        Ok(Self {
            id: DB::id(row, "id")?,
            owner_id: DB::opt_id(row, "owner_id")?,
            name: DB::text(row, "name")?,
            media_type: DB::text(row, "media_type")?,
            storage_id: DB::text(row, "storage_id")?,
            extension: DB::opt_text(row, "extension")?,
            alt_text: DB::opt_text(row, "alt_text")?,
        })
    }
}

/// Insert input for an `asset`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewAsset {
    pub owner_id: Option<UserId>,
    pub name: String,
    pub media_type: String,
    pub storage_id: String,
    pub extension: Option<String>,
    pub alt_text: Option<String>,
}

/// One row of `fulltext_search`, the search index.
///
/// The primary key is `(id, resource)`: `id` is the indexed resource's id and `resource` is its
/// resource name (`items`, `item_sets`, `media`), so one resource name holds one row per id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FulltextSearchRow {
    pub id: ResourceId,
    pub resource: String,
    pub owner_id: Option<UserId>,
    pub is_public: bool,
    pub title: Option<String>,
    pub text: Option<String>,
}

impl FulltextSearchRow {
    pub fn from_db_row<DB: Backend>(row: &DB::Row) -> Result<Self, DbError> {
        Ok(Self {
            id: DB::id(row, "id")?,
            resource: DB::text(row, "resource")?,
            owner_id: DB::opt_id(row, "owner_id")?,
            is_public: DB::boolean(row, "is_public")?,
            title: DB::opt_text(row, "title")?,
            text: DB::opt_text(row, "text")?,
        })
    }
}

/// One row of `item_item_set`, the item to item set membership.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemItemSetRow {
    pub item_id: ItemId,
    pub item_set_id: ItemSetId,
}

impl ItemItemSetRow {
    pub fn from_db_row<DB: Backend>(row: &DB::Row) -> Result<Self, DbError> {
        Ok(Self {
            item_id: DB::id(row, "item_id")?,
            item_set_id: DB::id(row, "item_set_id")?,
        })
    }
}

/// One row of `item_site`, the item attachment to a site.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemSiteRow {
    pub item_id: ItemId,
    pub site_id: SiteId,
}

impl ItemSiteRow {
    pub fn from_db_row<DB: Backend>(row: &DB::Row) -> Result<Self, DbError> {
        Ok(Self {
            item_id: DB::id(row, "item_id")?,
            site_id: DB::id(row, "site_id")?,
        })
    }
}

/// One row of `migration`, upstream's applied-migration ledger.
///
/// The port's own ledger is sqlx's `_sqlx_migrations`; this table stays for import and export.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationRow {
    pub version: String,
}

impl MigrationRow {
    pub fn from_db_row<DB: Backend>(row: &DB::Row) -> Result<Self, DbError> {
        Ok(Self {
            version: DB::text(row, "version")?,
        })
    }
}

/// One row of `session`, PHP session storage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionRow {
    pub id: String,
    pub data: Vec<u8>,
    /// Unix timestamp in seconds, as upstream stores it.
    pub modified: i64,
}

impl SessionRow {
    pub fn from_db_row<DB: Backend>(row: &DB::Row) -> Result<Self, DbError> {
        Ok(Self {
            id: DB::text(row, "id")?,
            data: DB::blob(row, "data")?,
            modified: DB::int(row, "modified")?,
        })
    }
}
