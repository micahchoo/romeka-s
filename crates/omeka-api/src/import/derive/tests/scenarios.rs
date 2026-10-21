//! The contract's scenarios S1–S8 and S15–S19, at model level, on
//! generated evidence.

use serde_json::json;

use super::support::*;
use crate::import::catalog::{Catalog, Template};
use crate::import::decisions::EntryRef;
use crate::import::derive::{Index, NoTags, Sheets, derive};
use crate::import::model::{Fate, Link, Model, OrphanReason, Parent, SetRef, Subject, TemplateRef};
use crate::import::plan::{FlagKind, FlagPolicy, ImportPlanV2, RecordKind};
use crate::import::values::Visibility;
use omeka_media::validation::FileValidation;

const BHC: &str = r"^(?<set>[^/]+)/(?:[^/]+/)*(?<identifier>BHC_006_[A-Z]{2}_\d{3})(?:_(?<part>[a-z0-9]+))?\.(?:jpg|tif)$";

fn bhc_index() -> Index {
    let mut files: Vec<(String, &str)> = Vec::new();
    for n in 1..=4 {
        files.push((format!("Artifacts/BHC_006_AF_00{n}_front.jpg"), JPEG));
        files.push((format!("Artifacts/BHC_006_AF_00{n}_back.jpg"), JPEG));
        files.push((format!("Photographs/1950s/BHC_006_PH_10{n}.tif"), TIFF));
    }
    for page in ["p1", "p2", "p10"] {
        files.push((format!("Scans/BHC_006_SC_001_{page}.tif"), TIFF));
    }
    files.push(("_scratch/notes.txt".into(), TEXT));
    files.push(("_scratch/Thumbs.db".into(), BINARY));
    let listed: Vec<(&str, &str)> = files
        .iter()
        .map(|(path, sniffed)| (path.as_str(), *sniffed))
        .collect();
    index(&listed)
}

fn bhc_catalog() -> Catalog {
    let mut catalog = catalog();
    catalog.files = FileValidation::Allow {
        media_types: [JPEG, TIFF, TEXT].iter().map(ToString::to_string).collect(),
        extensions: ["jpg", "tif", "txt"]
            .iter()
            .map(ToString::to_string)
            .collect(),
    };
    catalog
}

fn bhc_plan(pattern: &str, decisions: serde_json::Value) -> ImportPlanV2 {
    plan(|plan| {
        plan["grouping"]["pattern"] = json!(pattern);
        plan["slots"]["item"]["sets"] = rule(json!([{ "from": "group", "name": "set" }]), "first");
        plan["decisions"] = decisions;
    })
}

fn items_in_set(model: &Model, title: &str) -> usize {
    let set = model
        .new_sets
        .iter()
        .position(|set| set.title == title)
        .expect("set");
    kept(model, RecordKind::Item)
        .iter()
        .filter(|record| record.resolved.sets == [SetRef::New(set)])
        .count()
}

#[test]
fn s1_the_bhc_006_demo() {
    let plan = bhc_plan(BHC, json!([]));
    let model = derive(&bhc_index(), &Sheets::new(), &plan, &bhc_catalog(), &NoTags);
    assert_eq!(items_in_set(&model, "Artifacts"), 4);
    assert_eq!(items_in_set(&model, "Photographs"), 4);
    assert_eq!(items_in_set(&model, "Scans"), 1);
    assert_eq!(kept(&model, RecordKind::Item).len(), 9);
    assert!(
        model
            .kept()
            .all(|record| record.resolved.visibility == Visibility::Private),
        "all private"
    );
    let scratch: Vec<_> = model
        .entries
        .iter()
        .filter(|entry| format!("{:?}", entry.entry).contains("_scratch"))
        .collect();
    assert_eq!(scratch.len(), 2);
    let not_media = flags(&model, FlagKind::NotMedia);
    for entry in scratch {
        let flagged = not_media.iter().any(|flag| matches!(&flag.subject, crate::import::model::Subject::Entry(e) if *e == entry.entry));
        assert!(
            entry.record.is_none() && (entry.unmatched || flagged),
            "{:?}",
            entry.entry
        );
    }
    assert_eq!(
        media_paths(&model, item(&model, "BHC_006_SC_001"))[0],
        "Scans/BHC_006_SC_001_p1.tif"
    );
}

