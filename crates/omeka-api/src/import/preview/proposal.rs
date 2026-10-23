//! The structure proposer (`briefs/import-proposer-design.md`): from the
//! counts a scan already gathers, the archive's tree as a v3 structure, and
//! the evidence of each choice as counts. The server never stores a
//! proposal; only the person's save writes `plan.structure`, and the
//! person's acts are kept as the plan's `edits`.
//!
//! [`propose`] is the one call. It is pure. The detectors run in order,
//! each reading only the scan and what an earlier one chose: `folders`
//! (which folders are series), then `names` (where each series' item key
//! ends; `shape` writes its pattern), then a first derive of those patterns
//! with no list, then `lists` (which list describes each series, counted on
//! that derive), `pairing` (join rules that would pair more rows, offered;
//! a list no stated rule pairs describes the series its offer pairs into)
//! and `grain` (what each list's rows describe). The counts
//! come from a second derive of the whole proposed tree, so the person sees
//! what Run would do. `sets` then counts the sets each series could split
//! into on that derive; every set but a series folder is only offered. Each
//! edit is replayed by the detector whose choice it changes. What the person
//! made that the scan cannot see stays as written: the lists they uploaded,
//! in one series each, and their free sets.
//!
//! The rule against overfitting holds for every non-test file of this
//! module: no file names a token, an extension or a depth. `lint_tests`
//! enforces it, and every number a detector compares with is a documented
//! constant in [`thresholds`].

#[cfg(test)]
pub(crate) mod bhc006_tests;
mod fixes;
#[cfg(test)]
pub(crate) mod fixes_tests;
mod folders;
#[cfg(test)]
mod folders_tests;
mod grain;
#[cfg(test)]
mod grain_tests;
#[cfg(test)]
mod kept_tests;
#[cfg(test)]
mod lint_tests;
mod lists;
#[cfg(test)]
mod lists_tests;
mod names;
#[cfg(test)]
mod names_tests;
#[cfg(test)]
mod own_tests;
mod pairing;
#[cfg(test)]
mod pairing_tests;
#[cfg(test)]
mod results_tests;
mod sets;
#[cfg(test)]
mod sets_tests;
mod shape;
pub mod thresholds;

use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, HashSet};

use indexmap::IndexMap;

use serde::Serialize;

use super::candidates::{self, ColumnGrain, ListCandidate};
use super::columns::EVERY_FOLD;
use super::facts::{Bucket, BucketCount, MediaKind};
use crate::import::catalog::{self, Catalog};
use crate::import::derive::{Index, NoTags, Sheets, derive};
use crate::import::evidence::FileEntry;
use crate::import::flags::format_mismatch;
use crate::import::join::normalize;
use crate::import::model::{Fate, Model};
use crate::import::plan::{
    ColumnOptions, ColumnPlan, ColumnRef, Combine, EachSet, EachValue, Edit, FolderScope, Grouping,
    ImportPlan, Level, LevelId, ListFile, PairRule, Placement as FilePlace, PlanError, RecordKind,
    RelPath, Series, SeriesIx, SeriesScope, SetFrom, SetLevel, SetNames, SetSource, SheetId,
    SheetPlan, SlotName, SlotRule, SourcePlan, SourceRef, Structure, default_plan,
};
use crate::import::sheet::{Sheet, SheetError};
use crate::import::values::ValueType;
pub use fixes::{Because, Fix, FixCounts, JoinOffer, PairOffer, RowOffer, Stale, StaleWhy};
use names::SeriesCut;
use sets::Sets;
use shape::Key;

/// Everything the proposer reads. Nothing else enters a proposal.
pub struct Evidence<'e> {
    /// The scan: every source's walk, with sniffed types and list files.
    pub index: &'e Index,
    /// Every list file of the walk, read as Simple adds one.
    pub lists: &'e dyn Lists,
    /// The install: its file settings are the only media table the
    /// proposer may use; the derives need the rest.
    pub catalog: &'e Catalog,
    /// The draft's sources, so that each scope names a real source.
    pub sources: &'e [SourcePlan],
    /// The draft plan's structure: for [`Proposal::matches_plan`], the
    /// lists' `inPlan`, and what the person made that the scan cannot see
    /// (their free sets, the lists they uploaded, their own series).
    pub structure: &'e Structure,
    /// The plan's own lists, read as the plan reads them, by sheet id. A
    /// list uploaded to staging is only here: the walk never sees it.
    pub sheets: &'e Sheets,
}

/// The list files of the walk, each read on its own: a detector counts one
/// list and lets it go before the next is read, so no proposal holds every
/// sheet at once.
pub trait Lists {
    /// Every list file of the walk, in walk order.
    fn files(&self) -> Vec<ListFile>;
    /// Read one list and hand it to `count`, once. A list that is no longer
    /// there reads as an error.
    fn read(&self, file: &ListFile, count: &mut dyn FnMut(&Result<Sheet, SheetError>));
}

/// One list file of the walk and how it read.
pub struct FoundList {
    pub file: ListFile,
    pub sheet: Result<Sheet, SheetError>,
}

/// Lists already read, for tests that build their evidence in memory.
impl Lists for Vec<FoundList> {
    fn files(&self) -> Vec<ListFile> {
        self.iter().map(|list| list.file.clone()).collect()
    }

    fn read(&self, file: &ListFile, count: &mut dyn FnMut(&Result<Sheet, SheetError>)) {
        if let Some(list) = self.iter().find(|list| list.file == *file) {
            count(&list.sheet);
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Proposal {
    /// The proposed tree, built through [`Structure::build`], so it is
    /// always a valid plan structure; the plan's own tree when the build
    /// refused it (`refused`).
    pub structure: Structure,
    /// The evidence of each node of `structure`, in tree order.
    pub nodes: Vec<NodeEvidence>,
    /// Sets that would hold every item. Offered, never chosen.
    pub top_sets: Vec<Pick<SetFrom, SetCounts>>,
    /// The folders this proposal leaves outside, with their counts.
    pub outside: Vec<OutsideFolder>,
    /// The lists that describe no series, with their best counts.
    pub unattached: Vec<ListCounts>,
    /// Each edit of the plan, and whether it still found what it names.
    pub edits: Vec<EditResult>,
    /// True when `structure` equals the plan's own `structure`.
    pub matches_plan: bool,
    /// Why `structure` is the plan's own tree: the plan check refused the
    /// tree the detectors made. Absent when it did not.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refused: Option<Refused>,
}

/// A tree the proposer made and the plan check refused. The proposal is
/// then the plan's own tree with no evidence, so a save keeps what the
/// person has, and this says why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Refused {
    pub tree: RefusedTree,
    /// The check's words, as a save of that tree would be answered.
    pub reason: String,
}

/// Which of the proposer's trees the plan check refused, in the order it
/// makes them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RefusedTree {
    /// Each series with its item key.
    Items,
    /// With each series' lists and slot rules.
    Lists,
    /// With the sets the edits took.
    Sets,
    /// With the plan's free sets.
    FreeSets,
}

/// One node's evidence. The kinds mirror [`Level`]; a series the person
/// wrote by hand is `own`, and a set level the plan keeps around it is
/// `kept`.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[expect(
    clippy::large_enum_variant,
    reason = "a proposal holds a handful of nodes, made once per preview"
)]
pub enum NodeEvidence {
    Set(SetNode),
    Kept(KeptSetNode),
    Series(SeriesNode),
    Own(OwnNode),
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SeriesNode {
    pub level: LevelId,
    pub scope: ScopeCounts,
    pub items: Choice<ItemCut, CutCounts>,
    /// The list that describes this series, or none. A choice of no list
    /// has no list counts.
    pub list: Choice<Option<ListFile>, Option<ListCounts>>,
    /// What the chosen list's rows describe; `None` with no list.
    pub grain: Option<Choice<RecordKind, GrainCounts>>,
    /// Where each column of that list puts its values; filled only when its
    /// rows describe files.
    pub columns: Vec<ColumnRollUp>,
    /// The name of the series' own set, or no set.
    pub set: Choice<Option<SetFrom>, SetCounts>,
    /// Set levels the series could split into. Offered, never chosen.
    pub splits: Vec<Pick<SetFrom, SetCounts>>,
    /// Join rules that would pair more rows. Offered, never chosen.
    pub pairing: Vec<Pick<PairRule, PairCounts>>,
    /// Corrections of this series and its list, each one click. Offered,
    /// never chosen; absent when there are none.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub fixes: Vec<Fix>,
    /// The person's lists the scan cannot see, kept as written here.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub kept: Vec<KeptList>,
}

/// A series an `own` edit keeps as the person wrote it: counted on the
/// derive of the proposal, with no choice offered on it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OwnNode {
    pub level: LevelId,
    pub scope: ScopeCounts,
    /// The items its own pattern and lists make. No detector counted a cut
    /// here, so it has no parts and no shapes.
    pub items: CutCounts,
    /// The person's lists the scan cannot see, kept as written here.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub kept: Vec<KeptList>,
}

