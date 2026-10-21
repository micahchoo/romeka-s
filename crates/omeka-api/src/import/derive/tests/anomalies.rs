//! Step 7: anomalies, orphans, every flag kind, and the digest.

use std::collections::BTreeMap;

use serde_json::json;

use super::support::*;
use crate::import::catalog::{ExistingItem, Keys};
use crate::import::derive::{NoTags, Sheets, derive};
use crate::import::evidence::Anomalies;
use crate::import::model::{Anomaly, Model, OrphanReason};
use crate::import::plan::{FlagKind, FlagPolicy, PolicyDefault, RecordKind};
use crate::import::sheet::SheetError;
use omeka_media::validation::FileValidation;

fn anomaly(model: &Model, pick: impl Fn(&Anomaly) -> bool) -> Vec<&Anomaly> {
    model
        .anomalies
        .iter()
        .filter(|anomaly| pick(anomaly))
        .collect()
}

#[test]
fn a13_a29_records_whose_files_span_folders_are_anomalies() {
    let plan = plan(|plan| plan["grouping"]["pattern"] = json!(PATTERN));
    let model = run(
        &index(&[
            ("Letters/001.jpg", JPEG),
            ("Photos/001.jpg", JPEG),
            ("Photos/002.jpg", JPEG),
        ]),
        &Sheets::new(),
        &plan,
    );
    let span = anomaly(&model, |anomaly| {
        matches!(anomaly, Anomaly::SpanFolders { .. })
    });
    assert_eq!(span.len(), 1);
    assert!(
        matches!(span[0], Anomaly::SpanFolders { folders, .. } if folders == &["s1:Letters", "s1:Photos"])
    );
}

#[test]
fn a15_only_a_group_past_four_times_the_median_and_five_files_is_large() {
    let mut files: Vec<(String, &str)> = (1..=4).map(|n| (format!("I{n}.jpg"), JPEG)).collect();
    files.extend((1..=6).map(|n| (format!("BIG_p{n}.jpg"), JPEG)));
    files.extend((1..=200).map(|n| (format!("DIARY_p{n}.jpg"), JPEG)));
    let listed: Vec<(&str, &str)> = files
        .iter()
        .map(|(path, sniffed)| (path.as_str(), *sniffed))
        .collect();
    let plan = plan(|plan| plan["grouping"]["pattern"] = json!(PATTERN));
    let model = run(&index(&listed), &Sheets::new(), &plan);
    let large: Vec<usize> = model
        .anomalies
        .iter()
        .filter_map(|anomaly| match anomaly {
            Anomaly::LargeGroup { files, median, .. } => Some(*files * 1000 + *median),
            _ => None,
        })
        .collect();
    assert_eq!(
        large,
        [6001, 200_001],
        "median 1: 6 files and 200 files are large"
    );
    let pages = (1..=200)
        .map(|n| (format!("D{}_p{n}.jpg", n % 3), JPEG))
        .collect::<Vec<_>>();
    let listed: Vec<(&str, &str)> = pages
        .iter()
        .map(|(path, sniffed)| (path.as_str(), *sniffed))
        .collect();
    let model = run(&index(&listed), &Sheets::new(), &plan);
    assert!(
        anomaly(&model, |anomaly| matches!(
            anomaly,
            Anomaly::LargeGroup { .. }
        ))
        .is_empty(),
        "three diaries of ~67 pages"
    );
}

#[test]
fn unmapped_columns_header_notes_changed_sheets_and_unread_sheets_are_anomalies() {
    let plan = plan(|plan| {
        plan["sheets"] = json!([
            sheet_plan("r", &["Id"]),
            sheet_plan("gone", &["Id"]),
            sheet_plan("jp", &["Id"])
        ]);
        plan["sheets"][0]["hash"] = json!("0000");
    });
    let mut sheets = read_sheets(&plan, &[("r", b"Id,Note,#private,Note\n1,a,b,c\n")]);
    sheets.insert(
        list(&plan, 2).id.clone(),
        Err(SheetError::EncodingUnsupported("Shift_JIS")),
    );
    let model = run(&index(&[]), &sheets, &plan);
    assert!(
        model.anomalies.iter().any(
            |anomaly| matches!(anomaly, Anomaly::UnmappedColumns { columns, .. }
        if columns == &[(1, "Note".to_string()), (3, "Note".to_string())])
        ),
        "{:?}",
        model.anomalies
    );
    assert!(
        model
            .anomalies
            .iter()
            .any(|anomaly| matches!(anomaly, Anomaly::HeaderNotes { .. }))
    );
    assert!(model.anomalies.iter().any(
        |anomaly| matches!(anomaly, Anomaly::SheetChanged { stored, .. } if stored == "0000")
    ));
    assert!(
        model.anomalies.iter().any(
            |anomaly| matches!(anomaly, Anomaly::SheetUnread { sheet, .. } if sheet.0 == "gone")
        )
    );
    let unsupported = flags(&model, FlagKind::EncodingUnsupported);
    assert_eq!(unsupported[0].detail.as_deref(), Some("Shift_JIS"));
    assert!(model.blocked());
}

