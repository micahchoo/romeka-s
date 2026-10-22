#![allow(clippy::expect_used, clippy::unwrap_used)]

use serde_json::json;

use super::*;
use crate::import::catalog::{Catalog, ExistingSet};
use crate::import::derive::{Index, IndexedSource, NoTags};
use crate::import::evidence::Walk;
use crate::import::plan::{RelPath, SourcePlan, v2_default};

fn derived(paths: &[&str], edit: impl FnOnce(&mut serde_json::Value)) -> Derived {
    let mut value = v2_default(&[SourcePlan::Folder {
        id: SourceId("s1".into()),
        root: "archive".into(),
        path: RelPath::parse("").unwrap(),
    }]);
    value["grouping"]["pattern"] = json!(r"^(?<identifier>[A-Z]\d+)\.jpg$");
    edit(&mut value);
    derived_of(&value, paths, Catalog::default())
}

/// `plan` derived over `paths` of source `s1` against `catalog`.
fn derived_of(plan: &serde_json::Value, paths: &[&str], catalog: Catalog) -> Derived {
    let plan = Arc::new(ImportPlan::from_json(&plan.to_string()).unwrap());
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
    let model = derive(&index, &Sheets::new(), &plan, &catalog, &NoTags);
    let stored = StoredIndex {
        index: Arc::new(index),
        sources: Vec::new(),
    };
    Derived {
        plan,
        model,
        catalog,
        stored,
        sheets: BTreeMap::new(),
        tags: DiskTags {
            caches: Arc::default(),
            dirs: BTreeMap::new(),
        },
    }
}

fn request(value: serde_json::Value) -> PreviewRequest {
    serde_json::from_value(value).unwrap()
}

#[test]
fn only_the_sections_asked_for_are_answered() {
    let derived = derived(&["A1.jpg"], |_| {});
    let answer =
        serde_json::to_value(preview(&derived, &request(json!({ "include": ["check"] })))).unwrap();
    assert!(answer["check"]["canRun"].as_bool().unwrap());
    assert!(answer.get("records").is_none());
    assert!(answer.get("entries").is_none());
    assert!(answer["pattern"]["groups"].is_array());
}

#[test]
fn records_page_by_kind_and_filter_by_text_and_flag() {
    let paths: Vec<String> = (1..=7).map(|n| format!("A{n}.jpg")).collect();
    let mut listed: Vec<&str> = paths.iter().map(String::as_str).collect();
    listed.push("notes.jpg");
    let derived = derived(&listed, |_| {});
    let page = preview(
        &derived,
        &request(json!({
            "include": ["records"],
            "records": { "kind": "item", "page": 2, "perPage": 3 }
        })),
    )
    .records
    .unwrap();
    assert_eq!((page.total, page.page, page.per_page), (7, 2, 3));
    assert_eq!(page.items.len(), 3);

    let found = preview(
        &derived,
        &request(json!({ "include": ["records"], "records": { "text": "a7" } })),
    )
    .records
    .unwrap();
    assert_eq!(found.total, 1, "the identifier holds the text");

    let flagged = preview(
        &derived,
        &request(json!({ "include": ["entries"], "entries": { "text": "notes" } })),
    )
    .entries
    .unwrap();
    assert_eq!(flagged.total, 1);
    assert!(flagged.items[0].state.unmatched);
}

#[test]
fn entries_are_filtered_to_files_or_to_rows_by_kind() {
    let derived = derived(&["A1.jpg", "A2.jpg"], |_| {});
    let total = |entries: serde_json::Value| {
        preview(
            &derived,
            &request(json!({ "include": ["entries"], "entries": entries })),
        )
        .entries
        .unwrap()
        .total
    };
    assert_eq!(total(json!({})), 2);
    assert_eq!(total(json!({ "kind": "file" })), 2);
    assert_eq!(total(json!({ "kind": "row" })), 0, "no sheet, so no row");
    assert_eq!(
        total(json!({ "kind": "file", "folder": "" })),
        2,
        "the folder filter keeps answering files only"
    );
    assert_eq!(
        total(json!({ "sheet": "nowhere" })),
        0,
        "a sheet filter answers that sheet's rows only"
    );
    assert!(
        serde_json::from_value::<PreviewRequest>(
            json!({ "include": ["entries"], "entries": { "kind": "folder" } })
        )
        .is_err()
    );
}

#[test]
fn a_request_with_an_unknown_member_or_section_is_refused() {
    assert!(
        serde_json::from_value::<PreviewRequest>(json!({ "include": ["everything"] })).is_err()
    );
    assert!(serde_json::from_value::<PreviewRequest>(json!({ "include": [], "page": 1 })).is_err());
}

