//! The `pair` decisions of each sheet: which row joins which file, and the
//! target each paired row claims before the rule's rows are read.

use std::collections::HashMap;

use super::{Found, SheetIn, Target, kind_of, line_keys_changed, target_of};
use crate::import::decisions::{Decision, EntryRef, RowKey, pairs};
use crate::import::flags::FlagBook;
use crate::import::grouping::Grouping;
use crate::import::model::{GroupKey, Orphan, OrphanReason};
use crate::import::plan::{FlagKind, FlagPolicy, RecordKind};

/// One sheet's pairs, by the paired row's place in the sheet.
#[derive(Default)]
pub(super) struct Paired {
    /// What each paired row's join finds: its file and the file's group.
    pub found: HashMap<usize, Found>,
    /// The row that holds each target.
    pub claims: HashMap<Target, usize>,
}

/// One [`Paired`] per sheet, by the sheet's place in the plan. The latest
/// pair of a row wins, and so does the latest pair on a target.
pub(super) fn paired(
    decisions: &[Decision],
    sheets: &[SheetIn<'_>],
    grouping: &Grouping<'_>,
    book: &FlagBook<'_>,
    orphans: &mut Vec<Orphan>,
) -> Vec<Paired> {
    let mut paired: Vec<Paired> = sheets.iter().map(|_| Paired::default()).collect();
    let mut listed = pairs(decisions).peekable();
    if listed.peek().is_none() {
        return paired;
    }
    let group_of: HashMap<usize, &GroupKey> = grouping
        .groups
        .iter()
        .flat_map(|(key, group)| group.files.iter().map(move |file| (*file, key)))
        .collect();
    let mut rows_by_key: Vec<Option<HashMap<&RowKey, usize>>> =
        sheets.iter().map(|_| None).collect();
    let mut targets: Vec<HashMap<usize, Target>> = sheets.iter().map(|_| HashMap::new()).collect();

    for (decision, row, file) in listed {
        let mut orphan = |entry: EntryRef, reason| {
            orphans.push(Orphan {
                decision,
                entry,
                reason,
            });
        };
        let row_entry = || EntryRef::Row(row.clone());
        let Some(&at_file) = grouping.by_ref.get(file) else {
            orphan(EntryRef::File(file.clone()), OrphanReason::MissingEntry);
            continue;
        };
        let Some((at_sheet, sheet)) = sheets
            .iter()
            .enumerate()
            .find(|(_, sheet)| sheet.plan.id == row.sheet)
            .and_then(|(at, sheet)| Some((at, sheet.sheet?)))
        else {
            orphan(row_entry(), OrphanReason::MissingEntry);
            continue;
        };
        if line_keys_changed(&sheets[at_sheet]) {
            orphan(row_entry(), OrphanReason::LineKeyChanged);
            continue;
        }
        let by_key = rows_by_key[at_sheet].get_or_insert_with(|| {
            sheet
                .rows
                .iter()
                .enumerate()
                .map(|(at, row)| (&row.key, at))
                .collect()
        });
        let Some(&at_row) = by_key.get(&row.key) else {
            orphan(row_entry(), OrphanReason::MissingEntry);
            continue;
        };
        let read = &sheet.rows[at_row];
        let flagged = [row_entry()];
        let left = |kind| book.policy(kind, None, &flagged) == Some(FlagPolicy::Leave);
        // A row its own flag leaves out joins nothing; the pair is not lost.
        if read.cells_count.is_some() && left(FlagKind::CellsCount) {
            continue;
        }
        let kind = match kind_of(sheets[at_sheet].plan, read) {
            Ok(kind) => kind,
            Err(_) if left(FlagKind::UnknownKind) => continue,
            Err(_) => RecordKind::Item,
        };
        if kind == RecordKind::ItemSet {
            orphan(row_entry(), OrphanReason::WrongTarget);
            continue;
        }
        // A list joins only its own series' files.
        if grouping.files[at_file].series != Some(sheets[at_sheet].series) {
            orphan(EntryRef::File(file.clone()), OrphanReason::WrongTarget);
            continue;
        }
        let found = Found::File(at_file, group_of.get(&at_file).map(|key| (*key).clone()));
        let Some(target) = target_of(Some(&found), kind) else {
            orphan(EntryRef::File(file.clone()), OrphanReason::NoGroup);
            continue;
        };
        let sheet_pairs = &mut paired[at_sheet];
        if let Some(earlier) = targets[at_sheet].insert(at_row, target.clone())
            && sheet_pairs.claims.get(&earlier) == Some(&at_row)
        {
            sheet_pairs.claims.remove(&earlier);
        }
        sheet_pairs.found.insert(at_row, found);
        sheet_pairs.claims.insert(target, at_row);
    }
    paired
}