#[test]
fn a18_what_the_walk_skipped_and_decoded_is_an_anomaly() {
    let mut index = index(&[("A1.jpg", JPEG)]);
    index.sources[0].walk.anomalies = Anomalies {
        symlinks: 2,
        dot: 3,
        ..Anomalies::default()
    };
    index.sources[0].walk.entries[0].decoded = Some(crate::import::evidence::DecodedName {
        encoding: crate::import::evidence::NameEncoding::Cp437,
        raw: b"A1\x81.jpg".to_vec(),
    });
    let model = run(&index, &Sheets::new(), &plan(|_| {}));
    let walk = anomaly(&model, |anomaly| matches!(anomaly, Anomaly::Walk { .. }));
    assert!(
        matches!(walk[0], Anomaly::Walk { symlinks: 2, dot: 3, decoded, .. } if decoded.len() == 1)
    );
}

#[test]
fn entries_no_record_uses_are_listed_less_those_a_decision_left() {
    let plan = plan(|plan| {
        plan["grouping"]["pattern"] = json!(PATTERN);
        plan["decisions"] = json!([{ "kind": "move", "file": { "file": { "source": "s1", "path": "B2.jpg" } }, "to": "leave" }]);
    });
    let model = run(
        &index(&[("A1.jpg", JPEG), ("B2.jpg", JPEG), ("readme", TEXT)]),
        &Sheets::new(),
        &plan,
    );
    let unused = anomaly(&model, |anomaly| {
        matches!(anomaly, Anomaly::UnusedEntries { .. })
    });
    assert!(
        matches!(unused[0], Anomaly::UnusedEntries { entries } if entries.len() == 1 && format!("{entries:?}").contains("readme"))
    );
}

#[test]
fn a68_decisions_on_entries_of_one_record_setting_one_slot_overlap_and_the_later_wins() {
    let set = |path: &str, text: &str| {
        json!({ "kind": "set", "entry": { "file": { "source": "s1", "path": path } }, "target": "item",
                "slot": "dcterms:title", "values": [{ "type": "literal", "text": text, "lang": null, "label": null }] })
    };
    let plan = plan(|plan| {
        plan["grouping"]["pattern"] = json!(PATTERN);
        plan["decisions"] = json!([
            set("A1_front.jpg", "Front"),
            set("A1_back.jpg", "Back"),
            set("B2.jpg", "B")
        ]);
    });
    let model = run(
        &index(&[
            ("A1_front.jpg", JPEG),
            ("A1_back.jpg", JPEG),
            ("B2.jpg", JPEG),
        ]),
        &Sheets::new(),
        &plan,
    );
    assert_eq!(texts(item(&model, "A1"), "dcterms:title"), ["Back"]);
    let overlap = anomaly(&model, |anomaly| {
        matches!(anomaly, Anomaly::DecisionOverlap { .. })
    });
    assert_eq!(overlap.len(), 1);
    assert!(
        matches!(overlap[0], Anomaly::DecisionOverlap { decisions, .. } if decisions == &[0, 1])
    );
}

#[test]
fn orphans_are_decisions_on_missing_entries_or_entries_in_no_record_of_their_kind() {
    let plan = plan(|plan| {
        plan["grouping"]["pattern"] = json!(PATTERN);
        plan["decisions"] = json!([
            { "kind": "leave", "entry": { "file": { "source": "s9", "path": "A1.jpg" } } },
            { "kind": "set", "entry": { "file": { "source": "s1", "path": "A1.jpg" } }, "target": "itemSet", "slot": "dcterms:title", "values": [] },
            { "kind": "set", "entry": { "file": { "source": "s1", "path": "readme" } }, "target": "media", "slot": "dcterms:title", "values": [] },
            { "kind": "match", "entry": { "row": { "sheet": "none", "key": "1" } }, "target": "item", "resource": 3 },
            { "kind": "set", "entry": { "file": { "source": "s1", "path": "A1.jpg" } }, "target": "media", "slot": "dcterms:title", "values": [] }
        ]);
    });
    let model = run(
        &index(&[("A1.jpg", JPEG), ("readme", TEXT)]),
        &Sheets::new(),
        &plan,
    );
    let reasons: Vec<(usize, OrphanReason)> = model
        .orphans
        .iter()
        .map(|orphan| (orphan.decision, orphan.reason))
        .collect();
    assert_eq!(
        reasons,
        [
            (0, OrphanReason::MissingEntry),
            (1, OrphanReason::WrongTarget),
            (2, OrphanReason::WrongTarget),
            (3, OrphanReason::MissingEntry),
        ],
        "orphans are listed, never dropped; decision 4 applies"
    );
}

