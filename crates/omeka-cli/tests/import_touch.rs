//! The write paths undo's touched test relies on (`docs/import-v2.md`,
//! "Touched"): each change to an imported resource, made after the import
//! through the API, a site or a job, makes undo keep that resource and say
//! why. Most paths are seen through `modified`; media added, links and set
//! members are read directly.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod import_site;

use std::time::{Duration, Instant};

use import_site::{Site, jpeg, rule, site};
use omeka_db::ids::PropertyId;
use omeka_db::repos::{JobRepo, PropertyRepo, VocabularyRepo};
use omeka_db::rows::{ImportRow, ImportStatus, ItemRow, NewJob};
use serde_json::{Value, json};

const PATTERN: &str = r"^(?<set>[^/]+)/(?<identifier>[A-Z]+_\d{3})(?:_(?<part>[a-z0-9]+))?\.jpg$";

/// What one small import wrote: two items in one set, one media each.
struct Imported {
    site: Site,
    id: u64,
    items: Vec<ItemRow>,
    set: u64,
}

impl Imported {
    fn item(&self, at: usize) -> u64 {
        self.items[at].id().0
    }

    async fn media_of(&self, at: usize) -> u64 {
        self.site.media_of(&self.items[at]).await[0].id().0
    }

    async fn send(&self, method: &str, path: &str, body: Value) -> Value {
        let reply = self.site.send(method, path, Some(body)).await;
        assert!(
            (200..300).contains(&reply.status),
            "{method} {path}: {} {}",
            reply.status,
            reply.body
        );
        if reply.body.is_empty() {
            Value::Null
        } else {
            reply.json()
        }
    }

    async fn new_site(&self, slug: &str, sets: &[u64]) -> u64 {
        let sets: Vec<Value> = sets
            .iter()
            .map(|set| json!({ "o:item_set": { "o:id": set } }))
            .collect();
        let made = self
            .send(
                "POST",
                "/api/sites",
                json!({ "o:slug": slug, "o:title": slug, "o:theme": "default", "o:site_item_set": sets }),
            )
            .await;
        made["o:id"].as_u64().unwrap()
    }

    /// Undo with no force and answer the report.
    async fn undo(&self) -> Value {
        let started = self
            .site
            .send(
                "POST",
                &format!("/api/romeka/imports/{}/undo", self.id),
                None,
            )
            .await;
        assert_eq!(started.status, 200, "{}", started.body);
        let deadline = Instant::now() + Duration::from_secs(60);
        let row: ImportRow = loop {
            let row = self.site.row(self.id).await;
            if row.status != ImportStatus::Undoing {
                break row;
            }
            assert!(Instant::now() < deadline, "the undo did not end");
            tokio::time::sleep(Duration::from_millis(20)).await;
        };
        row.summary.unwrap()["undo"].clone()
    }
}

async fn imported() -> Imported {
    let site = site().await;
    let jpeg = jpeg();
    site.write("Letters/L_001.jpg", &jpeg);
    site.write("Letters/L_002.jpg", &jpeg);
    let id = site
        .draft(|plan| {
            plan["grouping"]["pattern"] = json!(PATTERN);
            plan["slots"]["item"]["sets"] =
                rule(json!([{ "from": "group", "name": "set" }]), "first");
        })
        .await;
    let row = site.run(id).await;
    assert_eq!(row.status, ImportStatus::Done, "{:?}", row.summary);
    let mut items = site.items().await;
    items.sort_by_key(ItemRow::id);
    assert_eq!(items.len(), 2);
    let set = site.item_sets().await[0].id().0;
    // Times are whole seconds: a change in the import's own second is not seen.
    tokio::time::sleep(Duration::from_millis(1100)).await;
    Imported {
        site,
        id,
        items,
        set,
    }
}

/// The reasons undo gave for keeping one resource; empty when it did not.
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

async fn property(site: &Site, term: &str) -> PropertyId {
    let (prefix, local) = term.split_once(':').unwrap();
    let vocabulary = VocabularyRepo::new(&site.pool)
        .get_by_prefix(prefix)
        .await
        .unwrap()
        .unwrap();
    PropertyRepo::new(&site.pool)
        .find(vocabulary.id, local)
        .await
        .unwrap()
        .unwrap()
        .id
}