#[test]
fn s2_names_in_any_script_make_one_item_per_object_and_the_example_matches_itself() {
    let files = [
        ("पत्र_001_आगे.jpg", JPEG),
        ("पत्र_001_पीछे.jpg", JPEG),
        ("LT_௧௨.jpg", JPEG),
        ("Jose\u{301}_001.jpg", JPEG),
        ("Kraków_001.jpg", JPEG),
    ];
    let plan = plan(|plan| {
        plan["grouping"]["pattern"] =
            json!(r"^(?<identifier>[^_]+_[^_.]+)(?:_(?<part>[^.]+))?\.jpg$")
    });
    let model = run(&index(&files), &Sheets::new(), &plan);
    let keys: Vec<&str> = kept(&model, RecordKind::Item)
        .iter()
        .filter_map(|record| record.resolved.item_key.as_deref())
        .collect();
    assert_eq!(keys, ["Jos\u{e9}_001", "Kraków_001", "LT_௧௨", "पत्र_001"]);
    assert_eq!(item(&model, "पत्र_001").files.len(), 2);
    let example = model
        .entries
        .iter()
        .find(|entry| entry.example)
        .expect("an example");
    assert!(!example.unmatched, "the example matches itself");
}

#[test]
fn s3_template_and_set_come_from_a_name_part_in_a_flat_folder_with_a_register() {
    let files = [
        ("BHC_006_AF_001.jpg", JPEG),
        ("BHC_006_AF_002.jpg", JPEG),
        ("BHC_006_PH_101.jpg", JPEG),
    ];
    let plan = plan(|plan| {
        plan["grouping"]["pattern"] =
            json!(r"^(?<identifier>BHC_006_(?<code>[A-Z]{2})_\d{3})\.jpg$");
        plan["slots"]["item"]["template"] =
            rule(json!([{ "from": "group", "name": "code" }]), "first");
        plan["slots"]["item"]["template"]["map"] = json!({ "AF": "Artifact", "PH": "Photograph" });
        plan["slots"]["item"]["sets"] = rule(json!([{ "from": "group", "name": "code" }]), "first");
        plan["slots"]["item"]["sets"]["map"] = json!({ "AF": "Artifacts", "PH": "Photographs" });
        plan["sheets"] = json!([sheet_plan("register", &["Identifier", "Title"])]);
        plan["sheets"][0]["join"] = join_on("identifier", 0, "Identifier", false, false, false);
        plan["slots"]["item"]["dcterms:title"] =
            rule(json!([col("register", 1, "Title")]), "first");
    });
    let csv: &[u8] = b"Identifier,Title\nBHC_006_AF_001,Brass lamp\nBHC_006_PH_101,Wedding\n";
    let mut catalog = catalog();
    catalog.templates.push(Template {
        id: 7,
        label: "Artifact".into(),
        fields: Vec::new(),
    });
    catalog.templates.push(Template {
        id: 8,
        label: "Photograph".into(),
        fields: Vec::new(),
    });
    let model = derive(
        &index(&files),
        &read_sheets(&plan, &[("register", csv)]),
        &plan,
        &catalog,
        &NoTags,
    );
    let lamp = item(&model, "BHC_006_AF_001");
    assert_eq!(lamp.resolved.template, Some(TemplateRef::Existing(7)));
    assert_eq!(texts(lamp, "dcterms:title"), ["Brass lamp"]);
    assert_eq!(
        item(&model, "BHC_006_PH_101").resolved.template,
        Some(TemplateRef::Existing(8))
    );
    assert_eq!(items_in_set(&model, "Artifacts"), 2);
    assert_eq!(items_in_set(&model, "Photographs"), 1);
}

