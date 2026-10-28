//! A media alone and an item set alone in the trash: a bulk delete whose change names `of`
//! (`briefs/archive-tools-2-design.md`, 5a), its undo and the purge, with the jobs run in
//! process by the same `Runner` that `romeka job run` uses.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod bulk_site;
mod import_site;

use bulk_site::*;
use import_site::PNG;
use omeka_db::ids::ItemId;
use omeka_db::repos::MediaRepo;
use omeka_db::rows::{NewMedia, NewResource};
use omeka_media::store::Store;
use serde_json::{Value, json};

fn delete_of(of: &str) -> Value {
    json!({ "kind": "delete", "of": of })
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
            position: Some(2),
            ..NewMedia::default()
        })
        .await
        .unwrap()
        .id()
        .0
}

async fn html_media(person: Option<&Person>, site: &Site, item: u64, name: &str) -> u64 {
    let body = json!({
        "o:ingester": "html", "html": "<p>note</p>", "o:item": { "o:id": item },
        "dcterms:title": title(name), "o:is_public": true,
    });
    let (status, media) = match person {
        Some(person) => person.send("POST", "/api/media", Some(body)).await,
        None => admin(site, "POST", "/api/media", Some(body)).await,
    };
    assert_eq!(status, 200, "{media}");
    media["o:id"].as_u64().unwrap()
}

