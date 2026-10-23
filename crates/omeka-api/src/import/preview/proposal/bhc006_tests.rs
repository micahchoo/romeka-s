//! The BHC006 replica as the proposer's evidence, at model level: the real
//! archive's file list (`omeka-server/tests/import_shapes/bhc006.listing`),
//! each file with the head the server harness writes, sniffed and sorted
//! into media and lists as the scan does, on an install with the default
//! file settings. The catalogues are the harness's: the real `Identifier`
//! cells (`bhc006.identifiers`) with the columns the tests read, the older
//! CSV copy of the first five rows in `Artifacts/`, and an `.ods` that does
//! not read.

#![allow(clippy::expect_used)]

use omeka_media::validation::FileValidation;

use crate::import::catalog::Catalog;
use crate::import::derive::tests::support::catalog;
use crate::import::derive::{Index, IndexedSource, NoTags, Sheets, derive};
use crate::import::evidence::{FileEntry, Walk, sniff};
use crate::import::placement::{Difference, ListFiles, Placement, differences};
use crate::import::plan::{CutEdit, Edit, FolderScope, ListFile, RecordKind, SetFrom};
use crate::import::plan::{ImportPlan, RelPath, SourceId, SourcePlan, default_plan};
use crate::import::preview::candidates::automatic;
use crate::import::preview::proposal::{
    Because, Evidence, Fix, FoundList, NodeEvidence, Proposal, Share, propose,
};
use crate::import::sheet::{self, SheetError};

const LISTING: &str =
    include_str!("../../../../../omeka-server/tests/import_shapes/bhc006.listing");
const IDENTIFIERS: &str =
    include_str!("../../../../../omeka-server/tests/import_shapes/bhc006.identifiers");
/// The frozen outcome: the plan the archive was imported with.
const FROZEN: &str = include_str!("../../fixtures/v3/bhc006.json");

/// The head the server's shape harness writes for a listed file.
fn head(path: &str, size: u64) -> &'static [u8] {
    let extension = path.rsplit_once('.').map_or("", |(_, ext)| ext);
    if size == 0 {
        return b"";
    }
    match extension.to_ascii_lowercase().as_str() {
        "jpg" | "jpeg" => b"\xFF\xD8\xFF\xE0\x00\x10JFIF\x00\x01\x01\x00\x00\x01\x00\x01\x00\x00",
        "tif" | "tiff" | "nef" => b"II*\x00\x08\x00\x00\x00\x00\x00",
        "mp3" => b"ID3\x03\x00\x00\x00\x00\x00\x00",
        "avi" => b"RIFF\x00\x00\x00\x00AVI LIST",
        "mov" => b"\x00\x00\x00\x14ftypqt  \x00\x00\x00\x00qt  ",
        "ods" | "xlsx" => b"PK\x03\x04\x14\x00\x06\x00",
        "ctg" => b"\x00\x00CTG",
        "csv" => b"Sl No,Identifier\n",
        _ => b"// source file\n",
    }
}

/// The replica's one source, as the scan walks it.
pub fn index() -> Index {
    let mut walk = Walk::default();
    for line in LISTING.lines() {
        let (path, size) = line.split_once('\t').expect("a listed file");
        let size: u64 = size.parse().expect("a size");
        let path = RelPath::parse(path).expect("a path");
        let (sniffed, is_sheet) = sniff(&path, head(path.as_str(), size));
        let entry = FileEntry {
            path,
            size,
            modified: None,
            sniffed,
            disk: None,
        };
        if is_sheet {
            walk.sheets.push(entry);
        } else {
            walk.entries.push(entry);
        }
    }
    Index {
        sources: vec![IndexedSource {
            id: SourceId("s1".into()),
            walk,
        }],
    }
}

/// An install with upstream's default file settings.
pub fn install() -> Catalog {
    Catalog {
        files: FileValidation::default(),
        ..catalog()
    }
}

/// The draft's plan before any confirmation.
pub fn draft() -> ImportPlan {
    default_plan(vec![SourcePlan::Folder {
        id: SourceId("s1".into()),
        root: "archive".into(),
        path: RelPath::root(),
    }])
    .expect("the default plan")
}

/// A catalogue as the harness writes it: one row per `Identifier` cell of
/// `name`, or the first `first` cells of the file when `name` is `None`.
fn catalogue(name: Option<&str>, first: usize) -> String {
    let mut out = String::from("Sl No,Identifier,Collection,Curator,Item Type\n");
    let cells = IDENTIFIERS
        .lines()
        .filter_map(|line| line.split_once('\t'))
        .filter(|(sheet, _)| name.is_none_or(|name| *sheet == name))
        .take(first);
    for (n, (sheet, identifier)) in cells.enumerate() {
        let curator = if sheet.contains("_GMT_") {
            "M. Curator"
        } else {
            "R. Curator"
        };
        let kind = match sheet {
            "BHC006_GMT_PN" => "Photo Narrative",
            "BHC006_GMT_D" => "Documents",
            _ => "Artifact",
        };
        out.push_str(&format!(
            "{},{identifier},Citizen Curators,{curator},{kind}\n",
            n + 1
        ));
    }
    out
}

