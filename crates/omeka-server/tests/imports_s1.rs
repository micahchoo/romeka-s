//! Scenario S1 (`docs/import-v2.md`, "Scenarios") through the routes up to
//! Check, on a generated BHC_006 folder; and the preview's time on a warm
//! index of 10,000 and 40,000 entries.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;
mod imports_fixture;

use std::time::{Duration, Instant};

use axum::http::StatusCode;
use imports_fixture::{BHC_PATTERN, bhc_006, fixture};
use omeka_db::ids::ImportId;
use omeka_db::repos::{EntryRepo, Paging};
use omeka_db::rows::NewEntry;
use serde_json::{Value, json};

#[tokio::test]
async fn s1_the_bhc_006_demo_through_the_routes() {
    let fixture = fixture().await;
    bhc_006(fixture.archive.path());
    let draft = fixture.draft().await;
    let id = draft["id"].clone();
    let saved = fixture
        .save(&draft, |plan| {
            plan["grouping"]["pattern"] = json!(BHC_PATTERN);
            plan["slots"]["item"]["sets"] = json!({
                "sources": [{ "from": "group", "name": "set" }],
                "combine": "first"
            });
        })
        .await;
    assert_eq!(saved.status, StatusCode::OK, "{}", saved.body);
    let scanned = fixture
        .call("POST", &format!("/api/romeka/imports/{id}/scan"), None)
        .await;
    assert_eq!(scanned.status, StatusCode::OK, "{}", scanned.body);
    assert_eq!(scanned.json()["sources"][0]["files"], 17);

    let preview = fixture
        .preview(
            &id,
            &json!({
                "include": ["check", "records", "entries"],
                "records": { "kind": "item", "perPage": 50 },
                "entries": { "perPage": 50, "text": "_scratch" }
            }),
        )
        .await;
    assert_eq!(preview.status, StatusCode::OK, "{}", preview.body);
    let preview = preview.json();
    let check = &preview["check"];
    assert_eq!(count(check, "item", "create"), 9, "{check}");
    let sets: Vec<&str> = check["newSets"]
        .as_array()
        .unwrap()
        .iter()
        .map(|set| set["title"].as_str().unwrap())
        .collect();
    assert_eq!(sets, ["Artifacts", "Photographs", "Scans"]);
    let items = preview["records"]["items"].as_array().unwrap();
    let in_set = |set: usize| {
        items
            .iter()
            .filter(|item| item["resolved"]["sets"] == json!([{ "new": set }]))
            .count()
    };
    assert_eq!((in_set(0), in_set(1), in_set(2)), (4, 4, 1));
    assert!(
        items
            .iter()
            .all(|item| item["resolved"]["visibility"] == "private"),
        "all private"
    );
    let scratch = preview["entries"]["items"].as_array().unwrap();
    assert_eq!(scratch.len(), 2);
    for entry in scratch {
        let not_media = entry["flags"]
            .as_array()
            .unwrap()
            .iter()
            .any(|flag| flag["kind"] == "not-media");
        assert!(
            entry["record"].is_null() && (entry["unmatched"] == true || not_media),
            "{entry}"
        );
    }
    assert!(check["canRun"].as_bool().unwrap());
    assert_eq!(check["digest"].as_str().unwrap().len(), 64);
}

