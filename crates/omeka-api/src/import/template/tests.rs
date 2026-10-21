#![allow(clippy::expect_used, clippy::unwrap_used)]

use serde_json::json;

use super::*;
use crate::import::catalog::Catalog;
use crate::import::derive::{Index, IndexedSource, NoTags, Sheets, derive};
use crate::import::evidence::{FileEntry, Walk};
use crate::import::plan::{RecordKind, default_plan};
use crate::import::sheet;

fn catalog() -> Catalog {
    let mut catalog = Catalog::default();
    for (term, label) in [("dcterms:title", "Title"), ("dcterms:date", "Date")] {
        catalog
            .properties
            .insert(Term::parse(term).unwrap(), label.to_string());
    }
    catalog
}

fn index(paths: &[&str]) -> Index {
    Index {
        sources: vec![IndexedSource {
            id: SourceId("s1".into()),
            walk: Walk {
                entries: paths
                    .iter()
                    .map(|path| FileEntry {
                        path: RelPath::parse(path).unwrap(),
                        size: 1,
                        modified: None,
                        sniffed: "image/jpeg".into(),
                        decoded: None,
                    })
                    .collect(),
                ..Walk::default()
            },
        }],
    }
}

fn plan() -> ImportPlan {
    let plan = default_plan(vec![SourcePlan::Folder {
        id: SourceId("s1".into()),
        root: "archive".into(),
        path: RelPath::parse("").unwrap(),
    }])
    .unwrap();
    let mut value = serde_json::to_value(plan).unwrap();
    value["grouping"]["pattern"] =
        json!(r"^(?:.*/)?(?<identifier>[A-Z]+_\d+)(?:_(?<part>[a-z]+))?\.jpg$");
    value["slots"]["item"]["dcterms:title"] = json!({
        "sources": [{ "from": "constant", "value": "Untitled, \"draft\"" }], "combine": "first"
    });
    ImportPlan::from_json(&value.to_string()).unwrap()
}

fn columns(texts: &[&str]) -> Vec<ListColumn> {
    texts
        .iter()
        .map(|text| ListColumn::parse(text, &catalog()).unwrap())
        .collect()
}

#[test]
fn the_template_has_one_row_per_grouped_item_with_bom_brackets_and_values() {
    let index = index(&[
        "Box 1/AF_001_front.jpg",
        "Box 1/AF_001_back.jpg",
        "AF_002.jpg",
    ]);
    let plan = plan();
    let model = derive(&index, &Sheets::new(), &plan, &catalog(), &NoTags);
    let csv = template_csv(
        &model,
        &catalog(),
        &columns(&[
            "dcterms:title",
            "dcterms:date",
            "sets",
            "template",
            "visibility",
        ]),
    );
    let text = String::from_utf8(csv).unwrap();
    assert!(text.starts_with('\u{feff}'), "a byte order mark");
    let lines: Vec<&str> = text.trim_start_matches('\u{feff}').split("\r\n").collect();
    assert_eq!(
        lines[0],
        "Identifier,Files,Title [dcterms:title],Date [dcterms:date],Item sets [sets],Item type [template],Visibility [visibility]"
    );
    assert_eq!(
        lines[1],
        "AF_001,AF_001_back.jpg|AF_001_front.jpg,\"Untitled, \"\"draft\"\"\",,,,private"
    );
    assert_eq!(
        lines[2],
        "AF_002,AF_002.jpg,\"Untitled, \"\"draft\"\"\",,,,private"
    );
    assert_eq!(lines[3], "", "every row ends in CRLF");
    assert_eq!(lines.len(), 4);
}

#[test]
fn a_column_must_be_a_property_of_the_install_or_a_list_slot() {
    assert!(ListColumn::parse("dcterms:title", &catalog()).is_ok());
    for refused in ["dcterms:nope", "owner", "mediaOrder", "nonsense"] {
        assert_eq!(
            ListColumn::parse(refused, &catalog()),
            Err(ListError::UnknownColumn(refused.into())),
            "{refused}"
        );
    }
}