/// Every list file of the walk, read as Simple adds one.
pub fn lists() -> Vec<FoundList> {
    index().sources[0]
        .walk
        .sheets
        .iter()
        .map(|entry| {
            let path = entry.path.as_str();
            let text = match path.rsplit_once('.') {
                Some((stem, "xlsx")) => Some(catalogue(Some(stem), usize::MAX)),
                Some((_, "csv")) => Some(catalogue(None, 5)),
                _ => None,
            };
            let file = ListFile {
                source: SourceId("s1".into()),
                file: entry.path.clone(),
            };
            let plan = automatic(&file.source, &file.file);
            let sheet = text.map_or_else(
                || Err(SheetError::Workbook("not a zip archive".into())),
                |text| sheet::read(text.as_bytes(), &plan.read, None),
            );
            FoundList { file, sheet }
        })
        .collect()
}

/// The proposal for the replica after `edits`, with every list read.
pub fn proposal_with_lists(edits: &[Edit]) -> Proposal {
    let index = index();
    let catalog = install();
    let plan = draft();
    let lists = lists();
    let evidence = Evidence {
        index: &index,
        lists: &lists,
        catalog: &catalog,
        sources: &plan.sources,
        structure: &plan.structure,
        sheets: &Sheets::new(),
    };
    propose(&evidence, edits)
}

/// The proposal for the replica after `edits`, with no list read.
pub fn proposal(edits: &[Edit]) -> Proposal {
    let index = index();
    let catalog = install();
    let plan = draft();
    let evidence = Evidence {
        index: &index,
        lists: &Vec::<FoundList>::new(),
        catalog: &catalog,
        sources: &plan.sources,
        structure: &plan.structure,
        sheets: &Sheets::new(),
    };
    propose(&evidence, edits)
}

/// Where `plan` places the replica's files, each of its lists read as
/// [`lists`] reads it.
fn placed(plan: &ImportPlan) -> Placement {
    let found = lists();
    let sheets: Sheets = plan
        .structure
        .lists()
        .filter_map(|(_, sheet)| {
            let list = found
                .iter()
                .find(|list| list.file.source == sheet.source && list.file.file == sheet.file)?;
            Some((sheet.id.clone(), list.sheet.clone()))
        })
        .collect();
    let model = derive(&index(), &sheets, plan, &install(), &NoTags);
    Placement::of_model(&model, plan, &ListFiles::new(plan, &sheets))
}

/// The judge of the click meter on the replica: the proposal after some
/// edits, and how what it places differs from the frozen outcome.
pub fn judge() -> impl Fn(&[Edit]) -> (Proposal, Vec<Difference>) {
    let frozen = ImportPlan::from_json(FROZEN).expect("the frozen plan");
    let want = placed(&frozen);
    move |edits| {
        let proposal = proposal_with_lists(edits);
        let plan = ImportPlan {
            structure: proposal.structure.clone(),
            edits: edits.to_vec(),
            ..frozen.clone()
        };
        let found = differences(&want, &placed(&plan).with_picks(&proposal));
        (proposal, found)
    }
}

/// The example the structure screen's test marks on `Artifacts`: the letters
/// glued after the number of `BHC006_GMT_AF_010a.jpg`, any letters. The
/// pattern is the one the admin's `patternOf` writes for that mark.
const MARKED_PATH: &str = "Artifacts/BHC006_GMT_AF_010a.jpg";
const MARKED_PATTERN: &str = r"^(?:[^/]*/)*(?<identifier>[^/]*?\d|[^/]*?[^/.][\p{L}\p{M}]*)(?:(?<part>[\p{L}\p{M}]+))?(?:\.[^./]*)?$";

fn artifacts() -> FolderScope {
    FolderScope {
        source: SourceId("s1".into()),
        folder: RelPath::parse("Artifacts").expect("a folder"),
    }
}