#[test]
fn s4_one_item_per_folder_pair_media_in_page_order_first_page_primary() {
    let files = [
        ("Box 1/Folder 1/0002.tif", TIFF),
        ("Box 1/Folder 1/0010.tif", TIFF),
        ("Box 1/Folder 1/0001.tif", TIFF),
        ("Box 1/Folder 2/0001.tif", TIFF),
        ("Box 2/Folder 1/0001.tif", TIFF),
    ];
    let plan = plan(|plan| {
        plan["grouping"]["pattern"] =
            json!(r"^(?<identifier>Box \d+/Folder \d+)/(?<part>\d+)\.tif$")
    });
    let model = run(&index(&files), &Sheets::new(), &plan);
    assert_eq!(kept(&model, RecordKind::Item).len(), 3);
    let folder = item(&model, "Box 1/Folder 1");
    assert_eq!(
        media_paths(&model, folder),
        [
            "Box 1/Folder 1/0001.tif",
            "Box 1/Folder 1/0002.tif",
            "Box 1/Folder 1/0010.tif"
        ]
    );
}

#[test]
fn s5_a_camera_dump_joins_a_sheet_by_file_name() {
    let files = [
        ("DCIM/100CANON/IMG_0001.JPG", JPEG),
        ("DCIM/100CANON/IMG_0002.JPG", JPEG),
        ("DCIM/100CANON/IMG_0003.JPG", JPEG),
    ];
    let plan = plan(|plan| {
        plan["grouping"]["pattern"] = json!(r"^(?:.*/)?(?<identifier>IMG_\d+)\.JPG$");
        plan["sheets"] = json!([sheet_plan("shots", &["File name", "Caption"])]);
        plan["sheets"][0]["join"] = join_on("fileName", 0, "File name", true, false, false);
        plan["slots"]["item"]["dcterms:title"] = rule(json!([col("shots", 1, "Caption")]), "first");
    });
    let csv: &[u8] =
        b"File name,Caption\nIMG_0003.jpg,Harbour\nIMG_0001.JPG,Gate\nIMG_0099.JPG,Lost\n";
    let model = run(
        &index(&files),
        &read_sheets(&plan, &[("shots", csv)]),
        &plan,
    );
    assert_eq!(texts(item(&model, "IMG_0001"), "dcterms:title"), ["Gate"]);
    assert_eq!(
        texts(item(&model, "IMG_0003"), "dcterms:title"),
        ["Harbour"]
    );
    assert!(texts(item(&model, "IMG_0002"), "dcterms:title").is_empty());
    assert_eq!(
        flags(&model, FlagKind::NoFile).len(),
        1,
        "IMG_0099 names no file"
    );
}

#[test]
fn s6_a_register_with_one_row_per_image_attaches_media_by_item_key() {
    let files = [
        ("scans/s001.tif", TIFF),
        ("scans/s002.tif", TIFF),
        ("scans/s003.tif", TIFF),
    ];
    let plan = plan(|plan| {
        // The files make no items of their own: the register's rows are the items.
        plan["grouping"]["pattern"] = json!(null);
        plan["sheets"] = json!([
            sheet_plan("objects", &["Id", "Title"]),
            sheet_plan("images", &["Object", "File", "Page"])
        ]);
        plan["sheets"][0]["itemKey"] = json!({ "index": 0, "header": "Id" });
        plan["sheets"][1]["rowKind"] = json!("media");
        plan["sheets"][1]["join"] = join_on("fileName", 1, "File", false, false, false);
        plan["sheets"][1]["itemKey"] = json!({ "index": 0, "header": "Object" });
        plan["slots"]["item"]["dcterms:title"] = rule(json!([col("objects", 1, "Title")]), "first");
        plan["slots"]["media"]["mediaOrder"] = rule(json!([col("images", 2, "Page")]), "first");
    });
    let objects: &[u8] = b"Id,Title\nO1,Diary\nO2,Map\n";
    let images: &[u8] = b"Object,File,Page\nO1,s002.tif,2\nO2,s003.tif,1\nO1,s001.tif,1\n";
    let model = run(
        &index(&files),
        &read_sheets(&plan, &[("objects", objects), ("images", images)]),
        &plan,
    );
    assert_eq!(
        media_paths(&model, item(&model, "O1")),
        ["scans/s001.tif", "scans/s002.tif"]
    );
    assert_eq!(media_paths(&model, item(&model, "O2")), ["scans/s003.tif"]);
    assert_eq!(kept(&model, RecordKind::Media).len(), 3);
}