/// A list the person made that the scan cannot see: one they uploaded, or
/// one a free set of theirs reads that no proposed series holds. A proposal
/// keeps it as written, in one series, and never drops it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeptList {
    pub list: SheetId,
    pub file: ListFile,
    /// The rows its own join pairs in this series, of its rows. `None` when
    /// it cannot be read now: it is kept as written all the same.
    pub rows: Option<Share>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetNode {
    pub level: LevelId,
    pub from: SetFrom,
    pub counts: SetCounts,
}

/// A set level around a series written by hand, kept as the plan holds it
/// (`Structure::sets_alone_around`). No detector chose it, so it has no
/// `from`: its `set` is what the plan writes.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeptSetNode {
    pub level: LevelId,
    pub set: SetSource,
    /// The sets it makes on the derive of the proposal; `None` when its
    /// source is not one the proposer counts.
    pub counts: Option<SetCounts>,
}

/// A choice the proposer made, the next best choice, and every other
/// choice it counted. An edit may ask for any of them.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Choice<T, C> {
    pub chosen: Pick<T, C>,
    pub rival: Option<Pick<T, C>>,
    pub others: Vec<Pick<T, C>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Pick<T, C> {
    pub value: T,
    pub counts: C,
}

/// `n` of `of`. With plain counts, the only number shape in evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Share {
    pub n: usize,
    pub of: usize,
}

/// Text the scan read: a folder name, a name place, a cell. Only this
/// module makes one, from scan input, so a detector cannot put its own text
/// into a title or a pattern.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct ScanText(String);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScopeCounts {
    pub scope: FolderScope,
    /// Every file in the scope.
    pub files: usize,
    /// The files that are media: the install's media table accepts them,
    /// they have bytes, and their bytes are what their extension names.
    pub media: usize,
    /// The accepted files by kind, most first.
    pub kinds: Vec<(MediaKind, usize)>,
}

/// Where a series' item key ends.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "cut", rename_all = "camelCase")]
pub enum ItemCut {
    /// Each name is one item. The cut of an archive as found.
    Whole,
    /// The key is the first `places` places of the name.
    Places { places: u8, rule: CutRule },
    /// The key is the folder `depth` levels under the scope. Its files are
    /// the parts.
    Folder { depth: u8 },
    /// No pattern: the rows of a list are the items.
    Rows { list: ListFile, key: ColumnRef },
}

/// Which count made a `Places` cut.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CutRule {
    RecurringParts,
    NamesEndHere,
    SameAsSibling,
    /// The series is one item: after a key that ends in a number, its files
    /// share every place but the last.
    RecurringInItem,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CutCounts {
    /// The items the derive of the proposal makes in the series.
    pub items: usize,
    /// Files in items, of the media in the scope.
    pub files: Share,
    /// Items by files per item, in the histogram's buckets.
    pub per_item: Vec<BucketCount>,
    /// Each part value with the number of items that hold it, most first.
    pub parts: Vec<(ScanText, usize)>,
    /// Names that end exactly at the cut.
    pub names_ending_here: usize,
    /// Media in the scope whose shape the key does not fit. They make no item.
    pub other_shapes: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetCounts {
    /// Each set with the items it would hold, most first.
    pub sets: Vec<(ScanText, usize)>,
    /// For a column: the rows that hold a value, of the rows.
    pub rows: Option<Share>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PairCounts {
    /// Rows the rule pairs to exactly one file of the archive, of the rows.
    pub rows: Share,
    /// How many more rows it pairs than the stated join.
    pub more: usize,
}

/// A list's counts, as the `listCandidates` section counts them.
pub type ListCounts = ListCandidate;
/// What a list's rows describe, as the `listCandidates` section counts it,
/// and what one answer makes of the series, counted on a derive of the
/// proposed tree with that answer.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GrainCounts {
    /// Item groups that more than one paired row names a file of.
    pub groups_with_many_rows: usize,
    /// Rows naming a file of a group that an earlier row named another
    /// file of.
    pub rows_naming_other_files_of_a_group: usize,
    /// Every column of the list, in sheet order.
    pub columns: Vec<ColumnGrain>,
    /// The list's rows that name no file of the import with this answer,
    /// as the `unpaired` section lists them.
    pub rows_naming_no_file: usize,
    /// The items the series keeps with this answer.
    pub items: usize,
}

/// Where a column of a list whose rows describe files puts its values. "Left
/// out" is never chosen by a count; it is the third answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RollUp {
    Item,
    File,
}

/// What one answer of a column's roll-up gives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RollUpCounts {
    /// Items that get at least one value from this column.
    pub items: usize,
    /// Files whose medium gets at least one value from this column.
    pub files: usize,
}

/// One column's roll-up: the count it rests on and the pick, with the other
/// level as its rival.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnRollUp {
    pub index: u32,
    pub header: String,
    pub varies: candidates::Varies,
    pub pick: Choice<RollUp, RollUpCounts>,
}

/// A folder the proposal leaves outside, and why.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutsideFolder {
    #[serde(flatten)]
    pub counts: ScopeCounts,
    pub reason: OutsideReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum OutsideReason {
    /// Nothing in it is media the install accepts.
    NoMedia,
    /// A `leaveOut` edit.
    Edit,
}

/// An edit of the plan, and whether this proposal found what it names. An
/// edit that is not applied is kept and shown, never dropped.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditResult {
    pub edit: Edit,
    pub applied: bool,
    /// Why the edit is not applied; `None` exactly when it is.
    pub reason: Option<NotApplied>,
    /// The instances of a correction that no longer hold. The others are
    /// applied all the same.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub stale: Vec<Stale>,
}

/// Why an edit of the plan is not applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum NotApplied {
    /// The folder, list or column it names is not in the scan.
    Gone,
    /// What it names is there, and the proposal does not offer its choice.
    NotOffered,
    /// A later edit answers the same question otherwise.
    Replaced,
}

