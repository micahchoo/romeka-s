//! Step 6: matching per kind with scopes, links, and flag decisions.

use serde_json::json;

use super::support::*;
use crate::import2::catalog::{Catalog, ExistingItem, ExistingMedia, ExistingSet, Keys};
use crate::import2::derive::{NoTags, Sheets, derive};
use crate::import2::model::{Fate, LeftBy, Link, Model};
use crate::import2::plan::{FlagKind, FlagPolicy, RecordKind};
use crate::import2::values::ValueType;

fn keys(identifier: &str) -> Keys {
    [(term("dcterms:identifier"), vec![identifier.to_string()])].into()
}

/// Item 10 `A1` in set 3, item 11 `A1` in set 4, item 12 `B2` in set 3,
/// item 13 `C3` in no set; media 20 of item 10 keyed `A1.jpg`.
fn install() -> Catalog {
    let item = |id, key: &str, sets: Vec<i64>| ExistingItem {
        id,
        keys: keys(key),
        sets,
    };
    let mut catalog = catalog();
    catalog.items = vec![
        item(10, "A1", vec![3]),
        item(11, "A1", vec![4]),
        item(12, "B2", vec![3]),
        item(13, "C3", vec![]),
    ];
    catalog.media = vec![ExistingMedia {
        id: 20,
        keys: keys("A1.jpg"),
        item: 10,
    }];
    catalog.item_sets = vec![
        ExistingSet {
            id: 3,
            title: "Photographs".into(),
            keys: keys("PH"),
        },
        ExistingSet {
            id: 4,
            title: "Letters".into(),
            keys: keys("LT"),
        },
    ];
    catalog
}

fn derive_on(
    action: &str,
    scope: serde_json::Value,
    unmatched: &str,
    decisions: serde_json::Value,
) -> Model {
    let files = [
        ("A1.jpg", JPEG),
        ("B2.jpg", JPEG),
        ("C3.jpg", JPEG),
        ("D4.jpg", JPEG),
    ];
    let plan = plan(|plan| {
        plan["grouping"]["pattern"] = json!(PATTERN);
        plan["action"]["action"] = json!(action);
        plan["action"]["unmatched"] = json!(unmatched);
        plan["action"]["match"]["item"]["scope"] = scope;
        plan["slots"]["item"]["sets"] = rule(json!([constant("Photographs")]), "first");
        plan["decisions"] = decisions;
    });
    derive(&index(&files), &Sheets::new(), &plan, &install(), &NoTags)
}

fn fate(model: &Model, key: &str) -> Fate {
    model
        .records
        .iter()
        .find(|record| {
            record.kind == RecordKind::Item && record.resolved.item_key.as_deref() == Some(key)
        })
        .map(|record| record.fate)
        .unwrap_or_else(|| panic!("no {key}"))
}

#[test]
fn a72_on_create_a_key_in_scope_flags_exists_and_one_outside_scope_is_an_anomaly() {
    let model = derive_on("create", json!("importSets"), "leave", json!([]));
    assert_eq!(
        fate(&model, "A1"),
        Fate::Left(LeftBy::Flag(FlagKind::Exists)),
        "item 10 is in set 3"
    );
    assert_eq!(
        fate(&model, "B2"),
        Fate::Left(LeftBy::Flag(FlagKind::Exists))
    );
    assert_eq!(
        fate(&model, "C3"),
        Fate::Create,
        "item 13 is in no set this import names"
    );
    assert_eq!(fate(&model, "D4"), Fate::Create);
    let exists = flags(&model, FlagKind::Exists);
    assert_eq!(exists.len(), 2);
    assert!(
        exists
            .iter()
            .all(|flag| flag.policy == Some(FlagPolicy::Leave))
    );
    let outside: Vec<String> = model
        .anomalies
        .iter()
        .filter_map(|anomaly| match anomaly {
            crate::import2::model::Anomaly::OutsideScope { key, ids, .. } => {
                Some(format!("{key}{ids:?}"))
            }
            _ => None,
        })
        .collect();
    assert_eq!(outside, ["C3[13]"]);
    let everywhere = derive_on("create", json!("all"), "leave", json!([]));
    assert_eq!(
        fate(&everywhere, "C3"),
        Fate::Left(LeftBy::Flag(FlagKind::Exists))
    );
}

