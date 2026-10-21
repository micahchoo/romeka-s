//! Archive shapes and the outcome an archivist wants from each, compared
//! through the preview route. Only `plan_for_*` knows the plan format, so the
//! shapes and their expected outcomes survive a new plan version.

#![allow(clippy::unwrap_used, clippy::expect_used, dead_code)]

pub mod bhc006;
pub mod born_digital;
pub mod oral_history;

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde_json::{Value, json};

use omeka_db::repos::{PropertyRepo, VocabularyRepo};
use omeka_db::rows::NewProperty;
use omeka_db::with_db;

use crate::imports_fixture::{Fixture, write_tree};

/// File headers that sniff as their kind.
pub mod kind {
    pub const JPEG: &[u8] = b"\xFF\xD8\xFF\xE0\x00\x10JFIF\x00\x01\x01\x00\x00\x01\x00\x01\x00\x00";
    pub const TIFF: &[u8] = b"II*\x00\x08\x00\x00\x00\x00\x00";
    pub const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR";
    pub const PDF: &[u8] = b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n";
    pub const MP3: &[u8] = b"ID3\x03\x00\x00\x00\x00\x00\x00";
    pub const ZIP: &[u8] = b"PK\x03\x04\x14\x00\x06\x00";
    pub const AVI: &[u8] = b"RIFF\x00\x00\x00\x00AVI LIST";
    pub const MOV: &[u8] = b"\x00\x00\x00\x14ftypqt  \x00\x00\x00\x00qt  ";
}

/// A sheet cell as the cataloguer typed it.
#[derive(Clone, Debug)]
pub enum Cell {
    Text(String),
    Number(f64),
    Blank,
}

/// A list file: `.xlsx` is written as a workbook, anything else as CSV.
#[derive(Clone, Debug)]
pub struct Sheet {
    pub path: String,
    pub header: Vec<&'static str>,
    pub rows: Vec<Vec<Cell>>,
}

/// An archive as files with their bytes, plus the lists in it.
#[derive(Default)]
pub struct Shape {
    pub files: Vec<(String, Vec<u8>)>,
    pub sheets: Vec<Sheet>,
}

impl Shape {
    /// A file whose bytes are `header` then its path, so no two files are
    /// identical by accident (the derive keeps one of identical files).
    pub fn object(&mut self, path: &str, header: &[u8]) {
        self.files
            .push((path.to_string(), [header, path.as_bytes()].concat()));
    }

    /// A file with exactly these bytes.
    pub fn raw(&mut self, path: &str, bytes: &[u8]) {
        self.files.push((path.to_string(), bytes.to_vec()));
    }

    pub fn sheet(&mut self, sheet: Sheet) {
        self.sheets.push(sheet);
    }
}

pub fn materialize(shape: &Shape, root: &Path) {
    for (path, bytes) in &shape.files {
        write_tree(root, &[(path, bytes)]);
    }
    for sheet in &shape.sheets {
        let bytes = if sheet.path.ends_with(".xlsx") {
            xlsx(sheet)
        } else {
            csv(sheet)
        };
        write_tree(root, &[(&sheet.path, &bytes)]);
    }
}

fn xlsx(sheet: &Sheet) -> Vec<u8> {
    let mut book = rust_xlsxwriter::Workbook::new();
    let page = book.add_worksheet();
    for (col, text) in (0_u16..).zip(&sheet.header) {
        page.write(0, col, *text).unwrap();
    }
    for (line, row) in (1_u32..).zip(&sheet.rows) {
        for (col, cell) in (0_u16..).zip(row) {
            match cell {
                Cell::Text(text) => page.write(line, col, text.as_str()).map(|_| ()),
                Cell::Number(number) => page.write(line, col, *number).map(|_| ()),
                Cell::Blank => Ok(()),
            }
            .unwrap();
        }
    }
    book.save_to_buffer().unwrap()
}

fn csv(sheet: &Sheet) -> Vec<u8> {
    let quote = |text: &str| {
        if text.contains([',', '"', '\n']) {
            format!("\"{}\"", text.replace('"', "\"\""))
        } else {
            text.to_string()
        }
    };
    let mut out = sheet.header.join(",");
    out.push('\n');
    for row in &sheet.rows {
        let cells: Vec<String> = row
            .iter()
            .map(|cell| match cell {
                Cell::Text(text) => quote(text),
                Cell::Number(number) => number.to_string(),
                Cell::Blank => String::new(),
            })
            .collect();
        out.push_str(&cells.join(","));
        out.push('\n');
    }
    out.into_bytes()
}

