//! Steps 1 to 4: grouping, `move` decisions, joins and records.

use serde_json::json;

use super::support::*;
use crate::import2::catalog::FileRules;
use crate::import2::decisions::EntryRef;
use crate::import2::derive::{NoTags, Sheets, derive};
use crate::import2::model::{Fate, GroupKey, LeftBy, Origin, OrphanReason};
use crate::import2::plan::{FlagKind, FlagPolicy, RecordKind};

fn file_ref(path: &str) -> serde_json::Value {
    json!({ "file": { "source": "s1", "path": path } })
}

#[test]
fn group_key_and_identifier_are_different_things() {
    let files = [
        ("Box/A1_front.jpg", JPEG),
        ("Box/A1_back.jpg", JPEG),
        ("Box/B2.jpg", JPEG),
    ];
    let plan = plan(|plan| {
        plan["grouping"]["pattern"] = json!(PATTERN);
        plan["sheets"] = json!([sheet_plan("register", &["Code", "Accession"])]);
        plan["sheets"][0]["join"] = join_on("identifier", 0, "Code", false, false, false);
        plan["slots"]["item"]["dcterms:identifier"] =
            rule(json!([col("register", 1, "Accession")]), "first");
    });
    let sheets = read_sheets(&plan, &[("register", b"Code,Accession\nA1,1987.4.1\n")]);
    let model = run(&index(&files), &sheets, &plan);
    let items = kept(&model, RecordKind::Item);
    assert_eq!(items.len(), 2, "one item per group key");
    let joined = item(&model, "1987.4.1");
    assert_eq!(
        joined.origin,
        Origin::Group(GroupKey::Pattern("A1".into())),
        "the join used the group key"
    );
    assert_eq!(
        texts(joined, "dcterms:identifier"),
        ["1987.4.1"],
        "the identifier is the column"
    );
    assert_eq!(media_paths(&model, joined).len(), 2);
    let unjoined = items
        .iter()
        .find(|record| record.origin == Origin::Group(GroupKey::Pattern("B2".into())))
        .expect("B2");
    assert!(
        texts(unjoined, "dcterms:identifier").is_empty(),
        "the rule's column gives an unjoined item nothing"
    );
}

#[test]
fn a14_case_fold_makes_case_variants_one_group() {
    let files = [("BHC6_front.jpg", JPEG), ("bhc6_back.jpg", JPEG)];
    let exact = plan(|plan| plan["grouping"]["pattern"] = json!(PATTERN));
    assert_eq!(
        kept(
            &run(&index(&files), &Sheets::new(), &exact),
            RecordKind::Item
        )
        .len(),
        2
    );
    let folded = plan(|plan| {
        plan["grouping"]["pattern"] = json!(PATTERN);
        plan["grouping"]["caseFold"] = json!(true);
    });
    let model = run(&index(&files), &Sheets::new(), &folded);
    assert_eq!(kept(&model, RecordKind::Item).len(), 1);
    assert_eq!(item(&model, "bhc6").files.len(), 2);
}

#[test]
fn a17_files_the_pattern_does_not_match_are_unmatched_entries() {
    let files = [("A1.jpg", JPEG), ("notes", TEXT), ("Thumbs", BINARY)];
    let plan = plan(|plan| plan["grouping"]["pattern"] = json!(PATTERN));
    let model = run(&index(&files), &Sheets::new(), &plan);
    assert_eq!(kept(&model, RecordKind::Item).len(), 1);
    let unmatched: Vec<String> = model
        .entries
        .iter()
        .filter(|entry| entry.unmatched)
        .map(|entry| format!("{:?}", entry.entry))
        .collect();
    assert_eq!(unmatched.len(), 2, "{unmatched:?}");
    let examples = model.entries.iter().filter(|entry| entry.example).count();
    assert_eq!(
        examples, 2,
        "only files with a media type are example candidates"
    );
}

