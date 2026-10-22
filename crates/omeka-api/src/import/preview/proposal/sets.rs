//! Detector 5, set candidates (design section 2): the set each series could
//! split into, and the sets that would hold every item. Only a series
//! folder is ever chosen, and only when detector 1 found that the folders
//! are not kinds of file; otherwise each folder set is offered, and so is
//! one set per top folder of a source. Every other candidate is offered,
//! because a name place or a column of few values can be a collection, a
//! date or a format code, and the counts are the same for all three.
//!
//! A candidate is a name place, a column of a list that describes the
//! series, or a level of numbered folders. It is a split when it makes at
//! least [`MIN_SETS`] sets, one of them holds [`MIN_ITEMS_PER_SET`] items,
//! and at least [`MIN_GROUPED_SHARE`] of its memberships are in such sets.
//! A column must also come from a list the series reads (detector 4's
//! choice, or a list the person uploaded), not a list the plan keeps only
//! for a free set, and [`MIN_ITEMS_OF_ONE_VALUE`] of the items it places
//! must hold one of its values. A candidate that fails is a further choice ("More…"),
//! never dropped. A candidate whose one set holds the items of its series is
//! never offered: a column with one value in every row is the rival title
//! of the series' set. A column with one value in every row of every list
//! that describes a series is offered at the top, as one set over
//! everything. Every other column of a series' lists is a further choice of
//! its set, and an `addSet` edit may take it.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::num::NonZeroU32;

use unicode_normalization::UnicodeNormalization;

use super::names::PlaceValues;
use super::shape::KeyShape;
use super::thresholds::{MIN_GROUPED_SHARE, MIN_ITEMS_OF_ONE_VALUE, MIN_ITEMS_PER_SET, MIN_SETS};
use super::{Pick, ScanText, SetCounts, Share};
use crate::import::derive::Sheets;
use crate::import::join::cell;
use crate::import::model::Model;
use crate::import::plan::{
    ColumnRef, Edit, FolderLevel, FolderScope, ListFile, RecordKind, RelPath, RowKind, SeriesIx,
    SetById, SetByTitle, SetFrom, SetSource, SheetPlan, SourceId, SourceRef, Structure,
};
use crate::import::sheet::{Row, Sheet};

/// One series as detector 5 reads it.
pub(super) struct SeriesIn<'s> {
    pub at: SeriesIx,
    pub scope: &'s FolderScope,
    /// Its key's name places; none when its items are rows.
    pub places: Vec<PlaceValues<'s>>,
    /// Its folders are numbered: their level is a candidate.
    pub numbered: bool,
    /// The set its folder's name could make (detector 1).
    pub folder: Option<&'s SetFrom>,
    /// The lists it reads: a column of any other list is no split.
    pub reads: Vec<ListFile>,
}

/// What the plan writes for one set the proposal holds: a new set by a
/// title, or a set already in Omeka (an `existing` edit).
#[derive(Debug, Clone)]
pub(super) enum OneSet {
    /// The scan's name, or the title and description a `title` edit gave.
    Title {
        title: ScanText,
        description: Option<String>,
    },
    /// `shown` is the title the counts keep showing.
    Existing { shown: ScanText, set: SetById },
}

impl OneSet {
    fn named(title: ScanText) -> Self {
        Self::Title {
            title,
            description: None,
        }
    }

    /// The title the counts show and a set level's id is made of.
    pub fn shown(&self) -> &ScanText {
        match self {
            Self::Title { title, .. } => title,
            Self::Existing { shown, .. } => shown,
        }
    }

    pub fn source(&self) -> SetSource {
        match self {
            Self::Title { title, description } => SetSource::Title(SetByTitle {
                title: title.0.clone(),
                description: description.clone(),
            }),
            Self::Existing { set, .. } => SetSource::Id(*set),
        }
    }
}

/// A series' folder set: chosen by detector 1, or offered.
#[derive(Debug, Clone)]
pub(super) struct FolderSet {
    pub from: SetFrom,
    /// The folder's name, or what a `title` or `existing` edit made it.
    pub set: OneSet,
    pub chosen: bool,
    /// The items of the series.
    pub items: usize,
}

impl FolderSet {
    pub fn pick(&self) -> Pick<SetFrom, SetCounts> {
        Pick {
            value: self.from.clone(),
            counts: SetCounts {
                sets: vec![(self.set.shown().clone(), self.items)],
                rows: None,
            },
        }
    }
}