#[test]
fn s7_a_utf16_tsv_splits_values_and_sets_languages() {
    let text = "Id\tTitle_en\tTitle_hi\tSubjects\r\nL1\tLetter\tपत्र\tfamily|travel\r\n";
    let mut bytes = vec![0xFF, 0xFE];
    bytes.extend(text.encode_utf16().flat_map(u16::to_le_bytes));
    let plan = plan(|plan| {
        plan["sheets"] = json!([sheet_plan("r", &["Id", "Title_en", "Title_hi", "Subjects"])]);
        plan["sheets"][0]["read"]["encoding"] = json!("utf-16le");
        plan["sheets"][0]["read"]["delimiter"] = json!("\t");
        plan["sheets"][0]["itemKey"] = json!({ "index": 0, "header": "Id" });
        plan["sheets"][0]["columns"][1]["options"]["lang"] = json!("en");
        plan["sheets"][0]["columns"][2]["options"]["lang"] = json!("hi");
        plan["sheets"][0]["columns"][3]["options"]["split"] = json!("|");
        plan["slots"]["item"]["dcterms:title"] = rule(
            json!([col("r", 1, "Title_en"), col("r", 2, "Title_hi")]),
            "all",
        );
        plan["slots"]["item"]["dcterms:subject"] = rule(json!([col("r", 3, "Subjects")]), "first");
    });
    let model = run(&index(&[]), &read_sheets(&plan, &[("r", &bytes)]), &plan);
    let letter = item(&model, "L1");
    assert_eq!(texts(letter, "dcterms:subject"), ["family", "travel"]);
    let titles =
        &letter.slots[&crate::import::plan::SlotName::parse("dcterms:title").expect("slot")];
    let pairs: Vec<(&str, Option<&str>)> = titles
        .iter()
        .map(|value| (value.value.text.as_str(), value.value.lang.as_deref()))
        .collect();
    assert_eq!(pairs, [("Letter", Some("en")), ("पत्र", Some("hi"))]);
}

#[test]
fn s8_a_mixed_sheet_makes_item_sets_items_and_media_linked_by_identifier() {
    let headers = ["Kind", "Id", "Title", "Set", "Item", "URL", "Related"];
    let plan = plan(|plan| {
        plan["sheets"] = json!([sheet_plan("mixed", &headers)]);
        plan["sheets"][0]["rowKind"] = json!({ "column": { "index": 0, "header": "Kind" } });
        plan["sheets"][0]["itemKey"] = json!({ "index": 4, "header": "Item" });
        plan["sheets"][0]["columns"][6]["options"]["type"] = json!({ "resource": "identifier" });
        for kind in ["item", "itemSet"] {
            plan["slots"][kind]["dcterms:identifier"] =
                rule(json!([col("mixed", 1, "Id")]), "first");
            plan["slots"][kind]["dcterms:title"] = rule(json!([col("mixed", 2, "Title")]), "first");
        }
        plan["slots"]["item"]["sets"] = rule(json!([col("mixed", 3, "Set")]), "first");
        plan["slots"]["item"]["dcterms:relation"] =
            rule(json!([col("mixed", 6, "Related")]), "first");
        plan["slots"]["media"]["ingester"] = rule(json!([constant("url")]), "first");
        plan["slots"]["media"]["mediaSource"] = rule(json!([col("mixed", 5, "URL")]), "first");
    });
    let csv: &[u8] = b"Kind,Id,Title,Set,Item,URL,Related\n\
        item set,SET1,Letters,,,,\n\
        item,L1,First letter,SET1,,,\n\
        item,L2,Reply,SET1,,,L1\n\
        media,,,,L1,https://example.org/1.jpg,\n";
    let model = run(&index(&[]), &read_sheets(&plan, &[("mixed", csv)]), &plan);
    let set = kept(&model, RecordKind::ItemSet);
    assert_eq!(set.len(), 1);
    assert_eq!(kept(&model, RecordKind::Item).len(), 2);
    let l1 = item(&model, "L1");
    assert_eq!(
        l1.resolved.sets,
        [SetRef::Record(set[0].id)],
        "the set by its item key"
    );
    assert!(model.new_sets.is_empty());
    let media = kept(&model, RecordKind::Media);
    assert_eq!(media.len(), 1);
    assert_eq!(media[0].parent, Some(Parent::Record(l1.id)));
    assert_eq!(
        media[0].resolved.media_source.as_deref(),
        Some("https://example.org/1.jpg")
    );
    assert_eq!(l1.media, [media[0].id]);
    let reply = item(&model, "L2");
    let related =
        &reply.slots[&crate::import::plan::SlotName::parse("dcterms:relation").expect("slot")][0];
    assert_eq!(related.link, Some(Link::Record(l1.id)));
}

