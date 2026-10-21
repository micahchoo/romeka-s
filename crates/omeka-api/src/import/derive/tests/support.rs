//! Evidence, plans and catalogs built in memory.

use std::collections::HashMap;

use serde_json::{Value as Json, json};

use crate::import::catalog::{Catalog, Template};
use crate::import::derive::{FileTags, Index, IndexedSource, NoTags, Sheets, derive};
use crate::import::evidence::{FileEntry, Walk};
use crate::import::model::{Flag, Model, Record};
use crate::import::plan::{
    FlagKind, ImportPlan, RecordKind, RelPath, SheetPlan, SlotName, SourceId, SourcePlan, TagId,
    Term, default_plan,
};
use crate::import::sheet::{self, key_column};
use crate::import::tags::Embedded;

pub const JPEG: &str = "image/jpeg";
pub const TIFF: &str = "image/tiff";
pub const TEXT: &str = "text/plain";
pub const BINARY: &str = "application/octet-stream";

pub fn entry(path: &str, sniffed: &str) -> FileEntry {
    FileEntry {
        path: RelPath::parse(path).expect("path"),
        size: 100,
        modified: None,
        sniffed: sniffed.to_string(),
        decoded: None,
    }
}

pub fn index(files: &[(&str, &str)]) -> Index {
    sources(&[("s1", files)])
}

pub fn sources(list: &[(&str, &[(&str, &str)])]) -> Index {
    Index {
        sources: list
            .iter()
            .map(|(id, files)| IndexedSource {
                id: SourceId((*id).to_string()),
                walk: Walk {
                    entries: files
                        .iter()
                        .map(|(path, sniffed)| entry(path, sniffed))
                        .collect(),
                    ..Walk::default()
                },
            })
            .collect(),
    }
}

/// The default plan of a draft on source `s1`, as JSON to edit.
pub fn base() -> Json {
    let plan = default_plan(vec![
        SourcePlan::Folder {
            id: SourceId("s1".into()),
            root: "archive".into(),
            path: RelPath::parse("").expect("root"),
        },
        SourcePlan::Staging {
            id: SourceId("s2".into()),
        },
    ])
    .expect("default plan");
    serde_json::to_value(plan).expect("json")
}

pub fn plan(edit: impl FnOnce(&mut Json)) -> ImportPlan {
    let mut value = base();
    edit(&mut value);
    ImportPlan::from_json(&value.to_string()).unwrap_or_else(|error| panic!("{error}: {value}"))
}

/// `<folders>/<identifier>[_<part>].<ext>`: `Box/A1_front.jpg` is part
/// `front` of `A1`.
pub const PATTERN: &str = r"^(?:.*/)?(?<identifier>[^/_]+)(?:_(?<part>[^/.]+))?\.[^/.]+$";

/// A sheet plan mapping every header as a literal column.
pub fn sheet_plan(id: &str, headers: &[&str]) -> Json {
    let columns: Vec<Json> = headers
        .iter()
        .enumerate()
        .map(|(index, header)| {
            json!({
                "column": { "index": index, "header": header },
                "options": { "split": null, "type": "literal", "lang": null, "private": false }
            })
        })
        .collect();
    json!({
        "id": id, "source": "s1", "file": format!("{id}.csv"), "hash": null,
        "read": { "encoding": "utf-8", "delimiter": ",", "quote": "\"", "headerLine": 1, "trim": false },
        "rowKind": "item", "join": null, "itemKey": null, "columns": columns
    })
}

pub fn join_on(
    on: &str,
    index: u32,
    header: &str,
    case_fold: bool,
    leading_zeros: bool,
    separators: bool,
) -> Json {
    json!({
        "on": on, "column": { "index": index, "header": header },
        "normalize": { "caseFold": case_fold, "leadingZeros": leading_zeros, "separators": separators }
    })
}

pub fn col(sheet: &str, index: u32, header: &str) -> Json {
    json!({ "from": "column", "sheet": sheet, "column": { "index": index, "header": header } })
}