#[test]
fn a_page_past_the_end_is_empty_and_per_page_is_capped() {
    let derived = derived(&["A1.jpg"], |_| {});
    let page = preview(
        &derived,
        &request(json!({ "include": ["entries"], "entries": { "page": 9, "perPage": 100000 } })),
    )
    .entries
    .unwrap();
    assert_eq!(page.total, 1);
    assert!(page.items.is_empty());
    assert_eq!(page.per_page, MAX_PER_PAGE);
}

#[test]
fn an_orphan_names_its_entry_and_not_the_whole_decision() {
    let file = |path: &str| json!({ "file": { "source": "s1", "path": path } });
    let derived = derived(&["A1.jpg"], |plan| {
        plan["decisions"] = json!([{
            "kind": "many",
            "entries": [file("A1.jpg"), file("gone-1.jpg"), file("gone-2.jpg")],
            "change": { "kind": "leave" }
        }]);
    });
    let answer = serde_json::to_value(preview(
        &derived,
        &request(json!({ "include": ["orphans"] })),
    ))
    .unwrap();
    let orphans = answer["orphans"].as_array().unwrap();
    assert_eq!(orphans.len(), 2);
    assert_eq!(orphans[0]["decision"], 0);
    assert_eq!(orphans[0]["entry"], file("gone-1.jpg"));
    assert_eq!(orphans[1]["entry"], file("gone-2.jpg"));
    assert!(
        orphans[0].get("detail").is_none(),
        "the decision is not cloned"
    );
}

/// A v3 plan on source `s1` with these levels and free sets.
fn v3_plan(levels: serde_json::Value, free_sets: serde_json::Value) -> serde_json::Value {
    let mut plan: serde_json::Value =
        serde_json::from_str(include_str!("../fixtures/v3/one_series.json")).unwrap();
    plan["structure"] = json!({ "levels": levels, "freeSets": free_sets });
    plan
}

fn series(id: &str, folder: &str, pattern: Option<&str>) -> serde_json::Value {
    json!({
        "kind": "series", "id": id, "set": null,
        "scope": [{ "source": "s1", "folder": folder }],
        "grouping": { "pattern": pattern, "caseFold": false },
        "example": null, "lists": [],
        "slots": { "item": {}, "media": {}, "itemSet": {} }
    })
}

#[test]
fn a_plan_of_several_series_answers_each_pattern_state_and_names_each_records_series() {
    let plan = v3_plan(
        json!([
            series(
                "letters",
                "Letters",
                Some(r"^(?<identifier>[A-Z]\d+)(?:_(?<part>p\d+))?\.jpg$")
            ),
            series("photos", "Photos", None),
        ]),
        json!([]),
    );
    let derived = derived_of(
        &plan,
        &["Letters/A1_p1.jpg", "Letters/A1_p2.jpg", "Photos/B2.jpg"],
        Catalog::default(),
    );
    let answer = serde_json::to_value(preview(
        &derived,
        &request(json!({ "include": ["records", "shapes"] })),
    ))
    .unwrap();
    assert_eq!(
        answer["pattern"],
        json!({ "groups": ["identifier", "part"] }),
        "pattern stays the first series' state"
    );
    assert_eq!(
        answer["patterns"],
        json!([
            { "series": "letters", "pattern": { "groups": ["identifier", "part"] } },
            { "series": "photos", "pattern": "missing" }
        ])
    );
    let items = answer["records"]["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["series"], "letters");
    assert_eq!(
        answer["shapes"]["groups"]["items"][0]["facts"]["parts"],
        json!(["p1", "p2"]),
        "the parts are read from the path inside the series folder"
    );
}

#[test]
fn a_plan_of_one_series_answers_no_member_that_names_a_series() {
    let derived = derived(&["A1.jpg"], |_| {});
    let answer = serde_json::to_value(preview(
        &derived,
        &request(json!({ "include": ["records"] })),
    ))
    .unwrap();
    assert!(answer.get("patterns").is_none());
    assert!(answer["records"]["items"][0].get("series").is_none());
}

#[test]
fn a_set_named_by_id_shows_the_install_sets_title_as_its_label() {
    let plan = v3_plan(
        json!([series(
            "letters",
            "",
            Some(r"^(?<identifier>[A-Z]\d+)\.jpg$")
        )]),
        json!([{ "id": 42 }, { "id": 43 }]),
    );
    let catalog = Catalog {
        item_sets: vec![ExistingSet {
            id: 42,
            title: "Letters of 1920".into(),
            keys: Default::default(),
        }],
        ..Catalog::default()
    };
    let derived = derived_of(&plan, &["A1.jpg"], catalog);
    let answer = serde_json::to_value(preview(
        &derived,
        &request(json!({ "include": ["records"] })),
    ))
    .unwrap();
    let sets = &answer["records"]["items"][0]["slots"]["sets"];
    assert_eq!(sets[0]["text"], "42", "the value stays the id");
    assert_eq!(sets[0]["label"], "Letters of 1920");
    assert_eq!(sets[1]["text"], "43");
    assert_eq!(sets[1]["label"], json!(null), "no set #43 in the install");
}