#[test]
fn s15_after_a_new_pattern_decisions_on_surviving_files_apply_and_the_rest_are_orphans() {
    let title = |path: &str, target: &str, text: &str| {
        json!({ "kind": "set", "entry": { "file": { "source": "s1", "path": path } }, "target": target,
                "slot": "dcterms:title", "values": [{ "type": "literal", "text": text, "lang": null, "label": null }] })
    };
    let decisions = json!([
        title("Artifacts/BHC_006_AF_001_front.jpg", "item", "Brass lamp"),
        title("Photographs/1950s/BHC_006_PH_101.tif", "media", "Print"),
        { "kind": "leave", "entry": { "file": { "source": "s1", "path": "Artifacts/BHC_006_AF_004_back.jpg" } } }
    ]);
    let before = derive(
        &bhc_index(),
        &Sheets::new(),
        &bhc_plan(BHC, decisions.clone()),
        &bhc_catalog(),
        &NoTags,
    );
    assert!(before.orphans.is_empty());
    assert_eq!(
        texts(item(&before, "BHC_006_AF_001"), "dcterms:title"),
        ["Brass lamp"]
    );
    let jpg_only = r"^(?<set>[^/]+)/(?:[^/]+/)*(?<identifier>BHC_006_[A-Z]{2}_\d{3})(?:_(?<part>[a-z0-9]+))?\.jpg$";
    let after = derive(
        &bhc_index(),
        &Sheets::new(),
        &bhc_plan(jpg_only, decisions),
        &bhc_catalog(),
        &NoTags,
    );
    assert_eq!(
        texts(item(&after, "BHC_006_AF_001"), "dcterms:title"),
        ["Brass lamp"],
        "survives the new rule"
    );
    assert!(
        after
            .records
            .iter()
            .any(|record| record.kind == RecordKind::Item && matches!(record.fate, Fate::Left(_))),
        "the leave still applies"
    );
    let reasons: Vec<(usize, OrphanReason)> = after
        .orphans
        .iter()
        .map(|orphan| (orphan.decision, orphan.reason))
        .collect();
    assert_eq!(
        reasons,
        [(1, OrphanReason::WrongTarget)],
        "the tif is in no record now"
    );
}

#[test]
fn s16_after_a_resort_value_keyed_decisions_keep_their_rows_and_line_keyed_ones_are_orphans() {
    let original: &[u8] = b"Id,Title\nA1,One\nB2,Two\n";
    let resorted: &[u8] = b"Id,Title\nB2,Two\nA1,One\n";
    let make = |hash: &str| {
        plan(|plan| {
            plan["sheets"] = json!([
                sheet_plan("keyed", &["Id", "Title"]),
                sheet_plan("lines", &["Id", "Title"])
            ]);
            plan["sheets"][0]["itemKey"] = json!({ "index": 0, "header": "Id" });
            plan["sheets"][0]["hash"] = json!(hash);
            plan["sheets"][1]["hash"] = json!(hash);
            plan["slots"]["item"]["dcterms:title"] = rule(
                json!([col("keyed", 1, "Title"), col("lines", 1, "Title")]),
                "first",
            );
            let set = |sheet: &str, key: &str| {
                json!({ "kind": "set", "entry": { "row": { "sheet": sheet, "key": key } }, "target": "item",
                        "slot": "dcterms:description", "values": [{ "type": "literal", "text": "checked", "lang": null, "label": null }] })
            };
            plan["decisions"] = json!([set("keyed", "A1#1"), set("lines", "2")]);
        })
    };
    let plan = make(&omeka_media::store::sha256_hex(original));
    let sheets = read_sheets(&plan, &[("keyed", resorted), ("lines", resorted)]);
    let model = run(&index(&[]), &sheets, &plan);
    let checked: Vec<&str> = model
        .kept()
        .filter(|record| texts(record, "dcterms:description") == ["checked"])
        .flat_map(|record| texts(record, "dcterms:title"))
        .collect();
    assert_eq!(checked, ["One"], "the value-keyed decision follows row A1");
    let reasons: Vec<(usize, OrphanReason)> = model
        .orphans
        .iter()
        .map(|orphan| (orphan.decision, orphan.reason))
        .collect();
    assert_eq!(reasons, [(1, OrphanReason::LineKeyChanged)]);
    let unchanged = run(
        &index(&[]),
        &read_sheets(&plan, &[("keyed", original), ("lines", original)]),
        &plan,
    );
    assert!(
        unchanged.orphans.is_empty(),
        "same bytes: line keys still hold"
    );
}

