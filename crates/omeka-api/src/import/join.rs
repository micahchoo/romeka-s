//! Step 3 of the derive: sheet rows joined to file groups
//! (`docs/import-v2.md`, "The derive").
//!
//! An item row joins the file group whose key equals its join cell, or, for
//! a `fileName` join, the group holding the file its cell names. Both sides
//! pass through [`normalize`] first. A media row with a `fileName` join
//! finds its file; every other media row attaches by item key later, once
//! item keys are resolved. An item-set row is a record of its own. A `pair`
//! decision joins its row to its file's group (or, for a media row, to the
//! file) before the rule, so a rule row that would join that target is a
//! duplicate.

mod pair;

use std::collections::{BTreeMap, HashMap, HashSet};

use unicode_normalization::UnicodeNormalization;

use super::decisions::{Decision, EntryRef, RowRef};
use super::grouping::Grouping;
use super::model::{GroupKey, Orphan, Subject};
use super::plan::{FlagKind, FlagPolicy, JoinNormalize, JoinOn, RecordKind, RowKind, SheetPlan};
use super::sheet::{Row, Sheet, key_column};
use super::values::row_kind_from_cell;
use crate::import::flags::FlagBook;

/// A sheet the derive reads: its plan, and its rows when it was read.
pub(crate) struct SheetIn<'i> {
    pub plan: &'i SheetPlan,
    pub sheet: Option<&'i Sheet>,
}

/// A row, by its sheet's place in the plan and its place in the sheet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct RowAt {
    pub sheet: usize,
    pub row: usize,
}

impl RowAt {
    pub(crate) fn row<'i>(self, sheets: &[SheetIn<'i>]) -> &'i Row {
        // The join makes a `RowAt` only for a row of a sheet it read.
        let sheet = sheets[self.sheet].sheet.unwrap_or_else(|| unreachable!());
        &sheet.rows[self.row]
    }

    pub(crate) fn entry(self, sheets: &[SheetIn<'_>]) -> RowRef {
        RowRef {
            sheet: sheets[self.sheet].plan.id.clone(),
            key: self.row(sheets).key.clone(),
        }
    }
}

pub(crate) struct MediaRow {
    pub at: RowAt,
    /// The file its `fileName` join cell names.
    pub file: Option<usize>,
}

#[derive(Default)]
pub(crate) struct Joined {
    /// Item rows by the group they joined, in sheet order.
    pub group_rows: BTreeMap<GroupKey, Vec<RowAt>>,
    /// Item rows that joined no group: items with no file.
    pub lone_items: Vec<RowAt>,
    pub media_rows: Vec<MediaRow>,
    pub set_rows: Vec<RowAt>,
    /// Each file a joined row names by its `fileName` cell or a `pair`,
    /// with the first row that names it.
    pub named: BTreeMap<usize, RowAt>,
    /// Rows a flag's `leave` policy took out.
    pub skipped: BTreeMap<RowAt, FlagKind>,
    /// `pair` decisions whose row or file is gone or cannot join.
    pub orphans: Vec<Orphan>,
}

/// A sheet keyed by line whose bytes changed since the plan stored a hash:
/// its line keys now name other rows.
pub(crate) fn line_keys_changed(sheet: &SheetIn<'_>) -> bool {
    key_column(sheet.plan).is_none()
        && match (&sheet.plan.hash, sheet.sheet) {
            (Some(stored), Some(read)) => *stored != read.hash,
            _ => false,
        }
}

/// The kind a row's kind cell names, or the cell's text when it names none.
fn kind_of<'r>(plan: &SheetPlan, row: &'r Row) -> Result<RecordKind, &'r str> {
    match &plan.row_kind {
        RowKind::All(kind) => Ok(*kind),
        RowKind::Column(column) => {
            let text = cell(row, column.column.index);
            row_kind_from_cell(text).ok_or(text)
        }
    }
}

/// What a row of `kind` joins when its join finds `found`.
fn target_of(found: Option<&Found>, kind: RecordKind) -> Option<Target> {
    match (found?, kind) {
        (Found::Group(group), RecordKind::Item) => Some(Target::Group(group.clone())),
        (Found::File(file, _), RecordKind::Media) => Some(Target::File(*file)),
        (Found::File(_, group), RecordKind::Item) => group.clone().map(Target::Group),
        _ => None,
    }
}

