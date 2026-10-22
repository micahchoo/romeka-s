//! Detector 3, lists (design section 2): which list describes each series.
//! Each list of the walk is read, counted in every series, and let go
//! before the next is read, so only counts are held. The counts are the
//! `listCandidates` counts, restricted to one series' groups and files.
//!
//! A list describes the series where its best column pairs the most rows;
//! a tie describes none. A fold is on when it pairs more rows and makes no
//! more cells find a text that two files or two groups share. A second list
//! joins a series only when it pairs a group the lists before it do not;
//! otherwise it is the series' rival.

use std::cmp::Reverse;
use std::collections::{HashMap, HashSet};

use super::Evidence;
use super::folders::FolderSeries;
use crate::import::derive::Index;
use crate::import::model::{GroupKey, Model};
use crate::import::plan::{
    ColumnRef, Edit, Join, JoinNormalize, JoinOn, ListFile, SeriesIx, Structure,
};
use crate::import::preview::candidates::{self, CandidateRead, ListCandidate};
use crate::import::preview::columns::{EVERY_FOLD, NO_FOLD, Targets, Within};
use crate::import::sheet::Sheet;

/// One list counted in one series.
pub(super) struct Counted<'d> {
    pub counts: ListCandidate,
    /// The best column with the folds that rule 2 turns on; `None` when no
    /// row pairs.
    pub join: Option<Join>,
    /// The rows the join pairs.
    pub paired: usize,
    /// The files of the series the join names, each once.
    pub files: usize,
    /// For a `fileName` join: the groups it names two files or more of.
    pub groups_with_rows_for_many_files: usize,
    /// The groups the join pairs.
    groups: HashSet<&'d GroupKey>,
}

impl Counted<'_> {
    /// A list that pairs no row by any stated rule, joined by file name on
    /// `column` because a pairing suggestion pairs its rows in this series
    /// only (rule 3). It pairs nothing until the person takes the
    /// suggestion.
    pub fn by_suggestion(counts: ListCandidate, column: ColumnRef) -> Self {
        Self {
            counts,
            join: Some(Join {
                on: JoinOn::FileName,
                column,
                normalize: NO_FOLD,
            }),
            paired: 0,
            files: 0,
            groups_with_rows_for_many_files: 0,
            groups: HashSet::new(),
        }
    }
}

/// A list and its counts in one series.
pub(super) struct InSeries<'d> {
    pub file: ListFile,
    pub counted: Counted<'d>,
}

/// The lists of one series.
#[derive(Default)]
pub(super) struct SeriesLists<'d> {
    /// The lists that describe it, the one that pairs the most first.
    pub joined: Vec<InSeries<'d>>,
    /// The list with the next most paired rows here.
    pub rival: Option<InSeries<'d>>,
}

/// A list that describes no series.
pub(super) struct Unattached {
    pub file: ListFile,
    pub counts: ListCandidate,
    /// It read, and its best column pairs no row in any series.
    pub pairs_nothing: bool,
}

/// What detector 3 chose.
pub(super) struct Found<'d> {
    /// By [`SeriesIx::index`].
    pub series: Vec<SeriesLists<'d>>,
    pub unattached: Vec<Unattached>,
}

/// The folds rule 2 tries, one at a time, in this order.
const FOLDS: &[fn(&mut JoinNormalize)] = &[
    |how| how.case_fold = true,
    |how| how.leading_zeros = true,
    |how| how.separators = true,
];

/// The targets of each series after each fold, made once.
struct Cache<'d> {
    model: &'d Model,
    index: &'d Index,
    structure: &'d Structure,
    made: HashMap<(SeriesIx, JoinNormalize), Targets<'d>>,
}

impl<'d> Cache<'d> {
    fn targets(&mut self, at: SeriesIx, how: JoinNormalize) -> &Targets<'d> {
        let (model, index, structure) = (self.model, self.index, self.structure);
        self.made
            .entry((at, how))
            .or_insert_with(|| Targets::of(model, index, Some(Within { structure, at }), how))
    }
}

/// What a join pairs: rows, rows that find a shared text, and files.
struct Pairs {
    rows: usize,
    shared: usize,
    files: usize,
}

fn pairs(sheet: &Sheet, column: u32, on: JoinOn, targets: &Targets<'_>) -> Pairs {
    let mut found = Pairs {
        rows: 0,
        shared: 0,
        files: 0,
    };
    let mut files = HashSet::new();
    for cell in targets.cells(sheet, column).flatten() {
        let (paired, shared) = match on {
            JoinOn::FileName => (cell.file.is_some(), cell.shared_file),
            JoinOn::Identifier => (cell.key.is_some(), cell.shared_key),
        };
        found.rows += usize::from(paired);
        found.shared += usize::from(paired && shared);
        if on == JoinOn::FileName
            && let Some(file) = cell.file
        {
            files.insert(file);
        }
    }
    found.files = files.len();
    found
}