#[test]
fn move_decisions_join_another_group_start_their_own_or_leave() {
    let files = [
        ("A1.jpg", JPEG),
        ("A1_b.jpg", JPEG),
        ("B2.jpg", JPEG),
        ("odd file.jpg", JPEG),
        ("x", TEXT),
    ];
    let plan = plan(|plan| {
        plan["grouping"]["pattern"] = json!(PATTERN);
        plan["decisions"] = json!([
            { "kind": "move", "file": file_ref("A1_b.jpg"), "to": { "with": file_ref("B2.jpg") } },
            { "kind": "move", "file": file_ref("x"), "to": "own" },
            { "kind": "move", "file": file_ref("odd file.jpg"), "to": "leave" },
            { "kind": "move", "file": file_ref("gone.jpg"), "to": "own" },
            { "kind": "move", "file": file_ref("A1.jpg"), "to": { "with": file_ref("x2") } },
            { "kind": "move", "file": file_ref("A1.jpg"), "to": { "with": file_ref("odd file.jpg") } },
        ]);
    });
    let model = run(&index(&files), &Sheets::new(), &plan);
    assert_eq!(
        media_paths(&model, item(&model, "B2")),
        ["A1_b.jpg", "B2.jpg"]
    );
    assert_eq!(media_paths(&model, item(&model, "A1")), ["A1.jpg"]);
    let own = item(&model, "x");
    assert_eq!(
        own.origin,
        Origin::Group(GroupKey::Own(crate::import2::decisions::FileRef {
            source: crate::import2::plan::SourceId("s1".into()),
            path: crate::import2::plan::RelPath::parse("x").expect("path"),
        }))
    );
    let odd = model
        .entries
        .iter()
        .find(|entry| format!("{:?}", entry.entry).contains("odd file"))
        .expect("entry");
    assert_eq!(odd.left, Some(LeftBy::Decision(2)));
    let reasons: Vec<(usize, OrphanReason)> = model
        .orphans
        .iter()
        .map(|orphan| (orphan.decision, orphan.reason))
        .collect();
    assert_eq!(
        reasons,
        [
            (3, OrphanReason::MissingEntry),
            (4, OrphanReason::MissingEntry),
            (5, OrphanReason::NoGroup)
        ]
    );
}

/// Rows `x_003`, `7` and `B 4` against groups `X_003`, `007` and `B_4`.
fn joined_count(case_fold: bool, leading_zeros: bool, separators: bool) -> usize {
    let files = [("X_003.jpg", JPEG), ("007.jpg", JPEG), ("B_4.jpg", JPEG)];
    let plan = plan(|plan| {
        plan["grouping"]["pattern"] = json!(r"^(?<identifier>.+)\.jpg$");
        plan["sheets"] = json!([sheet_plan("register", &["Id", "Title"])]);
        plan["sheets"][0]["join"] =
            join_on("identifier", 0, "Id", case_fold, leading_zeros, separators);
        plan["slots"]["item"]["dcterms:title"] =
            rule(json!([col("register", 1, "Title")]), "first");
    });
    let sheets = read_sheets(&plan, &[("register", b"Id,Title\nx_003,a\n7,b\nB 4,c\n")]);
    let model = run(&index(&files), &sheets, &plan);
    model
        .kept()
        .filter(|record| {
            record.kind == RecordKind::Item && !record.files.is_empty() && !record.rows.is_empty()
        })
        .count()
}

#[test]
fn a47_rows_join_by_identifier_after_each_normalisation_and_only_that_one() {
    assert_eq!(joined_count(false, false, false), 0, "exact");
    assert_eq!(joined_count(true, false, false), 1, "case fold joins x_003");
    assert_eq!(joined_count(false, true, false), 1, "leading zeros join 7");
    assert_eq!(joined_count(false, false, true), 1, "separators join B 4");
    assert_eq!(joined_count(true, true, true), 3);
}

