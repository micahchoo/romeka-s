#![allow(clippy::expect_used)]

use serde_json::{Value, json};

use super::*;
use crate::import::decisions::{Decision, EntryRef};

const FULL: &str = include_str!("../fixtures/full.json");
const DEFAULT: &str = include_str!("../fixtures/default.json");
const DELETE: &str = include_str!("../fixtures/delete.json");

fn parse(text: &str) -> ImportPlan {
    ImportPlan::from_json(text).expect("a valid plan")
}

/// The full fixture, changed by `edit`, through the boundary.
fn refused(base: &str, edit: impl FnOnce(&mut Value)) -> PlanError {
    let mut value: Value = serde_json::from_str(base).expect("fixture json");
    edit(&mut value);
    match ImportPlan::from_json(&value.to_string()) {
        Ok(_) => panic!("accepted: {value}"),
        Err(error) => error,
    }
}

fn shape_error(error: &PlanError) -> String {
    match error {
        PlanError::Shape(error) => error.to_string(),
        other => panic!("not a shape error: {other:?}"),
    }
}

#[test]
fn every_fixture_round_trips_to_the_same_json() {
    for fixture in [FULL, DEFAULT, DELETE] {
        let written = parse(fixture).to_json();
        let compact = serde_json::from_str::<Value>(fixture)
            .expect("fixture json")
            .to_string();
        assert_eq!(written, compact, "same members, same order");
        assert_eq!(parse(&written), parse(fixture));
    }
}

/// Every `plan` and `ranPlan` member under `value`.
fn stored_plans<'v>(value: &'v Value, out: &mut Vec<&'v Value>) {
    match value {
        Value::Object(members) => {
            for (key, member) in members {
                if matches!(key.as_str(), "plan" | "ranPlan") && member.get("version").is_some() {
                    out.push(member);
                } else {
                    stored_plans(member, out);
                }
            }
        }
        Value::Array(items) => items.iter().for_each(|item| stored_plans(item, out)),
        _ => {}
    }
}

#[test]
fn every_plan_the_admin_is_served_is_stored_byte_for_byte() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../web/admin/tests/fixtures/import");
    let mut seen = 0;
    for entry in std::fs::read_dir(&dir).expect("the admin's import fixtures") {
        let path = entry.expect("fixture entry").path();
        if path.extension().is_none_or(|ending| ending != "json") {
            continue;
        }
        let text = std::fs::read_to_string(&path).expect("fixture text");
        let value: Value = serde_json::from_str(&text).expect("fixture json");
        let mut plans = Vec::new();
        stored_plans(&value, &mut plans);
        for served in plans {
            let stored = parse(&served.to_string()).to_json();
            let at = path.display();
            assert_eq!(
                parse(&stored).to_json(),
                stored,
                "{at}: the stored text is kept"
            );
            let written: Value = serde_json::from_str(&stored).expect("stored json");
            assert_eq!(&written, served, "{at}: the same members");
            seen += 1;
        }
    }
    assert!(
        seen >= 17,
        "only {seen} stored plans found in {}",
        dir.display()
    );
}

#[test]
fn the_full_fixture_reaches_every_member_typed() {
    let plan = parse(FULL);
    assert_eq!(plan.sources[0].id(), &SourceId("s1".into()));
    let series = plan.structure.series(plan.structure.sole());
    assert_eq!(
        series.lists()[1].read.encoding,
        EncodingChoice::Is(Encoding::Utf16Le)
    );
    assert_eq!(series.lists()[1].read.delimiter.as_char(), Some('\t'));
    assert!(series.rules(RecordKind::Item).contains_key(&SlotName::Sets));
    assert_eq!(plan.action.match_by.media.key, MatchKey::InternalId);
    assert_eq!(plan.defaults.owner, Owner::User(4));
    assert_eq!(plan.decisions.len(), 10);
    assert!(matches!(
        &plan.decisions[3],
        Decision::Leave { entry: EntryRef::Row(row) } if row.key.as_str() == "BHC_006_AF_003 #2"
    ));
    assert!(matches!(&plan.decisions[1], Decision::Move { .. }));
}

#[test]
fn a_new_draft_starts_with_the_default_plan() {
    let sources = vec![
        SourcePlan::Folder {
            id: SourceId("s1".into()),
            root: "archive".into(),
            path: RelPath::parse("BHC_006").expect("path"),
        },
        SourcePlan::Staging {
            id: SourceId("staging".into()),
        },
    ];
    let plan = default_plan(sources).expect("default plan");
    assert_eq!(plan, parse(DEFAULT));
    assert_eq!(
        plan.defaults.visibility,
        crate::import::values::Visibility::Private
    );
}

