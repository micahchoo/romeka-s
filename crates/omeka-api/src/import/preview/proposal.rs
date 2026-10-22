//! The structure proposer (`briefs/import-proposer-design.md`): from the
//! counts a scan already gathers, the archive's tree as a v3 structure, and
//! the evidence of each choice as counts. The server never stores a
//! proposal; only the person's save writes `plan.structure`, and the
//! person's acts are kept as the plan's `edits`.
//!
//! [`propose`] is the one call. It is pure. No detector exists yet, so it
//! proposes the archive as found: one series per source, each name one
//! item, no list and no set. Its counts come from a derive of that tree, so
//! the person sees what Run would do.
//!
//! The rule against overfitting holds for every non-test file of this
//! module: no file names a token, an extension or a depth. `lint_tests`
//! enforces it, and every number a detector compares with is a documented
//! constant in [`thresholds`].

#[cfg(test)]
mod lint_tests;
pub mod thresholds;

use std::cmp::Reverse;
use std::collections::BTreeMap;

use serde::Serialize;

use super::candidates::{Grain, ListCandidate};
use super::facts::{Bucket, BucketCount, MediaKind};
use crate::import::catalog::{self, Catalog};
use crate::import::derive::{Index, NoTags, Sheets, derive};
use crate::import::model::{Fate, Model};
use crate::import::plan::{
    ColumnRef, Edit, FolderScope, Grouping, ImportPlan, Level, LevelId, ListFile, NAME_ALONE,
    PairRule, Placement as FilePlace, RecordKind, RelPath, Series, SeriesIx, SeriesScope, SetFrom,
    SourcePlan, Structure, default_plan,
};
use crate::import::sheet::{Sheet, SheetError};

/// Everything the proposer reads. Nothing else enters a proposal.
pub struct Evidence<'e> {
    /// The scan: every source's walk, with sniffed types and list files.
    pub index: &'e Index,
    /// Every list file of the walk, read as Simple adds one.
    pub lists: &'e [FoundList],
    /// The install: its file settings are the only media table the
    /// proposer may use; the derives need the rest.
    pub catalog: &'e Catalog,
    /// The draft's sources, so that each scope names a real source.
    pub sources: &'e [SourcePlan],
    /// The draft plan's structure. Only [`Proposal::matches_plan`] reads it.
    pub structure: &'e Structure,
}

/// One list file of the walk and how it read.
pub struct FoundList {
    pub file: ListFile,
    pub sheet: Result<Sheet, SheetError>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Proposal {
    /// The proposed tree, built through [`Structure::build`], so it is
    /// always a valid plan structure.
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
}

/// One node's evidence. The kinds mirror [`Level`].
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[expect(
    clippy::large_enum_variant,
    reason = "a proposal holds a handful of nodes, made once per preview"
)]
pub enum NodeEvidence {
    Set(SetNode),
    Series(SeriesNode),
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
    /// The name of the series' own set, or no set.
    pub set: Choice<Option<SetFrom>, SetCounts>,
    /// Set levels the series could split into. Offered, never chosen.
    pub splits: Vec<Pick<SetFrom, SetCounts>>,
    /// Join rules that would pair more rows. Offered, never chosen.
    pub pairing: Vec<Pick<PairRule, PairCounts>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetNode {
    pub level: LevelId,
    pub from: SetFrom,
    pub counts: SetCounts,
}

/// A choice the proposer made, and the next best choice.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Choice<T, C> {
    pub chosen: Pick<T, C>,
    pub rival: Option<Pick<T, C>>,
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
    /// The files the install's media table accepts.
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
    /// Rows it pairs that the stated join does not.
    pub more: usize,
}

/// A list's counts, as the `listCandidates` section counts them.
pub type ListCounts = ListCandidate;
/// What a list's rows describe, as the `listCandidates` section counts it.
pub type GrainCounts = Grain;

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
}