/// The proposal after the person marks an example on `Artifacts` from the
/// suggestion: the plan holds the suggested tree with that series' pattern
/// and example written by hand, and `edits`, which end in its `own` edit.
/// The written series' items are checked against the derive the preview
/// runs on that plan, so the count "Written by you" shows is the preview's.
fn marked_by_hand(edits: &[Edit]) -> Proposal {
    let mut structure =
        serde_json::to_value(&proposal_with_lists(&[]).structure).expect("the structure as JSON");
    let series = structure["levels"]
        .as_array_mut()
        .expect("levels")
        .iter_mut()
        .find(|level| level["id"] == "Artifacts")
        .expect("the suggested Artifacts series");
    series["grouping"]["pattern"] = MARKED_PATTERN.into();
    series["example"] = serde_json::json!({
        "path": MARKED_PATH,
        "marks": [{ "kind": "letters", "text": "a", "sep": "" }],
    });
    let mut plan = serde_json::to_value(draft()).expect("the draft as JSON");
    plan["structure"] = structure;
    let plan = ImportPlan::from_json(&plan.to_string()).expect("the plan written by hand");

    let index = index();
    let catalog = install();
    let lists = lists();
    let evidence = Evidence {
        index: &index,
        lists: &lists,
        catalog: &catalog,
        sources: &plan.sources,
        structure: &plan.structure,
        sheets: &Sheets::new(),
    };
    let proposal = propose(&evidence, edits);

    let (at, written) = plan
        .structure
        .all_series()
        .find(|(_, series)| series.id().0 == "Artifacts")
        .expect("the written series");
    let sheets: Sheets = plan
        .structure
        .lists()
        .filter_map(|(_, sheet)| {
            let list = lists
                .iter()
                .find(|list| list.file.source == sheet.source && list.file.file == sheet.file)?;
            Some((sheet.id.clone(), list.sheet.clone()))
        })
        .collect();
    let model = derive(&index, &sheets, &plan, &catalog, &NoTags);
    let made = model
        .kept()
        .filter(|record| record.kind == RecordKind::Item && record.series == at)
        .count();
    let own = proposal
        .nodes
        .iter()
        .find_map(|node| match node {
            NodeEvidence::Own(own) => Some(own),
            NodeEvidence::Set(_) | NodeEvidence::Kept(_) | NodeEvidence::Series(_) => None,
        })
        .expect("the written series' node");
    assert_eq!(own.level, *written.id());
    assert_eq!(
        own.items.items, made,
        "the preview's items of the written series"
    );
    proposal
}

/// The proposal after the person takes Documents' name place 4, uses that
/// tree, then writes Documents by hand (P5 design, probe P3): the set level
/// `Documents (D)4` stays around the written series, which no set node
/// counts.
fn split_then_own() -> Proposal {
    let documents = FolderScope {
        source: SourceId("s1".into()),
        folder: RelPath::parse("Documents (D)").expect("a folder"),
    };
    let split = Edit::AddSet {
        from: SetFrom::NamePlace {
            scope: documents.clone(),
            place: 4,
        },
    };
    let taken = proposal_with_lists(std::slice::from_ref(&split));
    let plan = ImportPlan {
        structure: taken.structure,
        ..draft()
    };
    let index = index();
    let catalog = install();
    let lists = lists();
    let evidence = Evidence {
        index: &index,
        lists: &lists,
        catalog: &catalog,
        sources: &plan.sources,
        structure: &plan.structure,
        sheets: &Sheets::new(),
    };
    propose(&evidence, &[split, Edit::Own { scope: documents }])
}

/// The admin's recorded BHC006 proposals are this replica's, as the
/// preview's `proposal` section sends them: with no edit, after the first of
/// the two pinned clicks (leave out `Extras`), after both (the PN list's
/// rows describe files), after a marked example on `Artifacts`
/// ([`marked_by_hand`]), after "each name an item" there, and with both,
/// where the hand edit mutes the click, and a split taken before Documents is
/// written by hand ([`split_then_own`]). The structure screen's tests read them, so a change
/// of the engine's counts shows there too. `UPDATE_FIXTURES=1` writes them
/// again.
#[test]
fn the_admins_recorded_proposals_are_the_replicas() {
    let extras = Edit::LeaveOut {
        scope: FolderScope {
            source: SourceId("s1".into()),
            folder: RelPath::parse("Extras").expect("a folder"),
        },
    };
    let narratives = Edit::Grain {
        list: ListFile {
            source: SourceId("s1".into()),
            file: RelPath::parse("BHC006_GMT_PN.xlsx").expect("a path"),
        },
        describes: RecordKind::Media,
    };
    let own = Edit::Own { scope: artifacts() };
    let whole = Edit::Items {
        scope: artifacts(),
        cut: CutEdit::Whole,
    };
    let found = proposal_with_lists(&[]);
    let first_pair = af_pairs(&found, 1);
    let every_pair = af_pairs(&found, 4);
    let recorded: [(&str, Proposal); 9] = [
        ("proposal-bhc006.json", found),
        (
            "proposal-bhc006-extras.json",
            proposal_with_lists(std::slice::from_ref(&extras)),
        ),
        (
            "proposal-bhc006-two.json",
            proposal_with_lists(&[extras, narratives]),
        ),
        (
            "proposal-bhc006-own.json",
            marked_by_hand(std::slice::from_ref(&own)),
        ),
        (
            "proposal-bhc006-whole.json",
            proposal_with_lists(std::slice::from_ref(&whole)),
        ),
        (
            "proposal-bhc006-own-waiting.json",
            marked_by_hand(&[whole, own]),
        ),
        ("proposal-bhc006-own-set.json", split_then_own()),
        (
            "proposal-bhc006-pair-one.json",
            proposal_with_lists(&[first_pair]),
        ),
        (
            "proposal-bhc006-pair-four.json",
            proposal_with_lists(&[every_pair]),
        ),
    ];
    for (name, proposal) in recorded {
        let text = serde_json::to_string_pretty(&proposal).expect("the proposal as JSON") + "\n";
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../web/admin/tests/fixtures/import")
            .join(name);
        if std::env::var_os("UPDATE_FIXTURES").is_some() {
            std::fs::write(&path, text).expect("the fixture is written");
        } else {
            assert_eq!(
                std::fs::read_to_string(&path).ok().as_deref(),
                Some(text.as_str()),
                "{name} is out of date; run with UPDATE_FIXTURES=1"
            );
        }
    }
}