pub fn constant(value: &str) -> Json {
    json!({ "from": "constant", "value": value })
}

pub fn rule(sources: Json, combine: &str) -> Json {
    json!({ "sources": sources, "combine": combine })
}

/// The plan's list at this place in plan order.
pub fn list(plan: &ImportPlan, at: usize) -> &SheetPlan {
    plan.structure.lists().nth(at).expect("a list there").1
}

pub fn read_sheets(plan: &ImportPlan, csv: &[(&str, &[u8])]) -> Sheets {
    csv.iter()
        .map(|(id, bytes)| {
            let (_, sheet_plan) = plan
                .structure
                .lists()
                .find(|(_, sheet)| sheet.id.0 == *id)
                .expect("sheet in plan");
            (
                sheet_plan.id.clone(),
                sheet::read(bytes, &sheet_plan.read, key_column(sheet_plan)),
            )
        })
        .collect()
}

/// Tags by path.
#[derive(Default)]
pub struct Tags(pub HashMap<(String, TagId), Embedded>);

impl Tags {
    pub fn put(&mut self, path: &str, tag: TagId, values: &[&str], windows_1252: bool) {
        let values = values.iter().map(|text| (*text).to_string()).collect();
        let found = Embedded {
            values,
            windows_1252,
        };
        self.0.insert((path.to_string(), tag), found);
    }
}

impl FileTags for Tags {
    fn tag(&self, _: &SourceId, entry: &FileEntry, tag: TagId) -> Option<Embedded> {
        self.0.get(&(entry.path.to_string(), tag)).cloned()
    }
}

pub fn term(text: &str) -> Term {
    Term::parse(text).expect("term")
}

pub fn catalog() -> Catalog {
    let properties = [
        "dcterms:title",
        "dcterms:identifier",
        "dcterms:date",
        "dcterms:created",
        "dcterms:description",
        "dcterms:creator",
        "dcterms:subject",
        "dcterms:format",
        "dcterms:isPartOf",
        "dcterms:relation",
        "foaf:name",
    ];
    Catalog {
        properties: properties
            .iter()
            .map(|text| (term(text), (*text).to_string()))
            .collect(),
        templates: vec![Template {
            id: 1,
            label: "Base Resource".into(),
            fields: Vec::new(),
        }],
        base_template: Some(1),
        files: omeka_media::validation::FileValidation::Disabled,
        ..Catalog::default()
    }
}

pub fn run(index: &Index, sheets: &Sheets, plan: &ImportPlan) -> Model {
    derive(index, sheets, plan, &catalog(), &NoTags)
}

pub fn kept(model: &Model, kind: RecordKind) -> Vec<&Record> {
    model.kept().filter(|record| record.kind == kind).collect()
}

pub fn texts<'m>(record: &'m Record, slot: &str) -> Vec<&'m str> {
    let slot = SlotName::parse(slot).expect("slot");
    record
        .slots
        .get(&slot)
        .map(|values| {
            values
                .iter()
                .map(|value| value.value.text.as_str())
                .collect()
        })
        .unwrap_or_default()
}

/// The kept item whose item key is `key`.
pub fn item<'m>(model: &'m Model, key: &str) -> &'m Record {
    model
        .kept()
        .find(|record| {
            record.kind == RecordKind::Item && record.resolved.item_key.as_deref() == Some(key)
        })
        .unwrap_or_else(|| panic!("no item {key}"))
}

/// The paths of an item's media files, primary first.
pub fn media_paths<'m>(model: &'m Model, record: &Record) -> Vec<&'m str> {
    record
        .media
        .iter()
        .flat_map(|media| {
            model
                .record(*media)
                .files
                .iter()
                .map(|file| file.path.as_str())
        })
        .collect()
}

pub fn flags(model: &Model, kind: FlagKind) -> Vec<&Flag> {
    model
        .flags
        .iter()
        .filter(|flag| flag.kind == kind)
        .collect()
}
