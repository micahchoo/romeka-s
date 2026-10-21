//! The undo preview and the undo keep the same things: a template or an item
//! set the import made is kept only when something the undo does not remove
//! still uses it (`docs/import-v2.md`, "Undo").

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod import_site;

use std::time::{Duration, Instant};

use import_site::{Site, jpeg, rule, site};
use omeka_db::rows::{ImportRow, ImportStatus, ImportTarget};
use serde_json::{Value, json};

/// Two artifacts and a photograph, all in one set, all of the new type
/// "Artifact".
fn archive(site: &Site) {
    let jpeg = jpeg();
    for name in ["BHC_006_AF_001", "BHC_006_AF_002", "BHC_006_PH_101"] {
        site.write_object(&format!("Things/{name}.jpg"), &jpeg);
    }
}

async fn typed_import(site: &Site) -> u64 {
    site.draft(|plan| {
        plan["grouping"]["pattern"] =
            json!(r"^(?<set>[^/]+)/(?<identifier>BHC_006_[A-Z]{2}_\d{3})\.jpg$");
        plan["slots"]["item"]["sets"] = rule(json!([{ "from": "group", "name": "set" }]), "first");
        plan["newTypes"] = json!([{ "label": "Artifact", "from": "mapping", "class": null }]);
        plan["slots"]["item"]["template"] = rule(
            json!([{ "from": "constant", "value": "Artifact" }]),
            "first",
        );
        plan["newTerms"] = json!([{ "label": "Shelf mark" }]);
        plan["slots"]["item"]["local:shelfMark"] =
            rule(json!([{ "from": "constant", "value": "A12" }]), "first");
    })
    .await
}

fn property_made(ledger: &[omeka_db::rows::ImportResourceRow]) -> u64 {
    ledger
        .iter()
        .find_map(|row| match row.target {
            ImportTarget::Property(id) => Some(id.0),
            _ => None,
        })
        .expect("the import made a field")
}

fn made(ledger: &[omeka_db::rows::ImportResourceRow]) -> (u64, u64) {
    let template = ledger
        .iter()
        .find_map(|row| match row.target {
            ImportTarget::ResourceTemplate(id) => Some(id.0),
            _ => None,
        })
        .expect("the import made a template");
    let set = ledger
        .iter()
        .find_map(|row| match row.target {
            ImportTarget::ItemSet(id) => Some(id.0),
            _ => None,
        })
        .expect("the import made an item set");
    (template, set)
}

async fn preview(site: &Site, id: u64) -> Value {
    let reply = site
        .send(
            "GET",
            &format!("/api/romeka/imports/{id}/undo/preview"),
            None,
        )
        .await;
    assert_eq!(reply.status, 200, "{}", reply.body);
    reply.json()
}

