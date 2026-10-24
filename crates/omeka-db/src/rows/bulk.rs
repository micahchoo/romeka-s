//! `romeka_bulk` and `romeka_bulk_target`: a bulk change to items and the items it touches
//! (migration 0012). Upstream has neither.
//!
//! `query` and `change` are JSON that `omeka-api` owns; this crate stores them and parses the
//! states. The life of a run is [`BulkStatus`], the life of one item in it [`TargetState`].

use std::collections::BTreeSet;

use chrono::NaiveDateTime;
use serde_json::Value;

use crate::backend::Backend;
use crate::error::DbError;
use crate::ids::{BulkId, ClassId, ItemId, ItemSetId, JobId, SiteId, TemplateId, UserId};
use crate::rows::{BeforeImage, ImageMemberships};

/// A closed set of names stored in a text column: `as_str` writes it, `parse` reads it back.
macro_rules! stored_names {
    ($(#[$doc:meta])* $name:ident, $column:literal, { $($(#[$vdoc:meta])* $variant:ident => $text:literal,)+ }) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub enum $name {
            $($(#[$vdoc])* $variant,)+
        }

        impl $name {
            pub const ALL: &'static [Self] = &[$(Self::$variant,)+];

            pub fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $text,)+
                }
            }

            pub fn parse(stored: &str) -> Result<Self, DbError> {
                match stored {
                    $($text => Ok(Self::$variant),)+
                    other => Err(DbError::Column {
                        column: $column,
                        reason: format!("`{other}` is not a {}", stringify!($name)),
                    }),
                }
            }
        }
    };
}

stored_names!(
    /// What a run does to its items. Delete joins in a later phase, with no new column.
    BulkKind, "kind", {
        Edit => "edit",
    }
);

stored_names!(
    /// Where a bulk run is in its life.
    ///
    /// ```text
    /// previewed -> running -> done | failed
    ///              running -> stopping -> stopped
    /// stopped | failed -> running                       (a new Run click goes on)
    /// done | stopped | failed | partly_undone -> undoing -> undone | partly_undone
    ///              undoing -> undo_stopping -> partly_undone
    /// ```
    BulkStatus, "status", {
        Previewed => "previewed",
        Running => "running",
        /// A stop was asked for; the job ends the run after its chunk.
        Stopping => "stopping",
        Stopped => "stopped",
        Done => "done",
        Failed => "failed",
        Undoing => "undoing",
        /// A stop was asked for during an undo.
        UndoStopping => "undo_stopping",
        Undone => "undone",
        /// The undo ended with items still changed: kept, stopped, or failed. Undo again goes on.
        PartlyUndone => "partly_undone",
    }
);

impl BulkStatus {
    /// Whether Run may start from here: a fresh preview, or a run that ended before its end.
    pub fn may_run(self) -> bool {
        matches!(self, Self::Previewed | Self::Stopped | Self::Failed)
    }

    /// Whether Undo may start from here: a run that wrote, or an undo that left items changed.
    pub fn may_undo(self) -> bool {
        matches!(
            self,
            Self::Done | Self::Stopped | Self::Failed | Self::PartlyUndone
        )
    }

    /// The states a job works in.
    pub const LIVE: [Self; 4] = [
        Self::Running,
        Self::Stopping,
        Self::Undoing,
        Self::UndoStopping,
    ];

    pub fn is_live(self) -> bool {
        Self::LIVE.contains(&self)
    }

    /// The state a stop request moves a live state to.
    pub fn stopping(self) -> Option<Self> {
        match self {
            Self::Running => Some(Self::Stopping),
            Self::Undoing => Some(Self::UndoStopping),
            _ => None,
        }
    }

    /// Whether the job was asked to stop.
    pub fn is_stopping(self) -> bool {
        matches!(self, Self::Stopping | Self::UndoStopping)
    }

    /// The state a dead worker leaves: a run is stopped, an undo is partly undone. Either one
    /// goes on with a new click.
    pub fn when_dead(self) -> Option<Self> {
        match self {
            Self::Running | Self::Stopping => Some(Self::Stopped),
            Self::Undoing | Self::UndoStopping => Some(Self::PartlyUndone),
            _ => None,
        }
    }
}

