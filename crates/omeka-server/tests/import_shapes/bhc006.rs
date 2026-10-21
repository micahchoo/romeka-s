//! Frozen outcomes, written from what the archive means: never edit them or the replica to pass.
//!
//! A replica of the BHC006 Gawan Museum Trust archive, built from its file
//! list, and its outcome group by group.
//!
//! `bhc006.listing` is the real archive's file list (`find . -type f -printf
//! '%P\t%s\n' | sort`) and `bhc006.identifiers` the real catalogues'
//! `Identifier` cells, both with personal names replaced. Every other
//! catalogue column is synthetic with the real column's shape.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};

use super::{Cell, Item, Outcome, Shape, Sheet, kind, outcome_of, run};
use crate::imports_fixture::fixture;

const LISTING: &str = include_str!("bhc006.listing");
const IDENTIFIERS: &str = include_str!("bhc006.identifiers");

/// The three catalogues of the Gawan Museum Trust, as sheet id, file and
/// item type. The fourth root catalogue (`BHC006_CC_SECOND_AF`) belongs to
/// another curator whose files are not in this archive.
const CATALOGUES: [(&str, &str, &str); 3] = [
    ("af", "BHC006_GMT_AF", "Artifact"),
    ("pn", "BHC006_GMT_PN", "Photo Narrative"),
    ("d", "BHC006_GMT_D", "Documents"),
];

const SERIES: [&str; 5] = [
    "Artifacts",
    "Photo Narratives (PN)",
    "Documents (D)",
    "Video (V)",
    "Audio Interviews (AI)",
];

const HEADER: [&str; 8] = [
    "Sl No",
    "Identifier",
    "Collection",
    "Curator",
    "Item Type",
    "Year Of Digitization",
    "Digitized by",
    "Keywords",
];
const IDENTIFIER: usize = 1;
const YEAR: usize = 5;
const DIGITIZED_BY: usize = 6;
const KEYWORDS: usize = 7;

/// Keywords written by hand for the objects the tests read closely:
/// `AF_010`'s views each carry a view word, `AF_011`'s views agree.
const HAND_KEYWORDS: [(&str, &str); 5] = [
    ("BHC006_GMT_AF_010a.jpg", "oil lamp, brass, Front View"),
    ("BHC006_GMT_AF_010b.jpg", "oil lamp, brass, Back View"),
    ("BHC006_GMT_AF_010c.jpg", "oil lamp,brass , Base"),
    ("BHC006_GMT_AF_011a.jpg", "coin, copper"),
    ("BHC006_GMT_AF_011b.jpg", "coin, copper"),
];

const TOPICS: [&str; 10] = [
    "brass",
    "copper",
    "vessel",
    "coin",
    "textile",
    "toy",
    "farm tool",
    "kitchen",
    "ritual",
    "photograph",
];

/// Invented digitizers; the real column has comma lists with a typo and a
/// case variant of one name.
const DIGITIZERS: [&str; 6] = [
    "Asha Rane",
    "Kiran Kale",
    "Asha Rane, Kiran Kale",
    "Kirn Kale",
    "meena joshi ",
    "Meena Joshi, Asha Rane",
];

/// A catalogue's rows in order, each as its cells.
pub fn rows(file: &str) -> Vec<Vec<Cell>> {
    let curator = if file.contains("_GMT_") {
        "M. Curator"
    } else {
        "R. Curator"
    };
    let item_type = CATALOGUES
        .iter()
        .find(|(_, name, _)| *name == file)
        .map_or("Artifact", |(_, _, kind)| kind);
    IDENTIFIERS
        .lines()
        .filter_map(|line| line.split_once('\t'))
        .filter(|(sheet, _)| *sheet == file)
        .enumerate()
        .map(|(n, (_, identifier))| {
            let year = match n {
                n if n % 40 == 39 => Cell::Text("To be assigned".into()),
                n if n % 57 == 56 => Cell::Blank,
                n if n < 74 => Cell::Number(2017.0),
                _ => Cell::Number(2018.0),
            };
            let keywords = HAND_KEYWORDS
                .iter()
                .find(|(name, _)| *name == identifier)
                .map_or_else(|| keywords(n), |(_, cell)| cell.to_string());
            vec![
                Cell::Number((n + 1) as f64),
                Cell::Text(identifier.to_string()),
                Cell::Text("Citizen Curators".into()),
                Cell::Text(curator.into()),
                Cell::Text(item_type.into()),
                year,
                Cell::Text(DIGITIZERS[n % DIGITIZERS.len()].into()),
                Cell::Text(keywords),
            ]
        })
        .collect()
}

