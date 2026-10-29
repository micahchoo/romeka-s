//! Site page version history over HTTP: every save is a version, the version routes list and
//! answer them to a signed-in reader of the page, and a save that sends a stale `o:modified` is
//! refused with 409 naming who saved since. These are Romeka additions; upstream has no history.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::Router;
use axum::http::StatusCode;
use common::{json_request, request, seeded_pool, send};
use omeka_db::ids::{PageId, SiteId, UserId};
use omeka_db::repos::{ApiKeyRepo, SiteRepo, UserRepo};
use omeka_db::rows::{NewApiKey, NewSite, NewSitePage, NewUser};
use serde_json::{Value, json};
use sqlx::SqlitePool;

const CREDENTIAL: &str = "page-history-credential";

struct Actor {
    id: UserId,
    identity: String,
}

impl Actor {
    fn uri(&self, path: &str) -> String {
        format!(
            "{path}?key_identity={}&key_credential={CREDENTIAL}",
            self.identity
        )
    }
}

async fn actor(pool: &SqlitePool, name: &str, role: &str) -> Actor {
    let user = UserRepo::new(pool)
        .create(&NewUser {
            email: format!("{name}@example.com"),
            name: name.to_string(),
            password_hash: None,
            role: role.to_string(),
            is_active: true,
            created: None,
        })
        .await
        .unwrap();
    let mut identity: String = name.chars().filter(char::is_ascii_alphanumeric).collect();
    while identity.len() < 32 {
        identity.push('0');
    }
    ApiKeyRepo::new(pool)
        .create(&NewApiKey {
            id: identity.clone(),
            owner_id: user.id,
            label: "page history".to_string(),
            credential_hash: bcrypt::hash(CREDENTIAL, 4).unwrap(),
            last_ip: None,
            last_accessed: None,
            created: None,
        })
        .await
        .unwrap();
    Actor {
        id: user.id,
        identity,
    }
}

/// A public site with one page, "Intro", and three people: Ana and Ben edit the site, Vic only
/// views it, and Out has no permission.
struct Scenario {
    pool: SqlitePool,
    app: Router,
    site: SiteId,
    page: PageId,
    ana: Actor,
    ben: Actor,
    vic: Actor,
    out: Actor,
}

async fn scenario() -> Scenario {
    let (pool, seed) = seeded_pool().await;
    let repo = SiteRepo::new(&pool);
    let site = repo
        .create(&NewSite {
            owner_id: Some(seed.user),
            slug: "history".to_string(),
            theme: "default".to_string(),
            title: "History".to_string(),
            is_public: true,
            ..NewSite::default()
        })
        .await
        .unwrap()
        .id;
    let page = repo
        .add_page(&NewSitePage {
            site_id: site,
            slug: "intro".to_string(),
            title: "Intro".to_string(),
            is_public: true,
            ..NewSitePage::default()
        })
        .await
        .unwrap()
        .id;
    let ana = actor(&pool, "Ana", "researcher").await;
    let ben = actor(&pool, "Ben", "researcher").await;
    let vic = actor(&pool, "Vic", "researcher").await;
    let out = actor(&pool, "Out", "researcher").await;
    repo.set_permission(site, ana.id, "editor").await.unwrap();
    repo.set_permission(site, ben.id, "editor").await.unwrap();
    repo.set_permission(site, vic.id, "viewer").await.unwrap();
    Scenario {
        app: common::router(pool.clone()),
        pool,
        site,
        page,
        ana,
        ben,
        vic,
        out,
    }
}

impl Scenario {
    fn page_path(&self) -> String {
        format!("/api/site_pages/{}", self.page.0)
    }

    async fn put(&self, who: &Actor, body: Value) -> common::Answer {
        send(
            self.app.clone(),
            json_request("PUT", &who.uri(&self.page_path()), &body.to_string()),
        )
        .await
    }

    async fn get(&self, who: Option<&Actor>, path: &str) -> common::Answer {
        let uri = who.map_or_else(|| path.to_string(), |actor| actor.uri(path));
        send(self.app.clone(), request("GET", &uri)).await
    }

    /// A full page write with one html block per text.
    fn write(&self, title: &str, texts: &[&str]) -> Value {
        json!({
            "o:site": { "o:id": self.site.0 },
            "o:title": title,
            "o:slug": "intro",
            "o:block": texts
                .iter()
                .map(|text| json!({ "o:layout": "html", "o:data": { "html": text } }))
                .collect::<Vec<_>>(),
        })
    }
}

fn listed(answer: &common::Answer) -> Vec<(u64, Value, u64)> {
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    answer
        .json()
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| {
            (
                entry["o:version"].as_u64().unwrap(),
                entry["o:user"]["o:name"].clone(),
                entry["o:block_count"].as_u64().unwrap(),
            )
        })
        .collect()
}

