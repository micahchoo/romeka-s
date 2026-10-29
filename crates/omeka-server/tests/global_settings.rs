//! The global settings `SettingForm` shows, end to end: `PATCH /api/settings`
//! stores and validates each one, `GET /api/settings` reads it back, and
//! `GET /api/romeka/installation` answers the four the admin applies outside
//! the Settings page to every caller.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::{Arc, Mutex};

use axum::Router;
use axum::http::StatusCode;
use omeka_db::ids::JobId;
use omeka_db::repos::{JobRepo, SettingRepo};
use omeka_media::store::FileStore;
use omeka_server::{Config, DispatchError, JobDispatch, Services};
use serde_json::{Value, json};
use sqlx::SqlitePool;

use common::{Answer, Seed, append_key, caller, get, json_request, seeded_pool, send};

/// Records each job it is asked to start.
#[derive(Default)]
struct Recorder {
    started: Mutex<Vec<JobId>>,
}

impl JobDispatch for Recorder {
    fn start(&self, job: JobId) -> Result<(), DispatchError> {
        self.started.lock().unwrap().push(job);
        Ok(())
    }
}

struct Fixture {
    app: Router,
    pool: SqlitePool,
    seed: Seed,
    jobs: Arc<Recorder>,
    _files: tempfile::TempDir,
}

async fn fixture() -> Fixture {
    let (pool, seed) = seeded_pool().await;
    let files = tempfile::tempdir().unwrap();
    let jobs = Arc::new(Recorder::default());
    let app = omeka_server::build_router_with_services(
        pool.clone(),
        &Config::default(),
        Services::new(Arc::new(FileStore::new(files.path(), "/files"))).with_jobs(jobs.clone()),
    );
    Fixture {
        app,
        pool,
        seed,
        jobs,
        _files: files,
    }
}

async fn patch(app: &Router, body: &Value) -> Answer {
    send(
        app.clone(),
        json_request("PATCH", &append_key("/api/settings"), &body.to_string()),
    )
    .await
}

async fn installation(app: &Router) -> Value {
    let answer = send(app.clone(), get("/api/romeka/installation", false)).await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    answer.json()
}

#[tokio::test]
async fn every_setting_form_field_saves_and_reads_back() {
    let Fixture { app, .. } = fixture().await;
    let written = json!({
        "installation_title": "Gawan Museum",
        "administrator_email": "registrar@gawan.example",
        "time_zone": "Asia/Kolkata",
        "pagination_per_page": 50,
        "default_to_private_item_sets": true,
        "version_notifications": true,
    });
    let answer = patch(&app, &written).await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    let read = send(app.clone(), get("/api/settings", true)).await.json();
    for (key, value) in written.as_object().unwrap() {
        assert_eq!(&read[key], value, "{key} reads back as written");
    }

    let cleared = patch(
        &app,
        &json!({
            "pagination_per_page": null,
            "default_to_private_item_sets": false,
            "version_notifications": false,
        }),
    )
    .await;
    assert_eq!(cleared.status, StatusCode::OK, "{}", cleared.body);
    let read = cleared.json();
    assert_eq!(
        read["pagination_per_page"],
        json!(25),
        "null restores the default"
    );
    assert_eq!(read["default_to_private_item_sets"], json!(false));
    assert_eq!(read["version_notifications"], json!(false));
    assert_eq!(
        read["installation_title"],
        json!("Gawan Museum"),
        "an omitted key keeps its value"
    );
}

#[tokio::test]
async fn a_new_install_reads_the_upstream_defaults() {
    let Fixture { app, .. } = fixture().await;
    let read = send(app.clone(), get("/api/settings", true)).await.json();
    assert_eq!(read["time_zone"], json!("UTC"));
    assert_eq!(read["pagination_per_page"], json!(25));
    assert_eq!(read["default_to_private_item_sets"], json!(false));
    assert_eq!(read["version_notifications"], json!(false));
    assert!(read.get("installation_title").is_none(), "no row, no title");
    assert!(
        read.get("administrator_email").is_none(),
        "no row, no email"
    );
}

#[tokio::test]
async fn each_field_is_validated_as_setting_form_does_and_a_refusal_stores_nothing() {
    let Fixture { app, .. } = fixture().await;
    for refused in [
        json!({"installation_title": ""}),
        json!({"installation_title": "   "}),
        json!({"installation_title": null}),
        json!({"administrator_email": "registrar"}),
        json!({"administrator_email": "registrar@gawan"}),
        json!({"administrator_email": ""}),
        json!({"time_zone": "Mars/Olympus_Mons"}),
        json!({"time_zone": "Asia/Calcutta"}),
        json!({"time_zone": ""}),
        json!({"pagination_per_page": 0}),
        json!({"pagination_per_page": "25"}),
        json!({"pagination_per_page": 2.5}),
        json!({"default_to_private_item_sets": "1"}),
        json!({"installation_title": "Kept out", "time_zone": "Nowhere"}),
    ] {
        let answer = patch(&app, &refused).await;
        assert_eq!(
            answer.status,
            StatusCode::UNPROCESSABLE_ENTITY,
            "{refused}: {}",
            answer.body
        );
    }
    let read = send(app.clone(), get("/api/settings", true)).await.json();
    assert!(
        read.get("installation_title").is_none(),
        "a refused map writes no key: {read}"
    );
    let accepted = patch(
        &app,
        &json!({"installation_title": " Gawan ", "time_zone": "UTC"}),
    )
    .await;
    assert_eq!(accepted.status, StatusCode::OK, "{}", accepted.body);
}