#[test]
fn a_new_draft_with_two_sources_of_one_id_is_refused() {
    let staging = || SourcePlan::Staging {
        id: SourceId("up".into()),
    };
    assert!(matches!(
        default_plan(vec![staging(), staging()]),
        Err(PlanError::DuplicateSource(id)) if id == "up"
    ));
}

#[test]
fn policies_come_from_the_plan_then_the_table_then_the_action() {
    let full = parse(FULL);
    assert_eq!(full.policy(FlagKind::SpaceAtEnds), Some(FlagPolicy::Keep));
    assert_eq!(full.policy(FlagKind::CellsCount), Some(FlagPolicy::Leave));
    assert_eq!(full.policy(FlagKind::Unmatched), Some(FlagPolicy::Leave));
    let mut create = parse(DEFAULT);
    create.action.unmatched = Unmatched::Create;
    assert_eq!(create.policy(FlagKind::Unmatched), Some(FlagPolicy::Keep));
    create.flags.insert(FlagKind::UnknownTerm, FlagPolicy::Keep);
    assert_eq!(
        create.policy(FlagKind::UnknownTerm),
        None,
        "blocking stays blocking"
    );
}

#[test]
fn a_v1_plan_is_refused_as_v1() {
    let v1 = json!({
        "pattern": null, "example": null, "setLevel": "level1", "sets": {}, "typeFor": {},
        "csv": null, "meta": [], "priority": "csv", "edits": {}, "skip": [],
        "withoutFiles": [], "csvRow": {}, "problemRows": "leave"
    });
    assert!(matches!(
        ImportPlan::from_json(&v1.to_string()),
        Err(PlanError::V1Plan)
    ));
}

#[test]
fn another_version_is_refused() {
    for version in [json!(1), json!(3), json!("2"), json!(2.5)] {
        let error = refused(FULL, |plan| plan["version"] = version.clone());
        assert!(
            matches!(error, PlanError::Version(_)),
            "{version}: {error:?}"
        );
    }
    let missing = refused(FULL, |plan| {
        plan.as_object_mut().map(|plan| plan.remove("version"));
    });
    assert!(shape_error(&missing).contains("version"));
}

#[test]
fn unknown_members_are_refused_at_every_depth() {
    let top = refused(FULL, |plan| plan["colour"] = json!("red"));
    assert!(shape_error(&top).contains("colour"));
    let nested = [
        "/sources/0",
        "/grouping",
        "/sheets/0",
        "/sheets/0/read",
        "/sheets/0/join",
        "/sheets/0/join/normalize",
        "/sheets/0/columns/0/options",
        "/slots",
        "/slots/item/sets",
        "/slots/item/sets/sources/0",
        "/action",
        "/action/match/item",
        "/defaults",
        "/decisions/0",
        "/decisions/4/values/0",
        "/newTypes/0",
        "/newTerms/0",
    ];
    for at in nested {
        let error = refused(FULL, |plan| {
            plan.pointer_mut(at).expect(at)["colour"] = json!(1);
        });
        assert!(matches!(error, PlanError::Shape(_)), "{at}");
    }
}

#[test]
fn a_nullable_member_must_still_be_present() {
    for (at, member) in [
        ("/sheets/0", "hash"),
        ("/sheets/0", "itemKey"),
        ("/grouping", "pattern"),
        ("/decisions/4/values/0", "lang"),
        ("/decisions/9", "slot"),
    ] {
        let error = refused(FULL, |plan| {
            plan.pointer_mut(at)
                .and_then(Value::as_object_mut)
                .map(|object| object.remove(member));
        });
        assert!(shape_error(&error).contains(member), "{at}/{member}");
    }
}

