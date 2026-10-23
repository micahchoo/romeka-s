//! The corrections (`briefs/import-gaps-design.md`): the three edits that
//! name files and rows, never rules. The replay writes each edit into the
//! series it corrects as [`Correction`]s, which derive reads as decisions.
//!
//! Detectors 7 to 9 find each correction's candidates on the derive that
//! counts the proposal, so an instance already taken is no longer one.
//! They count and compare places and characters; none names a token.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use serde::Serialize;

use super::folders::FolderSeries;
use super::thresholds::{MAX_EDITS_APART, MIN_SUGGESTION_SHARE};
use super::{ItemCut, Share};
use crate::import::decisions::{EntryRef, FileRef, RowRef};
use crate::import::derive::{Index, Sheets};
use crate::import::model::{GroupKey, LeftBy, Model, Origin, Parent, RecordId};
use crate::import::places::{PlaceClass, places, stem};
use crate::import::plan::{
    Correction, Edit, FolderScope, ItemJoin, ListFile, RecordKind, RelPath, RowCell, RowKind,
    RowPair, SeriesIx, SheetPlan, Structure,
};
use crate::import::sheet::{Row, Sheet, key_column, plan_keys};

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

/// Corrections the proposer offers on one series and its list. Offered,
/// never chosen: a person takes one with one click, every instance ticked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "fix", rename_all = "camelCase")]
pub enum Fix {
    /// These items are one item.
    OneItem {
        joins: Vec<JoinOffer>,
        counts: FixCounts,
    },
    /// These rows are these files.
    RowIs {
        list: ListFile,
        pairs: Vec<PairOffer>,
        counts: FixCounts,
    },
    /// These rows describe nothing.
    DescribesNothing {
        list: ListFile,
        rows: Vec<RowOffer>,
        counts: FixCounts,
    },
}

impl Fix {
    /// The edit that takes every instance of the offer, on the series of
    /// `scope`.
    #[must_use]
    pub fn edit(&self, scope: &FolderScope) -> Edit {
        match self {
            Self::OneItem { joins, .. } => Edit::OneItem {
                scope: scope.clone(),
                joins: joins.iter().map(|offer| offer.join.clone()).collect(),
            },
            Self::RowIs { list, pairs, .. } => Edit::RowIs {
                list: list.clone(),
                pairs: pairs.iter().map(|offer| offer.pair.clone()).collect(),
            },
            Self::DescribesNothing { list, rows, .. } => Edit::DescribesNothing {
                list: list.clone(),
                rows: rows.iter().map(|offer| offer.row.clone()).collect(),
            },
        }
    }
}

/// Why an instance is offered: the relation that found it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Because {
    /// The joining item's key is the other's and fewer places more.
    KeyExtends,
    /// A row that pairs no file starts the keys of these items.
    RowNamesBoth,
    /// The row's name and the file's differ by at most
    /// [`MAX_EDITS_APART`] edits.
    Near,
    /// The row's places are the item's key.
    NamesItem,
    /// The row's places are the item's key and a part that one file has.
    NamesPart,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JoinOffer {
    pub join: ItemJoin,
    pub because: Because,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PairOffer {
    pub pair: RowPair,
    pub because: Because,
}

/// A row that points at no file with no row by any relation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RowOffer {
    pub row: RowCell,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FixCounts {
    /// The instances offered.
    pub instances: usize,
    /// Items in the series, before and after the click.
    pub items: (usize, usize),
    /// Rows of the list paired to a file, before and after the click.
    pub rows: Option<(Share, Share)>,
}

/// One series as the detectors read it.
pub(super) struct Here<'e> {
    pub at: SeriesIx,
    pub scope: &'e FolderScope,
    pub cut: &'e ItemCut,
    /// The list has a pairing offer (detector 6), which explains it better
    /// than pairing row by row.
    pub pairing_offered: bool,
}