fn on_file(path: &str) -> impl Fn(&&crate::import::model::Flag) -> bool + '_ {
    move |flag| matches!(&flag.subject, Subject::Entry(EntryRef::File(file)) if file.path.as_str() == path)
}

/// Views `a`, `b`, `c` … of one object: `X_010b.jpg` is view `b` of `X_010`.
const VIEWS: &str = r"^(?<identifier>X_\d{3})(?<part>[a-z])\.[^.]+$";

/// A sheet with one row per file, joined by file name.
fn per_file_sheet(plan: &mut serde_json::Value) {
    plan["grouping"]["pattern"] = json!(VIEWS);
    plan["sheets"] = json!([sheet_plan("views", &["File", "Keywords", "By"])]);
    plan["sheets"][0]["rowKind"] = json!("media");
    plan["sheets"][0]["join"] = join_on("fileName", 0, "File", false, false, false);
    plan["sheets"][0]["columns"][1]["options"]["split"] = json!(",");
}

#[test]
fn s17_a_per_file_catalogue_gives_the_item_its_media_rows_and_each_media_its_own() {
    let files = [
        ("X_010a.jpg", JPEG),
        ("X_010b.jpg", JPEG),
        ("X_010c.jpg", JPEG),
    ];
    let plan = plan(|plan| {
        per_file_sheet(plan);
        plan["slots"]["item"]["dcterms:subject"] =
            rule(json!([col("views", 1, "Keywords")]), "all");
        plan["slots"]["media"]["dcterms:creator"] = rule(json!([col("views", 2, "By")]), "first");
    });
    let csv: &[u8] = b"File,Keywords,By\nX_010c.jpg,Side View,Asha\nX_010b.jpg,\"Back View, Lamp\",Ravi\nX_010a.jpg,\"Lamp, Front View\",Dilip\n";
    let model = run(
        &index(&files),
        &read_sheets(&plan, &[("views", csv)]),
        &plan,
    );
    assert_eq!(
        kept(&model, RecordKind::Item).len(),
        1,
        "one item for three views"
    );
    let x = item(&model, "X_010");
    assert_eq!(
        texts(x, "dcterms:subject"),
        ["Lamp", "Front View", "Back View", "Side View"],
        "the union of the views' keywords, each once, in media order"
    );
    assert_eq!(texts(x, "dcterms:identifier"), ["X_010"]);
    let by: Vec<Vec<&str>> = x
        .media
        .iter()
        .map(|media| texts(model.record(*media), "dcterms:creator"))
        .collect();
    assert_eq!(
        by,
        [["Dilip"], ["Ravi"], ["Asha"]],
        "each view keeps its own row"
    );
}

