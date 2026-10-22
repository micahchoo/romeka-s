#![allow(clippy::expect_used)]

use serde_json::{Value, json};

use super::*;

const FULL: &str = include_str!("../fixtures/full.json");
const DEFAULT: &str = include_str!("../fixtures/default.json");
const DELETE: &str = include_str!("../fixtures/delete.json");

const BHC006: &str = include_str!("../fixtures/v3/bhc006.json");
const ARCHIVE_AS_FOUND: &str = include_str!("../fixtures/v3/archive_as_found.json");
const ONE_SERIES: &str = include_str!("../fixtures/v3/one_series.json");

fn compact(text: &str) -> String {
    serde_json::from_str::<Value>(text)
        .expect("fixture json")
        .to_string()
}

fn parse(text: &str) -> ImportPlan {
    ImportPlan::from_json(text).unwrap_or_else(|error| panic!("{error}: {text}"))
}

/// The plan written as v3, whatever it came in as.
fn as_v3(plan: &ImportPlan) -> Value {
    let mut plan = plan.clone();
    plan.version = PlanVersion::V3;
    serde_json::from_str(&plan.to_json()).expect("v3 json")
}

/// Every v2 plan the repo holds: the plan fixtures, every plan the admin's
/// recorded fixtures were served, and the plan a new draft starts with.
fn every_v2_plan() -> Vec<(String, String)> {
    let mut plans: Vec<(String, String)> =
        [("full", FULL), ("default", DEFAULT), ("delete", DELETE)]
            .into_iter()
            .map(|(name, text)| (name.to_owned(), compact(text)))
            .collect();
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../web/admin/tests/fixtures/import");
    for entry in std::fs::read_dir(&dir).expect("the admin's import fixtures") {
        let path = entry.expect("fixture entry").path();
        if path.extension().is_none_or(|ending| ending != "json") {
            continue;
        }
        let value: Value =
            serde_json::from_str(&std::fs::read_to_string(&path).expect("fixture text"))
                .expect("fixture json");
        let mut stack = vec![&value];
        while let Some(value) = stack.pop() {
            match value {
                Value::Object(members) => {
                    for (key, member) in members {
                        if matches!(key.as_str(), "plan" | "ranPlan")
                            && member.get("version") == Some(&json!(2))
                        {
                            // The recording sorts members; the database
                            // holds the text the server wrote.
                            let stored = parse(&member.to_string()).to_json();
                            let name = format!("{} {key}", path.display());
                            let written: Value = serde_json::from_str(&stored).expect("json");
                            assert_eq!(&written, member, "{name}: the same members");
                            plans.push((name, stored));
                        } else {
                            stack.push(member);
                        }
                    }
                }
                Value::Array(items) => stack.extend(items),
                _ => {}
            }
        }
    }
    let draft = default_plan(vec![SourcePlan::Staging {
        id: SourceId("staging".into()),
    }])
    .expect("default plan");
    plans.push(("default_plan".to_owned(), draft.to_json()));
    plans
}

#[test]
fn lift_then_lower_is_identity() {
    let plans = every_v2_plan();
    assert!(plans.len() >= 20, "only {} v2 plans found", plans.len());
    for (name, text) in plans {
        let plan = parse(&text);
        assert_eq!(plan.version, PlanVersion::V2, "{name}");
        assert_eq!(plan.to_json(), text, "{name}: lowered byte for byte");

        let v2: Value = serde_json::from_str(&text).expect("v2 json");
        let v3 = as_v3(&plan);
        assert_eq!(v3["version"], 3, "{name}");
        let levels = v3["structure"]["levels"].as_array().expect("levels");
        assert_eq!(levels.len(), 1, "{name}: one series");
        let series = &levels[0];
        assert_eq!(
            (
                &series["kind"],
                &series["id"],
                &series["set"],
                &series["scope"]
            ),
            (&json!("series"), &json!("all"), &Value::Null, &json!("all")),
            "{name}: the series all, with no set, takes every file"
        );
        assert_eq!(series["grouping"], v2["grouping"], "{name}");
        assert_eq!(series["lists"], v2["sheets"], "{name}");
        let mut slots = v2["slots"].clone();
        let sets = slots["item"]
            .as_object_mut()
            .and_then(|item| item.shift_remove("sets"));
        assert_eq!(series["slots"], slots, "{name}: slots without item.sets");
        let free_sets = &v3["structure"]["freeSets"];
        match sets {
            None => assert_eq!(free_sets, &json!([]), "{name}"),
            Some(mut rule) => {
                rule["names"] = json!("text");
                assert_eq!(free_sets, &json!([{ "each": rule }]), "{name}");
            }
        }

        let mut through_v3 = parse(&v3.to_string());
        assert_eq!(through_v3.version, PlanVersion::V3, "{name}");
        through_v3.version = PlanVersion::V2;
        let lowered: Value = serde_json::from_str(&through_v3.to_json()).expect("json");
        assert_eq!(
            lowered, v2,
            "{name}: lowered from the v3 wire, the same members (a sets rule ends its kind)"
        );
    }
}

