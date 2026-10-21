//! The `many` and `pair` decisions (`docs/import-v2.md`, "Decisions" and
//! "What the admin screens need at scale").

use serde_json::{Value as Json, json};

use super::support::*;
use crate::import::decisions::EntryRef;
use crate::import::derive::Sheets;
use crate::import::model::{Anomaly, Fate, LeftBy, OrphanReason};
use crate::import::plan::{FlagKind, RecordKind};

fn file(path: &str) -> Json {
    json!({ "file": { "source": "s1", "path": path } })
}

fn files(paths: &[&str]) -> Json {
    paths.iter().map(|path| file(path)).collect()
}

fn title_change(target: &str, text: &str) -> Json {
    json!({ "kind": "set", "target": target, "slot": "dcterms:title",
            "values": [{ "type": "literal", "text": text, "lang": null, "label": null }] })
}

fn title(entry: Json, target: &str, text: &str) -> Json {
    let mut decision = title_change(target, text);
    decision["entry"] = entry;
    decision
}

fn many(entries: Json, change: Json) -> Json {
    json!({ "kind": "many", "entries": entries, "change": change })
}

#[test]
fn a_many_change_applies_at_its_place_in_the_list_like_one_decision_per_entry() {
    let index = index(&[
        ("A1.jpg", JPEG),
        ("A1_b.jpg", JPEG),
        ("B2.jpg", JPEG),
        ("C3.jpg", JPEG),
        ("D4.jpg", JPEG),
        ("E5.jpg", JPEG),
        ("F6.jpg", JPEG),
    ]);
    let plan = plan(|plan| {
        plan["grouping"]["pattern"] = json!(PATTERN);
        plan["decisions"] = json!([
            many(files(&["A1.jpg", "A1_b.jpg", "B2.jpg"]), title_change("item", "Many")),
            title(file("B2.jpg"), "item", "One"),
            title(file("C3.jpg"), "item", "Single"),
            many(files(&["C3.jpg"]), title_change("item", "Later")),
            many(files(&["D4.jpg"]), json!({ "kind": "leave" })),
            many(files(&["E5.jpg", "F6.jpg"]), json!({ "kind": "move", "to": { "with": file("A1.jpg") } })),
            { "kind": "move", "file": file("F6.jpg"), "to": "own" }
        ]);
    });
    let model = run(&index, &Sheets::new(), &plan);
    assert_eq!(texts(item(&model, "A1"), "dcterms:title"), ["Many"]);
    assert_eq!(
        texts(item(&model, "B2"), "dcterms:title"),
        ["One"],
        "a later single wins"
    );
    assert_eq!(
        texts(item(&model, "C3"), "dcterms:title"),
        ["Later"],
        "a later many wins"
    );
    let d4 = model
        .records
        .iter()
        .find(|record| {
            record.kind == RecordKind::Item
                && record.files.iter().any(|f| f.path.as_str() == "D4.jpg")
        })
        .expect("D4");
    assert_eq!(d4.fate, Fate::Left(LeftBy::Decision(4)));
    assert_eq!(
        media_paths(&model, item(&model, "A1")),
        ["A1.jpg", "A1_b.jpg", "E5.jpg"],
        "E5 moved in; F6's later move took it out again"
    );
    assert!(
        model.kept().any(
            |record| record.files.iter().any(|f| f.path.as_str() == "F6.jpg")
                && record.kind == RecordKind::Item
                && record.media.len() == 1
        ),
        "F6 has its own group"
    );
    assert!(
        !model
            .anomalies
            .iter()
            .any(|anomaly| matches!(anomaly, Anomaly::DecisionOverlap { .. })),
        "one many on two entries of one record is no overlap: {:?}",
        model.anomalies
    );
    assert!(model.orphans.is_empty(), "{:?}", model.orphans);
}

#[test]
fn a_many_change_survives_a_new_pattern_and_orphans_each_lost_entry_on_its_own() {
    let index = index(&[("A1.jpg", JPEG), ("B2.jpg", JPEG), ("C3.tif", TIFF)]);
    let derive_with = |pattern: &str| {
        let plan = plan(|plan| {
            plan["grouping"]["pattern"] = json!(pattern);
            plan["decisions"] = json!([many(
                files(&["A1.jpg", "gone.jpg", "C3.tif", "B2.jpg"]),
                title_change("media", "Scan")
            )]);
        });
        run(&index, &Sheets::new(), &plan)
    };
    let orphans = |model: &crate::import::model::Model| -> Vec<(usize, String, OrphanReason)> {
        model
            .orphans
            .iter()
            .map(|orphan| {
                let EntryRef::File(file) = &orphan.entry else {
                    panic!("a file entry");
                };
                (orphan.decision, file.path.to_string(), orphan.reason)
            })
            .collect()
    };
    let media_titles = |model: &crate::import::model::Model| -> Vec<String> {
        kept(model, RecordKind::Media)
            .iter()
            .map(|record| {
                format!(
                    "{} {:?}",
                    record.files[0].path,
                    texts(record, "dcterms:title")
                )
            })
            .collect()
    };

    let before = derive_with(r"^(?<identifier>[^.]+)\.(?:jpg|tif)$");
    assert_eq!(
        media_titles(&before),
        [
            "A1.jpg [\"Scan\"]",
            "B2.jpg [\"Scan\"]",
            "C3.tif [\"Scan\"]"
        ]
    );
    assert_eq!(
        orphans(&before),
        [(0, "gone.jpg".to_string(), OrphanReason::MissingEntry)]
    );

    let after = derive_with(r"^(?<identifier>[^.]+)\.jpg$");
    assert_eq!(
        media_titles(&after),
        ["A1.jpg [\"Scan\"]", "B2.jpg [\"Scan\"]"]
    );
    assert_eq!(
        orphans(&after),
        [
            (0, "gone.jpg".to_string(), OrphanReason::MissingEntry),
            (0, "C3.tif".to_string(), OrphanReason::WrongTarget),
        ],
        "in the many's entry order, one orphan per lost entry"
    );
}

