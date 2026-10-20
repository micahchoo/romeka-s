//! The prototype's BHC_006 sample, end to end: a real folder on disk, the
//! register saved in Windows-1252, and the files' EXIF from a fake reader.

#![allow(clippy::expect_used)]

use std::collections::BTreeMap;

use serde_json::{Value, json};

use super::archive::{ArchiveFile, ArchiveRoot};
use super::catalog::{Catalog, Template, TemplateField};
use super::derive::{Model, derive};
use super::inputs::Inputs;
use super::plan::ImportPlan;
use super::preview::{PreviewContext, PreviewRequest, build};
use super::tags::{TagCache, TagReader};

const FILES: [&str; 14] = [
    "Artifacts/BHC_006_AF_001_front.jpg",
    "Artifacts/BHC_006_AF_001_back.jpg",
    "Artifacts/BHC_006_AF_002_front.jpg",
    "Artifacts/BHC_006_AF_002_back.jpg",
    "Artifacts/BHC_006_AF_003_front.tif",
    "Artifacts/BHC_006_AF_004_front.jpg",
    "Artifacts/scan0042.jpg",
    "Photographs/1950s/BHC_006_PH_101.tif",
    "Photographs/1950s/BHC_006_PH_102.tif",
    "Photographs/1960s/BHC_006_PH_201.jpg",
    "Photographs/1960s/BHC_006_PH_202.jpg",
    "Photographs/1960s/BHC_006_PH_202_verso.jpg",
    "_scratch/test.png",
    "_scratch/notes.txt",
];

const PATTERN: &str = r"^(?<set>[^/]+)/(?:[^/]+/)*(?<identifier>BHC_\d{3}_[A-Z]{2}_\d{3})(?:_(?<part>[a-z]+))?\.(?<ext>jpe?g|tif|png)$";

/// The register as a spreadsheet saves it: Windows-1252, semicolons, a title
/// line, one short line (AF_003) and one row no file matches (PH_999).
fn register() -> Vec<u8> {
    let mut bytes = Vec::new();
    for line in [
        &b"BHC 006 accession register, exported 12/03/2024"[..],
        b"Accession No;Title;Date;Shelf mark;Notes",
        b"BHC_006_AF_001;Brass lamp;1890;A-3-12;",
        b"BHC_006_AF_002;;1890-13-01;A-3-14;",
        b"BHC_006_AF_003;\"Tin box, \"\"dented\";1902;A-4-01",
        b"BHC_006_AF_004;Caf\xE9 sign;1925;A-4-07;Enamel",
        b"BHC_006_PH_101;  Harbour at dawn ;;B-1-01;",
        b"BHC_006_PH_102;Market street;1958;B-1-02;",
        b"BHC_006_PH_201;School parade;1963;B-2-01;",
        b"BHC_006_PH_202;Lighthouse keeper;1964;B-2-02;",
        b"BHC_006_PH_999;Missing photo;1965;B-2-09;",
    ] {
        bytes.extend(line);
        bytes.extend(b"\r\n");
    }
    bytes
}

struct Exif;

impl TagReader for Exif {
    fn read(&self, file: &ArchiveFile) -> BTreeMap<&'static str, String> {
        let nikon = |date: &str| {
            BTreeMap::from([
                ("DateTimeOriginal", date.to_string()),
                ("Artist", "BHC digitisation team".to_string()),
                ("Model", "NIKON D750".to_string()),
            ])
        };
        match file.path.as_str() {
            "Artifacts/BHC_006_AF_001_front.jpg" => nikon("2019:05:14 11:02:10"),
            "Artifacts/BHC_006_AF_001_back.jpg" => nikon("2019:05:14 11:03:41"),
            "Artifacts/BHC_006_AF_002_front.jpg" => nikon("2019:05:14 11:20:05"),
            "Photographs/1950s/BHC_006_PH_101.tif" => {
                BTreeMap::from([("DateTimeOriginal", "1957:03:02 06:10:00".to_string())])
            }
            "Photographs/1960s/BHC_006_PH_201.jpg" => {
                BTreeMap::from([("Artist", "S. Rao".to_string())])
            }
            _ => BTreeMap::new(),
        }
    }
}

pub(super) struct Sample {
    _dir: tempfile::TempDir,
    pub(super) inputs: Inputs,
}

