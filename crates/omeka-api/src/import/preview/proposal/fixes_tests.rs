//! The corrections on small archives written from general archive
//! practice: the replay of each
//! edit, and what the detectors offer. None comes from a held-out archive.

#![allow(clippy::expect_used)]

use std::collections::BTreeMap;

use serde_json::{Value as Json, json};

use super::{Evidence, Fix, FoundList, NodeEvidence, Proposal, SeriesNode, propose};
use crate::import::decisions::{FileRef, RowKey, RowRef};
use crate::import::derive::tests::support;
use crate::import::derive::{Index, Sheets};
use crate::import::derive::{NoTags, derive};
use crate::import::placement::{Difference, ListFiles, Placement, differences};
use crate::import::plan::{
    Correction, Edit, FolderScope, ImportPlan, ItemJoin, ListFile, RelPath, RowCell, RowPair,
    SourceId,
};
use crate::import::preview::candidates::automatic;
use crate::import::sheet;

/// An archive of image files under one source, and at most one list.
pub(crate) struct Archive {
    pub files: Vec<String>,
    pub list: Option<(String, String)>,
}

impl Archive {
    /// Files at these paths, all TIFF images, and no list.
    pub(crate) fn of(files: &[&str]) -> Self {
        Self {
            files: files.iter().map(|file| (*file).to_owned()).collect(),
            list: None,
        }
    }

    /// The archive with a list at `path` that holds `text`.
    pub(crate) fn with_list(self, path: &str, text: &str) -> Self {
        Self {
            list: Some((path.to_owned(), text.to_owned())),
            ..self
        }
    }

    pub(crate) fn index(&self) -> Index {
        let files: Vec<(&str, &str)> = self
            .files
            .iter()
            .map(|file| (file.as_str(), support::TIFF))
            .collect();
        support::index(&files)
    }

    /// The list as Simple reads it.
    pub(crate) fn lists(&self) -> Vec<FoundList> {
        self.list
            .iter()
            .map(|(path, text)| {
                let file = ListFile {
                    source: SourceId("s1".into()),
                    file: RelPath::parse(path).expect("a path"),
                };
                let read = automatic(&file.source, &file.file).read;
                FoundList {
                    sheet: sheet::read(text.as_bytes(), &read, None),
                    file,
                }
            })
            .collect()
    }

    /// The draft plan on this archive's one source.
    pub(crate) fn draft() -> ImportPlan {
        support::plan(|_| {})
    }

    /// The proposal after `edits`.
    pub(crate) fn propose(&self, edits: &[Edit]) -> Proposal {
        let index = self.index();
        let lists = self.lists();
        let draft = Self::draft();
        let catalog = support::catalog();
        let evidence = Evidence {
            index: &index,
            lists: &lists,
            catalog: &catalog,
            sources: &draft.sources,
            structure: &draft.structure,
            sheets: &Sheets::new(),
        };
        propose(&evidence, edits)
    }
}

pub(crate) fn scope(folder: &str) -> FolderScope {
    FolderScope {
        source: SourceId("s1".into()),
        folder: RelPath::parse(folder).expect("a folder"),
    }
}

pub(crate) fn list_file(path: &str) -> ListFile {
    ListFile {
        source: SourceId("s1".into()),
        file: RelPath::parse(path).expect("a path"),
    }
}

pub(crate) fn path(text: &str) -> RelPath {
    RelPath::parse(text).expect("a path")
}

/// The proposal's one series.
pub(crate) fn only_series(proposal: &Proposal) -> &SeriesNode {
    let mut series = proposal.nodes.iter().filter_map(|node| match node {
        NodeEvidence::Series(series) => Some(series),
        _ => None,
    });
    let first = series.next().expect("a series");
    assert!(series.next().is_none(), "one series");
    first
}

/// Every correction the proposal's tree holds.
pub(crate) fn corrections(proposal: &Proposal) -> Vec<Correction> {
    proposal
        .structure
        .corrections()
        .map(|(_, correction)| correction.clone())
        .collect()
}

