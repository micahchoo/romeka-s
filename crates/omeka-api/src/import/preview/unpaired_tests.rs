//! The `unpaired` section when list rows describe items and when they
//! describe files, with files the install refuses.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::collections::BTreeMap;
use std::sync::Arc;

use omeka_media::validation::FileValidation;
use serde_json::{Value, json};

use super::super::{Derived, DiskTags, PreviewRequest, preview};
use crate::import::catalog::Catalog;
use crate::import::derive::{Index, IndexedSource, NoTags, Sheets, derive};
use crate::import::evidence::{FileEntry, Walk};
use crate::import::plan::{ImportPlan, RelPath, SourceId, SourcePlan, default_plan};
use crate::import::scan::StoredIndex;
use crate::import::sheet;

const JPEG: &str = "image/jpeg";
const RAW: &str = "application/octet-stream";

/// `X_010b.jpg` is view `b` of item `X_010`.
fn derived(files: &[&str], rows: &str, row_kind: &str) -> Derived {
    let plan = default_plan(vec![SourcePlan::Folder {
        id: SourceId("s1".into()),
        root: "archive".into(),
        path: RelPath::parse("").unwrap(),
    }])
    .unwrap();
    let mut value = serde_json::to_value(plan).unwrap();
    value["grouping"]["pattern"] = json!(r"^(?<identifier>X_\d{3})(?<part>[a-z])\.[^.]+$");
    value["sheets"] = json!([{
        "id": "views", "source": "s1", "file": "views.csv", "hash": null,
        "read": { "encoding": "utf-8", "delimiter": ",", "quote": "\"", "headerLine": 1, "trim": true },
        "rowKind": row_kind,
        "join": { "on": "fileName", "column": { "index": 0, "header": "File" },
                  "normalize": { "caseFold": false, "leadingZeros": false, "separators": false } },
        "itemKey": null,
        "columns": [{ "column": { "index": 0, "header": "File" },
                      "options": { "split": null, "type": "literal", "lang": null, "private": false } }]
    }]);
    let plan = Arc::new(ImportPlan::from_json(&value.to_string()).unwrap());
    let index = Index {
        sources: vec![IndexedSource {
            id: SourceId("s1".into()),
            walk: Walk {
                entries: files
                    .iter()
                    .map(|path| FileEntry {
                        path: RelPath::parse(path).unwrap(),
                        size: 10,
                        modified: None,
                        sniffed: if path.ends_with(".NEF") { RAW } else { JPEG }.into(),
                        decoded: None,
                    })
                    .collect(),
                ..Walk::default()
            },
        }],
    };
    let sheets: Sheets = plan
        .sheets
        .iter()
        .map(|sheet_plan| {
            let read = sheet::read(
                format!("File\n{rows}").as_bytes(),
                &sheet_plan.read,
                sheet::key_column(sheet_plan),
            );
            (sheet_plan.id.clone(), read)
        })
        .collect();
    let catalog = Catalog {
        files: FileValidation::Allow {
            media_types: [JPEG.to_string()].into(),
            extensions: ["jpg".to_string()].into(),
        },
        ..Catalog::default()
    };
    let model = derive(&index, &sheets, &plan, &catalog, &NoTags);
    Derived {
        plan,
        model,
        catalog,
        stored: StoredIndex {
            index: Arc::new(index),
            sources: Vec::new(),
        },
        sheets: BTreeMap::new(),
        tags: DiskTags {
            caches: Arc::default(),
            dirs: BTreeMap::new(),
        },
    }
}

fn unpaired(derived: &Derived) -> Value {
    let request: PreviewRequest =
        serde_json::from_value(json!({ "include": ["unpaired"] })).unwrap();
    serde_json::to_value(preview(derived, &request)).unwrap()["unpaired"].clone()
}

fn keys(rows: &Value) -> Vec<&str> {
    rows["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["row"]["key"].as_str().unwrap())
        .collect()
}

fn paths(files: &Value) -> Vec<&str> {
    files["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|file| file["file"]["path"].as_str().unwrap())
        .collect()
}

const FILES: &[&str] = &[
    "X_010a.jpg",
    "X_010b.jpg",
    "X_020a.jpg",
    "X_020b.jpg",
    "X_030a.jpg",
    "X_040a.NEF",
];
const ROWS: &str = "X_010a.jpg\nX_010b.jpg\nX_030a.jpg\nX_040a.NEF\nX_099a.jpg\n";

#[test]
fn an_item_has_a_row_when_a_row_describes_one_of_its_files() {
    let found = unpaired(&derived(FILES, ROWS, "media"));
    assert_eq!(
        paths(&found["files"]),
        ["X_020a.jpg", "X_020b.jpg"],
        "X_010 and X_030 have rows through their files"
    );
    assert_eq!(found["items"], 1, "two files, one item");
}

#[test]
fn rows_that_name_no_file_of_the_import_are_unpaired_and_a_refused_file_is_not_one() {
    for row_kind in ["item", "media"] {
        let found = unpaired(&derived(FILES, ROWS, row_kind));
        assert_eq!(
            keys(&found["rows"]),
            ["X_099a.jpg#1"],
            "{row_kind}: the NEF row names a file this install refuses, not no file"
        );
        assert_eq!(found["rows"]["total"], 1, "{row_kind}");
    }
}

#[test]
fn rows_that_describe_items_count_items_without_a_row_once() {
    let found = unpaired(&derived(FILES, "X_010a.jpg\n", "item"));
    assert_eq!(found["files"]["total"], 3, "X_020 a and b, X_030a");
    assert_eq!(found["items"], 2);
}