#[test]
fn the_design_examples_parse() {
    for (name, text) in [
        ("bhc006", BHC006),
        ("archive_as_found", ARCHIVE_AS_FOUND),
        ("one_series", ONE_SERIES),
    ] {
        if let Err(error) = serde_json::from_str::<v3::PlanV3>(text) {
            panic!("{name}: {error}");
        }
    }

    for text in [BHC006, ARCHIVE_AS_FOUND, ONE_SERIES] {
        let plan = parse(text);
        assert_eq!(plan.version, PlanVersion::V3);
        assert_eq!(plan.to_json(), compact(text), "written back byte for byte");
    }
}

#[test]
fn the_archive_as_found_is_the_lift_of_the_default_plan() {
    let lifted = parse(DEFAULT);
    let v3 = parse(ARCHIVE_AS_FOUND);
    assert_eq!(
        ImportPlan {
            version: PlanVersion::V3,
            ..lifted.clone()
        },
        v3
    );
    assert_eq!(as_v3(&lifted).to_string(), compact(ARCHIVE_AS_FOUND));
}

/// The archive-as-found fixture, changed by `edit`, through the boundary.
fn refused(edit: impl FnOnce(&mut Value)) -> PlanError {
    let mut value: Value = serde_json::from_str(ARCHIVE_AS_FOUND).expect("fixture json");
    edit(&mut value);
    match ImportPlan::from_json(&value.to_string()) {
        Ok(_) => panic!("accepted: {value}"),
        Err(error) => error,
    }
}

fn series(id: &str, scope: Value) -> Value {
    let mut series: Value =
        serde_json::from_str::<Value>(ARCHIVE_AS_FOUND).expect("json")["structure"]["levels"][0]
            .clone();
    series["id"] = json!(id);
    series["scope"] = scope;
    series
}

fn folder(source: &str, folder: &str) -> Value {
    json!([{ "source": source, "folder": folder }])
}

#[test]
fn unknown_members_are_refused_at_every_depth_of_the_tree() {
    let at = |pointer: &'static str| {
        refused(|plan| {
            plan.pointer_mut(pointer).expect(pointer)["colour"] = json!(1);
        })
    };
    for pointer in [
        "",
        "/structure",
        "/structure/levels/0",
        "/structure/levels/0/slots",
    ] {
        let error = at(pointer);
        assert!(
            matches!(&error, PlanError::ShapeV3(shape) if shape.to_string().contains("colour")),
            "{pointer}: {error:?}"
        );
    }
    let in_a_set = refused(|plan| {
        plan["structure"]["levels"] = json!([{
            "kind": "set", "id": "p", "set": { "title": "P", "description": null, "colour": 1 },
            "levels": []
        }]);
    });
    assert!(matches!(in_a_set, PlanError::ShapeV3(_)), "{in_a_set:?}");
    let in_each = refused(|plan| {
        plan["structure"]["freeSets"] = json!([{ "each": {
            "sources": [], "combine": "first", "names": "title", "colour": 1
        } }]);
    });
    assert!(matches!(in_each, PlanError::ShapeV3(_)), "{in_each:?}");
    let other_kind = refused(|plan| plan["structure"]["levels"][0]["kind"] = json!("file"));
    assert!(
        matches!(other_kind, PlanError::ShapeV3(_)),
        "{other_kind:?}"
    );
}

