//! Step 5: slot rules, defaults, the value map, named slots, dates.

use serde_json::json;

use super::support::*;
use crate::import2::catalog::{ExistingSet, Field, Template, User};
use crate::import2::derive::{Sheets, derive};
use crate::import2::model::{Fate, LeftBy, OwnerRef, SetRef, TemplateRef};
use crate::import2::plan::{FlagKind, FlagPolicy, Owner, RecordKind};
use crate::import2::values::{ValueType, Visibility};

/// One item per row of a register keyed by `Id`, the plan edited by `edit`.
fn register(
    headers: &[&str],
    csv: &str,
    edit: impl FnOnce(&mut serde_json::Value),
) -> crate::import2::model::Model {
    register_in(headers, csv, &catalog(), edit)
}

fn register_in(
    headers: &[&str],
    csv: &str,
    catalog: &crate::import2::catalog::Catalog,
    edit: impl FnOnce(&mut serde_json::Value),
) -> crate::import2::model::Model {
    let plan = plan(|plan| {
        plan["sheets"] = json!([sheet_plan("r", headers)]);
        plan["sheets"][0]["itemKey"] = json!({ "index": 0, "header": headers[0] });
        edit(plan);
    });
    let sheets = read_sheets(&plan, &[("r", csv.as_bytes())]);
    derive(
        &index(&[]),
        &sheets,
        &plan,
        catalog,
        &crate::import2::derive::NoTags,
    )
}

#[test]
fn combine_first_takes_the_first_source_with_a_value_and_all_the_union() {
    let model = register(&["Id", "A", "B"], "Id,A,B\n1,,b\n2,a,b\n", |plan| {
        plan["slots"]["item"]["dcterms:title"] =
            rule(json!([col("r", 1, "A"), col("r", 2, "B")]), "first");
        plan["slots"]["item"]["dcterms:subject"] = rule(
            json!([col("r", 1, "A"), col("r", 2, "B"), constant("b")]),
            "all",
        );
    });
    assert_eq!(texts(item(&model, "1"), "dcterms:title"), ["b"]);
    assert_eq!(texts(item(&model, "2"), "dcterms:title"), ["a"]);
    assert_eq!(
        texts(item(&model, "2"), "dcterms:subject"),
        ["a", "b"],
        "distinct by type, text and lang"
    );
}

#[test]
fn a51_values_are_distinct_by_type_text_and_lang() {
    let model = register(&["Id", "En", "Hi"], "Id,En,Hi\n1,Rain,Rain\n", |plan| {
        plan["sheets"][0]["columns"][1]["options"]["lang"] = json!("en");
        plan["sheets"][0]["columns"][2]["options"]["lang"] = json!("hi");
        plan["slots"]["item"]["dcterms:title"] = rule(
            json!([col("r", 1, "En"), col("r", 2, "Hi"), col("r", 1, "En")]),
            "all",
        );
    });
    let values = &item(&model, "1").slots
        [&crate::import2::plan::SlotName::parse("dcterms:title").expect("slot")];
    let langs: Vec<Option<&str>> = values
        .iter()
        .map(|value| value.value.lang.as_deref())
        .collect();
    assert_eq!(langs, [Some("en"), Some("hi")]);
}

#[test]
fn a50_a61_column_options_split_type_language_and_private_shape_values() {
    let model = register(
        &["Id", "Subjects", "Rights"],
        "Id,Subjects,Rights\n1,cats | dogs|  ,http://x.org/r Rights statement\n",
        |plan| {
            plan["sheets"][0]["columns"][1]["options"] =
                json!({ "split": "|", "type": "literal", "lang": "en", "private": true });
            plan["sheets"][0]["columns"][2]["options"]["type"] = json!("uri");
            plan["slots"]["item"]["dcterms:subject"] =
                rule(json!([col("r", 1, "Subjects")]), "first");
            plan["slots"]["item"]["dcterms:relation"] =
                rule(json!([col("r", 2, "Rights")]), "first");
        },
    );
    let record = item(&model, "1");
    assert_eq!(
        texts(record, "dcterms:subject"),
        ["cats", "dogs"],
        "split, trimmed, empty piece dropped"
    );
    let subject = &record.slots
        [&crate::import2::plan::SlotName::parse("dcterms:subject").expect("slot")][0]
        .value;
    assert_eq!(subject.lang.as_deref(), Some("en"));
    assert!(!subject.is_public);
    let rights = &record.slots
        [&crate::import2::plan::SlotName::parse("dcterms:relation").expect("slot")][0]
        .value;
    assert_eq!(rights.value_type, ValueType::Uri);
    assert_eq!(rights.text, "http://x.org/r");
    assert_eq!(rights.label.as_deref(), Some("Rights statement"));
}

