//! Who may preview, run and undo a bulk change, and what a run or an undo may write, through
//! `/api/romeka/bulk` (`briefs/items-bulk-edit-design.md`).

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod bulk_site;
mod import_site;

use std::time::{Duration, Instant};

use bulk_site::*;
use serde_json::{Value, json};

/// The id of the account a [`Person`] signed in to: their role names it.
async fn user_id(site: &Site, role: &str) -> u64 {
    let email = if role == "admin" {
        "admin@example.org".to_string()
    } else {
        format!("{role}@example.org")
    };
    let id: i64 = sqlx::query_scalar("SELECT id FROM user WHERE email = ?")
        .bind(email)
        .fetch_one(&site.pool)
        .await
        .unwrap();
    id as u64
}

/// An item `person` makes, so they own it.
async fn own_item(person: &Person, name: &str) -> u64 {
    let (status, item) = person
        .send(
            "POST",
            "/api/items",
            Some(json!({ "dcterms:title": title(name), "o:is_public": true })),
        )
        .await;
    assert_eq!(status, 200, "{item}");
    item["o:id"].as_u64().unwrap()
}

async fn person_preview(person: &Person, query: Value, change: Value) -> (u16, Value) {
    person
        .send(
            "POST",
            "/api/romeka/bulk",
            Some(json!({ "query": query, "change": change })),
        )
        .await
}

async fn person_run(site: &Site, person: &Person, id: u64) -> Value {
    let (status, started) = person
        .send("POST", &format!("/api/romeka/bulk/{id}/run"), None)
        .await;
    assert_eq!(status, 200, "{started}");
    settled(site, id).await
}

/// A private run of `person` that made their item private, done.
async fn privatised(site: &Site, person: &Person, item: u64) -> u64 {
    let (status, previewed) = person_preview(
        person,
        json!({ "id": [item] }),
        json!({ "kind": "edit", "isPublic": false }),
    )
    .await;
    assert_eq!(status, 201, "{previewed}");
    let id = previewed["id"].as_u64().unwrap();
    assert_eq!(person_run(site, person, id).await["status"], "done");
    id
}

async fn change_user(site: &Site, role: &str, change: Value) {
    let user = user_id(site, role).await;
    let (status, changed) = admin(site, "PATCH", &format!("/api/users/{user}"), Some(change)).await;
    assert_eq!(status, 200, "{changed}");
}

async fn item_json(site: &Site, id: u64) -> Value {
    let (status, item) = admin(site, "GET", &format!("/api/items/{id}"), None).await;
    assert_eq!(status, 200, "{item}");
    item
}

#[tokio::test(flavor = "multi_thread")]
async fn a_forced_undo_leaves_an_item_the_undoer_may_no_longer_edit() {
    let site = site().await;
    let author = Person::new(&site, "author").await;
    let own = own_item(&author, "Author's letter").await;
    let id = privatised(&site, &author, own).await;
    next_second().await;
    let admin_id = user_id(&site, "admin").await;
    let (status, taken) = admin(
        &site,
        "PATCH",
        &format!("/api/items/{own}"),
        Some(json!({ "o:owner": { "o:id": admin_id }, "dcterms:title": title("Admin's letter") })),
    )
    .await;
    assert_eq!(status, 200, "{taken}");
    let (status, _) = author
        .send(
            "PATCH",
            &format!("/api/items/{own}"),
            Some(json!({ "o:is_public": true })),
        )
        .await;
    assert_eq!(status, 403, "the author may no longer edit the item");

    let (status, started) = author
        .send(
            "POST",
            &format!("/api/romeka/bulk/{id}/undo"),
            Some(json!({ "force": true })),
        )
        .await;
    assert_eq!(status, 200, "{started}");
    let undone = settled(&site, id).await;

    assert_eq!(undone["status"], "partly_undone", "{undone}");
    assert_eq!(undone["failures"][0]["item"], own, "{undone}");
    assert_eq!(undone["failures"][0]["reason"], "undoDenied", "{undone}");
    let item = item_json(&site, own).await;
    assert_eq!(item["o:owner"]["o:id"], admin_id, "{item}");
    assert_eq!(item["o:title"], "Admin's letter", "{item}");
    assert_eq!(item["o:is_public"], false, "{item}");
}

