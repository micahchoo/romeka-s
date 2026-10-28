//! Item history through `GET /api/romeka/items/{id}/history` (`briefs/archive-tools-2-design.md`,
//! "Item history"): every write path that changes an item records exactly one entry naming who,
//! when and what made it, a write that changes nothing records none, and only a caller who may
//! edit the item reads its entries. The bulk, import and batch jobs run in process.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod bulk_site;
mod import_site;

use bulk_site::*;
use omeka_db::repos::JobRepo;
use omeka_db::rows::NewJob;
use serde_json::{Value, json};

async fn history(site: &Site, item: u64) -> Vec<Value> {
    let (status, answer) = admin(
        site,
        "GET",
        &format!("/api/romeka/items/{item}/history"),
        None,
    )
    .await;
    assert_eq!(status, 200, "{answer}");
    answer["entries"].as_array().unwrap().clone()
}

async fn property_id(site: &Site, term: &str) -> u64 {
    let (status, found) = admin(site, "GET", &format!("/api/properties?term={term}"), None).await;
    assert_eq!(status, 200, "{found}");
    found[0]["o:id"].as_u64().unwrap()
}

fn literal(property: u64, text: &str) -> Value {
    json!([{ "type": "literal", "property_id": property, "@value": text }])
}

/// The displays of one side of a change, in order.
fn shown(side: &Value) -> Vec<String> {
    side.as_array()
        .unwrap()
        .iter()
        .map(|value| value["display"].as_str().unwrap().to_string())
        .collect()
}

fn field<'e>(entry: &'e Value, name: &str, label: Option<&str>) -> &'e Value {
    entry["changes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|change| {
            change["field"] == name && label.is_none_or(|label| change["label"] == label)
        })
        .unwrap_or_else(|| panic!("no {name} {label:?} in {entry}"))
}

async fn patch(site: &Site, item: u64, body: Value) -> Value {
    let (status, answer) = admin(site, "PATCH", &format!("/api/items/{item}"), Some(body)).await;
    assert_eq!(status, 200, "{answer}");
    answer
}