/// An offered split the person took.
#[derive(Debug, Clone)]
pub(super) struct Taken {
    pub pick: Pick<SetFrom, SetCounts>,
    /// What each item's set name is read from.
    pub source: SourceRef,
    /// For a name place: the classes up to it, which the pattern captures.
    pub place: Option<KeyShape>,
    /// For a column: the list and column the plan must read.
    pub column: Option<(ListFile, ColumnRef)>,
    /// Text the scan read that tells this level from the series.
    pub name: String,
    /// It was offered as a split.
    pub split: bool,
}

/// What detector 5 found for one series.
#[derive(Debug, Default)]
pub(super) struct SeriesSets {
    /// Its folder set, chosen or offered.
    pub folder: Option<FolderSet>,
    /// The rival of the series' set: the folder set when it is only
    /// offered, else a column with one value here, as a rival title.
    pub rival: Option<Pick<SetFrom, SetCounts>>,
    /// The offered splits not taken.
    pub splits: Vec<Pick<SetFrom, SetCounts>>,
    /// Every other column of its lists not taken, with its sets.
    pub others: Vec<Pick<SetFrom, SetCounts>>,
    /// The splits taken, in the order of the edits.
    pub taken: Vec<Taken>,
}

/// What detector 5 found.
#[derive(Debug, Default)]
pub(super) struct Sets {
    /// Sets that would hold every item, not taken.
    pub top: Vec<Pick<SetFrom, SetCounts>>,
    /// The one top set taken, with what the plan writes for it.
    pub top_taken: Option<(Pick<SetFrom, SetCounts>, OneSet)>,
    pub series: Vec<SeriesSets>,
}

/// A candidate with what taking it means for the plan.
struct Candidate {
    pick: Pick<SetFrom, SetCounts>,
    source: SourceRef,
    place: Option<KeyShape>,
    column: Option<(ListFile, ColumnRef)>,
    name: String,
    /// A split: it sorts the series' items. Otherwise a further choice.
    split: bool,
}

fn text(row: &Row, column: u32) -> String {
    cell(row, column).trim().nfc().collect()
}

fn most_first<K: AsRef<str>>(values: BTreeMap<K, usize>) -> Vec<(ScanText, usize)> {
    let mut sets: Vec<(ScanText, usize)> = values
        .into_iter()
        .map(|(value, items)| (ScanText(value.as_ref().to_owned()), items))
        .collect();
    sets.sort_by(|(a, a_items), (b, b_items)| b_items.cmp(a_items).then_with(|| a.cmp(b)));
    sets
}

/// Whether sets of these sizes sort a series of `items`: enough sets, one
/// of them more than a single item, and none holding the whole series.
fn sorts(sets: &[(ScanText, usize)], items: usize) -> bool {
    sets.len() >= MIN_SETS
        && sets.iter().any(|(_, held)| *held >= MIN_ITEMS_PER_SET)
        && sets.iter().all(|(_, held)| *held < items)
}

/// Whether at least [`MIN_GROUPED_SHARE`] of the memberships of these sets
/// are in sets of [`MIN_ITEMS_PER_SET`] items or more.
fn groups(sets: &[(ScanText, usize)]) -> bool {
    let all: usize = sets.iter().map(|(_, held)| held).sum();
    let grouped: usize = sets
        .iter()
        .map(|(_, held)| *held)
        .filter(|held| *held >= MIN_ITEMS_PER_SET)
        .sum();
    grouped * MIN_GROUPED_SHARE.of >= MIN_GROUPED_SHARE.n * all
}

/// One list of a series, as its rows describe the series' items.
struct ListIn<'l> {
    file: ListFile,
    plan: &'l SheetPlan,
    sheet: &'l Sheet,
    /// The rows each item of the series has in this list.
    rows_of_items: Vec<Vec<&'l Row>>,
}

