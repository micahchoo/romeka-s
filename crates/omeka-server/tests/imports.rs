//! The import routes that outlive the v1 importer: the starter table and a
//! job's log, who may read them, and the 409 for deleting the job of an
//! import that is running or undoing.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::path::Path;

use axum::Router;
use axum::http::StatusCode;
use common::{Answer, append_key, get, json_request, request, seeded_pool, send};
use omeka_api::import::starters::STARTERS;
use omeka_api::jsonld::BaseUrl;
use omeka_db::ids::{ImportId, JobId};
use omeka_db::repos::{ApiKeyRepo, ImportRepo, JobRepo, StartOutcome, UserRepo};
use omeka_db::rows::{ImportEnding, ImportStatus, NewApiKey, NewImport, NewJob, NewUser};
use serde_json::json;
use sqlx::SqlitePool;

struct Fixture {
    app: Router,
    pool: SqlitePool,
}

async fn fixture() -> Fixture {
    let (pool, _) = seeded_pool().await;
    Fixture {
        app: omeka_server::build_router(pool.clone(), BaseUrl::parse("http://localhost").unwrap()),
        pool,
    }
}

impl Fixture {
    async fn delete_job(&self, job: JobId) -> Answer {
        send(
            self.app.clone(),
            json_request("DELETE", &append_key(&format!("/api/jobs/{}", job.0)), "{}"),
        )
        .await
    }

    async fn job(&self, status: &str) -> JobId {
        JobRepo::new(&self.pool)
            .create(&NewJob {
                owner_id: None,
                pid: None,
                status: Some(status.to_string()),
                class: "Romeka\\Job\\Import".to_string(),
                args: None,
                log: None,
                started: None,
            })
            .await
            .unwrap()
            .id
    }

    /// A draft started as a run, with a live job linked.
    async fn running(&self) -> (ImportId, JobId) {
        let imports = ImportRepo::new(&self.pool);
        let id = imports
            .create_draft(&NewImport {
                owner_id: None,
                sources: json!([]),
                plan: json!({ "version": 2 }),
                copied_from: None,
            })
            .await
            .unwrap()
            .id;
        assert_eq!(
            imports.start_run(id, 0).await.unwrap(),
            StartOutcome::Started
        );
        let job = self.job("in_progress").await;
        assert!(imports.set_job(id, job).await.unwrap());
        (id, job)
    }

    async fn status(&self, id: ImportId) -> ImportStatus {
        ImportRepo::new(&self.pool)
            .get(id)
            .await
            .unwrap()
            .unwrap()
            .status
    }
}

