//! The answers the admin needs at scale: shapes, histogram, cause samples,
//! identical files, unpaired lists, values with counts and the filters.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use serde_json::{Value, json};

use super::*;
use crate::import::derive::{Index, IndexedSource, NoTags};
use crate::import::evidence::{Anomalies, Identical, Walk};
use crate::import::plan::{RelPath, default_plan};
use crate::import::sheet;

const JPEG: &str = "image/jpeg";
const PDF: &str = "application/pdf";
const PATTERN: &str = r"^(?:[^/]+/)*(?<identifier>[A-Z]+\d+)(?:_(?<part>[a-z0-9]+))?\.[a-z]+$";

fn derived(files: &[(&str, &str)], list: Option<&str>, edit: impl FnOnce(&mut Value)) -> Derived {
    let plan = default_plan(vec![SourcePlan::Folder {
        id: SourceId("s1".into()),
        root: "archive".into(),
        path: RelPath::parse("").unwrap(),
    }])
    .unwrap();
    let mut value = serde_json::to_value(plan).unwrap();
    value["grouping"]["pattern"] = json!(PATTERN);
    if let Some(list) = list {
        let headers: Vec<&str> = list.lines().next().unwrap().split(',').collect();
        value["sheets"] = json!([{
            "id": "list", "source": "s1", "file": "list.csv", "hash": null,
            "read": { "encoding": "utf-8", "delimiter": ",", "quote": "\"", "headerLine": 1, "trim": true },
            "rowKind": "item",
            "join": { "on": "identifier", "column": { "index": 0, "header": headers[0] },
                      "normalize": { "caseFold": false, "leadingZeros": false, "separators": false } },
            "itemKey": null,
            "columns": headers.iter().enumerate().map(|(index, header)| json!({
                "column": { "index": index, "header": header },
                "options": { "split": null, "type": "literal", "lang": null, "private": false }
            })).collect::<Vec<_>>()
        }]);
    }
    edit(&mut value);
    let plan = Arc::new(ImportPlanV2::from_json(&value.to_string()).unwrap());
    let index = Index {
        sources: vec![IndexedSource {
            id: SourceId("s1".into()),
            walk: Walk {
                entries: files
                    .iter()
                    .map(|(path, sniffed)| FileEntry {
                        path: RelPath::parse(path).unwrap(),
                        size: 10,
                        modified: None,
                        sniffed: (*sniffed).into(),
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
            let bytes = list.unwrap().as_bytes();
            (
                sheet_plan.id.clone(),
                sheet::read(bytes, &sheet_plan.read, sheet::key_column(sheet_plan)),
            )
        })
        .collect();
    let catalog = Catalog::default();
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

fn answer(derived: &Derived, request: Value) -> Value {
    let request: PreviewRequest = serde_json::from_value(request).unwrap();
    serde_json::to_value(preview(derived, &request)).unwrap()
}

/// Four artifacts with a front and a back, one with only a back, two
/// single photos, a PDF scan of three pages, and a list row with no file.
fn archive() -> Derived {
    let mut files: Vec<(String, &str)> = Vec::new();
    for n in 1..=4 {
        files.push((format!("Artifacts/AF{n}_front.jpg"), JPEG));
        files.push((format!("Artifacts/AF{n}_back.jpg"), JPEG));
    }
    files.push(("Artifacts/AF5_back.jpg".into(), JPEG));
    files.push(("Photos/PH1.jpg".into(), JPEG));
    files.push(("Photos/PH2.jpg".into(), JPEG));
    for page in ["p1", "p2", "p3"] {
        files.push((format!("Scans/SC1_{page}.pdf"), PDF));
    }
    let listed: Vec<(&str, &str)> = files.iter().map(|(p, t)| (p.as_str(), *t)).collect();
    derived(
        &listed,
        Some("Id,Title\nAF1,Lamp\nPH1,Wedding\nLOST9,Missing object\n"),
        |_| {},
    )
}

fn groups(answer: &Value) -> &Vec<Value> {
    answer["shapes"]["groups"]["items"].as_array().unwrap()
}

#[test]
fn shapes_group_items_by_exact_facts_with_counts_and_samples() {
    let derived = archive();
    let found = answer(&derived, json!({ "include": ["shapes"], "seed": 1 }));
    assert_eq!(found["shapes"]["records"], 9, "{found}");
    let groups = groups(&found);
    let facts: Vec<(&Value, u64)> = groups
        .iter()
        .map(|group| (&group["facts"], group["count"].as_u64().unwrap()))
        .collect();
    let two_sides_no_row = json!({ "files": "2", "folders": ["Artifacts"], "media": ["image"],
        "pattern": true, "row": false, "parts": ["back", "front"], "pages": null });
    assert_eq!(facts[0], (&two_sides_no_row, 3), "largest first: {facts:?}");
    let expected = [
        json!({ "files": "0", "folders": [], "media": [], "pattern": false, "row": true,
                "parts": [], "pages": null }),
        json!({ "files": "1", "folders": ["Artifacts"], "media": ["image"], "pattern": true,
                "row": false, "parts": ["back"], "pages": null }),
        json!({ "files": "1", "folders": ["Photos"], "media": ["image"], "pattern": true,
                "row": false, "parts": [], "pages": null }),
        json!({ "files": "1", "folders": ["Photos"], "media": ["image"], "pattern": true,
                "row": true, "parts": [], "pages": null }),
        json!({ "files": "2", "folders": ["Artifacts"], "media": ["image"], "pattern": true,
                "row": true, "parts": ["back", "front"], "pages": null }),
        json!({ "files": "3", "folders": ["Scans"], "media": ["pdf"], "pattern": true,
                "row": false, "parts": ["p1", "p2", "p3"], "pages": null }),
    ];
    for shape in &expected {
        assert!(
            facts
                .iter()
                .any(|(facts, count)| *facts == shape && *count == 1),
            "{shape} in {facts:?}"
        );
    }
    assert_eq!(groups.len(), 1 + expected.len());
    assert_eq!(groups[0]["samples"].as_array().unwrap().len(), 3);
    assert_eq!(groups[0]["odd"], false, "3 of 9 is no outlier");
}

#[test]
fn a_group_under_two_percent_or_of_seven_files_is_odd_and_the_seed_draws_samples() {
    let mut files: Vec<String> = (1..=60).map(|n| format!("PH{n}.jpg")).collect();
    files.extend(["TW1_a.jpg", "TW1_b.jpg", "TW2_a.jpg", "TW2_b.jpg"].map(String::from));
    files.push("BIG1_a.jpg".into());
    files.extend((1..=7).map(|n| format!("BK1_p{n}.jpg")));
    let listed: Vec<(&str, &str)> = files.iter().map(|path| (path.as_str(), JPEG)).collect();
    let derived = derived(&listed, None, |_| {});
    let found = answer(&derived, json!({ "include": ["shapes"], "seed": 3 }));
    let all = groups(&found);
    assert_eq!(all.len(), 4, "{found}");
    let count_odd = |group: &Value| (group["count"].as_u64(), group["odd"].as_bool());
    assert_eq!(count_odd(&all[0]), (Some(60), Some(false)));
    assert_eq!(
        count_odd(&all[1]),
        (Some(2), Some(false)),
        "2 of 64 is over 2%"
    );
    for odd in &all[2..] {
        assert_eq!(odd["odd"], true, "one of 64, or 7 files: {odd}");
    }
    let sevens = all
        .iter()
        .find(|group| group["facts"]["files"] == "7+")
        .unwrap();
    assert_eq!(sevens["count"], 1);

    let samples = |seed: u64| -> Vec<Value> {
        let found = answer(&derived, json!({ "include": ["shapes"], "seed": seed }));
        groups(&found)[0]["samples"]
            .as_array()
            .unwrap()
            .iter()
            .map(|record| record["id"].clone())
            .collect()
    };
    assert_eq!(samples(3), samples(3), "one seed, one draw");
    assert!(
        (4..10).any(|seed| samples(seed) != samples(3)),
        "other seeds draw others"
    );
}

#[test]
fn the_pattern_page_group_is_a_fact_when_the_person_marked_it() {
    let derived = derived(
        &[
            ("BK1_1.jpg", JPEG),
            ("BK1_2.jpg", JPEG),
            ("BK2_1.jpg", JPEG),
        ],
        None,
        |plan| {
            plan["grouping"]["pattern"] = json!(r"^(?<identifier>[A-Z]+\d+)_(?<page>\d+)\.jpg$");
        },
    );
    let found = answer(&derived, json!({ "include": ["shapes"] }));
    let pages: Vec<&Value> = groups(&found)
        .iter()
        .map(|g| &g["facts"]["pages"])
        .collect();
    assert!(
        pages.contains(&&json!(["1", "2"])) && pages.contains(&&json!(["1"])),
        "{pages:?}"
    );
    assert!(
        groups(&found)
            .iter()
            .all(|group| group["facts"]["parts"].is_null())
    );
}

#[test]
fn the_histogram_counts_items_per_file_bucket() {
    let derived = archive();
    let found = answer(&derived, json!({ "include": ["histogram"] }));
    assert_eq!(
        found["histogram"],
        json!([
            { "files": "0", "count": 1 }, { "files": "1", "count": 3 },
            { "files": "2", "count": 4 }, { "files": "3", "count": 1 },
            { "files": "4-6", "count": 0 }, { "files": "7+", "count": 0 }
        ])
    );
    assert!(found.get("shapes").is_none(), "only the sections asked for");
}

#[test]
fn records_filter_by_shape_folder_bucket_and_left_out() {
    let derived = archive();
    let shapes = answer(&derived, json!({ "include": ["shapes"] }));
    let largest = &groups(&shapes)[0];
    let found = answer(
        &derived,
        json!({ "include": ["records"], "records": { "shape": largest["facts"] } }),
    );
    assert_eq!(found["records"]["total"], largest["count"]);
    let by = |records: Value| {
        answer(
            &derived,
            json!({ "include": ["records"], "records": records }),
        )["records"]["total"]
            .as_u64()
            .unwrap()
    };
    assert_eq!(by(json!({ "folder": "Photos" })), 2);
    assert_eq!(by(json!({ "folder": "Artifacts/" })), 5);
    assert_eq!(
        by(json!({ "folder": "Art" })),
        0,
        "a folder, not a prefix of a name"
    );
    assert_eq!(by(json!({ "files": "2" })), 4);
    assert_eq!(by(json!({ "files": "0" })), 1);
    assert_eq!(by(json!({ "left": true })), 0);
    assert_eq!(by(json!({ "left": false })), 9);
    assert_eq!(by(json!({ "template": "none" })), 9);
    assert_eq!(by(json!({ "set": { "new": 0 } })), 0);
}

#[test]
fn records_filter_by_set_and_item_type() {
    let derived = derived(
        &[("A1.jpg", JPEG), ("A2.jpg", JPEG), ("B1.jpg", JPEG)],
        Some("Id,Set,Type\nA1,Letters,Letter\nA2,Letters,\nB1,Photos,\n"),
        |plan| {
            plan["slots"]["item"]["sets"] = json!({ "sources": [
                { "from": "column", "sheet": "list", "column": { "index": 1, "header": "Set" } }
            ], "combine": "first" });
            plan["slots"]["item"]["template"] = json!({ "sources": [
                { "from": "column", "sheet": "list", "column": { "index": 2, "header": "Type" } }
            ], "combine": "first" });
            plan["newTypes"] = json!([{ "label": "Letter", "from": "mapping", "class": null }]);
        },
    );
    let by = |records: Value| {
        answer(
            &derived,
            json!({ "include": ["records"], "records": records }),
        )["records"]["total"]
            .as_u64()
            .unwrap()
    };
    assert_eq!(by(json!({ "set": { "new": 0 } })), 2, "Letters");
    assert_eq!(by(json!({ "set": { "new": 1 } })), 1, "Photos");
    assert_eq!(by(json!({ "template": { "new": 0 } })), 1);
    assert_eq!(by(json!({ "template": "none" })), 2);
}

#[test]
fn flag_counts_carry_three_seeded_samples_with_their_records() {
    const EXE: &str = "application/x-dosexec";
    let derived = derived(
        &[
            ("A1.exe", EXE),
            ("A2.exe", EXE),
            ("A3.exe", EXE),
            ("A4.exe", EXE),
            ("A5.jpg", JPEG),
        ],
        None,
        |plan| {
            plan["flags"] = json!({ "not-media": "keep" });
        },
    );
    let found = answer(&derived, json!({ "include": ["flags"], "seed": 5 }));
    let counts = found["flags"].as_array().unwrap();
    let not_media = counts
        .iter()
        .find(|count| count["kind"] == "not-media")
        .expect("{found}");
    assert_eq!(not_media["count"], 4, "{not_media}");
    let samples = not_media["samples"].as_array().unwrap();
    assert_eq!(samples.len(), 3);
    for sample in samples {
        assert!(
            sample["subject"]["entry"]["file"]["path"].is_string(),
            "{sample}"
        );
        assert!(
            sample["record"].is_u64(),
            "the media record the file lands in: {sample}"
        );
    }
}

#[test]
fn values_count_records_per_value_after_trim_nfc_and_lowercase_only() {
    let derived = derived(
        &[
            ("A1.jpg", JPEG),
            ("A2.jpg", JPEG),
            ("A3.jpg", JPEG),
            ("A4.jpg", JPEG),
            ("A5.jpg", JPEG),
        ],
        Some("Id,Set\nA1,Letter\nA2,letter \nA3,Letters\nA4,पत्र\nA5,Letter\n"),
        |plan| {
            plan["slots"]["item"]["sets"] = json!({ "sources": [
                { "from": "column", "sheet": "list", "column": { "index": 1, "header": "Set" } }
            ], "combine": "first" });
        },
    );
    let found = answer(&derived, json!({ "include": ["values"] }));
    let slots = found["values"].as_array().unwrap();
    assert_eq!(
        slots
            .iter()
            .map(|slot| slot["slot"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["sets", "template", "class"]
    );
    let sets = slots[0]["values"]["items"].as_array().unwrap();
    let counted: Vec<(&str, u64)> = sets
        .iter()
        .map(|value| {
            (
                value["text"].as_str().unwrap(),
                value["count"].as_u64().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        counted,
        [("Letter", 3), ("Letters", 1), ("पत्र", 1)],
        "{sets:?}"
    );
    assert_eq!(sets[0]["spellings"], json!(["Letter", "letter"]));
    assert_eq!(slots[1]["values"]["total"], 0);
}

#[test]
fn unpaired_lists_rows_with_no_file_and_files_with_no_row_by_name() {
    let derived = derived(
        &[
            ("b/ZED1.jpg", JPEG),
            ("a/AB2.jpg", JPEG),
            ("AB1.jpg", JPEG),
            ("AB1_back.jpg", JPEG),
        ],
        Some("Id,Title\nAB1,Lamp\nQQ9,Lost lamp\nCC3,Cup\n"),
        |plan| {
            plan["slots"]["item"]["dcterms:title"] = json!({ "sources": [
                { "from": "column", "sheet": "list", "column": { "index": 1, "header": "Title" } }
            ], "combine": "first" });
        },
    );
    let found = answer(&derived, json!({ "include": ["unpaired"] }));
    let rows: Vec<(&str, &str)> = found["unpaired"]["rows"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            (
                row["row"]["key"].as_str().unwrap(),
                row["title"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(rows, [("CC3#1", "Cup"), ("QQ9#1", "Lost lamp")]);
    let files: Vec<&str> = found["unpaired"]["files"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|file| file["file"]["path"].as_str().unwrap())
        .collect();
    assert_eq!(
        files,
        ["a/AB2.jpg", "b/ZED1.jpg"],
        "by name; AB1 has its row"
    );

    let searched = answer(
        &derived,
        json!({ "include": ["unpaired"], "unpaired": {
            "rows": { "text": "lamp" }, "files": { "perPage": 1, "page": 2 } } }),
    );
    assert_eq!(searched["unpaired"]["rows"]["total"], 1);
    assert_eq!(searched["unpaired"]["files"]["total"], 2);
    assert_eq!(
        searched["unpaired"]["files"]["items"][0]["file"]["path"],
        "b/ZED1.jpg"
    );
}

#[test]
fn entries_filter_by_use_reason_folder_and_pairing() {
    let derived = derived(
        &[
            ("a/AB1.jpg", JPEG),
            ("a/notes.jpg", JPEG),
            ("b/AB2.jpg", JPEG),
        ],
        Some("Id,Title\nAB1,Lamp\nQQ9,Lost\n"),
        |_| {},
    );
    let total = |entries: Value| {
        answer(
            &derived,
            json!({ "include": ["entries"], "entries": entries }),
        )["entries"]["total"]
            .as_u64()
            .unwrap()
    };
    assert_eq!(total(json!({ "used": false })), 1, "notes.jpg");
    assert_eq!(total(json!({ "reason": "unmatched" })), 1);
    assert_eq!(total(json!({ "used": true })), 4, "two files and two rows");
    assert_eq!(total(json!({ "folder": "a" })), 2);
    assert_eq!(total(json!({ "paired": true })), 1, "AB1's row");
    assert_eq!(total(json!({ "paired": false })), 1, "QQ9's row");
}

#[test]
fn identical_files_answer_the_sets_the_scan_stored() {
    let mut derived = derived(&[("a.jpg", JPEG), ("copy/a.jpg", JPEG)], None, |_| {});
    let mut index = (*derived.stored.index).clone();
    index.sources[0].walk.anomalies = Anomalies {
        identical: vec![Identical {
            size: 10,
            sha256: "0".repeat(64),
            paths: vec!["a.jpg".into(), "copy/a.jpg".into()],
        }],
        ..Anomalies::default()
    };
    derived.stored.index = Arc::new(index);
    let found = answer(&derived, json!({ "include": ["identical"] }));
    assert_eq!(found["identical"]["files"], 2);
    assert_eq!(found["identical"]["sets"]["total"], 1);
    assert_eq!(
        found["identical"]["sets"]["items"][0],
        json!({ "source": "s1", "size": 10, "sha256": "0".repeat(64),
                "paths": ["a.jpg", "copy/a.jpg"] })
    );
}