/// Objects of two sides each, front and back.
pub(crate) fn sides() -> Archive {
    Archive::of(&[
        "Objects/2019.4.2_front.tif",
        "Objects/2019.4.2_back.tif",
        "Objects/2019.4.3_front.tif",
        "Objects/2019.4.3_back.tif",
        "Objects/2019.4.30_front.tif",
        "Objects/2019.4.30_back.tif",
    ])
}

/// The typo scenario of the design: files `2019-4-1` to `2019-4-6` and
/// `2019-4-12`, and a register whose rows describe items, `2019-4-1` to
/// `2019-4-6` and `2019-4-21`. `21` and `12` are two neighbours swapped.
/// The design writes the numbers with dots; detector 6 then offers to pair
/// by the last place, because it reads `.21` as an ending, and its offer
/// keeps detector 8 silent. Dashes keep this case about the typo.
pub(crate) fn typo() -> Archive {
    numbered_typo("2019-4")
}

/// The typo scenario under another accession prefix.
pub(crate) fn numbered_typo(prefix: &str) -> Archive {
    let mut files: Vec<String> = (1..=6).map(|n| format!("{prefix}-{n}.tif")).collect();
    files.push(format!("{prefix}-12.tif"));
    let rows: Vec<String> = [1, 2, 3, 4, 5, 6, 21]
        .iter()
        .map(|n| format!("{prefix}-{n}"))
        .collect();
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    archive(&files, Some(&register(&rows)))
}

fn join(item: &str, with: &str) -> ItemJoin {
    ItemJoin {
        item: path(item),
        with: path(with),
    }
}

fn file(text: &str) -> FileRef {
    FileRef {
        source: SourceId("s1".into()),
        path: path(text),
    }
}

#[test]
fn one_item_replays_into_joins_of_every_file() {
    let archive = sides();
    let before = archive.propose(&[]);
    assert_eq!(only_series(&before).items.chosen.counts.items, 3);
    let edit = Edit::OneItem {
        scope: scope("Objects"),
        joins: vec![join(
            "Objects/2019.4.2_front.tif",
            "Objects/2019.4.3_back.tif",
        )],
    };
    let after = archive.propose(std::slice::from_ref(&edit));
    assert!(after.edits[0].applied, "{:?}", after.edits[0]);
    assert_eq!(
        corrections(&after),
        [
            Correction::Join {
                file: file("Objects/2019.4.3_back.tif"),
                with: file("Objects/2019.4.2_front.tif"),
            },
            Correction::Join {
                file: file("Objects/2019.4.3_front.tif"),
                with: file("Objects/2019.4.2_front.tif"),
            },
        ],
        "both sides of 2019.4.3 move, not only the file the join names"
    );
    assert_eq!(
        only_series(&after).items.chosen.counts.items,
        2,
        "the counts are the derive's with the corrections"
    );
}

#[test]
fn row_is_replays_into_a_pair_of_the_series_sheet() {
    let archive = typo();
    let before = archive.propose(&[]);
    assert_eq!(
        only_series(&before).items.chosen.counts.items,
        8,
        "seven files and the row 2019.4.21, which names none"
    );
    let edit = Edit::RowIs {
        list: list_file("register.csv"),
        pairs: vec![RowPair {
            row: RowCell {
                line: 8,
                cell: "2019-4-21".into(),
            },
            file: path("2019-4-12.tif"),
        }],
    };
    let after = archive.propose(std::slice::from_ref(&edit));
    assert!(after.edits[0].applied, "{:?}", after.edits[0]);
    let (at, list) = after.structure.lists().next().expect("the register");
    assert_eq!(
        after.structure.series(at).corrections(),
        [Correction::Pair {
            row: RowRef {
                sheet: list.id.clone(),
                key: RowKey::new("2019-4-21#1"),
            },
            file: file("2019-4-12.tif"),
        }],
        "the row as the plan keys it, on the series that holds the list"
    );
    assert_eq!(
        only_series(&after).items.chosen.counts.items,
        7,
        "the row joins the item of 2019-4-12 and makes none of its own"
    );
}

