//! `romeka_import_upload`, `romeka_import_entry` and `romeka_import_scan`: what an import holds before it writes
//! (migration 0006). Upstream has neither.

use chrono::NaiveDateTime;
use serde_json::Value;

use crate::backend::Backend;
use crate::error::DbError;
use crate::ids::ImportId;

/// One tus upload into an import's staging area.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UploadRow {
    pub import_id: ImportId,
    /// The upload's id in its URL, unique within the import.
    pub id: String,
    /// Where the file lands, relative to the staging area.
    pub path: String,
    /// The `Upload-Length` the client declared.
    pub length: u64,
    /// The bytes stored so far: the next `PATCH` must start here.
    pub received: u64,
    pub created: NaiveDateTime,
}

impl UploadRow {
    pub fn from_db_row<DB: Backend>(row: &DB::Row) -> Result<Self, DbError> {
        Ok(Self {
            import_id: DB::id(row, "import_id")?,
            id: DB::text(row, "id")?,
            path: DB::text(row, "path")?,
            length: DB::id(row, "length")?,
            received: DB::id(row, "received")?,
            created: DB::datetime(row, "created")?,
        })
    }

    /// Every declared byte has arrived.
    pub fn is_complete(&self) -> bool {
        self.received == self.length
    }
}

/// Insert input for an upload. It starts with nothing received.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewUpload {
    pub import_id: ImportId,
    pub id: String,
    pub path: String,
    pub length: u64,
}

/// One file of an import's evidence index, as the last scan found it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryRow {
    pub import_id: ImportId,
    /// The source's id in the plan.
    pub source: String,
    /// Relative to the source, NFC.
    pub path: String,
    pub size: u64,
    pub modified: NaiveDateTime,
    /// The type sniffed from the bytes; `None` when nothing matched.
    pub media_type: Option<String>,
    pub flags: Value,
}

impl EntryRow {
    pub fn from_db_row<DB: Backend>(row: &DB::Row) -> Result<Self, DbError> {
        Ok(Self {
            import_id: DB::id(row, "import_id")?,
            source: DB::text(row, "source")?,
            path: DB::text(row, "path")?,
            size: DB::id(row, "size")?,
            modified: DB::datetime(row, "modified")?,
            media_type: DB::opt_text(row, "media_type")?,
            flags: DB::json(row, "flags")?,
        })
    }
}

/// One entry a scan found, for [`EntryRepo::replace`](crate::repos::EntryRepo::replace).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewEntry {
    pub source: String,
    pub path: String,
    pub size: u64,
    pub modified: NaiveDateTime,
    pub media_type: Option<String>,
    pub flags: Value,
}

/// What a scan left of one source besides its files (`romeka_import_scan`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanRow {
    pub import_id: ImportId,
    pub source: String,
    /// What the walk skipped; the shape belongs to `omeka-api`.
    pub anomalies: Value,
    /// Why the source could not be read.
    pub error: Option<String>,
    /// Differs between two scans, so a reader can tell a rescan from the scan it holds.
    pub token: String,
    pub scanned: NaiveDateTime,
}

impl ScanRow {
    pub fn from_db_row<DB: Backend>(row: &DB::Row) -> Result<Self, DbError> {
        Ok(Self {
            import_id: DB::id(row, "import_id")?,
            source: DB::text(row, "source")?,
            anomalies: DB::json(row, "anomalies")?,
            error: DB::opt_text(row, "error")?,
            token: DB::text(row, "token")?,
            scanned: DB::datetime(row, "scanned")?,
        })
    }
}

/// One source's scan record, for [`EntryRepo::replace`](crate::repos::EntryRepo::replace).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewScan {
    pub source: String,
    pub anomalies: Value,
    pub error: Option<String>,
    pub token: String,
    pub scanned: NaiveDateTime,
}