#[test]
fn on_other_actions_one_match_acts_none_is_unmatched_and_several_flag_many_matches() {
    let model = derive_on("revise", json!("all"), "leave", json!([]));
    assert_eq!(fate(&model, "B2"), Fate::Act(12));
    assert_eq!(
        fate(&model, "A1"),
        Fate::Left(LeftBy::Flag(FlagKind::ManyMatches)),
        "items 10 and 11"
    );
    assert_eq!(
        fate(&model, "D4"),
        Fate::Left(LeftBy::Flag(FlagKind::Unmatched))
    );
    assert_eq!(
        flags(&model, FlagKind::Unmatched)[0].policy,
        Some(FlagPolicy::Leave),
        "action.unmatched"
    );
    let create = derive_on("update", json!("all"), "create", json!([]));
    assert_eq!(fate(&create, "D4"), Fate::Create);
    let scoped = derive_on("update", json!({ "sets": [4] }), "leave", json!([]));
    assert_eq!(
        fate(&scoped, "A1"),
        Fate::Act(11),
        "only item 11 is in set 4"
    );
}

#[test]
fn a_match_decision_chooses_among_many() {
    let decisions = json!([
        { "kind": "match", "entry": { "file": { "source": "s1", "path": "A1.jpg" } }, "target": "item", "resource": 11 },
        { "kind": "match", "entry": { "file": { "source": "s1", "path": "B2.jpg" } }, "target": "item", "resource": null },
        { "kind": "match", "entry": { "file": { "source": "s1", "path": "A1.jpg" } }, "target": "itemSet", "resource": 3 }
    ]);
    let model = derive_on("append", json!("all"), "create", decisions);
    assert_eq!(fate(&model, "A1"), Fate::Act(11));
    assert_eq!(
        fate(&model, "B2"),
        Fate::Create,
        "none chosen: unmatched, created"
    );
    assert_eq!(model.orphans.len(), 1, "no item-set record holds A1.jpg");
    assert_eq!(
        model.orphans[0].reason,
        crate::import2::model::OrphanReason::WrongTarget
    );
}

#[test]
fn media_in_import_sets_scope_match_only_the_media_of_the_matched_item() {
    let files = [("A1.jpg", JPEG)];
    let plan = plan(|plan| {
        plan["grouping"]["pattern"] = json!(PATTERN);
        plan["action"]["action"] = json!("append");
        plan["action"]["unmatched"] = json!("create");
        plan["action"]["match"]["item"] = json!({ "key": "internalId", "scope": "all" });
        plan["slots"]["item"]["itemKey"] = rule(json!([constant("10")]), "first");
        plan["slots"]["media"]["dcterms:identifier"] =
            rule(json!([{ "from": "file", "fact": "name" }]), "first");
    });
    let model = derive(&index(&files), &Sheets::new(), &plan, &install(), &NoTags);
    let item = model
        .kept()
        .find(|record| record.kind == RecordKind::Item)
        .expect("item");
    assert_eq!(item.fate, Fate::Act(10), "internalId reads the item key");
    let media = model.record(item.media[0]);
    assert_eq!(media.fate, Fate::Act(20), "media 20 belongs to item 10");

    let mut moved = install();
    moved.media[0].item = 12;
    let model = derive(&index(&files), &Sheets::new(), &plan, &moved, &NoTags);
    let media = model
        .records
        .iter()
        .find(|record| record.kind == RecordKind::Media)
        .expect("media");
    assert_eq!(
        media.fate,
        Fate::Create,
        "media of another item is out of scope; unmatched creates"
    );
}