#[tokio::test(flavor = "multi_thread")]
async fn an_undo_is_refused_to_a_maker_who_lost_the_batch_right() {
    let site = site().await;
    let author = Person::new(&site, "author").await;
    let own = own_item(&author, "Author's letter").await;
    let id = privatised(&site, &author, own).await;
    change_user(&site, "author", json!({ "o:role": "researcher" })).await;
    let (status, _) = author
        .send(
            "PATCH",
            &format!("/api/items/{own}"),
            Some(json!({ "o:is_public": true })),
        )
        .await;
    assert_eq!(status, 403, "a researcher edits no item");

    let (status, refused) = author
        .send(
            "POST",
            &format!("/api/romeka/bulk/{id}/undo"),
            Some(json!({ "force": true })),
        )
        .await;

    assert_eq!(status, 403, "{refused}");
    assert_eq!(settled(&site, id).await["status"], "done");
    assert_eq!(item_json(&site, own).await["o:is_public"], false);
}

#[tokio::test(flavor = "multi_thread")]
async fn only_an_administrator_changes_owners_in_bulk() {
    let site = site().await;
    let anna = item(&site, "Letter to Anna", &[]).await;
    let editor = Person::new(&site, "editor").await;
    let editor_id = user_id(&site, "editor").await;
    let to_editor = json!({ "kind": "edit", "owner": editor_id });

    let (status, refused) =
        person_preview(&editor, json!({ "id": [anna] }), to_editor.clone()).await;
    assert_eq!(status, 403, "{refused}");

    let site_admin = Person::new(&site, "site_admin").await;
    let (status, previewed) = person_preview(&site_admin, json!({ "id": [anna] }), to_editor).await;
    assert_eq!(status, 201, "{previewed}");
    change_user(&site, "site_admin", json!({ "o:role": "editor" })).await;
    let id = previewed["id"].as_u64().unwrap();
    let (status, refused) = site_admin
        .send("POST", &format!("/api/romeka/bulk/{id}/run"), None)
        .await;
    assert_eq!(status, 403, "the right is checked again at run: {refused}");
    assert_ne!(item_json(&site, anna).await["o:owner"]["o:id"], editor_id);
}

#[tokio::test(flavor = "multi_thread")]
async fn an_item_set_the_caller_cannot_see_reads_as_missing() {
    let site = site().await;
    let (status, hidden) = admin(
        &site,
        "POST",
        "/api/item_sets",
        Some(json!({ "dcterms:title": title("Hidden"), "o:is_public": false })),
    )
    .await;
    assert_eq!(status, 200, "{hidden}");
    let hidden = hidden["o:id"].as_u64().unwrap();
    let author = Person::new(&site, "author").await;
    let own = own_item(&author, "Author's letter").await;

    for change in [
        json!({ "kind": "edit", "itemSets": { "add": [hidden] } }),
        json!({ "kind": "edit", "itemSets": { "remove": [hidden] } }),
    ] {
        let (status, refused) = person_preview(&author, json!({ "id": [own] }), change).await;
        assert_eq!(status, 404, "{refused}");
    }
    let (status, _) = person_preview(
        &author,
        json!({ "id": [own] }),
        json!({ "kind": "edit", "itemSets": { "add": [hidden + 100] } }),
    )
    .await;
    assert_eq!(status, 404, "a missing set answers the same");
}

async fn site_items(site: &Site, id: u64, action: &str, item: u64) {
    let (status, changed) = admin(
        site,
        "POST",
        &format!("/api/sites/{id}/items"),
        Some(json!({ "action": action, "items": [item] })),
    )
    .await;
    assert_eq!(status, 200, "{changed}");
}

#[tokio::test(flavor = "multi_thread")]
async fn an_undo_keeps_a_site_membership_the_undoer_may_not_assign() {
    let site = site().await;
    let (status, made) = admin(
        &site,
        "POST",
        "/api/sites",
        Some(json!({ "o:slug": "harbour", "o:title": "Harbour", "o:theme": "default" })),
    )
    .await;
    assert_eq!(status, 200, "{made}");
    let harbour = made["o:id"].as_u64().unwrap();
    let author = Person::new(&site, "author").await;
    let own = own_item(&author, "Author's letter").await;
    site_items(&site, harbour, "add", own).await;
    let id = privatised(&site, &author, own).await;
    site_items(&site, harbour, "remove", own).await;

    // The site change after the run makes the item "edited after", so only a forced undo
    // restores it (`bulk_values.rs` covers the unforced one).
    let (status, started) = author
        .send(
            "POST",
            &format!("/api/romeka/bulk/{id}/undo"),
            Some(json!({ "force": true })),
        )
        .await;
    assert_eq!(status, 200, "{started}");
    let undone = settled(&site, id).await;

    assert_eq!(undone["status"], "undone", "{undone}");
    let item = item_json(&site, own).await;
    assert_eq!(item["o:is_public"], true, "{item}");
    assert_eq!(
        item["o:site"],
        json!([]),
        "the author may not put the item back on the admin's site"
    );
}