/// Detectors 7 to 9 on one series.
pub(super) fn detect(
    model: &Model,
    structure: &Structure,
    sheets: &Sheets,
    here: &Here<'_>,
) -> Vec<Fix> {
    let items = series_items(model, here);
    let mut fixes = Vec::new();
    let list = structure
        .series(here.at)
        .lists()
        .first()
        .and_then(|plan| Some((plan, sheets.get(&plan.id)?.as_ref().ok()?)));
    let read = list.map(|(plan, sheet)| ListRead::new(model, &items, plan, sheet));
    if matches!(here.cut, ItemCut::Whole | ItemCut::Places { .. })
        && let Some(fix) = one_item(&items, read.as_ref(), here)
    {
        fixes.push(fix);
    }
    if let Some(read) = read.filter(ListRead::works) {
        if !here.pairing_offered
            && let Some(fix) = row_is(&items, &read)
        {
            fixes.push(fix);
        }
        if let Some(fix) = describes_nothing(&items, &read) {
            fixes.push(fix);
        }
    }
    fixes
}

/// A name's places as text, letters case-folded: what the detectors
/// compare.
fn place_texts(text: &str) -> Vec<String> {
    places(text)
        .iter()
        .map(|place| text[place.start..place.end].to_lowercase())
        .collect()
}

/// The texts a cell may name a file by: the cell, and the cell with its
/// ending left out when it has one. An ending is one place of letters
/// after the last dot: in `2019.4.21` the `21` is a place of the name.
fn cell_names(cell: &str) -> Vec<&str> {
    let mut names = vec![cell];
    let ending = std::path::Path::new(cell)
        .extension()
        .and_then(std::ffi::OsStr::to_str)
        .unwrap_or_default();
    let letters = matches!(places(ending).as_slice(),
        [one] if one.class == PlaceClass::Letters && one.start == 0 && one.end == ending.len());
    if letters {
        names.push(stem(cell));
    }
    names
}

/// Whether two names are at most [`MAX_EDITS_APART`] edits apart, letters
/// case-folded. An edit is one character changed, added or removed, or two
/// neighbours swapped.
fn near(a: &str, b: &str) -> bool {
    let a: Vec<char> = a.to_lowercase().chars().collect();
    let b: Vec<char> = b.to_lowercase().chars().collect();
    edits_apart(&a, &b) <= MAX_EDITS_APART
}

/// The optimal string alignment distance: edits with neighbour swaps.
fn edits_apart(a: &[char], b: &[char]) -> usize {
    let width = b.len() + 1;
    let mut cost = vec![0; (a.len() + 1) * width];
    for i in 0..=a.len() {
        for j in 0..=b.len() {
            cost[i * width + j] = if i == 0 {
                j
            } else if j == 0 {
                i
            } else {
                let (before_i, before_j) = (i - 1, j - 1);
                let changed = usize::from(a[before_i] != b[before_j]);
                let mut best = (cost[before_i * width + j] + 1)
                    .min(cost[i * width + before_j] + 1)
                    .min(cost[before_i * width + before_j] + changed);
                let swapped = before_i > 0
                    && before_j > 0
                    && a[before_i] == b[before_j - 1]
                    && a[before_i - 1] == b[before_j];
                if swapped {
                    best = best.min(cost[(before_i - 1) * width + before_j - 1] + 1);
                }
                best
            };
        }
    }
    cost[a.len() * width + b.len()]
}

/// Whether `extra` places after `key` are fewer than the key's: the
/// short-parts guard of detector 2.
fn short_extension(key: &[String], longer: &[String]) -> bool {
    longer.len() > key.len() && longer.starts_with(key) && longer.len() - key.len() < key.len()
}

/// One item of the series on the counting derive.
struct Item {
    id: RecordId,
    /// Its key's places; empty for a key the cut did not make by name.
    key: Vec<String>,
    files: Vec<FileRef>,
}

fn series_items(model: &Model, here: &Here<'_>) -> Vec<Item> {
    model
        .kept()
        .filter(|record| record.kind == RecordKind::Item && record.series == here.at)
        .map(|record| Item {
            id: record.id,
            key: match &record.origin {
                Origin::Group(key @ GroupKey::Pattern(_)) => place_texts(key.text()),
                _ => Vec::new(),
            },
            files: record.files.clone(),
        })
        .collect()
}

/// Where a row of the list landed.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Landed {
    /// On a record with a file: on this item.
    Paired(RecordId),
    /// It made an item with no file.
    Alone(RecordId),
    /// It made no record.
    Nowhere,
}

/// One row of the list, as the detectors read it.
struct ListRow<'s> {
    row: &'s Row,
    cell: &'s str,
    landed: Landed,
}

