//! The `nameWords` and `columnMatches` sections: exact text and exact
//! counts the admin shows while the person marks an example or picks the
//! join column.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use serde_json::{Value, json};

use super::*;
use crate::import::derive::{Index, IndexedSource, NoTags};
use crate::import::evidence::Walk;
use crate::import::plan::{RelPath, v2_default};
use crate::import::sheet;

fn derived(paths: &[&str], list: Option<&str>) -> Derived {
    let mut value = v2_default(&[SourcePlan::Folder {
        id: SourceId("s1".into()),
        root: "archive".into(),
        path: RelPath::parse("").unwrap(),
    }]);
    value["grouping"]["pattern"] = json!(r"^(?<identifier>[A-Z]\d)\.jpg$");
    if let Some(list) = list {
        let headers: Vec<&str> = list.lines().next().unwrap().split(',').collect();
        value["sheets"] = json!([{
            "id": "list", "source": "s1", "file": "list.csv", "hash": null,
            "read": { "encoding": "utf-8", "delimiter": ",", "quote": "\"", "headerLine": 1, "trim": false },
            "rowKind": "item", "join": null, "itemKey": null,
            "columns": headers.iter().enumerate().map(|(index, header)| json!({
                "column": { "index": index, "header": header },
                "options": { "split": null, "type": "literal", "lang": null, "private": false }
            })).collect::<Vec<_>>()
        }]);
    }
    let plan = Arc::new(ImportPlan::from_json(&value.to_string()).unwrap());
    let index = Index {
        sources: vec![IndexedSource {
            id: SourceId("s1".into()),
            walk: Walk {
                entries: paths
                    .iter()
                    .map(|path| FileEntry {
                        path: RelPath::parse(path).unwrap(),
                        size: 1,
                        modified: None,
                        sniffed: "image/jpeg".into(),
                        decoded: None,
                    })
                    .collect(),
                ..Walk::default()
            },
        }],
    };
    let read: BTreeMap<SheetId, Arc<ReadSheet>> = plan
        .structure
        .lists()
        .map(|(_, sheet_plan)| {
            let bytes = list.unwrap().as_bytes();
            let read = ReadSheet {
                sheet: sheet::read(bytes, &sheet_plan.read, sheet::key_column(sheet_plan)),
                guess: sheet::guess(bytes),
                lines: Vec::new(),
            };
            (sheet_plan.id.clone(), Arc::new(read))
        })
        .collect();
    let sheets: Sheets = read
        .iter()
        .map(|(id, read)| (id.clone(), read.sheet.clone()))
        .collect();
    let model = derive(&index, &sheets, &plan, &Catalog::default(), &NoTags);
    Derived {
        plan,
        model,
        catalog: Catalog::default(),
        stored: StoredIndex {
            index: Arc::new(index),
            sources: Vec::new(),
        },
        sheets: read,
        tags: DiskTags {
            caches: Arc::default(),
            dirs: BTreeMap::new(),
        },
    }
}

fn answer(derived: &Derived, request: Value) -> Value {
    let request: PreviewRequest = serde_json::from_value(request).unwrap();
    serde_json::to_value(preview(derived, &request)).unwrap()
}

const NAMES: &[&str] = &[
    "Artifacts/BHC_006_AF_001_front.jpg",
    "Artifacts/BHC_006_AF_001_back.jpg",
    "Artifacts/BHC_006_AF_002_front.jpg",
    "Scans/BHC 006 AF-003.front.png",
    "Photos/BHC_007-PH-1.tif",
    "notes.txt",
];

fn words(place: &Value) -> Vec<(String, u64)> {
    place["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|word| {
            (
                word["text"].as_str().unwrap().to_string(),
                word["count"].as_u64().unwrap(),
            )
        })
        .collect()
}

#[test]
fn name_words_count_each_word_at_each_place_over_names_with_as_many_parts() {
    let derived = derived(NAMES, None);
    let answer = answer(
        &derived,
        json!({ "include": ["nameWords"],
                "nameWords": { "path": "Artifacts/BHC_006_AF_001_front.jpg" } }),
    );
    let found = &answer["nameWords"];
    assert_eq!(
        found["parts"],
        json!(["BHC", "006", "AF", "001", "front", "jpg"])
    );
    assert_eq!(found["names"], 4, "the four names of six parts");
    let places = found["places"].as_array().unwrap();
    assert_eq!(places.len(), 6);
    assert_eq!(words(&places[0]), [("BHC".into(), 4)]);
    assert_eq!(
        words(&places[3]),
        [("001".into(), 2), ("002".into(), 1), ("003".into(), 1)]
    );
    assert_eq!(words(&places[4]), [("front".into(), 3), ("back".into(), 1)]);
    assert_eq!(words(&places[5]), [("jpg".into(), 3), ("png".into(), 1)]);
}

#[test]
fn name_words_are_exact_text_and_paged_per_place() {
    let derived = derived(
        &["A_x.jpg", "a_x.jpg", "A_y.jpg", "B_y.jpg", "C_y.jpg"],
        None,
    );
    let answer = answer(
        &derived,
        json!({ "include": ["nameWords"],
                "nameWords": { "path": "A_x.jpg", "page": 2, "perPage": 1 } }),
    );
    let places = answer["nameWords"]["places"].as_array().unwrap();
    assert_eq!(places[0]["total"], 4, "A, a, B and C: case is not folded");
    assert_eq!(words(&places[0]), [("B".into(), 1)], "A (2), then B, C, a");
    assert_eq!(places[1]["total"], 2);
    assert_eq!(words(&places[1]), [("x".into(), 2)]);
}

#[test]
fn name_words_without_a_path_answer_no_places() {
    let derived = derived(NAMES, None);
    let answer = answer(&derived, json!({ "include": ["nameWords"] }));
    assert_eq!(answer["nameWords"]["places"], json!([]));
    assert_eq!(answer["nameWords"]["names"], 0);
}

#[test]
fn column_matches_count_exact_group_keys_file_names_and_empty_cells() {
    let derived = derived(
        &["A1.jpg", "A2.jpg", "B3.jpg", "Box/C4.jpg"],
        Some("Id,File,Note\nA1,A1.jpg,\nA2,x.jpg,hi\n,B3.jpg,A1\na1,Box/C4.jpg,\n"),
    );
    let answer = answer(&derived, json!({ "include": ["columnMatches"] }));
    assert_eq!(
        answer["columnMatches"],
        json!([{
            "sheet": "list",
            "rows": 4,
            "columns": [
                { "index": 0, "header": "Id", "groupKeys": 2, "fileNames": 0, "empty": 1 },
                { "index": 1, "header": "File", "groupKeys": 0, "fileNames": 3, "empty": 0 },
                { "index": 2, "header": "Note", "groupKeys": 1, "fileNames": 0, "empty": 2 }
            ]
        }])
    );
}