#[test]
fn closed_string_sets_refuse_other_strings() {
    for (at, bad) in [
        ("/sheets/0/read/delimiter", json!("#")),
        ("/sheets/0/read/encoding", json!("shift-jis")),
        ("/sheets/0/read/quote", json!("`")),
        ("/sheets/0/rowKind", json!("items")),
        (
            "/slots/item/dcterms:created/sources/0/tag",
            json!("ShutterSpeed"),
        ),
        ("/flags", json!({ "made-up": "keep" })),
        ("/flags", json!({ "exists": "fix" })),
        ("/defaults/owner", json!("admin")),
        ("/action/match/item/key", json!("title")),
        ("/newTypes/1/from", json!({ "starter": "map" })),
    ] {
        let error = refused(FULL, |plan| {
            *plan.pointer_mut(at).expect(at) = bad.clone();
        });
        assert!(matches!(error, PlanError::Shape(_)), "{at} = {bad}");
    }
}

#[test]
fn zero_is_not_a_header_line_or_a_folder_level() {
    for at in [
        "/sheets/0/read/headerLine",
        "/slots/item/sets/sources/0/level",
    ] {
        let error = refused(FULL, |plan| *plan.pointer_mut(at).expect(at) = json!(0));
        assert!(matches!(error, PlanError::Shape(_)), "{at}");
    }
}

#[test]
fn only_a_file_entry_moves() {
    let error = refused(FULL, |plan| {
        plan["decisions"][0]["file"] = json!({ "row": { "sheet": "register", "key": "1" } });
    });
    assert!(matches!(error, PlanError::Shape(_)));
}

fn with_decisions(decisions: Value) -> Result<ImportPlan, PlanError> {
    let mut value: Value = serde_json::from_str(FULL).expect("fixture json");
    value["decisions"] = decisions;
    ImportPlan::from_json(&value.to_string())
}

fn file(path: &str) -> Value {
    json!({ "file": { "source": "s1", "path": path } })
}

#[test]
fn a_many_change_and_a_pair_round_trip() {
    let decisions = json!([
        { "kind": "many", "entries": [file("a.jpg"), { "row": { "sheet": "register", "key": "A#1" } }],
          "change": { "kind": "set", "target": "item", "slot": "dcterms:title",
                      "values": [{ "type": "literal", "text": "T", "lang": null, "label": null }] } },
        { "kind": "many", "entries": [file("a.jpg")], "change": { "kind": "leave" } },
        { "kind": "many", "entries": [file("a.jpg")], "change": { "kind": "move", "to": { "with": file("b.jpg") } } },
        { "kind": "many", "entries": [file("a.jpg")], "change": { "kind": "match", "target": "item", "resource": null } },
        { "kind": "many", "entries": [file("a.jpg")],
          "change": { "kind": "flag", "flag": "date-form", "slot": null, "policy": "keep" } },
        { "kind": "pair", "row": { "sheet": "register", "key": "A#1" }, "file": { "source": "s1", "path": "a.jpg" } }
    ]);
    let plan = with_decisions(decisions.clone()).expect("a valid plan");
    let written: Value = serde_json::from_str(&plan.to_json()).expect("json");
    assert_eq!(written["decisions"], decisions);
}

#[test]
fn a_many_change_names_at_least_one_entry() {
    let error =
        with_decisions(json!([{ "kind": "many", "entries": [], "change": { "kind": "leave" } }]))
            .expect_err("refused");
    assert!(matches!(error, PlanError::EmptyMany(0)), "{error:?}");
}

#[test]
fn a_many_change_is_one_change_never_another_many() {
    let error = with_decisions(json!([{ "kind": "many", "entries": [file("a.jpg")],
        "change": { "kind": "many", "entries": [file("a.jpg")], "change": { "kind": "leave" } } }]))
    .expect_err("refused");
    assert!(shape_error(&error).contains("many"), "{error:?}");
    let error = with_decisions(json!([{ "kind": "many", "entries": [file("a.jpg")],
        "change": { "kind": "leave", "entry": file("a.jpg") } }]))
    .expect_err("refused");
    assert!(
        shape_error(&error).contains("entry"),
        "the change names no entry: {error:?}"
    );
}

#[test]
fn only_file_entries_move_in_a_many_change() {
    let error = with_decisions(json!([{ "kind": "many",
        "entries": [file("a.jpg"), { "row": { "sheet": "register", "key": "1" } }],
        "change": { "kind": "move", "to": "own" } }]))
    .expect_err("refused");
    assert!(matches!(error, PlanError::ManyMovesRow(0)), "{error:?}");
}

#[test]
fn a_pair_on_a_sheet_the_plan_lacks_is_kept_for_the_derive_to_orphan() {
    let plan = with_decisions(
        json!([{ "kind": "pair", "row": { "sheet": "gone", "key": "1" },
        "file": { "source": "gone", "path": "a.jpg" } }]),
    )
    .expect("not a refusal");
    assert_eq!(plan.decisions.len(), 1);
}