pub(super) fn sample(plan: &ImportPlan) -> Sample {
    let dir = tempfile::tempdir().expect("tempdir");
    let folder = dir.path().join("BHC_006");
    for path in FILES {
        let full = folder.join(path);
        std::fs::create_dir_all(full.parent().expect("parent")).expect("mkdir");
        std::fs::write(&full, path.as_bytes()).expect("write");
    }
    std::fs::write(folder.join("BHC006_items.csv"), register()).expect("csv");
    let root = ArchiveRoot::new(dir.path()).expect("root");
    let inputs = Inputs::load(&root.folder("BHC_006").expect("folder"), plan).expect("inputs");
    Sample { _dir: dir, inputs }
}

pub(super) fn catalog() -> Catalog {
    let field = |term: &str, required| TemplateField {
        term: term.to_string(),
        required,
    };
    Catalog {
        item_sets: BTreeMap::from([("Photographs".to_string(), 4)]),
        templates: BTreeMap::from([(
            1,
            Template {
                label: "Base Resource".to_string(),
                fields: vec![
                    field("dcterms:title", true),
                    field("dcterms:identifier", false),
                    field("dcterms:description", false),
                ],
            },
        )]),
        default_template: Some(1),
        labels: BTreeMap::from([
            ("dcterms:title".to_string(), "Title".to_string()),
            ("dcterms:date".to_string(), "Date".to_string()),
            ("dcterms:identifier".to_string(), "Identifier".to_string()),
            ("dcterms:description".to_string(), "Description".to_string()),
            ("dcterms:source".to_string(), "Source".to_string()),
        ]),
        existing: BTreeMap::new(),
        open_undo: None,
    }
}

pub(super) fn plan(edit: impl FnOnce(&mut Value)) -> ImportPlan {
    let mut value = ImportPlan::new_draft().to_json();
    value["pattern"] = json!(PATTERN);
    value["csv"] = json!({
        "file": "BHC006_items.csv", "delimiter": ";", "headerRow": 2, "windows1252": true,
        "trim": false, "joinColumn": "Accession No",
        "columns": { "Title": "dcterms:title", "Date": "dcterms:date" }
    });
    edit(&mut value);
    ImportPlan::from_json(&value).expect("plan")
}

fn preview(plan: &ImportPlan, request: Value) -> Value {
    let sample = sample(plan);
    let tags = TagCache::new(&Exif);
    let catalog = catalog();
    let model = derive(plan, &sample.inputs, &tags, &catalog);
    let request: PreviewRequest = serde_json::from_value(request).expect("request");
    let context = PreviewContext {
        plan,
        inputs: &sample.inputs,
        model: &model,
        tags: &tags,
        catalog: &catalog,
    };
    serde_json::to_value(build(&context, &request)).expect("json")
}

fn model(plan: &ImportPlan) -> Model {
    let sample = sample(plan);
    derive(plan, &sample.inputs, &TagCache::new(&Exif), &catalog())
}

fn identifiers(rows: &Value) -> Vec<String> {
    rows["page"]
        .as_array()
        .expect("page")
        .iter()
        .map(|row| row["identifier"].as_str().expect("id").to_string())
        .collect()
}

#[test]
fn eleven_of_fourteen_files_match_and_make_eight_items() {
    let out = preview(&plan(|_| {}), json!({ "include": ["files", "sets"] }));
    let files = &out["files"];
    assert_eq!(
        (
            files["total"].as_u64(),
            files["matched"].as_u64(),
            files["items"].as_u64()
        ),
        (Some(14), Some(11), Some(8))
    );
    assert_eq!(
        files["groupNames"],
        json!(["set", "identifier", "part", "ext"])
    );
    assert_eq!(files["buckets"], json!({ "1": 5, "2": 3 }));
    assert_eq!(
        files["biggest"],
        json!({ "identifier": "BHC_006_AF_001", "files": 2 })
    );
    assert_eq!(
        files["page"][0],
        json!({ "path": "Artifacts/BHC_006_AF_001_back.jpg", "matched": true, "item": 1, "itemFiles": 2,
                "groups": { "set": [0, 9], "identifier": [10, 24], "part": [25, 29], "ext": [30, 33] } })
    );
    assert_eq!(
        out["sets"],
        json!([
            { "key": "Artifacts", "name": "Artifacts", "use": true, "matched": 6, "total": 7, "items": 4, "existing": false },
            { "key": "Photographs", "name": "Photographs", "use": true, "matched": 5, "total": 5, "items": 4, "existing": true },
            { "key": "_scratch", "name": "_scratch", "use": false, "matched": 0, "total": 2, "items": 0, "existing": false }
        ])
    );
}

