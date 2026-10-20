//! The answers the admin needs at scale, and the list round trip, through
//! the routes (`docs/import-v2.md`, "What the admin screens need at scale",
//! "The list round trip").

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;
mod imports_fixture;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use imports_fixture::{ADMIN, BHC_PATTERN, Fixture, JPEG, bhc_006, fixture};
use serde_json::{Value, json};

const BOUNDARY: &str = "romeka-list-boundary";

async fn upload(fixture: &Fixture, id: &Value, name: &str, bytes: &[u8]) -> common::Answer {
    let mut body = format!(
        "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{name}\"\r\nContent-Type: text/csv\r\n\r\n"
    )
    .into_bytes();
    body.extend_from_slice(bytes);
    body.extend_from_slice(format!("\r\n--{BOUNDARY}--\r\n").as_bytes());
    let (identity, credential) = ADMIN;
    let request = Request::builder()
        .method("POST")
        .uri(format!(
            "/api/romeka/imports/{id}/sheets?key_identity={identity}&key_credential={credential}"
        ))
        .header(
            "content-type",
            format!("multipart/form-data; boundary={BOUNDARY}"),
        )
        .body(Body::from(body))
        .unwrap();
    common::send(fixture.app.clone(), request).await
}

/// The seeded install knows only `dcterms:title`.
async fn add_date(fixture: &Fixture) {
    use omeka_db::repos::{PropertyRepo, VocabularyRepo};
    let dcterms = VocabularyRepo::new(&fixture.pool)
        .get_by_prefix("dcterms")
        .await
        .unwrap()
        .unwrap();
    PropertyRepo::new(&fixture.pool)
        .create(&omeka_db::rows::NewProperty {
            owner_id: None,
            vocabulary_id: dcterms.id,
            local_name: "date".to_string(),
            label: "Date".to_string(),
            comment: None,
        })
        .await
        .unwrap();
}

async fn s1(fixture: &Fixture) -> Value {
    bhc_006(fixture.archive.path());
    let draft = fixture.draft().await;
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
        .call(
            "POST",
            &format!("/api/romeka/imports/{}/scan", draft["id"]),
            None,
        )
        .await;
    assert_eq!(scanned.status, StatusCode::OK, "{}", scanned.body);
    draft["id"].clone()
}

#[tokio::test]
async fn s1_shapes_histogram_and_values_through_the_routes() {
    let fixture = fixture().await;
    let id = s1(&fixture).await;
    let preview = fixture
        .preview(
            &id,
            &json!({ "include": ["shapes", "histogram", "values", "flags"], "seed": 2 }),
        )
        .await;
    assert_eq!(preview.status, StatusCode::OK, "{}", preview.body);
    let preview = preview.json();
    assert_eq!(preview["shapes"]["records"], 9);
    let groups: Vec<(Value, u64)> = preview["shapes"]["groups"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|group| (group["facts"].clone(), group["count"].as_u64().unwrap()))
        .collect();
    assert_eq!(
        groups,
        [
            (
                json!({ "files": "1", "folders": ["Photographs"], "media": ["image"],
                        "pattern": true, "row": false, "parts": [], "pages": null }),
                4
            ),
            (
                json!({ "files": "2", "folders": ["Artifacts"], "media": ["image"],
                        "pattern": true, "row": false, "parts": ["back", "front"], "pages": null }),
                4
            ),
            (
                json!({ "files": "3", "folders": ["Scans"], "media": ["image"],
                        "pattern": true, "row": false, "parts": ["p1", "p10", "p2"], "pages": null }),
                1
            ),
        ]
    );
    assert_eq!(
        preview["histogram"],
        json!([
            { "files": "0", "count": 0 }, { "files": "1", "count": 4 },
            { "files": "2", "count": 4 }, { "files": "3", "count": 1 },
            { "files": "4-6", "count": 0 }, { "files": "7+", "count": 0 }
        ])
    );
    let sets: Vec<(&str, u64)> = preview["values"][0]["values"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| {
            (
                value["text"].as_str().unwrap(),
                value["count"].as_u64().unwrap(),
            )
        })
        .collect();
    assert_eq!(sets, [("Artifacts", 4), ("Photographs", 4), ("Scans", 1)]);
    for count in preview["flags"].as_array().unwrap() {
        assert!(count["samples"].as_array().unwrap().len() <= 3, "{count}");
    }
}