#[test]
fn duplicate_source_ids_are_refused() {
    let error = refused(FULL, |plan| plan["sources"][1]["id"] = json!("s1"));
    assert!(matches!(error, PlanError::DuplicateSource(id) if id == "s1"));
}

#[test]
fn duplicate_sheet_ids_are_refused() {
    let error = refused(FULL, |plan| plan["sheets"][1]["id"] = json!("register"));
    assert!(matches!(error, PlanError::DuplicateSheet(id) if id == "register"));
}

#[test]
fn a_sheet_in_a_missing_source_is_refused() {
    let error = refused(FULL, |plan| plan["sheets"][1]["source"] = json!("gone"));
    assert!(matches!(
        error,
        PlanError::MissingSource { sheet, missing } if sheet == "mixed" && missing == "gone"
    ));
}

#[test]
fn a_slot_reading_a_missing_sheet_is_refused() {
    let error = refused(FULL, |plan| {
        plan["slots"]["media"]["ingester"]["sources"][0]["sheet"] = json!("gone");
    });
    assert!(matches!(error, PlanError::MissingSheet(sheet) if sheet == "gone"));
}

#[test]
fn a_slot_reading_an_unmapped_column_is_refused() {
    let error = refused(FULL, |plan| {
        plan["slots"]["item"]["dcterms:title"]["sources"][1]["column"]["index"] = json!(9);
    });
    assert!(matches!(
        error,
        PlanError::MissingColumn { sheet, index: 9 } if sheet == "register"
    ));
}

#[test]
fn a_column_mapped_twice_is_refused() {
    let error = refused(FULL, |plan| {
        plan["sheets"][0]["columns"][2]["column"]["index"] = json!(1);
    });
    assert!(matches!(
        error,
        PlanError::DuplicateColumn { sheet, index: 1 } if sheet == "register"
    ));
}

#[test]
fn combine_all_on_a_single_valued_slot_is_refused() {
    for (kind, slot) in [
        ("item", "template"),
        ("media", "mediaOrder"),
        ("media", "ingester"),
    ] {
        let error = refused(FULL, |plan| {
            plan["slots"][kind][slot]["combine"] = json!("all");
        });
        assert!(
            matches!(&error, PlanError::CombineAll { slot: name, .. } if name == slot),
            "{kind}.{slot}: {error:?}"
        );
    }
}

#[test]
fn slot_rules_on_a_delete_plan_are_refused() {
    let error = refused(DELETE, |plan| {
        plan["slots"]["itemSet"]["dcterms:title"] =
            json!({ "sources": [{ "from": "constant", "value": "x" }], "combine": "first" });
    });
    assert!(matches!(error, PlanError::SlotRulesOnDelete));
    let error = refused(FULL, |plan| plan["action"]["action"] = json!("delete"));
    assert!(matches!(error, PlanError::SlotRulesOnDelete));
}

#[test]
fn two_new_types_with_one_title_are_refused() {
    let error = refused(FULL, |plan| {
        plan["newTypes"][1]["label"] = json!(" ARTIFACT ")
    });
    assert!(matches!(error, PlanError::DuplicateNewType(label) if label == " ARTIFACT "));
}

#[test]
fn a_new_type_without_a_label_is_refused() {
    let error = refused(FULL, |plan| plan["newTypes"][0]["label"] = json!("  "));
    assert!(matches!(error, PlanError::EmptyTypeLabel(_)));
}

#[test]
fn a_new_type_with_an_existing_template_label_is_refused() {
    let plan = parse(FULL);
    assert!(
        plan.refuse_existing_labels(["Letter", "Base Resource"])
            .is_ok()
    );
    assert!(matches!(
        plan.refuse_existing_labels(["Letter", "photo"]),
        Err(PlanError::TypeExists(label)) if label == "Photo"
    ));
}

#[test]
fn new_terms_that_make_one_term_are_refused() {
    let error = refused(FULL, |plan| {
        plan["newTerms"][1]["label"] = json!("shelf-MARK");
    });
    assert!(matches!(
        error,
        PlanError::TermsCollide { first, second, term }
            if first == "Shelf mark" && second == "shelf-MARK" && term == "local:shelfMark"
    ));
}