/// An item whose `dcterms:relation` links to `target`.
async fn linking_item(site: &Site, name: &str, target: u64) -> u64 {
    let (status, found) = admin(site, "GET", "/api/properties?term=dcterms:relation", None).await;
    assert_eq!(status, 200, "{found}");
    let relation = found[0]["o:id"].as_u64().unwrap();
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

async fn get(site: &Site, path: &str) -> Value {
    let (status, doc) = admin(site, "GET", path, None).await;
    assert_eq!(status, 200, "{path}: {doc}");
    doc
}

async fn status_of(site: &Site, path: &str) -> u16 {
    admin(site, "GET", path, None).await.0
}

fn ids(list: &Value, key: &str) -> Vec<u64> {
    list.as_array()
        .unwrap()
        .iter()
        .map(|entry| {
            let entry = if key.is_empty() { entry } else { &entry[key] };
            entry["o:id"].as_u64().unwrap()
        })
        .collect()
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

fn exists(site: &Site, path: &str) -> bool {
    site.store.exists(path).unwrap()
}

async fn new_site(site: &Site, slug: &str, body: Value) -> u64 {
    let mut made = json!({ "o:slug": slug, "o:title": slug, "o:theme": "default" });
    for (key, value) in body.as_object().unwrap() {
        made[key] = value.clone();
    }
    let (status, made) = admin(site, "POST", "/api/sites", Some(made)).await;
    assert_eq!(status, 200, "{made}");
    made["o:id"].as_u64().unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_media_alone_goes_to_the_trash_and_undo_puts_it_back_on_its_item_with_its_file() {
    let site = site().await;
    let harbour = new_site(&site, "harbour", json!({})).await;
    let anna = item(&site, "Letter to Anna", &[]).await;
    let note = html_media(None, &site, anna, "Envelope").await;
    let png = PNG.to_vec();
    let scan = file_media(&site, anna, "annascan", &png).await;
    let (status, primary) = admin(
        &site,
        "PATCH",
        &format!("/api/items/{anna}"),
        Some(json!({ "o:primary_media": { "o:id": scan } })),
    )
    .await;
    assert_eq!(status, 200, "{primary}");
    let card = linking_item(&site, "Index card", scan).await;
    let pinned = attachment(&site, harbour, anna, scan).await;
    let order = ids(
        &get(&site, &format!("/api/items/{anna}")).await["o:media"],
        "",
    );
    assert_eq!(order, vec![note, scan]);

    let previewed = preview(&site, json!({ "id": [scan] }), delete_of("media")).await;
    assert_eq!(previewed["kind"], "delete", "{previewed}");
    assert_eq!(previewed["total"], 1, "{previewed}");
    assert_eq!(previewed["denied"], 0, "{previewed}");
    assert_eq!(previewed["media"], 1, "{previewed}");
    assert_eq!(previewed["bytes"], png.len(), "{previewed}");
    assert_eq!(previewed["samples"][0]["title"], "Scan of annascan");
    let id = previewed["id"].as_u64().unwrap();
    let ran = run(&site, id).await;
    assert_eq!(ran["status"], "done", "{ran}");
    assert_eq!(ran["counts"]["done"], 1, "{ran}");
    assert_eq!(ran["change"], delete_of("media"), "{ran}");

    assert_eq!(status_of(&site, &format!("/api/media/{scan}")).await, 404);
    let left = get(&site, &format!("/api/items/{anna}")).await;
    assert_eq!(ids(&left["o:media"], ""), vec![note], "the item stays");
    assert_eq!(
        left["o:primary_media"]["o:id"], note,
        "the first media stands in: {left}"
    );
    assert!(links_of(&site, card).await.is_empty(), "the link went");
    assert_eq!(attached(&site, pinned).await, (Some(anna as i64), None));
    assert!(!exists(&site, "original/annascan.png"));
    assert!(exists(&site, "trash/original/annascan.png"));
    assert!(exists(&site, "trash/square/annascan.jpg"));
    assert_eq!(
        file_allowed(&site, "/files/original/annascan.png").await,
        404
    );

    let undone = undo(&site, id, false).await;
    assert_eq!(undone["status"], "undone", "{undone}");
    assert_eq!(undone["counts"]["restored"], 1, "{undone}");
    let back = get(&site, &format!("/api/media/{scan}")).await;
    assert_eq!(back["o:item"]["o:id"], anna, "{back}");
    let anna_back = get(&site, &format!("/api/items/{anna}")).await;
    assert_eq!(ids(&anna_back["o:media"], ""), order, "in its place");
    assert_eq!(anna_back["o:primary_media"]["o:id"], scan, "{anna_back}");
    assert_eq!(site.store.read("original/annascan.png").unwrap(), png);
    assert!(exists(&site, "square/annascan.jpg"));
    assert!(!exists(&site, "trash/original/annascan.png"));
    assert_eq!(
        links_of(&site, card).await,
        vec![scan],
        "the link came back"
    );
    assert_eq!(
        attached(&site, pinned).await,
        (Some(anna as i64), Some(scan as i64))
    );
    assert_eq!(
        file_allowed(&site, "/files/original/annascan.png").await,
        204,
        "the proxy may serve the file again"
    );
}

/// What `/api/romeka/files/allow` answers the proxy for an anonymous `GET` of `path`.
async fn file_allowed(site: &Site, path: &str) -> u16 {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let request = format!(
        "GET /api/romeka/files/allow HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nX-Forwarded-Method: GET\r\nX-Forwarded-Uri: {path}\r\n\r\n"
    );
    let mut stream = tokio::net::TcpStream::connect(site.addr).await.unwrap();
    stream.write_all(request.as_bytes()).await.unwrap();
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).await.unwrap();
    String::from_utf8_lossy(&raw)[9..12].parse().unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_set_alone_goes_to_the_trash_and_undo_puts_back_its_members_sites_and_term_list() {
    let site = site().await;
    let letters = item_set(&site, "Letters").await;
    let other = item_set(&site, "Postcards").await;
    let members = vec![
        item(&site, "Letter to Anna", &[letters, other]).await,
        item(&site, "Letter to Bob", &[letters]).await,
        item(&site, "Letter to Cleo", &[letters]).await,
    ];
    let harbour = new_site(
        &site,
        "harbour",
        json!({
            "o:site_item_set": [{ "o:item_set": { "o:id": other } }, { "o:item_set": { "o:id": letters } }],
            "o:item_pool": { "item_set_id": [letters] },
        }),
    )
    .await;
    let card = linking_item(&site, "Index card", letters).await;
    let (status, list) = admin(
        &site,
        "POST",
        "/api/custom_vocabs",
        Some(json!({ "o:label": "Letter titles", "o:item_set": { "o:id": letters } })),
    )
    .await;
    assert_eq!(status, 200, "{list}");
    let list = list["o:id"].as_u64().unwrap();
    let pooled = format!("/api/items?item_set_id={letters}&sort_by=id");
    assert_eq!(ids(&get(&site, &pooled).await, ""), members);

    let previewed = preview(&site, json!({ "id": [letters] }), delete_of("item_sets")).await;
    assert_eq!(previewed["total"], 1, "{previewed}");
    assert_eq!(previewed["media"], 0, "{previewed}");
    assert_eq!(previewed["samples"][0]["title"], "Letters");
    let id = previewed["id"].as_u64().unwrap();
    assert_eq!(run(&site, id).await["counts"]["done"], 1);

    assert_eq!(
        status_of(&site, &format!("/api/item_sets/{letters}")).await,
        404
    );
    for member in &members {
        assert_eq!(status_of(&site, &format!("/api/items/{member}")).await, 200);
    }
    let anna = get(&site, &format!("/api/items/{}", members[0])).await;
    assert_eq!(ids(&anna["o:item_set"], ""), vec![other], "{anna}");
    let shown = get(&site, &format!("/api/sites/{harbour}")).await;
    assert_eq!(ids(&shown["o:site_item_set"], "o:item_set"), vec![other]);
    assert!(links_of(&site, card).await.is_empty());
    let vocab = get(&site, &format!("/api/custom_vocabs/{list}")).await;
    assert!(vocab["o:item_set"].is_null(), "{vocab}");

    let undone = undo(&site, id, false).await;
    assert_eq!(undone["status"], "undone", "{undone}");
    let back = get(&site, &format!("/api/item_sets/{letters}")).await;
    assert_eq!(back["dcterms:title"][0]["@value"], "Letters", "{back}");
    assert_eq!(
        ids(&get(&site, &pooled).await, ""),
        members,
        "every member is back in it"
    );
    let shown = get(&site, &format!("/api/sites/{harbour}")).await;
    assert_eq!(
        ids(&shown["o:site_item_set"], "o:item_set"),
        vec![other, letters],
        "the site shows it again, in its place"
    );
    assert_eq!(shown["o:item_pool"], json!({ "item_set_id": [letters] }));
    assert_eq!(links_of(&site, card).await, vec![letters]);
    let vocab = get(&site, &format!("/api/custom_vocabs/{list}")).await;
    assert_eq!(vocab["o:item_set"]["o:id"], letters, "{vocab}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_delete_of_media_or_sets_names_them_by_id() {
    let site = site().await;
    for of in ["media", "item_sets"] {
        let (status, refused) = admin(
            &site,
            "POST",
            "/api/romeka/bulk",
            Some(json!({ "query": { "search": "letter" }, "change": delete_of(of) })),
        )
        .await;
        assert_eq!(status, 400, "{of}: {refused}");
    }
    let (status, refused) = admin(
        &site,
        "POST",
        "/api/romeka/bulk",
        Some(json!({ "query": { "id": [1] }, "change": delete_of("sites") })),
    )
    .await;
    assert_eq!(status, 400, "{refused}");
}

/// Whether `person` moves `id` to the trash alone, as the admin's media page and set place do.
async fn trashes_alone(site: &Site, person: &Person, of: &str, id: u64) -> bool {
    let (status, previewed) = person
        .send(
            "POST",
            "/api/romeka/bulk",
            Some(json!({ "query": { "id": [id] }, "change": delete_of(of) })),
        )
        .await;
    if status == 403 {
        return false;
    }
    assert_eq!(status, 201, "{previewed}");
    let run = previewed["id"].as_u64().unwrap();
    let (status, started) = person
        .send("POST", &format!("/api/romeka/bulk/{run}/run"), None)
        .await;
    assert_eq!(status, 200, "{started}");
    let ran = settled(site, run).await;
    assert_eq!(ran["status"], "done", "{ran}");
    if ran["counts"]["failed"] == 1 {
        assert_eq!(ran["failures"][0]["reason"], "deleteDenied", "{ran}");
    }
    ran["counts"]["done"] == 1
}

/// A media and a set for each case: one to delete through the API, one to trash.
async fn two_of(site: &Site, person: Option<&Person>, of: &str) -> (u64, u64) {
    let mut made = Vec::new();
    for _ in 0..2 {
        let id = if of == "media" {
            let body = json!({ "dcterms:title": title("A letter"), "o:is_public": true });
            let (status, item) = match person {
                Some(person) => person.send("POST", "/api/items", Some(body)).await,
                None => admin(site, "POST", "/api/items", Some(body)).await,
            };
            assert_eq!(status, 200, "{item}");
            html_media(person, site, item["o:id"].as_u64().unwrap(), "A scan").await
        } else {
            let body = json!({ "dcterms:title": title("A set"), "o:is_public": true });
            let (status, set) = match person {
                Some(person) => person.send("POST", "/api/item_sets", Some(body)).await,
                None => admin(site, "POST", "/api/item_sets", Some(body)).await,
            };
            assert_eq!(status, 200, "{set}");
            set["o:id"].as_u64().unwrap()
        };
        made.push(id);
    }
    (made[0], made[1])
}

#[tokio::test(flavor = "multi_thread")]
async fn whoever_may_delete_a_media_or_set_may_trash_it_alone_and_nobody_else() {
    let site = site().await;
    let mut cases = Vec::new();
    for role in ["researcher", "author", "editor"] {
        let person = Person::new(&site, role).await;
        for of in ["media", "item_sets"] {
            let mut pairs = vec![("the administrator's", two_of(&site, None, of).await)];
            if role != "researcher" {
                pairs.push(("own", two_of(&site, Some(&person), of).await));
            }
            for (whose, (deleted, trashed)) in pairs {
                let (status, answer) = person
                    .send("DELETE", &format!("/api/{of}/{deleted}"), None)
                    .await;
                assert!(
                    [200, 204, 403].contains(&status),
                    "{role}, {whose} {of}: {status} {answer}"
                );
                let may_delete = status != 403;
                let may_trash = trashes_alone(&site, &person, of, trashed).await;
                assert_eq!(may_trash, may_delete, "{role} on {whose} {of}");
                cases.push((role, of, whose, may_delete));
            }
        }
    }
    for expected in [
        ("author", "media", "own", true),
        ("author", "media", "the administrator's", false),
        ("author", "item_sets", "the administrator's", false),
        ("researcher", "item_sets", "the administrator's", false),
        ("editor", "media", "the administrator's", true),
    ] {
        assert!(cases.contains(&expected), "{expected:?} in {cases:?}");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_media_whose_item_went_meanwhile_stays_in_the_trash_and_says_why() {
    let site = site().await;
    let anna = item(&site, "Letter to Anna", &[]).await;
    let scan = file_media(&site, anna, "annascan", PNG).await;
    let previewed = preview(&site, json!({ "id": [scan] }), delete_of("media")).await;
    let id = previewed["id"].as_u64().unwrap();
    assert_eq!(run(&site, id).await["counts"]["done"], 1);
    let (status, _) = admin(&site, "DELETE", &format!("/api/items/{anna}"), None).await;
    assert_eq!(status, 200);

    let undone = undo(&site, id, false).await;
    assert_eq!(undone["counts"]["kept"], 1, "{undone}");
    assert_eq!(undone["failures"][0]["reason"], "itemGone", "{undone}");
    assert_eq!(
        undone["failures"][0]["message"],
        "The item this media belongs to is gone, so the undo left the media in the trash."
    );
    assert_eq!(status_of(&site, &format!("/api/media/{scan}")).await, 404);
    assert!(
        exists(&site, "trash/original/annascan.png"),
        "its file stays in the trash"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn emptying_the_trash_deletes_a_trashed_media_s_files_and_counts_each_kind() {
    let site = site().await;
    let anna = item(&site, "Letter to Anna", &[]).await;
    let scan = file_media(&site, anna, "annascan", PNG).await;
    let letters = item_set(&site, "Letters").await;
    for (of, id) in [("media", scan), ("item_sets", letters)] {
        let previewed = preview(&site, json!({ "id": [id] }), delete_of(of)).await;
        assert_eq!(
            run(&site, previewed["id"].as_u64().unwrap()).await["counts"]["done"],
            1
        );
    }
    let (status, purged) = admin(&site, "POST", "/api/romeka/bulk/trash/purge", None).await;
    assert_eq!(status, 200, "{purged}");
    assert_eq!(
        purged,
        json!({ "runs": 2, "items": 0, "media": 1, "itemSets": 1 })
    );
    assert!(!exists(&site, "trash/original/annascan.png"));
    assert!(!exists(&site, "trash/square/annascan.jpg"));
    assert!(!exists(&site, "original/annascan.png"));
    assert_eq!(status_of(&site, &format!("/api/items/{anna}")).await, 200);
}