#[test]
fn a50_more_values_on_a_single_valued_slot_flag_many_values_and_keep_the_first() {
    let make = |policy: &str| {
        register(&["Id", "Vis"], "Id,Vis\n1,public;private\n", |plan| {
            plan["sheets"][0]["columns"][1]["options"]["split"] = json!(";");
            plan["slots"]["item"]["visibility"] = rule(json!([col("r", 1, "Vis")]), "first");
            plan["flags"] = json!({ "many-values": policy });
        })
    };
    let model = make("keep");
    assert_eq!(texts(item(&model, "1"), "visibility"), ["public"]);
    assert_eq!(item(&model, "1").resolved.visibility, Visibility::Public);
    assert_eq!(
        flags(&model, FlagKind::ManyValues)[0].detail.as_deref(),
        Some("private")
    );
    let model = make("leave");
    assert!(texts(item(&model, "1"), "visibility").is_empty());
    assert_eq!(
        item(&model, "1").resolved.visibility,
        Visibility::Private,
        "the plan default"
    );
}

#[test]
fn a78_visibility_defaults_to_the_plan_and_false_no_off_0_private_mean_private() {
    let model = register(&["Id", "Vis"], "Id,Vis\n1,no\n2,yes\n3,\n", |plan| {
        plan["defaults"]["visibility"] = json!("public");
        plan["slots"]["item"]["visibility"] = rule(json!([col("r", 1, "Vis")]), "first");
    });
    assert_eq!(item(&model, "1").resolved.visibility, Visibility::Private);
    assert_eq!(item(&model, "2").resolved.visibility, Visibility::Public);
    assert_eq!(item(&model, "3").resolved.visibility, Visibility::Public);
}

#[test]
fn the_value_map_names_a_template_and_an_empty_target_turns_a_set_off() {
    let mut catalog = catalog();
    catalog.templates.push(Template {
        id: 7,
        label: "Artifact".into(),
        fields: Vec::new(),
    });
    let model = register_in(&["Id", "Code"], "Id,Code\n1,AF\n2,PH\n", &catalog, |plan| {
        plan["slots"]["item"]["template"] = rule(json!([col("r", 1, "Code")]), "first");
        plan["slots"]["item"]["template"]["map"] = json!({ "af": "Artifact" });
        plan["slots"]["item"]["sets"] = rule(json!([col("r", 1, "Code")]), "first");
        plan["slots"]["item"]["sets"]["map"] = json!({ "AF": "Artifacts", " ph ": "" });
    });
    let one = item(&model, "1");
    assert_eq!(
        one.resolved.template,
        Some(TemplateRef::Existing(7)),
        "mapped, compared as titles"
    );
    assert_eq!(texts(one, "sets"), ["Artifacts"]);
    let two = item(&model, "2");
    assert!(two.resolved.sets.is_empty(), "the set is off");
    assert!(
        !matches!(two.fate, Fate::Left(_)),
        "a record whose every set is off is still a record"
    );
    assert_eq!(model.new_sets.len(), 1);
}

#[test]
fn a25_an_empty_value_makes_no_set_type_or_field() {
    let model = register(
        &["Id", "Set", "Type", "Title"],
        "Id,Set,Type,Title\n1,  ,  ,   \n",
        |plan| {
            plan["slots"]["item"]["sets"] = rule(json!([col("r", 1, "Set")]), "first");
            plan["slots"]["item"]["template"] = rule(json!([col("r", 2, "Type")]), "first");
            plan["slots"]["item"]["dcterms:title"] = rule(json!([col("r", 3, "Title")]), "first");
        },
    );
    let record = item(&model, "1");
    assert!(model.new_sets.is_empty());
    assert!(texts(record, "dcterms:title").is_empty());
    assert_eq!(
        record.resolved.template,
        Some(TemplateRef::Existing(1)),
        "Base Resource"
    );
    assert!(flags(&model, FlagKind::UnknownType).is_empty());
}