#[test]
fn a47_rows_join_by_file_name_with_normalisation() {
    let files = [("DCIM/IMG_0001.JPG", JPEG), ("DCIM/IMG_0002.JPG", JPEG)];
    let plan = plan(|plan| {
        plan["grouping"]["pattern"] = json!(r"^(?:.*/)?(?<identifier>[^/]+)\.JPG$");
        plan["sheets"] = json!([sheet_plan("shots", &["File", "Caption"])]);
        plan["sheets"][0]["join"] = join_on("fileName", 0, "File", true, false, false);
        plan["slots"]["item"]["dcterms:title"] = rule(json!([col("shots", 1, "Caption")]), "first");
    });
    let sheets = read_sheets(
        &plan,
        &[(
            "shots",
            b"File,Caption\nimg_0002.jpg,Harbour\nDCIM/IMG_0001.JPG,Gate\n",
        )],
    );
    let model = run(&index(&files), &sheets, &plan);
    assert_eq!(
        texts(item(&model, "IMG_0002"), "dcterms:title"),
        ["Harbour"],
        "by name, case folded"
    );
    assert_eq!(
        texts(item(&model, "IMG_0001"), "dcterms:title"),
        ["Gate"],
        "by path"
    );
}

#[test]
fn a48_a_second_row_with_one_join_key_flags_duplicate_row_and_only_the_first_joins() {
    let files = [("A1.jpg", JPEG)];
    let make = |policy: Option<&str>| {
        plan(|plan| {
            plan["grouping"]["pattern"] = json!(PATTERN);
            plan["sheets"] = json!([sheet_plan("register", &["Id", "Title"])]);
            plan["sheets"][0]["join"] = join_on("identifier", 0, "Id", false, false, false);
            plan["slots"]["item"]["dcterms:title"] =
                rule(json!([col("register", 1, "Title")]), "first");
            if let Some(policy) = policy {
                plan["flags"] = json!({ "duplicate-row": policy });
            }
        })
    };
    let csv: &[u8] = b"Id,Title\nA1,First\nA1,Second\n";
    let default = make(None);
    let model = run(
        &index(&files),
        &read_sheets(&default, &[("register", csv)]),
        &default,
    );
    assert_eq!(texts(item(&model, "A1"), "dcterms:title"), ["First"]);
    assert_eq!(kept(&model, RecordKind::Item).len(), 1);
    let duplicate = flags(&model, FlagKind::DuplicateRow);
    assert_eq!(duplicate.len(), 1);
    assert_eq!(duplicate[0].policy, Some(FlagPolicy::Leave));
    let keep = make(Some("keep"));
    let model = run(
        &index(&files),
        &read_sheets(&keep, &[("register", csv)]),
        &keep,
    );
    assert_eq!(
        kept(&model, RecordKind::Item).len(),
        2,
        "kept: a record of its own"
    );
    assert_eq!(texts(item(&model, "A1"), "dcterms:title"), ["First"]);
}

#[test]
fn a49_media_rows_attach_to_the_item_whose_item_key_their_cell_names() {
    let files = [("A1_p1.jpg", JPEG), ("A1_p2.jpg", JPEG)];
    let plan = plan(|plan| {
        plan["grouping"]["pattern"] = json!(PATTERN);
        plan["sheets"] = json!([sheet_plan("images", &["Item", "File", "Caption"])]);
        plan["sheets"][0]["rowKind"] = json!("media");
        plan["sheets"][0]["join"] = join_on("fileName", 1, "File", false, false, false);
        plan["sheets"][0]["itemKey"] = json!({ "index": 0, "header": "Item" });
        plan["slots"]["media"]["dcterms:title"] =
            rule(json!([col("images", 2, "Caption")]), "first");
    });
    let csv: &[u8] = b"Item,File,Caption\nA1,A1_p2.jpg,Back\nA1,,A link\nZ9,,Nowhere\n";
    let model = run(
        &index(&files),
        &read_sheets(&plan, &[("images", csv)]),
        &plan,
    );
    let a1 = item(&model, "A1");
    let captions: Vec<Vec<&str>> = a1
        .media
        .iter()
        .map(|id| texts(model.record(*id), "dcterms:title"))
        .collect();
    assert_eq!(
        captions,
        [vec![], vec!["Back"], vec!["A link"]],
        "the joined file keeps its place; the row media follows"
    );
    let unmatched = flags(&model, FlagKind::Unmatched);
    assert_eq!(unmatched.len(), 1, "Z9 names no item");
    assert_eq!(unmatched[0].detail.as_deref(), Some("Z9"));
}

