#![allow(clippy::expect_used, clippy::unwrap_used)]

use serde_json::{Value, json};

use super::*;
use crate::import::archive::SourceDir;
use crate::import::catalog::Catalog;
use crate::import::derive::{Index, IndexedSource, NoTags};
use crate::import::evidence::Walk;
use crate::import::plan::{RelPath, SourcePlan, default_plan};

/// Four items of two views each: `A1a.jpg`, `A1b.jpg` … `A4b.jpg`.
const FILES: [&str; 8] = [
    "A1a.jpg", "A1b.jpg", "A2a.jpg", "A2b.jpg", "A3a.jpg", "A3b.jpg", "A4a.jpg", "A4b.jpg",
];

fn entry(path: &str) -> FileEntry {
    FileEntry {
        path: RelPath::parse(path).unwrap(),
        size: 1,
        modified: None,
        sniffed: "image/jpeg".into(),
        decoded: None,
    }
}

/// The files above, the `lists` written into a folder source, and a plan
/// whose sheets `edit` adds.
fn derived(
    dir: &tempfile::TempDir,
    lists: &[(&str, &[u8])],
    edit: impl FnOnce(&mut Value),
) -> Derived {
    for (path, bytes) in lists {
        let path = dir.path().join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }
    let plan = default_plan(vec![SourcePlan::Folder {
        id: SourceId("s1".into()),
        root: "archive".into(),
        path: RelPath::parse("").unwrap(),
    }])
    .unwrap();
    let mut value = serde_json::to_value(plan).unwrap();
    value["grouping"]["pattern"] = json!(r"^(?<identifier>[A-Z]\d+)[a-z]\.jpg$");
    edit(&mut value);
    let plan = Arc::new(ImportPlanV2::from_json(&value.to_string()).unwrap());
    let index = Index {
        sources: vec![IndexedSource {
            id: SourceId("s1".into()),
            walk: Walk {
                entries: FILES.iter().map(|path| entry(path)).collect(),
                sheets: lists.iter().map(|(path, _)| entry(path)).collect(),
                ..Walk::default()
            },
        }],
    };
    let caches: Arc<Caches> = Arc::default();
    let dirs = BTreeMap::from([(
        SourceId("s1".into()),
        SourceDir::standalone(dir.path()).unwrap(),
    )]);
    let read: BTreeMap<SheetId, Arc<ReadSheet>> = plan
        .sheets
        .iter()
        .filter_map(|sheet| {
            Some((
                sheet.id.clone(),
                caches.sheets.read(sheet, dirs.get(&sheet.source)?)?,
            ))
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
        tags: DiskTags { caches, dirs },
    }
}

fn candidates(derived: &Derived) -> Vec<Value> {
    let request: PreviewRequest =
        serde_json::from_value(json!({ "include": ["listCandidates"] })).unwrap();
    let answer = serde_json::to_value(preview(derived, &request)).unwrap();
    answer["listCandidates"].as_array().unwrap().clone()
}

/// One row per file, every file.
const FULL: &str = "File,View\n\
A1a.jpg,front\nA1b.jpg,back\nA2a.jpg,front\nA2b.jpg,back\n\
A3a.jpg,front\nA3b.jpg,back\nA4a.jpg,front\nA4b.jpg,back\n";
/// An older copy: three files of A1 to A3 by their exact names, and two
/// more of A1 and A2 whose names differ only by case and zero padding.
const STALE: &str = "File\nA1a.jpg\nA2a.jpg\nA3a.jpg\na01B.JPG\na02B.JPG\n";
/// A list about something else.
const OTHER: &str = "Name,Place\nLamp,Pune\nBowl,Gawan\n";

#[test]
fn every_list_is_ranked_by_the_rows_it_pairs_with_its_grain_and_its_rivals() {
    let dir = tempfile::tempdir().unwrap();
    let derived = derived(
        &dir,
        &[
            ("lists/full.csv", FULL.as_bytes()),
            ("lists/other.csv", OTHER.as_bytes()),
            ("lists/stale.csv", STALE.as_bytes()),
        ],
        |plan| {
            plan["sheets"] = json!([{
                "id": "register", "source": "s1", "file": "lists/full.csv", "hash": null,
                "read": { "encoding": "auto", "delimiter": "auto", "quote": "\"",
                          "headerLine": 1, "trim": true },
                "rowKind": "item", "join": null, "itemKey": null, "columns": []
            }]);
        },
    );
    let found = candidates(&derived);
    let files: Vec<&str> = found
        .iter()
        .map(|one| one["file"].as_str().unwrap())
        .collect();
    assert_eq!(
        files,
        ["lists/full.csv", "lists/stale.csv", "lists/other.csv"],
        "most paired first, not by path"
    );
    let [full, stale, other] = &found[..] else {
        unreachable!()
    };

    assert_eq!(full["source"], "s1");
    assert_eq!(full["inPlan"], "register");
    assert_eq!(full["read"], "ok");
    assert_eq!(
        (full["rows"].as_u64(), full["columns"].as_u64()),
        (Some(8), Some(2))
    );
    assert_eq!(
        full["best"],
        json!({ "index": 0, "header": "File", "on": "fileName",
                "paired": 8, "pairedNormalized": 8, "empty": 0 })
    );
    assert_eq!(
        (
            &full["grain"]["groupsWithManyRows"],
            &full["grain"]["rowsNamingOtherFilesOfAGroup"]
        ),
        (&json!(4), &json!(4)),
        "two rows for each of the four items, each naming its other view"
    );
    assert_eq!(
        full["overlaps"],
        json!([{ "source": "s1", "file": "lists/stale.csv", "sharedTargets": 3 }])
    );

    assert_eq!(stale["inPlan"], Value::Null);
    assert_eq!(
        stale["best"],
        json!({ "index": 0, "header": "File", "on": "fileName",
                "paired": 3, "pairedNormalized": 5, "empty": 0 }),
        "case and zero padding pair two more rows when folded"
    );
    assert_eq!(
        (
            &stale["grain"]["groupsWithManyRows"],
            &stale["grain"]["rowsNamingOtherFilesOfAGroup"]
        ),
        (&json!(0), &json!(0))
    );
    assert_eq!(
        stale["overlaps"],
        json!([{ "source": "s1", "file": "lists/full.csv", "sharedTargets": 3 }])
    );

    assert_eq!(other["read"], "ok");
    assert_eq!(other["rows"], 2);
    assert_eq!(other["best"], Value::Null);
    assert_eq!(other["grain"], Value::Null);
    assert_eq!(other["overlaps"], json!([]));

    let simple_adds = derived.plan.sheets[0].clone();
    let mut stale_added = simple_adds;
    stale_added.file = RelPath::parse("lists/stale.csv").unwrap();
    let warm = derived
        .tags
        .caches
        .sheets
        .read(&stale_added, &derived.tags.dirs[&SourceId("s1".into())])
        .unwrap();
    let again = derived
        .tags
        .caches
        .sheets
        .read(&stale_added, &derived.tags.dirs[&SourceId("s1".into())])
        .unwrap();
    assert!(Arc::ptr_eq(&warm, &again));
    assert_eq!(warm.sheet.as_ref().unwrap().rows.len(), 5);
}

#[test]
fn a_list_that_does_not_read_says_why_and_the_others_still_come_back() {
    let dir = tempfile::tempdir().unwrap();
    let derived = derived(
        &dir,
        &[
            ("broken.xlsx", b"PK\x03\x04 not a book"),
            ("full.csv", FULL.as_bytes()),
        ],
        |_| {},
    );
    let found = candidates(&derived);
    assert_eq!(found.len(), 2);
    assert_eq!(found[0]["file"], "full.csv");
    assert_eq!(found[0]["read"], "ok");
    assert_eq!(found[0]["best"]["paired"], 8);
    let broken = &found[1];
    assert_eq!(broken["file"], "broken.xlsx");
    let error = broken["read"]["error"].as_str().unwrap();
    assert!(!error.is_empty());
    assert_eq!(
        (broken["rows"].as_u64(), broken["best"].is_null()),
        (Some(0), true)
    );
}

/// One row per file. A1 to A3 have two rows each and A4 one. `Year` is the
/// same in both rows of every item; `Keywords` differs in A1, and in A3 only
/// by order and spaces.
const PER_FILE: &str = "File,Year,Keywords\n\
A1a.jpg,2017,Front View\nA1b.jpg,2017,Back View\n\
A2a.jpg,2018,Lamp\nA2b.jpg,2018,Lamp\n\
A3a.jpg,2017,\"Bowl, Front\"\nA3b.jpg, 2017 ,\"Front ,Bowl\"\n\
A4a.jpg,2024,Pot\n";

fn varies(candidate: &Value) -> Vec<(String, Value)> {
    candidate["grain"]["columns"]
        .as_array()
        .unwrap()
        .iter()
        .map(|column| {
            (
                column["header"].as_str().unwrap().to_string(),
                column["varies"].clone(),
            )
        })
        .collect()
}

#[test]
fn grain_counts_in_how_many_items_each_column_varies_among_the_items_several_rows_name() {
    let dir = tempfile::tempdir().unwrap();
    let derived = derived(
        &dir,
        &[
            ("in-use.csv", PER_FILE.as_bytes()),
            ("unused.csv", PER_FILE.as_bytes()),
        ],
        |plan| {
            plan["sheets"] = json!([{
                "id": "register", "source": "s1", "file": "in-use.csv", "hash": null,
                "read": { "encoding": "auto", "delimiter": "auto", "quote": "\"",
                          "headerLine": 1, "trim": true },
                "rowKind": "media",
                "join": { "on": "fileName", "column": { "index": 0, "header": "File" },
                          "normalize": { "caseFold": false, "leadingZeros": false, "separators": false } },
                "itemKey": null,
                "columns": [{ "column": { "index": 2, "header": "Keywords" },
                              "options": { "split": ",", "type": "literal", "lang": null, "private": false } }]
            }]);
        },
    );
    let found = candidates(&derived);
    let in_use = found
        .iter()
        .find(|one| one["file"] == "in-use.csv")
        .unwrap();
    let unused = found
        .iter()
        .find(|one| one["file"] == "unused.csv")
        .unwrap();
    let of = |groups: u64| json!({ "groups": groups, "of": 3 });

    assert_eq!(in_use["grain"]["groupsWithManyRows"], 3);
    assert_eq!(
        varies(in_use),
        [
            ("File".to_string(), of(3)),
            ("Year".to_string(), of(0)),
            ("Keywords".to_string(), of(1)),
        ],
        "each row names its own file; the year is the item's; the keywords \
         differ in A1 only, as the plan splits them at commas"
    );
    assert_eq!(
        varies(unused)[2],
        ("Keywords".to_string(), of(2)),
        "a list no plan sheet reads has no split, so A3's cells differ as written"
    );
}