#[tokio::test]
async fn the_starters_are_listed_and_need_an_admin() {
    let fixture = fixture().await;
    let anonymous = send(fixture.app.clone(), get("/api/romeka/starters", false)).await;
    assert_eq!(anonymous.status, StatusCode::UNAUTHORIZED);

    let listed = send(fixture.app.clone(), get("/api/romeka/starters", true)).await;
    assert_eq!(listed.status, StatusCode::OK, "{}", listed.body);
    let starters = listed.json();
    let ids: Vec<&str> = starters
        .as_array()
        .unwrap()
        .iter()
        .map(|starter| starter["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        ids,
        [
            "letter",
            "photograph",
            "postcard",
            "diary",
            "person",
            "other"
        ]
    );
    assert_eq!(
        starters[0]["fields"][2],
        json!({
            "term": "dcterms:creator", "required": false,
            "label": "Sender", "dataTypes": ["literal"]
        })
    );
    assert_eq!(starters[0]["nameKey"], "starter.letter");
    assert_eq!(starters[0]["classTerm"], "bibo:Letter");
    assert_eq!(
        starters,
        serde_json::to_value(STARTERS).unwrap(),
        "the wire is the table the run reads"
    );
}

/// The admin's tests serve this file as the starters route. When the table
/// changes, rewrite it from the route's answer.
#[test]
fn the_admin_fixture_is_the_starter_table() {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../web/admin/tests/fixtures/starters.json");
    let fixture: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    assert_eq!(fixture, serde_json::to_value(STARTERS).unwrap());
}

#[tokio::test]
async fn a_job_log_is_text_and_an_author_is_refused_both_routes() {
    let fixture = fixture().await;
    let job = fixture.job("completed").await;
    JobRepo::new(&fixture.pool)
        .append_log(job, "imported 2 items")
        .await
        .unwrap();
    let log = send(
        fixture.app.clone(),
        get(&format!("/api/romeka/jobs/{}/log", job.0), true),
    )
    .await;
    assert_eq!(log.status, StatusCode::OK);
    assert!(log.body.contains("imported 2 items"), "{}", log.body);
    let missing = send(fixture.app.clone(), get("/api/romeka/jobs/999/log", true)).await;
    assert_eq!(missing.status, StatusCode::NOT_FOUND);

    let author = UserRepo::new(&fixture.pool)
        .create(&NewUser {
            email: "author@example.com".to_string(),
            name: "Author".to_string(),
            password_hash: None,
            role: "editor".to_string(),
            is_active: true,
            created: None,
        })
        .await
        .unwrap();
    let identity = "editorkey000000000000000000000ab";
    ApiKeyRepo::new(&fixture.pool)
        .create(&NewApiKey {
            id: identity.to_string(),
            owner_id: author.id,
            label: "editor".to_string(),
            credential_hash: bcrypt::hash("editor-credential", 4).unwrap(),
            last_ip: None,
            last_accessed: None,
            created: None,
        })
        .await
        .unwrap();
    for uri in ["/api/romeka/starters", "/api/romeka/jobs/1/log"] {
        let answer = send(
            fixture.app.clone(),
            request(
                "GET",
                &format!("{uri}?key_identity={identity}&key_credential=editor-credential"),
            ),
        )
        .await;
        assert_eq!(answer.status, StatusCode::FORBIDDEN, "{uri}");
    }
}

const JOB_HELD: &str = r#"{"errors":{"error":"This job belongs to an archive import that is running or undoing. Delete it once the import has ended."}}"#;

#[tokio::test]
async fn deleting_the_job_of_a_running_import_is_409_and_changes_nothing() {
    let fixture = fixture().await;
    let (id, job) = fixture.running().await;

    let refused = fixture.delete_job(job).await;

    assert_eq!(refused.status, StatusCode::CONFLICT, "{}", refused.body);
    assert_eq!(refused.body, JOB_HELD);
    assert!(
        JobRepo::new(&fixture.pool)
            .get(job)
            .await
            .unwrap()
            .is_some()
    );
    assert_eq!(fixture.status(id).await, ImportStatus::Running);
}

#[tokio::test]
async fn deleting_the_job_of_an_undoing_import_is_409() {
    let fixture = fixture().await;
    let (id, _) = fixture.running().await;
    let imports = ImportRepo::new(&fixture.pool);
    assert!(imports.finish(id, ImportEnding::Done).await.unwrap());
    assert_eq!(imports.start_undo(id).await.unwrap(), StartOutcome::Started);
    let job = fixture.job("in_progress").await;
    imports.set_job(id, job).await.unwrap();

    let refused = fixture.delete_job(job).await;

    assert_eq!(refused.status, StatusCode::CONFLICT, "{}", refused.body);
    assert_eq!(refused.body, JOB_HELD);
}

#[tokio::test]
async fn the_job_of_an_ended_or_dead_import_can_be_deleted() {
    let fixture = fixture().await;
    let (ended, ended_job) = fixture.running().await;
    ImportRepo::new(&fixture.pool)
        .finish(ended, ImportEnding::Done)
        .await
        .unwrap();
    assert_eq!(fixture.delete_job(ended_job).await.status, StatusCode::OK);

    let (dead, dead_job) = fixture.running().await;
    JobRepo::new(&fixture.pool)
        .finish(dead_job, "error")
        .await
        .unwrap();
    let answer = fixture.delete_job(dead_job).await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    assert_eq!(fixture.status(dead).await, ImportStatus::RunFailed);
}