/// The work a job does on a run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BulkPhase {
    Run,
    Undo,
}

impl BulkPhase {
    /// The state a start moves the run to.
    pub fn working(self) -> BulkStatus {
        match self {
            Self::Run => BulkStatus::Running,
            Self::Undo => BulkStatus::Undoing,
        }
    }

    /// The states this phase works in, the stop request's among them.
    pub fn live(self) -> [BulkStatus; 2] {
        match self {
            Self::Run => [BulkStatus::Running, BulkStatus::Stopping],
            Self::Undo => [BulkStatus::Undoing, BulkStatus::UndoStopping],
        }
    }

    /// The target states this phase still has work on.
    pub fn open_targets(self) -> &'static [TargetState] {
        match self {
            Self::Run => &[TargetState::Pending],
            Self::Undo => &[TargetState::Done, TargetState::Kept],
        }
    }
}

/// How a job ends a run. Each ending names the one phase it ends, so a run cannot end undone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BulkEnding {
    Done,
    Stopped,
    Failed,
    Undone,
    PartlyUndone,
}

impl BulkEnding {
    pub fn status(self) -> BulkStatus {
        match self {
            Self::Done => BulkStatus::Done,
            Self::Stopped => BulkStatus::Stopped,
            Self::Failed => BulkStatus::Failed,
            Self::Undone => BulkStatus::Undone,
            Self::PartlyUndone => BulkStatus::PartlyUndone,
        }
    }

    pub fn phase(self) -> BulkPhase {
        match self {
            Self::Done | Self::Stopped | Self::Failed => BulkPhase::Run,
            Self::Undone | Self::PartlyUndone => BulkPhase::Undo,
        }
    }
}

stored_names!(
    /// Where one item of a run is.
    ///
    /// ```text
    /// pending -> done | unchanged | failed      (the run)
    /// done | kept -> restored | kept | failed   (an undo; kept until forced)
    /// ```
    TargetState, "state", {
        Pending => "pending",
        /// Changed; `image` and `written` are set.
        Done => "done",
        /// Already in the state the change asks for, so nothing was written.
        Unchanged => "unchanged",
        Failed => "failed",
        Restored => "restored",
        /// Edited after the run wrote it, so undo left it; a forced undo restores it.
        Kept => "kept",
    }
);

stored_names!(
    /// Why a target failed, as the admin reads it: a code, not a sentence. A failed target holds
    /// it, and so does a target an undo refused, which stays `done` or `kept` for a later undo.
    FailReason, "error", {
        /// The item is gone.
        Gone => "gone",
        /// The run's owner may not update this item.
        Denied => "denied",
        /// The item type the change sets requires a value the item lacks.
        TemplateRequires => "templateRequires",
        /// The person who asked for the undo may not update this item, as it is now or as the
        /// undo would leave it.
        UndoDenied => "undoDenied",
    }
);

/// One row of `romeka_bulk`.
#[derive(Debug, Clone, PartialEq)]
pub struct BulkRow {
    pub id: BulkId,
    /// `None` once the owner's account is deleted.
    pub owner_id: Option<UserId>,
    pub kind: BulkKind,
    pub query: Value,
    pub change: Value,
    pub status: BulkStatus,
    /// The targets frozen at preview.
    pub total: u64,
    pub done: u64,
    pub failed: u64,
    pub job_id: Option<JobId>,
    /// Why the last job failed, when it failed as a whole.
    pub error: Option<String>,
    pub created: NaiveDateTime,
    pub started: Option<NaiveDateTime>,
    pub ended: Option<NaiveDateTime>,
    pub heartbeat: Option<NaiveDateTime>,
}

impl BulkRow {
    pub fn from_db_row<DB: Backend>(row: &DB::Row) -> Result<Self, DbError> {
        Ok(Self {
            id: DB::id(row, "id")?,
            owner_id: DB::opt_id(row, "owner_id")?,
            kind: BulkKind::parse(&DB::text(row, "kind")?)?,
            query: DB::json(row, "query")?,
            change: DB::json(row, "change_json")?,
            status: BulkStatus::parse(&DB::text(row, "status")?)?,
            total: DB::id(row, "total")?,
            done: DB::id(row, "done")?,
            failed: DB::id(row, "failed")?,
            job_id: DB::opt_id(row, "job_id")?,
            error: DB::opt_text(row, "error")?,
            created: DB::datetime(row, "created")?,
            started: DB::opt_datetime(row, "started")?,
            ended: DB::opt_datetime(row, "ended")?,
            heartbeat: DB::opt_datetime(row, "heartbeat")?,
        })
    }
}

