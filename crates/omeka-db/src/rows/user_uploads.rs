//! `romeka_user_upload`: a user's staged media uploads (migration 0009). Upstream has no such
//! table; a media is created from the multipart body in one request.
//!
//! The bytes live in the store under `uploads/<user_id>/`, keyed by `id`; `path` is the name the
//! client gave and never a store path.

use chrono::NaiveDateTime;

use crate::backend::Backend;
use crate::error::DbError;
use crate::ids::{ItemId, UserId};

/// One row of `romeka_user_upload`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserUploadRow {
    /// The person the upload belongs to; another person's id is not theirs to reach.
    pub user_id: UserId,
    pub id: String,
    /// The item the upload was started for, `None` when it names none or the item is gone.
    pub item_id: Option<ItemId>,
    /// The client's file name, as `Upload-Metadata filename` gave it.
    pub path: String,
    /// The type `Upload-Metadata filetype` gave, if it gave one.
    pub media_type: Option<String>,
    /// When a media create took the upload, `None` until one does. A claim older
    /// than the stale bound is reclaimable.
    pub claimed: Option<NaiveDateTime>,
    /// The `Upload-Length` the client declared.
    pub length: u64,
    /// The bytes stored so far: the next `PATCH` must start here.
    pub received: u64,
    pub created: NaiveDateTime,
    /// Twenty-four hours after the last byte arrived; the sweep removes the row past it.
    pub expires: NaiveDateTime,
}

impl UserUploadRow {
    pub fn from_db_row<DB: Backend>(row: &DB::Row) -> Result<Self, DbError> {
        Ok(Self {
            user_id: DB::id(row, "user_id")?,
            id: DB::text(row, "id")?,
            item_id: DB::opt_id(row, "item_id")?,
            path: DB::text(row, "path")?,
            media_type: DB::opt_text(row, "media_type")?,
            claimed: DB::opt_datetime(row, "claimed")?,
            length: DB::id(row, "length")?,
            received: DB::id(row, "received")?,
            created: DB::datetime(row, "created")?,
            expires: DB::datetime(row, "expires")?,
        })
    }

    /// Every declared byte has arrived.
    pub fn is_complete(&self) -> bool {
        self.received == self.length
    }
}

/// Insert input for a staged upload. It starts with nothing received and no claim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewUserUpload {
    pub user_id: UserId,
    pub id: String,
    pub item_id: Option<ItemId>,
    pub path: String,
    pub media_type: Option<String>,
    pub length: u64,
    pub expires: NaiveDateTime,
}