/// The proposal for this evidence after these edits. The same evidence and
/// the same edits give the same proposal.
///
/// With no source there is no folder to propose: the proposal is then the
/// plan's own tree, with no evidence. So it is when the plan check refuses
/// a tree the proposer makes, and the proposal then says why.
#[must_use]
pub fn propose(evidence: &Evidence<'_>, edits: &[Edit]) -> Proposal {
    let mut applied = vec![false; edits.len()];
    let none = sets::SeriesSets::default();
    let found = folders::detect(evidence, edits, &mut applied);
    // The detectors after the first never see an edit on a series written
    // by hand: nothing is offered there.
    let every = edits;
    let muted: Vec<bool> = every
        .iter()
        .map(|edit| found.series.iter().any(|one| one.mutes(edit)))
        .collect();
    let (live, mut applied): (Vec<Edit>, Vec<bool>) = every
        .iter()
        .zip(applied)
        .zip(&muted)
        .filter(|(_, muted)| !**muted)
        .map(|((edit, applied), _)| (edit.clone(), applied))
        .unzip();
    let edits = live.as_slice();
    let series = if found.series.is_empty() {
        as_found(evidence)
    } else {
        found.series
    };
    let whole: Vec<Level> = series
        .iter()
        .map(|one| level(one, Some(&Key::Whole), Plan::default(), &none))
        .collect();
    let cuts = Structure::build(whole, found.left.clone(), evidence.sources).map(|placed| {
        let names = series_names(evidence, &placed, series.len());
        let mut cuts: Vec<SeriesCut> = names.iter().map(|names| names::cut(names)).collect();
        if cuts.iter().any(SeriesCut::has_folder_key) {
            let cells = list_cells(evidence);
            for (cut, names) in cuts.iter_mut().zip(&names) {
                names::lead_by_lists(cut, names, &cells);
            }
        }
        names::same_as_sibling(&mut cuts, &names);
        names::replay(&mut cuts, &series, edits, &mut applied);
        (cuts, names)
    });
    let first = cuts.and_then(|(cuts, names)| {
        let levels = series
            .iter()
            .zip(&cuts)
            .map(|(one, cut)| level(one, Some(&cut.chosen.key), Plan::default(), &none))
            .collect();
        let structure = Structure::build(levels, found.left.clone(), evidence.sources)?;
        Ok((
            default_plan(evidence.sources.to_vec())?,
            structure,
            cuts,
            names,
        ))
    });
    let (plan, structure, cuts, names) = match first {
        Ok(first) => first,
        Err(error) => {
            let refused =
                (!evidence.sources.is_empty()).then(|| Refused::of(RefusedTree::Items, &error));
            let edits = results(evidence, every, unmute(applied, &muted), Vec::new());
            return plan_tree(evidence, found.outside, edits, refused);
        }
    };
    let patterns = ImportPlan { structure, ..plan };
    let model = derive(
        evidence.index,
        &Sheets::new(),
        &patterns,
        evidence.catalog,
        &NoTags,
    );
    let scopes: Vec<ScopeCounts> = patterns
        .structure
        .all_series()
        .zip(&series)
        .map(|((at, _), one)| scope_counts(evidence, &patterns.structure, at, one.scope().clone()))
        .collect();
    let mut lists = lists::detect(
        evidence,
        &patterns.structure,
        &model,
        &series,
        edits,
        &mut applied,
    );
    let mut offers = pairing::detect(evidence, &patterns.structure, &mut lists);
    // A series written by hand reads the lists the person gave it; a list
    // the detectors gave it describes no series.
    for (one, here) in series.iter().zip(&mut lists.series) {
        let Some(own) = &one.own else {
            continue;
        };
        let given = std::mem::take(here).joined.into_iter().filter(|given| {
            own.lists()
                .iter()
                .all(|held| held.source != given.file.source || held.file != given.file.file)
        });
        lists
            .unattached
            .extend(given.map(|given| lists::Unattached {
                file: given.file,
                counts: given.counted.counts,
                pairs_nothing: false,
            }));
    }
    let read: Vec<grain::SeriesRead<'_>> = series
        .iter()
        .zip(&scopes)
        .map(|(one, scope)| grain::SeriesRead {
            scope: one.scope(),
            media: scope.media,
            cut_chosen: edits.iter().zip(&applied).any(|(edit, applied)| {
                *applied
                    && matches!(edit, Edit::Items { scope, .. }
                        if one.scopes.contains(scope) || one.folders.contains(scope))
            }),
        })
        .collect();
    let mut rows = grain::detect(evidence, &mut lists, &read, edits, &mut applied);
    pairing::replay(&mut offers, &mut rows, edits, &mut applied);
    let kept = kept_lists(evidence, &patterns.structure, &model, &series, &rows);

    let keys: Vec<Option<&Key>> = cuts
        .iter()
        .zip(&rows)
        .map(|(cut, rows)| rows.items.is_none().then_some(&cut.chosen.key))
        .collect();
    let levels = tree(
        evidence.structure,
        &series,
        &keys,
        &rows,
        &Sets::default(),
        &kept,
    );
    let plan = match Structure::build(levels, found.left.clone(), evidence.sources) {
        Ok(structure) => ImportPlan {
            structure,
            ..patterns.clone()
        },
        Err(error) => {
            let edits = results(evidence, every, unmute(applied, &muted), Vec::new());
            let refused = Refused::of(RefusedTree::Lists, &error);
            return plan_tree(evidence, found.outside, edits, Some(refused));
        }
    };
    // What the person wrote is read as the plan reads it.
    let written: HashSet<&SheetId> = kept
        .iter()
        .map(|one| &one.list.id)
        .chain(
            series
                .iter()
                .filter_map(|one| one.own.as_ref())
                .flat_map(|own| own.lists().iter().map(|list| &list.id)),
        )
        .collect();
    let mut sheets = Sheets::new();
    for (_, list) in plan.structure.lists() {
        let mut keep = |sheet: &Result<Sheet, SheetError>| {
            sheets.insert(list.id.clone(), sheet.clone());
        };
        if written.contains(&list.id) {
            read_written(evidence, list, &mut keep);
        } else {
            evidence.lists.read(&list_file(list), &mut keep);
        }
    }
    let replayed = fixes::replay(
        &fixes::Seen {
            structure: &plan.structure,
            series: &series,
            items: &model,
            sheets: &sheets,
            index: evidence.index,
        },
        edits,
        &mut applied,
    );
    let plan = ImportPlan {
        structure: replayed.onto(plan.structure),
        ..plan
    };
    let model = derive(evidence.index, &sheets, &plan, evidence.catalog, &NoTags);
    // One derive with every series' rival grain counts what each grain
    // click would make: a series' items read only its own files and lists.
    let flipped: Vec<Option<grain::SeriesRows>> = rows
        .iter()
        .map(grain::SeriesRows::with_rival_grain)
        .collect();
    let rival_model = if flipped.iter().any(Option::is_some) {
        let rival_rows: Vec<grain::SeriesRows> = flipped
            .into_iter()
            .zip(&rows)
            .map(|(flipped, rows)| flipped.unwrap_or_else(|| rows.clone()))
            .collect();
        let levels = tree(
            evidence.structure,
            &series,
            &keys,
            &rival_rows,
            &Sets::default(),
            &kept,
        );
        Structure::build(levels, found.left.clone(), evidence.sources)
            .ok()
            .map(|structure| {
                let rival = ImportPlan {
                    structure: replayed.onto(structure),
                    ..patterns.clone()
                };
                derive(evidence.index, &sheets, &rival, evidence.catalog, &NoTags)
            })
    } else {
        None
    };

    let read: Vec<sets::SeriesIn<'_>> = plan
        .structure
        .all_series()
        .zip(&series)
        .zip(keys.iter().zip(&names))
        .zip(&rows)
        .map(|((((at, _), one), (key, names)), rows)| sets::SeriesIn {
            at,
            scope: one.scope(),
            places: key
                .map(|key| names::place_values(names, key))
                .unwrap_or_default(),
            numbered: one.numbered,
            folder: one.set.as_ref(),
            reads: rows
                .lists
                .iter()
                .map(|list| list.file.clone())
                .chain(
                    kept.iter()
                        .filter(|kept| kept.at == at.index() && kept.staged)
                        .map(|kept| list_file(kept.list)),
                )
                .collect(),
        })
        .collect();
    let found_sets = sets::detect(
        &model,
        &plan.structure,
        &sheets,
        &read,
        found.sets_chosen,
        edits,
        &mut applied,
    );
    let stale = unmute_each(replayed.stale.clone(), &muted);
    let edits = results(evidence, every, unmute(applied, &muted), stale);
    let plan = if found_sets.top_taken.is_some()
        || found_sets.series.iter().any(|sets| {
            !sets.taken.is_empty() || sets.folder.as_ref().is_some_and(|folder| folder.chosen)
        }) {
        let levels = tree(
            evidence.structure,
            &series,
            &keys,
            &rows,
            &found_sets,
            &kept,
        );
        match Structure::build(levels, found.left.clone(), evidence.sources) {
            Ok(structure) => ImportPlan {
                structure: replayed.onto(structure),
                ..plan
            },
            Err(error) => {
                let refused = Refused::of(RefusedTree::Sets, &error);
                return plan_tree(evidence, found.outside, edits, Some(refused));
            }
        }
    } else {
        plan
    };
    let free_sets = free_sets(evidence.structure, &plan.structure);
    let plan = match plan
        .structure
        .clone()
        .with_free_sets(free_sets, evidence.sources)
    {
        Ok(structure) => ImportPlan { structure, ..plan },
        Err(error) => {
            let refused = Refused::of(RefusedTree::FreeSets, &error);
            return plan_tree(evidence, found.outside, edits, Some(refused));
        }
    };
    let kept_in = |at: SeriesIx| -> Vec<KeptList> {
        kept.iter()
            .filter(|one| one.at == at.index())
            .map(Kept::shown)
            .collect()
    };

    let mut chosen_lists = lists.series.into_iter();
    let mut offers = offers.into_iter();
    let mut series_sets = found_sets.series.into_iter();
    let mut nodes = Vec::new();
    if let Some((pick, set)) = found_sets.top_taken {
        nodes.push(NodeEvidence::Set(SetNode {
            level: LevelId(set.shown().0.clone()),
            from: pick.value,
            counts: pick.counts,
        }));
    }
    let each = plan
        .structure
        .all_series()
        .zip(cuts.into_iter().zip(rows).zip(scopes));
    for (((at, level), ((cut, rows), scope)), one) in each.zip(&series) {
        let here = chosen_lists.next().unwrap_or_default();
        let sets = series_sets.next().unwrap_or_default();
        let pairing: Vec<Pick<PairRule, PairCounts>> = offers
            .next()
            .unwrap_or_default()
            .into_iter()
            .map(|offer| offer.pick)
            .collect();
        if one.own.is_some() {
            for kept in plan.structure.sets_alone_around(at) {
                nodes.push(NodeEvidence::Kept(KeptSetNode {
                    level: kept.id().clone(),
                    set: kept.set().clone(),
                    counts: sets::kept(
                        &model,
                        &plan.structure,
                        &sheets,
                        evidence.catalog,
                        at,
                        kept.set(),
                    ),
                }));
            }
            nodes.push(NodeEvidence::Own(OwnNode {
                level: level.id().clone(),
                items: cut_counts(&model, at, &scope, None),
                scope,
                kept: kept_in(at),
            }));
            continue;
        }
        let (item, counted, rival, others) = match &rows.items {
            Some((list, key)) => (
                ItemCut::Rows {
                    list: list.clone(),
                    key: key.clone(),
                },
                None,
                Some(cut.chosen),
                cut.rival.into_iter().chain(cut.others).collect(),
            ),
            None => (
                cut.chosen.item,
                Some(cut.chosen.counts),
                cut.rival,
                cut.others,
            ),
        };
        let items = cut_counts(&model, at, &scope, counted.as_ref());
        let other_cut = |other: names::Cut| Pick {
            counts: counted_counts(&other.counts, scope.media),
            value: other.item,
        };
        let rival = rival.map(other_cut);
        let others = others.into_iter().map(other_cut).collect();
        let folder = sets
            .folder
            .as_ref()
            .filter(|folder| folder.chosen)
            .map(|folder| {
                let pick = folder.pick();
                Pick {
                    value: Some(pick.value),
                    counts: pick.counts,
                }
            });
        let mut taken = sets.taken.into_iter();
        let set = match folder {
            Some(folder) => Some(folder),
            // The first split taken is the series' own set when it has none.
            None if rows.set.is_none() => taken.next().map(|first| Pick {
                value: Some(first.pick.value),
                counts: first.pick.counts,
            }),
            None => None,
        };
        let around: Vec<sets::Taken> = taken.collect();
        for taken in around.into_iter().rev() {
            nodes.push(NodeEvidence::Set(SetNode {
                level: split_id(level.id(), &taken),
                from: taken.pick.value,
                counts: taken.pick.counts,
            }));
        }
        let offered_fixes = fixes::detect(
            &model,
            &plan.structure,
            &sheets,
            &fixes::Here {
                at,
                scope: &scope.scope,
                cut: &item,
                pairing_offered: !pairing.is_empty(),
            },
        );
        let list_pick = |one: lists::InSeries<'_>| Pick {
            value: Some(one.file),
            counts: Some(one.counted.counts),
        };
        nodes.push(NodeEvidence::Series(SeriesNode {
            level: level.id().clone(),
            items: Choice {
                chosen: Pick {
                    value: item,
                    counts: items,
                },
                rival,
                others,
            },
            list: Choice {
                chosen: here.joined.into_iter().next().map_or(
                    Pick {
                        value: None,
                        counts: None,
                    },
                    list_pick,
                ),
                rival: here.rival.map(list_pick),
                others: here.others.into_iter().map(list_pick).collect(),
            },
            columns: if rows
                .grain
                .as_ref()
                .is_some_and(|grain| grain.chosen.value == RecordKind::Media)
            {
                rows.columns
            } else {
                Vec::new()
            },
            grain: rows
                .grain
                .zip(plan.structure.series(at).lists().first())
                .map(|(choice, first)| {
                    grain::answers(choice, &model, rival_model.as_ref(), at, &first.id)
                }),
            set: Choice {
                chosen: set.unwrap_or(Pick {
                    value: None,
                    counts: SetCounts {
                        sets: Vec::new(),
                        rows: None,
                    },
                }),
                rival: sets.rival.map(|rival| Pick {
                    value: Some(rival.value),
                    counts: rival.counts,
                }),
                others: sets
                    .others
                    .into_iter()
                    .map(|other| Pick {
                        value: Some(other.value),
                        counts: other.counts,
                    })
                    .collect(),
            },
            splits: sets.splits,
            pairing,
            fixes: offered_fixes,
            scope,
            kept: kept_in(at),
        }));
    }
    Proposal {
        matches_plan: plan.structure == *evidence.structure,
        structure: plan.structure,
        nodes,
        top_sets: found_sets.top,
        outside: found.outside,
        unattached: lists
            .unattached
            .into_iter()
            .filter(|list| kept.iter().all(|one| list_file(one.list) != list.file))
            .map(|list| list.counts)
            .collect(),
        edits,
        refused: None,
    }
}