/// A run as preview writes it, with its frozen selection.
#[derive(Debug, Clone, PartialEq)]
pub struct NewBulk {
    pub owner_id: Option<UserId>,
    pub kind: BulkKind,
    pub query: Value,
    pub change: Value,
    pub items: Vec<ItemId>,
}

/// One row of `romeka_bulk_target`.
#[derive(Debug, Clone, PartialEq)]
pub struct BulkTargetRow {
    pub bulk_id: BulkId,
    pub item_id: ItemId,
    pub state: TargetState,
    pub image: Option<Value>,
    pub error: Option<FailReason>,
    /// The item's `modified` the run's change left.
    pub written: Option<NaiveDateTime>,
}

impl BulkTargetRow {
    pub fn from_db_row<DB: Backend>(row: &DB::Row) -> Result<Self, DbError> {
        Ok(Self {
            bulk_id: DB::id(row, "bulk_id")?,
            item_id: DB::id(row, "item_id")?,
            state: TargetState::parse(&DB::text(row, "state")?)?,
            image: DB::opt_json(row, "image")?,
            error: DB::opt_text(row, "error")?
                .map(|code| FailReason::parse(&code))
                .transpose()?,
            written: DB::opt_datetime(row, "written")?,
        })
    }
}

/// How many targets of a run are in each state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TargetCounts {
    pub pending: u64,
    pub done: u64,
    pub unchanged: u64,
    pub failed: u64,
    pub restored: u64,
    pub kept: u64,
}

impl TargetCounts {
    pub(crate) fn add(&mut self, state: TargetState, count: u64) {
        let slot = match state {
            TargetState::Pending => &mut self.pending,
            TargetState::Done => &mut self.done,
            TargetState::Unchanged => &mut self.unchanged,
            TargetState::Failed => &mut self.failed,
            TargetState::Restored => &mut self.restored,
            TargetState::Kept => &mut self.kept,
        };
        *slot += count;
    }
}

/// The fields of an item a describing edit reads and writes. Two equal values mean the edit
/// changes nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Described {
    pub is_public: bool,
    pub owner: Option<UserId>,
    pub class: Option<ClassId>,
    pub template: Option<TemplateId>,
    pub item_sets: BTreeSet<ItemSetId>,
    pub sites: BTreeSet<SiteId>,
}

impl Described {
    /// The fields an item's before-image holds; `None` for the image of another kind.
    pub fn of_image(image: &BeforeImage) -> Option<Self> {
        let ImageMemberships::Item {
            item_sets, sites, ..
        } = &image.memberships
        else {
            return None;
        };
        Some(Self {
            is_public: image.is_public,
            owner: image.owner,
            class: image.class,
            template: image.template,
            item_sets: item_sets.iter().copied().collect(),
            sites: sites.iter().copied().collect(),
        })
    }
}

/// What the caller decided for one item, from its before-image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// Write these fields.
    Write(Described),
    /// The item already holds what the change asks for.
    Unchanged,
    Refuse(FailReason),
}

/// What one target's turn in a run did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Applied {
    /// The target was not pending: an earlier run dealt with it.
    Settled,
    Written,
    Unchanged,
    Failed(FailReason),
}

/// What one target's turn in an undo did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reverted {
    /// The target was neither done nor kept.
    Settled,
    Restored,
    /// Edited since the run wrote it, and not forced.
    Kept,
    Gone,
    /// The caller refused to restore it; the target keeps its state and records the reason.
    Refused(FailReason),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_live_state_has_a_dead_end_and_no_other_does() {
        for &status in BulkStatus::ALL {
            assert_eq!(status.when_dead().is_some(), status.is_live(), "{status:?}");
            assert_eq!(BulkStatus::parse(status.as_str()).unwrap(), status);
        }
        assert!(BulkStatus::parse("deleted").is_err());
    }
}
