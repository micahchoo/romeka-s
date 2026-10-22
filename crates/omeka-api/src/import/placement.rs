//! What an import places where, with every name left out: the oracle that
//! judges a proposal (`briefs/import-proposer-design.md`, section 4). Two
//! plans are equal here when they put the same files in the same items, the
//! items in the same sets, each list on the same items, and the same rows on
//! the same items. The comparison never reads a pattern, an id or a title.
//! Nor does it compare how items are split into series: a series is how a
//! plan is organized and never shows in Omeka, and a series split that
//! changes an item shows as a file, set, list or row difference.
//!
//! Two adapters build a [`Placement`]: [`Placement::of_model`] for the
//! model-level tests, and the server's shape harness from the preview JSON.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use super::decisions::{EntryRef, FileRef, RowKey, RowRef};
use super::derive::Sheets;
use super::model::{Fate, Model, Parent, Record, RecordId};
use super::plan::{ImportPlan, ListFile, RecordKind, RelPath, SeriesIx, SheetId, SourceId};

/// A file by where it is.
pub type FileAt = (SourceId, RelPath);

/// A list row by its list and the 1-based line it starts on. The line, not
/// the row key, because the key depends on which column a plan keys by.
pub type RowAt = (ListFile, usize);

/// An item by its least file, or by its least row when it has no file. The
/// least, not the first in media order, so that the media order a plan
/// states does not change which item is which.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ItemAt {
    File(FileAt),
    Row(RowAt),
}

/// What an import places where, with every name left out.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Placement {
    /// Each item as its files; an item made from rows only has none.
    pub items: BTreeMap<ItemAt, BTreeSet<FileAt>>,
    /// The items of each series that has any, as a partition. Kept to
    /// describe a plan; [`differences`] does not compare it.
    pub series: BTreeSet<BTreeSet<ItemAt>>,
    /// The items of each set. A set that holds every item is left out: it
    /// places nothing the import does not already place.
    pub sets: BTreeSet<BTreeSet<ItemAt>>,
    /// Each list with the items of the series it describes.
    pub lists: BTreeMap<ListFile, BTreeSet<ItemAt>>,
    /// Each paired row with its item, and its file when the row describes
    /// one file of an item that has several. For an item of one file, rows
    /// that describe the item and rows that describe its file place alike.
    pub rows: BTreeMap<RowAt, (ItemAt, Option<FileAt>)>,
    /// The rows that made item-set records.
    pub set_rows: BTreeSet<RowAt>,
    /// The files in no item.
    pub left_out: BTreeSet<FileAt>,
}

/// Each way `got` differs from `want`, with the paths that show it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Difference {
    /// The files of one wanted item are in several items.
    FilesSplit {
        want: BTreeSet<FileAt>,
        got: Vec<BTreeSet<FileAt>>,
    },
    /// The files of several wanted items are in one item.
    FilesMerged {
        want: Vec<BTreeSet<FileAt>>,
        got: BTreeSet<FileAt>,
    },
    /// An item made from rows only that one side makes and the other does
    /// not; `want_made` says which.
    RowItem {
        item: ItemAt,
        want_made: bool,
    },
    /// A set one side makes and the other does not.
    Set {
        want: Option<BTreeSet<ItemAt>>,
        got: Option<BTreeSet<ItemAt>>,
    },
    /// The list describes other items, or none.
    List {
        list: ListFile,
    },
    /// The row is paired otherwise, or it makes an item set on one side only.
    Row {
        row: RowAt,
    },
    LeftOut {
        file: FileAt,
        want_left_out: bool,
    },
}

/// The plan's lists by sheet id: where each is, and the line of each row.
pub struct ListFiles(BTreeMap<SheetId, (ListFile, HashMap<RowKey, usize>)>);

impl ListFiles {
    /// The plan's lists, with the rows of each as `sheets` read them. A list
    /// that did not read has no rows.
    #[must_use]
    pub fn new(plan: &ImportPlan, sheets: &Sheets) -> Self {
        Self(
            plan.structure
                .lists()
                .map(|(_, sheet)| {
                    let rows = sheets
                        .get(&sheet.id)
                        .and_then(|read| read.as_ref().ok())
                        .map(|read| {
                            read.rows
                                .iter()
                                .map(|row| (row.key.clone(), row.line as usize))
                                .collect()
                        })
                        .unwrap_or_default();
                    let file = ListFile {
                        source: sheet.source.clone(),
                        file: sheet.file.clone(),
                    };
                    (sheet.id.clone(), (file, rows))
                })
                .collect(),
        )
    }