/// One derive per row of the contract's table, so each kind is seen.
fn every_kind() -> Vec<Model> {
    let mut catalog = catalog();
    catalog.items = vec![ExistingItem {
        id: 9,
        keys: Keys::from([(term("dcterms:identifier"), vec!["E1".to_string()])]),
        sets: vec![],
    }];
    catalog.files = FileValidation::Allow {
        media_types: [JPEG.to_string(), "image/png".to_string()].into(),
        extensions: ["jpg".to_string(), "png".to_string()].into(),
    };
    catalog.templates.push(crate::import::catalog::Template {
        id: 7,
        label: "Artifact".into(),
        fields: vec![crate::import::catalog::Field {
            term: term("dcterms:title"),
            required: true,
            data_types: vec![],
        }],
    });
    let mut models = Vec::new();
    let headers = ["Id", "Kind", "Title", "Date", "Link", "Vis", "Type"];
    let plan_with = |action: &str| {
        plan(|plan| {
            plan["grouping"]["pattern"] = json!(PATTERN);
            plan["action"]["action"] = json!(action);
            plan["action"]["match"]["item"]["scope"] = json!("all");
            plan["sheets"] = json!([sheet_plan("r", &headers)]);
            plan["sheets"][0]["join"] = join_on("identifier", 0, "Id", false, false, false);
            plan["sheets"][0]["rowKind"] = json!({ "column": { "index": 1, "header": "Kind" } });
            plan["sheets"][0]["columns"][5]["options"]["split"] = json!(";");
            plan["sheets"][0]["columns"][4]["options"]["type"] =
                json!({ "resource": "identifier" });
            for (index, slot) in [
                (2, "dcterms:title"),
                (3, "dcterms:date"),
                (4, "dcterms:relation"),
                (5, "visibility"),
                (6, "template"),
            ] {
                plan["slots"]["item"][slot] =
                    rule(json!([col("r", index, headers[index as usize])]), "first");
            }
            plan["defaults"]["template"] = json!(7);
            plan["newTypes"] = json!([{ "label": "Artifact", "from": "mapping", "class": null }]);
            plan["slots"]["item"]["bibo:x"] = rule(json!([constant("x")]), "first");
            plan["decisions"] = json!([{
                "kind": "set", "entry": { "file": { "source": "s1", "path": "F5.jpg" } },
                "target": "item", "slot": "sets",
                "values": [{ "type": { "resource": "internalId" }, "text": "404", "lang": null, "label": null }]
            }]);
        })
    };
    let csv: &[u8] = b"Id,Kind,Title,Date,Link,Vis,Type\nE1,item,  T ,c. 1890,N9,public;private,Nope\nE1,item,,,,,\nN1,thing,,,,,\nN2,item,Caf\xc3\x83\xc2\xa9,,,,\nN3,item,a,b,c,d,e,f\n";
    let files = [
        ("E1.jpg", JPEG),
        ("E1_x.jpg", "image/png"),
        ("F5.jpg", JPEG),
        ("Z.db", BINARY),
    ];
    let mut index = index(&files);
    index.sources[0].walk.entries[0].decoded = Some(crate::import::evidence::DecodedName {
        encoding: crate::import::evidence::NameEncoding::Latin1,
        raw: b"E1.jpg".to_vec(),
    });
    for action in ["create", "update"] {
        let plan = plan_with(action);
        let sheets = read_sheets(&plan, &[("r", csv)]);
        models.push(derive(&index, &sheets, &plan, &catalog, &NoTags));
    }
    let mut many = catalog.clone();
    many.items.push(ExistingItem {
        id: 8,
        keys: many.items[0].keys.clone(),
        sets: vec![],
    });
    let mut sheets: Sheets = BTreeMap::new();
    let plan = plan_with("update");
    sheets.insert(
        list(&plan, 0).id.clone(),
        Err(SheetError::EncodingUnsupported("ISCII")),
    );
    let mut many_plan = plan.clone();
    many_plan.defaults.template = Some(99);
    models.push(derive(&index, &sheets, &many_plan, &many, &NoTags));
    let listed = super::support::plan(|plan| {
        plan["grouping"]["pattern"] = json!(PATTERN);
        plan["sheets"] = json!([sheet_plan("f", &["File"])]);
        plan["sheets"][0]["join"] = join_on("fileName", 0, "File", false, false, false);
    });
    let sheets = read_sheets(&listed, &[("f", b"File\nZ.db\n")]);
    models.push(derive(&index, &sheets, &listed, &catalog, &NoTags));
    models
}