fn count(check: &Value, kind: &str, does: &str) -> u64 {
    check["counts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["kind"] == kind && row["does"] == does)
        .map_or(0, |row| row["count"].as_u64().unwrap())
}

/// The contract's preview budget, release build, median of 7, warm index.
/// Run with `cargo test --release -p omeka-server --test imports_s1 -- --ignored --nocapture`.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "release-mode timing; run by hand"]
async fn a_preview_of_10000_and_40000_entries_fits_the_budget() {
    for (entries, budget) in [(10_000_usize, 150_u64), (40_000, 600)] {
        let fixture = fixture().await;
        let draft = fixture.draft().await;
        let id = ImportId(draft["id"].as_u64().unwrap());
        let mut register = String::from("Id,Title,Date\n");
        let rows: Vec<NewEntry> = (0..entries)
            .map(|n| {
                register.push_str(&format!("OBJ_{n:06},Object {n},19{:02}\n", n % 100));
                NewEntry {
                    source: "s1".into(),
                    path: format!("Box {}/OBJ_{n:06}.jpg", n / 500),
                    size: 1000,
                    modified: Default::default(),
                    media_type: Some("image/jpeg".into()),
                    flags: json!({}),
                }
            })
            .chain([NewEntry {
                source: "s1".into(),
                path: String::new(),
                size: 0,
                modified: Default::default(),
                media_type: None,
                flags: json!({ "walk": { "token": "t", "symlinks": 0, "dot": 0, "special": 0,
                    "unreadable": [], "unusableNames": [], "collisions": [] } }),
            }])
            .collect();
        std::fs::write(fixture.archive.path().join("register.csv"), register).unwrap();
        let repo = EntryRepo::new(&fixture.pool);
        repo.replace(id, &rows).await.unwrap();
        let started = Instant::now();
        let page = repo.page(id, &Paging::default()).await.unwrap();
        println!(
            "{entries} entries: EntryRepo::page {:?} for {} rows",
            started.elapsed(),
            page.len()
        );
        let saved = fixture
            .save(&draft, |plan| {
                plan["grouping"]["pattern"] = json!(r"^(?:.*/)?(?<identifier>OBJ_\d+)\.jpg$");
                plan["sheets"] = json!([{
                    "id": "register", "source": "s1", "file": "register.csv", "hash": null,
                    "read": { "encoding": "auto", "delimiter": "auto", "quote": "\"",
                              "headerLine": 1, "trim": true },
                    "rowKind": "item",
                    "join": { "on": "identifier", "column": { "index": 0, "header": "Id" },
                              "normalize": { "caseFold": true, "leadingZeros": true, "separators": true } },
                    "itemKey": null,
                    "columns": [
                        { "column": { "index": 0, "header": "Id" }, "options": { "split": null, "type": "literal", "lang": null, "private": false } },
                        { "column": { "index": 1, "header": "Title" }, "options": { "split": null, "type": "literal", "lang": null, "private": false } },
                        { "column": { "index": 2, "header": "Date" }, "options": { "split": null, "type": "literal", "lang": null, "private": false } }
                    ]
                }]);
                plan["slots"]["item"]["dcterms:title"] = json!({ "sources": [
                    { "from": "column", "sheet": "register", "column": { "index": 1, "header": "Title" } }
                ], "combine": "first" });
                plan["slots"]["item"]["dcterms:date"] = json!({ "sources": [
                    { "from": "column", "sheet": "register", "column": { "index": 2, "header": "Date" } }
                ], "combine": "first" });
                plan["slots"]["item"]["sets"] = json!({ "sources": [
                    { "from": "folder", "level": 1 }
                ], "combine": "first" });
            })
            .await;
        assert_eq!(saved.status, StatusCode::OK, "{}", saved.body);
        let body =
            json!({ "include": ["check", "records", "flags"], "records": { "perPage": 50 } });
        let warm = fixture.preview(&draft["id"], &body).await;
        assert_eq!(warm.status, StatusCode::OK, "{}", warm.body);
        assert_eq!(warm.json()["records"]["total"], entries);
        let mut runs = Vec::new();
        for _ in 0..7 {
            let started = Instant::now();
            let answer = fixture.preview(&draft["id"], &body).await;
            runs.push(started.elapsed());
            assert_eq!(answer.status, StatusCode::OK);
        }
        let mut sorted = runs.clone();
        sorted.sort();
        let median = sorted[3];
        println!("{entries} entries: preview median {median:?} of {runs:?} (budget {budget} ms)");
        assert!(
            median < Duration::from_millis(budget),
            "{entries}: {median:?}"
        );
    }
}