#[test]
fn a22_a24_a26_a27_sets_name_an_id_a_record_or_a_title_folded_and_else_a_new_open_set() {
    let mut catalog = catalog();
    catalog.item_sets = vec![
        ExistingSet {
            id: 3,
            title: "Photographs".into(),
            ..ExistingSet::default()
        },
        ExistingSet {
            id: 4,
            title: "Letters".into(),
            ..ExistingSet::default()
        },
        ExistingSet {
            id: 5,
            title: "letters".into(),
            ..ExistingSet::default()
        },
    ];
    let model = register_in(
        &["Id", "Set"],
        "Id,Set\n1,photographs \n2,3\n3,Obj\n4,Obj \n5,Letters\n",
        &catalog,
        |plan| {
            plan["slots"]["item"]["sets"] = rule(json!([col("r", 1, "Set")]), "first");
            plan["flags"] = json!({ "space-at-ends": "keep" });
        },
    );
    assert_eq!(
        item(&model, "1").resolved.sets,
        [SetRef::Existing(3)],
        "case and space folded"
    );
    assert_eq!(
        item(&model, "2").resolved.sets,
        [SetRef::Existing(3)],
        "by id"
    );
    assert_eq!(
        item(&model, "3").resolved.sets,
        item(&model, "4").resolved.sets,
        "Obj and Obj  are one new set"
    );
    assert_eq!(model.new_sets.len(), 1);
    assert!(model.new_sets[0].is_open);
    assert_eq!(
        model.new_sets[0].visibility,
        Visibility::Private,
        "the record's visibility"
    );
    let many = flags(&model, FlagKind::ManyMatches);
    assert_eq!(many.len(), 1, "two sets titled Letters");
    assert_eq!(many[0].policy, Some(FlagPolicy::Leave));
    assert!(
        model
            .records
            .iter()
            .any(|record| record.fate == Fate::Left(LeftBy::Flag(FlagKind::ManyMatches)))
    );
}

#[test]
fn a33_a36_template_by_id_or_label_then_a_new_type_else_unknown_type_and_the_default() {
    let mut catalog = catalog();
    catalog.templates.push(Template {
        id: 7,
        label: "Artifact".into(),
        fields: Vec::new(),
    });
    let model = register_in(
        &["Id", "Type"],
        "Id,Type\n1,7\n2, artifact\n3,Letter\n4,Map\n",
        &catalog,
        |plan| {
            plan["slots"]["item"]["template"] = rule(json!([col("r", 1, "Type")]), "first");
            plan["newTypes"] = json!([{ "label": "Letter", "from": "mapping", "class": null }]);
            plan["defaults"]["template"] = json!(7);
        },
    );
    assert_eq!(
        item(&model, "1").resolved.template,
        Some(TemplateRef::Existing(7))
    );
    assert_eq!(
        item(&model, "2").resolved.template,
        Some(TemplateRef::Existing(7))
    );
    assert_eq!(
        item(&model, "3").resolved.template,
        Some(TemplateRef::New(0))
    );
    assert_eq!(
        item(&model, "4").resolved.template,
        Some(TemplateRef::Existing(7)),
        "defaults.template"
    );
    assert_eq!(
        flags(&model, FlagKind::UnknownType)[0].policy,
        Some(FlagPolicy::Keep)
    );

    let mut bare = catalog.clone();
    bare.base_template = None;
    bare.templates.retain(|template| template.id == 7);
    let model = register_in(&["Id", "Type"], "Id,Type\n1,Map\n", &bare, |plan| {
        plan["slots"]["item"]["template"] = rule(json!([col("r", 1, "Type")]), "first");
    });
    assert_eq!(
        item(&model, "1").resolved.template,
        None,
        "no default and no Base Resource"
    );
}

