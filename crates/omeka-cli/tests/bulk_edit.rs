//! Bulk edit of items through `/api/romeka/bulk`, with the run and undo jobs run in process by
//! the same `Runner` that `romeka job run` uses (`briefs/items-bulk-edit-design.md`, phase 1).

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod bulk_site;
mod import_site;

use std::time::{Duration, Instant};

use bulk_site::*;
use omeka_db::repos::FulltextRepo;
use serde_json::json;

#[tokio::test(flavor = "multi_thread")]
async fn a_search_query_touches_only_the_items_it_matches() {
    let site = site().await;
    let anna = item(&site, "Letter to Anna", &[]).await;
    let bob = item(&site, "Letter to Bob", &[]).await;
    let photo = item(&site, "Harbour photograph", &[]).await;

    let previewed = preview(
        &site,
        json!({ "search": "Letter" }),
        json!({ "kind": "edit", "isPublic": false }),
    )
    .await;
    assert_eq!(previewed["total"], 2);
    assert_eq!(previewed["unchanged"], 0);
    assert_eq!(previewed["samples"].as_array().unwrap().len(), 2);

    let ran = run(&site, previewed["id"].as_u64().unwrap()).await;
    assert_eq!(ran["status"], "done", "{ran}");
    assert_eq!(ran["counts"]["done"], 2);
    assert!(!shown(&site, anna).await.public);
    assert!(!shown(&site, bob).await.public);
    assert!(
        shown(&site, photo).await.public,
        "the photograph is not a letter"
    );
}

/// The `fulltext_search` row of an item: its visibility, which a search by an anonymous reader
/// filters on.
async fn indexed_public(site: &Site, id: u64) -> bool {
    FulltextRepo::new(&site.pool)
        .list()
        .await
        .unwrap()
        .into_iter()
        .find(|row| row.resource == "items" && row.id.0 == id)
        .expect("an index row")
        .is_public
}

