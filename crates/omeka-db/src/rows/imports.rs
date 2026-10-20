//! `romeka_import`, its ledger and its before-images: import v2's own tables (migration 0006).
//! Upstream has none of them.
//!
//! `sources`, `plan`, `ran_plan`, `summary` and the before-image are JSON. The rows keep them as
//! JSON; `omeka-api` owns what they mean, so this crate stays the storage boundary and nothing
//! else.

use chrono::NaiveDateTime;
use serde_json::Value;

use crate::backend::Backend;
use crate::error::DbError;
use crate::ids::{ImportId, ItemId, ItemSetId, JobId, MediaId, PropertyId, TemplateId, UserId};

/// Where an import is in its life (`docs/import-v2.md`, "Lifecycle and the ledger"). `draft` is
/// editable; the rest belong to the runner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportStatus {
    Draft,
    Running,
    Done,
    RunFailed,
    Undoing,
    Undone,
    UndoFailed,
}

impl ImportStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Running => "running",
            Self::Done => "done",
            Self::RunFailed => "run_failed",
            Self::Undoing => "undoing",
            Self::Undone => "undone",
            Self::UndoFailed => "undo_failed",
        }
    }

    pub(crate) fn parse(stored: &str) -> Result<Self, DbError> {
        match stored {
            "draft" => Ok(Self::Draft),
            "running" => Ok(Self::Running),
            "done" => Ok(Self::Done),
            "run_failed" => Ok(Self::RunFailed),
            "undoing" => Ok(Self::Undoing),
            "undone" => Ok(Self::Undone),
            "undo_failed" => Ok(Self::UndoFailed),
            other => Err(DbError::Column {
                column: "status",
                reason: format!("`{other}` is not an import status"),
            }),
        }
    }
}

/// How a run or an undo ends. Each ending names the one state it ends, so a run cannot end as
/// undone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportEnding {
    Done,
    RunFailed,
    Undone,
    UndoFailed,
}

impl ImportEnding {
    /// The state this ending leaves.
    pub fn status(self) -> ImportStatus {
        match self {
            Self::Done => ImportStatus::Done,
            Self::RunFailed => ImportStatus::RunFailed,
            Self::Undone => ImportStatus::Undone,
            Self::UndoFailed => ImportStatus::UndoFailed,
        }
    }

    /// The state this ending ends.
    pub fn ends(self) -> ImportStatus {
        match self {
            Self::Done | Self::RunFailed => ImportStatus::Running,
            Self::Undone | Self::UndoFailed => ImportStatus::Undoing,
        }
    }
}

/// The table a recorded resource lives in.
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

/// One resource an import wrote, with the id type of its table.
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

    /// The `kind` and `target_id` columns, which the ledger and the before-images share.
    fn from_db_row<DB: Backend>(row: &DB::Row) -> Result<Self, DbError> {
        let raw: u64 = DB::id(row, "target_id")?;
        match DB::text(row, "kind")?.as_str() {
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

/// What an import did to a recorded resource.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportChange {
    /// The import made it; undo deletes it.
    Created,
    /// It existed; undo restores it from its before-image.
    Changed,
}

impl ImportChange {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::Changed => "changed",
        }
    }

    fn parse(stored: &str) -> Result<Self, DbError> {
        match stored {
            "created" => Ok(Self::Created),
            "changed" => Ok(Self::Changed),
            other => Err(DbError::Column {
                column: "change_kind",
                reason: format!("`{other}` is not an import change"),
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
    pub sources: Value,
    pub plan: Value,
    /// The plan as the first run started it; a retry runs this one.
    pub ran_plan: Option<Value>,
    /// How many times the plan was saved; a save names the revision it edited.
    pub revision: u64,
    pub status: ImportStatus,
    pub job_id: Option<JobId>,
    pub summary: Option<Value>,
    /// The import this draft was copied from, `None` once that import is deleted.
    pub copied_from: Option<ImportId>,
    pub created: NaiveDateTime,
    pub finished: Option<NaiveDateTime>,
}

impl ImportRow {
    pub fn from_db_row<DB: Backend>(row: &DB::Row) -> Result<Self, DbError> {
        Ok(Self {
            id: DB::id(row, "id")?,
            owner_id: DB::opt_id(row, "owner_id")?,
            sources: DB::json(row, "sources")?,
            plan: DB::json(row, "plan")?,
            ran_plan: DB::opt_json(row, "ran_plan")?,
            revision: DB::id(row, "revision")?,
            status: ImportStatus::parse(&DB::text(row, "status")?)?,
            job_id: DB::opt_id(row, "job_id")?,
            summary: DB::opt_json(row, "summary")?,
            copied_from: DB::opt_id(row, "copied_from")?,
            created: DB::datetime(row, "created")?,
            finished: DB::opt_datetime(row, "finished")?,
        })
    }
}

/// Insert input for a draft.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewImport {
    pub owner_id: Option<UserId>,
    pub sources: Value,
    pub plan: Value,
    pub copied_from: Option<ImportId>,
}

/// One row of `romeka_import_resource`: the ledger.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportResourceRow {
    pub import_id: ImportId,
    pub target: ImportTarget,
    /// The key of the record that wrote the target, so a retry skips it.
    pub record: String,
    pub change: ImportChange,
    /// When the import wrote it; a later `modified` means something else touched it.
    pub written: NaiveDateTime,
}

impl ImportResourceRow {
    pub fn from_db_row<DB: Backend>(row: &DB::Row) -> Result<Self, DbError> {
        Ok(Self {
            import_id: DB::id(row, "import_id")?,
            target: ImportTarget::from_db_row::<DB>(row)?,
            record: DB::text(row, "record")?,
            change: ImportChange::parse(&DB::text(row, "change_kind")?)?,
            written: DB::datetime(row, "written")?,
        })
    }
}

/// Insert input for a ledger row. The repository stamps `written`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewImportResource {
    pub target: ImportTarget,
    pub record: String,
    pub change: ImportChange,
}

/// One row of `romeka_import_before`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BeforeImageRow {
    pub import_id: ImportId,
    pub target: ImportTarget,
    /// Values, template, class, owner, visibility, item sets, sites and media order.
    pub image: Value,
}

impl BeforeImageRow {
    pub fn from_db_row<DB: Backend>(row: &DB::Row) -> Result<Self, DbError> {
        Ok(Self {
            import_id: DB::id(row, "import_id")?,
            target: ImportTarget::from_db_row::<DB>(row)?,
            image: DB::json(row, "image")?,
        })
    }
}