#[test]
fn a52_resource_values_link_to_the_install_then_to_this_imports_records_else_link_missing() {
    let plan = plan(|plan| {
        plan["sheets"] = json!([sheet_plan("r", &["Id", "Part of"])]);
        plan["sheets"][0]["itemKey"] = json!({ "index": 0, "header": "Id" });
        plan["sheets"][0]["columns"][1]["options"]["type"] = json!({ "resource": "identifier" });
        plan["action"]["match"]["item"]["scope"] = json!("all");
        plan["slots"]["item"]["dcterms:isPartOf"] = rule(json!([col("r", 1, "Part of")]), "first");
    });
    let csv: &[u8] = b"Id,Part of\nN1,B2\nN2,N1\nN3,N9\nN4,N2\n";
    let model = derive(
        &index(&[]),
        &read_sheets(&plan, &[("r", csv)]),
        &plan,
        &install(),
        &NoTags,
    );
    let link = |key: &str| {
        let record = item(&model, key);
        let value = &record.slots
            [&crate::import2::plan::SlotName::parse("dcterms:isPartOf").expect("slot")][0];
        (value.link, value.value.value_type.clone())
    };
    assert_eq!(link("N1").0, Some(Link::Existing(12)));
    let n1 = item(&model, "N1").id;
    assert_eq!(
        link("N2").0,
        Some(Link::Record(n1)),
        "a record of this import, written in pass two"
    );
    assert_eq!(link("N4").0, Some(Link::Record(item(&model, "N2").id)));
    assert_eq!(link("N3"), (None, ValueType::Literal), "kept as a literal");
    assert_eq!(
        flags(&model, FlagKind::LinkMissing)[0].policy,
        Some(FlagPolicy::Keep)
    );
}

#[test]
fn per_entry_flag_decisions_override_the_plan_policy_with_the_latest_and_their_slot() {
    let plan = plan(|plan| {
        plan["sheets"] = json!([sheet_plan("r", &["Id", "Title", "Date"])]);
        plan["sheets"][0]["itemKey"] = json!({ "index": 0, "header": "Id" });
        plan["slots"]["item"]["dcterms:title"] = rule(json!([col("r", 1, "Title")]), "first");
        plan["slots"]["item"]["dcterms:date"] = rule(json!([col("r", 2, "Date")]), "first");
        plan["flags"] = json!({ "date-form": "leave" });
        let row = |key: &str| json!({ "row": { "sheet": "r", "key": key } });
        plan["decisions"] = json!([
            { "kind": "flag", "entry": row("1#1"), "flag": "date-form", "slot": null, "policy": "keep" },
            { "kind": "flag", "entry": row("2#1"), "flag": "date-form", "slot": "dcterms:title", "policy": "keep" },
            { "kind": "flag", "entry": row("3#1"), "flag": "date-form", "slot": null, "policy": "keep" },
            { "kind": "flag", "entry": row("3#1"), "flag": "date-form", "slot": "dcterms:date", "policy": "leave" },
            { "kind": "flag", "entry": row("9#1"), "flag": "date-form", "slot": null, "policy": "keep" }
        ]);
    });
    let csv: &[u8] = b"Id,Title,Date\n1,a,c. 1890\n2,b,c. 1890\n3,c,c. 1890\n";
    let model = derive(
        &index(&[]),
        &read_sheets(&plan, &[("r", csv)]),
        &plan,
        &catalog(),
        &NoTags,
    );
    assert_eq!(
        texts(item(&model, "1"), "dcterms:date"),
        ["c. 1890"],
        "the decision keeps it"
    );
    assert!(
        texts(item(&model, "2"), "dcterms:date").is_empty(),
        "a decision on another slot does not apply"
    );
    assert!(
        texts(item(&model, "3"), "dcterms:date").is_empty(),
        "the later decision wins"
    );
    let policies: Vec<Option<FlagPolicy>> = flags(&model, FlagKind::DateForm)
        .iter()
        .map(|flag| flag.policy)
        .collect();
    assert_eq!(
        policies,
        [
            Some(FlagPolicy::Keep),
            Some(FlagPolicy::Leave),
            Some(FlagPolicy::Leave)
        ]
    );
    assert_eq!(model.orphans.len(), 1, "row 9 does not exist");
}