/// The `rowIs` edit of the first `take` pairs the replica offers on its AF
/// list as found, as the Structure screen sends them: the admin's tests take
/// one, then the other three as a second click on the same list.
fn af_pairs(found: &Proposal, take: usize) -> Edit {
    let offered = found
        .nodes
        .iter()
        .find_map(|node| match node {
            NodeEvidence::Series(series) => series.fixes.iter().find_map(|fix| match fix {
                Fix::RowIs { list, .. } if list.file.as_str() == "BHC006_GMT_AF.xlsx" => {
                    Some(fix.edit(&series.scope.scope))
                }
                _ => None,
            }),
            _ => None,
        })
        .expect("the AF list offers its row pairs");
    let Edit::RowIs { list, pairs } = offered else {
        unreachable!("a rowIs offer is a rowIs edit")
    };
    assert_eq!(pairs.len(), 4, "the replica as found offers 4 AF pairs");
    Edit::RowIs {
        list,
        pairs: pairs.into_iter().take(take).collect(),
    }
}

/// After its two pinned clicks the replica's AF list has 10 rows that pair
/// no file (design section 0). Four point at exactly one file with no row,
/// and that file at only them. `116a` and `116b` share `116.JPG`, and `184`
/// names an item of two files, so they are not offered.
#[test]
fn bhc006_offers_four_row_pairs() {
    let extras = Edit::LeaveOut {
        scope: FolderScope {
            source: SourceId("s1".into()),
            folder: RelPath::parse("Extras").expect("a folder"),
        },
    };
    let narratives = Edit::Grain {
        list: ListFile {
            source: SourceId("s1".into()),
            file: RelPath::parse("BHC006_GMT_PN.xlsx").expect("a path"),
        },
        describes: RecordKind::Media,
    };
    let proposal = proposal_with_lists(&[extras, narratives]);
    let mut pairs = Vec::new();
    for node in &proposal.nodes {
        let NodeEvidence::Series(series) = node else {
            continue;
        };
        for fix in &series.fixes {
            match fix {
                Fix::RowIs {
                    list,
                    pairs: offered,
                    counts,
                } => {
                    assert_eq!(list.file.as_str(), "BHC006_GMT_AF.xlsx");
                    assert_eq!(
                        counts.rows,
                        Some((Share { n: 680, of: 690 }, Share { n: 684, of: 690 }))
                    );
                    pairs.extend(offered.iter().map(|offer| {
                        (
                            offer.pair.row.cell.clone(),
                            offer.pair.file.to_string(),
                            offer.because,
                        )
                    }));
                }
                Fix::OneItem { .. } | Fix::DescribesNothing { .. } => {
                    panic!("{}: no join and no row left out: {fix:?}", series.level.0)
                }
            }
        }
    }
    let row = |cell: &str, file: &str, because| (cell.to_owned(), file.to_owned(), because);
    assert_eq!(
        pairs,
        [
            row(
                "BHC006_GMT_AF_022.jpg",
                "Artifacts/BHC006_GMT_AF_022a.jpg",
                Because::Near
            ),
            row(
                "BHC_006_GMT_AF_193a.tif",
                "Artifacts/BHC_006_GMT_AF_193a (2).tif",
                Because::NamesPart
            ),
            row(
                "BHC_006_GMT_AF_193b.tif",
                "Artifacts/BHC_006_GMT_AF_193b (2).tif",
                Because::NamesPart
            ),
            row(
                "BHC_006_GMT_AF_300.NEF",
                "Artifacts/BHC_006_GMT_AF_300a.NEF",
                Because::Near
            ),
        ]
    );
    for refused in ["116a", "116b", "_184"] {
        assert!(
            pairs.iter().all(|(cell, _, _)| !cell.contains(refused)),
            "{refused} is not offered"
        );
    }
}
