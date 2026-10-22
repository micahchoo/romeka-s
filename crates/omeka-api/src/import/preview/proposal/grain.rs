//! Detector 4, row grain (design section 2): what each list's rows
//! describe. A list joined by file name describes files when it names two
//! files or more of some item; else, and with an `identifier` join, items.
//!
//! A list that pairs no file has rows that are records. Its key column is
//! the first whose cells are all filled and all different. A column whose
//! every filled cell the engine reads as a record kind, with two kinds or
//! more, is its kind column, and each kind has its own key. A column whose
//! filled values are all key values of other rows is a reference: from
//! media rows to item rows it is the item key, from item rows to item-set
//! rows it is a set source. Such a list enters the plan when a list that
//! pairs every file of a series refers to it (the series' items are then
//! its rows), or when the archive has no media to pair at all.
//!
//! A column of row numbers (1 to n in row order, or n to 1) is never a
//! key or a reference: it would name the rows of any longer list.
//!
//! The record-kind words are the engine's (`values::row_kind_from_cell`);
//! this module holds none.

use std::collections::{BTreeSet, HashSet};

use unicode_normalization::UnicodeNormalization;

use super::lists::{Found, InSeries};
use super::thresholds::{MIN_ITEMS_WITH_FILE_ROWS, MIN_KINDS};
use super::{Choice, Evidence, GrainCounts, Pick};
use crate::import::join::cell;
use crate::import::plan::{
    ColumnRef, Edit, FolderScope, Join, JoinOn, KindColumn, ListFile, RecordKind, RowKind,
};
use crate::import::sheet::{Row, Sheet};
use crate::import::values::row_kind_from_cell;

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

/// The key column of some rows, and its values.
struct Keyed {
    column: ColumnRef,
    values: HashSet<String>,
}

/// A list whose rows pair no file, as records.
struct Records {
    file: ListFile,
    kind: Option<ColumnRef>,
    items: Option<Keyed>,
    sets: Option<Keyed>,
    /// From media rows to item rows.
    media_key: Option<ColumnRef>,
    /// From item rows to item-set rows.
    set_key: Option<ColumnRef>,
}

fn text(row: &Row, column: u32) -> String {
    cell(row, column).trim().nfc().collect()
}

fn column_ref(sheet: &Sheet, index: u32) -> Option<ColumnRef> {
    let column = sheet.columns.iter().find(|column| column.index == index)?;
    Some(ColumnRef {
        index: column.index,
        header: column.header.clone(),
    })
}

/// The first column whose every filled cell reads as a record kind, with
/// at least [`MIN_KINDS`] kinds.
fn kind_column(sheet: &Sheet) -> Option<ColumnRef> {
    let column = sheet.columns.iter().find(|column| {
        let mut kinds = BTreeSet::new();
        for row in &sheet.rows {
            let cell = cell(row, column.index).trim();
            if cell.is_empty() {
                continue;
            }
            match row_kind_from_cell(cell) {
                Some(kind) => kinds.insert(kind),
                None => return false,
            };
        }
        kinds.len() >= MIN_KINDS
    })?;
    column_ref(sheet, column.index)
}

/// Whether a column's cells are exactly the whole numbers 1 to n in row
/// order, or n to 1. Such a column numbers the rows and names nothing, so it
/// is never a key or a reference.
pub(super) fn numbers_rows(sheet: &Sheet, column: u32) -> bool {
    let rows = sheet.rows.len();
    let numbers: Option<Vec<usize>> = sheet
        .rows
        .iter()
        .map(|row| cell(row, column).trim().parse().ok())
        .collect();
    numbers.is_some_and(|numbers| {
        let up = numbers.iter().enumerate().all(|(at, n)| *n == at + 1);
        up || numbers.iter().enumerate().all(|(at, n)| *n == rows - at)
    })
}

/// The first column but `skip` whose cells in `rows` are all filled and all
/// different, with its values.
fn key(sheet: &Sheet, rows: &[&Row], skip: Option<u32>) -> Option<Keyed> {
    if rows.is_empty() {
        return None;
    }
    sheet
        .columns
        .iter()
        .filter(|column| Some(column.index) != skip && !numbers_rows(sheet, column.index))
        .find_map(|column| {
            let mut values = HashSet::new();
            for row in rows {
                let value = text(row, column.index);
                if value.is_empty() || !values.insert(value) {
                    return None;
                }
            }
            Some(Keyed {
                column: column_ref(sheet, column.index)?,
                values,
            })
        })
}