fn photo_plan(decisions: Json) -> crate::import::plan::ImportPlan {
    plan(|plan| {
        plan["grouping"]["pattern"] = json!(r"^(?<identifier>[^/]+)\.jpg$");
        plan["sheets"] = json!([sheet_plan("list", &["File", "Title"])]);
        plan["sheets"][0]["join"] = join_on("fileName", 0, "File", false, false, false);
        plan["slots"]["item"]["dcterms:title"] = rule(json!([col("list", 1, "Title")]), "first");
        plan["decisions"] = decisions;
    })
}

const PHOTOS: &[(&str, &str)] = &[("weding photo 1952.jpg", JPEG), ("harbour.jpg", JPEG)];

fn pair(sheet: &str, key: &str, path: &str) -> Json {
    json!({ "kind": "pair", "row": { "sheet": sheet, "key": key },
            "file": { "source": "s1", "path": path } })
}

#[test]
fn a_pair_joins_a_row_to_the_file_its_cell_misspells() {
    let csv: &[u8] = b"File,Title\nwedding photo 1952.jpg,Wedding\nharbour.jpg,Harbour\n";
    let unpaired = photo_plan(json!([]));
    let model = run(
        &index(PHOTOS),
        &read_sheets(&unpaired, &[("list", csv)]),
        &unpaired,
    );
    assert!(
        texts(item(&model, "weding photo 1952"), "dcterms:title").is_empty(),
        "the join rule misses the misspelt file"
    );

    let paired = photo_plan(json!([pair(
        "list",
        "wedding photo 1952.jpg#1",
        "weding photo 1952.jpg"
    )]));
    let model = run(
        &index(PHOTOS),
        &read_sheets(&paired, &[("list", csv)]),
        &paired,
    );
    assert_eq!(
        texts(item(&model, "weding photo 1952"), "dcterms:title"),
        ["Wedding"]
    );
    assert_eq!(texts(item(&model, "harbour"), "dcterms:title"), ["Harbour"]);
    assert_eq!(
        kept(&model, RecordKind::Item).len(),
        2,
        "no item without a file is left"
    );
    assert!(flags(&model, FlagKind::NoFile).is_empty());
    assert!(model.orphans.is_empty(), "{:?}", model.orphans);
}

#[test]
fn a_pair_wins_over_the_join_rule_for_its_file() {
    let csv: &[u8] =
        b"File,Title\nweding photo 1952.jpg,By the rule\nwedding photo 1952.jpg,By hand\n";
    let plan = photo_plan(json!([pair(
        "list",
        "wedding photo 1952.jpg#1",
        "weding photo 1952.jpg"
    )]));
    let model = run(&index(PHOTOS), &read_sheets(&plan, &[("list", csv)]), &plan);
    assert_eq!(
        texts(item(&model, "weding photo 1952"), "dcterms:title"),
        ["By hand"]
    );
    assert_eq!(
        flags(&model, FlagKind::DuplicateRow).len(),
        1,
        "the rule's row is the duplicate"
    );
}

#[test]
fn a_pair_whose_row_or_file_is_missing_is_an_orphan_that_names_it() {
    let csv: &[u8] = b"File,Title\nharbour.jpg,Harbour\n";
    let plan = photo_plan(json!([
        pair("nosuch", "x#1", "harbour.jpg"),
        pair("list", "gone#1", "harbour.jpg"),
        pair("list", "harbour.jpg#1", "gone.jpg")
    ]));
    let model = run(&index(PHOTOS), &read_sheets(&plan, &[("list", csv)]), &plan);
    let named: Vec<(usize, String, OrphanReason)> = model
        .orphans
        .iter()
        .map(|orphan| {
            let named = match &orphan.entry {
                EntryRef::Row(row) => format!("{}/{}", row.sheet.0, row.key.as_str()),
                EntryRef::File(file) => file.path.to_string(),
            };
            (orphan.decision, named, orphan.reason)
        })
        .collect();
    assert_eq!(
        named,
        [
            (0, "nosuch/x#1".to_string(), OrphanReason::MissingEntry),
            (1, "list/gone#1".to_string(), OrphanReason::MissingEntry),
            (2, "gone.jpg".to_string(), OrphanReason::MissingEntry),
        ]
    );
    assert_eq!(
        texts(item(&model, "harbour"), "dcterms:title"),
        ["Harbour"],
        "the rule still joins"
    );
}

#[test]
fn orphans_from_every_step_are_listed_in_plan_order() {
    let plan = photo_plan(json!([
        title(file("gone.jpg"), "item", "Lost"),
        pair("list", "gone#1", "harbour.jpg"),
        { "kind": "move", "file": file("gone.jpg"), "to": "own" }
    ]));
    let csv: &[u8] = b"File,Title\nharbour.jpg,Harbour\n";
    let model = run(&index(PHOTOS), &read_sheets(&plan, &[("list", csv)]), &plan);
    let decisions: Vec<usize> = model.orphans.iter().map(|orphan| orphan.decision).collect();
    assert_eq!(
        decisions,
        [0, 1, 2],
        "the slot step, the join and the grouping each found one"
    );
}