#[test]
fn the_file_filters_page_the_right_files() {
    let plan = plan(|_| {});
    let two = preview(
        &plan,
        json!({ "include": ["files"], "files": { "perItem": 2, "offset": 0, "limit": 50 } }),
    );
    assert_eq!(two["files"]["filtered"], 6);
    let miss = preview(
        &plan,
        json!({ "include": ["files"], "files": { "filter": "miss", "offset": 1, "limit": 1 } }),
    );
    assert_eq!(miss["files"]["filtered"], 3);
    assert_eq!(
        miss["files"]["page"],
        json!([{ "path": "_scratch/notes.txt", "matched": false, "item": null, "itemFiles": 0, "groups": {} }])
    );
}

#[test]
fn the_register_is_detected_repaired_and_joined() {
    let out = preview(&plan(|_| {}), json!({ "include": ["csv"] }));
    let csv = &out["csv"];
    assert_eq!(csv["files"], json!(["BHC006_items.csv"]));
    assert_eq!(csv["detected"], json!({ "delimiter": ";", "headerRow": 2 }));
    assert_eq!(
        csv["head"],
        json!(["Accession No", "Title", "Date", "Shelf mark", "Notes"])
    );
    assert_eq!(
        csv["rawLines"][5],
        "BHC_006_AF_004;Café sign;1925;A-4-07;Enamel"
    );
    assert_eq!(
        (
            csv["cellsLookLike1252"].as_u64(),
            csv["cellsWithSpaces"].as_u64()
        ),
        (Some(1), Some(1))
    );
    assert_eq!(csv["rowCountError"], Value::Null);
    assert_eq!(
        (
            csv["joined"].as_u64(),
            csv["rowsWithoutFile"].as_u64(),
            csv["itemsWithoutRow"].as_u64()
        ),
        (Some(8), Some(1), Some(0))
    );

    let title_line = preview(
        &plan(|p| p["csv"]["headerRow"] = json!(1)),
        json!({ "include": ["csv"] }),
    );
    assert_eq!(title_line["csv"]["rowCountError"], "headerRow");
    assert_eq!(title_line["error"], json!({ "key": "headerRow" }));
    let tab = preview(
        &plan(|p| p["csv"]["delimiter"] = json!("\t")),
        json!({ "include": ["csv"] }),
    );
    assert_eq!(tab["csv"]["rowCountError"], "delimiter");
}

#[test]
fn problem_kinds_count_each_kind_once_per_row() {
    let out = preview(&plan(|_| {}), json!({ "include": ["rows"] }));
    let rows = &out["rows"];
    assert_eq!(
        (rows["total"].as_u64(), rows["withProblems"].as_u64()),
        (Some(9), Some(4))
    );
    assert_eq!(
        rows["kinds"],
        json!([
            { "kind": "empty", "term": "dcterms:title", "count": 1 },
            { "kind": "date", "term": "dcterms:date", "count": 1 },
            { "kind": "space", "term": "dcterms:title", "count": 1 },
            { "kind": "cells", "term": null, "count": 1 },
            { "kind": "nofile", "term": null, "count": 1 }
        ])
    );
    assert_eq!(
        rows["columns"],
        json!([
            { "term": "dcterms:title", "label": "Title", "sources": ["type", "csv:Title"] },
            { "term": "dcterms:description", "label": "Description", "sources": ["type"] },
            { "term": "dcterms:date", "label": "Date", "sources": ["csv:Date"] }
        ])
    );
    assert_eq!(rows["unmapped"], json!(["Shelf mark", "Notes"]));
    let af_004 = &rows["page"][3];
    assert_eq!(
        af_004["cells"][0],
        json!({ "term": "dcterms:title", "value": "Café sign", "source": "csv", "problems": [] })
    );
    assert_eq!(af_004["extra"], json!(["A-4-07", "Enamel"]));
    assert_eq!(
        rows["page"][2]["rowProblems"],
        json!([{ "kind": "cells", "line": 5, "cells": 4 }])
    );
    assert_eq!(
        rows["page"][8]["rowProblems"],
        json!([{ "kind": "nofile" }])
    );

    let unrepaired = preview(
        &plan(|p| p["csv"]["windows1252"] = json!(false)),
        json!({ "include": ["rows"], "rows": { "kind": "encoding" } }),
    );
    assert_eq!(identifiers(&unrepaired["rows"]), ["BHC_006_AF_004"]);
}

#[test]
fn choices_clear_the_problems_they_answer() {
    let plan = plan(|p| {
        p["csv"]["trim"] = json!(true);
        p["csvRow"] = json!({ "BHC_006_AF_003": "keep" });
        p["withoutFiles"] = json!(["BHC_006_PH_999"]);
        p["edits"] = json!({ "BHC_006_AF_002": { "dcterms:title": "Copper kettle", "dcterms:date": "1890-12-01" } });
    });
    let model = model(&plan);
    assert!(model.kinds().is_empty(), "{:?}", model.kinds());
    assert_eq!(model.check(&plan, &catalog()).create, 9);
}