/// A candidate with nothing counted yet, named as `listCandidates` names it.
fn candidate(evidence: &Evidence<'_>, file: &ListFile) -> ListCandidate {
    let held = evidence
        .structure
        .lists()
        .find(|(_, sheet)| sheet.source == file.source && sheet.file == file.file);
    ListCandidate {
        source: file.source.clone(),
        file: file.file.clone(),
        in_plan: held.map(|(_, sheet)| sheet.id.clone()),
        // The node names the series; the member would repeat it.
        series: None,
        read: CandidateRead::Ok,
        rows: 0,
        columns: 0,
        best: None,
        grain: None,
        cells: Vec::new(),
        overlaps: Vec::new(),
    }
}

/// One read list counted in the series at `at`.
fn count<'d>(
    cache: &mut Cache<'d>,
    groups_of_files: &[Option<&'d GroupKey>],
    at: SeriesIx,
    sheet: &Sheet,
    mut counts: ListCandidate,
) -> Counted<'d> {
    counts.rows = sheet.rows.len();
    counts.columns = sheet.columns.len();
    counts.cells = candidates::column_cells(sheet);
    cache.targets(at, NO_FOLD);
    cache.targets(at, EVERY_FOLD);
    let best = candidates::best(
        sheet,
        &cache.made[&(at, NO_FOLD)],
        &cache.made[&(at, EVERY_FOLD)],
    );
    counts.best.clone_from(&best);
    let Some(best) = best else {
        return Counted {
            counts,
            join: None,
            paired: 0,
            files: 0,
            groups_with_rows_for_many_files: 0,
            groups: HashSet::new(),
        };
    };
    let mut how = NO_FOLD;
    let mut found = pairs(sheet, best.index, best.on, cache.targets(at, how));
    for fold in FOLDS {
        let mut tried = how;
        fold(&mut tried);
        let more = pairs(sheet, best.index, best.on, cache.targets(at, tried));
        if more.rows > found.rows && more.shared <= found.shared {
            how = tried;
            found = more;
        }
    }
    let paired = candidates::targets(sheet, None, &best, cache.targets(at, how), groups_of_files);
    counts.grain = paired.grain;
    Counted {
        counts,
        join: Some(Join {
            on: best.on,
            column: ColumnRef {
                index: best.index,
                header: best.header.clone(),
            },
            normalize: how,
        }),
        paired: found.rows,
        files: found.files,
        groups_with_rows_for_many_files: paired.groups_with_rows_for_many_files,
        groups: paired.groups,
    }
}

