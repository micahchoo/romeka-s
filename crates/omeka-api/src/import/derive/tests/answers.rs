//! The plan's answers to anomalies, applied in the derive
//! (`docs/import-v2.md`, "Answers to anomalies").

use serde_json::json;

use super::support::*;
use crate::import::decisions::EntryRef;
use crate::import::derive::{Index, Sheets};
use crate::import::evidence::Identical;
use crate::import::model::{Fate, GroupKey, LeftBy, Model, Origin};
use crate::import::plan::RecordKind;

const FILES: &[(&str, &str)] = &[("A1_front.jpg", JPEG), ("B2.jpg", JPEG), ("C3.jpg", JPEG)];

/// `FILES`, with `A1_front.jpg` and `B2.jpg` byte for byte the same.
fn with_identical() -> Index {
    let mut index = index(FILES);
    index.sources[0].walk.anomalies.identical = vec![Identical {
        size: 100,
        sha256: "ab".repeat(32),
        paths: vec!["A1_front.jpg".into(), "B2.jpg".into()],
    }];
    index
}

fn left_of(model: &Model, path: &str) -> Option<LeftBy> {
    model
        .entries
        .iter()
        .find(|state| matches!(&state.entry, EntryRef::File(file) if file.path.as_str() == path))
        .unwrap_or_else(|| panic!("no entry {path}"))
        .left
}

fn item_keys(model: &Model) -> Vec<String> {
    kept(model, RecordKind::Item)
        .iter()
        .map(|record| record.origin.key())
        .collect()
}

#[test]
fn keep_one_is_the_default_and_keeps_the_first_identical_file_by_path() {
    let plan = plan(|plan| plan["grouping"]["pattern"] = json!(PATTERN));
    let model = run(&with_identical(), &Sheets::new(), &plan);
    assert_eq!(left_of(&model, "A1_front.jpg"), None);
    assert_eq!(left_of(&model, "B2.jpg"), Some(LeftBy::Identical));
    assert_eq!(left_of(&model, "C3.jpg"), None);
    assert_eq!(media_paths(&model, item(&model, "A1")), ["A1_front.jpg"]);
    assert!(
        kept(&model, RecordKind::Item)
            .iter()
            .all(|record| record.resolved.item_key.as_deref() != Some("B2")),
        "B2 had one file, and it is left out"
    );
}

#[test]
fn keep_one_keeps_the_file_a_list_row_names() {
    let plan = plan(|plan| {
        plan["grouping"]["pattern"] = json!(PATTERN);
        plan["sheets"] = json!([sheet_plan("list", &["File"])]);
        plan["sheets"][0]["join"] = join_on("fileName", 0, "File", false, false, false);
    });
    let sheets = read_sheets(&plan, &[("list", b"File\nB2.jpg\n")]);
    let model = run(&with_identical(), &sheets, &plan);
    assert_eq!(left_of(&model, "B2.jpg"), None, "the row names B2.jpg");
    assert_eq!(left_of(&model, "A1_front.jpg"), Some(LeftBy::Identical));
}

#[test]
fn keep_all_keeps_every_identical_file() {
    let plan = plan(|plan| {
        plan["grouping"]["pattern"] = json!(PATTERN);
        plan["anomalies"] = json!({ "identical": "keepAll" });
    });
    let model = run(&with_identical(), &Sheets::new(), &plan);
    assert_eq!(left_of(&model, "B2.jpg"), None);
    assert_eq!(item_keys(&model).len(), 3);
}

#[test]
fn a_move_decision_wins_over_keep_one() {
    let plan = plan(|plan| {
        plan["grouping"]["pattern"] = json!(PATTERN);
        plan["decisions"] = json!([{ "kind": "move",
            "file": { "file": { "source": "s1", "path": "B2.jpg" } }, "to": "own" }]);
    });
    let model = run(&with_identical(), &Sheets::new(), &plan);
    assert_eq!(left_of(&model, "B2.jpg"), None, "the person moved it");
}

const STRAY_PATTERN: &str = r"^(?<identifier>[A-Z]\d)_(?<part>[a-z]+)\.jpg$";