#[test]
fn a_correction_row_is_found_by_its_place_in_a_list_read_by_line() {
    let archive = typo();
    let edit = Edit::DescribesNothing {
        list: list_file("register.csv"),
        rows: vec![RowCell {
            line: 8,
            cell: "2019-4-21".into(),
        }],
    };
    let after = archive.propose(std::slice::from_ref(&edit));
    assert!(after.edits[0].applied, "{:?}", after.edits[0]);
    // The proposer reads its lists by line; the plan keys this one by its
    // join column. The derive still finds the row.
    assert_eq!(only_series(&after).items.chosen.counts.items, 7);
}

/// An open scenario: the archive, and the plan a person reaches today on
/// the draft's proposal with Advanced decisions (`move`, `pair`, `leave`).
pub(crate) struct Scenario {
    pub archive: Archive,
    pub wanted: ImportPlan,
}

impl Scenario {
    /// The draft's proposal with these decisions, given as JSON that may
    /// name the proposal's list by its sheet id.
    pub(crate) fn new(archive: Archive, decisions: impl FnOnce(&Proposal) -> Json) -> Self {
        let proposal = archive.propose(&[]);
        let mut plan = serde_json::to_value(Archive::draft()).expect("the draft as JSON");
        plan["structure"] = serde_json::to_value(&proposal.structure).expect("the structure");
        plan["decisions"] = decisions(&proposal);
        let wanted = ImportPlan::from_json(&plan.to_string())
            .unwrap_or_else(|error| panic!("{error}: {plan}"));
        Self { archive, wanted }
    }

    /// Where `plan` places the archive's files, each list read as Run
    /// reads it, by the plan's own read and key.
    pub(crate) fn placed(&self, plan: &ImportPlan) -> Placement {
        let found = self.archive.lists();
        let sheets: Sheets = plan
            .structure
            .lists()
            .filter_map(|(_, list)| {
                let (_, text) = self.archive.list.as_ref()?;
                found.iter().find(|one| one.file.file == list.file)?;
                Some((
                    list.id.clone(),
                    sheet::read(text.as_bytes(), &list.read, sheet::key_column(list)),
                ))
            })
            .collect();
        let index = self.archive.index();
        let model = derive(&index, &sheets, plan, &support::catalog(), &NoTags);
        Placement::of_model(&model, plan, &ListFiles::new(plan, &sheets))
    }

    /// The meter's judge: the proposal after some edits, and how what it
    /// places differs from the wanted plan. The person's own decisions are
    /// what the edits must make, so the judged plan holds none.
    pub(crate) fn judge(&self) -> impl Fn(&[Edit]) -> (Proposal, Vec<Difference>) + '_ {
        let want = self.placed(&self.wanted);
        move |edits| {
            let proposal = self.archive.propose(edits);
            let plan = ImportPlan {
                structure: proposal.structure.clone(),
                edits: edits.to_vec(),
                decisions: Vec::new(),
                ..self.wanted.clone()
            };
            let found = differences(&want, &self.placed(&plan).with_picks(&proposal));
            (proposal, found)
        }
    }

    /// The differences of the archive as found, by kind.
    pub(crate) fn as_found(&self) -> BTreeMap<&'static str, usize> {
        let (_, found) = self.judge()(&[]);
        kinds(&found)
    }
}

/// Differences by kind, as the meter names them.
pub(crate) fn kinds(found: &[Difference]) -> BTreeMap<&'static str, usize> {
    let mut by_kind = BTreeMap::new();
    for difference in found {
        *by_kind
            .entry(crate::import::clicks::kind(difference))
            .or_default() += 1;
    }
    by_kind
}

pub(crate) fn move_with(file: &str, with: &str) -> Json {
    json!({ "kind": "move", "file": { "file": { "source": "s1", "path": file } },
            "to": { "with": { "file": { "source": "s1", "path": with } } } })
}

/// The proposal's one list's sheet id.
pub(crate) fn sheet_of(proposal: &Proposal) -> String {
    proposal
        .structure
        .lists()
        .next()
        .map(|(_, list)| list.id.0.clone())
        .expect("a list")
}

