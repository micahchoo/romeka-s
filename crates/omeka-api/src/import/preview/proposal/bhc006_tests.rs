//! The BHC006 replica as the proposer's evidence, at model level: the real
//! archive's file list (`omeka-server/tests/import_shapes/bhc006.listing`),
//! each file with the head the server harness writes, sniffed and sorted
//! into media and lists as the scan does, on an install with the default
//! file settings.

#![allow(clippy::expect_used)]

use omeka_media::validation::FileValidation;

use crate::import::catalog::Catalog;
use crate::import::derive::tests::support::catalog;
use crate::import::derive::{Index, IndexedSource};
use crate::import::evidence::{FileEntry, Walk, sniff};
use crate::import::plan::Edit;
use crate::import::plan::{ImportPlan, RelPath, SourceId, SourcePlan, default_plan};
use crate::import::preview::proposal::{Evidence, Proposal, propose};

const LISTING: &str =
    include_str!("../../../../../omeka-server/tests/import_shapes/bhc006.listing");

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

/// The proposal for the replica after `edits`, with no list read.
pub fn proposal(edits: &[Edit]) -> Proposal {
    let index = index();
    let catalog = install();
    let plan = draft();
    let evidence = Evidence {
        index: &index,
        lists: &[],
        catalog: &catalog,
        sources: &plan.sources,
        structure: &plan.structure,
    };
    propose(&evidence, edits)
}