impl Refused {
    fn of(tree: RefusedTree, error: &PlanError) -> Self {
        Self {
            tree,
            reason: error.to_string(),
        }
    }
}

/// The plan's own tree with no evidence: with no source there is no folder
/// to propose, and otherwise the check `refused` the proposer's tree.
fn plan_tree(
    evidence: &Evidence<'_>,
    outside: Vec<OutsideFolder>,
    edits: Vec<EditResult>,
    refused: Option<Refused>,
) -> Proposal {
    Proposal {
        structure: evidence.structure.clone(),
        nodes: Vec::new(),
        top_sets: Vec::new(),
        outside,
        unattached: Vec::new(),
        edits,
        matches_plan: true,
        refused,
    }
}

/// The applied marks of every edit, from those of the edits that were not
/// muted: a muted edit is not applied.
fn unmute(live: Vec<bool>, muted: &[bool]) -> Vec<bool> {
    let mut live = live.into_iter();
    muted
        .iter()
        .map(|muted| !muted && live.next().unwrap_or_default())
        .collect()
}

/// What each edit got, from what each edit that was not muted got: a muted
/// edit got nothing.
fn unmute_each<T>(live: Vec<Vec<T>>, muted: &[bool]) -> Vec<Vec<T>> {
    let mut live = live.into_iter();
    muted
        .iter()
        .map(|muted| {
            if *muted {
                Vec::new()
            } else {
                live.next().unwrap_or_default()
            }
        })
        .collect()
}