/// The series' list as read, and where each row landed on the counting
/// derive.
struct ListRead<'s> {
    file: ListFile,
    /// The rows describe files, not items.
    of_files: bool,
    rows: Vec<ListRow<'s>>,
    /// The item that holds each of the list's rows, by item.
    rows_on: HashMap<RecordId, usize>,
    /// The files of the series that a row of this list describes.
    files_with_row: BTreeSet<FileRef>,
}

impl<'s> ListRead<'s> {
    fn new(model: &Model, items: &[Item], plan: &'s SheetPlan, sheet: &'s Sheet) -> Self {
        let item_ids: BTreeSet<RecordId> = items.iter().map(|item| item.id).collect();
        let states: HashMap<&RowRef, (Option<RecordId>, Option<LeftBy>)> = model
            .entries
            .iter()
            .filter_map(|state| match &state.entry {
                EntryRef::Row(row) if row.sheet == plan.id => {
                    Some((row, (state.record, state.left)))
                }
                _ => None,
            })
            .collect();
        let column = key_column(plan).map(|column| column.index as usize);
        let mut rows = Vec::new();
        let mut rows_on: HashMap<RecordId, usize> = HashMap::new();
        let mut files_with_row = BTreeSet::new();
        for row in &sheet.rows {
            let Some(cell) = column.and_then(|column| row.cells.get(column)) else {
                continue;
            };
            let key = RowRef {
                sheet: plan.id.clone(),
                key: row.key.clone(),
            };
            let (record, left) = states.get(&key).copied().unwrap_or_default();
            if matches!(left, Some(LeftBy::Decision(_))) {
                continue;
            }
            let landed = match record.map(|id| model.record(id)) {
                None => Landed::Nowhere,
                Some(record) => match record.kind {
                    RecordKind::ItemSet => continue,
                    RecordKind::Media => {
                        files_with_row.extend(record.files.iter().cloned());
                        match record.parent {
                            Some(Parent::Record(item)) => Landed::Paired(item),
                            _ => Landed::Nowhere,
                        }
                    }
                    RecordKind::Item if record.files.is_empty() => Landed::Alone(record.id),
                    RecordKind::Item => Landed::Paired(record.id),
                },
            };
            if let Landed::Paired(item) = landed
                && item_ids.contains(&item)
            {
                *rows_on.entry(item).or_default() += 1;
            }
            rows.push(ListRow {
                row,
                cell: cell.trim(),
                landed,
            });
        }
        Self {
            file: ListFile {
                source: plan.source.clone(),
                file: plan.file.clone(),
            },
            of_files: plan.row_kind == RowKind::All(RecordKind::Media),
            rows,
            rows_on,
            files_with_row,
        }
    }

    fn paired(&self) -> Share {
        Share {
            n: self
                .rows
                .iter()
                .filter(|row| matches!(row.landed, Landed::Paired(_)))
                .count(),
            of: self.rows.len(),
        }
    }

    /// The list pairs at least [`MIN_SUGGESTION_SHARE`] of its rows: a near
    /// name is an exception in a list that works, and chance in one that
    /// does not.
    fn works(&self) -> bool {
        let paired = self.paired();
        paired.of > 0 && paired.n * MIN_SUGGESTION_SHARE.of >= paired.of * MIN_SUGGESTION_SHARE.n
    }
}

/// A file or an item with no row of the list: what a row may point at.
struct Target<'i> {
    item: &'i Item,
    /// The file, when the rows describe files.
    file: Option<&'i FileRef>,
}

fn targets<'i>(items: &'i [Item], read: &ListRead<'_>) -> Vec<Target<'i>> {
    let mut found = Vec::new();
    for item in items.iter().filter(|item| !item.files.is_empty()) {
        if read.of_files {
            for file in item
                .files
                .iter()
                .filter(|file| !read.files_with_row.contains(*file))
            {
                found.push(Target {
                    item,
                    file: Some(file),
                });
            }
        } else if !read.rows_on.contains_key(&item.id) {
            found.push(Target { item, file: None });
        }
    }
    found
}