#[test]
fn a_plan_with_the_other_versions_members_is_refused() {
    let v3_with_v2_members = {
        let mut value: Value = serde_json::from_str(FULL).expect("json");
        value["version"] = json!(3);
        ImportPlan::from_json(&value.to_string()).expect_err("refused")
    };
    assert!(
        matches!(&v3_with_v2_members, PlanError::ShapeV3(shape) if shape.to_string().contains("grouping")),
        "{v3_with_v2_members:?}"
    );
    let v2_with_v3_members = refused(|plan| plan["version"] = json!(2));
    assert!(
        matches!(&v2_with_v3_members, PlanError::Shape(shape) if shape.to_string().contains("structure")),
        "{v2_with_v3_members:?}"
    );
}

#[test]
fn two_levels_with_one_id_are_refused() {
    let two_series = refused(|plan| {
        plan["structure"]["levels"] = json!([
            series("a", folder("s1", "A")),
            series("a", folder("s1", "B")),
        ]);
    });
    assert!(
        matches!(&two_series, PlanError::DuplicateLevel(id) if id == "a"),
        "{two_series:?}"
    );
    let set_and_series = refused(|plan| {
        plan["structure"]["levels"] = json!([{
            "kind": "set", "id": "a", "set": { "title": "A", "description": null },
            "levels": [series("a", json!("all"))]
        }]);
    });
    assert!(
        matches!(&set_and_series, PlanError::DuplicateLevel(id) if id == "a"),
        "{set_and_series:?}"
    );
}

#[test]
fn two_series_that_take_the_same_files_are_refused() {
    let both_all = refused(|plan| {
        plan["structure"]["levels"] = json!([series("a", json!("all")), series("b", json!("all"))]);
    });
    assert!(
        matches!(&both_all, PlanError::DuplicateScope { first, second } if first == "a" && second == "b"),
        "{both_all:?}"
    );
    let one_folder = refused(|plan| {
        plan["structure"]["levels"] = json!([
            series("a", folder("s1", "Letters")),
            series("b", folder("s1", "Letters")),
        ]);
    });
    assert!(
        matches!(one_folder, PlanError::DuplicateScope { .. }),
        "{one_folder:?}"
    );
}

#[test]
fn a_scope_in_a_missing_source_is_refused() {
    let error = refused(|plan| {
        plan["structure"]["levels"][0]["scope"] = folder("s9", "Letters");
    });
    assert!(
        matches!(&error, PlanError::ScopeSource { series, missing } if series == "all" && missing == "s9"),
        "{error:?}"
    );
}

#[test]
fn a_series_slot_that_reads_another_series_list_is_refused() {
    let list = json!({
        "id": "register", "source": "s1", "file": "register.csv", "hash": null,
        "read": { "encoding": "auto", "delimiter": "auto", "quote": "\"", "headerLine": 1, "trim": true },
        "rowKind": "item", "join": null, "itemKey": null,
        "columns": [{ "column": { "index": 0, "header": "Title" },
            "options": { "split": null, "type": "literal", "lang": null, "private": false } }]
    });
    let error = refused(|plan| {
        let mut a = series("a", folder("s1", "A"));
        a["lists"] = json!([list]);
        let mut b = series("b", folder("s1", "B"));
        b["slots"]["item"]["dcterms:title"] = json!({
            "sources": [{ "from": "column", "sheet": "register", "column": { "index": 0, "header": "Title" } }],
            "combine": "first"
        });
        plan["structure"]["levels"] = json!([a, b]);
    });
    assert!(
        matches!(&error, PlanError::ForeignList { series, sheet } if series == "b" && sheet == "register"),
        "{error:?}"
    );
}

#[test]
fn a_series_slot_rule_for_sets_is_refused() {
    let error = refused(|plan| {
        plan["structure"]["levels"][0]["slots"]["item"]["sets"] =
            json!({ "sources": [{ "from": "folder", "level": 1 }], "combine": "first" });
    });
    assert!(
        matches!(&error, PlanError::SeriesSets(id) if id == "all"),
        "{error:?}"
    );
}