fn keywords(n: usize) -> String {
    let first = TOPICS[n % TOPICS.len()];
    let first = if n % 15 == 7 {
        first.to_uppercase()
    } else {
        first.to_string()
    };
    let mut cell = format!("{first}, {}", TOPICS[(n / 3 + 4) % TOPICS.len()]);
    if n.is_multiple_of(4) {
        cell.push_str(", Front View");
    }
    cell
}

/// Each path in the listing, with its size.
pub fn listing() -> Vec<(&'static str, u64)> {
    LISTING
        .lines()
        .filter_map(|line| line.split_once('\t'))
        .map(|(path, size)| (path, size.parse().unwrap()))
        .collect()
}

/// The archive: a file of the right kind per listed path (empty where the
/// listing says 0 bytes), and the root catalogues as workbooks.
pub fn shape() -> Shape {
    let mut shape = Shape::default();
    for (path, size) in listing() {
        let extension = path.rsplit_once('.').map_or("", |(_, ext)| ext);
        match extension.to_ascii_lowercase().as_str() {
            "xlsx" if !path.contains('/') => {
                let file = path.trim_end_matches(".xlsx");
                shape.sheet(Sheet {
                    path: path.to_string(),
                    header: HEADER.to_vec(),
                    rows: rows(file),
                });
            }
            _ if size == 0 => shape.raw(path, b""),
            "jpg" | "jpeg" => shape.object(path, kind::JPEG),
            // A Nikon NEF is a TIFF container.
            "tif" | "tiff" | "nef" => shape.object(path, kind::TIFF),
            "mp3" => shape.object(path, kind::MP3),
            "avi" => shape.object(path, kind::AVI),
            "mov" => shape.object(path, kind::MOV),
            "ods" => shape.object(path, kind::ZIP),
            "ctg" => shape.object(path, b"\x00\x00CTG"),
            "csv" => shape.raw(path, stale_csv().as_bytes()),
            _ => shape.object(path, b"// source file\n"),
        }
    }
    shape
}

/// The older copy of the AF catalogue in `Artifacts/`: the same header,
/// other keywords.
fn stale_csv() -> String {
    let mut out = HEADER.join(",");
    for (n, (_, identifier)) in IDENTIFIERS
        .lines()
        .filter_map(|line| line.split_once('\t'))
        .take(5)
        .enumerate()
    {
        out.push_str(&format!(
            "\n{},{identifier},Citizen Curators,M. Curator,Artifact,2017,Asha Rane,old keyword",
            n + 1
        ));
    }
    out.push('\n');
    out
}

/// The item code of each series' files: the object number, then the view
/// letters and a `(1)` copy mark that belong to the file, not the item.
const PATTERN: &str = r"^(?<series>Artifacts|Photo Narratives \(PN\)|Documents \(D\)|Video \(V\)|Audio Interviews \(AI\))/(?<identifier>BHC_?006_GMT_AF_\d+|BHC006_GMT_PN_\d+|BHC006_KHAN_Donor\d{8}_(?:D|V|AI)\d+|REC\d+)(?<part>[a-z]{1,2})?(?: ?\(\d\))?\.(?i:jpe?g|tiff?|nef|png|avi|mp3)$";

fn column(index: usize) -> Value {
    json!({ "index": index, "header": HEADER[index] })
}

fn from_columns(index: usize) -> Value {
    let sources: Vec<Value> = CATALOGUES
        .iter()
        .map(|(sheet, _, _)| json!({ "from": "column", "sheet": sheet, "column": column(index) }))
        .collect();
    json!({ "sources": sources, "combine": "all" })
}