async fn undo(site: &Site, id: u64) -> ImportRow {
    let started = site
        .send("POST", &format!("/api/romeka/imports/{id}/undo"), None)
        .await;
    assert_eq!(started.status, 200, "{}", started.body);
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        let row = site.row(id).await;
        if row.status != ImportStatus::Undoing {
            return row;
        }
        assert!(Instant::now() < deadline, "import {id} stayed undoing");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

fn undo_report(row: &ImportRow) -> Value {
    row.summary.clone().unwrap_or(Value::Null)["undo"].clone()
}

fn kept_ids(report: &Value, kind: &str) -> Vec<u64> {
    report["kept"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|kept| kept["kind"] == kind)
        .map(|kept| kept["id"].as_u64().unwrap())
        .collect()
}

fn reasons(report: &Value, kind: &str, id: u64) -> Vec<String> {
    report["kept"]
        .as_array()
        .unwrap()
        .iter()
        .find(|kept| kept["kind"] == kind && kept["id"] == id)
        .map(|kept| {
            kept["reasons"]
                .as_array()
                .unwrap()
                .iter()
                .map(|reason| reason["reason"].as_str().unwrap().to_string())
                .collect()
        })
        .unwrap_or_default()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_type_and_a_set_used_only_by_the_import_are_not_kept_and_are_removed() {
    let site = site().await;
    archive(&site);
    let id = typed_import(&site).await;
    assert_eq!(site.run(id).await.status, ImportStatus::Done);

    let previewed = preview(&site, id).await;

    assert_eq!(previewed["kept"], json!([]), "{previewed}");
    assert_eq!(previewed["remove"]["templates"], 1);
    assert_eq!(previewed["remove"]["itemSets"], 1);
    assert_eq!(previewed["remove"]["properties"], 1);

    let row = undo(&site, id).await;

    assert_eq!(row.status, ImportStatus::Undone);
    let report = undo_report(&row);
    assert_eq!(report["kept"], json!([]));
    assert_eq!(
        report["removed"]["templates"],
        previewed["remove"]["templates"]
    );
    assert_eq!(
        report["removed"]["itemSets"],
        previewed["remove"]["itemSets"]
    );
    assert_eq!(
        report["removed"]["properties"],
        previewed["remove"]["properties"]
    );
    assert!(site.item_sets().await.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_type_used_by_an_item_outside_the_import_is_kept_by_both() {
    let site = site().await;
    archive(&site);
    let id = typed_import(&site).await;
    assert_eq!(site.run(id).await.status, ImportStatus::Done);
    let (template, _) = made(&site.ledger(id).await);
    let outside = site
        .send(
            "POST",
            "/api/items",
            Some(json!({
                "o:resource_template": { "o:id": template },
                "dcterms:title": [{ "type": "literal", "property_id": 1, "@value": "Mine" }]
            })),
        )
        .await;
    assert_eq!(outside.status, 200, "{}", outside.body);

    let previewed = preview(&site, id).await;

    assert_eq!(
        kept_ids(&previewed, "resourceTemplate"),
        [template],
        "{previewed}"
    );
    assert_eq!(reasons(&previewed, "resourceTemplate", template), ["inUse"]);
    assert_eq!(previewed["remove"]["templates"], 0);

    let row = undo(&site, id).await;

    assert_eq!(row.status, ImportStatus::PartlyUndone);
    let report = undo_report(&row);
    assert_eq!(kept_ids(&report, "resourceTemplate"), [template]);
    assert_eq!(report["removed"]["templates"], 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_set_that_an_item_outside_the_import_joined_is_kept_by_both() {
    let site = site().await;
    archive(&site);
    let id = typed_import(&site).await;
    assert_eq!(site.run(id).await.status, ImportStatus::Done);
    let (_, set) = made(&site.ledger(id).await);
    let outside = site
        .send(
            "POST",
            "/api/items",
            Some(json!({ "o:item_set": [{ "o:id": set }] })),
        )
        .await;
    assert_eq!(outside.status, 200, "{}", outside.body);

    let previewed = preview(&site, id).await;

    assert_eq!(kept_ids(&previewed, "itemSet"), [set], "{previewed}");
    assert_eq!(previewed["remove"]["itemSets"], 0);

    let row = undo(&site, id).await;

    assert_eq!(row.status, ImportStatus::PartlyUndone);
    let report = undo_report(&row);
    assert_eq!(kept_ids(&report, "itemSet"), [set]);
    assert_eq!(report["removed"]["itemSets"], 0);
    assert_eq!(site.item_sets().await.len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_field_used_by_an_item_outside_the_import_is_kept_by_both() {
    let site = site().await;
    archive(&site);
    let id = typed_import(&site).await;
    assert_eq!(site.run(id).await.status, ImportStatus::Done);
    let ledger = site.ledger(id).await;
    let property = property_made(&ledger);
    let outside = site
        .send(
            "POST",
            "/api/items",
            Some(json!({
                "dcterms:title": [{ "type": "literal", "property_id": 1, "@value": "Mine" }],
                "local:shelfMark": [{ "type": "literal", "property_id": property, "@value": "B3" }]
            })),
        )
        .await;
    assert_eq!(outside.status, 200, "{}", outside.body);

    let previewed = preview(&site, id).await;

    assert_eq!(kept_ids(&previewed, "property"), [property], "{previewed}");
    assert_eq!(reasons(&previewed, "property", property), ["inUse"]);
    assert_eq!(kept_ids(&previewed, "resourceTemplate"), [] as [u64; 0]);
    assert_eq!(previewed["remove"]["templates"], 1);

    let row = undo(&site, id).await;

    assert_eq!(row.status, ImportStatus::PartlyUndone);
    let report = undo_report(&row);
    assert_eq!(kept_ids(&report, "property"), [property]);
    assert_eq!(report["removed"]["templates"], 1);
}
