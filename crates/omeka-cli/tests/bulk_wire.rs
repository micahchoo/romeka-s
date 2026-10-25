//! The bulk change answers as the admin's TypeScript client reads them: one change is previewed,
//! run, edited behind its back, and undone, and each answer is the fixture
//! `web/admin/tests/fixtures/bulk/<name>.json` that `tests/bulk-api.test.ts` parses.
//!
//! `UPDATE_FIXTURES=1 cargo test -p omeka-cli --test bulk_wire` rewrites them; without it a file
//! that differs fails the test.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod import_site;

use std::path::PathBuf;
use std::time::{Duration, Instant};

use import_site::{Site, site};
use serde_json::{Value, json};

const STEADY_TIME: &str = "2026-10-24T09:00:00+00:00";

fn steady(value: &mut Value) {
    match value {
        Value::Object(members) => {
            for (key, member) in members.iter_mut() {
                if ["created", "started", "ended"].contains(&key.as_str()) && member.is_string() {
                    *member = json!(STEADY_TIME);
                } else {
                    steady(member);
                }
            }
        }
        Value::Array(items) => items.iter_mut().for_each(steady),
        _ => {}
    }
}

fn record(name: &str, mut answer: Value) {
    steady(&mut answer);
    let text = serde_json::to_string_pretty(&answer).unwrap() + "\n";
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../web/admin/tests/fixtures/bulk")
        .join(format!("{name}.json"));
    if std::env::var_os("UPDATE_FIXTURES").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, text).unwrap();
    } else {
        assert_eq!(
            std::fs::read_to_string(&path).ok().as_deref(),
            Some(text.as_str()),
            "{name} is out of date; run with UPDATE_FIXTURES=1"
        );
    }
}

async fn send(site: &Site, method: &str, path: &str, body: Option<Value>, status: u16) -> Value {
    let reply = site.send(method, path, body).await;
    assert_eq!(reply.status, status, "{method} {path}: {}", reply.body);
    serde_json::from_str(&reply.body).unwrap_or(Value::Null)
}

async fn item(site: &Site, name: &str) -> u64 {
    let title = json!([{ "type": "literal", "property_id": 1, "@value": name }]);
    let body = json!({ "dcterms:title": title, "o:is_public": true });
    send(site, "POST", "/api/items", Some(body), 200).await["o:id"]
        .as_u64()
        .unwrap()
}

/// Wait until no job works on the run, and answer it.
async fn settled(site: &Site, id: u64) -> Value {
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        let run = send(site, "GET", &format!("/api/romeka/bulk/{id}"), None, 200).await;
        let live = ["running", "stopping", "undoing", "undo_stopping"];
        if !live.contains(&run["status"].as_str().unwrap()) {
            return run;
        }
        assert!(
            Instant::now() < deadline,
            "run {id} stayed {}",
            run["status"]
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn the_admin_fixtures_are_what_the_routes_answer() {
    let site = site().await;
    let set = send(
        &site,
        "POST",
        "/api/item_sets",
        Some(json!({ "o:is_public": true })),
        200,
    )
    .await["o:id"]
        .as_u64()
        .unwrap();
    let [anna, bob, cleo] = [
        item(&site, "Letter to Anna").await,
        item(&site, "Letter to Bob").await,
        item(&site, "Letter to Cleo").await,
    ];
    let change = json!({ "kind": "edit", "isPublic": false, "itemSets": { "add": [set] } });
    let body = json!({ "query": { "search": "Letter", "sort_by": "title" }, "change": change });
    let preview = send(&site, "POST", "/api/romeka/bulk", Some(body), 201).await;
    let id = preview["id"].as_u64().unwrap();
    record("preview", preview);

    send(&site, "DELETE", &format!("/api/items/{cleo}"), None, 200).await;
    send(
        &site,
        "POST",
        &format!("/api/romeka/bulk/{id}/run"),
        None,
        200,
    )
    .await;
    record("run-done", settled(&site, id).await);
    record(
        "runs",
        send(&site, "GET", "/api/romeka/bulk", None, 200).await,
    );

    // Times are whole seconds: the edit must land in a later one to read as "edited since".
    tokio::time::sleep(Duration::from_millis(1100)).await;
    send(
        &site,
        "PATCH",
        &format!("/api/items/{bob}"),
        Some(json!({ "o:is_public": true })),
        200,
    )
    .await;
    let path = format!("/api/romeka/bulk/{id}/undo/preview");
    record("undo-preview", send(&site, "POST", &path, None, 200).await);

    let undo = format!("/api/romeka/bulk/{id}/undo");
    send(&site, "POST", &undo, Some(json!({ "force": false })), 200).await;
    let undone = settled(&site, id).await;
    assert_eq!(undone["counts"]["restored"], 1, "{undone}");
    assert_eq!(undone["counts"]["kept"], 1, "{undone}");
    record("run-partly-undone", undone);
    let anna = send(&site, "GET", &format!("/api/items/{anna}"), None, 200).await;
    assert_eq!(anna["o:is_public"], true, "Anna is back as she was");
}

#[tokio::test(flavor = "multi_thread")]
async fn the_value_edit_fixtures_are_what_the_routes_answer() {
    let site = site().await;
    let found = send(
        &site,
        "GET",
        "/api/properties?term=dcterms:subject",
        None,
        200,
    )
    .await;
    let subject = found[0]["o:id"].as_u64().unwrap();
    let photo = item(&site, "Harbour photograph").await;
    let anna = item(&site, "Letter to Anna").await;
    let change = json!({ "kind": "edit", "values": [
        { "op": "add", "property": subject, "isPublic": false,
          "value": { "type": "literal", "text": "Maps", "lang": "en" } },
        { "op": "add", "property": subject,
          "value": { "type": "uri", "uri": "https://example.org/harbour", "label": "Harbour" } },
        { "op": "add", "property": subject, "value": { "type": "item", "item": photo } },
    ] });
    let body = json!({ "query": { "id": [anna] }, "change": change });
    let preview = send(&site, "POST", "/api/romeka/bulk", Some(body), 201).await;
    let id = preview["id"].as_u64().unwrap();
    record("preview-values", preview);
    send(
        &site,
        "POST",
        &format!("/api/romeka/bulk/{id}/run"),
        None,
        200,
    )
    .await;
    let run = settled(&site, id).await;
    assert_eq!(run["counts"]["done"], 1, "{run}");
    record("run-values", run);
}
