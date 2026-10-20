//! `romeka_import` and `romeka_import_resource`: the archive importer's own tables (migration
//! 0005). Upstream has neither.
//!
//! `plan` and `summary` are JSON. The row keeps them as JSON; `omeka-api` owns what they mean, so
//! this crate stays the storage boundary and nothing else.

use chrono::NaiveDateTime;
use serde_json::Value;

use crate::backend::Backend;
use crate::error::DbError;
use crate::ids::{ImportId, ItemId, ItemSetId, JobId, MediaId, PropertyId, TemplateId, UserId};

/// Where an import is in its life. `draft` is editable; the rest belong to the runner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportStatus {
    Draft,
    Running,
    Done,
    Failed,
    Undoing,
    Undone,
}

impl ImportStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Running => "running",
            Self::Done => "done",
            Self::Failed => "failed",
            Self::Undoing => "undoing",
            Self::Undone => "undone",
        }
    }

    pub(crate) fn parse(stored: &str) -> Result<Self, DbError> {
        match stored {
            "draft" => Ok(Self::Draft),
            "running" => Ok(Self::Running),
            "done" => Ok(Self::Done),
            "failed" => Ok(Self::Failed),
            "undoing" => Ok(Self::Undoing),
            "undone" => Ok(Self::Undone),
            other => Err(DbError::Column {
                column: "status",
                reason: format!("`{other}` is not an import status"),
            }),
        }
    }
}

/// The table a created thing lives in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportKind {
    Item,
    Media,
    ItemSet,
    ResourceTemplate,
    Property,
}

impl ImportKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Item => "item",
            Self::Media => "media",
            Self::ItemSet => "item_set",
            Self::ResourceTemplate => "resource_template",
            Self::Property => "property",
        }
    }
}

/// One thing an import created, with the id type of its table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportTarget {
    Item(ItemId),
    Media(MediaId),
    ItemSet(ItemSetId),
    ResourceTemplate(TemplateId),
    Property(PropertyId),
}

impl ImportTarget {
    pub fn kind(self) -> ImportKind {
        match self {
            Self::Item(_) => ImportKind::Item,
            Self::Media(_) => ImportKind::Media,
            Self::ItemSet(_) => ImportKind::ItemSet,
            Self::ResourceTemplate(_) => ImportKind::ResourceTemplate,
            Self::Property(_) => ImportKind::Property,
        }
    }

    pub(crate) fn raw_id(self) -> i64 {
        use crate::repos::bind_id;
        match self {
            Self::Item(id) => bind_id(id),
            Self::Media(id) => bind_id(id),
            Self::ItemSet(id) => bind_id(id),
            Self::ResourceTemplate(id) => bind_id(id),
            Self::Property(id) => bind_id(id),
        }
    }

    fn parse(kind: &str, raw: u64) -> Result<Self, DbError> {
        match kind {
            "item" => Ok(Self::Item(ItemId(raw))),
            "media" => Ok(Self::Media(MediaId(raw))),
            "item_set" => Ok(Self::ItemSet(ItemSetId(raw))),
            "resource_template" => Ok(Self::ResourceTemplate(TemplateId(raw))),
            "property" => Ok(Self::Property(PropertyId(raw))),
            other => Err(DbError::Column {
                column: "kind",
                reason: format!("`{other}` is not an import resource kind"),
            }),
        }
    }
}

/// One row of `romeka_import`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportRow {
    pub id: ImportId,
    /// `None` once the owner's account is deleted.
    pub owner_id: Option<UserId>,
    /// The folder, relative to the archive root.
    pub folder: String,
    pub plan: Value,
    pub status: ImportStatus,
    /// True for the one import whose undo window is open.
    pub undoable: bool,
    pub job_id: Option<JobId>,
    pub summary: Option<Value>,
    pub created: NaiveDateTime,
    pub finished: Option<NaiveDateTime>,
}

impl ImportRow {
    pub fn from_db_row<DB: Backend>(row: &DB::Row) -> Result<Self, DbError> {
        Ok(Self {
            id: DB::id(row, "id")?,
            owner_id: DB::opt_id(row, "owner_id")?,
            folder: DB::text(row, "folder")?,
            plan: DB::json(row, "plan")?,
            status: ImportStatus::parse(&DB::text(row, "status")?)?,
            undoable: DB::boolean(row, "undoable")?,
            job_id: DB::opt_id(row, "job_id")?,
            summary: DB::opt_json(row, "summary")?,
            created: DB::datetime(row, "created")?,
            finished: DB::opt_datetime(row, "finished")?,
        })
    }
}

/// One row of `romeka_import_resource`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImportResourceRow {
    pub import_id: ImportId,
    pub target: ImportTarget,
}

impl ImportResourceRow {
    pub fn from_db_row<DB: Backend>(row: &DB::Row) -> Result<Self, DbError> {
        let kind = DB::text(row, "kind")?;
        let raw: u64 = DB::id(row, "target_id")?;
        Ok(Self {
            import_id: DB::id(row, "import_id")?,
            target: ImportTarget::parse(&kind, raw)?,
        })
    }
}

/// Insert input for a draft.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewImport {
    pub owner_id: Option<UserId>,
    pub folder: String,
    pub plan: Value,
}