#[tokio::test]
async fn identical_files_are_found_by_the_scan_only_when_size_and_hash_agree() {
    let fixture = fixture().await;
    fixture.write(&[
        ("a/A1.jpg", JPEG),
        ("b/A1 copy.jpg", JPEG),
        (
            "c/A2.jpg",
            b"\xFF\xD8\xFF\xE0\x00\x10JFIF\x00\x01\x01\x00\x00\x01\x00\x01\x00\x01",
        ),
    ]);
    let draft = fixture.draft().await;
    let scanned = fixture
        .call(
            "POST",
            &format!("/api/romeka/imports/{}/scan", draft["id"]),
            None,
        )
        .await;
    assert_eq!(scanned.status, StatusCode::OK, "{}", scanned.body);
    let preview = fixture
        .preview(&draft["id"], &json!({ "include": ["identical"] }))
        .await
        .json();
    assert_eq!(preview["identical"]["files"], 2, "{preview}");
    assert_eq!(
        preview["identical"]["sets"]["items"][0]["paths"],
        json!(["a/A1.jpg", "b/A1 copy.jpg"]),
        "c/A2.jpg has the same size and other bytes"
    );
}

#[tokio::test]
async fn the_list_round_trips_from_template_to_values_on_the_records() {
    let fixture = fixture().await;
    add_date(&fixture).await;
    let id = s1(&fixture).await;
    let template = fixture
        .call(
            "GET",
            &format!("/api/romeka/imports/{id}/template.csv?columns=dcterms:title,dcterms:date"),
            None,
        )
        .await;
    assert_eq!(template.status, StatusCode::OK, "{}", template.body);
    assert_eq!(template.headers["content-type"], "text/csv; charset=utf-8");
    let lines: Vec<&str> = template.body.split("\r\n").collect();
    assert_eq!(
        lines[0],
        "\u{feff}Identifier,Files,Title [dcterms:title],Date [dcterms:date]"
    );
    assert_eq!(
        lines.len(),
        1 + 9 + 1,
        "a row per item and a final line end"
    );
    assert_eq!(
        lines[1],
        "BHC_006_AF_001,BHC_006_AF_001_back.jpg|BHC_006_AF_001_front.jpg,,"
    );

    let mut filled: Vec<String> = lines.iter().map(|line| (*line).to_string()).collect();
    filled[1] = "BHC_006_AF_001,BHC_006_AF_001_back.jpg|BHC_006_AF_001_front.jpg,दीपक,1952".into();
    filled[2] = format!("{},Brass bowl,c. 1900", lines[2].trim_end_matches(','));
    filled[2] = filled[2].replacen(",,", ",", 1);
    let filled = filled.join("\r\n");
    let uploaded = upload(&fixture, &id, "list.csv", filled.as_bytes()).await;
    assert_eq!(uploaded.status, StatusCode::CREATED, "{}", uploaded.body);
    let view = uploaded.json();
    assert_eq!(view["revision"], 2);
    assert_eq!(view["plan"]["sheets"][0]["file"], "lists/1.csv");
    assert_eq!(
        view["plan"]["sources"][1],
        json!({ "kind": "staging", "id": "staging" })
    );
    assert!(
        fixture
            .files
            .path()
            .join(format!("staging/{id}/lists/1.csv"))
            .is_file(),
        "stored through the store, in staging"
    );

    let preview = fixture
        .preview(
            &id,
            &json!({ "include": ["records"], "records": { "slot": "dcterms:title", "perPage": 50 } }),
        )
        .await;
    assert_eq!(preview.status, StatusCode::OK, "{}", preview.body);
    let items = preview.json()["records"]["items"]
        .as_array()
        .unwrap()
        .clone();
    let value = |key: &str, term: &str| -> Vec<String> {
        items
            .iter()
            .find(|item| item["resolved"]["itemKey"] == key)
            .and_then(|item| item["slots"][term].as_array())
            .map(|values| {
                values
                    .iter()
                    .map(|value| value["text"].as_str().unwrap().to_string())
                    .collect()
            })
            .unwrap_or_default()
    };
    assert_eq!(value("BHC_006_AF_001", "dcterms:title"), ["दीपक"]);
    assert_eq!(value("BHC_006_AF_001", "dcterms:date"), ["1952"]);
    assert_eq!(value("BHC_006_AF_002", "dcterms:title"), ["Brass bowl"]);
    assert_eq!(value("BHC_006_AF_002", "dcterms:date"), ["c. 1900"]);
    assert_eq!(items.len(), 2, "only the two filled rows give a title");

    let second = upload(
        &fixture,
        &id,
        "list.tsv",
        "Identifier\tFiles\nBHC_006_PH_101\t\n".as_bytes(),
    )
    .await;
    assert_eq!(second.status, StatusCode::CREATED, "{}", second.body);
    assert_eq!(second.json()["plan"]["sheets"][1]["file"], "lists/2.tsv");
}

