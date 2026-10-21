//! Flag kinds and policies: the "Flags" table of `docs/import-v2.md`.

use serde::{Deserialize, Serialize};

/// What happens to a flagged value, entry or record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FlagPolicy {
    /// Write it as it is.
    Keep,
    /// Apply the reader's automatic repair.
    Repair,
    /// Leave it out.
    Leave,
}

/// A policy a kind starts with when the plan names none.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyDefault {
    Is(FlagPolicy),
    /// `unmatched` follows the plan's `action.unmatched`.
    ActionUnmatched,
    /// The kind's only policy: no plan policy or decision changes it.
    Only(FlagPolicy),
}

/// What a flag is about: the table's "On" column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FlagOn {
    Value,
    Entry,
    Row,
    ItemRecord,
    File,
    Record,
    Plan,
    Sheet,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FlagKind {
    EmptyRequired,
    DateForm,
    SpaceAtEnds,
    EncodingSuspect,
    NameEncoding,
    CellsCount,
    NoFile,
    NotMedia,
    /// A file a list row names that the install refuses.
    ListedNotMedia,
    FormatMismatch,
    Exists,
    Unmatched,
    ManyMatches,
    ManyValues,
    LinkMissing,
    UnknownType,
    UnknownKind,
    DuplicateRow,
    UnknownTerm,
    TemplateMissing,
    TypeExists,
    EncodingUnsupported,
}

impl FlagKind {
    /// Every kind, in the table's order.
    pub const ALL: [Self; 22] = [
        Self::EmptyRequired,
        Self::DateForm,
        Self::SpaceAtEnds,
        Self::EncodingSuspect,
        Self::NameEncoding,
        Self::CellsCount,
        Self::NoFile,
        Self::NotMedia,
        Self::ListedNotMedia,
        Self::FormatMismatch,
        Self::Exists,
        Self::Unmatched,
        Self::ManyMatches,
        Self::ManyValues,
        Self::LinkMissing,
        Self::UnknownType,
        Self::UnknownKind,
        Self::DuplicateRow,
        Self::UnknownTerm,
        Self::TemplateMissing,
        Self::TypeExists,
        Self::EncodingUnsupported,
    ];

    /// Whether the kind stops a run. `encoding-unsupported` stops it for
    /// the sheet it is on.
    #[must_use]
    pub fn is_blocking(self) -> bool {
        self.default_policy().is_none()
    }

    /// What a flag of this kind is about.
    #[must_use]
    pub fn on(self) -> FlagOn {
        match self {
            Self::EmptyRequired
            | Self::DateForm
            | Self::SpaceAtEnds
            | Self::EncodingSuspect
            | Self::ManyValues
            | Self::LinkMissing => FlagOn::Value,
            Self::NameEncoding => FlagOn::Entry,
            Self::CellsCount | Self::UnknownKind | Self::DuplicateRow => FlagOn::Row,
            Self::NoFile => FlagOn::ItemRecord,
            Self::NotMedia | Self::ListedNotMedia | Self::FormatMismatch => FlagOn::File,
            Self::Exists | Self::Unmatched | Self::ManyMatches | Self::UnknownType => {
                FlagOn::Record
            }
            Self::UnknownTerm | Self::TemplateMissing | Self::TypeExists => FlagOn::Plan,
            Self::EncodingUnsupported => FlagOn::Sheet,
        }
    }

    /// The policy a kind has when the plan names none. A blocking kind has
    /// no policy: nothing the plan says lets it through.
    #[must_use]
    pub fn default_policy(self) -> Option<PolicyDefault> {
        use FlagPolicy::{Keep, Leave, Repair};
        let policy = match self {
            Self::EmptyRequired
            | Self::DateForm
            | Self::EncodingSuspect
            | Self::NameEncoding
            | Self::NoFile
            | Self::FormatMismatch
            | Self::ManyValues
            | Self::LinkMissing
            | Self::UnknownType => Keep,
            Self::SpaceAtEnds => Repair,
            Self::CellsCount
            | Self::NotMedia
            | Self::Exists
            | Self::ManyMatches
            | Self::UnknownKind
            | Self::DuplicateRow => Leave,
            Self::Unmatched => return Some(PolicyDefault::ActionUnmatched),
            // Keeping it cannot work: ingest checks the file again.
            Self::ListedNotMedia => return Some(PolicyDefault::Only(Leave)),
            Self::UnknownTerm
            | Self::TemplateMissing
            | Self::TypeExists
            | Self::EncodingUnsupported => return None,
        };
        Some(PolicyDefault::Is(policy))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use FlagPolicy::{Keep, Leave, Repair};

    /// The contract's table, row by row: kind, default, blocking.
    const TABLE: [(&str, Option<PolicyDefault>); 22] = [
        ("empty-required", Some(PolicyDefault::Is(Keep))),
        ("date-form", Some(PolicyDefault::Is(Keep))),
        ("space-at-ends", Some(PolicyDefault::Is(Repair))),
        ("encoding-suspect", Some(PolicyDefault::Is(Keep))),
        ("name-encoding", Some(PolicyDefault::Is(Keep))),
        ("cells-count", Some(PolicyDefault::Is(Leave))),
        ("no-file", Some(PolicyDefault::Is(Keep))),
        ("not-media", Some(PolicyDefault::Is(Leave))),
        ("listed-not-media", Some(PolicyDefault::Only(Leave))),
        ("format-mismatch", Some(PolicyDefault::Is(Keep))),
        ("exists", Some(PolicyDefault::Is(Leave))),
        ("unmatched", Some(PolicyDefault::ActionUnmatched)),
        ("many-matches", Some(PolicyDefault::Is(Leave))),
        ("many-values", Some(PolicyDefault::Is(Keep))),
        ("link-missing", Some(PolicyDefault::Is(Keep))),
        ("unknown-type", Some(PolicyDefault::Is(Keep))),
        ("unknown-kind", Some(PolicyDefault::Is(Leave))),
        ("duplicate-row", Some(PolicyDefault::Is(Leave))),
        ("unknown-term", None),
        ("template-missing", None),
        ("type-exists", None),
        ("encoding-unsupported", None),
    ];

    /// The table's "On" column, row by row.
    #[test]
    fn every_kind_is_on_what_the_table_says() {
        use FlagOn::{Entry, File, ItemRecord, Plan, Record, Row, Sheet, Value};
        let on = [
            Value, Value, Value, Value, Entry, Row, ItemRecord, File, File, File, Record, Record,
            Record, Value, Value, Record, Row, Row, Plan, Plan, Plan, Sheet,
        ];
        for (kind, on) in FlagKind::ALL.into_iter().zip(on) {
            assert_eq!(kind.on(), on, "{kind:?}");
        }
    }

    #[test]
    fn every_kind_has_the_contract_default_and_blocking() {
        let named: Vec<FlagKind> = TABLE
            .iter()
            .filter_map(|(name, _)| serde_json::from_value(serde_json::json!(name)).ok())
            .collect();
        assert_eq!(
            named,
            FlagKind::ALL,
            "every table row names one kind, in order"
        );
        for (kind, (name, default)) in FlagKind::ALL.into_iter().zip(TABLE) {
            assert_eq!(kind.default_policy(), default, "{name}");
            assert_eq!(kind.is_blocking(), default.is_none(), "{name}");
        }
        let blocking = FlagKind::ALL
            .iter()
            .filter(|kind| kind.is_blocking())
            .count();
        assert_eq!(blocking, 4, "four blocking plan kinds");
    }
}