#[test]
fn item_set_rows_are_item_set_records_and_kind_columns_say_per_row() {
    let plan = plan(|plan| {
        plan["sheets"] = json!([sheet_plan("mixed", &["Kind", "Title"])]);
        plan["sheets"][0]["rowKind"] = json!({ "column": { "index": 0, "header": "Kind" } });
        plan["slots"]["itemSet"]["dcterms:title"] =
            rule(json!([col("mixed", 1, "Title")]), "first");
        plan["slots"]["item"]["dcterms:title"] = rule(json!([col("mixed", 1, "Title")]), "first");
    });
    let csv: &[u8] = b"Kind,Title\nItem Set,Letters\nitems,A letter\nfolder,Huh\n";
    let model = run(&index(&[]), &read_sheets(&plan, &[("mixed", csv)]), &plan);
    let sets = kept(&model, RecordKind::ItemSet);
    assert_eq!(sets.len(), 1);
    assert_eq!(texts(sets[0], "dcterms:title"), ["Letters"]);
    assert_eq!(kept(&model, RecordKind::Item).len(), 1);
    let unknown = flags(&model, FlagKind::UnknownKind);
    assert_eq!(unknown.len(), 1);
    assert_eq!(unknown[0].policy, Some(FlagPolicy::Leave));
}

#[test]
fn a55_a_cells_count_row_is_left_out_by_default() {
    let plan = plan(|plan| {
        plan["sheets"] = json!([sheet_plan("register", &["Id", "Title"])]);
        plan["sheets"][0]["itemKey"] = json!({ "index": 0, "header": "Id" });
        plan["slots"]["item"]["dcterms:title"] =
            rule(json!([col("register", 1, "Title")]), "first");
    });
    let csv: &[u8] = b"Id,Title\nA1,Fine\nA2,Shifted,1890\n";
    let model = run(
        &index(&[]),
        &read_sheets(&plan, &[("register", csv)]),
        &plan,
    );
    assert_eq!(kept(&model, RecordKind::Item).len(), 1);
    assert_eq!(
        flags(&model, FlagKind::CellsCount)[0].policy,
        Some(FlagPolicy::Leave)
    );
}

#[test]
fn an_item_row_with_no_file_group_is_an_item_flagged_no_file() {
    let plan = plan(|plan| {
        plan["grouping"]["pattern"] = json!(PATTERN);
        plan["sheets"] = json!([sheet_plan("register", &["Id"])]);
        plan["sheets"][0]["join"] = join_on("identifier", 0, "Id", false, false, false);
    });
    let model = run(
        &index(&[("A1.jpg", JPEG)]),
        &read_sheets(&plan, &[("register", b"Id\nA1\nC3\n")]),
        &plan,
    );
    let lone = item(&model, "C3");
    assert!(lone.files.is_empty());
    assert_eq!(texts(lone, "dcterms:identifier"), ["C3"], "the join cell");
    let no_file = flags(&model, FlagKind::NoFile);
    assert_eq!(no_file.len(), 1);
    assert_eq!(no_file[0].policy, Some(FlagPolicy::Keep));
}