#[tokio::test]
async fn a_list_upload_is_refused_when_it_cannot_be_read_as_a_list() {
    let fixture = fixture().await;
    let id = s1(&fixture).await;
    let wrong_type = upload(&fixture, &id, "list.xlsx", b"PK\x03\x04").await;
    assert_eq!(
        wrong_type.status,
        StatusCode::BAD_REQUEST,
        "{}",
        wrong_type.body
    );
    let no_identifier = upload(
        &fixture,
        &id,
        "list.csv",
        b"Name,Title [dcterms:title]\nA,B\n",
    )
    .await;
    assert_eq!(
        no_identifier.status,
        StatusCode::BAD_REQUEST,
        "{}",
        no_identifier.body
    );
    assert!(
        !fixture
            .files
            .path()
            .join(format!("staging/{id}/lists/1.csv"))
            .exists(),
        "a refused list is not kept"
    );
    let too_large = upload(&fixture, &id, "list.csv", &vec![b'a'; 50 * 1024 * 1024 + 1]).await;
    assert_eq!(too_large.status, StatusCode::PAYLOAD_TOO_LARGE);
    let unknown = fixture
        .call(
            "GET",
            &format!("/api/romeka/imports/{id}/template.csv?columns=dcterms:nope"),
            None,
        )
        .await;
    assert_eq!(unknown.status, StatusCode::BAD_REQUEST, "{}", unknown.body);
    let json_body = fixture
        .call(
            "POST",
            &format!("/api/romeka/imports/{id}/sheets"),
            Some(&json!({})),
        )
        .await;
    assert_eq!(
        json_body.status,
        StatusCode::BAD_REQUEST,
        "{}",
        json_body.body
    );
}

#[tokio::test]
async fn a_running_import_shows_its_progress_and_seconds_left() {
    let fixture = fixture().await;
    let draft = fixture.draft().await;
    let id = omeka_db::ids::ImportId(draft["id"].as_u64().unwrap());
    let repo = omeka_db::repos::ImportRepo::new(&fixture.pool);
    repo.set_summary(id, &json!({ "progress": { "done": 3, "total": 12 } }))
        .await
        .unwrap();
    let shown = fixture
        .call("GET", &format!("/api/romeka/imports/{}", draft["id"]), None)
        .await
        .json();
    assert_eq!(
        shown["progress"],
        json!({ "done": 3, "total": 12, "secondsLeft": null }),
        "a draft has no running job to count from"
    );
}