    /// Where the plan's list `sheet` is.
    #[must_use]
    pub fn list(&self, sheet: &SheetId) -> Option<&ListFile> {
        self.0.get(sheet).map(|(file, _)| file)
    }

    /// The list and line of a row, or `None` for a row its list does not hold.
    #[must_use]
    pub fn row(&self, row: &RowRef) -> Option<RowAt> {
        let (file, lines) = self.0.get(&row.sheet)?;
        Some((file.clone(), *lines.get(&row.key)?))
    }
}

fn file_at(file: &FileRef) -> FileAt {
    (file.source.clone(), file.path.clone())
}

fn is_kept(record: &Record) -> bool {
    !matches!(record.fate, Fate::Left(_))
}

impl Placement {
    /// From a derived model and the plan that made it.
    #[must_use]
    pub fn of_model(model: &Model, plan: &ImportPlan, lists: &ListFiles) -> Self {
        let mut placement = Self::default();
        let mut item_of: HashMap<RecordId, ItemAt> = HashMap::new();
        let mut series: BTreeMap<SeriesIx, BTreeSet<ItemAt>> = BTreeMap::new();
        let items = model
            .kept()
            .filter(|record| record.kind == RecordKind::Item);
        for record in items {
            let files: BTreeSet<FileAt> = record
                .media
                .iter()
                .map(|media| model.record(*media))
                .filter(|media| is_kept(media))
                .flat_map(|media| media.files.iter().map(file_at))
                .collect();
            let at = match files.first() {
                Some(file) => ItemAt::File(file.clone()),
                None => match record.rows.iter().filter_map(|row| lists.row(row)).min() {
                    Some(row) => ItemAt::Row(row),
                    None => continue,
                },
            };
            item_of.insert(record.id, at.clone());
            series.entry(record.series).or_default().insert(at.clone());
            placement.items.insert(at, files);
        }
        placement.series = series.into_values().collect();

        let mut sets: BTreeMap<_, BTreeSet<ItemAt>> = BTreeMap::new();
        for record in model.kept() {
            let Some(at) = item_of.get(&record.id) else {
                continue;
            };
            for set in &record.resolved.sets {
                sets.entry(*set).or_default().insert(at.clone());
            }
        }
        placement.sets = sets
            .into_values()
            .filter(|set| set.len() < placement.items.len())
            .collect();

        for (at, sheet) in plan.structure.lists() {
            if let Some(list) = lists.list(&sheet.id) {
                let described = model
                    .kept()
                    .filter(|record| record.series == at)
                    .filter_map(|record| item_of.get(&record.id).cloned())
                    .collect();
                placement.lists.insert(list.clone(), described);
            }
        }

        for record in model.kept() {
            let rows = record.rows.iter().filter_map(|row| lists.row(row));
            match record.kind {
                RecordKind::ItemSet => placement.set_rows.extend(rows),
                RecordKind::Item => {
                    if let Some(at) = item_of.get(&record.id) {
                        placement
                            .rows
                            .extend(rows.map(|row| (row, (at.clone(), None))));
                    }
                }
                RecordKind::Media => {
                    let Some(Parent::Record(parent)) = record.parent else {
                        continue;
                    };
                    let Some(at) = item_of.get(&parent) else {
                        continue;
                    };
                    let several = placement.items.get(at).is_some_and(|files| files.len() > 1);
                    let file = record.files.first().map(file_at).filter(|_| several);
                    placement
                        .rows
                        .extend(rows.map(|row| (row, (at.clone(), file.clone()))));
                }
            }
        }

        let placed: BTreeSet<&FileAt> = placement.items.values().flatten().collect();
        placement.left_out = model
            .entries
            .iter()
            .filter_map(|entry| match &entry.entry {
                EntryRef::File(file) => Some(file_at(file)),
                EntryRef::Row(_) => None,
            })
            .filter(|file| !placed.contains(file))
            .collect();
        placement
    }
}