#[tokio::test(flavor = "multi_thread")]
async fn an_edit_records_one_entry_with_each_changed_field_before_and_after() {
    let site = site().await;
    let date = property_id(&site, "dcterms:date").await;
    let anna = item(&site, "Letter", &[]).await;

    let created = history(&site, anna).await;
    assert_eq!(created.len(), 1, "the create is one entry: {created:?}");
    assert_eq!(created[0]["source"], json!({ "kind": "edit", "id": null }));
    assert_eq!(created[0]["user"]["name"], "Admin");
    assert_eq!(
        shown(&field(&created[0], "values", Some("Title"))["after"]),
        ["Letter"]
    );
    assert_eq!(
        field(&created[0], "values", Some("Title"))["before"],
        json!([])
    );

    patch(
        &site,
        anna,
        json!({ "dcterms:title": title("Letter to Anna"), "dcterms:date": literal(date, "1950"), "o:is_public": false }),
    )
    .await;

    let entries = history(&site, anna).await;
    assert_eq!(entries.len(), 2, "{entries:?}");
    let edit = &entries[0];
    let title_change = field(edit, "values", Some("Title"));
    assert_eq!(shown(&title_change["before"]), ["Letter"]);
    assert_eq!(shown(&title_change["after"]), ["Letter to Anna"]);
    let date_change = field(edit, "values", Some("Date"));
    assert_eq!(date_change["before"], json!([]));
    assert_eq!(shown(&date_change["after"]), ["1950"]);
    assert_eq!(
        field(edit, "visibility", None),
        &json!({ "field": "visibility", "before": true, "after": false })
    );
    assert_eq!(edit["changes"].as_array().unwrap().len(), 3, "{edit}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_save_that_changes_nothing_records_nothing() {
    let site = site().await;
    let anna = item(&site, "Letter to Anna", &[]).await;
    let created =
        json!({ "dcterms:title": title("Letter to Anna"), "o:item_set": [], "o:is_public": true });

    let (status, answer) = admin(&site, "PUT", &format!("/api/items/{anna}"), Some(created)).await;
    assert_eq!(status, 200, "{answer}");
    patch(
        &site,
        anna,
        json!({ "dcterms:title": title("Letter to Anna") }),
    )
    .await;

    assert_eq!(history(&site, anna).await.len(), 1, "only the create");
}

#[tokio::test(flavor = "multi_thread")]
async fn putting_a_value_back_is_an_edit_that_records_its_own_entry() {
    let site = site().await;
    let anna = item(&site, "Letter", &[]).await;
    patch(
        &site,
        anna,
        json!({ "dcterms:title": title("Letter to Anna") }),
    )
    .await;
    let before = field(&history(&site, anna).await[0], "values", Some("Title"))["before"].clone();

    patch(&site, anna, json!({ "dcterms:title": before })).await;

    let (_, shown_item) = admin(&site, "GET", &format!("/api/items/{anna}"), None).await;
    assert_eq!(shown_item["o:title"], "Letter");
    let entries = history(&site, anna).await;
    assert_eq!(entries.len(), 3);
    let back = field(&entries[0], "values", Some("Title"));
    assert_eq!(shown(&back["before"]), ["Letter to Anna"]);
    assert_eq!(shown(&back["after"]), ["Letter"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_bulk_run_records_one_entry_per_item_and_its_undo_one_restore() {
    let site = site().await;
    let letters = item_set(&site, "Letters").await;
    let items = [
        item(&site, "Letter to Anna", &[]).await,
        item(&site, "Letter to Bob", &[]).await,
    ];
    let previewed = preview(
        &site,
        json!({ "id": items }),
        json!({ "kind": "edit", "isPublic": false, "itemSets": { "add": [letters], "remove": [] } }),
    )
    .await;
    let run_id = previewed["id"].as_u64().unwrap();
    assert_eq!(run(&site, run_id).await["status"], "done");

    for item in items {
        let entries = history(&site, item).await;
        assert_eq!(entries.len(), 2, "{entries:?}");
        assert_eq!(
            entries[0]["source"],
            json!({ "kind": "bulk", "id": run_id })
        );
        assert_eq!(field(&entries[0], "visibility", None)["after"], false);
        assert_eq!(
            field(&entries[0], "item_sets", None)["after"],
            json!([{ "id": letters, "title": "Letters" }])
        );
    }

    assert_eq!(undo(&site, run_id, false).await["status"], "undone");

    for item in items {
        let entries = history(&site, item).await;
        assert_eq!(entries.len(), 3, "{entries:?}");
        assert_eq!(
            entries[0]["source"],
            json!({ "kind": "bulk_undo", "id": run_id })
        );
        assert_eq!(field(&entries[0], "visibility", None)["after"], true);
        assert_eq!(field(&entries[0], "item_sets", None)["after"], json!([]));
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_delete_undo_records_one_restore_and_keeps_the_earlier_entries() {
    let site = site().await;
    let anna = item(&site, "Letter", &[]).await;
    patch(
        &site,
        anna,
        json!({ "dcterms:title": title("Letter to Anna") }),
    )
    .await;
    let previewed = preview(&site, json!({ "id": [anna] }), json!({ "kind": "delete" })).await;
    let run_id = previewed["id"].as_u64().unwrap();
    assert_eq!(run(&site, run_id).await["status"], "done");
    let (status, _) = admin(
        &site,
        "GET",
        &format!("/api/romeka/items/{anna}/history"),
        None,
    )
    .await;
    assert_eq!(status, 404, "an item in the trash has no history to read");

    assert_eq!(undo(&site, run_id, false).await["status"], "undone");

    let entries = history(&site, anna).await;
    assert_eq!(entries.len(), 3, "{entries:?}");
    assert_eq!(
        entries[0]["source"],
        json!({ "kind": "bulk_undo", "id": run_id })
    );
    let back = field(&entries[0], "values", Some("Title"));
    assert_eq!(back["before"], json!([]));
    assert_eq!(shown(&back["after"]), ["Letter to Anna"]);
    assert_eq!(
        entries[2]["source"]["kind"], "edit",
        "the create is still there"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn the_batch_update_job_records_one_entry_per_item() {
    let site = site().await;
    let items = [
        item(&site, "Letter to Anna", &[]).await,
        item(&site, "Letter to Bob", &[]).await,
    ];
    let job = JobRepo::new(&site.pool)
        .create(&NewJob {
            owner_id: Some(omeka_db::ids::UserId(1)),
            pid: None,
            status: Some("starting".to_string()),
            class: "Omeka\\Job\\BatchUpdate".to_string(),
            args: Some(json!({
                "resource": "items",
                "data_append": { "dcterms:subject": ["letters"] }
            })),
            log: None,
            started: None,
        })
        .await
        .unwrap();
    omeka_cli::jobs::Runner::new(&omeka_db::DbPool::from(site.pool.clone()))
        .run(job.id)
        .await
        .unwrap();

    for item in items {
        let entries = history(&site, item).await;
        assert_eq!(entries.len(), 2, "{entries:?}");
        assert_eq!(entries[0]["user"]["name"], "Admin");
        assert_eq!(
            shown(&field(&entries[0], "values", Some("Subject"))["after"]),
            ["letters"]
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn only_a_caller_who_may_edit_the_item_reads_its_history() {
    let site = site().await;
    let public = item(&site, "Letter to Anna", &[]).await;
    let (status, private) = admin(
        &site,
        "POST",
        "/api/items",
        Some(json!({ "dcterms:title": title("Diary"), "o:is_public": false })),
    )
    .await;
    assert_eq!(status, 200);
    let private = private["o:id"].as_u64().unwrap();
    let author = Person::new(&site, "author").await;
    let (status, mine) = author
        .send(
            "POST",
            "/api/items",
            Some(json!({ "dcterms:title": title("My note"), "o:is_public": true })),
        )
        .await;
    assert_eq!(status, 200, "{mine}");
    let mine = mine["o:id"].as_u64().unwrap();
    let anonymous = Person {
        addr: site.addr,
        cookie: String::new(),
    };
    let path = |id: u64| format!("/api/romeka/items/{id}/history");

    assert_eq!(anonymous.send("GET", &path(public), None).await.0, 403);
    assert_eq!(anonymous.send("GET", &path(private), None).await.0, 404);
    assert_eq!(anonymous.send("GET", &path(999_999), None).await.0, 404);
    assert_eq!(
        author.send("GET", &path(private), None).await.0,
        404,
        "hidden is missing"
    );
    assert_eq!(
        author.send("GET", &path(public), None).await.0,
        403,
        "seen, not editable"
    );
    let (status, own) = author.send("GET", &path(mine), None).await;
    assert_eq!(status, 200, "{own}");
    assert_eq!(own["entries"][0]["user"]["name"], "author");
}

const BHC_PATTERN: &str = r"^(?<set>[^/]+)/(?:[^/]+/)*(?<identifier>BHC_006_[A-Z]{2}_\d{3})(?:_(?<part>[a-z0-9]+))?\.(?:jpg|tif)$";

/// Two artifacts with two sides each, as `import_undo.rs` lays out the BHC_006 demo.
fn artifacts(site: &Site) {
    let jpeg = import_site::jpeg();
    for n in 1..=2 {
        site.write_object(&format!("Artifacts/BHC_006_AF_00{n}_front.jpg"), &jpeg);
        site.write_object(&format!("Artifacts/BHC_006_AF_00{n}_back.jpg"), &jpeg);
    }
}

fn grouped(plan: &mut Value) {
    plan["grouping"]["pattern"] = json!(BHC_PATTERN);
    plan["slots"]["item"]["sets"] =
        import_site::rule(json!([{ "from": "group", "name": "set" }]), "first");
}

/// A revise of the artifacts from a register: a new title each.
async fn retitle(site: &Site) -> u64 {
    let register: String = std::iter::once("Id,Title\n".to_string())
        .chain((1..=2).map(|n| format!("BHC_006_AF_00{n},Artifact {n}\n")))
        .collect();
    site.write("register.csv", register.as_bytes());
    site.draft(|plan| {
        grouped(plan);
        plan["action"]["action"] = json!("revise");
        plan["action"]["match"]["item"]["scope"] = json!("all");
        plan["sheets"] = json!([import_site::sheet_plan("register", &["Id", "Title"])]);
        plan["sheets"][0]["join"] = json!({
            "on": "identifier", "column": { "index": 0, "header": "Id" },
            "normalize": { "caseFold": false, "leadingZeros": false, "separators": false }
        });
        plan["slots"]["item"]["dcterms:title"] =
            import_site::rule(json!([import_site::col("register", 1, "Title")]), "first");
    })
    .await
}

async fn undo_import(site: &Site, id: u64) {
    let started = site
        .send("POST", &format!("/api/romeka/imports/{id}/undo"), None)
        .await;
    assert_eq!(started.status, 200, "{}", started.body);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    while site.row(id).await.status == omeka_db::rows::ImportStatus::Undoing {
        assert!(
            std::time::Instant::now() < deadline,
            "import {id} stayed undoing"
        );
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn an_import_records_one_entry_per_item_and_its_undo_one_restore() {
    use omeka_db::rows::ImportStatus;
    let site = site().await;
    artifacts(&site);
    let first = site.draft(grouped).await;
    assert_eq!(site.run(first).await.status, ImportStatus::Done);
    let items = site.items().await;
    assert_eq!(items.len(), 2);
    for item in &items {
        let entries = history(&site, item.id().0).await;
        assert_eq!(entries.len(), 1, "{entries:?}");
        assert_eq!(
            entries[0]["source"],
            json!({ "kind": "import", "id": first })
        );
        assert_eq!(entries[0]["user"]["name"], "Admin");
    }

    let second = retitle(&site).await;
    assert_eq!(site.run(second).await.status, ImportStatus::Done);
    let one = site.item("Artifact 1").await.id().0;
    let entries = history(&site, one).await;
    assert_eq!(entries.len(), 2, "{entries:?}");
    assert_eq!(
        entries[0]["source"],
        json!({ "kind": "import", "id": second })
    );
    assert_eq!(
        shown(&field(&entries[0], "values", Some("Title"))["after"]),
        ["Artifact 1"]
    );

    undo_import(&site, second).await;

    let entries = history(&site, one).await;
    assert_eq!(entries.len(), 3, "{entries:?}");
    assert_eq!(
        entries[0]["source"],
        json!({ "kind": "import_undo", "id": second })
    );
    assert_eq!(
        shown(&field(&entries[0], "values", Some("Title"))["before"]),
        ["Artifact 1"]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_tag_merge_records_one_entry_per_item_it_changes() {
    let site = site().await;
    let subject = property_id(&site, "dcterms:subject").await;
    let mut items = Vec::new();
    for (name, tag) in [("Letter to Anna", "Co. Cork"), ("Letter to Bob", "Cork")] {
        let (status, made) = admin(
            &site,
            "POST",
            "/api/items",
            Some(json!({ "dcterms:title": title(name), "dcterms:subject": literal(subject, tag) })),
        )
        .await;
        assert_eq!(status, 200, "{made}");
        items.push(made["o:id"].as_u64().unwrap());
    }

    let (status, merged) = admin(
        &site,
        "POST",
        "/api/tags/merge",
        Some(json!({ "from": ["Co. Cork"], "to": "Cork" })),
    )
    .await;
    assert_eq!(status, 200, "{merged}");

    let anna = history(&site, items[0]).await;
    assert_eq!(anna.len(), 2, "{anna:?}");
    let change = field(&anna[0], "values", Some("Subject"));
    assert_eq!(shown(&change["before"]), ["Co. Cork"]);
    assert_eq!(shown(&change["after"]), ["Cork"]);
    assert_eq!(
        history(&site, items[1]).await.len(),
        1,
        "Bob's tag was already Cork"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn adding_items_to_a_site_records_one_entry_per_item_it_joins() {
    let site = site().await;
    let (status, made) = admin(
        &site,
        "POST",
        "/api/sites",
        Some(json!({ "o:slug": "letters", "o:title": "Letters", "o:theme": "default" })),
    )
    .await;
    assert_eq!(status, 200, "{made}");
    let site_id = made["o:id"].as_u64().unwrap();
    let anna = item(&site, "Letter to Anna", &[]).await;
    let bob = item(&site, "Letter to Bob", &[]).await;

    let (status, changed) = admin(
        &site,
        "POST",
        &format!("/api/sites/{site_id}/items"),
        Some(json!({ "action": "add", "items": [anna] })),
    )
    .await;
    assert_eq!(status, 200, "{changed}");

    let entries = history(&site, anna).await;
    assert_eq!(entries.len(), 2, "{entries:?}");
    assert_eq!(
        field(&entries[0], "sites", None)["after"],
        json!([{ "id": site_id, "title": "Letters" }])
    );
    assert_eq!(history(&site, bob).await.len(), 1, "Bob was not picked");
}

/// What one item save costs over HTTP, entry included, for scale beside the repository
/// measurement (`omeka-db/tests/item_history.rs`). Run with `cargo test -p omeka-cli --release
/// --test item_history -- --ignored --nocapture`.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "a measurement, not a check"]
async fn measure_a_save_over_http() {
    const SAVES: u32 = 200;
    let site = site().await;
    let date = property_id(&site, "dcterms:date").await;
    let anna = item(&site, "Letter to Anna", &[]).await;
    let started = std::time::Instant::now();
    for n in 0..SAVES {
        patch(
            &site,
            anna,
            json!({ "dcterms:title": title("Letter to Anna"), "dcterms:date": literal(date, &format!("19{:02}", n % 100)) }),
        )
        .await;
    }
    println!(
        "PATCH with entry {:?} per save",
        started.elapsed() / SAVES
    );
}