pub(crate) fn pair_row(proposal: &Proposal, key: &str, file: &str) -> Json {
    json!({ "kind": "pair", "row": { "sheet": sheet_of(proposal), "key": key },
            "file": { "source": "s1", "path": file } })
}

pub(crate) fn leave_row(proposal: &Proposal, key: &str) -> Json {
    json!({ "kind": "leave", "entry": { "row": { "sheet": sheet_of(proposal), "key": key } } })
}

/// A register whose rows describe items: one row per identifier.
pub(crate) fn register(identifiers: &[&str]) -> String {
    let mut text = String::from("Identifier,Title\n");
    for identifier in identifiers {
        text.push_str(&format!("{identifier},Object {identifier}\n"));
    }
    text
}

/// Every TIFF named by these stems.
pub(crate) fn tiffs(stems: &[&str]) -> Vec<String> {
    stems.iter().map(|stem| format!("{stem}.tif")).collect()
}

/// The archive of `tiffs`, with a list `register.csv` when `rows` is some.
pub(crate) fn archive(files: &[String], rows: Option<&str>) -> Archive {
    let names: Vec<&str> = files.iter().map(String::as_str).collect();
    let found = Archive::of(&names);
    match rows {
        Some(rows) => found.with_list("register.csv", rows),
        None => found,
    }
}

pub(crate) fn parts_named_by_the_whole() -> Scenario {
    let files = tiffs(&[
        "1987.12",
        "1987.12a",
        "2019.4.2",
        "2019.4.2a",
        "2019.4.2b",
        "2019.4.3",
    ]);
    Scenario::new(archive(&files, None), |_| {
        json!([
            move_with("1987.12a.tif", "1987.12.tif"),
            move_with("2019.4.2a.tif", "2019.4.2.tif"),
            move_with("2019.4.2b.tif", "2019.4.2.tif"),
        ])
    })
}

pub(crate) fn a_range_part() -> Scenario {
    let files = tiffs(&[
        "1987.12",
        "1987.12a",
        "2019.4.2",
        "2019.4.2a",
        "2019.4.2b",
        "2019.4.3",
        "2019.4.5",
        "2019.4.5a-b",
    ]);
    Scenario::new(archive(&files, None), |_| {
        json!([
            move_with("1987.12a.tif", "1987.12.tif"),
            move_with("2019.4.2a.tif", "2019.4.2.tif"),
            move_with("2019.4.2b.tif", "2019.4.2.tif"),
            move_with("2019.4.5a-b.tif", "2019.4.5.tif"),
        ])
    })
}

pub(crate) fn a_row_one_typo_from_a_file() -> Scenario {
    Scenario::new(typo(), |proposal| {
        json!([pair_row(proposal, "2019-4-21#1", "2019-4-12.tif")])
    })
}

pub(crate) fn an_ambiguous_row() -> Scenario {
    let files = tiffs(&[
        "2019-4-1", "2019-4-2", "2019-4-3", "2019-4-4", "2019-4-5", "2019-4-6", "2019-4-8",
        "2019-4-9",
    ]);
    let rows = register(&[
        "2019-4-1", "2019-4-2", "2019-4-3", "2019-4-4", "2019-4-5", "2019-4-6", "2019-4-7",
    ]);
    Scenario::new(archive(&files, Some(&rows)), |proposal| {
        json!([pair_row(proposal, "2019-4-7#1", "2019-4-8.tif")])
    })
}

pub(crate) fn rows_for_things_never_digitized() -> Scenario {
    let files = tiffs(&["2019-4-1", "2019-4-2", "2019-4-3", "2019-4-4"]);
    let rows = register(&[
        "2019-4-1", "2019-4-2", "2019-4-3", "2019-4-4", "1953-77", "1953-88",
    ]);
    Scenario::new(archive(&files, Some(&rows)), |proposal| {
        json!([
            leave_row(proposal, "1953-77#1"),
            leave_row(proposal, "1953-88#1"),
        ])
    })
}