/// Each way `got` differs from `want`. Empty means equal.
///
/// Items are matched by their files first. Sets, lists and rows are
/// then compared on the items both sides make alike, so one split item is
/// one difference and not one per set it is in.
#[must_use]
pub fn differences(want: &Placement, got: &Placement) -> Vec<Difference> {
    let mut found = Vec::new();
    let item_of = |placement: &Placement| -> BTreeMap<FileAt, ItemAt> {
        placement
            .items
            .iter()
            .flat_map(|(at, files)| files.iter().map(move |file| (file.clone(), at.clone())))
            .collect()
    };
    let (want_of, got_of) = (item_of(want), item_of(got));
    let spread = |files: &BTreeSet<FileAt>,
                  into: &BTreeMap<FileAt, ItemAt>,
                  side: &Placement|
     -> Vec<BTreeSet<FileAt>> {
        let ats: BTreeSet<&ItemAt> = files.iter().filter_map(|file| into.get(file)).collect();
        ats.into_iter()
            .filter_map(|at| side.items.get(at).cloned())
            .collect()
    };
    for files in want.items.values().filter(|files| !files.is_empty()) {
        let got_items = spread(files, &got_of, got);
        if got_items.len() > 1 {
            found.push(Difference::FilesSplit {
                want: files.clone(),
                got: got_items,
            });
        }
    }
    for files in got.items.values().filter(|files| !files.is_empty()) {
        let want_items = spread(files, &want_of, want);
        if want_items.len() > 1 {
            found.push(Difference::FilesMerged {
                want: want_items,
                got: files.clone(),
            });
        }
    }
    let row_items = |placement: &Placement| -> BTreeSet<ItemAt> {
        placement
            .items
            .iter()
            .filter(|(_, files)| files.is_empty())
            .map(|(at, _)| at.clone())
            .collect()
    };
    let (want_rows, got_rows) = (row_items(want), row_items(got));
    for item in want_rows.symmetric_difference(&got_rows) {
        found.push(Difference::RowItem {
            item: item.clone(),
            want_made: want_rows.contains(item),
        });
    }
    for file in want.left_out.symmetric_difference(&got.left_out) {
        found.push(Difference::LeftOut {
            file: file.clone(),
            want_left_out: want.left_out.contains(file),
        });
    }

    let alike: BTreeSet<&ItemAt> = want
        .items
        .iter()
        .filter(|(at, files)| got.items.get(*at) == Some(*files))
        .map(|(at, _)| at)
        .collect();
    let on_alike = |items: &BTreeSet<ItemAt>| -> BTreeSet<ItemAt> {
        items
            .iter()
            .filter(|at| alike.contains(at))
            .cloned()
            .collect()
    };
    let sets = |placement: &Placement| -> BTreeSet<BTreeSet<ItemAt>> {
        placement
            .sets
            .iter()
            .map(on_alike)
            .filter(|set| !set.is_empty())
            .collect()
    };
    let (want_sets, got_sets) = (sets(want), sets(got));
    for set in want_sets.difference(&got_sets) {
        found.push(Difference::Set {
            want: Some(set.clone()),
            got: None,
        });
    }
    for set in got_sets.difference(&want_sets) {
        found.push(Difference::Set {
            want: None,
            got: Some(set.clone()),
        });
    }

    let lists: BTreeSet<&ListFile> = want.lists.keys().chain(got.lists.keys()).collect();
    for list in lists {
        let described = |placement: &Placement| placement.lists.get(list).map(on_alike);
        if described(want) != described(got) {
            found.push(Difference::List { list: list.clone() });
        }
    }

    let rows: BTreeSet<&RowAt> = want.rows.keys().chain(got.rows.keys()).collect();
    for row in rows {
        let (want_row, got_row) = (want.rows.get(row), got.rows.get(row));
        // A row on an item one side makes otherwise is that item's
        // difference, already found by its files.
        let consequence = [want_row, got_row]
            .into_iter()
            .flatten()
            .all(|(at, _)| !alike.contains(at));
        if want_row != got_row && !(want_row.is_some() && got_row.is_some() && consequence) {
            found.push(Difference::Row { row: row.clone() });
        }
    }
    for row in want.set_rows.symmetric_difference(&got.set_rows) {
        found.push(Difference::Row { row: row.clone() });
    }
    found
}
