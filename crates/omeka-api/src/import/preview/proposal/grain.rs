//! Detector 4, row grain, is not built yet: the rows of every list that
//! describes a series describe its items.

use super::lists::Found;
use super::{Choice, Evidence, GrainCounts};
use crate::import::plan::{ColumnRef, Edit, FolderScope, Join, ListFile, RecordKind, RowKind};

/// One list as the plan will read it.
#[derive(Debug, Clone)]
pub(super) struct ListPlan {
    pub file: ListFile,
    pub row_kind: RowKind,
    pub join: Option<Join>,
    pub item_key: Option<ColumnRef>,
    /// The columns slot rules or a set source read.
    pub mapped: Vec<ColumnRef>,
}

/// What detector 4 chose for one series.
#[derive(Debug, Default)]
pub(super) struct SeriesRows {
    pub lists: Vec<ListPlan>,
    /// The grain of the series' first list, when it joins by file name.
    pub grain: Option<Choice<RecordKind, GrainCounts>>,
    /// The rows that are the series' items, by their list and key column.
    pub items: Option<(ListFile, ColumnRef)>,
    /// A column of item rows whose values are keys of item-set rows.
    pub set: Option<(ListFile, ColumnRef)>,
    /// The key column of each kind of rows of a list with a kind column.
    pub keys: Vec<(RecordKind, ListFile, ColumnRef)>,
}

/// Each series, as detector 4 reads it.
#[expect(dead_code, reason = "detector 4 reads it")]
pub(super) struct SeriesRead<'s> {
    /// The scope its evidence names.
    pub scope: &'s FolderScope,
    /// The media in its scope.
    pub media: usize,
    /// A person chose its cut: its items do not move to rows.
    pub cut_chosen: bool,
}

/// Each series' lists, their rows read as items.
pub(super) fn detect(
    _evidence: &Evidence<'_>,
    found: &mut Found<'_>,
    _series: &[SeriesRead<'_>],
    _edits: &[Edit],
    _applied: &mut [bool],
) -> Vec<SeriesRows> {
    found
        .series
        .iter()
        .map(|lists| SeriesRows {
            lists: lists
                .joined
                .iter()
                .map(|one| ListPlan {
                    file: one.file.clone(),
                    row_kind: RowKind::All(RecordKind::Item),
                    join: one.counted.join.clone(),
                    item_key: None,
                    mapped: Vec::new(),
                })
                .collect(),
            ..SeriesRows::default()
        })
        .collect()
}