/// The plan the P1 screens can make: one item per object with its views as
/// files, the three GMT catalogues as rows that describe files, values
/// rolled up to the item or kept on each file, and two item sets per item.
pub fn plan_for_bhc006(plan: &mut Value) {
    plan["grouping"]["pattern"] = json!(PATTERN);
    let split = |index: usize| match index {
        DIGITIZED_BY | KEYWORDS => json!(","),
        _ => Value::Null,
    };
    plan["sheets"] = CATALOGUES
        .iter()
        .map(|(id, file, _)| {
            json!({
                "id": id, "source": "s1", "file": format!("{file}.xlsx"), "hash": null,
                "read": { "encoding": "auto", "delimiter": "auto", "quote": "\"",
                          "headerLine": 1, "trim": true },
                "rowKind": "media",
                "join": { "on": "fileName", "column": column(IDENTIFIER),
                          "normalize": { "caseFold": true, "leadingZeros": true, "separators": true } },
                "itemKey": null,
                "columns": (0..HEADER.len()).map(|index| json!({
                    "column": column(index),
                    "options": { "split": split(index), "type": "literal", "lang": null, "private": false }
                })).collect::<Vec<_>>(),
            })
        })
        .collect();
    let item = &mut plan["slots"]["item"];
    item["dcterms:title"] =
        json!({ "sources": [{ "from": "group", "name": "identifier" }], "combine": "first" });
    item["dcterms:subject"] = from_columns(KEYWORDS);
    item["dcterms:dateSubmitted"] = from_columns(YEAR);
    item["sets"] = json!({
        "sources": [{ "from": "constant", "value": "Citizen Curators" }, { "from": "folder", "level": 1 }],
        "combine": "all"
    });
    plan["slots"]["media"]["dcterms:contributor"] = from_columns(DIGITIZED_BY);
}

const SLOTS: [&str; 4] = [
    "dcterms:identifier",
    "dcterms:subject",
    "dcterms:dateSubmitted",
    "dcterms:contributor",
];

async fn outcome() -> (Outcome, Value) {
    let fixture = fixture().await;
    let shape = shape();
    let preview = run(&fixture, &shape, &SLOTS, plan_for_bhc006).await;
    (outcome_of(&preview, &shape, &SLOTS), preview)
}

fn item<'a>(outcome: &'a Outcome, identifier: &str) -> &'a Item {
    outcome
        .items
        .get(identifier)
        .unwrap_or_else(|| panic!("no item {identifier}"))
}

fn split(cell: &str) -> Vec<String> {
    let mut values: Vec<String> = cell
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect();
    values.sort();
    values
}

/// The cell of `column` in the row whose identifier is `name`.
fn cell(file: &str, name: &str, column: usize) -> String {
    let row = rows(file)
        .into_iter()
        .find(|row| matches!(&row[IDENTIFIER], Cell::Text(text) if text == name))
        .unwrap_or_else(|| panic!("no row {name}"));
    match &row[column] {
        Cell::Text(text) => text.clone(),
        other => panic!("{other:?}"),
    }
}

#[tokio::test]
async fn bhc006_an_object_is_one_item_with_its_views_in_order() {
    let (outcome, _) = outcome().await;
    let object = item(&outcome, "BHC006_GMT_AF_010");
    assert_eq!(
        object.media_paths(),
        [
            "Artifacts/BHC006_GMT_AF_010a.jpg",
            "Artifacts/BHC006_GMT_AF_010b.jpg",
            "Artifacts/BHC006_GMT_AF_010c.jpg",
        ]
    );
    assert_eq!(object.title.as_deref(), Some("BHC006_GMT_AF_010"));
    assert_eq!(object.values["dcterms:identifier"], ["BHC006_GMT_AF_010"]);
    let sets: BTreeSet<String> = ["Artifacts", "Citizen Curators"].map(String::from).into();
    assert_eq!(object.sets, sets);
}

#[tokio::test]
#[ignore = "imp/engine: item values from media rows"]
async fn bhc006_an_item_has_its_rows_keywords_each_once() {
    let (outcome, _) = outcome().await;
    let lamp = item(&outcome, "BHC006_GMT_AF_010");
    assert_eq!(
        lamp.values.get("dcterms:subject").map(Vec::as_slice),
        Some(&["Back View", "Base", "Front View", "brass", "oil lamp"].map(String::from)[..])
    );
    assert_eq!(
        lamp.values.get("dcterms:dateSubmitted").map(Vec::as_slice),
        Some(&["2017".to_string()][..])
    );
    let coin = item(&outcome, "BHC006_GMT_AF_011");
    assert_eq!(
        coin.values.get("dcterms:subject").map(Vec::as_slice),
        Some(&["coin", "copper"].map(String::from)[..])
    );
}

#[tokio::test]
async fn bhc006_each_file_has_its_own_rows_digitizers() {
    let (outcome, _) = outcome().await;
    for media in &item(&outcome, "BHC006_GMT_AF_010").media {
        let name = media.path.trim_start_matches("Artifacts/");
        assert_eq!(
            media.values.get("dcterms:contributor"),
            Some(&split(&cell("BHC006_GMT_AF", name, DIGITIZED_BY))),
            "{name}"
        );
    }
}