/// Detector 3 on every list of the walk, over the series of `structure`
/// (the proposed patterns, derived into `model` with no list). The plan's
/// `list` edits are replayed; each one it applies is marked in `applied`.
pub(super) fn detect<'d>(
    evidence: &Evidence<'d>,
    structure: &'d Structure,
    model: &'d Model,
    series: &[FolderSeries],
    edits: &[Edit],
    applied: &mut [bool],
) -> Found<'d> {
    let mut cache = Cache {
        model,
        index: evidence.index,
        structure,
        made: HashMap::new(),
    };
    let groups_of_files = candidates::groups_of_files(model, evidence.index);
    let ats: Vec<SeriesIx> = structure.all_series().map(|(at, _)| at).collect();
    let mut counted: Vec<(ListFile, Vec<Counted<'d>>)> = Vec::new();
    let mut unattached = Vec::new();
    for file in evidence.lists.files() {
        let mut read = None;
        evidence.lists.read(&file, &mut |sheet| {
            read = Some(match sheet {
                Ok(sheet) => Ok(ats
                    .iter()
                    .map(|at| {
                        let counts = candidate(evidence, &file);
                        count(&mut cache, &groups_of_files, *at, sheet, counts)
                    })
                    .collect()),
                Err(error) => {
                    let mut counts = candidate(evidence, &file);
                    counts.read = CandidateRead::Error(error.to_string());
                    Err(counts)
                }
            });
        });
        match read {
            Some(Ok(each)) => counted.push((file, each)),
            Some(Err(counts)) => unattached.push(Unattached {
                file,
                counts,
                pairs_nothing: false,
            }),
            None => {}
        }
    }
    let mut chosen: Vec<Vec<usize>> = ats.iter().map(|_| Vec::new()).collect();
    for (list, (file, each)) in counted.iter().enumerate() {
        let most = each.iter().map(|one| one.paired).max().unwrap_or_default();
        let mut at_most = each
            .iter()
            .enumerate()
            .filter(|(_, one)| one.paired == most);
        match (at_most.next(), at_most.next()) {
            (Some((at, _)), None) if most > 0 => chosen[at].push(list),
            // Its best counts: where it pairs the most rows, the first on a tie.
            (Some((_, best)), _) => unattached.push(Unattached {
                file: file.clone(),
                counts: best.counts.clone(),
                pairs_nothing: most == 0,
            }),
            (None, _) => {}
        }
    }
    let mut taken: Vec<Vec<Option<Counted<'d>>>> = counted
        .iter_mut()
        .map(|(_, each)| std::mem::take(each).into_iter().map(Some).collect())
        .collect();
    let files: Vec<ListFile> = counted.iter().map(|(file, _)| file.clone()).collect();
    let mut found = Found {
        series: Vec::new(),
        unattached,
    };
    for (at, mut lists) in chosen.into_iter().enumerate() {
        overlaps(&mut taken, at);
        lists.sort_by_key(|list| {
            let paired = taken[*list][at].as_ref().map_or(0, |one| one.paired);
            (Reverse(paired), files[*list].clone())
        });
        let mut here = SeriesLists::default();
        let mut union: HashSet<&GroupKey> = HashSet::new();
        let mut not_joined = Vec::new();
        for list in lists {
            let Some(one) = taken[list][at].take() else {
                continue;
            };
            if here.joined.is_empty() || !one.groups.is_subset(&union) {
                union.extend(one.groups.iter().copied());
                here.joined.push(InSeries {
                    file: files[list].clone(),
                    counted: one,
                });
            } else {
                not_joined.push(InSeries {
                    file: files[list].clone(),
                    counted: one,
                });
            }
        }
        let others = taken.iter_mut().enumerate().filter_map(|(list, each)| {
            let one = each[at].take_if(|one| one.paired > 0)?;
            Some(InSeries {
                file: files[list].clone(),
                counted: one,
            })
        });
        not_joined.extend(others);
        here.rival = not_joined
            .into_iter()
            .min_by_key(|one| (Reverse(one.counted.paired), one.file.clone()));
        found.series.push(here);
    }
    replay(&mut found, series, edits, applied);
    found
}

/// Each list counted in the series at `at` that pairs a row there gets the
/// other such lists that pair some of the same groups.
fn overlaps(taken: &mut [Vec<Option<Counted<'_>>>], at: usize) {
    let here: Vec<usize> = (0..taken.len())
        .filter(|list| taken[*list][at].as_ref().is_some_and(|one| one.paired > 0))
        .collect();
    let pairs: Vec<(ListCandidate, HashSet<&GroupKey>)> = here
        .iter()
        .filter_map(|list| {
            let one = taken[*list][at].as_ref()?;
            Some((one.counts.clone(), one.groups.clone()))
        })
        .collect();
    let found: Vec<_> = (0..pairs.len())
        .map(|one| candidates::overlaps_of(&pairs, one))
        .collect();
    for (list, overlaps) in here.iter().zip(found) {
        if let Some(one) = taken[*list][at].as_mut() {
            one.counts.overlaps = overlaps;
        }
    }
}

/// Replay the `list` edits: each takes the series' rival, or no list, when
/// that is what the proposal shows. Each edit it applies is marked.
fn replay(found: &mut Found<'_>, series: &[FolderSeries], edits: &[Edit], applied: &mut [bool]) {
    for (at, edit) in edits.iter().enumerate() {
        let Edit::List { scope, list } = edit else {
            continue;
        };
        let Some(here) = series
            .iter()
            .position(|one| one.scopes.contains(scope) || one.folders.contains(scope))
            .and_then(|found_at| found.series.get_mut(found_at))
        else {
            continue;
        };
        let first = here.joined.first().map(|one| &one.file);
        match list {
            Some(asked) if first == Some(asked) => applied[at] = true,
            Some(asked) => {
                if let Some(rival) = here.rival.take_if(|rival| rival.file == *asked) {
                    here.rival = (!here.joined.is_empty()).then(|| here.joined.remove(0));
                    here.joined.insert(0, rival);
                    applied[at] = true;
                }
            }
            None if first.is_none() => applied[at] = true,
            None => {
                let mut joined = std::mem::take(&mut here.joined).into_iter();
                here.rival = joined.next();
                applied[at] = true;
            }
        }
    }
}