#[test]
fn a82_files_the_install_refuses_are_not_media_and_left_out() {
    let files = [
        ("A1_front.jpg", JPEG),
        ("A1_db.db", BINARY),
        ("B2.db", BINARY),
    ];
    let plan = plan(|plan| plan["grouping"]["pattern"] = json!(PATTERN));
    let mut catalog = catalog();
    catalog.files = FileRules::Allow {
        media_types: [JPEG.to_string()].into(),
        extensions: ["jpg".to_string()].into(),
    };
    let model = derive(&index(&files), &Sheets::new(), &plan, &catalog, &NoTags);
    assert_eq!(
        kept(&model, RecordKind::Item).len(),
        1,
        "B2 has no media left, so no item"
    );
    assert_eq!(media_paths(&model, item(&model, "A1")), ["A1_front.jpg"]);
    let not_media = flags(&model, FlagKind::NotMedia);
    assert_eq!(not_media.len(), 2);
    assert!(
        not_media
            .iter()
            .all(|flag| flag.policy == Some(FlagPolicy::Leave))
    );
    assert_eq!(
        not_media[0].detail.as_deref(),
        Some(BINARY),
        "names the refused type"
    );
}

#[test]
fn a60_format_mismatch_and_name_encoding_are_kept_by_default() {
    let mut index = index(&[("A1.jpg", "image/png"), ("B2.jpg", JPEG)]);
    index.sources[0].walk.entries[1].decoded = Some(crate::import2::evidence::DecodedName {
        encoding: crate::import2::evidence::NameEncoding::Latin1,
        raw: b"B2.jpg".to_vec(),
    });
    let plan = plan(|plan| plan["grouping"]["pattern"] = json!(PATTERN));
    let model = run(&index, &Sheets::new(), &plan);
    assert_eq!(kept(&model, RecordKind::Item).len(), 2);
    let mismatch = flags(&model, FlagKind::FormatMismatch);
    assert_eq!(mismatch.len(), 1);
    assert_eq!(mismatch[0].policy, Some(FlagPolicy::Keep));
    let names = flags(&model, FlagKind::NameEncoding);
    assert_eq!(names.len(), 1);
    assert_eq!(names[0].policy, Some(FlagPolicy::Keep));
}

#[test]
fn a_leave_decision_leaves_the_record_that_holds_the_entry() {
    let plan = plan(|plan| {
        plan["grouping"]["pattern"] = json!(PATTERN);
        plan["decisions"] = json!([{ "kind": "leave", "entry": file_ref("A1_back.jpg") }]);
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
    let left: Vec<_> = model
        .records
        .iter()
        .filter(|record| record.fate == Fate::Left(LeftBy::Decision(0)))
        .collect();
    assert_eq!(left.len(), 1);
    assert_eq!(left[0].kind, RecordKind::Item);
    let media_left = model
        .records
        .iter()
        .filter(|record| record.fate == Fate::Left(LeftBy::Parent))
        .count();
    assert_eq!(media_left, 2, "its media go with it");
    assert_eq!(kept(&model, RecordKind::Item).len(), 1);
    assert!(matches!(model.entries[0].entry, EntryRef::File(_)));
}

#[test]
fn a48_two_rows_whose_file_names_land_in_one_group_are_a_duplicate() {
    let plan = plan(|plan| {
        plan["grouping"]["pattern"] = json!(PATTERN);
        plan["sheets"] = json!([sheet_plan("shots", &["File", "Caption"])]);
        plan["sheets"][0]["join"] = join_on("fileName", 0, "File", false, false, false);
        plan["slots"]["item"]["dcterms:title"] = rule(json!([col("shots", 1, "Caption")]), "first");
    });
    let csv: &[u8] = b"File,Caption\nA1_front.jpg,Front\nA1_back.jpg,Back\n";
    let model = run(
        &index(&[("A1_front.jpg", JPEG), ("A1_back.jpg", JPEG)]),
        &read_sheets(&plan, &[("shots", csv)]),
        &plan,
    );
    assert_eq!(texts(item(&model, "A1"), "dcterms:title"), ["Front"]);
    assert_eq!(flags(&model, FlagKind::DuplicateRow).len(), 1);
    assert_eq!(kept(&model, RecordKind::Item).len(), 1);
}
