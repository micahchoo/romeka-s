//! Detector 5, set candidates (design section 2): the set each series could
//! split into, and the sets that would hold every item. Only the series
//! folder (detector 1) is ever chosen; every other candidate is offered,
//! because a name place or a column of few values can be a collection, a
//! date or a format code, and the counts are the same for all three.
//!
//! A candidate is a name place, a column of a list that describes the
//! series, or a level of numbered folders. It is a split when it makes at
//! least [`MIN_SETS`] sets and one of them holds [`MIN_ITEMS_PER_SET`]
//! items. A candidate whose one set holds the items of its series is never
//! offered: a column with one value in every row is the rival title of the
//! series' set. A
//! column with one value in every row of every list that describes a series
//! is offered at the top, as one set over everything.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::num::NonZeroU32;

use unicode_normalization::UnicodeNormalization;

use super::names::PlaceValues;
use super::shape::KeyShape;
use super::thresholds::{MIN_ITEMS_PER_SET, MIN_SETS};
use super::{Pick, ScanText, SetCounts, Share};
use crate::import::derive::Sheets;
use crate::import::join::cell;
use crate::import::model::Model;
use crate::import::plan::{
    ColumnRef, Edit, FolderLevel, FolderScope, ListFile, RecordKind, RelPath, RowKind, SeriesIx,
    SetFrom, SheetPlan, SourceRef, Structure,
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
}

/// What detector 5 found for one series.
#[derive(Debug, Default)]
pub(super) struct SeriesSets {
    /// A column with one value here: the rival title of the series' set.
    pub rival: Option<Pick<SetFrom, SetCounts>>,
    /// The offered splits not taken.
    pub splits: Vec<Pick<SetFrom, SetCounts>>,
    /// The splits taken, in the order of the edits.
    pub taken: Vec<Taken>,
}

/// What detector 5 found.
#[derive(Debug, Default)]
pub(super) struct Sets {
    /// Sets that would hold every item, not taken.
    pub top: Vec<Pick<SetFrom, SetCounts>>,
    /// The one top set taken, with its title.
    pub top_taken: Option<(Pick<SetFrom, SetCounts>, ScanText)>,
    pub series: Vec<SeriesSets>,
}

/// A candidate with what taking it means for the plan.
struct Candidate {
    pick: Pick<SetFrom, SetCounts>,
    source: SourceRef,
    place: Option<KeyShape>,
    column: Option<(ListFile, ColumnRef)>,
    name: String,
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

    /// Each filled value of `column` with the items that hold it, and the
    /// rows that hold a value.
    fn values(&self, column: &ColumnRef) -> (BTreeMap<String, usize>, Share) {
        let mut values: BTreeMap<String, usize> = BTreeMap::new();
        for rows in &self.rows_of_items {
            let held: BTreeSet<String> = rows
                .iter()
                .map(|row| text(row, column.index))
                .filter(|value| !value.is_empty())
                .collect();
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
        (values, rows)
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
        found.push(Candidate {
            pick: Pick {
                value: SetFrom::FolderLevel {
                    scope: FolderScope {
                        source: one.scope.source.clone(),
                        folder: RelPath::root(),
                    },
                    depth: 1,
                },
                counts: SetCounts {
                    sets: most_first(sets),
                    rows: None,
                },
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
            let (values, rows) = list.values(&column);
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
            if pick.counts.sets.len() == 1 && rows.n == rows.of {
                if rival.is_none() && !top.contains(&column.header) {
                    rival = Some(pick);
                }
            } else if sorts(&pick.counts.sets, items) {
                found.push(Candidate {
                    source: SourceRef::Column {
                        sheet: list.plan.id.clone(),
                        column: column.clone(),
                    },
                    place: None,
                    column: Some((list.file.clone(), column.clone())),
                    name: column.header.clone(),
                    pick,
                });
            }
        }
    }
    (found, rival)
}

/// Detector 5 on the derive of the proposal. The plan's `addSet` edits are
/// replayed: each takes an offer; each one it applies is marked in
/// `applied`. A name place is captured by one group, so a series takes at
/// most one name place.
pub(super) fn detect(
    model: &Model,
    structure: &Structure,
    sheets: &Sheets,
    series: &[SeriesIn<'_>],
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
        offered.push(candidates);
        found.series.push(SeriesSets {
            rival,
            ..SeriesSets::default()
        });
    }
    for (at, edit) in edits.iter().enumerate() {
        let Edit::AddSet { from } = edit else {
            continue;
        };
        if found
            .top_taken
            .as_ref()
            .is_some_and(|(pick, _)| pick.value == *from)
            || found
                .series
                .iter()
                .any(|sets| sets.taken.iter().any(|taken| taken.pick.value == *from))
        {
            applied[at] = true;
            continue;
        }
        if found.top_taken.is_none()
            && let Some(taken) = top.iter().position(|(pick, _)| pick.value == *from)
        {
            found.top_taken = Some(top.remove(taken));
            applied[at] = true;
            continue;
        }
        for (sets, candidates) in found.series.iter_mut().zip(&mut offered) {
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
            });
            applied[at] = true;
            break;
        }
    }
    for (sets, candidates) in found.series.iter_mut().zip(offered) {
        sets.splits = candidates
            .into_iter()
            .map(|candidate| candidate.pick)
            .collect();
    }
    found.top = top.into_iter().map(|(pick, _)| pick).collect();
    found
}