/// NFC and trimmed always; then the optional case fold, separator fold
/// (`_`, `-`, `.` and space, in runs, become one `_`) and leading-zero strip
/// (each run of digits loses its leading zeros, keeping one digit).
#[must_use]
pub fn normalize(text: &str, how: JoinNormalize) -> String {
    let mut text: String = text.trim().nfc().collect();
    if how.case_fold {
        text = text.to_lowercase();
    }
    if how.separators {
        let mut folded = String::with_capacity(text.len());
        for c in text.chars() {
            if matches!(c, '_' | '-' | '.' | ' ') {
                if !folded.ends_with('_') {
                    folded.push('_');
                }
            } else {
                folded.push(c);
            }
        }
        text = folded;
    }
    if how.leading_zeros {
        let mut stripped = String::with_capacity(text.len());
        let mut chars = text.chars().peekable();
        let mut in_digits = false;
        while let Some(c) = chars.next() {
            let next_is_digit = chars.peek().is_some_and(char::is_ascii_digit);
            if c == '0' && !in_digits && next_is_digit {
                continue;
            }
            in_digits = c.is_ascii_digit();
            stripped.push(c);
        }
        text = stripped;
    }
    text
}

/// Cell `index` of a row, extra cells included, `""` past the end.
pub(crate) fn cell(row: &Row, index: u32) -> &str {
    let index = usize::try_from(index).unwrap_or(usize::MAX);
    row.cells
        .iter()
        .chain(&row.extra)
        .nth(index)
        .map_or("", String::as_str)
}

const EXACT: JoinNormalize = JoinNormalize {
    case_fold: false,
    leading_zeros: false,
    separators: false,
};