#[test]
fn a_set_named_by_an_empty_title_is_refused() {
    let error = refused(|plan| {
        plan["structure"]["freeSets"] = json!([{ "title": "  ", "description": null }]);
    });
    assert!(matches!(error, PlanError::EmptySetTitle(_)), "{error:?}");
}

#[test]
fn set_levels_series_sets_and_free_sets_pass_the_boundary() {
    let mut set = series("all", json!("all"));
    set["set"] = json!({ "id": 4 });
    let structures = [
        json!({ "levels": [{ "kind": "set", "id": "p", "set": { "id": 4 },
            "levels": [series("all", json!("all"))] }], "freeSets": [] }),
        json!({ "levels": [series("all", json!("all"))],
            "freeSets": [{ "title": "Exhibition 2018", "description": null }] }),
        json!({ "levels": [series("all", json!("all"))], "freeSets": [{ "each": {
            "sources": [{ "from": "constant", "value": "2018" }], "combine": "first", "names": "title"
        } }] }),
        json!({ "levels": [set], "freeSets": [] }),
    ];
    for structure in structures {
        let mut plan: Value = serde_json::from_str(ARCHIVE_AS_FOUND).expect("fixture json");
        plan["structure"] = structure.clone();
        let parsed = parse(&plan.to_string());
        assert_eq!(parsed.to_json(), plan.to_string(), "{structure}");
    }
}

#[test]
fn the_lower_refuses_what_v2_cannot_say() {
    let drive = parse(ONE_SERIES);
    let mut as_v2 = drive.clone();
    as_v2.version = PlanVersion::V2;
    assert!(matches!(
        v2::PlanV2::try_from(as_v2.clone()),
        Err(PlanError::NotV2(why)) if why.contains("\"all\"")
    ));
    assert_eq!(
        as_v2.to_json(),
        drive.to_json(),
        "a v2 plan the lower refuses is written as v3, not cut"
    );

    let refusals = [
        json!({ "levels": [], "freeSets": [] }),
        json!({ "levels": [{ "kind": "set", "id": "p", "set": { "id": 4 }, "levels": [] }], "freeSets": [] }),
        json!({ "levels": [series("all", folder("s1", ""))], "freeSets": [] }),
        json!({ "levels": [series("all", json!("all"))],
            "freeSets": [{ "title": "Exhibition 2018", "description": null }] }),
    ];
    for structure in refusals {
        let mut value: Value = serde_json::from_str(ARCHIVE_AS_FOUND).expect("json");
        value["structure"] = structure.clone();
        let wire: v3::PlanV3 = serde_json::from_value(value).expect("v3 wire");
        let plan = ImportPlan::from(wire);
        assert!(
            matches!(v2::PlanV2::try_from(plan), Err(PlanError::NotV2(_))),
            "{structure}"
        );
    }
}

#[test]
fn a_join_by_last_places_is_said_only_in_v3() {
    let full = parse(FULL);
    assert!(
        !full.to_json().contains("lastPlaces"),
        "the member is left out when off"
    );
    let mut value: Value = serde_json::from_str(FULL).expect("json");
    value["sheets"][1]["join"]["normalize"]["lastPlaces"] = json!(2);
    let plan = parse(&value.to_string());
    let written: Value = serde_json::from_str(&plan.to_json()).expect("json");
    assert_eq!(written["version"], 3, "v2 cannot say it");
    let joins: Vec<&Value> = written["structure"]["levels"][0]["lists"]
        .as_array()
        .expect("lists")
        .iter()
        .map(|list| &list["join"]["normalize"]["lastPlaces"])
        .collect();
    assert_eq!(joins, [&Value::Null, &json!(2)]);
    assert!(matches!(
        v2::PlanV2::try_from(plan),
        Err(PlanError::NotV2(_))
    ));
    value["sheets"][1]["join"]["normalize"]["lastPlaces"] = json!(0);
    assert!(
        ImportPlan::from_json(&value.to_string()).is_err(),
        "no places is no fold"
    );
}

