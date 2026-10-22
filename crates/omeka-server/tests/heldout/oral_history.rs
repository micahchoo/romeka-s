//! Frozen held-out shape, written before the importer was read: never edit it or `expected()`.
//!
//! An oral history collection.
//!
//! One folder per interview, named by its code and the narrator. Each holds
//! the recording in parts, a transcript and sometimes a photo of the
//! narrator. One spreadsheet has a row per interview, keyed by the code; a
//! tape log has a row per tape and is not part of this plan.

use serde_json::{Value, json};

use super::{Cell, Item, Outcome, Shape, Sheet, assert_same, kind, outcome_of, run};
use crate::imports_fixture::fixture;

const INTERVIEWS: [(&str, &str, &[&str], bool); 4] = [
    ("OH-2019-001", "Patel", &["part1.mp3", "part2.mp3"], true),
    ("OH-2019-002", "Okafor", &["part1.mp3"], false),
    (
        "OH-2019-003",
        "Lindqvist",
        &["part1.mp3", "part2.mp3", "part3.mp3"],
        false,
    ),
    ("OH-2019-004", "Smith", &["part1.mp3", "part2.mp3"], true),
];

fn folder(code: &str, narrator: &str) -> String {
    format!("Interviews/{code} {narrator}")
}

pub fn shape() -> Shape {
    let mut shape = Shape::default();
    for (code, narrator, parts, photo) in INTERVIEWS {
        let folder = folder(code, narrator);
        for part in parts {
            shape.object(&format!("{folder}/{part}"), kind::MP3);
        }
        shape.object(&format!("{folder}/transcript.pdf"), kind::PDF);
        if photo {
            shape.object(&format!("{folder}/photo.jpg"), kind::JPEG);
        }
    }
    let text = |value: &str| Cell::Text(value.to_string());
    let row = |code: &str, title: &str, narrator: &str, date: &str, language: &str| {
        vec![
            text(code),
            text(title),
            text(narrator),
            text("R. Interviewer"),
            text(date),
            text(language),
        ]
    };
    shape.sheet(Sheet {
        path: "interviews.xlsx".into(),
        header: vec![
            "Code",
            "Title",
            "Narrator",
            "Interviewer",
            "Date",
            "Language",
        ],
        rows: vec![
            row(
                "OH-2019-001",
                "Growing up by the mill",
                "A. Patel",
                "2019-03-02",
                "en",
            ),
            row(
                "OH-2019-002",
                "The night market",
                "C. Okafor",
                "2019-03-09",
                "en",
            ),
            // The cataloguer typed this code in lower case.
            row(
                "oh-2019-003",
                "Fishing the outer islands",
                "E. Lindqvist",
                "2019-04-13",
                "sv",
            ),
            row(
                "OH-2019-004",
                "Forty years at the depot",
                "J. Smith",
                "2019-05-01",
                "en",
            ),
        ],
    });
    shape.sheet(Sheet {
        path: "tapes.csv".into(),
        header: vec!["Tape", "Interview", "Part", "Minutes"],
        rows: [
            ("T-001", "OH-2019-001", "1", "44"),
            ("T-002", "OH-2019-001", "2", "31"),
            ("T-003", "OH-2019-002", "1", "52"),
            ("T-004", "OH-2019-003", "1", "40"),
            ("T-005", "OH-2019-003", "2", "40"),
            ("T-006", "OH-2019-003", "3", "12"),
            ("T-007", "OH-2019-004", "1", "45"),
            ("T-008", "OH-2019-004", "2", "45"),
        ]
        .into_iter()
        .map(|(a, b, c, d)| vec![text(a), text(b), text(c), text(d)])
        .collect(),
    });
    shape
}

/// The slots `expected()` names values for.
pub const SLOTS: [&str; 4] = [
    "dcterms:creator",
    "dcterms:contributor",
    "dcterms:date",
    "dcterms:language",
];