#[test]
fn a_new_term_without_letters_is_refused() {
    let error = refused(FULL, |plan| plan["newTerms"][0]["label"] = json!(" - "));
    assert!(matches!(error, PlanError::EmptyTermLabel(_)));
}

#[test]
fn every_path_member_refuses_a_path_that_leaves_its_source() {
    let members = [
        "/sources/0/path",
        "/sheets/0/file",
        "/decisions/0/file/file/path",
        "/decisions/0/to/with/file/path",
        "/decisions/4/entry/file/path",
    ];
    let bad = [
        "../x",
        "a/%2e%2e/x",
        "a\u{0}b",
        "a%00b",
        "a\\b",
        "a%5Cb",
        "/x",
        "%2fx",
    ];
    for at in members {
        for path in bad {
            let error = refused(FULL, |plan| {
                *plan.pointer_mut(at).expect(at) = json!(path);
            });
            assert!(shape_error(&error).contains("path"), "{at} = {path:?}");
        }
    }
}

#[test]
fn paths_and_row_keys_are_stored_nfc() {
    let decomposed = edited(|plan| {
        plan["sources"][0]["path"] = json!("Cafe\u{301}");
        plan["decisions"][3]["entry"]["row"]["key"] = json!("Cafe\u{301}");
    });
    let SourcePlan::Folder { path, .. } = &decomposed.sources[0] else {
        panic!("folder source");
    };
    assert_eq!(path.as_str(), "Caf\u{e9}");
    assert!(matches!(
        &decomposed.decisions[3],
        Decision::Leave { entry: EntryRef::Row(row) } if row.key.as_str() == "Caf\u{e9}"
    ));
}

fn edited(edit: impl FnOnce(&mut Value)) -> ImportPlan {
    let mut value: Value = serde_json::from_str(FULL).expect("fixture json");
    edit(&mut value);
    parse(&value.to_string())
}

#[test]
fn a_slot_rule_value_map_round_trips_and_a_missing_map_is_empty() {
    let plan = edited(|plan| {
        plan["slots"]["item"]["template"]["map"] = json!({ "AF": "Artifact", "PH": "" });
    });
    fn item_rules(plan: &ImportPlan) -> &SlotRules {
        plan.structure
            .series(plan.structure.sole())
            .rules(RecordKind::Item)
    }
    let map = &item_rules(&plan)[&SlotName::Template].map;
    assert_eq!(map.get("AF").map(String::as_str), Some("Artifact"));
    assert_eq!(map.get("PH").map(String::as_str), Some(""));
    let written: Value = serde_json::from_str(&plan.to_json()).expect("json");
    assert_eq!(
        written["slots"]["item"]["template"]["map"],
        json!({ "AF": "Artifact", "PH": "" }),
        "the map keeps its order"
    );
    assert_eq!(parse(&plan.to_json()), plan);
    let without = parse(FULL);
    assert!(item_rules(&without)[&SlotName::Template].map.is_empty());
}

#[test]
fn a_value_map_holds_only_text() {
    let error = refused(FULL, |plan| {
        plan["slots"]["item"]["template"]["map"] = json!({ "AF": 3 });
    });
    assert!(shape_error(&error).contains("string"), "{error:?}");
}

#[test]
fn two_value_map_keys_that_fold_to_one_title_are_refused() {
    let error = refused(FULL, |plan| {
        plan["slots"]["item"]["template"]["map"] =
            json!({ "AF": "Artifact", " af ": "Photograph" });
    });
    assert!(
        matches!(&error, PlanError::MapKeysCollide { first, second, .. }
            if first == "AF" && second == " af "),
        "{error:?}"
    );
}

#[test]
fn the_simple_screen_state_is_kept_and_an_absent_one_is_not_written() {
    let mut value: Value = serde_json::from_str(DEFAULT).expect("fixture json");
    assert!(
        value.get("simple").is_none(),
        "the recorded fixture has none"
    );
    value["simple"] =
        json!({ "answered": ["identical"], "notLists": [{ "source": "s1", "file": "a.csv" }] });
    let plan = parse(&value.to_string());
    assert_eq!(
        plan.simple.as_ref().map(|state| state["answered"].clone()),
        Some(json!(["identical"]))
    );
    assert_eq!(
        serde_json::from_str::<Value>(&plan.to_json()).expect("json"),
        value
    );
    let without = parse(DEFAULT);
    assert!(without.simple.is_none());
    assert!(!without.to_json().contains("simple"));
}