#[test]
fn the_rows_filters_page_the_right_rows() {
    let plan = plan(|_| {});
    let page = |rows: Value| {
        identifiers(&preview(&plan, json!({ "include": ["rows"], "rows": rows }))["rows"])
    };
    assert_eq!(
        page(json!({ "offset": 2, "limit": 3 })),
        ["BHC_006_AF_003", "BHC_006_AF_004", "BHC_006_PH_101"]
    );
    assert_eq!(
        page(json!({ "problemsOnly": true })),
        [
            "BHC_006_AF_002",
            "BHC_006_AF_003",
            "BHC_006_PH_101",
            "BHC_006_PH_999"
        ]
    );
    assert_eq!(
        page(json!({ "kind": "date", "term": "dcterms:date" })),
        ["BHC_006_AF_002"]
    );
    assert_eq!(
        page(json!({ "kind": "space", "term": "dcterms:date" })),
        Vec::<String>::new()
    );
    assert_eq!(
        page(json!({ "query": " ph_2 " })),
        ["BHC_006_PH_201", "BHC_006_PH_202"]
    );
    assert_eq!(page(json!({ "offset": 0, "limit": 500 })).len(), 9);
    let bad: Result<PreviewRequest, _> =
        serde_json::from_value(json!({ "include": ["rows"], "rows": { "colour": 1 } }));
    assert!(bad.is_err());
}

#[test]
fn the_check_leaves_out_rows_with_problems() {
    let out = preview(&plan(|_| {}), json!({ "include": ["check"] }));
    assert_eq!(
        out["check"],
        json!({ "create": 5, "files": 7, "newSets": ["Artifacts"], "existingSets": ["Photographs"], "newTypes": [],
                "newFields": [], "withProblems": 4, "skipped": 0, "existing": 0, "unmatchedFiles": 3,
                "unmatchedSample": ["Artifacts/scan0042.jpg", "_scratch/notes.txt", "_scratch/test.png"],
                "canRun": true, "openUndo": null })
    );
    let fix = preview(
        &plan(|p| p["problemRows"] = json!("fix")),
        json!({ "include": ["check"] }),
    );
    assert_eq!(
        (
            fix["check"]["create"].as_u64(),
            fix["check"]["canRun"].as_bool()
        ),
        (Some(9), Some(false))
    );
}

#[test]
fn exif_date_time_original_becomes_an_iso_date_in_a_date_field() {
    let plan = plan(|p| {
        p["meta"] = json!([
            { "tag": "DateTimeOriginal", "term": "dcterms:created", "on": "item", "from": "part", "part": "front" },
            { "tag": "DateTimeOriginal", "term": "dcterms:description", "on": "media", "from": "first" },
            { "tag": "Artist", "term": "dcterms:creator", "on": "item", "from": "all" }
        ]);
        p["typeFor"] = json!({ "Photographs": { "starter": "photograph", "name": "Photograph" } });
    });
    let out = preview(&plan, json!({ "include": ["rows", "tags"] }));
    let cell = |row: usize, term: &str| {
        out["rows"]["page"][row]["cells"]
            .as_array()
            .expect("cells")
            .iter()
            .find(|c| c["term"] == term)
            .cloned()
            .expect("cell")
    };
    assert_eq!(cell(0, "dcterms:created")["value"], "2019-05-14");
    assert_eq!(cell(0, "dcterms:created")["source"], "file");
    assert_eq!(cell(4, "dcterms:created")["value"], "1957-03-02");
    assert_eq!(cell(0, "dcterms:creator")["value"], "BHC digitisation team");
    assert_eq!(out["rows"]["page"][4]["type"], "Photograph");
    assert_eq!(
        out["tags"][0],
        json!({ "tag": "DateTimeOriginal", "kind": "exif", "found": 4, "sample": "2019:05:14 11:03:41" })
    );

    let sample = sample(&plan);
    let tags = TagCache::new(&Exif);
    let model = derive(&plan, &sample.inputs, &tags, &catalog());
    let sources = super::values::Sources {
        plan: &plan,
        sheet: model.sheet.as_ref().and_then(|s| s.as_ref().ok()),
        files: &model.files,
        tags: &tags,
    };
    assert_eq!(
        sources.media_values(&model.files[0]),
        [(
            "dcterms:description".to_string(),
            "2019:05:14 11:03:41".to_string()
        )]
    );
}

