//! Bulk edit of items through `/api/romeka/bulk`, with the run and undo jobs run in process by
//! the same `Runner` that `romeka job run` uses (`briefs/items-bulk-edit-design.md`, phase 1).

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod import_site;

use std::collections::BTreeSet;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

use import_site::{Site, site};
use omeka_db::ids::ItemSetId;
use omeka_db::repos::{ItemRepo, UserRepo};
use omeka_db::rows::{NewItem, NewResource, NewUser, NewWholeItem};
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const PASSWORD: &str = "a long test password";

/// A signed-in person other than the site's administrator.
struct Person {
    addr: SocketAddr,
    cookie: String,
}

impl Person {
    async fn new(site: &Site, role: &str) -> Self {
        let email = format!("{role}@example.org");
        UserRepo::new(&site.pool)
            .create(&NewUser {
                email: email.clone(),
                name: role.to_string(),
                password_hash: Some(bcrypt::hash(PASSWORD, 4).unwrap()),
                role: role.to_string(),
                is_active: true,
                created: None,
            })
            .await
            .unwrap();
        let mut person = Self {
            addr: site.addr,
            cookie: String::new(),
        };
        let (status, head, body) = person
            .raw(
                "POST",
                "/api/login",
                Some(json!({ "email": email, "password": PASSWORD })),
            )
            .await;
        assert_eq!(status, 200, "{body}");
        let cookie = head
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("set-cookie")
                    .then(|| value.trim())
            })
            .expect("a session cookie");
        person.cookie = cookie.split(';').next().unwrap().to_string();
        person
    }

    async fn send(&self, method: &str, path: &str, body: Option<Value>) -> (u16, Value) {
        let (status, _, body) = self.raw(method, path, body).await;
        (status, serde_json::from_str(&body).unwrap_or(Value::Null))
    }

    async fn raw(&self, method: &str, path: &str, body: Option<Value>) -> (u16, String, String) {
        let body = body.map(|body| body.to_string()).unwrap_or_default();
        let mut request = format!(
            "{method} {path} HTTP/1.1\r\nHost: localhost\r\nOrigin: http://localhost\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n",
            body.len()
        );
        if !self.cookie.is_empty() {
            request.push_str(&format!("Cookie: {}\r\n", self.cookie));
        }
        request.push_str("\r\n");
        request.push_str(&body);
        let mut stream = tokio::net::TcpStream::connect(self.addr).await.unwrap();
        stream.write_all(request.as_bytes()).await.unwrap();
        let mut raw = Vec::new();
        stream.read_to_end(&mut raw).await.unwrap();
        let raw = String::from_utf8_lossy(&raw).into_owned();
        let (head, body) = raw.split_once("\r\n\r\n").expect("a head");
        let body = if head
            .to_ascii_lowercase()
            .contains("transfer-encoding: chunked")
        {
            unchunk(body)
        } else {
            body.to_string()
        };
        (head[9..12].parse().unwrap(), head.to_string(), body)
    }
}

fn unchunk(mut rest: &str) -> String {
    let mut out = String::new();
    while let Some((size, after)) = rest.split_once("\r\n") {
        let size = usize::from_str_radix(size.trim(), 16).unwrap_or(0);
        if size == 0 {
            break;
        }
        out.push_str(&after[..size]);
        rest = &after[size + 2..];
    }
    out
}

/// The site administrator, as a [`Person`]'s answers read.
async fn admin(site: &Site, method: &str, path: &str, body: Option<Value>) -> (u16, Value) {
    let reply = site.send(method, path, body).await;
    (
        reply.status,
        serde_json::from_str(&reply.body).unwrap_or(Value::Null),
    )
}

fn title(text: &str) -> Value {
    json!([{ "type": "literal", "property_id": 1, "@value": text }])
}

async fn item_set(site: &Site, name: &str) -> u64 {
    let (status, set) = admin(
        site,
        "POST",
        "/api/item_sets",
        Some(json!({ "dcterms:title": title(name) })),
    )
    .await;
    assert_eq!(status, 200, "{set}");
    set["o:id"].as_u64().unwrap()
}

