//! What an undo is told (`force`) and what it reports, in the summary's
//! `undo` member.

use std::collections::HashSet;

use omeka_db::ids::{ItemId, ItemSetId, MediaId, PropertyId, TemplateId};
use omeka_db::rows::{ImportKind, ImportTarget};
use serde::Serialize;
use serde_json::Value;

use crate::import2::touched::Touch;

/// Which touched resources the undo restores or deletes anyway.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Force {
    #[default]
    Nothing,
    All,
    These(HashSet<ImportTarget>),
}

impl Force {
    /// Read the job's `force` argument: absent, `"all"`, or a list of
    /// `{ kind, id }` with the route's camel-case kinds.
    ///
    /// # Errors
    /// The reason, for any other shape.
    pub fn from_arg(arg: Option<&Value>) -> Result<Self, String> {
        let list = match arg {
            None | Some(Value::Null) => return Ok(Self::Nothing),
            Some(Value::String(word)) if word == "all" => return Ok(Self::All),
            Some(Value::Array(list)) => list,
            Some(other) => return Err(format!("`force` is not \"all\" or a list: {other}")),
        };
        list.iter()
            .map(|entry| {
                let id = entry.get("id").and_then(Value::as_u64);
                let kind = entry.get("kind").and_then(Value::as_str);
                match (kind, id) {
                    (Some("item"), Some(id)) => Ok(ImportTarget::Item(ItemId(id))),
                    (Some("media"), Some(id)) => Ok(ImportTarget::Media(MediaId(id))),
                    (Some("itemSet"), Some(id)) => Ok(ImportTarget::ItemSet(ItemSetId(id))),
                    (Some("resourceTemplate"), Some(id)) => {
                        Ok(ImportTarget::ResourceTemplate(TemplateId(id)))
                    }
                    (Some("property"), Some(id)) => Ok(ImportTarget::Property(PropertyId(id))),
                    _ => Err(format!("`force` names no kind and id: {entry}")),
                }
            })
            .collect::<Result<_, _>>()
            .map(Self::These)
    }

    pub(super) fn covers(&self, target: ImportTarget) -> bool {
        match self {
            Self::Nothing => false,
            Self::All => true,
            Self::These(targets) => targets.contains(&target),
        }
    }
}

/// A resource as the report names it, by the route's kind and its id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Named {
    pub kind: &'static str,
    pub id: u64,
}

impl Named {
    #[must_use]
    pub fn of(target: ImportTarget) -> Self {
        let (kind, id) = match target {
            ImportTarget::Item(id) => ("item", id.0),
            ImportTarget::Media(id) => ("media", id.0),
            ImportTarget::ItemSet(id) => ("itemSet", id.0),
            ImportTarget::ResourceTemplate(id) => ("resourceTemplate", id.0),
            ImportTarget::Property(id) => ("property", id.0),
        };
        Self { kind, id }
    }
}

/// One reason a resource was kept, with the message the admin translates.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Reason {
    #[serde(flatten)]
    pub touch: Touch,
    pub key: String,
    pub detail: String,
}

/// A resource the undo left in place.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Kept {
    #[serde(flatten)]
    pub resource: Named,
    pub reasons: Vec<Reason>,
}

/// Resources by kind.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Counts {
    pub items: usize,
    pub media: usize,
    pub item_sets: usize,
    pub templates: usize,
    pub properties: usize,
}

impl Counts {
    pub(super) fn add(&mut self, kind: ImportKind) {
        match kind {
            ImportKind::Item => self.items += 1,
            ImportKind::Media => self.media += 1,
            ImportKind::ItemSet => self.item_sets += 1,
            ImportKind::ResourceTemplate => self.templates += 1,
            ImportKind::Property => self.properties += 1,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct Progress {
    pub done: usize,
    pub total: usize,
}

/// Why the undo stopped before the end.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Stopped {
    pub key: String,
    pub detail: String,
}

/// What an undo did, kept as the summary's [`UNDO_MEMBER`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct UndoReport {
    pub progress: Progress,
    pub removed: Counts,
    pub restored: Counts,
    pub kept: Vec<Kept>,
    /// Already gone when the undo reached them.
    pub skipped: Vec<Named>,
    /// Touched, and restored or deleted because `force` named them.
    pub forced: Vec<Named>,
    /// A stop was asked for.
    pub stopped: bool,
    pub error: Option<Stopped>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn force_reads_all_a_list_or_nothing() {
        assert_eq!(Force::from_arg(None), Ok(Force::Nothing));
        assert_eq!(Force::from_arg(Some(&json!("all"))), Ok(Force::All));
        let these = Force::from_arg(Some(&json!([
            { "kind": "item", "id": 4 },
            { "kind": "itemSet", "id": 9 }
        ])))
        .unwrap_or_default();
        assert!(these.covers(ImportTarget::Item(ItemId(4))));
        assert!(these.covers(ImportTarget::ItemSet(ItemSetId(9))));
        assert!(!these.covers(ImportTarget::Item(ItemId(9))));
        assert!(Force::from_arg(Some(&json!([{ "kind": "page", "id": 1 }]))).is_err());
        assert!(Force::from_arg(Some(&json!("some"))).is_err());
    }

    #[test]
    fn a_kept_resource_lists_each_reason_with_its_message() {
        let kept = Kept {
            resource: Named::of(ImportTarget::Item(ItemId(3))),
            reasons: vec![Reason {
                touch: Touch::LaterImport { import: 8 },
                key: Touch::LaterImport { import: 8 }.key(),
                detail: Touch::LaterImport { import: 8 }.detail(),
            }],
        };
        assert_eq!(
            serde_json::to_value(kept).ok(),
            Some(json!({
                "kind": "item", "id": 3,
                "reasons": [{
                    "reason": "laterImport", "import": 8,
                    "key": "server.importUndoLaterImport",
                    "detail": "Import 8 changed this after this import did."
                }]
            }))
        );
    }
}