/// The relation by which a row's cell points at a target, if one holds.
fn points_at(cell: &str, target: &Target<'_>, of_files: bool) -> Option<Because> {
    let names = cell_names(cell);
    let files: Vec<&FileRef> = match target.file {
        Some(file) => vec![file],
        None => target.item.files.iter().collect(),
    };
    let is_near = names.iter().any(|name| {
        files.iter().any(|file| {
            let path = file.path.as_str();
            let whole = std::path::Path::new(path)
                .file_name()
                .and_then(std::ffi::OsStr::to_str)
                .unwrap_or(path);
            near(name, whole) || near(name, stem(path))
        })
    });
    if is_near {
        return Some(Because::Near);
    }
    let key = &target.item.key;
    if key.is_empty() {
        return None;
    }
    let row_places: Vec<Vec<String>> = names.iter().map(|name| place_texts(name)).collect();
    let one_file = target.item.files.len() == 1;
    if row_places.iter().any(|places| places == key) && (!of_files || one_file) {
        return Some(Because::NamesItem);
    }
    let names_part = row_places.iter().any(|places| {
        if !short_extension(key, places) {
            return false;
        }
        let part = &places[key.len()..];
        let has_part = |file: &FileRef| {
            let own = place_texts(stem(file.path.as_str()));
            own.starts_with(key) && own[key.len()..].starts_with(part)
        };
        match target.file {
            None => true,
            Some(file) => {
                has_part(file) && target.item.files.iter().filter(|one| has_part(one)).count() == 1
            }
        }
    });
    names_part.then_some(Because::NamesPart)
}

/// Detector 8: rows that pair no file, each pointing at exactly one target
/// that no other row points at.
fn row_is(items: &[Item], read: &ListRead<'_>) -> Option<Fix> {
    let targets = targets(items, read);
    let wrong_item = |row: &ListRow<'_>| {
        !read.of_files
            && matches!(row.landed, Landed::Paired(item) if read.rows_on.get(&item).is_some_and(|rows| *rows > 1))
    };
    let pointing: Vec<(&ListRow<'_>, Vec<(usize, Because)>)> = read
        .rows
        .iter()
        .filter(|row| !matches!(row.landed, Landed::Paired(_)) || wrong_item(row))
        .map(|row| {
            let only_near = matches!(row.landed, Landed::Paired(_));
            let hits = targets
                .iter()
                .enumerate()
                .filter_map(|(at, target)| {
                    let because = points_at(row.cell, target, read.of_files)?;
                    (!only_near || because == Because::Near).then_some((at, because))
                })
                .collect();
            (row, hits)
        })
        .collect();
    let mut pointed: HashMap<usize, usize> = HashMap::new();
    for (_, hits) in &pointing {
        for (at, _) in hits {
            *pointed.entry(*at).or_default() += 1;
        }
    }
    let mut pairs = Vec::new();
    let mut alone = 0;
    for (row, hits) in &pointing {
        let [(at, because)] = hits.as_slice() else {
            continue;
        };
        if pointed.get(at) != Some(&1) {
            continue;
        }
        let target = &targets[*at];
        let Some(file) = target.file.or_else(|| target.item.files.first()) else {
            continue;
        };
        if file.source != read.file.source {
            continue;
        }
        alone += usize::from(matches!(row.landed, Landed::Alone(_)));
        pairs.push(PairOffer {
            pair: RowPair {
                row: RowCell {
                    line: row.row.line,
                    cell: row.cell.to_owned(),
                },
                file: file.path.clone(),
            },
            because: *because,
        });
    }
    if pairs.is_empty() {
        return None;
    }
    let before = read.paired();
    let newly = pairs.len()
        - pointing
            .iter()
            .filter(|(row, _)| matches!(row.landed, Landed::Paired(_)))
            .filter(|(row, _)| pairs.iter().any(|pair| pair.pair.row.line == row.row.line))
            .count();
    Some(Fix::RowIs {
        list: read.file.clone(),
        counts: FixCounts {
            instances: pairs.len(),
            items: (items.len(), items.len() - alone),
            rows: Some((
                before,
                Share {
                    n: before.n + newly,
                    of: before.of,
                },
            )),
        },
        pairs,
    })
}

/// Detector 9: rows that made an item with no file and point at no target
/// by any relation of detector 8.
fn describes_nothing(items: &[Item], read: &ListRead<'_>) -> Option<Fix> {
    let targets = targets(items, read);
    let rows: Vec<RowOffer> = read
        .rows
        .iter()
        .filter(|row| matches!(row.landed, Landed::Alone(_)))
        .filter(|row| {
            targets
                .iter()
                .all(|target| points_at(row.cell, target, read.of_files).is_none())
        })
        .map(|row| RowOffer {
            row: RowCell {
                line: row.row.line,
                cell: row.cell.to_owned(),
            },
        })
        .collect();
    if rows.is_empty() {
        return None;
    }
    let paired = read.paired();
    Some(Fix::DescribesNothing {
        list: read.file.clone(),
        counts: FixCounts {
            instances: rows.len(),
            items: (items.len(), items.len() - rows.len()),
            rows: Some((paired, paired)),
        },
        rows,
    })
}