#[test]
fn unmatched_files_are_left_out_by_default() {
    let plan = plan(|plan| plan["grouping"]["pattern"] = json!(STRAY_PATTERN));
    let model = run(&index(FILES), &Sheets::new(), &plan);
    assert_eq!(item_keys(&model), ["group:A1"]);
    let stray = model
        .entries
        .iter()
        .find(
            |state| matches!(&state.entry, EntryRef::File(file) if file.path.as_str() == "B2.jpg"),
        )
        .expect("B2.jpg");
    assert!(stray.unmatched && stray.record.is_none());
}

#[test]
fn own_makes_each_unmatched_file_an_item_of_its_own() {
    let plan = plan(|plan| {
        plan["grouping"]["pattern"] = json!(STRAY_PATTERN);
        plan["anomalies"] = json!({ "unmatchedFiles": "own" });
    });
    let model = run(&index(FILES), &Sheets::new(), &plan);
    let items = kept(&model, RecordKind::Item);
    assert_eq!(items.len(), 3);
    for path in ["B2.jpg", "C3.jpg"] {
        assert!(
            items.iter().any(|record| matches!(&record.origin,
                Origin::Group(GroupKey::Own(file)) if file.path.as_str() == path)),
            "{path} is its own item"
        );
    }
}

#[test]
fn own_needs_a_pattern_that_compiles() {
    let plan = plan(|plan| {
        plan["grouping"]["pattern"] = json!(null);
        plan["anomalies"] = json!({ "unmatchedFiles": "own" });
    });
    let model = run(&index(FILES), &Sheets::new(), &plan);
    assert!(
        item_keys(&model).is_empty(),
        "with no pattern yet, no file is one that a pattern does not fit"
    );
}

fn listed(answer: &str) -> Model {
    let plan = plan(|plan| {
        plan["grouping"]["pattern"] = json!(PATTERN);
        plan["sheets"] = json!([sheet_plan("list", &["Id"])]);
        plan["sheets"][0]["join"] = join_on("identifier", 0, "Id", false, false, false);
        plan["anomalies"] = json!({ "itemsWithoutRow": answer });
    });
    let sheets = read_sheets(&plan, &[("list", b"Id\nA1\n")]);
    run(&index(FILES), &sheets, &plan)
}

#[test]
fn items_without_a_row_are_kept_by_default() {
    assert_eq!(item_keys(&listed("keep")).len(), 3);
}

#[test]
fn leave_leaves_out_the_items_no_row_joined_and_their_media() {
    let model = listed("leave");
    assert_eq!(item_keys(&model), ["group:A1"]);
    let b2 = model
        .records
        .iter()
        .find(|record| record.origin == Origin::Group(GroupKey::Pattern("B2".into())))
        .expect("B2");
    assert_eq!(b2.fate, Fate::Left(LeftBy::NoRow));
    assert_eq!(left_of(&model, "B2.jpg"), Some(LeftBy::Parent));
}

#[test]
fn leave_changes_nothing_when_no_list_joins_items() {
    let plan = plan(|plan| {
        plan["grouping"]["pattern"] = json!(PATTERN);
        plan["anomalies"] = json!({ "itemsWithoutRow": "leave" });
    });
    let model = run(&index(FILES), &Sheets::new(), &plan);
    assert_eq!(item_keys(&model).len(), 3);
}

#[test]
fn leave_keeps_an_item_whose_rows_describe_its_files() {
    let plan = plan(|plan| {
        plan["grouping"]["pattern"] = json!(r"^(?<identifier>X_\d{3})(?<part>[a-z])\.[^.]+$");
        plan["sheets"] = json!([sheet_plan("views", &["File"])]);
        plan["sheets"][0]["rowKind"] = json!("media");
        plan["sheets"][0]["join"] = join_on("fileName", 0, "File", false, false, false);
        plan["anomalies"] = json!({ "itemsWithoutRow": "leave" });
    });
    let files = [
        ("X_010a.jpg", JPEG),
        ("X_010b.jpg", JPEG),
        ("X_020a.jpg", JPEG),
    ];
    let sheets = read_sheets(&plan, &[("views", b"File\nX_010b.jpg\n")]);
    let model = run(&index(&files), &sheets, &plan);
    assert_eq!(
        item_keys(&model),
        ["group:X_010"],
        "X_010 has a row through its view b; X_020 has none"
    );
    assert_eq!(left_of(&model, "X_020a.jpg"), Some(LeftBy::Parent));
}
