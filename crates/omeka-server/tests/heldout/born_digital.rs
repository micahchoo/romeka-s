//! Frozen held-out shape, written before the importer was read: never edit it or `expected()`.
//!
//! A born-digital dump of a personal drive with no catalogue.
//!
//! The drive is taken as found: no item sets are invented, each item is one
//! original file named by its path without the extension, and a camera's
//! RAW and JPEG of one frame are one item. Junk the operating system wrote
//! and editing sidecars make no item.

use serde_json::{Value, json};

use super::{Item, Outcome, Shape, assert_same, kind, outcome_of, run};
use crate::imports_fixture::fixture;

/// A file's name and its leading bytes.
type File = (&'static str, &'static [u8]);

/// The files that make items, by the item they make, in media order.
const ITEMS: [(&str, &[File]); 10] = [
    (
        "Pictures/Camera Uploads/IMG_0001",
        &[("IMG_0001.JPG", kind::JPEG)],
    ),
    (
        "Pictures/Camera Uploads/IMG_0002",
        &[("IMG_0002.JPG", kind::JPEG)],
    ),
    // Camera numbering restarts: this frame shares its name with the one above.
    (
        "Pictures/Old Phone/IMG_0001",
        &[("IMG_0001.JPG", kind::JPEG)],
    ),
    (
        "Pictures/Nikon/2018-07 Hampi/DSC_0002",
        &[("DSC_0002.JPG", kind::JPEG), ("DSC_0002.NEF", kind::TIFF)],
    ),
    (
        "Pictures/Nikon/2018-07 Hampi/DSC_0003",
        &[("DSC_0003.NEF", kind::TIFF)],
    ),
    (
        "Desktop/Screen Shot 2019-11-02 at 09.01.10",
        &[("Screen Shot 2019-11-02 at 09.01.10.png", kind::PNG)],
    ),
    (
        "Desktop/Screenshot 2021-03-04 at 10.15.22",
        &[("Screenshot 2021-03-04 at 10.15.22.png", kind::PNG)],
    ),
    (
        "Documents/Letters/letter to council",
        &[("letter to council.docx", kind::ZIP)],
    ),
    (
        "Documents/Letters/reply from council",
        &[("reply from council.pdf", kind::PDF)],
    ),
    (
        "Documents/notes",
        &[("notes.txt", b"shopping: rice, dal\n")],
    ),
];

/// Files that make no item: operating-system junk, a Word lock file and
/// editing sidecars.
const LEFT_OUT: [File; 8] = [
    (".DS_Store", b"\x00\x00\x00\x01Bud1"),
    ("desktop.ini", b"[.ShellClassInfo]\r\n"),
    ("Pictures/Camera Uploads/.DS_Store", b"\x00\x00\x00\x01Bud1"),
    (
        "Pictures/Camera Uploads/IMG_0002.AAE",
        b"<?xml version=\"1.0\"?><plist/>",
    ),
    (
        "Pictures/Nikon/2018-07 Hampi/DSC_0002.xmp",
        b"<x:xmpmeta xmlns:x=\"adobe:ns:meta/\"/>",
    ),
    (
        "Pictures/Nikon/2018-07 Hampi/Thumbs.db",
        b"\xD0\xCF\x11\xE0\xA1\xB1\x1A\xE1",
    ),
    ("Documents/Letters/~$tter to council.docx", b"\x05owner"),
    ("Documents/.DS_Store", b"\x00\x00\x00\x01Bud1"),
];

fn folder_of(item: &str) -> &str {
    item.rsplit_once('/').map_or("", |(folder, _)| folder)
}

pub fn shape() -> Shape {
    let mut shape = Shape::default();
    for (item, files) in ITEMS {
        for (name, header) in files {
            shape.object(&format!("{}/{name}", folder_of(item)), header);
        }
    }
    for (path, bytes) in LEFT_OUT {
        shape.object(path, bytes);
    }
    shape
}

/// The title is the file's name without its folder or extension.
pub fn expected() -> Outcome {
    let mut outcome = Outcome::default();
    for (item, files) in ITEMS {
        let title = item.rsplit_once('/').map_or(item, |(_, name)| name);
        let media: Vec<String> = files
            .iter()
            .map(|(name, _)| format!("{}/{name}", folder_of(item)))
            .collect();
        outcome
            .items
            .insert(item.to_string(), Item::new(title, &media, &[]));
    }
    outcome
        .left_out
        .extend(LEFT_OUT.iter().map(|(path, _)| path.to_string()));
    outcome
}

/// One item per path without its extension, titled by the name part of it,
/// with no item sets. A name that starts with `~` is an Office lock file.
pub fn plan_for_born_digital(plan: &mut Value) {
    plan["grouping"]["pattern"] =
        json!(r"^(?<identifier>(?:[^/]*/)*(?<title>[^/~][^/]*?))(?:\.[^./]*)?$");
    plan["slots"]["item"]["dcterms:title"] =
        json!({ "sources": [{ "from": "group", "name": "title" }], "combine": "first" });
}

#[tokio::test]
async fn born_digital_dump_is_taken_as_found() {
    let fixture = fixture().await;
    let shape = shape();
    let preview = run(&fixture, &shape, &[], plan_for_born_digital).await;
    assert_same(&outcome_of(&preview, &shape, &[]), &expected());
}