/// The first column but `skip` with a filled cell in `rows` whose filled
/// cells there are all among `keys`.
fn reference(
    sheet: &Sheet,
    rows: &[&Row],
    keys: &HashSet<String>,
    skip: Option<u32>,
) -> Option<ColumnRef> {
    let column = sheet
        .columns
        .iter()
        .filter(|column| Some(column.index) != skip && !numbers_rows(sheet, column.index))
        .find(|column| {
            let mut filled = rows
                .iter()
                .map(|row| text(row, column.index))
                .filter(|value| !value.is_empty())
                .peekable();
            filled.peek().is_some() && filled.all(|value| keys.contains(&value))
        })?;
    column_ref(sheet, column.index)
}

fn records(file: ListFile, sheet: &Sheet) -> Records {
    let kind = kind_column(sheet);
    let skip = kind.as_ref().map(|column| column.index);
    let rows_of = |wanted: RecordKind| -> Vec<&Row> {
        sheet
            .rows
            .iter()
            .filter(|row| match skip {
                Some(column) => row_kind_from_cell(cell(row, column)) == Some(wanted),
                None => wanted == RecordKind::Item,
            })
            .collect()
    };
    let items = key(sheet, &rows_of(RecordKind::Item), skip);
    let sets = kind
        .as_ref()
        .and_then(|_| key(sheet, &rows_of(RecordKind::ItemSet), skip));
    let media_key = kind
        .as_ref()
        .zip(items.as_ref())
        .and_then(|(_, items)| reference(sheet, &rows_of(RecordKind::Media), &items.values, skip));
    let set_key = sets
        .as_ref()
        .and_then(|sets| reference(sheet, &rows_of(RecordKind::Item), &sets.values, skip));
    Records {
        file,
        kind,
        items,
        sets,
        media_key,
        set_key,
    }
}

impl Records {
    /// How the plan reads it, and what the series takes from it.
    fn attach(self, rows: &mut SeriesRows) {
        let Some(items) = self.items else {
            return;
        };
        let mut plan = ListPlan {
            file: self.file.clone(),
            row_kind: RowKind::All(RecordKind::Item),
            join: None,
            item_key: Some(items.column.clone()),
            mapped: Vec::new(),
        };
        if let Some(kind) = self.kind {
            plan.row_kind = RowKind::Column(KindColumn { column: kind });
            plan.item_key = self.media_key;
            plan.mapped.push(items.column.clone());
            rows.keys
                .push((RecordKind::Item, self.file.clone(), items.column.clone()));
            if let Some(sets) = self.sets {
                if !plan.mapped.contains(&sets.column) {
                    plan.mapped.push(sets.column.clone());
                }
                rows.keys
                    .push((RecordKind::ItemSet, self.file.clone(), sets.column));
            }
            if let Some(set) = self.set_key {
                plan.mapped.push(set.clone());
                rows.set = Some((self.file.clone(), set));
            }
        }
        if rows.items.is_none() {
            rows.items = Some((self.file, items.column));
        }
        rows.lists.push(plan);
    }
}

/// Rule 1 and 2: what the rows of a list that pairs describe.
fn grain(one: &InSeries<'_>) -> (ListPlan, Option<Choice<RecordKind, GrainCounts>>) {
    let join = one.counted.join.clone();
    let mut describes = RecordKind::Item;
    let mut choice = None;
    let by_name = join
        .as_ref()
        .is_some_and(|join| join.on == JoinOn::FileName);
    if by_name && one.counted.paired == 0 {
        // Detector 3 rule 3: only a suggestion that pairs each row to one
        // file attaches it, so each row describes a file. Item rows would
        // each make an item of no file.
        describes = RecordKind::Media;
        let counts = GrainCounts {
            groups_with_many_rows: 0,
            rows_naming_other_files_of_a_group: 0,
            columns: Vec::new(),
        };
        choice = Some(Choice {
            chosen: Pick {
                value: RecordKind::Media,
                counts: counts.clone(),
            },
            rival: Some(Pick {
                value: RecordKind::Item,
                counts,
            }),
        });
    } else if let (
        Some(Join {
            on: JoinOn::FileName,
            ..
        }),
        Some(counts),
    ) = (&join, &one.counted.counts.grain)
    {
        let (chosen, rival) =
            if one.counted.groups_with_rows_for_many_files >= MIN_ITEMS_WITH_FILE_ROWS {
                (RecordKind::Media, RecordKind::Item)
            } else {
                (RecordKind::Item, RecordKind::Media)
            };
        describes = chosen;
        choice = Some(Choice {
            chosen: Pick {
                value: chosen,
                counts: counts.clone(),
            },
            rival: Some(Pick {
                value: rival,
                counts: counts.clone(),
            }),
        });
    }
    let plan = ListPlan {
        file: one.file.clone(),
        row_kind: RowKind::All(describes),
        join,
        item_key: None,
        mapped: Vec::new(),
    };
    (plan, choice)
}