#[tokio::test]
async fn each_save_is_a_version_and_a_version_answers_its_page_document() {
    let scenario = scenario().await;
    assert_eq!(
        scenario
            .put(&scenario.ana, scenario.write("First", &["a", "b"]))
            .await
            .status,
        StatusCode::OK
    );
    assert_eq!(
        scenario
            .put(&scenario.ben, scenario.write("Second", &["c"]))
            .await
            .status,
        StatusCode::OK
    );

    let versions_path = format!("{}/versions", scenario.page_path());
    let list = scenario.get(Some(&scenario.ana), &versions_path).await;
    assert_eq!(
        listed(&list),
        [
            (3, json!("Ben"), 1),
            (2, json!("Ana"), 2),
            (1, Value::Null, 0),
        ]
    );
    let stored = scenario
        .get(Some(&scenario.ana), &scenario.page_path())
        .await
        .json();
    assert_eq!(list.json()[0]["o:created"], stored["o:modified"]);

    let first = scenario
        .get(Some(&scenario.ana), &format!("{versions_path}/2"))
        .await;
    assert_eq!(first.status, StatusCode::OK);
    let document = first.json();
    assert_eq!(document["o:title"], "First");
    assert_eq!(document["o:id"], scenario.page.0);
    let texts: Vec<Value> = document["o:block"]
        .as_array()
        .unwrap()
        .iter()
        .map(|block| block["o:data"]["html"].clone())
        .collect();
    assert_eq!(texts, [json!("a"), json!("b")]);
    let original = scenario
        .get(Some(&scenario.ana), &format!("{versions_path}/1"))
        .await
        .json();
    assert_eq!(original["o:title"], "Intro");
}

#[tokio::test]
async fn a_save_with_a_stale_modified_is_refused_and_names_who_saved() {
    let scenario = scenario().await;
    let loaded = scenario
        .get(Some(&scenario.ben), &scenario.page_path())
        .await
        .json()["o:modified"]
        .clone();
    // A save in a later second than Ben's load, as a second editor's would be.
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
    let anas = scenario
        .put(&scenario.ana, scenario.write("Ana's", &["ana"]))
        .await;
    assert_eq!(anas.status, StatusCode::OK);

    let mut bens = scenario.write("Ben's", &["ben"]);
    bens["o:modified"] = loaded;
    let refused = scenario.put(&scenario.ben, bens.clone()).await;

    assert_eq!(refused.status, StatusCode::CONFLICT, "{}", refused.body);
    let errors = refused.json()["errors"].clone();
    assert_eq!(
        errors["error"],
        "Someone saved this page after you opened it."
    );
    assert_eq!(errors["o:modified"], anas.json()["o:modified"]);
    assert_eq!(
        errors["o:user"],
        json!({ "o:id": scenario.ana.id.0, "o:name": "Ana" })
    );
    let stored = scenario
        .get(Some(&scenario.ben), &scenario.page_path())
        .await
        .json();
    assert_eq!(stored["o:title"], "Ana's");

    bens["o:modified"] = errors["o:modified"].clone();
    let kept = scenario.put(&scenario.ben, bens).await;
    assert_eq!(kept.status, StatusCode::OK, "{}", kept.body);
    assert_eq!(kept.json()["o:title"], "Ben's");
}

#[tokio::test]
async fn a_save_without_modified_is_never_refused() {
    let scenario = scenario().await;
    let stale = json!({ "@value": "2001-01-01T00:00:00+00:00", "@type": "http://www.w3.org/2001/XMLSchema#dateTime" });
    let mut first = scenario.write("Checked", &["x"]);
    first["o:modified"] = stale;
    assert_eq!(
        scenario.put(&scenario.ana, first).await.status,
        StatusCode::CONFLICT
    );

    let unchecked = scenario
        .put(&scenario.ben, scenario.write("Unchecked", &["y"]))
        .await;
    assert_eq!(unchecked.status, StatusCode::OK);
    assert_eq!(unchecked.json()["o:title"], "Unchecked");
}