impl ListIn<'_> {
    /// The columns a set may come from: not the join, the key, the kind or
    /// a column of row numbers.
    fn columns(&self) -> impl Iterator<Item = ColumnRef> + '_ {
        let join = self.plan.join.as_ref().map(|join| join.column.index);
        let key = self.plan.item_key.as_ref().map(|key| key.index);
        let kind = match &self.plan.row_kind {
            RowKind::Column(kind) => Some(kind.column.index),
            RowKind::All(_) => None,
        };
        self.sheet
            .columns
            .iter()
            .filter(move |column| {
                let index = Some(column.index);
                index != join
                    && index != key
                    && index != kind
                    && !super::grain::numbers_rows(self.sheet, column.index)
            })
            .map(|column| ColumnRef {
                index: column.index,
                header: column.header.clone(),
            })
    }

    /// Each filled value of `column` with the items that hold it, the rows
    /// that hold a value, and the items that hold one value of the items
    /// that hold any.
    fn values(&self, column: &ColumnRef) -> (BTreeMap<String, usize>, Share, Share) {
        let mut values: BTreeMap<String, usize> = BTreeMap::new();
        let mut one_value = Share { n: 0, of: 0 };
        for rows in &self.rows_of_items {
            let held: BTreeSet<String> = rows
                .iter()
                .map(|row| text(row, column.index))
                .filter(|value| !value.is_empty())
                .collect();
            if !held.is_empty() {
                one_value.of += 1;
                one_value.n += usize::from(held.len() == 1);
            }
            for value in held {
                *values.entry(value).or_default() += 1;
            }
        }
        let filled = self
            .sheet
            .rows
            .iter()
            .filter(|row| !text(row, column.index).is_empty())
            .count();
        let rows = Share {
            n: filled,
            of: self.sheet.rows.len(),
        };
        (values, rows, one_value)
    }
}

/// The lists of the series at `at` that read, each with its items' rows in
/// the derive of the proposal.
fn lists_of<'l>(
    model: &Model,
    structure: &'l Structure,
    sheets: &'l Sheets,
    at: SeriesIx,
) -> Vec<ListIn<'l>> {
    let items: Vec<_> = model
        .kept()
        .filter(|record| record.kind == RecordKind::Item && record.series == at)
        .collect();
    structure
        .series(at)
        .lists()
        .iter()
        .filter_map(|plan| {
            let sheet = sheets.get(&plan.id)?.as_ref().ok()?;
            let by_key: HashMap<_, &Row> = sheet.rows.iter().map(|row| (&row.key, row)).collect();
            let rows_of_items = items
                .iter()
                .map(|item| {
                    let media = item.media.iter().map(|media| model.record(*media));
                    std::iter::once(*item)
                        .chain(media)
                        .flat_map(|record| &record.rows)
                        .filter(|row| row.sheet == plan.id)
                        .filter_map(|row| by_key.get(&row.key).copied())
                        .collect()
                })
                .collect();
            Some(ListIn {
                file: ListFile {
                    source: plan.source.clone(),
                    file: plan.file.clone(),
                },
                plan,
                sheet,
                rows_of_items,
            })
        })
        .collect()
}

/// The one value of a column, when every row of every list holds it.
fn one_value<'l>(lists: &[(&ListIn<'l>, ColumnRef)]) -> Option<(String, Share)> {
    let mut value = None;
    let mut rows = 0;
    for (list, column) in lists {
        for row in &list.sheet.rows {
            let cell = text(row, column.index);
            if cell.is_empty() || value.as_ref().is_some_and(|value| *value != cell) {
                return None;
            }
            value.get_or_insert(cell);
            rows += 1;
        }
    }
    Some((value?, Share { n: rows, of: rows }))
}