#[test]
fn a_filled_list_maps_its_bracketed_columns_first_and_its_values_reach_the_records() {
    let index = index(&["AF_001.jpg", "AF_002.jpg", "AF_003.jpg"]);
    let mut plan = plan();
    let model = derive(&index, &Sheets::new(), &plan, &catalog(), &NoTags);
    let template = template_csv(
        &model,
        &catalog(),
        &columns(&["dcterms:title", "dcterms:date"]),
    );
    let template = String::from_utf8(template).unwrap();
    let header = template.split("\r\n").next().unwrap();
    assert_eq!(
        header,
        "\u{feff}Identifier,Files,Title [dcterms:title],Date [dcterms:date]"
    );
    let filled = format!(
        "{header},Notes\r\nAF_001,AF_001.jpg,दीपक|Lamp,1952,x\r\nAF_002,AF_002.jpg,Cup,,\r\nAF_003,AF_003.jpg,,,\r\n"
    );
    let read = list_read(false);
    let sheet = sheet::read(filled.as_bytes(), &read, None).unwrap();
    let added = add_list(
        &mut plan,
        RelPath::parse("lists/1.csv").unwrap(),
        read,
        &sheet,
        &catalog(),
    )
    .unwrap();
    assert_eq!(added.sheet, SheetId("list-1".into()));
    assert_eq!(
        added.mapped,
        [2, 3],
        "Identifier, Files and Notes stay unmapped"
    );
    assert!(matches!(&plan.sources[1], SourcePlan::Staging { id } if id.0 == STAGING));
    let series = plan.structure.series(plan.structure.sole());
    let title = &series.rules(RecordKind::Item)[&SlotName::parse("dcterms:title").unwrap()];
    assert!(
        matches!(&title.sources[0], SourceRef::Column { sheet, .. } if sheet.0 == "list-1"),
        "the list first"
    );
    assert_eq!(title.sources.len(), 2, "the constant stays behind it");
    let plan = ImportPlan::from_json(&plan.to_json()).expect("the boundary takes the plan");

    let (_, list_plan) = plan.structure.lists().next().unwrap();
    let sheets: Sheets = [(
        list_plan.id.clone(),
        sheet::read(
            filled.as_bytes(),
            &list_plan.read,
            sheet::key_column(list_plan),
        ),
    )]
    .into_iter()
    .collect();
    let model = derive(&index, &sheets, &plan, &catalog(), &NoTags);
    let texts = |key: &str, term: &str| -> Vec<String> {
        let record = model
            .kept()
            .find(|record| {
                record.kind == RecordKind::Item && record.resolved.item_key.as_deref() == Some(key)
            })
            .unwrap();
        record
            .slots
            .get(&SlotName::parse(term).unwrap())
            .map(|values| {
                values
                    .iter()
                    .map(|value| value.value.text.clone())
                    .collect()
            })
            .unwrap_or_default()
    };
    assert_eq!(
        texts("AF_001", "dcterms:title"),
        ["दीपक", "Lamp"],
        "split on |"
    );
    assert_eq!(texts("AF_001", "dcterms:date"), ["1952"]);
    assert_eq!(texts("AF_002", "dcterms:title"), ["Cup"]);
    assert_eq!(
        texts("AF_003", "dcterms:title"),
        ["Untitled, \"draft\""],
        "an unfilled row keeps the next source"
    );
}

#[test]
fn a_list_without_an_identifier_column_is_refused() {
    let read = list_read(true);
    let sheet = sheet::read(b"Name\tTitle [dcterms:title]\nA\tB\n", &read, None).unwrap();
    let mut plan = plan();
    assert_eq!(
        add_list(
            &mut plan,
            RelPath::parse("lists/1.tsv").unwrap(),
            read,
            &sheet,
            &catalog()
        ),
        Err(ListError::NoIdentifier)
    );
    assert_eq!(bracketed("Title [dcterms:title] "), Some("dcterms:title"));
    assert_eq!(bracketed("Title"), None);
}

fn plan_titled(title: &str) -> ImportPlan {
    let mut value = serde_json::to_value(plan()).unwrap();
    value["slots"]["item"]["dcterms:title"]["sources"][0]["value"] = json!(title);
    ImportPlan::from_json(&value.to_string()).unwrap()
}

fn title_cell(title: &str) -> (String, String) {
    let index = index(&["AF_001.jpg"]);
    let plan = plan_titled(title);
    let model = derive(&index, &Sheets::new(), &plan, &catalog(), &NoTags);
    let csv = template_csv(&model, &catalog(), &columns(&["dcterms:title"]));
    let text = String::from_utf8(csv).unwrap();
    let text = text.trim_start_matches('\u{feff}');
    let read = list_read(false);
    let mut sheet = sheet::read(text.as_bytes(), &read, None).unwrap();
    let raw = sheet.rows[0].cells[2].clone();
    unescape_list(&mut sheet, None);
    (raw, sheet.rows[0].cells[2].clone())
}

#[test]
fn a_cell_a_spreadsheet_would_run_as_a_formula_leaves_the_template_with_a_leading_quote() {
    for (value, written) in [
        ("=1+1", "'=1+1"),
        ("+1", "'+1"),
        ("-5", "'-5"),
        ("@SUM(A1)", "'@SUM(A1)"),
    ] {
        let index = index(&["AF_001.jpg"]);
        let plan = plan_titled(value);
        let model = derive(&index, &Sheets::new(), &plan, &catalog(), &NoTags);
        let text = String::from_utf8(template_csv(
            &model,
            &catalog(),
            &columns(&["dcterms:title"]),
        ))
        .unwrap();
        let line = text.split("\r\n").nth(1).unwrap();
        let cell = line.strip_prefix("AF_001,AF_001.jpg,").unwrap();
        let cell = cell.trim_matches('"');
        assert_eq!(cell, written, "{value:?}");
    }
}

#[test]
fn a_leading_tab_or_carriage_return_is_guarded_too() {
    assert_eq!(guard("\tx"), "'\tx");
    assert_eq!(guard("\rx"), "'\rx");
    assert_eq!(guard("x\t"), "x\t");
}