#[test]
fn every_flag_kind_in_the_table_is_raised_with_its_default_policy() {
    let models = every_kind();
    let plans = [
        ("create", &models[0]),
        ("update", &models[1]),
        ("update", &models[2]),
        ("create", &models[3]),
    ];
    let mut seen = std::collections::BTreeSet::new();
    for (action, model) in plans {
        for flag in &model.flags {
            seen.insert(flag.kind);
            let expected = flag.kind.default_policy().map(|default| match default {
                PolicyDefault::Is(policy) | PolicyDefault::Only(policy) => policy,
                PolicyDefault::ActionUnmatched => FlagPolicy::Leave,
            });
            assert_eq!(flag.policy, expected, "{action}: {:?}", flag.kind);
        }
    }
    let missing: Vec<FlagKind> = FlagKind::ALL
        .into_iter()
        .filter(|kind| !seen.contains(kind))
        .collect();
    assert!(missing.is_empty(), "never raised: {missing:?}");
    assert!(
        models[0]
            .kept()
            .any(|record| record.kind == RecordKind::Item)
    );
}

#[test]
fn the_digest_is_stable_across_runs_and_input_order_and_moves_with_any_change() {
    let files = [
        ("A1_p1.jpg", JPEG),
        ("A1_p2.jpg", JPEG),
        ("B2.jpg", JPEG),
        ("C3.jpg", JPEG),
    ];
    let plan = plan(|plan| {
        plan["grouping"]["pattern"] = json!(PATTERN);
        plan["slots"]["item"]["dcterms:title"] =
            rule(json!([{ "from": "group", "name": "identifier" }]), "first");
    });
    let first = run(&index(&files), &Sheets::new(), &plan);
    assert_eq!(first.digest.len(), 64);
    for _ in 0..5 {
        assert_eq!(
            run(&index(&files), &Sheets::new(), &plan).digest,
            first.digest,
            "same input, same digest"
        );
    }
    let mut reversed = files;
    reversed.reverse();
    assert_eq!(
        run(&index(&reversed), &Sheets::new(), &plan).digest,
        first.digest,
        "input order does not count"
    );

    let mut changed_value = plan.clone();
    let (only, _) = changed_value
        .structure
        .all_series()
        .next()
        .expect("a series");
    changed_value.structure.rules_mut(only, RecordKind::Item)[0].sources =
        vec![crate::import::plan::SourceRef::Constant { value: "x".into() }];
    let digests = [
        run(&index(&files[..3]), &Sheets::new(), &plan).digest,
        run(&index(&files), &Sheets::new(), &changed_value).digest,
        run(
            &index(&[
                ("A1_p1.jpg", JPEG),
                ("A1_p2.jpg", "image/png"),
                ("B2.jpg", JPEG),
                ("C3.jpg", JPEG),
            ]),
            &Sheets::new(),
            &plan,
        )
        .digest,
    ];
    for (at, digest) in digests.iter().enumerate() {
        assert_ne!(*digest, first.digest, "change {at}");
    }
    let with_flag = run(&index(&[("A1.jpg", "image/png")]), &Sheets::new(), &plan);
    let flag_policy = run(
        &index(&[("A1.jpg", "image/png")]),
        &Sheets::new(),
        &crate::import::plan::ImportPlan {
            flags: [(FlagKind::FormatMismatch, FlagPolicy::Repair)]
                .into_iter()
                .collect(),
            ..plan.clone()
        },
    );
    assert_ne!(
        with_flag.digest, flag_policy.digest,
        "a flag's policy counts"
    );
}

/// The digest the admin saw on Check must still match at Run, so a change
/// to how the model is hashed must not change the hex of an unchanged model.
#[test]
fn the_digest_of_a_fixed_model_is_a_fixed_hex() {
    let plan = plan(|plan| {
        plan["grouping"]["pattern"] = json!(PATTERN);
        plan["slots"]["item"]["dcterms:title"] =
            rule(json!([{ "from": "group", "name": "identifier" }]), "first");
    });
    let model = run(
        &index(&[
            ("A1_p1.jpg", JPEG),
            ("A1_p2.jpg", JPEG),
            ("B2.jpg", "image/png"),
        ]),
        &Sheets::new(),
        &plan,
    );
    assert!(
        model.orphans.is_empty(),
        "no member this unit adds is in it"
    );
    assert_eq!(
        model.digest,
        "1f278c9892c94165ea8c9bb28dffdf3c890a70396411e00f452ed4bb7a1bd246"
    );
}
