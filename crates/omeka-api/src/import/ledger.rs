//! What a run reads back from the ledger and writes into the summary
//! (`docs/import-v2.md`, "Lifecycle and the ledger").
//!
//! Every resource a run writes gets a ledger row under a record key. A
//! record's key is [`Origin::key`](super::model::Origin::key); what the run
//! makes before the records (new fields, new types, sets named by a title)
//! has a key of its own here. A retry reads the ledger into a [`Written`],
//! skips every key it holds, and finds there the ids later records link to,
//! so a retry converges.

use std::collections::HashMap;

use omeka_db::rows::{ImportResourceRow, ImportTarget};
use serde::Serialize;
use serde_json::{Value, json};

use super::values::fold_title;

/// The ledger key of a field the run makes in the `local` vocabulary.
#[must_use]
pub fn term_key(label: &str) -> String {
    format!("term:{label}")
}

/// The ledger key of an item type the run makes from the plan's `newTypes`.
#[must_use]
pub fn type_key(label: &str) -> String {
    format!("type:{label}")
}

/// The ledger key of an item set a record names by a title no set has.
/// Titles compare folded, as the derive matched them.
#[must_use]
pub fn set_key(title: &str) -> String {
    format!("set:{}", fold_title(title))
}

/// What the ledger already holds for an import, by record key.
#[derive(Debug, Clone, Default)]
pub struct Written {
    by_record: HashMap<String, Vec<ImportTarget>>,
}

impl Written {
    #[must_use]
    pub fn from_rows(rows: &[ImportResourceRow]) -> Self {
        let mut by_record: HashMap<String, Vec<ImportTarget>> = HashMap::new();
        for row in rows {
            by_record
                .entry(row.record.clone())
                .or_default()
                .push(row.target);
        }
        Self { by_record }
    }

    #[must_use]
    pub fn holds(&self, key: &str) -> bool {
        self.by_record.contains_key(key)
    }

    /// The resource a record key wrote. A record writes one resource.
    #[must_use]
    pub fn target(&self, key: &str) -> Option<ImportTarget> {
        self.by_record.get(key)?.first().copied()
    }

    pub fn add(&mut self, key: String, target: ImportTarget) {
        self.by_record.entry(key).or_default().push(target);
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_record.is_empty()
    }
}

/// A record the run could not write: its ledger key, the message's
/// catalogue key, and the English message the admin translates by pattern.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Failure {
    pub record: String,
    pub key: String,
    pub detail: String,
}

/// The summary a run keeps: progress, failures, and whether it stopped on
/// request. The repository stamps the heartbeat on every write.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Summary {
    pub done: usize,
    pub total: usize,
    pub failures: Vec<Failure>,
    pub stopped: bool,
    /// How often the server resumed the run by itself; it outlives each
    /// attempt, so the run carries it from the summary it found.
    pub resumed: u64,
}

impl Summary {
    /// The JSON the import row stores. `failures` is always an array, empty
    /// when nothing failed, because a `done` import may retry only when it
    /// lists failures.
    #[must_use]
    pub fn to_json(&self) -> Value {
        let mut summary = json!({
            "progress": { "done": self.done, "total": self.total },
            "failures": self.failures,
            "stopped": self.stopped,
        });
        if self.resumed > 0 {
            summary[omeka_db::repos::RESUMED] = json!(self.resumed);
        }
        summary
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use omeka_db::ids::ItemId;

    #[test]
    fn a_set_key_folds_its_title() {
        assert_eq!(set_key("  Photographs "), set_key("photographs"));
        assert_ne!(set_key("Photographs"), set_key("Photograph"));
    }

    #[test]
    fn the_summary_lists_failures_as_an_array() {
        let summary = Summary {
            done: 2,
            total: 3,
            ..Summary::default()
        };
        assert_eq!(
            summary.to_json(),
            json!({ "progress": { "done": 2, "total": 3 }, "failures": [], "stopped": false })
        );
        let mut written = Written::default();
        written.add("group:A1".into(), ImportTarget::Item(ItemId(4)));
        assert!(written.holds("group:A1"));
        assert_eq!(
            written.target("group:A1"),
            Some(ImportTarget::Item(ItemId(4)))
        );
    }
}