#[test]
fn a_plan_with_no_series_is_refused() {
    let set_alone = json!({ "kind": "set", "id": "p", "set": { "id": 4 }, "levels": [] });
    for levels in [json!([]), json!([set_alone])] {
        let error = refused(|plan| plan["structure"]["levels"] = levels.clone());
        assert!(matches!(error, PlanError::NoSeries), "{levels}: {error:?}");
    }
}

#[test]
fn a_file_goes_to_the_series_whose_folder_is_the_longest_prefix() {
    let levels = json!([
        series("all", json!("all")),
        series("letters", folder("s1", "Letters")),
        series("drafts", folder("s1", "Letters/Drafts")),
        series("staged", folder("staging", "")),
    ]);
    let mut value: Value = serde_json::from_str(ARCHIVE_AS_FOUND).expect("json");
    value["structure"]["levels"] = levels;
    let plan = parse(&value.to_string());
    let structure = &plan.structure;
    let place = |source: &str, path: &str| {
        let path = RelPath::parse(path).expect("path");
        match structure.place(&SourceId(source.into()), &path) {
            Placement::Series { at, path } => {
                Some((structure.series(at).id().0.clone(), path.to_string()))
            }
            Placement::Outside => None,
        }
    };
    let placed = |series: &str, path: &str| Some((series.to_string(), path.to_string()));
    assert_eq!(
        place("s1", "Letters/1901/a.jpg"),
        placed("letters", "1901/a.jpg"),
        "the pattern matches the path relative to the scope folder"
    );
    assert_eq!(
        place("s1", "Letters/Drafts/b.jpg"),
        placed("drafts", "b.jpg"),
        "the longer folder wins"
    );
    assert_eq!(
        place("s1", "LettersX/c.jpg"),
        placed("all", "LettersX/c.jpg"),
        "a folder is a prefix of whole names only; `all` takes the rest"
    );
    assert_eq!(
        place("staging", "list.csv"),
        placed("staged", "list.csv"),
        "the whole source is a longer prefix than `all`"
    );

    let mut value: Value = serde_json::from_str(ARCHIVE_AS_FOUND).expect("json");
    value["structure"]["levels"] = json!([series("letters", folder("s1", "Letters"))]);
    let plan = parse(&value.to_string());
    let path = RelPath::parse("Extras/d.jpg").expect("path");
    assert_eq!(
        plan.structure.place(&SourceId("s1".into()), &path),
        Placement::Outside,
        "a file in no scope is outside the structure"
    );
}

#[test]
fn a_path_left_out_is_outside_whatever_scope_holds_it() {
    let mut value: Value = serde_json::from_str(ARCHIVE_AS_FOUND).expect("json");
    value["structure"]["levels"] = json!([series("letters", folder("s1", "Letters"))]);
    value["structure"]["outside"] = json!([
        { "source": "s1", "folder": "Letters/~lock.docx" },
        { "source": "s1", "folder": "Letters/Drafts" }
    ]);
    let plan = parse(&value.to_string());
    let place = |path: &str| {
        let path = RelPath::parse(path).expect("path");
        plan.structure.place(&SourceId("s1".into()), &path) == Placement::Outside
    };
    assert!(place("Letters/~lock.docx"), "one file");
    assert!(place("Letters/Drafts/b.jpg"), "a folder below the scope");
    assert!(!place("Letters/Drafts2/c.jpg"), "a whole name only");
    assert!(!place("Letters/a.jpg"));

    let written: Value = serde_json::from_str(&plan.to_json()).expect("json");
    assert_eq!(
        written["structure"]["outside"],
        value["structure"]["outside"]
    );
    let none: Value = serde_json::from_str(&parse(ARCHIVE_AS_FOUND).to_json()).expect("json");
    assert!(
        none["structure"].get("outside").is_none(),
        "left out of the wire when empty, so every fixture keeps its bytes"
    );

    let error = refused(|plan| {
        plan["structure"]["outside"] = json!([{ "source": "s9", "folder": "a.jpg" }]);
    });
    assert!(
        matches!(&error, PlanError::OutsideSource(missing) if missing == "s9"),
        "{error:?}"
    );
}