#[test]
fn owners_are_a_user_id_an_email_or_runner() {
    let mut catalog = catalog();
    catalog.users = vec![User {
        id: 4,
        email: "ana@example.org".into(),
    }];
    let model = register_in(
        &["Id", "Owner"],
        "Id,Owner\n1,4\n2,ANA@example.org\n3,runner\n4,bob@example.org\n5,\n",
        &catalog,
        |plan| {
            plan["slots"]["item"]["owner"] = rule(json!([col("r", 1, "Owner")]), "first");
            plan["defaults"]["owner"] = json!(4);
        },
    );
    assert_eq!(
        item(&model, "1").resolved.owner,
        OwnerRef::Is(Owner::User(4))
    );
    assert_eq!(
        item(&model, "2").resolved.owner,
        OwnerRef::Is(Owner::User(4))
    );
    assert_eq!(
        item(&model, "3").resolved.owner,
        OwnerRef::Is(Owner::Runner)
    );
    assert_eq!(
        item(&model, "4").resolved.owner,
        OwnerRef::Unknown("bob@example.org".into())
    );
    assert_eq!(
        item(&model, "5").resolved.owner,
        OwnerRef::Is(Owner::User(4))
    );
}

#[test]
fn a54_dcterms_identifier_defaults_to_the_group_key_unless_it_has_a_rule() {
    let files = [("A1.jpg", JPEG)];
    let default = plan(|plan| plan["grouping"]["pattern"] = json!(PATTERN));
    let model = run(&index(&files), &Sheets::new(), &default);
    assert_eq!(texts(item(&model, "A1"), "dcterms:identifier"), ["A1"]);
    let ruled = plan(|plan| {
        plan["grouping"]["pattern"] = json!(PATTERN);
        plan["slots"]["item"]["dcterms:identifier"] =
            rule(json!([constant("BHC-1"), constant("old-7")]), "all");
    });
    let model = run(&index(&files), &Sheets::new(), &ruled);
    let record = item(&model, "BHC-1");
    assert_eq!(
        texts(record, "dcterms:identifier"),
        ["BHC-1", "old-7"],
        "both kept; the first is the item key"
    );
    let keyed = plan(|plan| {
        plan["grouping"]["pattern"] = json!(PATTERN);
        plan["slots"]["item"]["itemKey"] = rule(json!([constant("K")]), "first");
    });
    let model = run(&index(&files), &Sheets::new(), &keyed);
    assert_eq!(texts(item(&model, "K"), "dcterms:identifier"), ["A1"]);
}

#[test]
fn a16_media_sort_naturally_by_path_or_media_order_and_the_first_is_primary() {
    let files = [
        ("D1_p10.tif", TIFF),
        ("D1_p2.tif", TIFF),
        ("D1_p1.tif", TIFF),
    ];
    let by_path = plan(|plan| plan["grouping"]["pattern"] = json!(PATTERN));
    let model = run(&index(&files), &Sheets::new(), &by_path);
    assert_eq!(
        media_paths(&model, item(&model, "D1")),
        ["D1_p1.tif", "D1_p2.tif", "D1_p10.tif"]
    );
    let by_order = plan(|plan| {
        plan["grouping"]["pattern"] = json!(PATTERN);
        plan["slots"]["media"]["mediaOrder"] =
            rule(json!([{ "from": "group", "name": "part" }]), "first");
        plan["slots"]["media"]["mediaOrder"]["map"] = json!({ "p10": "a" });
    });
    let model = run(&index(&files), &Sheets::new(), &by_order);
    assert_eq!(
        media_paths(&model, item(&model, "D1"))[0],
        "D1_p10.tif",
        "media order a sorts first"
    );
}

#[test]
fn a34_a37_new_types_from_the_mapping_get_a_field_per_term_written_and_require_a_title() {
    let model = register(
        &["Id", "Type", "Title", "Date"],
        "Id,Type,Title,Date\n1,Letter,Dear,1901\n2,Letter,,1902\n3,Person,Ana,\n",
        |plan| {
            plan["slots"]["item"]["template"] = rule(json!([col("r", 1, "Type")]), "first");
            plan["slots"]["item"]["dcterms:title"] = rule(json!([col("r", 2, "Title")]), "first");
            plan["slots"]["item"]["dcterms:date"] = rule(json!([col("r", 3, "Date")]), "first");
            plan["newTypes"] = json!([
                { "label": "Letter", "from": "mapping", "class": null },
                { "label": "Person", "from": "mapping", "class": "foaf:Person" }
            ]);
        },
    );
    let letter = &model.new_types[0];
    let fields: Vec<&str> = letter.fields.iter().map(|term| term.as_str()).collect();
    assert_eq!(
        fields,
        ["dcterms:title", "dcterms:date", "dcterms:identifier"]
    );
    assert_eq!(model.new_types[1].required.as_str(), "foaf:name");
    let empty = flags(&model, FlagKind::EmptyRequired);
    let slots: Vec<String> = empty
        .iter()
        .map(|flag| {
            flag.slot
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_default()
        })
        .collect();
    assert_eq!(
        slots,
        ["dcterms:title", "foaf:name"],
        "item 2 has no title; the person no name"
    );
    assert!(
        empty
            .iter()
            .all(|flag| flag.policy == Some(FlagPolicy::Keep))
    );
}