#[tokio::test]
async fn the_version_routes_need_a_signed_in_reader_of_the_page() {
    let scenario = scenario().await;
    scenario
        .put(&scenario.ana, scenario.write("Once", &["a"]))
        .await;
    let list = format!("{}/versions", scenario.page_path());
    let one = format!("{list}/1");

    assert_eq!(
        scenario.get(None, &list).await.status,
        StatusCode::FORBIDDEN
    );
    assert_eq!(scenario.get(None, &one).await.status, StatusCode::FORBIDDEN);
    assert_eq!(
        listed(&scenario.get(Some(&scenario.vic), &list).await).len(),
        2
    );
    assert_eq!(
        scenario.get(Some(&scenario.vic), &one).await.status,
        StatusCode::OK
    );
    assert_eq!(
        scenario.get(Some(&scenario.out), &list).await.status,
        StatusCode::OK
    );

    SiteRepo::new(&scenario.pool)
        .update_page(&omeka_db::rows::SitePageRow {
            is_public: false,
            ..SiteRepo::new(&scenario.pool)
                .get_page(scenario.page)
                .await
                .unwrap()
                .unwrap()
        })
        .await
        .unwrap();
    assert_eq!(
        scenario.get(Some(&scenario.out), &list).await.status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        scenario.get(Some(&scenario.out), &one).await.status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        scenario.get(Some(&scenario.vic), &list).await.status,
        StatusCode::OK
    );

    let missing = format!("{list}/99");
    assert_eq!(
        scenario.get(Some(&scenario.ana), &missing).await.status,
        StatusCode::NOT_FOUND
    );
    let word = format!("{list}/first");
    assert_eq!(
        scenario.get(Some(&scenario.ana), &word).await.status,
        StatusCode::NOT_FOUND
    );

    let deleted = send(
        scenario.app.clone(),
        request("DELETE", &scenario.ana.uri(&scenario.page_path())),
    )
    .await;
    assert_eq!(deleted.status, StatusCode::OK);
    assert_eq!(
        scenario.get(Some(&scenario.ana), &list).await.status,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn each_listed_version_says_what_it_changed_and_why_an_author_is_missing() {
    let scenario = scenario().await;
    let saves = [
        (&scenario.ana, scenario.write("First", &["a", "b"])),
        (&scenario.ben, scenario.write("First", &["b", "a"])),
        (&scenario.ana, scenario.write("First", &["b", "a, edited"])),
    ];
    for (who, body) in saves {
        assert_eq!(scenario.put(who, body).await.status, StatusCode::OK);
    }
    assert!(
        UserRepo::new(&scenario.pool)
            .delete(scenario.ana.id)
            .await
            .unwrap()
    );

    let list = scenario
        .get(
            Some(&scenario.ben),
            &format!("{}/versions", scenario.page_path()),
        )
        .await;
    assert_eq!(list.status, StatusCode::OK, "{}", list.body);
    let entries = list.json();
    let authors: Vec<(Value, Value, Value)> = entries
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| {
            (
                entry["o:origin"].clone(),
                entry["o:author"].clone(),
                entry["o:user"]["o:name"].clone(),
            )
        })
        .collect();
    assert_eq!(
        authors,
        [
            (json!("save"), json!("deleted_user"), Value::Null),
            (json!("save"), json!("user"), json!("Ben")),
            (json!("save"), json!("deleted_user"), Value::Null),
            (json!("before_history"), json!("not_recorded"), Value::Null),
        ]
    );
    let edited = json!({
        "o:title": null, "o:settings_changed": false,
        "o:blocks_added": 0, "o:blocks_removed": 0, "o:blocks_changed": 1, "o:blocks_moved": 0,
        "o:changed_blocks": [{ "o:layout": "html", "o:layout_data": null }],
    });
    let moved = json!({
        "o:title": null, "o:settings_changed": false,
        "o:blocks_added": 0, "o:blocks_removed": 0, "o:blocks_changed": 0, "o:blocks_moved": 1,
        "o:changed_blocks": [],
    });
    let first = json!({
        "o:title": { "o:before": "Intro", "o:after": "First" }, "o:settings_changed": false,
        "o:blocks_added": 2, "o:blocks_removed": 0, "o:blocks_changed": 0, "o:blocks_moved": 0,
        "o:changed_blocks": [],
    });
    let changes: Vec<Value> = entries
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["o:changes"].clone())
        .collect();
    assert_eq!(changes, [edited, moved, first, Value::Null]);
}

#[tokio::test]
async fn a_page_created_over_the_api_starts_its_history_with_a_created_version_by_its_creator() {
    let scenario = scenario().await;
    let created = send(
        scenario.app.clone(),
        json_request(
            "POST",
            &scenario.ana.uri("/api/site_pages"),
            &json!({
                "o:site": { "o:id": scenario.site.0 },
                "o:title": "Made",
                "o:slug": "made",
                "o:block": [{ "o:layout": "html", "o:data": { "html": "a" } }],
            })
            .to_string(),
        ),
    )
    .await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.body);
    let versions = format!("/api/site_pages/{}/versions", created.json()["o:id"]);
    let summary = |answer: &common::Answer| -> Vec<Value> {
        assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
        answer
            .json()
            .as_array()
            .unwrap()
            .iter()
            .map(|entry| {
                json!([
                    entry["o:version"],
                    entry["o:origin"],
                    entry["o:author"],
                    entry["o:user"]["o:name"],
                    entry["o:block_count"],
                    entry["o:changes"],
                ])
            })
            .collect()
    };

    assert_eq!(
        summary(&scenario.get(Some(&scenario.ben), &versions).await),
        [json!([1, "create", "user", "Ana", 1, null])]
    );

    assert!(
        UserRepo::new(&scenario.pool)
            .delete(scenario.ana.id)
            .await
            .unwrap()
    );
    assert_eq!(
        summary(&scenario.get(Some(&scenario.ben), &versions).await),
        [json!([1, "create", "deleted_user", null, 1, null])]
    );
}