/// The contract's preview budget with every section this unit adds, release
/// build, median of 7, warm index. Run with
/// `cargo test --release -p omeka-server --test imports_lists -- --ignored --nocapture`.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "release-mode timing; run by hand"]
async fn a_preview_of_40000_entries_with_every_fact_section_fits_the_budget() {
    use std::time::{Duration, Instant};

    use omeka_db::ids::ImportId;
    use omeka_db::repos::EntryRepo;
    use omeka_db::rows::{NewEntry, NewScan};

    let entries = 40_000_usize;
    let fixture = fixture().await;
    let draft = fixture.draft().await;
    let id = ImportId(draft["id"].as_u64().unwrap());
    let mut register = String::from("Id,Title,Date,Set\n");
    let rows: Vec<NewEntry> = (0..entries)
        .map(|n| {
            if n % 10 != 0 {
                register.push_str(&format!(
                    "OBJ_{n:06},Object {n},19{:02},{}\n",
                    n % 100,
                    ["Letter", "letter", "Letters", "पत्र"][n % 4]
                ));
            }
            let side = if n % 2 == 1 { "_back" } else { "_front" };
            NewEntry {
                source: "s1".into(),
                path: format!("Box {}/OBJ_{:06}{side}.jpg", n / 500, n - n % 2),
                size: 1000 + (n % 7) as u64,
                modified: Default::default(),
                media_type: Some("image/jpeg".into()),
                flags: json!({}),
            }
        })
        .collect();
    std::fs::write(fixture.archive.path().join("register.csv"), register).unwrap();
    let identical: Vec<Value> = (0..200)
        .map(|n| json!({ "size": 1000, "sha256": format!("{n:064}"),
                         "paths": [format!("Box 0/OBJ_{:06}_front.jpg", n * 2), format!("Box 1/OBJ_{:06}_front.jpg", n * 2)] }))
        .collect();
    let scan = NewScan {
        source: "s1".into(),
        anomalies: json!({ "identical": identical }),
        error: None,
        token: "t".into(),
        scanned: Default::default(),
    };
    EntryRepo::new(&fixture.pool)
        .replace(id, &[scan], &rows)
        .await
        .unwrap();
    let column = |index: usize, header: &str| {
        json!({ "column": { "index": index, "header": header },
                "options": { "split": null, "type": "literal", "lang": null, "private": false } })
    };
    let saved = fixture
        .save(&draft, |plan| {
            plan["grouping"]["pattern"] =
                json!(r"^(?:.*/)?(?<identifier>OBJ_\d+)(?:_(?<part>[a-z]+))?\.jpg$");
            plan["sheets"] = json!([{
                "id": "register", "source": "s1", "file": "register.csv", "hash": null,
                "read": { "encoding": "auto", "delimiter": "auto", "quote": "\"",
                          "headerLine": 1, "trim": true },
                "rowKind": "item",
                "join": { "on": "identifier", "column": { "index": 0, "header": "Id" },
                          "normalize": { "caseFold": true, "leadingZeros": true, "separators": true } },
                "itemKey": null,
                "columns": [column(0, "Id"), column(1, "Title"), column(2, "Date"), column(3, "Set")]
            }]);
            let from = |index: usize, header: &str| {
                json!({ "sources": [{ "from": "column", "sheet": "register",
                                      "column": { "index": index, "header": header } }],
                        "combine": "first" })
            };
            plan["slots"]["item"]["dcterms:title"] = from(1, "Title");
            plan["slots"]["item"]["sets"] = from(3, "Set");
        })
        .await;
    assert_eq!(saved.status, StatusCode::OK, "{}", saved.body);
    let measure = |body: Value| {
        let fixture = &fixture;
        let id = draft["id"].clone();
        async move {
            let warm = fixture.preview(&id, &body).await;
            assert_eq!(warm.status, StatusCode::OK, "{}", warm.body);
            let mut runs = Vec::new();
            for _ in 0..7 {
                let started = Instant::now();
                let answer = fixture.preview(&id, &body).await;
                runs.push(started.elapsed());
                assert_eq!(answer.status, StatusCode::OK);
            }
            let mut sorted = runs.clone();
            sorted.sort();
            (sorted[3], runs, warm.json())
        }
    };
    let (base, base_runs, _) =
        measure(json!({ "include": ["check", "records", "flags"], "records": { "perPage": 50 } }))
            .await;
    println!("{entries} entries, check+records+flags: median {base:?} of {base_runs:?}");
    for section in ["shapes", "histogram", "identical", "unpaired", "values"] {
        let (median, _, _) = measure(json!({ "include": ["check", "records", "flags", section],
                                             "records": { "perPage": 50 } }))
        .await;
        println!("{entries} entries, base + {section}: median {median:?}");
    }
    let (all, all_runs, answer) = measure(json!({
        "include": ["check", "records", "flags", "shapes", "histogram", "identical", "unpaired", "values"],
        "records": { "perPage": 50 }, "seed": 1
    }))
    .await;
    println!("{entries} entries, every section: median {all:?} of {all_runs:?} (budget 600 ms)");
    assert!(answer["shapes"]["records"].as_u64().unwrap() > 10_000);
    assert_eq!(answer["identical"]["sets"]["total"], 200);
    assert!(all < Duration::from_millis(600), "{all:?}");
}