/// One media of an item: its file and the values of the named slots.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Media {
    pub path: String,
    pub values: BTreeMap<String, Vec<String>>,
}

/// One item as an archivist reads it. Values are sorted, not deduplicated,
/// so a value written twice shows twice.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Item {
    pub title: Option<String>,
    pub media: Vec<Media>,
    pub sets: BTreeSet<String>,
    pub values: BTreeMap<String, Vec<String>>,
}

impl Item {
    pub fn new(title: &str, media: &[String], sets: &[&str]) -> Item {
        Item {
            title: Some(title.to_string()),
            media: media
                .iter()
                .map(|path| Media {
                    path: path.clone(),
                    values: BTreeMap::new(),
                })
                .collect(),
            sets: sets.iter().map(|set| set.to_string()).collect(),
            values: BTreeMap::new(),
        }
    }

    pub fn value(&mut self, slot: &str, text: &str) -> &mut Item {
        let values = self.values.entry(slot.to_string()).or_default();
        values.push(text.to_string());
        values.sort();
        self
    }

    pub fn media_paths(&self) -> Vec<&str> {
        self.media.iter().map(|media| media.path.as_str()).collect()
    }
}

/// What an import makes of an archive: items by identifier, and the paths
/// that make no item.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Outcome {
    pub items: BTreeMap<String, Item>,
    pub left_out: BTreeSet<String>,
}

/// Give the install the Dublin Core terms in `slots` it lacks (the seed has
/// only the title), as an archivist's install has them all.
async fn add_terms(fixture: &Fixture, slots: &[&str]) {
    let dcterms = with_db!(&fixture.pool, |db| VocabularyRepo::new(db)
        .get_by_prefix("dcterms")
        .await)
    .unwrap()
    .unwrap();
    let names: BTreeSet<&str> = slots
        .iter()
        .chain(&["dcterms:identifier"])
        .filter_map(|slot| slot.strip_prefix("dcterms:"))
        .filter(|name| *name != "title")
        .collect();
    for name in names {
        with_db!(&fixture.pool, |db| PropertyRepo::new(db)
            .create(&NewProperty {
                owner_id: None,
                vocabulary_id: dcterms.id,
                local_name: name.to_string(),
                label: name.to_string(),
                comment: None,
            })
            .await)
        .unwrap();
    }
}

/// Make a draft on the shape, save the plan `plan_for` edits, scan, and read
/// every page of the preview. `slots` names the terms the plan writes.
pub async fn run(
    fixture: &Fixture,
    shape: &Shape,
    slots: &[&str],
    plan_for: impl FnOnce(&mut Value),
) -> Value {
    add_terms(fixture, slots).await;
    materialize(shape, fixture.archive.path());
    let draft = fixture.draft().await;
    let id = draft["id"].clone();
    let saved = fixture.save(&draft, plan_for).await;
    assert_eq!(saved.status, 200, "{}", saved.body);
    let scanned = fixture
        .call("POST", &format!("/api/romeka/imports/{id}/scan"), None)
        .await;
    assert_eq!(scanned.status, 200, "{}", scanned.body);
    let first = fixture.preview(&id, &json!({ "include": ["check"] })).await;
    assert_eq!(first.status, 200, "{}", first.body);
    let check = first.json()["check"].clone();
    assert_eq!(check["blocking"], json!([]), "the plan cannot run");
    json!({
        "check": check,
        "entries": pages(fixture, &id, "entries", json!({})).await,
        "items": pages(fixture, &id, "records", json!({ "kind": "item" })).await,
        "media": pages(fixture, &id, "records", json!({ "kind": "media" })).await,
    })
}