/// Detector 7: items that are one item, by their keys (rule A) or by a row
/// that names them both (rule B).
fn one_item(items: &[Item], read: Option<&ListRead<'_>>, here: &Here<'_>) -> Option<Fix> {
    let keyed: Vec<&Item> = items
        .iter()
        .filter(|item| !item.key.is_empty() && !item.files.is_empty())
        .collect();
    let own_row = |item: &Item| {
        read.is_some_and(|read| {
            read.rows.iter().any(|row| {
                let on_item = match row.landed {
                    Landed::Paired(id) => id == item.id,
                    Landed::Alone(_) | Landed::Nowhere => false,
                };
                on_item
                    && cell_names(row.cell)
                        .iter()
                        .any(|name| place_texts(name) == item.key)
            })
        })
    };
    // Rule A: each item's one longest item whose key its own extends.
    let mut parent: HashMap<RecordId, RecordId> = HashMap::new();
    for joining in &keyed {
        let extended: Vec<&&Item> = keyed
            .iter()
            .filter(|into| short_extension(&into.key, &joining.key))
            .collect();
        let Some(longest) = extended.iter().map(|into| into.key.len()).max() else {
            continue;
        };
        let mut at_longest = extended.iter().filter(|into| into.key.len() == longest);
        let (Some(into), None) = (at_longest.next(), at_longest.next()) else {
            continue;
        };
        if !own_row(joining) {
            parent.insert(joining.id, into.id);
        }
    }
    let mut joins: Vec<(RecordId, RecordId, Because)> = parent
        .keys()
        .map(|joining| {
            let mut root = *joining;
            while let Some(next) = parent.get(&root) {
                root = *next;
            }
            (root, *joining, Because::KeyExtends)
        })
        .collect();
    // Rule B: a row that pairs no file starts the keys of two items or more
    // with no row, when the rows describe items.
    if let Some(read) = read.filter(|read| !read.of_files) {
        let joined: BTreeSet<RecordId> = joins.iter().map(|(_, joining, _)| *joining).collect();
        for row in read
            .rows
            .iter()
            .filter(|row| matches!(row.landed, Landed::Alone(_)))
        {
            let named: Vec<&&Item> = keyed
                .iter()
                .filter(|item| !read.rows_on.contains_key(&item.id) && !joined.contains(&item.id))
                .filter(|item| {
                    cell_names(row.cell)
                        .iter()
                        .any(|name| short_extension(&place_texts(name), &item.key))
                })
                .collect();
            if let [first, rest @ ..] = named.as_slice()
                && !rest.is_empty()
            {
                joins.extend(
                    rest.iter()
                        .map(|one| (first.id, one.id, Because::RowNamesBoth)),
                );
            }
        }
    }
    let least = |id: RecordId| {
        items.iter().find(|item| item.id == id).and_then(|item| {
            item.files
                .iter()
                .filter(|file| file.source == here.scope.source)
                .min()
        })
    };
    let mut offers: Vec<JoinOffer> = joins
        .iter()
        .filter_map(|(into, joining, because)| {
            Some(JoinOffer {
                join: ItemJoin {
                    item: least(*into)?.path.clone(),
                    with: least(*joining)?.path.clone(),
                },
                because: *because,
            })
        })
        .collect();
    offers.sort_by(|a, b| (&a.join.item, &a.join.with).cmp(&(&b.join.item, &b.join.with)));
    let mut seen = BTreeSet::new();
    offers.retain(|offer| seen.insert(offer.join.with.clone()));
    if offers.is_empty() {
        return None;
    }
    let joining: BTreeSet<&RelPath> = offers.iter().map(|offer| &offer.join.with).collect();
    Some(Fix::OneItem {
        counts: FixCounts {
            instances: offers.len(),
            items: (items.len(), items.len() - joining.len()),
            rows: read.map(|read| {
                let paired = read.paired();
                (paired, paired)
            }),
        },
        joins: offers,
    })
}