/// Each series, as detector 4 reads it.
pub(super) struct SeriesRead<'s> {
    /// The scope its evidence names.
    pub scope: &'s FolderScope,
    /// The media in its scope.
    pub media: usize,
    /// A person chose its cut: its items do not move to rows.
    pub cut_chosen: bool,
}

/// Detector 4 on what detector 3 chose. Lists that become records leave
/// `found.unattached`. The plan's `grain` edits are replayed; each one it
/// applies is marked in `applied`.
pub(super) fn detect(
    evidence: &Evidence<'_>,
    found: &mut Found<'_>,
    series: &[SeriesRead<'_>],
    edits: &[Edit],
    applied: &mut [bool],
) -> Vec<SeriesRows> {
    let mut candidates: Vec<Records> = Vec::new();
    for list in found.unattached.iter().filter(|list| list.pairs_nothing) {
        evidence.lists.read(&list.file, &mut |sheet| {
            if let Ok(sheet) = sheet {
                candidates.push(records(list.file.clone(), sheet));
            }
        });
    }
    let mut claimed: Vec<Option<usize>> = candidates.iter().map(|_| None).collect();
    let mut out: Vec<SeriesRows> = Vec::new();
    for (at, (lists, one)) in found.series.iter().zip(series).enumerate() {
        let mut rows = SeriesRows::default();
        for (n, list) in lists.joined.iter().enumerate() {
            let (plan, choice) = grain(list);
            if n == 0 {
                rows.grain = choice;
            }
            rows.lists.push(plan);
        }
        let pairs_every_file = lists.joined.iter().position(|list| {
            list.counted
                .join
                .as_ref()
                .is_some_and(|join| join.on == JoinOn::FileName)
                && list.counted.files == one.media
        });
        if let Some(n) = pairs_every_file.filter(|_| !one.cut_chosen && one.media > 0) {
            let join_column = rows.lists[n].join.as_ref().map(|join| join.column.index);
            let mut referred = None;
            evidence.lists.read(&lists.joined[n].file, &mut |sheet| {
                let Ok(sheet) = sheet else {
                    return;
                };
                let all: Vec<&Row> = sheet.rows.iter().collect();
                referred = candidates
                    .iter()
                    .enumerate()
                    .find_map(|(found_at, records)| {
                        let items = records.items.as_ref()?;
                        if claimed[found_at].is_some() || records.kind.is_some() {
                            return None;
                        }
                        Some((
                            found_at,
                            reference(sheet, &all, &items.values, join_column)?,
                        ))
                    });
            });
            if let Some((found_at, column)) = referred {
                claimed[found_at] = Some(at);
                rows.lists[n].row_kind = RowKind::All(RecordKind::Media);
                rows.lists[n].item_key = Some(column);
                if n == 0 {
                    rows.grain = None;
                }
            }
        }
        out.push(rows);
    }
    let no_media = series.iter().all(|one| one.media == 0);
    for (found_at, records) in candidates.iter().enumerate() {
        if no_media && claimed[found_at].is_none() && records.items.is_some() {
            claimed[found_at] = series.iter().position(|one| {
                one.scope.source == records.file.source && one.scope.folder.as_str().is_empty()
            });
        }
    }
    for (records, at) in candidates.into_iter().zip(&claimed) {
        let Some(at) = *at else {
            continue;
        };
        found.unattached.retain(|list| list.file != records.file);
        records.attach(&mut out[at]);
    }
    replay(&mut out, edits, applied);
    out
}

/// Replay the `grain` edits: each takes the rival grain of a series' first
/// list. Each edit it applies is marked in `applied`.
fn replay(out: &mut [SeriesRows], edits: &[Edit], applied: &mut [bool]) {
    for (at, edit) in edits.iter().enumerate() {
        let Edit::Grain { list, describes } = edit else {
            continue;
        };
        for rows in out.iter_mut() {
            let first = rows.lists.first_mut().filter(|first| first.file == *list);
            let (Some(first), Some(choice)) = (first, rows.grain.as_mut()) else {
                continue;
            };
            if choice.chosen.value == *describes {
                applied[at] = true;
            } else if let Some(rival) = choice.rival.take_if(|rival| rival.value == *describes) {
                choice.rival = Some(std::mem::replace(&mut choice.chosen, rival));
                first.row_kind = RowKind::All(*describes);
                applied[at] = true;
            }
        }
    }
}
