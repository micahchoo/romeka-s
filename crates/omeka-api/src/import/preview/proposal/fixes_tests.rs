//! The corrections on small archives written from general archive
//! practice (`briefs/import-gaps-design.md`, section 4): the replay of each
//! edit, and what the detectors offer. None comes from a held-out archive.

#![allow(clippy::expect_used)]

use super::{Evidence, FoundList, NodeEvidence, Proposal, SeriesNode, propose};
use crate::import::decisions::{FileRef, RowKey, RowRef};
use crate::import::derive::tests::support;
use crate::import::derive::{Index, Sheets};
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

/// The typo scenario of the design: files `2019.4.1` to `2019.4.6` and
/// `2019.4.12`, and a register whose rows describe items, `2019.4.1` to
/// `2019.4.6` and `2019.4.21`. `21` and `12` are two neighbours swapped.
pub(crate) fn typo() -> Archive {
    let mut files: Vec<String> = (1..=6).map(|n| format!("2019.4.{n}.tif")).collect();
    files.push("2019.4.12.tif".into());
    let mut register = String::from("Identifier,Title\n");
    for n in [1, 2, 3, 4, 5, 6, 21] {
        register.push_str(&format!("2019.4.{n},Object {n}\n"));
    }
    Archive { files, list: None }.with_list("register.csv", &register)
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
                cell: "2019.4.21".into(),
            },
            file: path("2019.4.12.tif"),
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
                key: RowKey::new("2019.4.21#1"),
            },
            file: file("2019.4.12.tif"),
        }],
        "the row as the plan keys it, on the series that holds the list"
    );
    assert_eq!(
        only_series(&after).items.chosen.counts.items,
        7,
        "the row joins the item of 2019.4.12 and makes none of its own"
    );
}

#[test]
fn a_correction_row_is_found_by_its_place_in_a_list_read_by_line() {
    let archive = typo();
    let edit = Edit::DescribesNothing {
        list: list_file("register.csv"),
        rows: vec![RowCell {
            line: 8,
            cell: "2019.4.21".into(),
        }],
    };
    let after = archive.propose(std::slice::from_ref(&edit));
    assert!(after.edits[0].applied, "{:?}", after.edits[0]);
    // The proposer reads its lists by line; the plan keys this one by its
    // join column. The derive still finds the row.
    assert_eq!(only_series(&after).items.chosen.counts.items, 7);
}
