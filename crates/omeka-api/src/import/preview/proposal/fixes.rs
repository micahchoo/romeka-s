//! The corrections (`briefs/import-gaps-design.md`): the three edits that
//! name files and rows, never rules. The replay writes each edit into the
//! series it corrects as [`Correction`]s, which derive reads as decisions.

use std::collections::{BTreeMap, HashMap};

use serde::Serialize;

use super::folders::FolderSeries;
use crate::import::decisions::{FileRef, RowRef};
use crate::import::derive::{Index, Sheets};
use crate::import::model::{Model, Parent, RecordId};
use crate::import::plan::{
    Correction, Edit, FolderScope, ItemJoin, ListFile, RecordKind, RelPath, RowCell, SeriesIx,
    SheetPlan, Structure,
};
use crate::import::sheet::{Sheet, key_column, plan_keys};

/// An instance of a correction that no longer holds, by its place in the
/// edit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Stale {
    pub at: usize,
    pub why: StaleWhy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum StaleWhy {
    /// The line holds another row now, or no row.
    RowChanged,
    /// A file it names is not in the scan.
    FileGone,
}

/// What the replay made: each series' corrections, by its place, and the
/// instances of each edit that no longer hold.
#[derive(Debug, Default)]
pub(super) struct Replayed {
    pub corrections: BTreeMap<SeriesIx, Vec<Correction>>,
    pub stale: Vec<Vec<Stale>>,
}

impl Replayed {
    /// `structure` with each series' corrections on it.
    pub fn onto(&self, mut structure: Structure) -> Structure {
        for (at, corrections) in &self.corrections {
            structure.set_corrections(*at, corrections.clone());
        }
        structure
    }
}

/// What the replay reads: the tree with its lists, the series detector 1
/// made, in the tree's order, a derive of that tree's item keys with no
/// correction, and the lists as read.
pub(super) struct Seen<'e> {
    pub structure: &'e Structure,
    pub series: &'e [FolderSeries],
    pub items: &'e Model,
    pub sheets: &'e Sheets,
    pub index: &'e Index,
}

/// Replay each correction edit. An edit is applied when its series or list
/// is in the tree and every instance holds; the instances that hold are
/// written even when some do not.
pub(super) fn replay(seen: &Seen<'_>, edits: &[Edit], applied: &mut [bool]) -> Replayed {
    let mut replayed = Replayed {
        stale: vec![Vec::new(); edits.len()],
        ..Replayed::default()
    };
    let items = ItemsOf::new(seen.items);
    for (at, edit) in edits.iter().enumerate() {
        let (series, made, stale) = match edit {
            Edit::OneItem { scope, joins } => {
                let Some(series) = series_of(seen, scope) else {
                    continue;
                };
                let (made, stale) = joins_of(&items, scope, joins);
                (series, made, stale)
            }
            Edit::RowIs { list, pairs } => {
                let Some((series, plan, sheet)) = list_of(seen, list) else {
                    continue;
                };
                let rows = Rows::new(plan, sheet);
                let mut made = Vec::new();
                let mut stale = Vec::new();
                for (place, pair) in pairs.iter().enumerate() {
                    let file = FileRef {
                        source: list.source.clone(),
                        path: pair.file.clone(),
                    };
                    match rows.find(&pair.row) {
                        None => stale.push(Stale {
                            at: place,
                            why: StaleWhy::RowChanged,
                        }),
                        Some(_) if !in_index(seen.index, &file) => stale.push(Stale {
                            at: place,
                            why: StaleWhy::FileGone,
                        }),
                        Some(row) => made.push(Correction::Pair { row, file }),
                    }
                }
                (series, made, stale)
            }
            Edit::DescribesNothing { list, rows: named } => {
                let Some((series, plan, sheet)) = list_of(seen, list) else {
                    continue;
                };
                let rows = Rows::new(plan, sheet);
                let mut made = Vec::new();
                let mut stale = Vec::new();
                for (place, cell) in named.iter().enumerate() {
                    match rows.find(cell) {
                        Some(row) => made.push(Correction::Leave { row }),
                        None => stale.push(Stale {
                            at: place,
                            why: StaleWhy::RowChanged,
                        }),
                    }
                }
                (series, made, stale)
            }
            _ => continue,
        };
        applied[at] = stale.is_empty();
        replayed.stale[at] = stale;
        replayed.corrections.entry(series).or_default().extend(made);
    }
    replayed
}