/// A name as a person reads it past zero padding and capitals.
fn loose(name: &str) -> String {
    let mut out = String::new();
    let mut digits = String::new();
    for ch in name.chars().chain(std::iter::once('/')) {
        if ch.is_ascii_digit() {
            digits.push(ch);
            continue;
        }
        if !digits.is_empty() {
            let stripped = digits.trim_start_matches('0');
            out.push_str(if stripped.is_empty() { "0" } else { stripped });
            digits.clear();
        }
        out.extend(ch.to_lowercase());
    }
    out.pop();
    out
}

#[tokio::test]
async fn bhc006_photo_narratives_pair_past_padding_and_case() {
    let (outcome, _) = outcome().await;
    let rows: BTreeMap<String, String> = rows("BHC006_GMT_PN")
        .into_iter()
        .filter_map(|row| match (&row[IDENTIFIER], &row[DIGITIZED_BY]) {
            (Cell::Text(name), Cell::Text(by)) => Some((loose(name), by.clone())),
            _ => None,
        })
        .collect();
    let mut paired = 0;
    for media in outcome.items.values().flat_map(|item| &item.media) {
        let Some(name) = media.path.strip_prefix("Photo Narratives (PN)/") else {
            continue;
        };
        if let Some(by) = rows.get(&loose(name)) {
            paired += 1;
            assert_eq!(
                media.values.get("dcterms:contributor"),
                Some(&split(by)),
                "{name}"
            );
        }
    }
    assert!(paired > 100, "only {paired} PN files have a row to pair");
}

#[tokio::test]
async fn bhc006_documents_make_items_without_rows() {
    let (outcome, _) = outcome().await;
    let documents: Vec<&Item> = outcome
        .items
        .values()
        .filter(|item| item.sets.contains("Documents (D)"))
        .collect();
    assert_eq!(documents.len(), 15);
    for document in documents {
        assert_eq!(document.media.len(), 1, "{document:?}");
        assert!(
            !document.values.contains_key("dcterms:subject")
                && document.media.iter().all(|media| media.values.is_empty()),
            "a Documents file paired with a row: {document:?}"
        );
    }
}

#[tokio::test]
async fn bhc006_only_the_series_folders_make_items() {
    let (outcome, _) = outcome().await;
    for (identifier, item) in &outcome.items {
        assert!(
            !identifier.starts_with("BHC006_CC_"),
            "the other curator's catalogue made {identifier}"
        );
        let series: Vec<&String> = item
            .sets
            .iter()
            .filter(|set| *set != "Citizen Curators")
            .collect();
        assert!(
            item.sets.contains("Citizen Curators")
                && series.len() == 1
                && SERIES.contains(&series[0].as_str()),
            "{identifier}: {:?}",
            item.sets
        );
    }
    for (path, _) in listing() {
        let never = path.ends_with(".json")
            || path.starts_with("tiny-iiif/")
            || path.starts_with("Extras/")
            || path.starts_with("Artifacts/BHC006_GMT_AF.");
        if never {
            assert!(outcome.left_out.contains(path), "{path} made an item");
        }
    }
}

/// The flag kind a person reads when a row names a file the install's file
/// settings refuse. The engine unit names it.
const NAMED_FILE_REFUSED: &str = "named-file-refused";

/// Whether the preview shows a problem of `kind` on the file at `path`.
fn problem_on(preview: &Value, kind: &str, path: &str) -> bool {
    preview["entries"].as_array().unwrap().iter().any(|entry| {
        entry["entry"]["file"]["path"] == path
            && entry["flags"]
                .as_array()
                .unwrap()
                .iter()
                .any(|flag| flag["kind"] == kind)
    })
}

#[tokio::test]
#[ignore = "imp/engine: a refused file a row names"]
async fn bhc006_a_row_naming_a_raw_file_is_a_problem() {
    let (_, preview) = outcome().await;
    let named = cell("BHC006_GMT_AF", "BHC_006_GMT_AF_297a.NEF", IDENTIFIER);
    assert!(
        problem_on(&preview, NAMED_FILE_REFUSED, &format!("Artifacts/{named}")),
        "the only files of AF_297 are NEF files its rows name, and nothing says so"
    );
}
