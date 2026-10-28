//! Bulk delete of items through `/api/romeka/bulk`, its undo from the trash and the purge
//! (`briefs/items-bulk-edit-design.md`, phase 4), with the jobs run in process by the same
//! `Runner` that `romeka job run` uses.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod bulk_site;
mod import_site;

use std::time::{Duration, Instant};

use bulk_site::*;
use import_site::PNG;
use omeka_db::ids::ItemId;
use omeka_db::repos::{FulltextRepo, MediaRepo};
use omeka_db::rows::{NewMedia, NewResource};
use omeka_media::store::Store;
use serde_json::{Value, json};

const DELETE: &str = r#"{ "kind": "delete" }"#;

fn delete() -> Value {
    serde_json::from_str(DELETE).unwrap()
}

/// A file media on `item` written straight to the store: its original and three thumbnails.
async fn file_media(site: &Site, item: u64, storage_id: &str, bytes: &[u8]) -> u64 {
    site.store
        .put(
            bytes,
            &format!("original/{storage_id}.png"),
            Some("image/png"),
        )
        .unwrap();
    for kind in ["large", "medium", "square"] {
        site.store
            .put(b"thumbnail", &format!("{kind}/{storage_id}.jpg"), None)
            .unwrap();
    }
    MediaRepo::new(&site.pool)
        .create(&NewMedia {
            resource: NewResource {
                title: Some(format!("Scan of {storage_id}")),
                is_public: true,
                ..NewResource::default()
            },
            item_id: ItemId(item),
            ingester: "upload".to_string(),
            renderer: "file".to_string(),
            source: Some("scan.png".to_string()),
            media_type: Some("image/png".to_string()),
            storage_id: Some(storage_id.to_string()),
            extension: Some("png".to_string()),
            sha256: Some(String::new()),
            size: Some(bytes.len() as i64),
            has_original: true,
            has_thumbnails: true,
            ..NewMedia::default()
        })
        .await
        .unwrap()
        .id()
        .0
}

/// An HTML media on `item`, made through the API, with a title value.
async fn html_media(site: &Site, item: u64, name: &str) -> u64 {
    let (status, media) = admin(
        site,
        "POST",
        "/api/media",
        Some(json!({
            "o:ingester": "html", "html": "<p>note</p>", "o:item": { "o:id": item },
            "dcterms:title": title(name),
        })),
    )
    .await;
    assert_eq!(status, 200, "{media}");
    media["o:id"].as_u64().unwrap()
}

async fn property(site: &Site, term: &str) -> u64 {
    let (status, found) = admin(site, "GET", &format!("/api/properties?term={term}"), None).await;
    assert_eq!(status, 200, "{found}");
    found[0]["o:id"].as_u64().unwrap()
}

/// An item whose `dcterms:relation` links to `target`.
async fn linking_item(site: &Site, name: &str, target: u64) -> u64 {
    let relation = property(site, "dcterms:relation").await;
    let (status, item) = admin(
        site,
        "POST",
        "/api/items",
        Some(json!({
            "dcterms:title": title(name),
            "dcterms:relation": [{ "type": "resource", "property_id": relation, "value_resource_id": target }],
        })),
    )
    .await;
    assert_eq!(status, 200, "{item}");
    item["o:id"].as_u64().unwrap()
}

async fn links_of(site: &Site, item: u64) -> Vec<u64> {
    let (status, doc) = admin(site, "GET", &format!("/api/items/{item}"), None).await;
    assert_eq!(status, 200, "{doc}");
    doc["dcterms:relation"]
        .as_array()
        .map(|values| {
            values
                .iter()
                .filter_map(|value| value["value_resource_id"].as_u64())
                .collect()
        })
        .unwrap_or_default()
}

async fn media_of(site: &Site, item: u64) -> Vec<u64> {
    let (status, doc) = admin(site, "GET", &format!("/api/items/{item}"), None).await;
    assert_eq!(status, 200, "{doc}");
    doc["o:media"]
        .as_array()
        .unwrap()
        .iter()
        .map(|media| media["o:id"].as_u64().unwrap())
        .collect()
}

async fn new_site(site: &Site, slug: &str) -> u64 {
    let (status, made) = admin(
        site,
        "POST",
        "/api/sites",
        Some(json!({ "o:slug": slug, "o:title": slug, "o:theme": "default" })),
    )
    .await;
    assert_eq!(status, 200, "{made}");
    made["o:id"].as_u64().unwrap()
}