#[tokio::test]
async fn an_adopted_database_reads_its_php_checkbox_and_digit_strings() {
    let Fixture { app, pool, .. } = fixture().await;
    let settings = SettingRepo::new(&pool);
    settings
        .set("default_to_private_item_sets", &json!("1"))
        .await
        .unwrap();
    settings
        .set("version_notifications", &json!("0"))
        .await
        .unwrap();
    settings
        .set("pagination_per_page", &json!("40"))
        .await
        .unwrap();
    let read = send(app.clone(), get("/api/settings", true)).await.json();
    assert_eq!(read["default_to_private_item_sets"], json!(true));
    assert_eq!(read["version_notifications"], json!(false));
    assert_eq!(read["pagination_per_page"], json!(40));
}

#[tokio::test]
async fn the_installation_facts_answer_any_caller_and_follow_a_save() {
    let Fixture { app, pool, .. } = fixture().await;
    assert_eq!(
        installation(&app).await,
        json!({
            "installation_title": "Romeka S",
            "pagination_per_page": 25,
            "time_zone": "UTC",
            "default_to_private_item_sets": false,
        }),
        "a signed-out caller reads the defaults"
    );
    let answer = patch(
        &app,
        &json!({
            "installation_title": "Gawan Museum",
            "time_zone": "Europe/Paris",
            "pagination_per_page": 10,
            "default_to_private_item_sets": true,
        }),
    )
    .await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    assert_eq!(
        installation(&app).await,
        json!({
            "installation_title": "Gawan Museum",
            "pagination_per_page": 10,
            "time_zone": "Europe/Paris",
            "default_to_private_item_sets": true,
        })
    );
    let researcher = caller(&pool, "researcher").await;
    let read = send(
        app.clone(),
        get(&researcher.uri("/api/romeka/installation"), false),
    )
    .await;
    assert_eq!(read.json()["installation_title"], json!("Gawan Museum"));
    let refused = send(app.clone(), get(&researcher.uri("/api/settings"), false)).await;
    assert_eq!(
        refused.status,
        StatusCode::FORBIDDEN,
        "the full map stays a global admin's"
    );
}

#[tokio::test]
async fn the_time_zone_options_are_the_list_the_write_accepts() {
    let Fixture { app, .. } = fixture().await;
    let answer = send(app.clone(), get("/api/romeka/time-zones", false)).await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    let zones = answer.json();
    let zones: Vec<&str> = zones
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .collect();
    assert_eq!(zones.last(), Some(&"UTC"), "upstream lists UTC last");
    assert!(zones.contains(&"Asia/Kolkata"));
    assert!(
        !zones.contains(&"Asia/Calcutta"),
        "a backward link is not offered"
    );
    for zone in ["Africa/Abidjan", "Pacific/Wallis", "UTC"] {
        let answer = patch(&app, &json!({ "time_zone": zone })).await;
        assert_eq!(answer.status, StatusCode::OK, "{zone}: {}", answer.body);
    }
}

#[tokio::test]
async fn index_fulltext_search_starts_the_index_job_and_is_not_stored() {
    let Fixture {
        app,
        pool,
        seed,
        jobs,
        ..
    } = fixture().await;
    let answer = patch(&app, &json!({ "index_fulltext_search": true })).await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    let started = jobs.started.lock().unwrap().clone();
    assert_eq!(started.len(), 1, "one job started");
    assert_eq!(
        answer.json()["index_fulltext_search_job"],
        json!(started[0].0)
    );
    let job = JobRepo::new(&pool).get(started[0]).await.unwrap().unwrap();
    assert_eq!(job.class, "Omeka\\Job\\IndexFulltextSearch");
    assert_eq!(job.owner_id, Some(seed.user));

    let quiet = patch(&app, &json!({ "index_fulltext_search": false })).await;
    assert_eq!(quiet.status, StatusCode::OK, "{}", quiet.body);
    assert_eq!(
        jobs.started.lock().unwrap().len(),
        1,
        "false starts nothing"
    );
    assert!(quiet.json().get("index_fulltext_search_job").is_none());
    let read = send(app.clone(), get("/api/settings", true)).await.json();
    assert!(
        read.get("index_fulltext_search").is_none(),
        "upstream never stores the switch"
    );
    assert!(
        SettingRepo::new(&pool)
            .get("index_fulltext_search")
            .await
            .unwrap()
            .is_none()
    );
}