async fn pages(fixture: &Fixture, id: &Value, section: &str, query: Value) -> Vec<Value> {
    let mut all = Vec::new();
    for page in 1.. {
        let mut query = query.clone();
        query["page"] = json!(page);
        query["perPage"] = json!(500);
        let answer = fixture
            .preview(id, &json!({ "include": [section], section: query }))
            .await;
        assert_eq!(answer.status, 200, "{}", answer.body);
        let body = answer.json();
        let items = body[section]["items"].as_array().unwrap().clone();
        let done = items.is_empty()
            || all.len() + items.len() >= body[section]["total"].as_u64().unwrap() as usize;
        all.extend(items);
        if done {
            break;
        }
    }
    all
}

fn texts(slots: &Value, slot: &str) -> Vec<String> {
    let mut texts: Vec<String> = slots[slot]
        .as_array()
        .map(|values| {
            values
                .iter()
                .map(|value| value["text"].as_str().unwrap().to_string())
                .collect()
        })
        .unwrap_or_default();
    texts.sort();
    texts
}

fn named(slots: &Value, names: &[&str]) -> BTreeMap<String, Vec<String>> {
    names
        .iter()
        .map(|name| (name.to_string(), texts(slots, name)))
        .filter(|(_, values)| !values.is_empty())
        .collect()
}

/// The outcome a preview read by [`run`] describes, with the values of the
/// `slots` named, on items and on media alike. An item is keyed by its
/// `dcterms:identifier`; a record the import leaves out is no item.
pub fn outcome_of(preview: &Value, shape: &Shape, slots: &[&str]) -> Outcome {
    let made = |record: &&Value| record["fate"] != "leave";
    let media: BTreeMap<u64, &Value> = preview["media"]
        .as_array()
        .unwrap()
        .iter()
        .filter(made)
        .map(|record| (record["id"].as_u64().unwrap(), record))
        .collect();
    let new_sets = preview["check"]["newSets"].as_array().unwrap();
    let mut outcome = Outcome::default();
    for record in preview["items"].as_array().unwrap().iter().filter(made) {
        let slots_of = &record["slots"];
        let identifier = texts(slots_of, "dcterms:identifier")
            .into_iter()
            .next()
            .unwrap_or_default();
        let item = Item {
            title: slots_of["dcterms:title"][0]["text"]
                .as_str()
                .map(str::to_string),
            media: record["media"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|id| media.get(&id.as_u64().unwrap()))
                .map(|media| Media {
                    path: media["files"][0]["path"].as_str().unwrap().to_string(),
                    values: named(&media["slots"], slots),
                })
                .collect(),
            sets: record["resolved"]["sets"]
                .as_array()
                .unwrap()
                .iter()
                .map(|set| {
                    let new = set["new"].as_u64().expect("a set this import makes");
                    new_sets[new as usize]["title"]
                        .as_str()
                        .unwrap()
                        .to_string()
                })
                .collect(),
            values: named(slots_of, slots),
        };
        let clash = outcome.items.insert(identifier.clone(), item);
        assert!(clash.is_none(), "two items are identified as {identifier}");
    }
    let used: BTreeSet<&str> = outcome
        .items
        .values()
        .flat_map(|item| item.media.iter().map(|media| media.path.as_str()))
        .collect();
    outcome.left_out = shape
        .files
        .iter()
        .map(|(path, _)| path)
        .chain(shape.sheets.iter().map(|sheet| &sheet.path))
        .filter(|path| !used.contains(path.as_str()))
        .cloned()
        .collect();
    outcome
}

/// `assert_eq!` on outcomes, naming only the items and paths that differ.
pub fn assert_same(actual: &Outcome, expected: &Outcome) {
    let mut differences = Vec::new();
    let identifiers: BTreeSet<&String> = actual.items.keys().chain(expected.items.keys()).collect();
    for identifier in identifiers {
        match (actual.items.get(identifier), expected.items.get(identifier)) {
            (Some(got), Some(want)) if got != want => {
                differences.push(format!("{identifier}:\n   got {got:?}\n  want {want:?}"));
            }
            (Some(got), None) => differences.push(format!("{identifier}: unexpected {got:?}")),
            (None, Some(want)) => differences.push(format!("{identifier}: missing {want:?}")),
            _ => {}
        }
    }
    for path in actual.left_out.symmetric_difference(&expected.left_out) {
        let made = if expected.left_out.contains(path) {
            "makes an item"
        } else {
            "makes no item"
        };
        differences.push(format!("{path}: {made}"));
    }
    assert!(differences.is_empty(), "{}", differences.join("\n"));
}
