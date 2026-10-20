//! The answers to anomalies (`docs/import-v2.md`, "Answers to anomalies").
//! They are not flags: an anomaly has no value, entry or record a policy is
//! keyed by. They live in the plan, not in decisions, so a rescan applies
//! them to files it finds later.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
pub struct AnomalyAnswers {
    pub identical: IdenticalAnswer,
    pub unmatched_files: UnmatchedFilesAnswer,
    pub items_without_row: ItemsWithoutRowAnswer,
}

impl AnomalyAnswers {
    pub(crate) fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// Files of equal size and equal SHA-256.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum IdenticalAnswer {
    /// Of each set, keep the file a list row names, else the first by path.
    #[default]
    KeepOne,
    KeepAll,
}

/// Files the compiled pattern finds no `identifier` in.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum UnmatchedFilesAnswer {
    #[default]
    Leave,
    /// Each file is an item of its own.
    Own,
}

/// Items with files that no list row joined, when a list joins items.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ItemsWithoutRowAnswer {
    #[default]
    Keep,
    Leave,
}