#[test]
fn an_invalid_pattern_is_the_preview_error_and_blocks_the_run() {
    let out = preview(
        &plan(|p| p["pattern"] = json!("^(?<identifier>a)(?=b)")),
        json!({ "include": ["files", "check"] }),
    );
    assert_eq!(out["error"]["key"], "patternSyntax");
    assert_eq!(out["files"]["matched"], 0);
    assert_eq!(out["check"]["canRun"], false);
    let none = preview(
        &plan(|p| p["pattern"] = json!("^(?<id>.+)$")),
        json!({ "include": [] }),
    );
    assert_eq!(none, json!({ "error": { "key": "patternIdentifier" } }));
}

#[test]
fn the_disk_preview_reads_file_facts() {
    let plan = plan(|_| {});
    let sample = sample(&plan);
    let folder = sample.inputs.files[0]
        .absolute
        .parent()
        .and_then(|p| p.parent())
        .expect("folder")
        .to_path_buf();
    let root = ArchiveRoot::new(folder.parent().expect("root")).expect("root");
    let request: PreviewRequest =
        serde_json::from_value(json!({ "include": ["tags"] })).expect("request");
    let out = super::preview(
        &root.folder("BHC_006").expect("folder"),
        &plan,
        &catalog(),
        &request,
    )
    .expect("preview");
    let tags = out.tags.expect("tags");
    let size = tags.iter().find(|t| t.tag == "Size").expect("size");
    assert_eq!((size.found, size.sample.as_str()), (11, "33 B"));
    assert_eq!(tags[0].found, 0);
}

#[test]
fn a_column_mapped_to_an_unknown_term_blocks_the_check() {
    let out = preview(
        &plan(|p| p["csv"]["columns"]["Title"] = json!("dcterms:nonsense")),
        json!({ "include": ["check"] }),
    );
    assert_eq!(
        out["error"],
        json!({ "key": "termUnknown", "detail": "dcterms:nonsense" })
    );
    assert_eq!(out["check"]["canRun"], false);
}

#[test]
fn a_new_type_named_like_an_existing_template_blocks_the_check() {
    let out = preview(
        &plan(|p| {
            p["typeFor"] =
                json!({ "Artifacts": { "starter": "other", "name": " Base Resource " } });
        }),
        json!({ "include": ["check"] }),
    );
    assert_eq!(
        out["error"],
        json!({ "key": "typeExists", "detail": "Base Resource" })
    );
    assert_eq!(out["check"]["canRun"], false);
}

#[test]
fn a_request_plan_replaces_the_stored_one_and_limit_zero_answers_counts() {
    let stored = plan(|_| {});
    let sample = sample(&stored);
    let folder = sample.inputs.files[0]
        .absolute
        .parent()
        .and_then(|p| p.parent())
        .expect("folder")
        .to_path_buf();
    let root = ArchiveRoot::new(folder.parent().expect("root")).expect("root");
    let mut candidate = stored.to_json();
    candidate["pattern"] = json!(r"^Artifacts/(?<identifier>BHC_\d{3}_AF_\d{3})_front\.jpg$");
    let request: PreviewRequest = serde_json::from_value(json!({
        "include": ["files"],
        "files": { "offset": 0, "limit": 0 },
        "plan": candidate,
    }))
    .expect("request");
    let out = super::preview(
        &root.folder("BHC_006").expect("folder"),
        &stored,
        &catalog(),
        &request,
    )
    .expect("preview");
    let files = out.files.expect("files");
    assert_eq!((files.total, files.matched, files.items), (14, 3, 3));
    assert!(files.page.is_empty());
    let bad: Result<PreviewRequest, _> =
        serde_json::from_value(json!({ "include": [], "plan": { "pattern": 1 } }));
    assert!(bad.is_err(), "the override passes the plan boundary");
}

#[test]
fn a_rows_problem_count_is_its_cell_problems_plus_its_row_problems() {
    let out = preview(
        &plan(|_| {}),
        json!({ "include": ["rows"], "rows": { "offset": 0, "limit": 200 } }),
    );
    let page = out["rows"]["page"].as_array().expect("page");
    let mut with_row_problems = 0;
    for row in page {
        let cells: usize = row["cells"]
            .as_array()
            .expect("cells")
            .iter()
            .map(|cell| cell["problems"].as_array().expect("problems").len())
            .sum();
        let rows = row["rowProblems"].as_array().expect("row problems").len();
        with_row_problems += usize::from(rows > 0);
        assert_eq!(
            row["problems"].as_u64(),
            Some((cells + rows) as u64),
            "{}",
            row["identifier"]
        );
    }
    assert!(
        with_row_problems > 0,
        "the sample has a row problem to count"
    );
}
