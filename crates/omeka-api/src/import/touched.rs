//! The touched test (`docs/import-v2.md`, "Lifecycle and the ledger"): what
//! changed a resource an import wrote, after the import's ledger row says it
//! wrote it.
//!
//! A resource is touched when its `modified` is later than the row's written
//! time, when a later import wrote it, or, for what the import made, when
//! something outside the import now depends on it: a link, a media added to
//! the item, an item put in the set. A changed resource is restored, not
//! deleted, so a link to it counts only when the linking resource changed
//! after the write, and media the item held before the import never count.
//!
//! Times are whole seconds, as upstream stores them: a change in the second
//! the import wrote is not seen.

use std::collections::HashSet;

use omeka_db::Backend;
use omeka_db::DbError;
use omeka_db::ids::{ImportId, MediaId, ResourceId};
use omeka_db::repos::{ImportTouchRepo, Pointer};
use omeka_db::rows::{ImportChange, ImportResourceRow, ImportTarget};
use serde::Serialize;
use sqlx::{ColumnIndex, Decode, Encode, Executor, IntoArguments, Pool, Type};

use crate::messages::{self, ServerMessage};

/// Why undo keeps a resource.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "reason", rename_all = "camelCase")]
pub enum Touch {
    /// Its `modified` is later than the import's write.
    Edited,
    /// A resource outside the import links to it.
    Linked,
    /// Media outside the import were added to the item.
    MediaAdded,
    /// Items outside the import were put in the set.
    MembersAdded,
    /// A later import wrote it.
    LaterImport { import: u64 },
    /// A template or field the import made that something outside it uses.
    InUse,
    /// A media whose item undo keeps.
    ItemKept,
    /// An item set that holds an item undo keeps.
    MemberKept,
}

impl Touch {
    /// The registry name of the message that says why.
    fn message(self) -> &'static str {
        match self {
            Self::Edited => "IMPORT_UNDO_EDITED",
            Self::Linked => "IMPORT_UNDO_LINKED",
            Self::MediaAdded => "IMPORT_UNDO_MEDIA_ADDED",
            Self::MembersAdded => "IMPORT_UNDO_MEMBERS_ADDED",
            Self::LaterImport { .. } => "import_undo_later_import",
            Self::InUse => "IMPORT_UNDO_IN_USE",
            Self::ItemKept => "IMPORT_UNDO_ITEM_KEPT",
            Self::MemberKept => "IMPORT_UNDO_MEMBER_KEPT",
        }
    }

    /// The catalogue key the admin translates by.
    #[must_use]
    pub fn key(self) -> String {
        ServerMessage {
            name: self.message(),
            pattern: "",
        }
        .key()
    }

    /// The English text.
    #[must_use]
    pub fn detail(self) -> String {
        match self {
            Self::Edited => messages::IMPORT_UNDO_EDITED.to_string(),
            Self::Linked => messages::IMPORT_UNDO_LINKED.to_string(),
            Self::MediaAdded => messages::IMPORT_UNDO_MEDIA_ADDED.to_string(),
            Self::MembersAdded => messages::IMPORT_UNDO_MEMBERS_ADDED.to_string(),
            Self::LaterImport { import } => messages::import_undo_later_import(import),
            Self::InUse => messages::IMPORT_UNDO_IN_USE.to_string(),
            Self::ItemKept => messages::IMPORT_UNDO_ITEM_KEPT.to_string(),
            Self::MemberKept => messages::IMPORT_UNDO_MEMBER_KEPT.to_string(),
        }
    }
}

/// What the import itself wrote, which never touches its own records.
#[derive(Debug, Clone, Default)]
pub struct Own {
    targets: HashSet<ImportTarget>,
    resources: HashSet<ResourceId>,
}

impl Own {
    #[must_use]
    pub fn from_rows(rows: &[ImportResourceRow]) -> Self {
        let targets: HashSet<ImportTarget> = rows.iter().map(|row| row.target).collect();
        let resources = targets
            .iter()
            .filter_map(|target| resource(*target))
            .collect();
        Self { targets, resources }
    }

    #[must_use]
    pub fn holds(&self, target: ImportTarget) -> bool {
        self.targets.contains(&target)
    }
}

/// The resource row of an item, media or item set; none for a template or a
/// property.
#[must_use]
pub fn resource(target: ImportTarget) -> Option<ResourceId> {
    match target {
        ImportTarget::Item(id) => Some(id.as_resource()),
        ImportTarget::Media(id) => Some(id.as_resource()),
        ImportTarget::ItemSet(id) => Some(id.as_resource()),
        ImportTarget::ResourceTemplate(_) | ImportTarget::Property(_) => None,
    }
}

/// What touched the item, media or item set `row` records, which exists and
/// was last modified at `modified`. `before` is the item's media order before
/// the import, for a changed item. Empty when nothing did.
///
/// # Errors
/// A [`DbError`] when a read fails.
pub async fn touched<DB>(
    pool: &Pool<DB>,
    import: ImportId,
    row: &ImportResourceRow,
    modified: Option<chrono::NaiveDateTime>,
    own: &Own,
    before: &[MediaId],
) -> Result<Vec<Touch>, DbError>
where
    DB: Backend,
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
    usize: ColumnIndex<DB::Row>,
    for<'x> i64: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> String: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> &'x str: Encode<'x, DB> + Type<DB> + ColumnIndex<DB::Row>,
{
    let reads = ImportTouchRepo::<DB>::new(pool);
    let created = row.change == ImportChange::Created;
    let after = |pointer: &Pointer| pointer.modified.is_some_and(|at| at > row.written);
    let outside = |pointer: &Pointer| !own.resources.contains(&pointer.resource);
    let mut touches = Vec::new();
    if modified.is_some_and(|at| at > row.written) {
        touches.push(Touch::Edited);
    }
    if let Some(id) = resource(row.target) {
        let linked = reads
            .linkers(id)
            .await?
            .iter()
            .any(|pointer| outside(pointer) && (created || after(pointer)));
        if linked {
            touches.push(Touch::Linked);
        }
    }
    match row.target {
        ImportTarget::Item(item) => {
            let added =
                reads.media_of(item).await?.into_iter().any(|media| {
                    !own.holds(ImportTarget::Media(media)) && !before.contains(&media)
                });
            if added {
                touches.push(Touch::MediaAdded);
            }
        }
        ImportTarget::ItemSet(set) if created => {
            let added = reads.set_members(set).await?.iter().any(outside);
            if added {
                touches.push(Touch::MembersAdded);
            }
        }
        _ => {}
    }
    for later in reads.later_imports(import, row.target, row.written).await? {
        touches.push(Touch::LaterImport { import: later.0 });
    }
    Ok(touches)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_reason_names_a_registered_message() {
        let all = [
            Touch::Edited,
            Touch::Linked,
            Touch::MediaAdded,
            Touch::MembersAdded,
            Touch::LaterImport { import: 7 },
            Touch::InUse,
            Touch::ItemKept,
            Touch::MemberKept,
        ];
        for touch in all {
            let registered = messages::REGISTRY
                .iter()
                .find(|message| message.key() == touch.key())
                .unwrap_or_else(|| panic!("{touch:?} is not registered"));
            assert!(!registered.pattern.is_empty());
        }
        assert_eq!(
            Touch::LaterImport { import: 7 }.detail(),
            "Import 7 changed this after this import did."
        );
        assert_eq!(
            serde_json::to_value(Touch::LaterImport { import: 7 }).ok(),
            Some(serde_json::json!({ "reason": "laterImport", "import": 7 }))
        );
    }
}