#[tokio::test(flavor = "multi_thread")]
async fn an_item_patch_of_its_values_keeps_it_and_nothing_else() {
    let at = imported().await;
    let title = property(&at.site, "dcterms:title").await;
    at.send(
        "PATCH",
        &format!("/api/items/{}", at.item(0)),
        json!({ "dcterms:title": [{ "type": "literal", "property_id": title.0, "@value": "Edited" }] }),
    )
    .await;

    let report = at.undo().await;

    assert_eq!(reasons(&report, "item", at.item(0)), ["edited"]);
    assert!(reasons(&report, "item", at.item(1)).is_empty(), "{report}");
    assert_eq!(report["removed"]["items"], 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn an_item_patch_of_its_sets_alone_keeps_it() {
    let at = imported().await;
    at.send(
        "PATCH",
        &format!("/api/items/{}", at.item(0)),
        json!({ "o:item_set": [] }),
    )
    .await;

    let report = at.undo().await;

    assert_eq!(reasons(&report, "item", at.item(0)), ["edited"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn an_item_patch_of_its_sites_alone_keeps_it() {
    let at = imported().await;
    let site = at.new_site("one", &[]).await;
    at.send(
        "PATCH",
        &format!("/api/items/{}", at.item(0)),
        json!({ "o:site": [{ "o:id": site }] }),
    )
    .await;

    let report = at.undo().await;

    assert_eq!(reasons(&report, "item", at.item(0)), ["edited"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn an_item_put_keeps_it() {
    let at = imported().await;
    at.send(
        "PUT",
        &format!("/api/items/{}", at.item(0)),
        json!({ "o:is_public": true }),
    )
    .await;

    let report = at.undo().await;

    assert_eq!(reasons(&report, "item", at.item(0)), ["edited"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_media_patch_keeps_the_media_and_its_item() {
    let at = imported().await;
    let media = at.media_of(0).await;
    at.send(
        "PATCH",
        &format!("/api/media/{media}"),
        json!({ "o:alt_text": "A letter" }),
    )
    .await;

    let report = at.undo().await;

    assert_eq!(reasons(&report, "media", media), ["edited"]);
    assert!(
        reasons(&report, "item", at.item(0)).is_empty(),
        "a media edit alone leaves the item to undo: {report}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn moving_a_media_in_its_item_keeps_the_item() {
    let at = imported().await;
    let media = at.media_of(0).await;
    at.send(
        "PATCH",
        &format!("/api/media/{media}"),
        json!({ "position": 7 }),
    )
    .await;

    let report = at.undo().await;

    assert_eq!(reasons(&report, "item", at.item(0)), ["edited"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn an_item_set_patch_keeps_the_set() {
    let at = imported().await;
    at.send(
        "PATCH",
        &format!("/api/item_sets/{}", at.set),
        json!({ "o:is_open": false }),
    )
    .await;

    let report = at.undo().await;

    assert_eq!(reasons(&report, "itemSet", at.set), ["edited"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_site_that_takes_the_item_keeps_it() {
    let at = imported().await;
    let site = at.new_site("pool", &[]).await;
    at.send(
        "POST",
        &format!("/api/sites/{site}/items"),
        json!({ "action": "add", "items": [at.item(0)] }),
    )
    .await;

    let report = at.undo().await;

    assert_eq!(reasons(&report, "item", at.item(0)), ["edited"]);
    assert!(reasons(&report, "item", at.item(1)).is_empty(), "{report}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_site_update_that_lists_the_set_keeps_it() {
    let at = imported().await;
    let site = at.new_site("later", &[]).await;
    at.send(
        "PATCH",
        &format!("/api/sites/{site}"),
        json!({ "o:site_item_set": [{ "o:item_set": { "o:id": at.set } }] }),
    )
    .await;

    let report = at.undo().await;

    assert_eq!(reasons(&report, "itemSet", at.set), ["edited"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_new_site_that_lists_the_set_keeps_it() {
    let at = imported().await;
    at.new_site("new", &[at.set]).await;

    let report = at.undo().await;

    assert_eq!(reasons(&report, "itemSet", at.set), ["edited"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_batch_update_keeps_what_it_edits() {
    let at = imported().await;
    let job = JobRepo::new(&at.site.pool)
        .create(&NewJob {
            owner_id: None,
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
    omeka_cli::jobs::Runner::new(&at.site.pool)
        .run(job.id)
        .await
        .unwrap();

    let report = at.undo().await;

    assert_eq!(reasons(&report, "item", at.item(0)), ["edited"]);
    assert_eq!(reasons(&report, "item", at.item(1)), ["edited"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_media_added_to_the_item_keeps_it() {
    let at = imported().await;
    at.send(
        "POST",
        "/api/media",
        json!({ "o:ingester": "html", "html": "<p>note</p>", "o:item": { "o:id": at.item(0) } }),
    )
    .await;

    let report = at.undo().await;

    assert_eq!(reasons(&report, "item", at.item(0)), ["mediaAdded"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn an_outside_item_that_links_to_it_keeps_it() {
    let at = imported().await;
    let relation = property(&at.site, "dcterms:relation").await;
    at.send(
        "POST",
        "/api/items",
        json!({ "dcterms:relation": [{
            "type": "resource:item", "property_id": relation.0, "value_resource_id": at.item(0)
        }] }),
    )
    .await;

    let report = at.undo().await;

    assert_eq!(reasons(&report, "item", at.item(0)), ["linked"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn an_outside_item_put_in_the_set_keeps_the_set() {
    let at = imported().await;
    at.send(
        "POST",
        "/api/items",
        json!({ "o:item_set": [{ "o:id": at.set }] }),
    )
    .await;

    let report = at.undo().await;

    assert_eq!(reasons(&report, "itemSet", at.set), ["membersAdded"]);
    assert_eq!(report["removed"]["items"], 2, "the import's own items go");
}
