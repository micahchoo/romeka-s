//! The `listCandidates` section: every list file the scan found in a
//! folder source, with how well it fits the files, so the admin ranks the
//! lists and asks the right question about each. Nothing enters the plan.
//!
//! Each list is read as Simple adds one (automatic read options, no key
//! column) through the shared [`SheetCache`](super::SheetCache), so adding
//! it later reads warm. A list is read, counted and let go before the next,
//! so the cache's bounds hold; a list that does not read is a candidate
//! with its error. Its `best` column is the column and join that pair the
//! most rows exactly, counted as `columnMatches` counts. The groups it pairs
//! are its targets, which `grain` and `overlaps` count. `grain` also counts,
//! for each column, in how many of the groups that several rows name those
//! rows disagree, so the admin can suggest where each column goes. `cells`
//! says, for every column, how many cells hold a separator, read as the
//! plan sheet reads the list, so the admin can suggest a split; how many
//! cells are filled is `columnMatches`'s count.

use std::cmp::Reverse;
use std::collections::{BTreeSet, HashMap, HashSet};

use serde::Serialize;

use super::Derived;
use super::columns::{Targets, column_counts, walk};
use crate::import::archive::SourceDir;
use crate::import::join::cell;
use crate::import::model::{GroupKey, Origin};
use crate::import::plan::{
    Auto, Delimiter, EncodingChoice, JoinOn, LevelId, Quote, ReadOptions, RecordKind, RelPath,
    RowKind, SheetId, SheetPlan, SourceId,
};
use crate::import::sheet::Sheet;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListCandidate {
    pub source: SourceId,
    pub file: RelPath,
    /// The plan sheet that reads this file.
    pub in_plan: Option<SheetId>,
    /// The series that holds that sheet; only in a plan of several series.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub series: Option<LevelId>,
    pub read: CandidateRead,
    /// 0 when the list did not read.
    pub rows: usize,
    pub columns: usize,
    /// `None` when no column pairs a row, even with every fold.
    pub best: Option<BestColumn>,
    /// Set when `best` pairs by file name.
    pub grain: Option<Grain>,
    /// Every column, in sheet order; empty when the list did not read.
    pub cells: Vec<ColumnCells>,
    /// The other candidates that pair some of the same groups, most first.
    pub overlaps: Vec<Overlap>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CandidateRead {
    Ok,
    Error(String),
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BestColumn {
    pub index: u32,
    pub header: String,
    pub on: JoinOn,
    /// Cells that exactly equal a group key (`identifier`) or a file's name
    /// or path (`fileName`).
    pub paired: usize,
    /// The same, with every `JoinNormalize` fold on.
    pub paired_normalized: usize,
    pub empty: usize,
}

/// The evidence that rows describe files rather than items.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Grain {
    /// Item groups that more than one paired row names a file of.
    pub groups_with_many_rows: usize,
    /// Rows naming a file of a group that an earlier row named another
    /// file of.
    pub rows_naming_other_files_of_a_group: usize,
    /// Every column of the list, in sheet order.
    pub columns: Vec<ColumnGrain>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnGrain {
    pub index: u32,
    pub header: String,
    pub varies: Varies,
}

/// Of the `of` groups that more than one row names, the `groups` whose rows
/// do not all hold the same values in this column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Varies {
    pub groups: usize,
    pub of: usize,
}

/// What one column's cells hold.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnCells {
    pub index: u32,
    pub header: String,
    /// Of the separators that cut a cell into several values, the one the
    /// most cells hold; `None` when no cell holds one.
    pub separator: Option<Separator>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Separator {
    pub text: String,
    pub cells: usize,
}

/// The separators counted, in the order a tie goes.
const SEPARATORS: [char; 3] = [',', ';', '|'];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Overlap {
    pub source: SourceId,
    pub file: RelPath,
    pub shared_targets: usize,
}

const NOT_FOUND: &str = "the file is not in its source";

/// Every list of every folder source, the best paired first, then by path.
pub(super) fn list_candidates(derived: &Derived) -> Vec<ListCandidate> {
    let exact = Targets::exact(derived);
    let folded = Targets::folded(derived);
    let groups = groups_of_files(derived);
    let mut found: Vec<(ListCandidate, HashSet<&GroupKey>)> = derived
        .stored
        .index
        .sources
        .iter()
        .filter_map(|source| Some((source, derived.tags.dirs.get(&source.id)?)))
        .flat_map(|(source, dir)| {
            source
                .walk
                .sheets
                .iter()
                .map(move |entry| (source, dir, entry))
        })
        .map(|(source, dir, entry)| {
            candidate(
                derived,
                &exact,
                &folded,
                &groups,
                &source.id,
                dir,
                &entry.path,
            )
        })
        .collect();
    let overlaps: Vec<Vec<Overlap>> = (0..found.len()).map(|at| overlaps_of(&found, at)).collect();
    for ((candidate, _), overlaps) in found.iter_mut().zip(overlaps) {
        candidate.overlaps = overlaps;
    }
    let mut candidates: Vec<ListCandidate> =
        found.into_iter().map(|(candidate, _)| candidate).collect();
    candidates.sort_by(|a, b| {
        Reverse(a.best.as_ref().map(|best| best.paired))
            .cmp(&Reverse(b.best.as_ref().map(|best| best.paired)))
            .then_with(|| a.file.cmp(&b.file))
            .then_with(|| a.source.cmp(&b.source))
    });
    candidates
}

/// How Simple adds a list found in a folder (`simple/list-plan.ts`,
/// `sheetOf`): the same read options make the same cache key.
pub(super) fn automatic(source: &SourceId, file: &RelPath) -> SheetPlan {
    SheetPlan {
        id: SheetId(String::new()),
        source: source.clone(),
        file: file.clone(),
        hash: None,
        read: ReadOptions {
            encoding: EncodingChoice::Auto(Auto::Auto),
            delimiter: Delimiter::Auto,
            quote: Quote::Double,
            header_line: std::num::NonZeroU32::MIN,
            trim: true,
            sheet_name: None,
        },
        row_kind: RowKind::All(RecordKind::Item),
        join: None,
        item_key: None,
        columns: Vec::new(),
    }
}

fn candidate<'d>(
    derived: &'d Derived,
    exact: &Targets<'d>,
    folded: &Targets<'d>,
    groups: &[Option<&'d GroupKey>],
    source: &SourceId,
    dir: &SourceDir,
    file: &RelPath,
) -> (ListCandidate, HashSet<&'d GroupKey>) {
    let structure = &derived.plan.structure;
    let held = structure
        .lists()
        .find(|(_, sheet)| sheet.source == *source && sheet.file == *file);
    let mut candidate = ListCandidate {
        source: source.clone(),
        file: file.clone(),
        in_plan: held.map(|(_, sheet)| sheet.id.clone()),
        series: held.and_then(|(at, _)| structure.series_names()[at.index()].cloned()),
        read: CandidateRead::Ok,
        rows: 0,
        columns: 0,
        best: None,
        grain: None,
        cells: Vec::new(),
        overlaps: Vec::new(),
    };
    let read = derived
        .tags
        .caches
        .sheets
        .read(&automatic(source, file), dir);
    let sheet = match read.as_deref().map(|read| &read.sheet) {
        None => {
            candidate.read = CandidateRead::Error(NOT_FOUND.to_string());
            return (candidate, HashSet::new());
        }
        Some(Err(error)) => {
            candidate.read = CandidateRead::Error(error.to_string());
            return (candidate, HashSet::new());
        }
        Some(Ok(sheet)) => sheet,
    };
    candidate.rows = sheet.rows.len();
    candidate.columns = sheet.columns.len();
    // The plan's column indexes are its own read's, which may differ from
    // the automatic one by header line or worksheet.
    let in_plan = candidate
        .in_plan
        .as_ref()
        .and_then(|id| derived.sheets.get(id))
        .and_then(|read| read.sheet.as_ref().ok());
    candidate.cells = column_cells(in_plan.unwrap_or(sheet));
    candidate.best = best(sheet, exact, folded);
    let Some(best) = &candidate.best else {
        return (candidate, HashSet::new());
    };
    let plan = candidate
        .in_plan
        .as_ref()
        .and_then(|id| derived.plan.structure.list(id))
        .map(|(_, sheet)| sheet);
    let (targets, grain) = targets(sheet, plan, best, exact, groups);
    candidate.grain = grain;
    (candidate, targets)
}

fn column_cells(sheet: &Sheet) -> Vec<ColumnCells> {
    sheet
        .columns
        .iter()
        .map(|column| {
            let separator = SEPARATORS
                .iter()
                .map(|separator| Separator {
                    text: separator.to_string(),
                    cells: sheet
                        .rows
                        .iter()
                        .filter(|row| cell(row, column.index).contains(*separator))
                        .count(),
                })
                .rev()
                .max_by_key(|separator| separator.cells)
                .filter(|separator| separator.cells > 0);
            ColumnCells {
                index: column.index,
                header: column.header.clone(),
                separator,
            }
        })
        .collect()
}

/// The column and join that pair the most rows exactly; a tie goes to the
/// one that pairs more folded, then to `fileName`, as Simple's join choice
/// does, then to the first column.
fn best(sheet: &Sheet, exact: &Targets<'_>, folded: &Targets<'_>) -> Option<BestColumn> {
    sheet
        .columns
        .iter()
        .flat_map(|column| {
            let exact = column_counts(sheet, column, exact);
            let folded = column_counts(sheet, column, folded);
            [
                (JoinOn::FileName, exact.file_names, folded.file_names),
                (JoinOn::Identifier, exact.group_keys, folded.group_keys),
            ]
            .map(|(on, paired, paired_normalized)| BestColumn {
                index: column.index,
                header: column.header.clone(),
                on,
                paired,
                paired_normalized,
                empty: exact.empty,
            })
        })
        .filter(|best| best.paired_normalized > 0)
        .max_by_key(|best| {
            (
                best.paired,
                best.paired_normalized,
                best.on == JoinOn::FileName,
                Reverse(best.index),
            )
        })
}

/// The groups `best` pairs exactly, and for a `fileName` join its grain.
/// `plan` is the plan sheet that reads the list, whose columns' splits the
/// values are cut by.
fn targets<'d>(
    sheet: &Sheet,
    plan: Option<&SheetPlan>,
    best: &BestColumn,
    exact: &Targets<'d>,
    groups_of_files: &[Option<&'d GroupKey>],
) -> (HashSet<&'d GroupKey>, Option<Grain>) {
    let cells = exact.cells(sheet, best.index);
    match best.on {
        JoinOn::Identifier => (
            cells.flatten().filter_map(|found| found.key).collect(),
            None,
        ),
        JoinOn::FileName => {
            let mut groups: HashMap<&GroupKey, (Vec<usize>, HashSet<usize>)> = HashMap::new();
            for (row, found) in cells.enumerate() {
                if let Some(file) = found.and_then(|found| found.file)
                    && let Some(group) = groups_of_files[file]
                {
                    let (rows, files) = groups.entry(group).or_default();
                    rows.push(row);
                    files.insert(file);
                }
            }
            let shared: Vec<&[usize]> = groups
                .values()
                .map(|(rows, _)| rows.as_slice())
                .filter(|rows| rows.len() > 1)
                .collect();
            let grain = Grain {
                groups_with_many_rows: shared.len(),
                rows_naming_other_files_of_a_group: groups
                    .values()
                    .map(|(_, files)| files.len() - 1)
                    .sum(),
                columns: sheet
                    .columns
                    .iter()
                    .map(|column| ColumnGrain {
                        index: column.index,
                        header: column.header.clone(),
                        varies: varies(sheet, plan, column.index, &shared),
                    })
                    .collect(),
            };
            (groups.into_keys().collect(), Some(grain))
        }
    }
}

/// How many of `shared` (each group's rows) hold different values in
/// `column`. Values are cut as a column rule cuts them: by the column's
/// split, each piece trimmed, empty pieces dropped. Order and repeats do not
/// count, as an item keeps each value of its rows once.
fn varies(sheet: &Sheet, plan: Option<&SheetPlan>, column: u32, shared: &[&[usize]]) -> Varies {
    let split = plan
        .and_then(|plan| plan.column(column))
        .and_then(|column| column.options.split.as_deref())
        .filter(|split| !split.is_empty());
    let values = |row: usize| -> BTreeSet<&str> {
        let text = cell(&sheet.rows[row], column);
        let pieces: Vec<&str> = match split {
            Some(split) => text.split(split).collect(),
            None => vec![text],
        };
        pieces
            .into_iter()
            .map(str::trim)
            .filter(|piece| !piece.is_empty())
            .collect()
    };
    Varies {
        groups: shared
            .iter()
            .filter(|rows| {
                let first = values(rows[0]);
                rows[1..].iter().any(|row| values(*row) != first)
            })
            .count(),
        of: shared.len(),
    }
}

/// Each file's item group, by its place in [`walk`]; `None` for a file in
/// no item, such as one left out.
fn groups_of_files(derived: &Derived) -> Vec<Option<&GroupKey>> {
    let mut group_of: HashMap<(&str, &str), &GroupKey> = HashMap::new();
    for record in &derived.model.records {
        if let (RecordKind::Item, Origin::Group(key)) = (record.kind, &record.origin) {
            for file in &record.files {
                group_of.insert((file.source.0.as_str(), file.path.as_str()), key);
            }
        }
    }
    walk(derived)
        .map(|(source, entry)| {
            group_of
                .get(&(source.0.as_str(), entry.path.as_str()))
                .copied()
        })
        .collect()
}

fn overlaps_of(found: &[(ListCandidate, HashSet<&GroupKey>)], at: usize) -> Vec<Overlap> {
    let mine = &found[at].1;
    let mut overlaps: Vec<Overlap> = found
        .iter()
        .enumerate()
        .filter(|(other, _)| *other != at)
        .filter_map(|(_, (candidate, theirs))| {
            let shared_targets = mine.intersection(theirs).count();
            (shared_targets > 0).then(|| Overlap {
                source: candidate.source.clone(),
                file: candidate.file.clone(),
                shared_targets,
            })
        })
        .collect();
    overlaps.sort_by(|a, b| {
        b.shared_targets
            .cmp(&a.shared_targets)
            .then_with(|| a.file.cmp(&b.file))
    });
    overlaps
}