#[test]
fn s20_an_item_with_its_own_row_in_a_sheet_reads_only_that_row_of_it() {
    let files = [
        ("X_010a.jpg", JPEG),
        ("X_010b.jpg", JPEG),
        ("X_010c.jpg", JPEG),
    ];
    let plan = plan(|plan| {
        plan["grouping"]["pattern"] = json!(VIEWS);
        plan["sheets"] = json!([sheet_plan("mixed", &["Kind", "File", "Title"])]);
        plan["sheets"][0]["rowKind"] = json!({ "column": { "index": 0, "header": "Kind" } });
        plan["sheets"][0]["join"] = join_on("fileName", 1, "File", false, false, false);
        plan["slots"]["item"]["dcterms:title"] = rule(json!([col("mixed", 2, "Title")]), "first");
        plan["slots"]["media"]["dcterms:title"] = rule(json!([col("mixed", 2, "Title")]), "first");
    });
    let csv: &[u8] =
        b"Kind,File,Title\nitem,X_010c.jpg,Lamp\nmedia,X_010a.jpg,Front\nmedia,X_010b.jpg,Back\n";
    let model = run(
        &index(&files),
        &read_sheets(&plan, &[("mixed", csv)]),
        &plan,
    );
    let x = item(&model, "X_010");
    assert_eq!(
        texts(x, "dcterms:title"),
        ["Lamp"],
        "the item's own row speaks for it; its media's rows of that sheet do not add to it"
    );
    assert!(
        flags(&model, FlagKind::ManyValues).is_empty(),
        "no many-values flag from media rows"
    );
}

#[test]
fn s18_an_identifier_rule_that_gives_nothing_falls_back_to_the_item_code() {
    let files = [("A1.jpg", JPEG), ("B2.jpg", JPEG), ("C3.jpg", JPEG)];
    let csv: &[u8] = b"Id,Code\nA1,ACC-1\nB2,\n";
    for slot in ["dcterms:identifier", "itemKey"] {
        let plan = plan(|plan| {
            plan["grouping"]["pattern"] = json!(PATTERN);
            plan["sheets"] = json!([sheet_plan("register", &["Id", "Code"])]);
            plan["sheets"][0]["join"] = join_on("identifier", 0, "Id", false, false, false);
            plan["slots"]["item"][slot] = rule(json!([col("register", 1, "Code")]), "first");
        });
        let model = run(
            &index(&files),
            &read_sheets(&plan, &[("register", csv)]),
            &plan,
        );
        let a1 = item(&model, "ACC-1");
        assert_eq!(
            media_paths(&model, a1),
            ["A1.jpg"],
            "{slot}: a value is kept"
        );
        for code in ["B2", "C3"] {
            assert_eq!(
                texts(item(&model, code), "dcterms:identifier"),
                [code],
                "{slot}: an empty cell or no row gives the group key"
            );
        }
    }
}

#[test]
fn s19_a_refused_file_a_list_row_names_is_flagged_with_the_row() {
    let files = [
        ("X_011a.jpg", JPEG),
        ("X_011b.NEF", BINARY),
        ("X_012a.NEF", BINARY),
    ];
    let mut catalog = catalog();
    catalog.files = FileValidation::Allow {
        media_types: [JPEG.to_string()].into(),
        extensions: ["jpg".to_string()].into(),
    };
    let plan = plan(|plan| {
        per_file_sheet(plan);
        plan["flags"] = json!({ "listed-not-media": "keep", "not-media": "keep" });
        plan["decisions"] = json!([{ "kind": "flag", "entry": { "file": { "source": "s1", "path": "X_011b.NEF" } },
                                     "flag": "listed-not-media", "slot": null, "policy": "keep" }]);
    });
    let csv: &[u8] = b"File,Keywords,By\nX_011a.jpg,Front,Dilip\nX_011b.NEF,Back,Dilip\n";
    let model = derive(
        &index(&files),
        &read_sheets(&plan, &[("views", csv)]),
        &plan,
        &catalog,
        &NoTags,
    );
    let listed = flags(&model, FlagKind::ListedNotMedia);
    assert_eq!(listed.len(), 1, "{listed:?}");
    assert!(on_file("X_011b.NEF")(&listed[0]));
    assert_eq!(listed[0].detail.as_deref(), Some("X_011b.NEF#1 .nef"));
    assert_eq!(
        listed[0].policy,
        Some(FlagPolicy::Leave),
        "keeping it cannot work: ingest refuses the file"
    );
    let not_media = flags(&model, FlagKind::NotMedia);
    assert!(not_media.iter().any(on_file("X_011b.NEF")));
    assert!(
        not_media.iter().any(on_file("X_012a.NEF")),
        "a refused file no row names is only not-media"
    );
    assert_eq!(
        media_paths(&model, item(&model, "X_011")),
        ["X_011a.jpg"],
        "the named NEF is left out"
    );
}