#[tokio::test(flavor = "multi_thread")]
async fn the_search_index_follows_a_visibility_change_and_its_undo() {
    let site = site().await;
    let anna = item(&site, "Letter to Anna", &[]).await;
    assert!(indexed_public(&site, anna).await);
    let previewed = preview(
        &site,
        json!({ "id": [anna] }),
        json!({ "kind": "edit", "isPublic": false }),
    )
    .await;
    let id = previewed["id"].as_u64().unwrap();

    assert_eq!(run(&site, id).await["status"], "done");
    assert!(!indexed_public(&site, anna).await, "the run hid the item");

    assert_eq!(undo(&site, id, false).await["status"], "undone");
    assert!(
        indexed_public(&site, anna).await,
        "the undo showed it again"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn the_run_touches_the_previewed_items_even_when_one_is_added_after_the_preview() {
    let site = site().await;
    item(&site, "Letter to Anna", &[]).await;
    item(&site, "Letter to Bob", &[]).await;
    let previewed = preview(
        &site,
        json!({ "search": "Letter" }),
        json!({ "kind": "edit", "isPublic": false }),
    )
    .await;
    let later = item(&site, "Letter to Cleo", &[]).await;

    let ran = run(&site, previewed["id"].as_u64().unwrap()).await;

    assert_eq!(previewed["total"], ran["total"]);
    assert_eq!(ran["counts"]["done"], previewed["total"]);
    assert!(
        shown(&site, later).await.public,
        "Cleo came after the preview"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn undo_restores_the_class_the_visibility_and_the_sets() {
    let site = site().await;
    let (letters, archive) = (
        item_set(&site, "Letters").await,
        item_set(&site, "Archive").await,
    );
    let letter = class(&site, "bibo:Letter").await;
    let items = [
        item(&site, "Letter to Anna", &[letters]).await,
        item(&site, "Letter to Bob", &[letters]).await,
    ];
    let before = shown(&site, items[0]).await;
    let previewed = preview(
        &site,
        json!({ "id": items }),
        json!({
            "kind": "edit", "isPublic": false, "class": letter,
            "itemSets": { "add": [archive], "remove": [letters] },
        }),
    )
    .await;
    let id = previewed["id"].as_u64().unwrap();
    assert_eq!(run(&site, id).await["status"], "done");
    let changed = Shown {
        public: false,
        class: Some(letter),
        sets: [archive].into(),
    };
    for item in items {
        assert_eq!(shown(&site, item).await, changed);
    }

    let undone = undo(&site, id, false).await;

    assert_eq!(undone["status"], "undone", "{undone}");
    assert_eq!(undone["counts"]["restored"], 2);
    for item in items {
        assert_eq!(shown(&site, item).await, before);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn undo_keeps_an_item_edited_after_the_run_and_force_restores_it() {
    let site = site().await;
    let (letter, person) = (
        class(&site, "bibo:Letter").await,
        class(&site, "foaf:Person").await,
    );
    let edited = item(&site, "Letter to Anna", &[]).await;
    let other = item(&site, "Letter to Bob", &[]).await;
    let previewed = preview(
        &site,
        json!({ "id": [edited, other] }),
        json!({ "kind": "edit", "class": letter }),
    )
    .await;
    let id = previewed["id"].as_u64().unwrap();
    assert_eq!(run(&site, id).await["status"], "done");
    next_second().await;
    let (status, patched) = admin(
        &site,
        "PATCH",
        &format!("/api/items/{edited}"),
        Some(json!({ "o:resource_class": { "o:id": person } })),
    )
    .await;
    assert_eq!(status, 200, "{patched}");

    let (status, would) = admin(
        &site,
        "POST",
        &format!("/api/romeka/bulk/{id}/undo/preview"),
        None,
    )
    .await;
    assert_eq!(status, 200, "{would}");
    assert_eq!(would["restore"], 1);
    assert_eq!(would["editedItems"], json!([edited]));

    let kept = undo(&site, id, false).await;
    assert_eq!(kept["status"], "partly_undone", "{kept}");
    assert_eq!(kept["counts"]["kept"], 1);
    assert_eq!(shown(&site, edited).await.class, Some(person));
    assert_eq!(shown(&site, other).await.class, None);

    let forced = undo(&site, id, true).await;
    assert_eq!(forced["status"], "undone", "{forced}");
    assert_eq!(shown(&site, edited).await.class, None);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_rerun_after_a_crash_applies_each_item_once_and_undo_still_restores_it() {
    let site = site().await;
    let (letters, archive) = (
        item_set(&site, "Letters").await,
        item_set(&site, "Archive").await,
    );
    let items = postcards(&site, &[letters]).await;
    let previewed = preview(
        &site,
        json!({ "id": items }),
        json!({ "kind": "edit", "itemSets": { "add": [archive], "remove": [letters] } }),
    )
    .await;
    let id = previewed["id"].as_u64().unwrap();
    start(&site, id).await;
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let (_, run) = admin(&site, "GET", &format!("/api/romeka/bulk/{id}"), None).await;
        if run["counts"]["done"].as_u64().unwrap() > 0 {
            break;
        }
        assert!(Instant::now() < deadline, "the run never began");
        tokio::time::sleep(Duration::from_millis(2)).await;
    }
    for task in site.jobs.tasks.lock().unwrap().drain(..) {
        task.abort();
    }
    // The worker is gone; let its heartbeat age past the dead line.
    sqlx::query("UPDATE romeka_bulk SET heartbeat = '2000-01-01 00:00:00' WHERE id = ?")
        .bind(id as i64)
        .execute(&site.pool)
        .await
        .unwrap();
    let crashed = settled(&site, id).await;
    assert_eq!(crashed["status"], "stopped", "{crashed}");
    assert!(
        crashed["counts"]["pending"].as_u64().unwrap() > 0,
        "the crash came before the end: {crashed}"
    );

    let rerun = run(&site, id).await;

    assert_eq!(rerun["status"], "done", "{rerun}");
    assert_eq!(rerun["counts"]["done"], items.len());
    assert_eq!(members(&site).await, [(archive, items.len() as u64)].into());
    assert_eq!(undo(&site, id, false).await["status"], "undone");
    assert_eq!(
        members(&site).await,
        [(letters, items.len() as u64)].into(),
        "each before-image is the one the first write kept"
    );
}

/// How many items each item set holds.
async fn members(site: &Site) -> std::collections::BTreeMap<u64, u64> {
    let rows: Vec<(i64, i64)> =
        sqlx::query_as("SELECT item_set_id, COUNT(*) FROM item_item_set GROUP BY item_set_id")
            .fetch_all(&site.pool)
            .await
            .unwrap();
    rows.into_iter()
        .map(|(set, count)| (set as u64, count as u64))
        .collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_stop_mid_run_leaves_a_stopped_run_that_undo_reverses() {
    let site = site().await;
    let items = postcards(&site, &[]).await;
    let previewed = preview(
        &site,
        json!({ "id": items }),
        json!({ "kind": "edit", "isPublic": false }),
    )
    .await;
    let id = previewed["id"].as_u64().unwrap();
    start(&site, id).await;
    let (status, _) = admin(&site, "POST", &format!("/api/romeka/bulk/{id}/stop"), None).await;
    assert_eq!(status, 202);

    let stopped = settled(&site, id).await;

    assert_eq!(stopped["status"], "stopped", "{stopped}");
    let done = stopped["counts"]["done"].as_u64().unwrap();
    assert!(done > 0 && done < items.len() as u64, "{stopped}");
    assert_eq!(
        done + stopped["counts"]["pending"].as_u64().unwrap(),
        items.len() as u64
    );
    assert_eq!(
        private_items(&site).await,
        done,
        "each done target is one written item"
    );

    assert_eq!(undo(&site, id, false).await["status"], "undone");
    assert_eq!(private_items(&site).await, 0);
}

async fn private_items(site: &Site) -> u64 {
    let private: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM resource JOIN item ON item.id = resource.id WHERE is_public = 0",
    )
    .fetch_one(&site.pool)
    .await
    .unwrap();
    private as u64
}

#[tokio::test(flavor = "multi_thread")]
async fn a_reviewer_edits_all_an_author_only_ticked_items_and_a_researcher_nothing() {
    let site = site().await;
    let theirs = item(&site, "Letter to Anna", &[]).await;
    let all =
        json!({ "query": { "search": "Letter" }, "change": { "kind": "edit", "isPublic": false } });

    let reviewer = Person::new(&site, "reviewer").await;
    let (status, previewed) = reviewer
        .send("POST", "/api/romeka/bulk", Some(all.clone()))
        .await;
    assert_eq!(status, 201, "{previewed}");

    let author = Person::new(&site, "author").await;
    let (status, refused) = author.send("POST", "/api/romeka/bulk", Some(all)).await;
    assert_eq!(status, 403, "{refused}");
    let (status, own) = author
        .send(
            "POST",
            "/api/items",
            Some(json!({ "dcterms:title": title("Author's letter"), "o:is_public": true })),
        )
        .await;
    assert_eq!(status, 200, "{own}");
    let own = own["o:id"].as_u64().unwrap();
    let ticked = json!({
        "query": { "id": [own, theirs] },
        "change": { "kind": "edit", "isPublic": false },
    });
    let (status, previewed) = author
        .send("POST", "/api/romeka/bulk", Some(ticked.clone()))
        .await;
    assert_eq!(status, 201, "{previewed}");
    assert_eq!(
        previewed["denied"], 1,
        "the author may not update the admin's item"
    );
    let id = previewed["id"].as_u64().unwrap();
    let (status, _) = author
        .send("POST", &format!("/api/romeka/bulk/{id}/run"), None)
        .await;
    assert_eq!(status, 200);
    let ran = settled(&site, id).await;
    assert_eq!(ran["status"], "done", "{ran}");
    assert_eq!(
        ran["failures"],
        json!([{
            "item": theirs,
            "reason": "denied",
            "message": "The person who started this change may not edit this item.",
        }])
    );
    assert!(!shown(&site, own).await.public);
    assert!(shown(&site, theirs).await.public);

    let researcher = Person::new(&site, "researcher").await;
    let (status, refused) = researcher
        .send("POST", "/api/romeka/bulk", Some(ticked))
        .await;
    assert_eq!(status, 403, "{refused}");
    let (status, _) = researcher
        .send("POST", &format!("/api/romeka/bulk/{id}/run"), None)
        .await;
    assert_eq!(status, 403, "a run belongs to the person who previewed it");
}

#[tokio::test]
async fn the_recent_list_shows_no_one_else_s_runs_to_a_signed_out_or_ordinary_caller() {
    let site = site().await;
    item(&site, "Letter to Anna", &[]).await;
    preview(
        &site,
        json!({ "search": "Letter" }),
        json!({ "kind": "edit", "isPublic": false }),
    )
    .await;
    let (status, runs) = admin(&site, "GET", "/api/romeka/bulk", None).await;
    assert_eq!((status, runs.as_array().unwrap().len()), (200, 1), "{runs}");

    let signed_out = Person {
        addr: site.addr,
        cookie: String::new(),
    };
    let (status, runs) = signed_out.send("GET", "/api/romeka/bulk", None).await;
    assert_eq!(
        (status, runs),
        (200, json!([])),
        "a signed-out caller has no runs"
    );

    let researcher = Person::new(&site, "researcher").await;
    let (status, runs) = researcher.send("GET", "/api/romeka/bulk", None).await;
    assert_eq!(
        (status, runs),
        (200, json!([])),
        "only the admin made a run"
    );
}