/// Every cell of every list of the walk, folded as a join folds with every
/// fold on.
fn list_cells(evidence: &Evidence<'_>) -> HashSet<String> {
    let mut cells = HashSet::new();
    for file in evidence.lists.files() {
        evidence.lists.read(&file, &mut |sheet| {
            let rows = sheet.iter().flat_map(|sheet| &sheet.rows);
            cells.extend(
                rows.flat_map(|row| &row.cells)
                    .map(|cell| normalize(cell, EVERY_FOLD)),
            );
        });
    }
    cells
}

/// Each edit with whether it is applied, and why not. An applied edit is
/// replaced when a later applied edit answers its question otherwise; an
/// edit no detector applied is gone when the scan no longer has what it
/// names, and not offered otherwise.
fn results(
    evidence: &Evidence<'_>,
    edits: &[Edit],
    applied: Vec<bool>,
    mut stale: Vec<Vec<Stale>>,
) -> Vec<EditResult> {
    stale.resize(edits.len(), Vec::new());
    let reasons: Vec<Option<NotApplied>> = edits
        .iter()
        .zip(&applied)
        .zip(&stale)
        .enumerate()
        .map(|(at, ((edit, done), stale))| {
            let answered_later = || {
                edits
                    .iter()
                    .zip(&applied)
                    .skip(at + 1)
                    .any(|(later, later_done)| {
                        *later_done && later != edit && Question::of(later) == Question::of(edit)
                    })
            };
            if *done {
                answered_later().then_some(NotApplied::Replaced)
            } else if stale.is_empty() && in_scan(evidence, edit) {
                Some(NotApplied::NotOffered)
            } else {
                Some(NotApplied::Gone)
            }
        })
        .collect();
    edits
        .iter()
        .zip(reasons)
        .zip(stale)
        .map(|((edit, reason), stale)| EditResult {
            edit: edit.clone(),
            applied: reason.is_none(),
            reason,
            stale,
        })
        .collect()
}