pub(crate) fn a_row_naming_a_part() -> Scenario {
    let files: Vec<String> = [
        "GMT_AF_191a.tif",
        "GMT_AF_191b.tif",
        "GMT_AF_192a.tif",
        "GMT_AF_192b.tif",
        "GMT_AF_194a.tif",
        "GMT_AF_194b.tif",
        "GMT_AF_193a (2).tif",
        "GMT_AF_193b (2).tif",
    ]
    .map(str::to_owned)
    .to_vec();
    let mut rows = String::from("File,View\n");
    for name in [
        "GMT_AF_191a.tif",
        "GMT_AF_191b.tif",
        "GMT_AF_192a.tif",
        "GMT_AF_192b.tif",
        "GMT_AF_194a.tif",
        "GMT_AF_194b.tif",
        "GMT_AF_193a.tif",
        "GMT_AF_193b.tif",
    ] {
        rows.push_str(&format!("{name},View of {name}\n"));
    }
    Scenario::new(archive(&files, Some(&rows)), |proposal| {
        json!([
            pair_row(proposal, "GMT_AF_193a.tif#1", "GMT_AF_193a (2).tif"),
            pair_row(proposal, "GMT_AF_193b.tif#1", "GMT_AF_193b (2).tif"),
        ])
    })
}

/// Parts cut as two items, because the archive's keys have two lengths,
/// and one register row for both.
pub(crate) fn one_row_for_two_parts() -> Scenario {
    let files = tiffs(&["1987-12", "2019-4-1", "2019-4-2a", "2019-4-2b", "2019-4-3"]);
    let rows = register(&["1987-12", "2019-4-1", "2019-4-2", "2019-4-3"]);
    Scenario::new(archive(&files, Some(&rows)), |proposal| {
        json!([
            move_with("2019-4-2b.tif", "2019-4-2a.tif"),
            pair_row(proposal, "2019-4-2#1", "2019-4-2a.tif"),
        ])
    })
}

/// The first scenario's parts and the typo, in one archive.
pub(crate) fn a_typo_and_a_split() -> Scenario {
    let typo = numbered_typo("2020-1");
    let mut files = typo.files.clone();
    files.extend(tiffs(&[
        "1987.12",
        "1987.12a",
        "2019.4.2",
        "2019.4.2a",
        "2019.4.2b",
        "2019.4.3",
    ]));
    let rows = typo.list.map(|(_, rows)| rows).expect("the register");
    Scenario::new(archive(&files, Some(&rows)), |proposal| {
        json!([
            move_with("1987.12a.tif", "1987.12.tif"),
            move_with("2019.4.2a.tif", "2019.4.2.tif"),
            move_with("2019.4.2b.tif", "2019.4.2.tif"),
            pair_row(proposal, "2020-1-21#1", "2020-1-12.tif"),
        ])
    })
}

/// The proposal's offers on its one series, each as its kind and its
/// instances in a line of text.
fn offers(scenario: &Scenario) -> Vec<(&'static str, Vec<String>)> {
    let proposal = scenario.archive.propose(&[]);
    only_series(&proposal)
        .fixes
        .iter()
        .map(|fix| match fix {
            Fix::OneItem { joins, .. } => (
                "oneItem",
                joins
                    .iter()
                    .map(|offer| {
                        format!(
                            "{} <- {} ({:?})",
                            offer.join.item, offer.join.with, offer.because
                        )
                    })
                    .collect(),
            ),
            Fix::RowIs { pairs, .. } => (
                "rowIs",
                pairs
                    .iter()
                    .map(|offer| {
                        format!(
                            "{} {} = {} ({:?})",
                            offer.pair.row.line,
                            offer.pair.row.cell,
                            offer.pair.file,
                            offer.because
                        )
                    })
                    .collect(),
            ),
            Fix::DescribesNothing { rows, .. } => (
                "describesNothing",
                rows.iter()
                    .map(|offer| format!("{} {}", offer.row.line, offer.row.cell))
                    .collect(),
            ),
        })
        .collect()
}

