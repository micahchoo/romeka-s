//! The round-4 answers through the routes: the plan's answers to anomalies,
//! name words, column matches, and an identifier that starts like a formula
//! in the list round trip (`docs/import-v2.md`, "What the admin screens need
//! at scale").

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;
mod imports_fixture;

use axum::http::StatusCode;
use imports_fixture::{Fixture, JPEG, fixture};
use serde_json::{Value, json};

async fn scanned(fixture: &Fixture, edit: impl FnOnce(&mut Value)) -> Value {
    let draft = fixture.draft().await;
    let saved = fixture.save(&draft, edit).await;
    assert_eq!(saved.status, StatusCode::OK, "{}", saved.body);
    let scan = fixture
        .call(
            "POST",
            &format!("/api/romeka/imports/{}/scan", draft["id"]),
            None,
        )
        .await;
    assert_eq!(scan.status, StatusCode::OK, "{}", scan.body);
    draft["id"].clone()
}

#[tokio::test]
async fn the_answers_to_anomalies_name_words_and_column_matches_through_the_preview() {
    let fixture = fixture().await;
    fixture.write_objects(&[
        ("A1_front.jpg", JPEG),
        ("A1_back.jpg", JPEG),
        ("stray.png", JPEG),
    ]);
    fixture.write(&[
        ("B2.jpg", JPEG),
        ("copy/B2.jpg", JPEG),
        ("register.csv", b"Id,File\nA1,A1_front.jpg\n"),
    ]);
    let column = |index: usize, header: &str| {
        json!({ "column": { "index": index, "header": header },
                "options": { "split": null, "type": "literal", "lang": null, "private": false } })
    };
    let id = scanned(&fixture, |plan| {
        plan["structure"]["levels"][0]["grouping"]["pattern"] =
            json!(r"^(?:.*/)?(?<identifier>[A-Z]\d)(?:_(?<part>[a-z]+))?\.jpg$");
        plan["structure"]["levels"][0]["lists"] = json!([{
            "id": "register", "source": "s1", "file": "register.csv", "hash": null,
            "read": { "encoding": "utf-8", "delimiter": ",", "quote": "\"", "headerLine": 1, "trim": true },
            "rowKind": "item",
            "join": { "on": "identifier", "column": { "index": 0, "header": "Id" },
                      "normalize": { "caseFold": false, "leadingZeros": false, "separators": false } },
            "itemKey": null,
            "columns": [column(0, "Id"), column(1, "File")]
        }]);
        plan["anomalies"] = json!({ "unmatchedFiles": "own", "itemsWithoutRow": "leave" });
    })
    .await;
    let preview = fixture
        .preview(
            &id,
            &json!({
                "include": ["entries", "records", "nameWords", "columnMatches"],
                "entries": { "text": "copy" },
                "records": { "left": true },
                "nameWords": { "path": "A1_front.jpg" }
            }),
        )
        .await;
    assert_eq!(preview.status, StatusCode::OK, "{}", preview.body);
    let preview = preview.json();

    assert_eq!(
        preview["entries"]["items"][0]["left"], "identical",
        "keepOne: B2.jpg is first by path, so its copy is left out"
    );
    let left: Vec<&str> = preview["records"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|record| record["fate"]["left"].as_str().unwrap())
        .collect();
    assert_eq!(
        left,
        ["noRow", "noRow"],
        "B2 and stray.png, its own item, have no row"
    );
    assert_eq!(
        preview["nameWords"],
        json!({
            "parts": ["A1", "front", "jpg"],
            "names": 2,
            "places": [
                { "total": 1, "page": 1, "perPage": 50, "items": [{ "text": "A1", "count": 2 }] },
                { "total": 2, "page": 1, "perPage": 50,
                  "items": [{ "text": "back", "count": 1 }, { "text": "front", "count": 1 }] },
                { "total": 1, "page": 1, "perPage": 50, "items": [{ "text": "jpg", "count": 2 }] }
            ]
        })
    );
    assert_eq!(
        preview["columnMatches"],
        json!([{ "sheet": "register", "rows": 1, "columns": [
            { "index": 0, "header": "Id", "groupKeys": 1, "fileNames": 0, "empty": 0 },
            { "index": 1, "header": "File", "groupKeys": 0, "fileNames": 1, "empty": 0 }
        ] }])
    );
}

#[tokio::test]
async fn an_identifier_that_starts_like_a_formula_round_trips_through_the_list() {
    let fixture = fixture().await;
    fixture.write_objects(&[("-001.jpg", JPEG), ("=X.jpg", JPEG)]);
    let id = scanned(&fixture, |plan| {
        plan["structure"]["levels"][0]["grouping"]["pattern"] =
            json!(r"^(?<identifier>[^/]+)\.jpg$");
    })
    .await;
    let template = fixture
        .call(
            "GET",
            &format!("/api/romeka/imports/{id}/template.csv?columns=dcterms:title"),
            None,
        )
        .await;
    assert_eq!(template.status, StatusCode::OK, "{}", template.body);
    let lines: Vec<&str> = template.body.split("\r\n").collect();
    assert_eq!(lines[1], "'-001,'-001.jpg,");
    assert_eq!(lines[2], "'=X,'=X.jpg,");

    let filled = format!("{}\r\n{}Lamp\r\n{}Bowl\r\n", lines[0], lines[1], lines[2]);
    let uploaded = fixture.upload(&id, "list.csv", filled.as_bytes()).await;
    assert_eq!(uploaded.status, StatusCode::CREATED, "{}", uploaded.body);
    let preview = fixture
        .preview(
            &id,
            &json!({ "include": ["records"], "records": { "slot": "dcterms:title" } }),
        )
        .await;
    assert_eq!(preview.status, StatusCode::OK, "{}", preview.body);
    let titles: Vec<(String, String)> = preview.json()["records"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|record| {
            (
                record["resolved"]["itemKey"].as_str().unwrap().to_string(),
                record["slots"]["dcterms:title"][0]["text"]
                    .as_str()
                    .unwrap()
                    .to_string(),
            )
        })
        .collect();
    assert_eq!(
        titles,
        [
            ("-001".to_string(), "Lamp".to_string()),
            ("=X".to_string(), "Bowl".to_string())
        ]
    );
}
