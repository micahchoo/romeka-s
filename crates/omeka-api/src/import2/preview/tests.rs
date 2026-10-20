#![allow(clippy::expect_used, clippy::unwrap_used)]

use serde_json::json;

use super::*;
use crate::import2::catalog::Catalog;
use crate::import2::derive::{Index, IndexedSource, NoTags};
use crate::import2::evidence::Walk;
use crate::import2::plan::{RelPath, SourcePlan, default_plan};

fn derived(paths: &[&str], edit: impl FnOnce(&mut serde_json::Value)) -> Derived {
    let plan = default_plan(vec![SourcePlan::Folder {
        id: SourceId("s1".into()),
        root: "archive".into(),
        path: RelPath::parse("").unwrap(),
    }])
    .unwrap();
    let mut value = serde_json::to_value(plan).unwrap();
    value["grouping"]["pattern"] = json!(r"^(?<identifier>[A-Z]\d+)\.jpg$");
    edit(&mut value);
    let plan = Arc::new(ImportPlanV2::from_json(&value.to_string()).unwrap());
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
    let model = derive(&index, &Sheets::new(), &plan, &Catalog::default(), &NoTags);
    let stored = StoredIndex {
        index: Arc::new(index),
        sources: Vec::new(),
    };
    Derived {
        plan,
        model,
        catalog: Catalog::default(),
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
