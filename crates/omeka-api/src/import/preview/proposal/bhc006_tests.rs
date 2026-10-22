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
use crate::import::plan::{Edit, ListFile};
use crate::import::plan::{ImportPlan, RelPath, SourceId, SourcePlan, default_plan};
use crate::import::preview::candidates::automatic;
use crate::import::preview::proposal::{Evidence, FoundList, Proposal, propose};
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
            decoded: None,
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
        let found = differences(&want, &placed(&plan));
        (proposal, found)
    }
}