pub(crate) fn join(
    sheets: &[SheetIn<'_>],
    grouping: &Grouping<'_>,
    decisions: &[Decision],
    book: &mut FlagBook<'_>,
) -> Joined {
    let mut joined = Joined::default();
    let paired = pair::paired(decisions, sheets, grouping, book, &mut joined.orphans);
    for (at_sheet, sheet_in) in sheets.iter().enumerate() {
        let Some(sheet) = sheet_in.sheet else {
            continue;
        };
        let plan = sheet_in.plan;
        let how = plan.join.as_ref().map_or(EXACT, |join| join.normalize);
        let by_key = plan.join.as_ref().map(|join| match join.on {
            JoinOn::Identifier => groups_by_key(grouping, how),
            JoinOn::FileName => files_by_name(grouping, how),
        });
        let mut seen_keys: HashSet<String> = HashSet::new();
        let mut taken: HashSet<Target> = HashSet::new();
        for (at_row, row) in sheet.rows.iter().enumerate() {
            let at = RowAt {
                sheet: at_sheet,
                row: at_row,
            };
            let entry = [EntryRef::Row(at.entry(sheets))];
            let raise = |book: &mut FlagBook<'_>, kind, detail: String| {
                book.raise(
                    kind,
                    Subject::Entry(entry[0].clone()),
                    None,
                    Some(detail),
                    &entry,
                )
            };
            if let Some(problem) = &row.cells_count
                && raise(book, FlagKind::CellsCount, problem.raw.clone()) == Some(FlagPolicy::Leave)
            {
                joined.skipped.insert(at, FlagKind::CellsCount);
                continue;
            }
            let kind = match kind_of(plan, row) {
                Ok(kind) => kind,
                Err(text)
                    if raise(book, FlagKind::UnknownKind, text.to_string())
                        == Some(FlagPolicy::Leave) =>
                {
                    joined.skipped.insert(at, FlagKind::UnknownKind);
                    continue;
                }
                Err(_) => RecordKind::Item,
            };
            if kind == RecordKind::ItemSet {
                joined.set_rows.push(at);
                continue;
            }
            let key = plan
                .join
                .as_ref()
                .map(|join| normalize(cell(row, join.column.index), how))
                .filter(|key| !key.is_empty());
            let pair = paired[at_sheet].found.get(&at_row);
            let found = match pair {
                Some(found) => Some(found),
                None => key.as_ref().and_then(|key| by_key.as_ref()?.get(key)),
            };
            let target = target_of(found, kind);
            let claimed_by_another = target
                .as_ref()
                .and_then(|target| paired[at_sheet].claims.get(target))
                .is_some_and(|row| *row != at_row);
            // A paired row did not join by its cell, so its key is not seen.
            let duplicate = (pair.is_none()
                && key
                    .as_ref()
                    .is_some_and(|key| !seen_keys.insert(key.clone())))
                || target.as_ref().is_some_and(|target| taken.contains(target))
                || claimed_by_another;
            if duplicate {
                let detail = key.clone().unwrap_or_default();
                if raise(book, FlagKind::DuplicateRow, detail) == Some(FlagPolicy::Leave) {
                    joined.skipped.insert(at, FlagKind::DuplicateRow);
                    continue;
                }
                match kind {
                    RecordKind::Media => joined.media_rows.push(MediaRow { at, file: None }),
                    _ => joined.lone_items.push(at),
                }
                continue;
            }
            if let Some(target) = &target {
                taken.insert(target.clone());
            }
            if let Some(Found::File(file, _)) = found {
                joined.named.entry(*file).or_insert(at);
            }
            match (kind, target) {
                (RecordKind::Media, Some(Target::File(file))) => {
                    joined.media_rows.push(MediaRow {
                        at,
                        file: Some(file),
                    });
                }
                (RecordKind::Media, _) => joined.media_rows.push(MediaRow { at, file: None }),
                (_, Some(Target::Group(group))) => {
                    joined.group_rows.entry(group).or_default().push(at)
                }
                _ => joined.lone_items.push(at),
            }
        }
    }
    joined
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub(super) enum Target {
    Group(GroupKey),
    File(usize),
}

#[derive(Clone)]
pub(super) enum Found {
    Group(GroupKey),
    /// A file, and the group it is in.
    File(usize, Option<GroupKey>),
}

/// Each group by its normalised key. Two keys that normalise alike join
/// the first in key order.
fn groups_by_key(grouping: &Grouping<'_>, how: JoinNormalize) -> HashMap<String, Found> {
    let mut by_key = HashMap::new();
    for key in grouping.groups.keys() {
        by_key
            .entry(normalize(key.text(), how))
            .or_insert_with(|| Found::Group(key.clone()));
    }
    by_key
}

/// Each file by its normalised name and by its normalised path. A name two
/// files share finds the first in path order; the path still finds each.
fn files_by_name(grouping: &Grouping<'_>, how: JoinNormalize) -> HashMap<String, Found> {
    let mut group_of: HashMap<usize, GroupKey> = HashMap::new();
    for (key, files) in &grouping.groups {
        for file in files {
            group_of.insert(*file, key.clone());
        }
    }
    let mut by_name = HashMap::new();
    for (at, file) in grouping.files.iter().enumerate() {
        let found = Found::File(at, group_of.get(&at).cloned());
        by_name.insert(normalize(file.entry.path.as_str(), how), found.clone());
        by_name
            .entry(normalize(file.entry.name(), how))
            .or_insert(found);
    }
    by_name
}

#[cfg(test)]
mod tests {
    use super::*;

    fn how(case_fold: bool, leading_zeros: bool, separators: bool) -> JoinNormalize {
        JoinNormalize {
            case_fold,
            leading_zeros,
            separators,
        }
    }

    #[test]
    fn a47_each_normalisation_folds_what_it_names_and_nothing_else() {
        assert_eq!(normalize(" Cafe\u{301} ", EXACT), "Caf\u{e9}");
        assert_eq!(normalize("X_003", how(true, false, false)), "x_003");
        assert_eq!(normalize("X_003", EXACT), "X_003");
        assert_eq!(normalize("007", how(false, true, false)), "7");
        assert_eq!(normalize("B_0040_00", how(false, true, false)), "B_40_0");
        assert_eq!(normalize("B 4", how(false, false, true)), "B_4");
        assert_eq!(normalize("B.-4", how(false, false, true)), "B_4");
        assert_eq!(normalize("B 4", EXACT), "B 4");
        assert_eq!(
            normalize("BHC 006", how(true, true, true)),
            normalize("bhc_6", how(true, true, true))
        );
    }
}