/// Rule 2: the columns that hold one value in every row of every list that
/// describes a series, matched across lists by header.
fn top_sets(lists: &[ListIn<'_>], items: usize) -> Vec<(Pick<SetFrom, SetCounts>, ScanText)> {
    let Some(first) = lists.first() else {
        return Vec::new();
    };
    first
        .columns()
        .filter_map(|column| {
            let mut same: Vec<(&ListIn<'_>, ColumnRef)> = Vec::new();
            for list in lists {
                let found = list.columns().find(|other| other.header == column.header)?;
                same.push((list, found));
            }
            let (value, rows) = one_value(&same)?;
            let title = ScanText(value);
            let pick = Pick {
                value: SetFrom::Column {
                    list: first.file.clone(),
                    column,
                },
                counts: SetCounts {
                    sets: vec![(title.clone(), items)],
                    rows: Some(rows),
                },
            };
            Some((pick, title))
        })
        .collect()
}

/// The candidates of one series, and the rival title of its set.
fn candidates(
    model: &Model,
    one: &SeriesIn<'_>,
    lists: &[ListIn<'_>],
    top: &BTreeSet<String>,
) -> (Vec<Candidate>, Option<Pick<SetFrom, SetCounts>>) {
    let items = model
        .kept()
        .filter(|record| record.kind == RecordKind::Item && record.series == one.at)
        .count();
    let mut found = Vec::new();
    if one.numbered {
        let mut sets: BTreeMap<&str, usize> = BTreeMap::new();
        for record in model
            .kept()
            .filter(|record| record.kind == RecordKind::Item && record.series == one.at)
        {
            if let Some(top) = record
                .files
                .first()
                .and_then(|file| file.path.segments().next())
            {
                *sets.entry(top).or_default() += 1;
            }
        }
        let sets = most_first(sets);
        found.push(Candidate {
            split: groups(&sets),
            pick: Pick {
                value: SetFrom::FolderLevel {
                    scope: FolderScope {
                        source: one.scope.source.clone(),
                        folder: RelPath::root(),
                    },
                    depth: 1,
                },
                counts: SetCounts { sets, rows: None },
            },
            source: SourceRef::Folder {
                level: FolderLevel::Level(NonZeroU32::MIN),
            },
            place: None,
            column: None,
            name: NonZeroU32::MIN.to_string(),
        });
    }
    for (place, values) in one.places.iter().enumerate() {
        let sets = most_first(values.values.clone());
        let Ok(at) = u8::try_from(place) else {
            continue;
        };
        if sorts(&sets, items) {
            found.push(Candidate {
                split: groups(&sets),
                pick: Pick {
                    value: SetFrom::NamePlace {
                        scope: one.scope.clone(),
                        place: at,
                    },
                    counts: SetCounts { sets, rows: None },
                },
                source: SourceRef::Group {
                    name: crate::import::grouping::pattern::SET.to_owned(),
                },
                place: Some(values.classes.clone()),
                column: None,
                name: place.to_string(),
            });
        }
    }
    let mut rival = None;
    for list in lists {
        for column in list.columns() {
            let (values, rows, one_value) = list.values(&column);
            let sets = most_first(values);
            let pick = Pick {
                value: SetFrom::Column {
                    list: list.file.clone(),
                    column: column.clone(),
                },
                counts: SetCounts {
                    sets,
                    rows: Some(rows),
                },
            };
            // One value in every row: its one set is the series, as the
            // list describes it.
            let one_set = pick.counts.sets.len() == 1 && rows.n == rows.of;
            if one_set && rival.is_none() && !top.contains(&column.header) {
                rival = Some(pick.clone());
            }
            found.push(Candidate {
                source: SourceRef::Column {
                    sheet: list.plan.id.clone(),
                    column: column.clone(),
                },
                place: None,
                column: Some((list.file.clone(), column.clone())),
                name: column.header.clone(),
                split: !one_set
                    && sorts(&pick.counts.sets, items)
                    && groups(&pick.counts.sets)
                    && one_value.n * MIN_ITEMS_OF_ONE_VALUE.of
                        >= MIN_ITEMS_OF_ONE_VALUE.n * one_value.of
                    && one.reads.contains(&list.file),
                pick,
            });
        }
    }
    (found, rival)
}

/// One set per top folder of `source`, over its series: what the folder
/// level offers or takes.
fn folder_level(source: &FolderScope) -> SetFrom {
    SetFrom::FolderLevel {
        scope: FolderScope {
            source: source.source.clone(),
            folder: RelPath::root(),
        },
        depth: 1,
    }
}

/// Detector 5 on the derive of the proposal. Each series' folder set is
/// chosen when `folder_sets` (detector 1) and offered otherwise, one by one
/// and as one folder level per source. The plan's `addSet`, `dropSet` and
/// `title` edits are replayed: each one it applies is marked in `applied`.
/// A name place is captured by one group, so a series takes at most one
/// name place.
pub(super) fn detect(
    model: &Model,
    structure: &Structure,
    sheets: &Sheets,
    series: &[SeriesIn<'_>],
    folder_sets: bool,
    edits: &[Edit],
    applied: &mut [bool],
) -> Sets {
    let lists: Vec<Vec<ListIn<'_>>> = series
        .iter()
        .map(|one| lists_of(model, structure, sheets, one.at))
        .collect();
    let every_list: Vec<ListIn<'_>> = series
        .iter()
        .flat_map(|one| lists_of(model, structure, sheets, one.at))
        .collect();
    let items = model
        .kept()
        .filter(|record| record.kind == RecordKind::Item)
        .count();
    let mut top = top_sets(&every_list, items);
    let top_headers: BTreeSet<String> = top
        .iter()
        .filter_map(|(pick, _)| match &pick.value {
            SetFrom::Column { column, .. } => Some(column.header.clone()),
            _ => None,
        })
        .collect();
    let mut found = Sets::default();
    let mut offered: Vec<Vec<Candidate>> = Vec::new();
    for (one, lists) in series.iter().zip(&lists) {
        let (candidates, rival) = candidates(model, one, lists, &top_headers);
        // A column whose one value holds every item names the top offer.
        offered.push(
            candidates
                .into_iter()
                .filter(|candidate| {
                    top.iter()
                        .all(|(pick, _)| pick.value != candidate.pick.value)
                })
                .collect(),
        );
        let items = model
            .kept()
            .filter(|record| record.kind == RecordKind::Item && record.series == one.at)
            .count();
        found.series.push(SeriesSets {
            folder: one.folder.map(|from| FolderSet {
                from: from.clone(),
                set: OneSet::named(super::folders::folder_title(one.scope)),
                chosen: folder_sets,
                items,
            }),
            rival,
            ..SeriesSets::default()
        });
    }
    for (at, edit) in edits.iter().enumerate() {
        match edit {
            Edit::AddSet { from } => {
                applied[at] = add(&mut found, &mut top, &mut offered, series, from);
            }
            Edit::DropSet { from } => {
                applied[at] = drop(&mut found, &mut top, &mut offered, series, from);
            }
            Edit::Title {
                from,
                title,
                description,
            } => {
                let set = |_: &ScanText| OneSet::Title {
                    title: ScanText(title.clone()),
                    description: description.clone(),
                };
                applied[at] = rename(&mut found, from, &set);
            }
            Edit::Existing { from, id } => {
                let set = |shown: &ScanText| OneSet::Existing {
                    shown: shown.clone(),
                    set: SetById { id: *id },
                };
                applied[at] = rename(&mut found, from, &set);
            }
            _ => {}
        }
    }
    for (sets, candidates) in found.series.iter_mut().zip(offered) {
        let rival = sets.rival.as_ref().map(|rival| rival.value.clone());
        let (splits, others): (Vec<Candidate>, Vec<Candidate>) = candidates
            .into_iter()
            .partition(|candidate| candidate.split);
        sets.splits = splits.into_iter().map(|candidate| candidate.pick).collect();
        sets.others = others
            .into_iter()
            .map(|candidate| candidate.pick)
            .filter(|pick| Some(&pick.value) != rival.as_ref())
            .collect();
        if let Some(folder) = sets.folder.as_ref().filter(|folder| !folder.chosen) {
            sets.rival = Some(folder.pick());
        } else if sets
            .taken
            .iter()
            .any(|taken| Some(&taken.pick.value) == rival.as_ref())
        {
            sets.rival = None;
        }
    }
    found.top = top.into_iter().map(|(pick, _)| pick).collect();
    found.top.extend(folder_offers(&found, series));
    found
}

/// One folder level per source whose series offer two folder sets or more
/// and choose none.
fn folder_offers(found: &Sets, series: &[SeriesIn<'_>]) -> Vec<Pick<SetFrom, SetCounts>> {
    let mut by_source: BTreeMap<&SourceId, Vec<&FolderSet>> = BTreeMap::new();
    for (one, sets) in series.iter().zip(&found.series) {
        if let Some(folder) = &sets.folder {
            by_source.entry(&one.scope.source).or_default().push(folder);
        }
    }
    by_source
        .into_iter()
        .map(|(source, folders)| {
            let root = FolderScope {
                source: source.clone(),
                folder: RelPath::root(),
            };
            (folder_level(&root), folders)
        })
        .filter(|(_, folders)| {
            folders.len() >= MIN_SETS && folders.iter().all(|folder| !folder.chosen)
        })
        .map(|(value, folders)| {
            let mut sets: Vec<(ScanText, usize)> = folders
                .iter()
                .map(|folder| (folder.set.shown().clone(), folder.items))
                .collect();
            sets.sort_by(|(a, a_items), (b, b_items)| b_items.cmp(a_items).then_with(|| a.cmp(b)));
            Pick {
                value,
                counts: SetCounts { sets, rows: None },
            }
        })
        .collect()
}

/// The folder sets `from` names: one series' folder, or every folder of a
/// source's folder level.
fn folder_sets_named<'f>(
    found: &'f mut Sets,
    series: &[SeriesIn<'_>],
    from: &SetFrom,
) -> Vec<&'f mut FolderSet> {
    found
        .series
        .iter_mut()
        .zip(series)
        .filter_map(|(sets, one)| {
            let folder = sets.folder.as_mut()?;
            (folder.from == *from || folder_level(one.scope) == *from).then_some(folder)
        })
        .collect()
}

/// Replay one `addSet`: whether the proposal now holds what it asks for.
fn add(
    found: &mut Sets,
    top: &mut Vec<(Pick<SetFrom, SetCounts>, ScanText)>,
    offered: &mut [Vec<Candidate>],
    series: &[SeriesIn<'_>],
    from: &SetFrom,
) -> bool {
    let folders = folder_sets_named(found, series, from);
    let named_folders = !folders.is_empty();
    for folder in folders {
        folder.chosen = true;
    }
    if found
        .top_taken
        .as_ref()
        .is_some_and(|(pick, _)| pick.value == *from)
        || found
            .series
            .iter()
            .any(|sets| sets.taken.iter().any(|taken| taken.pick.value == *from))
    {
        return true;
    }
    if found.top_taken.is_none()
        && let Some(taken) = top.iter().position(|(pick, _)| pick.value == *from)
    {
        let (pick, title) = top.remove(taken);
        found.top_taken = Some((pick, OneSet::named(title)));
        return true;
    }
    for (sets, candidates) in found.series.iter_mut().zip(offered) {
        let has_place = sets.taken.iter().any(|taken| taken.place.is_some());
        let Some(taken) = candidates.iter().position(|candidate| {
            candidate.pick.value == *from && !(has_place && candidate.place.is_some())
        }) else {
            continue;
        };
        let candidate = candidates.remove(taken);
        sets.taken.push(Taken {
            pick: candidate.pick,
            source: candidate.source,
            place: candidate.place,
            column: candidate.column,
            name: candidate.name,
            split: candidate.split,
        });
        return true;
    }
    named_folders
}

/// Replay one `dropSet`: whether the proposal now holds no set `from`
/// names, of the sets it shows.
fn drop(
    found: &mut Sets,
    top: &mut Vec<(Pick<SetFrom, SetCounts>, ScanText)>,
    offered: &mut [Vec<Candidate>],
    series: &[SeriesIn<'_>],
    from: &SetFrom,
) -> bool {
    let folders = folder_sets_named(found, series, from);
    let mut shown = !folders.is_empty();
    for folder in folders {
        folder.chosen = false;
    }
    if found
        .top_taken
        .as_ref()
        .is_some_and(|(pick, _)| pick.value == *from)
        && let Some((pick, set)) = found.top_taken.take()
    {
        top.push((pick, set.shown().clone()));
    }
    shown |= top.iter().any(|(pick, _)| pick.value == *from);
    for (sets, candidates) in found.series.iter_mut().zip(offered) {
        let (dropped, kept): (Vec<Taken>, Vec<Taken>) = std::mem::take(&mut sets.taken)
            .into_iter()
            .partition(|taken| taken.pick.value == *from);
        sets.taken = kept;
        candidates.extend(dropped.into_iter().map(|taken| Candidate {
            pick: taken.pick,
            source: taken.source,
            place: taken.place,
            column: taken.column,
            name: taken.name,
            split: taken.split,
        }));
        shown |= candidates
            .iter()
            .any(|candidate| candidate.pick.value == *from);
    }
    shown
}

/// Replay one `title` or `existing` edit on a folder set or the top set
/// the proposal holds: whether it found one. `named` makes the new set
/// from the title the counts show now.
fn rename(found: &mut Sets, from: &SetFrom, named: &dyn Fn(&ScanText) -> OneSet) -> bool {
    let mut found_one = false;
    for sets in &mut found.series {
        if let Some(folder) = sets
            .folder
            .as_mut()
            .filter(|folder| folder.chosen && folder.from == *from)
        {
            folder.set = named(folder.set.shown());
            found_one = true;
        }
    }
    if let Some((pick, set)) = found.top_taken.as_mut()
        && pick.value == *from
    {
        *set = named(set.shown());
        let shown = set.shown();
        pick.counts.sets = pick
            .counts
            .sets
            .iter()
            .map(|(_, items)| (shown.clone(), *items))
            .collect();
        found_one = true;
    }
    found_one
}