/// A page block on the site's first page with one attachment that names `item` and `media`.
async fn attachment(site: &Site, site_id: u64, item: u64, media: u64) -> i64 {
    let page: i64 = sqlx::query_scalar("SELECT id FROM site_page WHERE site_id = ? LIMIT 1")
        .bind(site_id as i64)
        .fetch_one(&site.pool)
        .await
        .unwrap();
    let block = sqlx::query(
        "INSERT INTO site_page_block (page_id, layout, data, position) VALUES (?, 'media', '{}', 9)",
    )
    .bind(page)
    .execute(&site.pool)
    .await
    .unwrap()
    .last_insert_rowid();
    sqlx::query(
        "INSERT INTO site_block_attachment (block_id, item_id, media_id, caption, position)
         VALUES (?, ?, ?, '', 1)",
    )
    .bind(block)
    .bind(item as i64)
    .bind(media as i64)
    .execute(&site.pool)
    .await
    .unwrap()
    .last_insert_rowid()
}

async fn attached(site: &Site, attachment: i64) -> (Option<i64>, Option<i64>) {
    sqlx::query_as("SELECT item_id, media_id FROM site_block_attachment WHERE id = ?")
        .bind(attachment)
        .fetch_one(&site.pool)
        .await
        .unwrap()
}

async fn status_of(site: &Site, path: &str) -> u16 {
    admin(site, "GET", path, None).await.0
}

async fn indexed(site: &Site, resource: &str, id: u64) -> bool {
    FulltextRepo::new(&site.pool)
        .list()
        .await
        .unwrap()
        .iter()
        .any(|row| row.resource == resource && row.id.0 == id)
}

async fn found_by_search(site: &Site, text: &str) -> Vec<u64> {
    let (status, items) = admin(
        site,
        "GET",
        &format!("/api/items?fulltext_search={text}"),
        None,
    )
    .await;
    assert_eq!(status, 200, "{items}");
    items
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["o:id"].as_u64().unwrap())
        .collect()
}