/// One edit of each kind, as the admin sends them.
fn every_edit() -> Value {
    let scope = json!({ "source": "s1", "folder": "Artifacts" });
    let list = json!({ "source": "s1", "file": "BHC006_GMT_AF.xlsx" });
    let column = json!({ "index": 0, "header": "Identifier" });
    let from = json!({ "from": "namePlace", "scope": scope, "place": 2 });
    json!([
        { "edit": "leaveOut", "scope": { "source": "s1", "folder": "Extras" } },
        { "edit": "keep", "scope": scope },
        { "edit": "items", "scope": scope, "cut": { "cut": "places", "places": 5 } },
        { "edit": "items", "scope": scope, "cut": { "cut": "whole" } },
        { "edit": "items", "scope": scope, "cut": { "cut": "folder", "depth": 1 } },
        { "edit": "list", "scope": scope, "list": list },
        { "edit": "list", "scope": scope, "list": null },
        { "edit": "grain", "list": list, "describes": "media" },
        { "edit": "addSet", "from": from },
        { "edit": "dropSet", "from": { "from": "folder", "scope": scope } },
        { "edit": "addSet", "from": { "from": "column", "list": list, "column": column } },
        { "edit": "addSet", "from": { "from": "folderLevel", "scope": scope, "depth": 1 } },
        { "edit": "pair", "list": list, "rule": { "rule": "lastPlaces", "places": 2 } },
        { "edit": "pair", "list": list, "rule": { "rule": "column", "column": column, "on": "fileName" } },
        { "edit": "pair", "list": list, "rule": { "rule": "folds",
            "normalize": { "caseFold": true, "leadingZeros": false, "separators": true } } },
        { "edit": "title", "from": from, "title": "Artifacts" }
    ])
}

#[test]
fn edits_are_kept_in_order_and_left_out_when_there_are_none() {
    let mut value: Value = serde_json::from_str(BHC006).expect("json");
    assert!(value.get("edits").is_none());
    assert!(parse(BHC006).edits.is_empty());
    value["edits"] = every_edit();
    let plan = parse(&value.to_string());
    assert_eq!(plan.edits.len(), 16);
    assert_eq!(
        plan.to_json(),
        value.to_string(),
        "written back byte for byte"
    );
    assert_eq!(
        plan.edits[0],
        Edit::LeaveOut {
            scope: FolderScope {
                source: SourceId("s1".into()),
                folder: RelPath::parse("Extras").expect("path"),
            }
        }
    );
}

#[test]
fn an_edit_the_types_do_not_define_is_refused() {
    let bad = [
        json!({ "edit": "leaveOut", "scope": { "source": "s1", "folder": "" }, "extra": 1 }),
        json!({ "edit": "rename", "scope": { "source": "s1", "folder": "" } }),
        json!({ "edit": "list", "scope": { "source": "s1", "folder": "" } }),
        json!({ "edit": "items", "scope": { "source": "s1", "folder": "" }, "cut": { "cut": "places" } }),
        json!({ "edit": "leaveOut", "scope": { "source": "s1", "folder": "../up" } }),
    ];
    for edit in bad {
        let error = refused(|plan| plan["edits"] = json!([edit.clone()]));
        assert!(matches!(error, PlanError::ShapeV3(_)), "{edit}: {error:?}");
    }
}

#[test]
fn a_v2_plan_with_edits_is_written_as_v3_and_v2_has_no_member_for_them() {
    let mut plan = parse(DEFAULT);
    plan.edits = serde_json::from_value(every_edit()).expect("edits");
    let written: Value = serde_json::from_str(&plan.to_json()).expect("json");
    assert_eq!(written["version"], 3, "the lower refuses edits");
    assert_eq!(written["edits"], every_edit());

    let mut v2: Value = serde_json::from_str(DEFAULT).expect("json");
    v2["edits"] = json!([]);
    assert!(matches!(
        ImportPlan::from_json(&v2.to_string()),
        Err(PlanError::Shape(_))
    ));
}