/// One item per interview, identified by its code. The recording comes
/// first, its parts in order, then the transcript made from it, then the
/// photo. The narrator created the testimony; the interviewer contributed.
pub fn expected() -> Outcome {
    let titles = [
        ("Growing up by the mill", "A. Patel", "2019-03-02", "en"),
        ("The night market", "C. Okafor", "2019-03-09", "en"),
        (
            "Fishing the outer islands",
            "E. Lindqvist",
            "2019-04-13",
            "sv",
        ),
        ("Forty years at the depot", "J. Smith", "2019-05-01", "en"),
    ];
    let mut outcome = Outcome::default();
    for ((code, narrator, parts, photo), (title, name, date, language)) in
        INTERVIEWS.into_iter().zip(titles)
    {
        let folder = folder(code, narrator);
        let mut media: Vec<String> = parts
            .iter()
            .map(|part| format!("{folder}/{part}"))
            .collect();
        media.push(format!("{folder}/transcript.pdf"));
        if photo {
            media.push(format!("{folder}/photo.jpg"));
        }
        let mut item = Item::new(title, &media, &["Interviews"]);
        item.value("dcterms:creator", name)
            .value("dcterms:contributor", "R. Interviewer")
            .value("dcterms:date", date)
            .value("dcterms:language", language);
        outcome.items.insert(code.to_string(), item);
    }
    outcome
        .left_out
        .extend(["interviews.xlsx".to_string(), "tapes.csv".to_string()]);
    outcome
}

fn column(index: usize, header: &str) -> Value {
    json!({ "index": index, "header": header })
}

fn from_column(index: usize, header: &str) -> Value {
    json!({ "sources": [{ "from": "column", "sheet": "interviews", "column": column(index, header) }],
            "combine": "first" })
}

/// The interviews sheet joined on the code in the folder's name, each column
/// on its term, the top folder as the set, and the parts ordered by a value
/// map: recording, transcript, photo.
pub fn plan_for_oral_history(plan: &mut Value) {
    plan["grouping"]["pattern"] =
        json!(r"^[^/]+/(?<identifier>OH-\d{4}-\d{3}) [^/]+/(?<part>[^/]+)$");
    let headers = [
        "Code",
        "Title",
        "Narrator",
        "Interviewer",
        "Date",
        "Language",
    ];
    plan["sheets"] = json!([{
        "id": "interviews", "source": "s1", "file": "interviews.xlsx", "hash": null,
        "read": { "encoding": "auto", "delimiter": "auto", "quote": "\"", "headerLine": 1, "trim": true },
        "rowKind": "item",
        "join": { "on": "identifier", "column": column(0, "Code"),
                  "normalize": { "caseFold": true, "leadingZeros": false, "separators": false } },
        "itemKey": null,
        "columns": headers.iter().enumerate().map(|(index, header)| json!({
            "column": column(index, header),
            "options": { "split": null, "type": "literal", "lang": null, "private": false }
        })).collect::<Vec<_>>(),
    }]);
    let item = &mut plan["slots"]["item"];
    item["dcterms:title"] = from_column(1, "Title");
    item["dcterms:creator"] = from_column(2, "Narrator");
    item["dcterms:contributor"] = from_column(3, "Interviewer");
    item["dcterms:date"] = from_column(4, "Date");
    item["dcterms:language"] = from_column(5, "Language");
    item["sets"] = json!({ "sources": [{ "from": "folder", "level": 1 }], "combine": "first" });
    plan["slots"]["media"]["mediaOrder"] = json!({
        "sources": [{ "from": "group", "name": "part" }], "combine": "first",
        "map": { "part1.mp3": "1", "part2.mp3": "2", "part3.mp3": "3",
                 "transcript.pdf": "4", "photo.jpg": "5" }
    });
}

#[tokio::test]
async fn oral_history_makes_one_item_per_interview() {
    let fixture = fixture().await;
    let shape = shape();
    let preview = run(&fixture, &shape, &SLOTS, plan_for_oral_history).await;
    assert_same(&outcome_of(&preview, &shape, &SLOTS), &expected());
}