fn exists(site: &Site, path: &str) -> bool {
    site.store.exists(path).unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn deleting_all_that_match_moves_them_to_the_trash_and_undo_puts_everything_back() {
    let site = site().await;
    let letters = item_set(&site, "Letters").await;
    let harbour = new_site(&site, "harbour").await;
    let anna = item(&site, "Letter to Anna", &[letters]).await;
    let (status, _) = admin(
        &site,
        "POST",
        &format!("/api/sites/{harbour}/items"),
        Some(json!({ "action": "add", "items": [anna] })),
    )
    .await;
    assert_eq!(status, 200);
    let note = html_media(&site, anna, "Envelope").await;
    let png = PNG.to_vec();
    let scan = file_media(&site, anna, "annascan", &png).await;
    let bob = item(&site, "Letter to Bob", &[]).await;
    let photo = item(&site, "Harbour photograph", &[]).await;
    let card = linking_item(&site, "Index card", anna).await;
    let pinned = attachment(&site, harbour, anna, scan).await;
    let order = media_of(&site, anna).await;
    assert_eq!(order.len(), 2);
    assert_eq!(found_by_search(&site, "Envelope").await, vec![anna]);

    // The title, not the full text: the index card's text holds Anna's title through its link.
    let letters_query = json!({ "property": [{ "property": 1, "type": "in", "text": "Letter" }] });
    let previewed = preview(&site, letters_query, delete()).await;
    assert_eq!(previewed["kind"], "delete", "{previewed}");
    assert_eq!(previewed["total"], 2, "{previewed}");
    assert_eq!(previewed["media"], 2, "{previewed}");
    assert_eq!(previewed["bytes"], png.len(), "{previewed}");
    assert_eq!(previewed["trashDays"], 30, "{previewed}");
    assert_eq!(previewed["denied"], 0, "{previewed}");
    let samples: Vec<&str> = previewed["samples"]
        .as_array()
        .unwrap()
        .iter()
        .map(|sample| sample["title"].as_str().unwrap())
        .collect();
    assert_eq!(samples, ["Letter to Anna", "Letter to Bob"]);

    let id = previewed["id"].as_u64().unwrap();
    let ran = run(&site, id).await;
    assert_eq!(ran["status"], "done", "{ran}");
    assert_eq!(ran["kind"], "delete", "{ran}");
    assert_eq!(ran["counts"]["done"], 2, "{ran}");

    for path in [
        format!("/api/items/{anna}"),
        format!("/api/items/{bob}"),
        format!("/api/media/{note}"),
        format!("/api/media/{scan}"),
    ] {
        assert_eq!(status_of(&site, &path).await, 404, "{path} is gone");
    }
    assert_eq!(status_of(&site, &format!("/api/items/{photo}")).await, 200);
    assert!(
        found_by_search(&site, "Envelope").await.is_empty(),
        "the index forgot it"
    );
    assert!(!indexed(&site, "media", note).await);
    let (_, pool) = admin(&site, "GET", &format!("/api/items?site_id={harbour}"), None).await;
    assert_eq!(pool, json!([]), "the site's items no longer list it");
    let public = site
        .send("GET", &format!("/s/harbour/item/{anna}"), None)
        .await;
    assert_eq!(public.status, 404, "the public site no longer shows it");
    assert!(
        links_of(&site, card).await.is_empty(),
        "the link to it went too"
    );
    assert_eq!(attached(&site, pinned).await, (None, None));
    assert!(!exists(&site, "original/annascan.png"));
    assert!(exists(&site, "trash/original/annascan.png"));
    assert!(exists(&site, "trash/square/annascan.jpg"));

    let undone = undo(&site, id, false).await;
    assert_eq!(undone["status"], "undone", "{undone}");
    assert_eq!(undone["counts"]["restored"], 2, "{undone}");

    let (status, back) = admin(&site, "GET", &format!("/api/items/{anna}"), None).await;
    assert_eq!(status, 200, "{back}");
    assert_eq!(back["o:id"], anna);
    assert_eq!(back["dcterms:title"][0]["@value"], "Letter to Anna");
    assert_eq!(back["o:item_set"][0]["o:id"], letters, "{back}");
    assert_eq!(back["o:site"][0]["o:id"], harbour, "{back}");
    assert_eq!(
        media_of(&site, anna).await,
        order,
        "the same media, in the same order"
    );
    let (status, envelope) = admin(&site, "GET", &format!("/api/media/{note}"), None).await;
    assert_eq!(status, 200, "{envelope}");
    assert_eq!(envelope["dcterms:title"][0]["@value"], "Envelope");
    assert_eq!(status_of(&site, &format!("/api/items/{bob}")).await, 200);
    assert_eq!(
        site.store.read("original/annascan.png").unwrap(),
        png,
        "the original is back, byte for byte"
    );
    assert!(exists(&site, "square/annascan.jpg"));
    assert!(!exists(&site, "trash/original/annascan.png"));
    assert_eq!(
        links_of(&site, card).await,
        vec![anna],
        "the link came back"
    );
    assert_eq!(
        attached(&site, pinned).await,
        (Some(anna as i64), Some(scan as i64))
    );
    assert_eq!(
        found_by_search(&site, "Envelope").await,
        vec![anna],
        "indexed again"
    );
    assert!(indexed(&site, "media", note).await);
    let public = site
        .send("GET", &format!("/s/harbour/item/{anna}"), None)
        .await;
    assert_eq!(public.status, 200, "the public site shows it again");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_ticked_delete_takes_the_ticked_items_and_no_other() {
    let site = site().await;
    let anna = item(&site, "Letter to Anna", &[]).await;
    let bob = item(&site, "Letter to Bob", &[]).await;
    let previewed = preview(&site, json!({ "id": [bob] }), delete()).await;
    assert_eq!(previewed["total"], 1, "{previewed}");
    assert_eq!(
        run(&site, previewed["id"].as_u64().unwrap()).await["status"],
        "done"
    );
    assert_eq!(status_of(&site, &format!("/api/items/{bob}")).await, 404);
    assert_eq!(status_of(&site, &format!("/api/items/{anna}")).await, 200);
}

async fn person_preview(person: &Person, query: Value) -> (u16, Value) {
    person
        .send(
            "POST",
            "/api/romeka/bulk",
            Some(json!({ "query": query, "change": delete() })),
        )
        .await
}

async fn own_item(person: &Person, name: &str) -> u64 {
    let (status, made) = person
        .send(
            "POST",
            "/api/items",
            Some(json!({ "dcterms:title": title(name), "o:is_public": true })),
        )
        .await;
    assert_eq!(status, 200, "{made}");
    made["o:id"].as_u64().unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn delete_rights_follow_the_role_and_each_item_s_own_delete_right() {
    let site = site().await;
    let admins = item(&site, "The administrator's letter", &[]).await;
    let author = Person::new(&site, "author").await;
    let reviewer = Person::new(&site, "reviewer").await;
    let editor = Person::new(&site, "editor").await;
    let researcher = Person::new(&site, "researcher").await;

    for person in [&author, &reviewer] {
        let (status, refused) = person_preview(person, json!({ "search": "letter" })).await;
        assert_eq!(
            status, 403,
            "all that match needs batch_delete_all: {refused}"
        );
        assert!(
            refused.to_string().contains("batch_delete_all"),
            "{refused}"
        );
    }
    let (status, _) = person_preview(&researcher, json!({ "id": [admins] })).await;
    assert_eq!(status, 403, "a researcher deletes nothing");
    let (status, matching) = person_preview(&editor, json!({ "search": "letter" })).await;
    assert_eq!(
        status, 201,
        "an editor may delete all that match: {matching}"
    );
    assert_eq!(matching["denied"], 0, "an editor may delete any item");

    let mine = own_item(&author, "The author's letter").await;
    let (status, ticked) = person_preview(&author, json!({ "id": [mine, admins] })).await;
    assert_eq!(status, 201, "{ticked}");
    assert_eq!(ticked["total"], 2);
    assert_eq!(ticked["denied"], 1, "an author deletes only her own items");
    let id = ticked["id"].as_u64().unwrap();
    let (status, started) = author
        .send("POST", &format!("/api/romeka/bulk/{id}/run"), None)
        .await;
    assert_eq!(status, 200, "{started}");
    let ran = settled(&site, id).await;
    assert_eq!(ran["status"], "done", "{ran}");
    assert_eq!(ran["counts"]["done"], 1, "{ran}");
    assert_eq!(ran["counts"]["failed"], 1, "{ran}");
    assert_eq!(ran["failures"][0]["item"], admins);
    assert_eq!(ran["failures"][0]["reason"], "deleteDenied");
    assert_eq!(status_of(&site, &format!("/api/items/{admins}")).await, 200);
    assert_eq!(status_of(&site, &format!("/api/items/{mine}")).await, 404);

    let (status, started) = author
        .send(
            "POST",
            &format!("/api/romeka/bulk/{id}/undo"),
            Some(json!({})),
        )
        .await;
    assert_eq!(status, 200, "{started}");
    assert_eq!(settled(&site, id).await["status"], "undone");
    assert_eq!(status_of(&site, &format!("/api/items/{mine}")).await, 200);
}

async fn item_count(site: &Site) -> u64 {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM item")
        .fetch_one(&site.pool)
        .await
        .unwrap();
    count as u64
}

#[tokio::test(flavor = "multi_thread")]
async fn a_stop_mid_delete_leaves_a_stopped_run_whose_undo_puts_every_item_back() {
    let site = site().await;
    let items = postcards(&site, &[]).await;
    let previewed = preview(&site, json!({ "id": items }), delete()).await;
    let id = previewed["id"].as_u64().unwrap();
    start(&site, id).await;
    let (status, _) = admin(&site, "POST", &format!("/api/romeka/bulk/{id}/stop"), None).await;
    assert_eq!(status, 202);

    let stopped = settled(&site, id).await;
    assert_eq!(stopped["status"], "stopped", "{stopped}");
    let done = stopped["counts"]["done"].as_u64().unwrap();
    assert!(done > 0 && done < items.len() as u64, "{stopped}");
    assert_eq!(item_count(&site).await, items.len() as u64 - done);

    assert_eq!(undo(&site, id, false).await["status"], "undone");
    assert_eq!(item_count(&site).await, items.len() as u64);
}

#[tokio::test(flavor = "multi_thread")]
async fn undo_leaves_out_a_link_from_an_item_edited_since_and_says_so() {
    let site = site().await;
    let anna = item(&site, "Letter to Anna", &[]).await;
    let card = linking_item(&site, "Index card", anna).await;
    let note = linking_item(&site, "Note", anna).await;
    let previewed = preview(&site, json!({ "id": [anna] }), delete()).await;
    let id = previewed["id"].as_u64().unwrap();
    assert_eq!(run(&site, id).await["status"], "done");
    next_second().await;
    let (status, edited) = admin(
        &site,
        "PATCH",
        &format!("/api/items/{card}"),
        Some(json!({ "dcterms:title": title("Index card, revised") })),
    )
    .await;
    assert_eq!(status, 200, "{edited}");

    let undone = undo(&site, id, false).await;
    assert_eq!(undone["status"], "undone", "{undone}");
    assert_eq!(status_of(&site, &format!("/api/items/{anna}")).await, 200);
    assert!(
        links_of(&site, card).await.is_empty(),
        "the edited card keeps its edit"
    );
    assert_eq!(
        links_of(&site, note).await,
        vec![anna],
        "the unedited note links again"
    );
    assert_eq!(undone["failures"][0]["item"], anna, "{undone}");
    assert_eq!(undone["failures"][0]["reason"], "linksLeft", "{undone}");
}

#[tokio::test(flavor = "multi_thread")]
async fn an_item_whose_id_was_taken_meanwhile_stays_in_the_trash() {
    let site = site().await;
    let anna = item(&site, "Letter to Anna", &[]).await;
    let previewed = preview(&site, json!({ "id": [anna] }), delete()).await;
    let id = previewed["id"].as_u64().unwrap();
    assert_eq!(run(&site, id).await["status"], "done");
    sqlx::query(
        "INSERT INTO resource (id, title, is_public, created, resource_type)
         VALUES (?, 'Squatter', 1, '2026-01-01 00:00:00', 'Omeka\\Entity\\ItemSet')",
    )
    .bind(anna as i64)
    .execute(&site.pool)
    .await
    .unwrap();

    let undone = undo(&site, id, false).await;
    assert_eq!(undone["status"], "partly_undone", "{undone}");
    assert_eq!(undone["counts"]["kept"], 1, "{undone}");
    assert_eq!(undone["failures"][0]["reason"], "idTaken", "{undone}");
}

async fn purge_now(site: &Site) -> Value {
    let (status, purged) = admin(site, "POST", "/api/romeka/bulk/trash/purge", None).await;
    assert_eq!(status, 200, "{purged}");
    purged
}

async fn image_count(site: &Site) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM romeka_bulk_target WHERE image IS NOT NULL")
        .fetch_one(&site.pool)
        .await
        .unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn emptying_the_trash_removes_its_rows_and_files_and_undo_then_refuses() {
    let site = site().await;
    let anna = item(&site, "Letter to Anna", &[]).await;
    file_media(&site, anna, "annascan", PNG).await;
    let bob = item(&site, "Letter to Bob", &[]).await;
    file_media(&site, bob, "bobscan", PNG).await;
    let previewed = preview(&site, json!({ "id": [anna, bob] }), delete()).await;
    let id = previewed["id"].as_u64().unwrap();
    assert_eq!(run(&site, id).await["status"], "done");
    assert!(exists(&site, "trash/original/bobscan.png"));
    // A file already gone does not stop the purge.
    site.store.delete("trash/original/bobscan.png").unwrap();

    let reviewer = Person::new(&site, "reviewer").await;
    let (status, _) = reviewer
        .send("POST", "/api/romeka/bulk/trash/purge", None)
        .await;
    assert_eq!(status, 403, "only an administrator empties the trash");

    let purged = purge_now(&site).await;
    assert_eq!(purged["runs"], 1, "{purged}");
    assert_eq!(purged["items"], 2, "{purged}");
    for path in [
        "trash/original/annascan.png",
        "trash/square/annascan.jpg",
        "trash/large/bobscan.jpg",
        "original/annascan.png",
    ] {
        assert!(!exists(&site, path), "{path} is gone");
    }
    assert_eq!(image_count(&site).await, 0, "the snapshots are gone");
    assert_eq!(
        purge_now(&site).await["runs"],
        0,
        "a second purge finds nothing"
    );

    let (status, run) = admin(&site, "GET", &format!("/api/romeka/bulk/{id}"), None).await;
    assert_eq!(status, 200);
    assert!(run["purged"].is_string(), "{run}");
    let (status, refused) = admin(
        &site,
        "POST",
        &format!("/api/romeka/bulk/{id}/undo"),
        Some(json!({})),
    )
    .await;
    assert_eq!(status, 409, "{refused}");
    assert_eq!(refused["code"], "purged", "{refused}");
    assert!(
        refused["errors"]["error"]
            .as_str()
            .unwrap()
            .contains("emptied"),
        "{refused}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn trash_older_than_the_setting_is_emptied_when_the_recent_list_is_read() {
    let site = site().await;
    let anna = item(&site, "Letter to Anna", &[]).await;
    file_media(&site, anna, "annascan", PNG).await;
    let fresh = item(&site, "Letter to Bob", &[]).await;
    let old = preview(&site, json!({ "id": [anna] }), delete()).await["id"]
        .as_u64()
        .unwrap();
    assert_eq!(run(&site, old).await["status"], "done");
    let new = preview(&site, json!({ "id": [fresh] }), delete()).await["id"]
        .as_u64()
        .unwrap();
    assert_eq!(run(&site, new).await["status"], "done");
    let (status, settings) = admin(
        &site,
        "PATCH",
        "/api/settings",
        Some(json!({ "romeka_trash_days": 2 })),
    )
    .await;
    assert_eq!(status, 200, "{settings}");
    assert_eq!(settings["romeka_trash_days"], 2, "{settings}");
    sqlx::query("UPDATE romeka_bulk SET ended = '2000-01-01 00:00:00' WHERE id = ?")
        .bind(old as i64)
        .execute(&site.pool)
        .await
        .unwrap();

    let (status, _) = admin(&site, "GET", "/api/romeka/bulk", None).await;
    assert_eq!(status, 200);
    let deadline = Instant::now() + Duration::from_secs(30);
    while exists(&site, "trash/original/annascan.png") {
        assert!(Instant::now() < deadline, "the due trash was never emptied");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let (_, old_run) = admin(&site, "GET", &format!("/api/romeka/bulk/{old}"), None).await;
    let (_, new_run) = admin(&site, "GET", &format!("/api/romeka/bulk/{new}"), None).await;
    assert!(old_run["purged"].is_string(), "{old_run}");
    assert!(
        new_run["purged"].is_null(),
        "the fresh trash stays: {new_run}"
    );
}

/// Whether `person` moves `item` to the trash with a delete of it alone, as the admin's item page
/// does: preview, then run.
async fn trashes_alone(site: &Site, person: &Person, item: u64) -> bool {
    let (status, previewed) = person_preview(person, json!({ "id": [item] })).await;
    if status == 403 {
        return false;
    }
    assert_eq!(status, 201, "{previewed}");
    let id = previewed["id"].as_u64().unwrap();
    let (status, started) = person
        .send("POST", &format!("/api/romeka/bulk/{id}/run"), None)
        .await;
    assert_eq!(status, 200, "{started}");
    let ran = settled(site, id).await;
    assert_eq!(ran["status"], "done", "{ran}");
    ran["counts"]["done"] == 1
}

#[tokio::test(flavor = "multi_thread")]
async fn whoever_may_delete_an_item_may_trash_it_alone_and_nobody_else() {
    let site = site().await;
    let mut cases = Vec::new();
    for role in ["researcher", "author", "reviewer", "editor", "site_admin"] {
        let person = Person::new(&site, role).await;
        let mut pairs = vec![(
            "the administrator's",
            item(&site, "A letter", &[]).await,
            item(&site, "A letter", &[]).await,
        )];
        if role != "researcher" {
            pairs.push((
                "own",
                own_item(&person, "A letter").await,
                own_item(&person, "A letter").await,
            ));
        }
        for (whose, deleted, trashed) in pairs {
            let (status, answer) = person
                .send("DELETE", &format!("/api/items/{deleted}"), None)
                .await;
            assert!(
                [200, 204, 403].contains(&status),
                "{role}, {whose}: {status} {answer}"
            );
            let may_delete = status != 403;
            let may_trash = trashes_alone(&site, &person, trashed).await;
            assert_eq!(
                may_trash, may_delete,
                "{role} on {whose} item: DELETE {status}"
            );
            cases.push((role, whose, may_delete));
        }
    }
    for expected in [
        ("author", "own", true),
        ("author", "the administrator's", false),
        ("researcher", "the administrator's", false),
        ("editor", "the administrator's", true),
    ] {
        assert!(cases.contains(&expected), "{expected:?} in {cases:?}");
    }
}