fn found(kinds: &[(&'static str, usize)]) -> BTreeMap<&'static str, usize> {
    kinds.iter().copied().collect()
}

#[test]
fn parts_named_by_the_whole_are_one_item() {
    let scenario = parts_named_by_the_whole();
    assert_eq!(scenario.as_found(), found(&[("filesSplit", 2)]));
    assert_eq!(
        offers(&scenario),
        [(
            "oneItem",
            vec![
                "1987.12.tif <- 1987.12a.tif (KeyExtends)".to_owned(),
                "2019.4.2.tif <- 2019.4.2a.tif (KeyExtends)".to_owned(),
                "2019.4.2.tif <- 2019.4.2b.tif (KeyExtends)".to_owned(),
            ]
        )],
        "two key lengths, so each name is an item; `2019.4.3` extends no key"
    );
}

#[test]
fn a_range_part_joins_its_object() {
    let scenario = a_range_part();
    assert_eq!(scenario.as_found(), found(&[("filesSplit", 3)]));
    let offered = offers(&scenario);
    assert_eq!(offered.len(), 1);
    assert_eq!(offered[0].0, "oneItem");
    assert!(
        offered[0]
            .1
            .contains(&"2019.4.5.tif <- 2019.4.5a-b.tif (KeyExtends)".to_owned()),
        "a range is two places more, fewer than the key's three: {offered:?}"
    );
    assert_eq!(offered[0].1.len(), 4);
}

#[test]
fn a_row_one_typo_from_a_file_pairs_it() {
    let scenario = a_row_one_typo_from_a_file();
    assert_eq!(scenario.as_found(), found(&[("row", 1), ("rowItem", 1)]));
    assert_eq!(
        offers(&scenario),
        [(
            "rowIs",
            vec!["8 2019-4-21 = 2019-4-12.tif (Near)".to_owned()]
        )],
        "two neighbours swapped are one edit"
    );
}

#[test]
fn an_ambiguous_row_is_not_offered() {
    let scenario = an_ambiguous_row();
    assert_eq!(scenario.as_found(), found(&[("row", 1), ("rowItem", 1)]));
    assert_eq!(
        offers(&scenario),
        [],
        "2019-4-7 is one edit from both 2019-4-8 and 2019-4-9: a pair would be a guess"
    );
}

#[test]
fn rows_for_things_never_digitized_leave() {
    let scenario = rows_for_things_never_digitized();
    assert_eq!(scenario.as_found(), found(&[("row", 2), ("rowItem", 2)]));
    assert_eq!(
        offers(&scenario),
        [(
            "describesNothing",
            vec!["6 1953-77".to_owned(), "7 1953-88".to_owned()]
        )]
    );
}

#[test]
fn a_row_naming_a_part_pairs_that_file() {
    let scenario = a_row_naming_a_part();
    assert_eq!(scenario.as_found(), found(&[("row", 2)]));
    assert_eq!(
        offers(&scenario),
        [(
            "rowIs",
            vec![
                "8 GMT_AF_193a.tif = GMT_AF_193a (2).tif (NamesPart)".to_owned(),
                "9 GMT_AF_193b.tif = GMT_AF_193b (2).tif (NamesPart)".to_owned(),
            ]
        )],
        "the rows describe files: each names the item and the part one file has"
    );
}

#[test]
fn one_row_for_two_parts_joins_them() {
    let scenario = one_row_for_two_parts();
    assert_eq!(
        scenario.as_found(),
        found(&[("filesSplit", 1), ("rowItem", 1)])
    );
    assert_eq!(
        offers(&scenario),
        [(
            "oneItem",
            vec!["2019-4-2a.tif <- 2019-4-2b.tif (RowNamesBoth)".to_owned()]
        )]
    );
}

#[test]
fn a_typo_and_a_split_need_two_corrections() {
    let scenario = a_typo_and_a_split();
    assert_eq!(
        scenario.as_found(),
        found(&[("filesSplit", 2), ("row", 1), ("rowItem", 1)])
    );
    let kinds: Vec<&str> = offers(&scenario).iter().map(|(kind, _)| *kind).collect();
    assert_eq!(kinds, ["oneItem", "rowIs"]);
}