async fn item(site: &Site, name: &str, sets: &[u64]) -> u64 {
    let sets: Vec<Value> = sets.iter().map(|id| json!({ "o:id": id })).collect();
    let (status, item) = admin(
        site,
        "POST",
        "/api/items",
        Some(json!({ "dcterms:title": title(name), "o:item_set": sets, "o:is_public": true })),
    )
    .await;
    assert_eq!(status, 200, "{item}");
    item["o:id"].as_u64().unwrap()
}

async fn class(site: &Site, term: &str) -> u64 {
    let (status, classes) = admin(
        site,
        "GET",
        &format!("/api/resource_classes?term={term}"),
        None,
    )
    .await;
    assert_eq!(status, 200, "{classes}");
    classes[0]["o:id"].as_u64().unwrap()
}

/// What a bulk edit may change on an item, as `GET /api/items/{id}` shows it.
#[derive(Debug, PartialEq, Eq)]
struct Shown {
    public: bool,
    class: Option<u64>,
    sets: BTreeSet<u64>,
}

async fn shown(site: &Site, id: u64) -> Shown {
    let (status, item) = admin(site, "GET", &format!("/api/items/{id}"), None).await;
    assert_eq!(status, 200, "{item}");
    Shown {
        public: item["o:is_public"].as_bool().unwrap(),
        class: item["o:resource_class"]["o:id"].as_u64(),
        sets: item["o:item_set"]
            .as_array()
            .unwrap()
            .iter()
            .map(|set| set["o:id"].as_u64().unwrap())
            .collect(),
    }
}

async fn preview(site: &Site, query: Value, change: Value) -> Value {
    let (status, preview) = admin(
        site,
        "POST",
        "/api/romeka/bulk",
        Some(json!({ "query": query, "change": change })),
    )
    .await;
    assert_eq!(status, 201, "{preview}");
    preview
}

/// Wait until no job works on the run, and answer it.
async fn settled(site: &Site, id: u64) -> Value {
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        let (status, run) = admin(site, "GET", &format!("/api/romeka/bulk/{id}"), None).await;
        assert_eq!(status, 200, "{run}");
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

async fn run(site: &Site, id: u64) -> Value {
    let (status, started) = admin(site, "POST", &format!("/api/romeka/bulk/{id}/run"), None).await;
    assert_eq!(status, 200, "{started}");
    settled(site, id).await
}

async fn undo(site: &Site, id: u64, force: bool) -> Value {
    let (status, started) = admin(
        site,
        "POST",
        &format!("/api/romeka/bulk/{id}/undo"),
        Some(json!({ "force": force })),
    )
    .await;
    assert_eq!(status, 200, "{started}");
    settled(site, id).await
}

/// Wait out the second the run wrote in: times are whole seconds.
async fn next_second() {
    tokio::time::sleep(Duration::from_millis(1100)).await;
}

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

/// Enough public items that the job is still at work when the test acts on it, written straight
/// to the store: the API would take too long to make them.
async fn postcards(site: &Site, sets: &[u64]) -> Vec<u64> {
    let mut items = Vec::new();
    for n in 0..1000 {
        let new = NewWholeItem {
            item: NewItem {
                resource: NewResource {
                    title: Some(format!("Postcard {n}")),
                    is_public: true,
                    ..NewResource::default()
                },
                primary_media_id: None,
            },
            item_sets: sets.iter().map(|id| ItemSetId(*id)).collect(),
            ..NewWholeItem::default()
        };
        items.push(
            ItemRepo::new(&site.pool)
                .create_whole(&new)
                .await
                .unwrap()
                .id()
                .0,
        );
    }
    items
}

async fn start(site: &Site, id: u64) {
    let (status, started) = admin(site, "POST", &format!("/api/romeka/bulk/{id}/run"), None).await;
    assert_eq!(status, 200, "{started}");
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
        json!([{ "item": theirs, "reason": "denied" }])
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