/// A run of `person`'s over a thousand items they own, started; answers once it has written some.
async fn running(site: &Site, person: &Person, role: &str) -> u64 {
    let items = postcards(site, &[]).await;
    sqlx::query("UPDATE resource SET owner_id = ?")
        .bind(user_id(site, role).await as i64)
        .execute(&site.pool)
        .await
        .unwrap();
    let (status, previewed) = person_preview(
        person,
        json!({ "id": items }),
        json!({ "kind": "edit", "isPublic": false }),
    )
    .await;
    assert_eq!(status, 201, "{previewed}");
    assert_eq!(previewed["denied"], 0, "{previewed}");
    let id = previewed["id"].as_u64().unwrap();
    let (status, started) = person
        .send("POST", &format!("/api/romeka/bulk/{id}/run"), None)
        .await;
    assert_eq!(status, 200, "{started}");
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let (_, run) = admin(site, "GET", &format!("/api/romeka/bulk/{id}"), None).await;
        if run["counts"]["done"].as_u64().unwrap() > 0 {
            return id;
        }
        assert!(Instant::now() < deadline, "the run never began");
        tokio::time::sleep(Duration::from_millis(2)).await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_run_fails_when_its_maker_is_deactivated_mid_run() {
    let site = site().await;
    let author = Person::new(&site, "author").await;
    let id = running(&site, &author, "author").await;

    change_user(&site, "author", json!({ "o:is_active": false })).await;
    let ended = settled(&site, id).await;

    assert_eq!(
        ended["status"], "failed",
        "{} {}",
        ended["status"], ended["counts"]
    );
    assert!(
        ended["counts"]["pending"].as_u64().unwrap() > 0,
        "{} {}",
        ended["status"],
        ended["counts"]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_run_fails_when_its_maker_loses_the_batch_right_mid_run() {
    let site = site().await;
    let author = Person::new(&site, "author").await;
    let id = running(&site, &author, "author").await;

    change_user(&site, "author", json!({ "o:role": "researcher" })).await;
    let ended = settled(&site, id).await;

    assert_eq!(
        ended["status"], "failed",
        "{} {}",
        ended["status"], ended["counts"]
    );
    assert!(
        ended["counts"]["pending"].as_u64().unwrap() > 0,
        "{} {}",
        ended["status"],
        ended["counts"]
    );
    assert!(
        ended["error"].as_str().is_some(),
        "{} {}",
        ended["status"],
        ended["counts"]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn another_person_s_run_answers_as_a_missing_one_does() {
    let site = site().await;
    item(&site, "Letter to Anna", &[]).await;
    let previewed = preview(
        &site,
        json!({ "search": "Letter" }),
        json!({ "kind": "edit", "isPublic": false }),
    )
    .await;
    let id = previewed["id"].as_u64().unwrap();
    let missing = id + 100;
    let reviewer = Person::new(&site, "reviewer").await;

    for (method, path) in [
        ("GET", ""),
        ("POST", "/run"),
        ("POST", "/stop"),
        ("POST", "/undo"),
        ("POST", "/undo/preview"),
    ] {
        let (theirs, _) = reviewer
            .send(method, &format!("/api/romeka/bulk/{id}{path}"), None)
            .await;
        let (none, _) = reviewer
            .send(method, &format!("/api/romeka/bulk/{missing}{path}"), None)
            .await;
        assert_eq!((theirs, none), (404, 404), "{method} {path}");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn an_expired_preview_is_deleted_with_its_targets() {
    let site = site().await;
    item(&site, "Letter to Anna", &[]).await;
    let previewed = preview(
        &site,
        json!({ "search": "Letter" }),
        json!({ "kind": "edit", "isPublic": false }),
    )
    .await;
    let id = previewed["id"].as_u64().unwrap();
    sqlx::query("UPDATE romeka_bulk SET created = '2000-01-01 00:00:00' WHERE id = ?")
        .bind(id as i64)
        .execute(&site.pool)
        .await
        .unwrap();

    let (status, refused) = admin(&site, "POST", &format!("/api/romeka/bulk/{id}/run"), None).await;
    assert_eq!(
        (status, &refused["code"]),
        (409, &json!("expired")),
        "{refused}"
    );

    let (status, _) = admin(&site, "GET", &format!("/api/romeka/bulk/{id}"), None).await;
    assert_eq!(status, 404);
    let targets: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM romeka_bulk_target WHERE bulk_id = ?")
            .bind(id as i64)
            .fetch_one(&site.pool)
            .await
            .unwrap();
    assert_eq!(targets, 0);
}