/// The series of the tree that `scope` names, when it is not written by
/// hand.
fn series_of(seen: &Seen<'_>, scope: &FolderScope) -> Option<SeriesIx> {
    seen.structure
        .all_series()
        .zip(seen.series)
        .find(|(_, one)| one.own.is_none() && one.names(scope))
        .map(|((at, _), _)| at)
}

/// The series that holds `list`, the list as the tree states it, and the
/// list as read.
fn list_of<'s>(seen: &Seen<'s>, list: &ListFile) -> Option<(SeriesIx, &'s SheetPlan, &'s Sheet)> {
    let (at, plan) = seen
        .structure
        .lists()
        .find(|(_, plan)| plan.source == list.source && plan.file == list.file)?;
    let sheet = seen.sheets.get(&plan.id)?.as_ref().ok()?;
    Some((at, plan, sheet))
}

fn in_index(index: &Index, file: &FileRef) -> bool {
    index
        .sources
        .iter()
        .filter(|source| source.id == file.source)
        .any(|source| {
            source
                .walk
                .entries
                .iter()
                .any(|entry| entry.path == file.path)
        })
}

/// A list's rows by line, each with its join cell and the key the plan
/// reads it by.
struct Rows<'s> {
    plan: &'s SheetPlan,
    by_line: HashMap<u32, (usize, Option<&'s str>)>,
    keys: Vec<crate::import::decisions::RowKey>,
}

impl<'s> Rows<'s> {
    fn new(plan: &'s SheetPlan, sheet: &'s Sheet) -> Self {
        let column = key_column(plan).map(|column| column.index as usize);
        Self {
            plan,
            by_line: sheet
                .rows
                .iter()
                .enumerate()
                .map(|(at, row)| {
                    let cell = column
                        .and_then(|column| row.cells.get(column))
                        .map(|cell| cell.trim());
                    (row.line, (at, cell))
                })
                .collect(),
            keys: plan_keys(&sheet.rows, plan),
        }
    }

    /// The row at the cell's line, when its join cell is still that text.
    /// A list with no join cell names no row this way.
    fn find(&self, cell: &RowCell) -> Option<RowRef> {
        let (at, text) = self.by_line.get(&cell.line)?;
        (*text == Some(cell.cell.trim())).then(|| RowRef {
            sheet: self.plan.id.clone(),
            key: self.keys[*at].clone(),
        })
    }
}

/// Each file's item, and each item's files, in a derive of item keys.
struct ItemsOf {
    item: HashMap<FileRef, RecordId>,
    files: HashMap<RecordId, Vec<FileRef>>,
}

impl ItemsOf {
    fn new(model: &Model) -> Self {
        let mut item = HashMap::new();
        let mut files: HashMap<RecordId, Vec<FileRef>> = HashMap::new();
        for record in model
            .records
            .iter()
            .filter(|record| record.kind == RecordKind::Media)
        {
            let Some(Parent::Record(parent)) = record.parent else {
                continue;
            };
            for file in &record.files {
                item.insert(file.clone(), parent);
                files.entry(parent).or_default().push(file.clone());
            }
        }
        Self { item, files }
    }
}

/// The corrections of a `oneItem` edit: every file of the item that holds
/// `with` joins the item that holds `item`.
fn joins_of(
    items: &ItemsOf,
    scope: &FolderScope,
    joins: &[ItemJoin],
) -> (Vec<Correction>, Vec<Stale>) {
    let mut made = Vec::new();
    let mut stale = Vec::new();
    let file = |path: &RelPath| FileRef {
        source: scope.source.clone(),
        path: path.clone(),
    };
    for (place, join) in joins.iter().enumerate() {
        let (item, with) = (file(&join.item), file(&join.with));
        let (Some(into), Some(from)) = (items.item.get(&item), items.item.get(&with)) else {
            stale.push(Stale {
                at: place,
                why: StaleWhy::FileGone,
            });
            continue;
        };
        if into == from {
            continue;
        }
        for moved in items.files.get(from).into_iter().flatten() {
            made.push(Correction::Join {
                file: moved.clone(),
                with: item.clone(),
            });
        }
    }
    (made, stale)
}