#[test]
fn a64_a_required_template_field_with_no_value_flags_empty_required() {
    let mut catalog = catalog();
    catalog.templates.push(Template {
        id: 7,
        label: "Artifact".into(),
        fields: vec![Field {
            term: term("dcterms:title"),
            required: true,
            data_types: Vec::new(),
        }],
    });
    let model = register_in(
        &["Id", "Title"],
        "Id,Title\n1,\n2,Pot\n",
        &catalog,
        |plan| {
            plan["defaults"]["template"] = json!(7);
            plan["slots"]["item"]["dcterms:title"] = rule(json!([col("r", 1, "Title")]), "first");
            plan["flags"] = json!({ "empty-required": "leave" });
        },
    );
    assert_eq!(kept(&model, RecordKind::Item).len(), 1);
    assert_eq!(flags(&model, FlagKind::EmptyRequired).len(), 1);
}

#[test]
fn a62_a63_a65_dates_not_edtf_flag_date_form_on_dc_terms_and_date_fields_and_are_kept() {
    let mut catalog = catalog();
    catalog.properties.insert(term("local:born"), "Born".into());
    catalog.templates.push(Template {
        id: 7,
        label: "Person".into(),
        fields: vec![Field {
            term: term("local:born"),
            required: false,
            data_types: vec!["numeric:timestamp".into()],
        }],
    });
    let model = register_in(
        &["Id", "Date", "Born", "Note"],
        "Id,Date,Born,Note\n1,c. 1890,1890s,c. 1890\n2,1890~,1890-03-12,x\n",
        &catalog,
        |plan| {
            plan["defaults"]["template"] = json!(7);
            plan["slots"]["item"]["dcterms:date"] = rule(json!([col("r", 1, "Date")]), "first");
            plan["slots"]["item"]["local:born"] = rule(json!([col("r", 2, "Born")]), "first");
            plan["slots"]["item"]["dcterms:description"] =
                rule(json!([col("r", 3, "Note")]), "first");
        },
    );
    let dates: Vec<&str> = flags(&model, FlagKind::DateForm)
        .iter()
        .filter_map(|flag| flag.detail.as_deref())
        .collect();
    assert_eq!(dates, ["c. 1890", "1890s"], "a description is not a date");
    assert_eq!(
        texts(item(&model, "1"), "dcterms:date"),
        ["c. 1890"],
        "kept as written"
    );
    assert_eq!(texts(item(&model, "2"), "dcterms:date"), ["1890~"]);
}

#[test]
fn a53_new_terms_are_known_and_any_other_unknown_term_blocks() {
    let model = register(&["Id", "Shelf", "X"], "Id,Shelf,X\n1,S-1,x\n", |plan| {
        plan["newTerms"] = json!([{ "label": "Shelf mark" }]);
        plan["slots"]["item"]["local:shelfMark"] = rule(json!([col("r", 1, "Shelf")]), "first");
        plan["slots"]["item"]["bibo:locator"] = rule(json!([col("r", 2, "X")]), "first");
    });
    let unknown = flags(&model, FlagKind::UnknownTerm);
    assert_eq!(unknown.len(), 1);
    assert_eq!(unknown[0].detail.as_deref(), Some("bibo:locator"));
    assert_eq!(unknown[0].policy, None, "blocking");
    assert!(model.blocked());
}

#[test]
fn a56_a67_space_at_ends_is_repaired_and_encoding_suspect_kept_by_default() {
    let model = register(
        &["Id", "Title"],
        "Id,Title\n1, Caf\u{c3}\u{a9} \n",
        |plan| {
            plan["slots"]["item"]["dcterms:title"] = rule(json!([col("r", 1, "Title")]), "first");
        },
    );
    assert_eq!(
        texts(item(&model, "1"), "dcterms:title"),
        ["Caf\u{c3}\u{a9}"]
    );
    assert_eq!(
        flags(&model, FlagKind::SpaceAtEnds)[0].policy,
        Some(FlagPolicy::Repair)
    );
    assert_eq!(
        flags(&model, FlagKind::EncodingSuspect)[0].policy,
        Some(FlagPolicy::Keep)
    );
}

