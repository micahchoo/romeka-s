//! What a `sets` value names, by the shape of its set source: a title, an
//! id, a row key, or v2's text; and the order of an item's memberships.

#![allow(clippy::expect_used)]

use serde_json::{Value as Json, json};

use crate::import::catalog::{Catalog, ExistingSet};
use crate::import::derive::tests::support::{
    JPEG, catalog, col, constant, flags, index, join_on, kept, plan, read_sheets, rule, sheet_plan,
};
use crate::import::derive::{NoTags, Sheets, derive};
use crate::import::model::{Fate, LeftBy, Model, Record, SetRef};
use crate::import::plan::{FlagKind, FlagPolicy, ImportPlan, RecordKind};

const BORN_DIGITAL: &str = include_str!("../fixtures/v3/born_digital.json");

/// The install's sets: #2018 "Annual report", #7 and #8 both "Letters".
fn install() -> Catalog {
    let set = |id, title: &str| ExistingSet {
        id,
        title: title.into(),
        ..ExistingSet::default()
    };
    Catalog {
        item_sets: vec![
            set(2018, "Annual report"),
            set(7, "Letters"),
            set(8, "Letters"),
        ],
        ..catalog()
    }
}

/// The born-digital plan (one series over everything) with `edit` applied
/// to its JSON.
fn v3(edit: impl FnOnce(&mut Json)) -> ImportPlan {
    let mut value: Json = serde_json::from_str(BORN_DIGITAL).expect("fixture json");
    edit(&mut value);
    ImportPlan::from_json(&value.to_string()).unwrap_or_else(|error| panic!("{error}: {value}"))
}

fn one_item(plan: &ImportPlan) -> (Model, Record) {
    let model = derive(
        &index(&[("A1.jpg", JPEG)]),
        &Sheets::new(),
        plan,
        &install(),
        &NoTags,
    );
    let items = kept(&model, RecordKind::Item);
    let item = (*items.first().expect("the item")).clone();
    (model, item)
}

fn new_titles(model: &Model) -> Vec<&str> {
    model
        .new_sets
        .iter()
        .map(|set| set.title.as_str())
        .collect()
}

#[test]
fn a_title_of_digits_makes_a_new_set() {
    let plan = v3(|plan| {
        plan["structure"]["freeSets"] =
            json!([{ "title": "2018", "description": "Shown in 2018" }]);
    });
    let (model, item) = one_item(&plan);
    assert_eq!(item.resolved.sets, [SetRef::New(0)], "not set #2018");
    assert_eq!(new_titles(&model), ["2018"]);
    assert_eq!(
        model.new_sets[0].description.as_deref(),
        Some("Shown in 2018"),
        "a new set gets its source's description"
    );
}

#[test]
fn a_title_an_existing_set_has_is_that_set_and_keeps_its_description() {
    let plan = v3(|plan| {
        plan["structure"]["freeSets"] =
            json!([{ "title": " annual REPORT ", "description": "Not written" }]);
    });
    let (model, item) = one_item(&plan);
    assert_eq!(item.resolved.sets, [SetRef::Existing(2018)]);
    assert!(model.new_sets.is_empty());
}

#[test]
fn a_title_two_sets_have_flags_many_matches() {
    let plan = v3(|plan| {
        plan["structure"]["freeSets"] = json!([{ "title": "Letters", "description": null }]);
    });
    let model = derive(
        &index(&[("A1.jpg", JPEG)]),
        &Sheets::new(),
        &plan,
        &install(),
        &NoTags,
    );
    let raised = flags(&model, FlagKind::ManyMatches);
    assert_eq!(raised.len(), 1);
    assert_eq!(raised[0].policy, Some(FlagPolicy::Leave));
    let item = model
        .records
        .iter()
        .find(|record| record.kind == RecordKind::Item)
        .expect("the item");
    assert_eq!(item.fate, Fate::Left(LeftBy::Flag(FlagKind::ManyMatches)));
}

#[test]
fn an_id_naming_no_set_flags_unknown_set_and_keeps_the_item_without_it() {
    let plan = v3(|plan| {
        plan["structure"]["freeSets"] = json!([{ "id": 404 }, { "id": 2018 }]);
    });
    let (model, item) = one_item(&plan);
    assert_eq!(item.resolved.sets, [SetRef::Existing(2018)]);
    let raised = flags(&model, FlagKind::UnknownSet);
    assert_eq!(raised.len(), 1);
    assert_eq!(raised[0].policy, Some(FlagPolicy::Keep));
    assert_eq!(raised[0].detail.as_deref(), Some("404"));
    assert!(model.new_sets.is_empty(), "an id never makes a set");

    let leave = v3(|plan| {
        plan["structure"]["freeSets"] = json!([{ "id": 404 }]);
        plan["flags"] = json!({ "unknown-set": "leave" });
    });
    let model = derive(
        &index(&[("A1.jpg", JPEG)]),
        &Sheets::new(),
        &leave,
        &install(),
        &NoTags,
    );
    assert!(
        kept(&model, RecordKind::Item).is_empty(),
        "leave leaves the item out"
    );
}