/// The question of the screen an edit answers: one edit per question.
#[derive(PartialEq, Eq)]
enum Question<'e> {
    Import(&'e FolderScope),
    Items(&'e FolderScope),
    List(&'e FolderScope),
    Grain(&'e ListFile),
    Set(&'e SetFrom),
    Pair(&'e ListFile),
    Title(&'e SetFrom),
    Own(&'e FolderScope),
    /// A correction names its own files and rows, so a later one adds to
    /// it and never answers it otherwise.
    Fix(&'e Edit),
}

impl<'e> Question<'e> {
    fn of(edit: &'e Edit) -> Self {
        match edit {
            Edit::LeaveOut { scope } | Edit::Keep { scope } => Self::Import(scope),
            Edit::Items { scope, .. } => Self::Items(scope),
            Edit::List { scope, .. } => Self::List(scope),
            Edit::Grain { list, .. } => Self::Grain(list),
            Edit::AddSet { from } | Edit::DropSet { from } => Self::Set(from),
            Edit::Pair { list, .. } => Self::Pair(list),
            Edit::Title { from, .. } | Edit::Existing { from, .. } => Self::Title(from),
            Edit::Own { scope } => Self::Own(scope),
            Edit::OneItem { .. } | Edit::RowIs { .. } | Edit::DescribesNothing { .. } => {
                Self::Fix(edit)
            }
        }
    }
}

/// Whether the scan still has every folder, list and column the edit names.
fn in_scan(evidence: &Evidence<'_>, edit: &Edit) -> bool {
    let folder = |scope: &FolderScope| {
        evidence
            .index
            .sources
            .iter()
            .filter(|source| source.id == scope.source)
            .flat_map(|source| source.walk.entries.iter().chain(&source.walk.sheets))
            .any(|entry| {
                let mut segments = entry.path.segments();
                scope
                    .folder
                    .segments()
                    .all(|segment| segments.next() == Some(segment))
            })
    };
    let files = evidence.lists.files();
    let list = |file: &ListFile| files.contains(file);
    let column = |file: &ListFile, column: &ColumnRef| {
        let mut found = false;
        if list(file) {
            evidence.lists.read(file, &mut |sheet| {
                found = sheet.as_ref().is_ok_and(|sheet| {
                    sheet
                        .columns
                        .iter()
                        .any(|one| one.index == column.index && one.header == column.header)
                });
            });
        }
        found
    };
    let set = |from: &SetFrom| match from {
        SetFrom::Folder { scope }
        | SetFrom::NamePlace { scope, .. }
        | SetFrom::FolderLevel { scope, .. } => folder(scope),
        SetFrom::Column {
            list,
            column: named,
        } => column(list, named),
    };
    match edit {
        Edit::LeaveOut { scope }
        | Edit::Keep { scope }
        | Edit::Items { scope, .. }
        | Edit::Own { scope }
        | Edit::OneItem { scope, .. } => folder(scope),
        Edit::List { scope, list: asked } => folder(scope) && asked.as_ref().is_none_or(list),
        Edit::Grain { list: file, .. }
        | Edit::Pair { list: file, .. }
        | Edit::RowIs { list: file, .. }
        | Edit::DescribesNothing { list: file, .. } => list(file),
        Edit::AddSet { from }
        | Edit::DropSet { from }
        | Edit::Title { from, .. }
        | Edit::Existing { from, .. } => set(from),
    }
}

/// What a series holds beyond its scope and pattern: its lists as the plan
/// reads them, its set from rows, its slot rules, and the value maps the
/// person wrote on its sets.
#[derive(Default)]
struct Plan {
    lists: Vec<SheetPlan>,
    set: Option<SetSource>,
    rules: Vec<(RecordKind, SlotName, SlotRule)>,
    maps: WrittenMaps,
}

impl Plan {
    /// Detector 4's choices for one series, each list with a sheet id not in
    /// `ids`: its file's path, and its source's id after it when another
    /// list has that path.
    ///
    /// The person's choices in `kept`, the plan's structure, stay: a list
    /// the plan holds keeps how it is read and where its columns go, and
    /// the series takes the slot rules of the plan's series that holds one
    /// of its lists, else of the plan's series of the same scope. A rule
    /// that reads a list this series does not hold is not taken, and the
    /// key rules the rows need win over the person's. Each list of
    /// `kept_lists` follows the proposed ones as written, with its id.
    fn of(
        rows: &grain::SeriesRows,
        ids: &mut BTreeSet<SheetId>,
        taken: &[sets::Taken],
        kept: &Structure,
        scope: &SeriesScope,
        kept_lists: &[&SheetPlan],
    ) -> Self {
        let mut of_file = BTreeMap::new();
        let mut of_kept: BTreeMap<SheetId, SheetId> = BTreeMap::new();
        let mut lists: Vec<SheetPlan> =
            rows.lists
                .iter()
                .map(|list| {
                    let mut id = SheetId(list.file.file.as_str().to_owned());
                    if ids.contains(&id) {
                        id.0.push_str(&list.file.source.0);
                    }
                    ids.insert(id.clone());
                    of_file.insert(list.file.clone(), id.clone());
                    let mut plan = candidates::automatic(&list.file.source, &list.file.file);
                    plan.id = id;
                    plan.row_kind = list.row_kind.clone();
                    plan.join = list.join.clone();
                    plan.item_key = list.item_key.clone();
                    let mut mapped = list.mapped.clone();
                    let split = taken.iter().filter_map(|taken| match &taken.column {
                        Some((file, column)) if *file == list.file => Some(column),
                        _ => None,
                    });
                    for column in split {
                        if !mapped.contains(column) {
                            mapped.push(column.clone());
                        }
                    }
                    let held = kept.lists().map(|(_, held)| held).find(|held| {
                        held.source == list.file.source && held.file == list.file.file
                    });
                    let mut columns = held.map(|held| held.columns.clone()).unwrap_or_default();
                    for column in mapped {
                        if columns.iter().all(|kept| kept.column.index != column.index) {
                            columns.push(ColumnPlan {
                                column,
                                options: ColumnOptions {
                                    split: None,
                                    value_type: ValueType::Literal,
                                    lang: None,
                                    private: false,
                                },
                            });
                        }
                    }
                    plan.columns = columns;
                    if let Some(held) = held {
                        plan.read = held.read.clone();
                        of_kept.insert(held.id.clone(), plan.id.clone());
                    }
                    plan
                })
                .collect();
        for list in kept_lists {
            of_kept.insert(list.id.clone(), list.id.clone());
            lists.push((*list).clone());
        }
        let column = |file: &ListFile, column: &ColumnRef| {
            Some(SourceRef::Column {
                sheet: of_file.get(file)?.clone(),
                column: column.clone(),
            })
        };
        let maps = WrittenMaps::of(kept, scope);
        let set = rows.set.as_ref().and_then(|(file, set)| {
            Some(SetSource::Each(EachValue {
                each: EachSet {
                    sources: vec![column(file, set)?],
                    combine: Combine::First,
                    map: maps.map(&[Reads::Column(file.clone(), set.clone())]),
                    names: SetNames::Key,
                },
            }))
        });
        let mut rules = kept_rules(kept, scope, &of_kept);
        rules.extend(rows.keys.iter().filter_map(|(kind, file, key)| {
            let rule = SlotRule {
                sources: vec![column(file, key)?],
                combine: Combine::First,
                map: IndexMap::new(),
            };
            Some((*kind, SlotName::ItemKey, rule))
        }));
        Self {
            lists,
            set,
            rules,
            maps,
        }
    }
}

/// What an `each` set reads. A column is named by its list's file, so a
/// list the proposal gives another sheet id still reads the same column.
#[derive(PartialEq)]
enum Reads {
    Column(ListFile, ColumnRef),
    Other(SourceRef),
}

impl Reads {
    fn of_taken(taken: &sets::Taken) -> Self {
        match &taken.column {
            Some((file, column)) => Self::Column(file.clone(), column.clone()),
            None => Self::Other(taken.source.clone()),
        }
    }
}

/// The value maps the person wrote on the `each` sets of one series of the
/// plan: its own set and the set levels around it alone. The proposer makes
/// those sets again on every click; each one it makes that reads the same
/// sources takes the person's map, as a kept list keeps its columns.
#[derive(Default)]
struct WrittenMaps(Vec<(Vec<Reads>, IndexMap<String, String>)>);

impl WrittenMaps {
    /// The maps of the plan's series of `scope`; none when the plan has no
    /// such series.
    fn of(kept: &Structure, scope: &SeriesScope) -> Self {
        let Some((at, series)) = kept
            .all_series()
            .find(|(_, series)| series.scope() == scope)
        else {
            return Self::default();
        };
        let reads = |source: &SourceRef| match source {
            SourceRef::Column { sheet, column } => kept.list(sheet).map_or_else(
                || Reads::Other(source.clone()),
                |(_, list)| Reads::Column(list_file(list), column.clone()),
            ),
            other => Reads::Other(other.clone()),
        };
        let sets = kept
            .sets_alone_around(at)
            .into_iter()
            .map(SetLevel::set)
            .chain(series.set());
        Self(
            sets.filter_map(|set| match set {
                SetSource::Each(EachValue { each }) if !each.map.is_empty() => {
                    Some((each.sources.iter().map(reads).collect(), each.map.clone()))
                }
                _ => None,
            })
            .collect(),
        )
    }

    /// The person's map of the set that reads `reads`; empty when they wrote
    /// none.
    fn map(&self, reads: &[Reads]) -> IndexMap<String, String> {
        self.0
            .iter()
            .find(|(written, _)| written.as_slice() == reads)
            .map(|(_, map)| map.clone())
            .unwrap_or_default()
    }
}

/// The slot rules of the plan's series that holds a list of `of_kept` (the
/// plan's sheet ids, each with this series' id for it), else of the plan's
/// series of `scope`. Each list it reads is renamed to this series' id; a
/// rule that reads another list is left behind.
fn kept_rules(
    kept: &Structure,
    scope: &SeriesScope,
    of_kept: &BTreeMap<SheetId, SheetId>,
) -> Vec<(RecordKind, SlotName, SlotRule)> {
    let each: Vec<&Series> = kept.all_series().map(|(_, series)| series).collect();
    let held = each
        .iter()
        .find(|series| {
            series
                .lists()
                .iter()
                .any(|list| of_kept.contains_key(&list.id))
        })
        .or_else(|| each.iter().find(|series| series.scope() == scope));
    let Some(held) = held else {
        return Vec::new();
    };
    let renamed = |rule: &SlotRule| -> Option<SlotRule> {
        let sources = rule
            .sources
            .iter()
            .map(|source| match source {
                SourceRef::Column { sheet, column } => Some(SourceRef::Column {
                    sheet: of_kept.get(sheet)?.clone(),
                    column: column.clone(),
                }),
                other => Some(other.clone()),
            })
            .collect::<Option<Vec<_>>>()?;
        Some(SlotRule {
            sources,
            ..rule.clone()
        })
    };
    [RecordKind::Item, RecordKind::Media, RecordKind::ItemSet]
        .into_iter()
        .flat_map(|kind| {
            held.rules(kind)
                .iter()
                .filter(|(slot, _)| **slot != SlotName::Sets)
                .filter_map(move |(slot, rule)| Some((kind, slot.clone(), renamed(rule)?)))
        })
        .collect()
}

/// A list the person made that the scan cannot see, and the series it goes
/// to.
struct Kept<'e> {
    list: &'e SheetPlan,
    /// By [`SeriesIx::index`].
    at: usize,
    /// Its paired rows there, of its rows; `None` when it cannot be read.
    rows: Option<Share>,
    /// The person uploaded it, so the series reads it. Otherwise a free set
    /// reads it and the series only holds it.
    staged: bool,
}

impl Kept<'_> {
    fn shown(&self) -> KeptList {
        KeptList {
            list: self.list.id.clone(),
            file: list_file(self.list),
            rows: self.rows,
        }
    }
}

/// The lists of the plan the scan cannot see: each list the person uploaded
/// to staging, and each list a free set reads that no proposed or written
/// series holds. Each goes to the written series that holds it, else to the
/// series whose scope is its plan series' scope, else to the series where
/// its own join pairs the most rows on `model` (the derive of `patterns`),
/// the first on a tie.
fn kept_lists<'e>(
    evidence: &Evidence<'e>,
    patterns: &Structure,
    model: &Model,
    series: &[folders::FolderSeries],
    rows: &[grain::SeriesRows],
) -> Vec<Kept<'e>> {
    let staged = |list: &SheetPlan| {
        evidence
            .sources
            .iter()
            .any(|source| matches!(source, SourcePlan::Staging { id } if *id == list.source))
    };
    let proposed = |list: &SheetPlan| {
        let file = list_file(list);
        rows.iter()
            .flat_map(|rows| &rows.lists)
            .any(|one| one.file == file)
    };
    let written = |list: &SheetPlan| {
        series.iter().position(|one| {
            one.own
                .as_ref()
                .is_some_and(|own| own.lists().iter().any(|held| held.id == list.id))
        })
    };
    let read_by_sets: HashSet<&SheetId> = evidence
        .structure
        .free_sets()
        .iter()
        .filter_map(|set| match set {
            SetSource::Each(each) => Some(&each.each.sources),
            SetSource::Id(_) | SetSource::Title(_) => None,
        })
        .flatten()
        .filter_map(|source| match source {
            SourceRef::Column { sheet, .. } => Some(sheet),
            _ => None,
        })
        .collect();
    let ats: Vec<SeriesIx> = patterns.all_series().map(|(at, _)| at).collect();
    evidence
        .structure
        .all_series()
        .flat_map(|(_, held)| held.lists().iter().map(move |list| (held, list)))
        .filter(|(_, list)| {
            staged(list)
                || (read_by_sets.contains(&list.id) && !proposed(list) && written(list).is_none())
        })
        .map(|(held, list)| {
            let mut read = None;
            read_written(evidence, list, &mut |sheet| {
                if let Ok(sheet) = sheet {
                    let paired: Vec<usize> = ats
                        .iter()
                        .map(|at| {
                            list.join.as_ref().map_or(0, |join| {
                                lists::paired_by(evidence.index, patterns, model, *at, sheet, join)
                            })
                        })
                        .collect();
                    read = Some((paired, sheet.rows.len()));
                }
            });
            let same_scope = series
                .iter()
                .position(|one| SeriesScope::Folders(one.scopes.clone()) == *held.scope());
            let most_paired = || {
                let paired = read.as_ref().map(|(paired, _)| paired.as_slice());
                (0..ats.len())
                    .max_by_key(|at| {
                        let n = paired.and_then(|paired| paired.get(*at)).copied();
                        (n.unwrap_or_default(), Reverse(*at))
                    })
                    .unwrap_or_default()
            };
            let at = written(list).or(same_scope).unwrap_or_else(most_paired);
            Kept {
                list,
                at,
                staged: staged(list),
                rows: read.as_ref().map(|(paired, of)| Share {
                    n: paired.get(at).copied().unwrap_or_default(),
                    of: *of,
                }),
            }
        })
        .collect()
}

/// Read a list the person wrote as the plan reads it: from the plan's own
/// sheets, else from the walk.
fn read_written(
    evidence: &Evidence<'_>,
    list: &SheetPlan,
    count: &mut dyn FnMut(&Result<Sheet, SheetError>),
) {
    match evidence.sheets.get(&list.id) {
        Some(sheet) => count(sheet),
        None => evidence.lists.read(&list_file(list), count),
    }
}

fn list_file(list: &SheetPlan) -> ListFile {
    ListFile {
        source: list.source.clone(),
        file: list.file.clone(),
    }
}

/// The plan's free sets, each list a set reads renamed to the proposal's
/// list of the same file. The proposer makes no free set, so each one is the
/// person's, unless the proposal's tree already makes a set from the same
/// columns: that set is the proposer's answer, and an edit changes it.
fn free_sets(kept: &Structure, proposed: &Structure) -> Vec<SetSource> {
    let renamed = |sheet: &SheetId| {
        kept.list(sheet)
            .and_then(|(_, held)| {
                proposed
                    .lists()
                    .find(|(_, list)| list.source == held.source && list.file == held.file)
            })
            .map_or_else(|| sheet.clone(), |(_, list)| list.id.clone())
    };
    let made: Vec<&Vec<SourceRef>> = proposed
        .all_series()
        .flat_map(|(at, _)| proposed.memberships(at))
        .filter_map(|set| match set {
            SetSource::Each(each) => Some(&each.each.sources),
            SetSource::Id(_) | SetSource::Title(_) => None,
        })
        .collect();
    kept.free_sets()
        .iter()
        .filter_map(|set| match set {
            SetSource::Each(each) => {
                let sources: Vec<SourceRef> = each
                    .each
                    .sources
                    .iter()
                    .map(|source| match source {
                        SourceRef::Column { sheet, column } => SourceRef::Column {
                            sheet: renamed(sheet),
                            column: column.clone(),
                        },
                        other => other.clone(),
                    })
                    .collect();
                (!made.contains(&&sources)).then(|| {
                    SetSource::Each(EachValue {
                        each: EachSet {
                            sources,
                            ..each.each.clone()
                        },
                    })
                })
            }
            other => Some(other.clone()),
        })
        .collect()
}

/// The archive as found: one series per source, over the whole source.
fn as_found(evidence: &Evidence<'_>) -> Vec<folders::FolderSeries> {
    evidence
        .sources
        .iter()
        .map(|source| {
            let scope = FolderScope {
                source: source.id().clone(),
                folder: RelPath::root(),
            };
            folders::FolderSeries {
                folders: vec![scope.clone()],
                scopes: vec![scope],
                set: None,
                numbered: false,
                own: None,
            }
        })
        .collect()
}

/// The proposed levels: each series with its key, lists and rules, and
/// its sets; a series written by hand as the person wrote it, inside the
/// set levels the plan holds around it alone. Each list in
/// `kept_lists` goes, as written, after the lists of its series. The first
/// split taken is a series' own set when it has none; each other split is a
/// set level around the series; a top set taken is a set level around
/// everything.
fn tree(
    kept: &Structure,
    series: &[folders::FolderSeries],
    keys: &[Option<&Key>],
    rows: &[grain::SeriesRows],
    sets: &Sets,
    kept_lists: &[Kept<'_>],
) -> Vec<Level> {
    // A written series and a kept list keep their sheet ids, so no proposed
    // list takes one.
    let mut ids: BTreeSet<SheetId> = series
        .iter()
        .filter_map(|one| one.own.as_ref())
        .flat_map(|own| own.lists().iter().map(|list| list.id.clone()))
        .chain(kept_lists.iter().map(|one| one.list.id.clone()))
        .collect();
    let none = sets::SeriesSets::default();
    let levels: Vec<Level> = series
        .iter()
        .zip(keys)
        .zip(rows)
        .enumerate()
        .map(|(at, ((one, key), rows))| {
            let here: Vec<&SheetPlan> = kept_lists
                .iter()
                .filter(|kept| kept.at == at)
                .map(|kept| kept.list)
                .collect();
            if let Some(own) = &one.own {
                let more = here
                    .into_iter()
                    .filter(|list| own.lists().iter().all(|held| held.id != list.id));
                let lists = own.lists().iter().chain(more).cloned().collect();
                let written = Level::Series(own.clone().with_lists(lists));
                return match kept.all_series().find(|(_, held)| held.id() == own.id()) {
                    Some((held, _)) => kept.inside_its_sets(held, written),
                    None => written,
                };
            }
            let sets = sets.series.get(at).unwrap_or(&none);
            let scope = SeriesScope::Folders(one.scopes.clone());
            let plan = Plan::of(rows, &mut ids, &sets.taken, kept, &scope, &here);
            level(one, *key, plan, sets)
        })
        .collect();
    match &sets.top_taken {
        Some((_, set)) => vec![Level::Set(SetLevel::new(
            LevelId(set.shown().0.clone()),
            set.source(),
            levels,
        ))],
        None => levels,
    }
}

/// The id of the set level a split taken around the series `series` makes.
fn split_id(series: &LevelId, taken: &sets::Taken) -> LevelId {
    let mut id = series.0.clone();
    id.push_str(&taken.name);
    LevelId(id)
}

/// The plan's series for one detected series. Its id is its folder's name,
/// or its source's id when it takes the whole source. With no key, the files
/// make no item of their own: the rows of a list are the items.
fn level(
    one: &folders::FolderSeries,
    key: Option<&Key>,
    plan: Plan,
    sets: &sets::SeriesSets,
) -> Level {
    let taken = &sets.taken;
    let scope = one.scope();
    let id = if scope.folder.as_str().is_empty() {
        scope.source.0.clone()
    } else {
        scope.folder.as_str().to_owned()
    };
    let place = taken.iter().find_map(|taken| taken.place.as_ref());
    let pattern = key.map(|key| match place {
        Some(place) => key.pattern_with_set(place),
        None => key.pattern(),
    });
    let id = LevelId(id);
    let mut series = Series::new(
        id.clone(),
        SeriesScope::Folders(one.scopes.clone()),
        Grouping {
            pattern,
            case_fold: false,
        },
    )
    .with_lists(plan.lists);
    for (kind, slot, rule) in plan.rules {
        series = series.with_rule(kind, slot, rule);
    }
    let folder = sets
        .folder
        .as_ref()
        .filter(|folder| folder.chosen)
        .map(|folder| folder.set.source());
    let each = |taken: &sets::Taken| {
        SetSource::Each(EachValue {
            each: EachSet {
                sources: vec![taken.source.clone()],
                combine: Combine::First,
                map: plan.maps.map(&[Reads::of_taken(taken)]),
                names: SetNames::Title,
            },
        })
    };
    let mut taken = taken.iter();
    let own = folder.or(plan.set).or_else(|| taken.next().map(each));
    let mut level = Level::Series(match own {
        Some(set) => series.with_set(set),
        None => series,
    });
    for around in taken {
        level = Level::Set(SetLevel::new(
            split_id(&id, around),
            each(around),
            vec![level],
        ));
    }
    level
}

/// Whether the install's media table accepts the file.
fn accepted(evidence: &Evidence<'_>, entry: &FileEntry) -> bool {
    catalog::refusal(&evidence.catalog.files, entry.name(), &entry.sniffed).is_none()
}

/// Whether the file is media: the media table accepts it, it has bytes, and
/// its bytes are what its extension names. Any other file makes no item.
fn is_media(evidence: &Evidence<'_>, entry: &FileEntry) -> bool {
    entry.size > 0
        && accepted(evidence, entry)
        && format_mismatch(entry.name(), &entry.sniffed).is_none()
}

/// The media among `entries` by kind, most first.
fn kinds_most_first<'e>(
    evidence: &Evidence<'_>,
    entries: impl Iterator<Item = &'e FileEntry>,
) -> Vec<(MediaKind, usize)> {
    let mut kinds: BTreeMap<MediaKind, usize> = BTreeMap::new();
    for entry in entries {
        if is_media(evidence, entry) {
            *kinds
                .entry(MediaKind::of(&entry.sniffed, entry.path.as_str()))
                .or_default() += 1;
        }
    }
    let mut kinds: Vec<(MediaKind, usize)> = kinds.into_iter().collect();
    kinds.sort_by_key(|(kind, files)| (Reverse(*files), *kind));
    kinds
}

/// The files the series at `at` takes, by the rule the engine places them.
fn scope_counts(
    evidence: &Evidence<'_>,
    structure: &Structure,
    at: SeriesIx,
    scope: FolderScope,
) -> ScopeCounts {
    let entries: Vec<&FileEntry> = evidence
        .index
        .sources
        .iter()
        .flat_map(|source| source.walk.entries.iter().map(move |entry| (source, entry)))
        .filter(|(source, entry)| {
            matches!(
                structure.place(&source.id, &entry.path),
                FilePlace::Series { at: placed, .. } if placed == at
            )
        })
        .map(|(_, entry)| entry)
        .collect();
    let kinds = kinds_most_first(evidence, entries.iter().copied());
    ScopeCounts {
        scope,
        files: entries.len(),
        media: kinds.iter().map(|(_, files)| files).sum(),
        kinds,
    }
}

/// Each series' media, as its pattern sees them in `placed`, by
/// [`SeriesIx::index`].
fn series_names<'e>(
    evidence: &Evidence<'e>,
    placed: &Structure,
    series: usize,
) -> Vec<Vec<names::Name<'e>>> {
    let mut out: Vec<Vec<names::Name<'e>>> = (0..series).map(|_| Vec::new()).collect();
    for source in &evidence.index.sources {
        for entry in &source.walk.entries {
            let FilePlace::Series { at, path } = placed.place(&source.id, &entry.path) else {
                continue;
            };
            let media = is_media(evidence, entry);
            if let Some(name) = media.then(|| names::Name::of(entry, path)).flatten()
                && let Some(names) = out.get_mut(at.index())
            {
                names.push(name);
            }
        }
    }
    out
}

/// Items by files per item, in the histogram's buckets.
fn buckets(per_item: &[usize]) -> Vec<BucketCount> {
    let mut buckets: BTreeMap<Bucket, usize> =
        Bucket::ALL.iter().map(|bucket| (*bucket, 0)).collect();
    for files in per_item {
        *buckets.entry(Bucket::of(*files)).or_default() += 1;
    }
    buckets
        .into_iter()
        .map(|(files, count)| BucketCount { files, count })
        .collect()
}

/// The counts of a cut the proposal did not take, as detector 2 counted it.
fn counted_counts(counted: &names::Counted, media: usize) -> CutCounts {
    CutCounts {
        items: counted.items,
        files: Share {
            n: counted.files,
            of: media,
        },
        per_item: buckets(&counted.per_item),
        parts: counted.parts.clone(),
        names_ending_here: counted.names_ending_here,
        other_shapes: counted.other_shapes,
    }
}

/// The counts of a series' cut: the items and their files from the derive
/// of the proposal, the parts and shapes as detector 2 counted them. Items
/// made from rows have no parts and no shapes.
fn cut_counts(
    model: &Model,
    at: SeriesIx,
    scope: &ScopeCounts,
    counted: Option<&names::Counted>,
) -> CutCounts {
    let per_item: Vec<usize> = model
        .kept()
        .filter(|record| record.kind == RecordKind::Item && record.series == at)
        .map(|record| {
            record
                .media
                .iter()
                .filter(|media| !matches!(model.record(**media).fate, Fate::Left(_)))
                .count()
        })
        .collect();
    CutCounts {
        items: per_item.len(),
        files: Share {
            n: per_item.iter().sum(),
            of: scope.media,
        },
        per_item: buckets(&per_item),
        parts: counted
            .map(|counted| counted.parts.clone())
            .unwrap_or_default(),
        names_ending_here: counted.map_or(0, |counted| counted.names_ending_here),
        other_shapes: counted.map_or(0, |counted| counted.other_shapes),
    }
}
