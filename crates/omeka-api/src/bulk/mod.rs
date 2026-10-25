//! Bulk changes to items (`briefs/items-bulk-edit-design.md`): one change applied to every item a
//! browse query matches, or to the items a person ticked, with an undo.
//!
//! A run's selection is frozen at preview ([`preview::preview`]): the query resolves through the
//! items list's own read ([`crate::read::matching_item_ids`]) with the caller's visibility, and
//! the run touches those ids and no others. [`start`] moves a run into its job; the job's body is
//! [`work::work`], one transaction per item (`BulkRepo::apply_target`), so a rerun after a crash
//! finds what is done and goes on. A change sets describing fields and memberships, and edits
//! values ([`values`]). Undo restores each item from the before-image its run kept, and keeps an
//! item someone edited since unless forced, as import undo does; an item whose sets or sites are
//! no longer those the run left counts as edited, because a site's item pool changes them without
//! a new `modified`.
//!
//! Rights are upstream's batch privileges (`AclFactory`): a query over everything that matches
//! needs `batch_update_all` (reviewer and up), a ticked list `batch_update` (author and up). Each
//! item then needs the update right the item `PATCH` checks; an item that fails it is recorded as
//! failed, not written.

mod access;
mod change;
pub mod preview;
mod scope;
mod start;
mod values;
mod view;
pub mod work;

use omeka_db::DbError;
use omeka_db::rows::BulkStatus;

pub use change::{Assign, BulkChange, BulkEdit, Membership};
pub use scope::Scope;
pub use start::{run, stop, undo};
pub use view::{BulkView, UndoPreview, list, show, undo_preview};

use crate::errors::ApiError;

/// The job class of a run; its args are `{"bulkId": n}`.
pub const RUN_JOB: &str = "Romeka\\Job\\BulkRun";

/// The job class of an undo; its args are `{"bulkId": n, "force": bool}`.
pub const UNDO_JOB: &str = "Romeka\\Job\\BulkUndo";

/// A refused or failed bulk request.
#[derive(Debug, thiserror::Error)]
pub enum BulkError {
    /// A 400, 403, 404 or 500 in the API's own error document.
    #[error(transparent)]
    Api(#[from] ApiError),
    /// A 409: the run's state does not allow the request. `code` is what the admin branches on.
    #[error("{message}")]
    Conflict {
        code: &'static str,
        message: &'static str,
        status: BulkStatus,
    },
}

impl From<DbError> for BulkError {
    fn from(error: DbError) -> Self {
        Self::Api(crate::read::db_error(error))
    }
}

impl BulkError {
    fn conflict(code: &'static str, message: &'static str, status: BulkStatus) -> Self {
        Self::Conflict {
            code,
            message,
            status,
        }
    }
}