#[test]
fn each_value_reads_as_its_names_say() {
    let each = |value: &str, names: &str| json!({ "each": { "sources": [constant(value)], "combine": "first", "names": names } });
    let plan = v3(|plan| {
        plan["structure"]["freeSets"] = json!([
            each("2018", "title"),
            each("7", "id"),
            each("Annual report", "text"),
            each("99", "id"),
            each("nobody", "key"),
        ]);
    });
    let (model, item) = one_item(&plan);
    assert_eq!(
        item.resolved.sets,
        [SetRef::New(0), SetRef::Existing(7), SetRef::Existing(2018)]
    );
    assert_eq!(new_titles(&model), ["2018"]);
    let unknown: Vec<_> = flags(&model, FlagKind::UnknownSet)
        .iter()
        .map(|flag| flag.detail.clone())
        .collect();
    assert_eq!(unknown, [Some("99".into()), Some("nobody".into())]);
}

#[test]
fn memberships_go_set_levels_top_first_then_the_series_then_the_free_sets_once_each() {
    let plan = v3(|plan| {
        let mut series = plan["structure"]["levels"][0].clone();
        series["set"] = json!({ "title": "Drive", "description": null });
        plan["structure"]["levels"] = json!([{
            "kind": "set", "id": "top", "set": { "title": "Project", "description": null },
            "levels": [{
                "kind": "set", "id": "middle", "set": { "id": 2018 },
                "levels": [series]
            }]
        }]);
        plan["structure"]["freeSets"] = json!([
            { "title": "Exhibition", "description": null },
            { "title": "Project", "description": null },
            { "each": { "sources": [constant("drive")], "combine": "first", "names": "title" } }
        ]);
    });
    let (model, item) = one_item(&plan);
    assert_eq!(new_titles(&model), ["Project", "Drive", "Exhibition"]);
    assert_eq!(
        item.resolved.sets,
        [
            SetRef::New(0),
            SetRef::Existing(2018),
            SetRef::New(1),
            SetRef::New(2)
        ],
        "a source given twice, and a value naming a set already in, count once"
    );
}

#[test]
fn a_free_set_reads_a_list_of_one_series_and_gives_nothing_to_another() {
    let name = r"^(?<identifier>[^/_]+)\.[^/.]+$";
    let series = |id: &str, folder: &str, lists: Json| {
        json!({
            "kind": "series", "id": id, "set": null,
            "scope": [{ "source": "s1", "folder": folder }],
            "grouping": { "pattern": name, "caseFold": false },
            "example": null, "lists": lists,
            "slots": { "item": {}, "media": {}, "itemSet": {} }
        })
    };
    let mut register = sheet_plan("register", &["Code", "Box"]);
    register["join"] = join_on("identifier", 0, "Code", false, false, false);
    let plan = v3(|plan| {
        plan["structure"]["levels"] = json!([
            series("letters", "Letters", json!([register])),
            series("photos", "Photos", json!([])),
        ]);
        plan["structure"]["freeSets"] = json!([{ "each": {
            "sources": [col("register", 1, "Box")], "combine": "first", "names": "title"
        } }]);
    });
    let sheets = read_sheets(&plan, &[("register", b"Code,Box\nA1,Box 1\n")]);
    let files = [("Letters/A1.jpg", JPEG), ("Photos/A1.jpg", JPEG)];
    let model = derive(&index(&files), &sheets, &plan, &install(), &NoTags);
    let sets_of = |key: &str| {
        let item = model
            .records
            .iter()
            .find(|record| record.origin.key() == key)
            .unwrap_or_else(|| panic!("no item {key}"));
        item.resolved.sets.clone()
    };
    assert_eq!(sets_of("group:letters/A1"), [SetRef::New(0)]);
    assert_eq!(sets_of("group:photos/A1"), [], "no row of the list, no set");
    assert_eq!(new_titles(&model), ["Box 1"]);
}

/// A v2 plan with one `set` decision on the item's `sets`.
fn decided(value: Json) -> ImportPlan {
    plan(|plan| {
        plan["decisions"] = json!([{
            "kind": "set", "entry": { "file": { "source": "s1", "path": "A1.jpg" } },
            "target": "item", "slot": "sets", "values": [value]
        }]);
    })
}

#[test]
fn a_literal_set_decision_reads_as_a_title_and_an_internal_id_as_an_id() {
    let literal =
        decided(json!({ "type": "literal", "text": "2018", "lang": null, "label": null }));
    let (model, item) = one_item(&literal);
    assert_eq!(item.resolved.sets, [SetRef::New(0)], "not set #2018");
    assert_eq!(new_titles(&model), ["2018"]);

    let by_id = decided(json!({
        "type": { "resource": "internalId" }, "text": "2018", "lang": null, "label": null
    }));
    let (model, item) = one_item(&by_id);
    assert_eq!(item.resolved.sets, [SetRef::Existing(2018)]);
    assert!(model.new_sets.is_empty());
}

#[test]
fn a_v2_sets_rule_keeps_its_reading_an_id_then_a_key_then_a_title() {
    let plan = plan(|plan| {
        plan["slots"]["item"]["sets"] = rule(json!([constant("2018")]), "first");
    });
    let (model, item) = one_item(&plan);
    assert_eq!(item.resolved.sets, [SetRef::Existing(2018)]);
    assert!(model.new_sets.is_empty());
}