#[test]
fn the_template_leaves_safe_text_alone() {
    let index = index(&["AF_001.jpg"]);
    let plan = plan_titled("दीपक");
    let model = derive(&index, &Sheets::new(), &plan, &catalog(), &NoTags);
    let text = String::from_utf8(template_csv(
        &model,
        &catalog(),
        &columns(&["dcterms:title"]),
    ))
    .unwrap();
    assert!(text.contains("AF_001,AF_001.jpg,दीपक\r\n"), "{text}");
}

#[test]
fn a_file_name_that_starts_like_a_formula_is_quoted_in_the_files_column() {
    let index = index(&["=cmd.jpg"]);
    let mut value = serde_json::to_value(plan()).unwrap();
    value["grouping"]["pattern"] = json!(r"^(?<identifier>.+)\.jpg$");
    let plan = ImportPlan::from_json(&value.to_string()).unwrap();
    let model = derive(&index, &Sheets::new(), &plan, &catalog(), &NoTags);
    let text = String::from_utf8(template_csv(&model, &catalog(), &[])).unwrap();
    let line = text.split("\r\n").nth(1).unwrap();
    assert!(line.ends_with(",'=cmd.jpg"), "{line}");
}

#[test]
fn the_quote_comes_off_on_upload_so_every_value_round_trips() {
    for value in [
        "=1+1",
        "+1",
        "-5",
        "@x",
        "-दीपक",
        "=दीपक",
        "'-5",
        "'=x",
        "'plain",
        "'",
        "-",
        "दीपक",
    ] {
        let (raw, back) = title_cell(value);
        assert_eq!(back, value, "{value:?} written as {raw:?}");
    }
}

#[test]
fn only_a_quote_before_a_formula_character_comes_off_and_only_in_the_identifier_and_bracketed_columns()
 {
    let read = list_read(false);
    let text = "Identifier,Files,Title [dcterms:title],Notes\r\n'-5,'-5,'-5,'-5\r\nB,,'x,'=y\r\n";
    let mut sheet = sheet::read(text.as_bytes(), &read, None).unwrap();
    unescape_list(&mut sheet, None);
    assert_eq!(sheet.rows[0].cells, ["-5", "'-5", "-5", "'-5"]);
    assert_eq!(sheet.rows[1].cells, ["B", "", "'x", "'=y"]);
}

#[test]
fn an_identifier_that_starts_like_a_formula_round_trips_and_joins_its_item() {
    let index = index(&["-001.jpg", "=X.jpg"]);
    let mut value = serde_json::to_value(plan()).unwrap();
    value["grouping"]["pattern"] = json!(r"^(?<identifier>[^/]+)\.jpg$");
    let mut plan = ImportPlan::from_json(&value.to_string()).unwrap();
    let model = derive(&index, &Sheets::new(), &plan, &catalog(), &NoTags);
    let text = String::from_utf8(template_csv(
        &model,
        &catalog(),
        &columns(&["dcterms:title"]),
    ))
    .unwrap();
    let lines: Vec<&str> = text.split("\r\n").collect();
    assert_eq!(lines[1], "'-001,'-001.jpg,\"Untitled, \"\"draft\"\"\"");
    assert_eq!(lines[2], "'=X,'=X.jpg,\"Untitled, \"\"draft\"\"\"");

    let filled = format!(
        "{}\r\n'-001,'-001.jpg,Lamp\r\n'=X,'=X.jpg,Bowl\r\n",
        lines[0].trim_start_matches('\u{feff}')
    );
    let read = list_read(false);
    let key = ColumnRef {
        index: 0,
        header: IDENTIFIER.to_string(),
    };
    let mut sheet = sheet::read(filled.as_bytes(), &read, Some(&key)).unwrap();
    let added = add_list(
        &mut plan,
        RelPath::parse("lists/1.csv").unwrap(),
        read,
        &sheet,
        &catalog(),
    )
    .unwrap();
    unescape_list(
        &mut sheet,
        sheet::key_column(plan.structure.lists().next().unwrap().1),
    );
    assert_eq!(sheet.rows[0].cells[0], "-001");
    assert_eq!(sheet.rows[0].key.as_str(), "-001#1");
    assert_eq!(sheet.rows[1].cells[0], "=X");

    let sheets: Sheets = [(added.sheet, Ok(sheet))].into_iter().collect();
    let model = derive(&index, &sheets, &plan, &catalog(), &NoTags);
    let title = |key: &str| {
        model
            .kept()
            .find(|record| {
                record.kind == RecordKind::Item && record.resolved.item_key.as_deref() == Some(key)
            })
            .and_then(|record| {
                record.slots.get(
                    &ListColumn::parse("dcterms:title", &catalog())
                        .unwrap()
                        .slot(),
                )
            })
            .map(|values| values[0].value.text.clone())
    };
    assert_eq!(title("-001").as_deref(), Some("Lamp"));
    assert_eq!(title("=X").as_deref(), Some("Bowl"));
}