#[test]
fn set_decisions_replace_a_slot_and_an_empty_list_clears_it() {
    let model = register(&["Id", "Title"], "Id,Title\n1,Old\n2,Old\n", |plan| {
        plan["slots"]["item"]["dcterms:title"] = rule(json!([col("r", 1, "Title")]), "first");
        plan["decisions"] = json!([
            { "kind": "set", "entry": { "row": { "sheet": "r", "key": "1#1" } }, "target": "item", "slot": "dcterms:title",
              "values": [{ "type": "literal", "text": "New", "lang": "en", "label": null }] },
            { "kind": "set", "entry": { "row": { "sheet": "r", "key": "2#1" } }, "target": "item", "slot": "dcterms:title", "values": [] }
        ]);
    });
    assert_eq!(texts(item(&model, "1"), "dcterms:title"), ["New"]);
    assert!(texts(item(&model, "2"), "dcterms:title").is_empty());
}

#[test]
fn a57_a69_tags_read_the_first_every_or_one_part_file_and_folders_read_by_level() {
    let files = [
        ("Letters/1950s/L1_front.jpg", JPEG),
        ("Letters/1950s/L1_back.jpg", JPEG),
    ];
    let mut tags = Tags::default();
    for (path, artist, date) in [
        ("Letters/1950s/L1_front.jpg", "Rao", "2019:05:14 11:02:10"),
        ("Letters/1950s/L1_back.jpg", "Iyer", "2019:05:15 09:00:00"),
    ] {
        tags.0.insert(
            (path.to_string(), crate::import2::plan::TagId::Artist),
            artist.to_string(),
        );
        tags.0.insert(
            (
                path.to_string(),
                crate::import2::plan::TagId::DateTimeOriginal,
            ),
            date.to_string(),
        );
    }
    let plan = plan(|plan| {
        plan["grouping"]["pattern"] = json!(PATTERN);
        plan["slots"]["item"]["dcterms:creator"] = rule(
            json!([{ "from": "tag", "tag": "Artist", "files": "all" }]),
            "all",
        );
        plan["slots"]["item"]["dcterms:created"] = rule(
            json!([{ "from": "tag", "tag": "DateTimeOriginal", "files": "first" }]),
            "first",
        );
        plan["slots"]["item"]["dcterms:description"] = rule(
            json!([{ "from": "tag", "tag": "Artist", "files": { "part": "front" } }]),
            "first",
        );
        plan["slots"]["item"]["sets"] = rule(json!([{ "from": "folder", "level": 2 }]), "first");
        plan["slots"]["item"]["dcterms:subject"] =
            rule(json!([{ "from": "folder", "level": "path" }]), "first");
        plan["slots"]["media"]["dcterms:creator"] = rule(
            json!([{ "from": "tag", "tag": "Artist", "files": "first" }]),
            "first",
        );
        plan["slots"]["media"]["dcterms:format"] =
            rule(json!([{ "from": "file", "fact": "format" }]), "first");
        plan["flags"] = json!({ "date-form": "keep" });
    });
    let model = derive(&index(&files), &Sheets::new(), &plan, &catalog(), &tags);
    let letter = item(&model, "L1");
    assert_eq!(
        texts(letter, "dcterms:creator"),
        ["Iyer", "Rao"],
        "every file, in media order"
    );
    assert_eq!(
        texts(letter, "dcterms:created"),
        ["2019:05:15 09:00:00"],
        "the first file: back sorts first"
    );
    assert_eq!(
        texts(letter, "dcterms:description"),
        ["Rao"],
        "the front part"
    );
    assert_eq!(texts(letter, "sets"), ["1950s"]);
    assert_eq!(texts(letter, "dcterms:subject"), ["Letters/1950s"]);
    let media: Vec<Vec<&str>> = letter
        .media
        .iter()
        .map(|id| texts(model.record(*id), "dcterms:creator"))
        .collect();
    assert_eq!(
        media,
        [vec!["Iyer"], vec!["Rao"]],
        "a media record reads its own file"
    );
    assert_eq!(
        texts(model.record(letter.media[0]), "dcterms:format"),
        [JPEG]
    );
}