/// The proposal for this evidence after these edits. The same evidence and
/// the same edits give the same proposal.
///
/// With no source there is no folder to propose: the proposal is then the
/// plan's own tree, with no evidence.
#[must_use]
pub fn propose(evidence: &Evidence<'_>, edits: &[Edit]) -> Proposal {
    let found: Vec<(FolderScope, ItemCut)> = evidence
        .sources
        .iter()
        .map(|source| {
            let scope = FolderScope {
                source: source.id().clone(),
                folder: RelPath::root(),
            };
            (scope, ItemCut::Whole)
        })
        .collect();
    let levels = found
        .iter()
        .map(|(scope, _)| {
            Level::Series(Series::new(
                LevelId(scope.source.0.clone()),
                SeriesScope::Folders(vec![scope.clone()]),
                Grouping {
                    pattern: Some(NAME_ALONE.to_owned()),
                    case_fold: false,
                },
            ))
        })
        .collect();
    let edits = edits
        .iter()
        .map(|edit| EditResult {
            edit: edit.clone(),
            applied: false,
        })
        .collect();
    let built = Structure::build(levels, evidence.sources)
        .and_then(|structure| Ok((default_plan(evidence.sources.to_vec())?, structure)));
    let Ok((plan, structure)) = built else {
        return Proposal {
            structure: evidence.structure.clone(),
            nodes: Vec::new(),
            top_sets: Vec::new(),
            outside: Vec::new(),
            unattached: Vec::new(),
            edits,
            matches_plan: true,
        };
    };
    let plan = ImportPlan { structure, ..plan };
    let model = derive(
        evidence.index,
        &Sheets::new(),
        &plan,
        evidence.catalog,
        &NoTags,
    );
    let nodes = plan
        .structure
        .all_series()
        .zip(found)
        .map(|((at, series), (scope, cut))| {
            let scope = scope_counts(evidence, &plan.structure, at, scope);
            let items = cut_counts(&model, at, &scope);
            NodeEvidence::Series(SeriesNode {
                level: series.id().clone(),
                scope,
                items: Choice {
                    chosen: Pick {
                        value: cut,
                        counts: items,
                    },
                    rival: None,
                },
                list: Choice {
                    chosen: Pick {
                        value: None,
                        counts: None,
                    },
                    rival: None,
                },
                grain: None,
                set: Choice {
                    chosen: Pick {
                        value: None,
                        counts: SetCounts {
                            sets: Vec::new(),
                            rows: None,
                        },
                    },
                    rival: None,
                },
                splits: Vec::new(),
                pairing: Vec::new(),
            })
        })
        .collect();
    Proposal {
        matches_plan: plan.structure == *evidence.structure,
        structure: plan.structure,
        nodes,
        top_sets: Vec::new(),
        outside: Vec::new(),
        unattached: Vec::new(),
        edits,
    }
}

/// The files the series at `at` takes, by the rule the engine places them.
fn scope_counts(
    evidence: &Evidence<'_>,
    structure: &Structure,
    at: SeriesIx,
    scope: FolderScope,
) -> ScopeCounts {
    let entries: Vec<_> = evidence
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
    let mut kinds: BTreeMap<MediaKind, usize> = BTreeMap::new();
    for entry in &entries {
        if catalog::refusal(&evidence.catalog.files, entry.name(), &entry.sniffed).is_none() {
            *kinds
                .entry(MediaKind::of(&entry.sniffed, entry.path.as_str()))
                .or_default() += 1;
        }
    }
    let mut kinds: Vec<(MediaKind, usize)> = kinds.into_iter().collect();
    kinds.sort_by_key(|(kind, files)| (Reverse(*files), *kind));
    ScopeCounts {
        scope,
        files: entries.len(),
        media: kinds.iter().map(|(_, files)| files).sum(),
        kinds,
    }
}

/// The counts of a series' cut, from the derive of the proposal. Every
/// name ends at the whole-name cut, and every name fits it.
fn cut_counts(model: &Model, at: SeriesIx, scope: &ScopeCounts) -> CutCounts {
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
    let mut buckets: BTreeMap<Bucket, usize> =
        Bucket::ALL.iter().map(|bucket| (*bucket, 0)).collect();
    for files in &per_item {
        *buckets.entry(Bucket::of(*files)).or_default() += 1;
    }
    CutCounts {
        items: per_item.len(),
        files: Share {
            n: per_item.iter().sum(),
            of: scope.media,
        },
        per_item: buckets
            .into_iter()
            .map(|(files, count)| BucketCount { files, count })
            .collect(),
        parts: Vec::new(),
        names_ending_here: scope.media,
        other_shapes: 0,
    }
}
