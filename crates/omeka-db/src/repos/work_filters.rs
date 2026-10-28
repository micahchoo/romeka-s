//! Romeka's own item filters: the lists the dashboard's counts open. Upstream has no parameter
//! for any of them (`ItemAdapter::buildQuery` reads `item_set_id` and `resource_template_id` only
//! as ids to match).
//!
//! | Parameter | Items kept |
//! | --- | --- |
//! | `romeka_no_item_set=1` | in no item set |
//! | `romeka_no_resource_template=1` | with no resource template |
//! | `romeka_changed_by_others_since=<time>` | with a history entry since then by anyone but the reader |

use chrono::NaiveDateTime;
use sqlx::{Encode, Type};

use super::{Statement, bind_id};
use crate::DbError;
use crate::backend::Backend;
use crate::ids::UserId;
use crate::time;

/// The work filters of one item query. The default adds nothing.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WorkFilters {
    pub no_item_set: bool,
    pub no_resource_template: bool,
    pub changed_by_others: Option<OthersChanges>,
}

/// Which history entries make an item "changed by others".
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OthersChanges {
    /// Entries made at or after `since` by any user but `reader`, or by no user (a system job, or
    /// a user since deleted).
    Since {
        since: NaiveDateTime,
        reader: UserId,
    },
    /// No entry: the reader may not ask, or `since` was not a time.
    None,
}

impl WorkFilters {
    pub(crate) fn push<'q, DB: Backend>(
        &self,
        statement: &mut Statement<'q, DB>,
    ) -> Result<(), DbError>
    where
        for<'x> i64: Encode<'x, DB> + Type<DB>,
        for<'x> String: Encode<'x, DB> + Type<DB>,
    {
        if self.no_item_set {
            statement.push(
                " AND NOT EXISTS (SELECT 1 FROM item_item_set
                   WHERE item_item_set.item_id = resource.id)",
            );
        }
        if self.no_resource_template {
            statement.push(" AND resource.resource_template_id IS NULL");
        }
        match &self.changed_by_others {
            None => {}
            Some(OthersChanges::None) => {
                statement.push(" AND 1 = 0");
            }
            Some(OthersChanges::Since { since, reader }) => {
                statement.push(
                    " AND EXISTS (SELECT 1 FROM romeka_item_history h
                       WHERE h.item_id = resource.id AND h.created >= ",
                );
                statement.bind(time::format(*since))?;
                statement.push(" AND (h.user_id IS NULL OR h.user_id <> ");
                statement.bind(bind_id(*reader))?;
                statement.push("))");
            }
        }
        Ok(())
    }
}
