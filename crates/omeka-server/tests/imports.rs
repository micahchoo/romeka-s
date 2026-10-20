//! The archive import routes over the router: who may call them, the archive
//! listing, the draft's life, the preview, and the run and undo state rules
//! with a dispatcher that records the jobs it is asked to start.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::path::Path;
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::http::StatusCode;
use common::{Answer, append_key, get, json_request, request, seeded_pool, send};
use omeka_api::jsonld::BaseUrl;
use omeka_db::ids::{ImportId, JobId};
use omeka_db::repos::{ApiKeyRepo, ImportRepo, JobRepo, PropertyRepo, UserRepo};
use omeka_db::rows::{NewApiKey, NewProperty, NewUser};
use omeka_server::{Config, DispatchError, JobDispatch, Services};
use serde_json::{Value, json};
use sqlx::SqlitePool;
use tempfile::TempDir;

/// Records each job it is asked to start, or refuses every one.
#[derive(Default)]
struct Recorder {
    started: Mutex<Vec<JobId>>,
    refuse: bool,
}

impl JobDispatch for Recorder {
    fn start(&self, job: JobId) -> Result<(), DispatchError> {
        if self.refuse {
            return Err(DispatchError("no runner in this test".to_string()));
        }
        self.started.lock().unwrap().push(job);
        Ok(())
    }
}

struct Fixture {
    _dir: TempDir,
    app: Router,
    pool: SqlitePool,
    jobs: Arc<Recorder>,
}

fn write(root: &Path, relative: &str) {
    let path = root.join(relative);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, relative.as_bytes()).unwrap();
}

async fn fixture_with(archive: bool, jobs: Recorder) -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("archive");
    write(&root, "Box/Letters/L_001_front.jpg");
    write(&root, "Box/Letters/L_001_back.jpg");
    write(&root, "Box/Photos/P_001.jpg");
    write(&root, "Other/x.jpg");
    std::fs::write(dir.path().join("secret.txt"), "outside").unwrap();
    let (pool, seed) = seeded_pool().await;
    PropertyRepo::new(&pool)
        .create(&NewProperty {
            owner_id: None,
            vocabulary_id: seed.vocabulary,
            local_name: "identifier".to_string(),
            label: "Identifier".to_string(),
            comment: None,
        })
        .await
        .unwrap();
    let config = Config {
        base_url: BaseUrl::parse("http://localhost").unwrap(),
        archive_root: archive.then_some(root),
        ..Config::default()
    };
    let jobs = Arc::new(jobs);
    let services =
        Services::new(omeka_server::file_store(dir.path().join("files"))).with_jobs(jobs.clone());
    Fixture {
        app: omeka_server::build_router_with_services(pool.clone(), &config, services),
        pool,
        jobs,
        _dir: dir,
    }
}

async fn fixture() -> Fixture {
    fixture_with(true, Recorder::default()).await
}

fn plan() -> Value {
    json!({
        "pattern": r"^(?<set>[^/]+)/(?<identifier>[A-Z]_\d{3})(?:_(?<part>[a-z]+))?\.jpg$",
        "example": null, "setLevel": "level1", "sets": {}, "typeFor": {}, "csv": null,
        "meta": [], "priority": "csv", "edits": {}, "skip": [], "withoutFiles": [], "csvRow": {},
        "problemRows": "leave"
    })
}

impl Fixture {
    async fn send(&self, method: &str, uri: &str, body: Value) -> Answer {
        send(
            self.app.clone(),
            json_request(method, &append_key(uri), &body.to_string()),
        )
        .await
    }

    async fn draft(&self) -> u64 {
        let created = self
            .send("POST", "/api/romeka/imports", json!({ "folder": "Box" }))
            .await;
        assert_eq!(created.status, StatusCode::OK, "{}", created.body);
        let id = created.json()["id"].as_u64().unwrap();
        let saved = self
            .send(
                "PUT",
                &format!("/api/romeka/imports/{id}"),
                json!({ "plan": plan() }),
            )
            .await;
        assert_eq!(saved.status, StatusCode::OK, "{}", saved.body);
        id
    }

    async fn import(&self, id: u64) -> Value {
        send(
            self.app.clone(),
            get(&format!("/api/romeka/imports/{id}"), true),
        )
        .await
        .json()
    }
}

#[tokio::test]
async fn with_no_archive_root_the_listing_says_off_and_nothing_can_be_created() {
    let fixture = fixture_with(false, Recorder::default()).await;
    let listing = send(fixture.app.clone(), get("/api/romeka/archive", true)).await;
    assert_eq!(listing.status, StatusCode::OK);
    assert_eq!(
        listing.json(),
        json!({ "enabled": false, "path": "", "entries": [] })
    );
    let created = fixture
        .send("POST", "/api/romeka/imports", json!({ "folder": "Box" }))
        .await;
    assert_eq!(created.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn the_starters_are_listed_even_with_the_import_off_and_need_an_admin() {
    let fixture = fixture_with(false, Recorder::default()).await;
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
        serde_json::to_value(omeka_api::import::catalog::STARTERS).unwrap(),
        "the wire is the table the run reads"
    );
}

/// The admin's tests serve this file as the starters route. When the table
/// changes, rewrite it from the route's answer.
#[test]
fn the_admin_fixture_is_the_starter_table() {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../web/admin/tests/fixtures/starters.json");
    let fixture: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    assert_eq!(
        fixture,
        serde_json::to_value(omeka_api::import::catalog::STARTERS).unwrap()
    );
}

#[tokio::test]
async fn the_archive_lists_folders_and_refuses_a_path_outside_the_root() {
    let fixture = fixture().await;
    let root = send(fixture.app.clone(), get("/api/romeka/archive?path=", true)).await;
    assert_eq!(
        root.json(),
        json!({ "enabled": true, "path": "", "entries": [
            { "name": "Box", "kind": "dir", "files": 3 },
            { "name": "Other", "kind": "dir", "files": 1 }
        ] })
    );
    let box_folder = send(
        fixture.app.clone(),
        get("/api/romeka/archive?path=Box", true),
    )
    .await;
    assert_eq!(box_folder.json()["entries"][0]["name"], "Letters");
    for escape in ["..", "Box/../..", "/etc"] {
        let answer = send(
            fixture.app.clone(),
            get(&format!("/api/romeka/archive?path={escape}"), true),
        )
        .await;
        assert_eq!(answer.status, StatusCode::BAD_REQUEST, "{escape}");
    }
    let missing = send(
        fixture.app.clone(),
        get("/api/romeka/archive?path=Nope", true),
    )
    .await;
    assert_eq!(missing.status, StatusCode::NOT_FOUND);
    let outside = fixture
        .send("POST", "/api/romeka/imports", json!({ "folder": "../" }))
        .await;
    assert_eq!(outside.status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn an_anonymous_caller_is_401_and_an_author_is_403() {
    let fixture = fixture().await;
    let anonymous = send(fixture.app.clone(), get("/api/romeka/imports", false)).await;
    assert_eq!(anonymous.status, StatusCode::UNAUTHORIZED);
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
    for uri in [
        "/api/romeka/imports",
        "/api/romeka/archive",
        "/api/romeka/starters",
        "/api/romeka/jobs/1/log",
    ] {
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

#[tokio::test]
async fn a_draft_keeps_its_plan_and_previews_the_sections_asked_for() {
    let fixture = fixture().await;
    let id = fixture.draft().await;
    let shown = fixture.import(id).await;
    assert_eq!(shown["status"], "draft");
    assert_eq!(shown["folder"], "Box");
    assert_eq!(shown["plan"]["pattern"], plan()["pattern"]);
    assert_eq!(shown["jobId"], Value::Null);

    let preview = fixture
        .send(
            "POST",
            &format!("/api/romeka/imports/{id}/preview"),
            json!({ "include": ["files", "sets", "check"], "files": { "offset": 0, "limit": 0 } }),
        )
        .await;
    assert_eq!(preview.status, StatusCode::OK, "{}", preview.body);
    let preview = preview.json();
    assert_eq!(preview["files"]["matched"], 3);
    assert_eq!(preview["files"]["page"], json!([]));
    assert_eq!(preview["sets"][0]["items"], 1);
    assert_eq!(preview["check"]["create"], 2);
    assert_eq!(preview["check"]["canRun"], true, "{preview}");
    assert!(preview.get("rows").is_none(), "only the sections asked for");

    let mut candidate = plan();
    candidate["pattern"] = json!(r"^Photos/(?<identifier>P_\d{3})\.jpg$");
    let unsaved = fixture
        .send(
            "POST",
            &format!("/api/romeka/imports/{id}/preview"),
            json!({ "include": ["check"], "plan": candidate }),
        )
        .await;
    assert_eq!(unsaved.json()["check"]["create"], 1, "the body's plan wins");
    let bad = fixture
        .send(
            "POST",
            &format!("/api/romeka/imports/{id}/preview"),
            json!({ "include": ["check"], "colour": 1 }),
        )
        .await;
    assert_eq!(bad.status, StatusCode::BAD_REQUEST);
    let bad_plan = fixture
        .send(
            "PUT",
            &format!("/api/romeka/imports/{id}"),
            json!({ "plan": { "pattern": 1 } }),
        )
        .await;
    assert_eq!(bad_plan.status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn a_run_starts_one_job_and_only_a_draft_can_change() {
    let fixture = fixture().await;
    let first = fixture.draft().await;
    let second = fixture.draft().await;
    let run = fixture
        .send(
            "POST",
            &format!("/api/romeka/imports/{first}/run"),
            json!({}),
        )
        .await;
    assert_eq!(run.status, StatusCode::OK, "{}", run.body);
    let job = JobId(run.json()["jobId"].as_u64().unwrap());
    assert_eq!(*fixture.jobs.started.lock().unwrap(), [job]);
    let row = JobRepo::new(&fixture.pool).get(job).await.unwrap().unwrap();
    assert_eq!(row.class, "Romeka\\Job\\ImportArchive");
    assert_eq!(row.args, Some(json!({ "importId": first })));
    let shown = fixture.import(first).await;
    assert_eq!(
        (shown["status"].clone(), shown["undoable"].clone()),
        (json!("running"), json!(true))
    );
    assert_eq!(shown["jobId"], job.0);

    let busy = fixture
        .send(
            "POST",
            &format!("/api/romeka/imports/{second}/run"),
            json!({}),
        )
        .await;
    assert_eq!(
        busy.status,
        StatusCode::CONFLICT,
        "another import is running"
    );
    let not_yet = fixture
        .send(
            "POST",
            &format!("/api/romeka/imports/{first}/undo"),
            json!({}),
        )
        .await;
    assert_eq!(
        not_yet.status,
        StatusCode::CONFLICT,
        "the run has not ended"
    );
    let edit = fixture
        .send(
            "PUT",
            &format!("/api/romeka/imports/{first}"),
            json!({ "plan": plan() }),
        )
        .await;
    assert_eq!(edit.status, StatusCode::CONFLICT);
    let delete = send(
        fixture.app.clone(),
        request(
            "DELETE",
            &append_key(&format!("/api/romeka/imports/{first}")),
        ),
    )
    .await;
    assert_eq!(delete.status, StatusCode::CONFLICT);
    let delete = send(
        fixture.app.clone(),
        request(
            "DELETE",
            &append_key(&format!("/api/romeka/imports/{second}")),
        ),
    )
    .await;
    assert_eq!(delete.status, StatusCode::NO_CONTENT);
    assert_eq!(
        send(
            fixture.app.clone(),
            get(&format!("/api/romeka/imports/{second}"), true)
        )
        .await
        .status,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn a_draft_whose_check_cannot_run_is_refused() {
    let fixture = fixture().await;
    let id = fixture.draft().await;
    let mut unknown = plan();
    unknown["meta"] =
        json!([{ "tag": "File name", "term": "dcterms:nonsense", "on": "media", "from": "first" }]);
    fixture
        .send(
            "PUT",
            &format!("/api/romeka/imports/{id}"),
            json!({ "plan": unknown }),
        )
        .await;
    let run = fixture
        .send("POST", &format!("/api/romeka/imports/{id}/run"), json!({}))
        .await;
    assert_eq!(run.status, StatusCode::CONFLICT);
    assert!(run.body.contains("termUnknown"), "{}", run.body);
    assert!(fixture.jobs.started.lock().unwrap().is_empty());
}

#[tokio::test]
async fn a_job_that_cannot_start_leaves_the_import_failed_and_undoable() {
    let fixture = fixture_with(
        true,
        Recorder {
            refuse: true,
            ..Recorder::default()
        },
    )
    .await;
    let id = fixture.draft().await;
    let run = fixture
        .send("POST", &format!("/api/romeka/imports/{id}/run"), json!({}))
        .await;
    assert_eq!(run.status, StatusCode::INTERNAL_SERVER_ERROR);
    let shown = fixture.import(id).await;
    assert_eq!(
        (shown["status"].clone(), shown["undoable"].clone()),
        (json!("failed"), json!(true))
    );
    let job = shown["jobId"].as_u64().unwrap();
    let log = send(
        fixture.app.clone(),
        get(&format!("/api/romeka/jobs/{job}/log"), true),
    )
    .await;
    assert_eq!(log.status, StatusCode::OK);
    assert!(
        common::header(&log, "content-type")
            .unwrap()
            .starts_with("text/plain"),
        "the log is text"
    );
    assert_eq!(log.body, "no runner in this test\n");
    let missing = send(fixture.app.clone(), get("/api/romeka/jobs/999/log", true)).await;
    assert_eq!(missing.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn the_import_shows_the_undo_report_the_undo_job_kept_in_its_summary() {
    let fixture = fixture().await;
    let id = fixture.draft().await;
    assert_eq!(fixture.import(id).await["undoReport"], Value::Null);

    let report = json!({ "items": 2, "media": 3, "itemSets": 1, "templates": 0, "fields": 0,
                         "kept": [{ "kind": "field", "id": 41 }] });
    ImportRepo::new(&fixture.pool)
        .set_summary(ImportId(id), &json!({ "items": 2, "undo": report }))
        .await
        .unwrap();
    let shown = fixture.import(id).await;
    assert_eq!(shown["undoReport"], report);
    assert_eq!(
        shown["summary"],
        json!({ "items": 2 }),
        "the report is not in the summary"
    );
}

#[tokio::test]
async fn left_csv_lists_the_skipped_and_failed_rows_with_their_reason() {
    let fixture = fixture().await;
    let id = fixture.draft().await;
    let mut skipping = plan();
    skipping["skip"] = json!(["P_001"]);
    fixture
        .send(
            "PUT",
            &format!("/api/romeka/imports/{id}"),
            json!({ "plan": skipping }),
        )
        .await;
    ImportRepo::new(&fixture.pool)
        .set_summary(
            ImportId(id),
            &json!({ "failures": [{ "identifier": "L_001", "reason": "disk full" }] }),
        )
        .await
        .unwrap();
    let uri = format!("/api/romeka/imports/{id}/left.csv");

    let csv = send(fixture.app.clone(), get(&uri, true)).await;
    assert_eq!(csv.status, StatusCode::OK, "{}", csv.body);
    assert!(
        common::header(&csv, "content-type")
            .unwrap()
            .starts_with("text/csv"),
        "the rows are CSV"
    );
    assert_eq!(
        csv.body,
        "dcterms:identifier,#reason\nL_001,failed: disk full\nP_001,skipped\n"
    );
    let anonymous = send(fixture.app.clone(), get(&uri, false)).await;
    assert_eq!(anonymous.status, StatusCode::UNAUTHORIZED);
}

impl Fixture {
    /// Start `id`'s run and mark its job `in_progress`, as a job process does.
    async fn running(&self, id: u64) -> JobId {
        let run = self
            .send("POST", &format!("/api/romeka/imports/{id}/run"), json!({}))
            .await;
        assert_eq!(run.status, StatusCode::OK, "{}", run.body);
        let job = JobId(run.json()["jobId"].as_u64().unwrap());
        assert!(JobRepo::new(&self.pool).start(job, "4242").await.unwrap());
        job
    }

    /// The job process of `id` beat last ten minutes ago, then died.
    async fn died(&self, id: u64) {
        sqlx::query(
            "UPDATE romeka_import
             SET summary = json_object('heartbeat', datetime('now', '-10 minutes'))
             WHERE id = ?",
        )
        .bind(i64::try_from(id).unwrap())
        .execute(&self.pool)
        .await
        .unwrap();
    }
}

#[tokio::test]
async fn an_import_whose_job_stopped_beating_shows_failed_and_lets_the_next_run_start() {
    let fixture = fixture().await;
    let first = fixture.draft().await;
    let second = fixture.draft().await;
    let job = fixture.running(first).await;
    fixture.died(first).await;

    let next = fixture
        .send(
            "POST",
            &format!("/api/romeka/imports/{second}/run"),
            json!({}),
        )
        .await;

    assert_eq!(next.status, StatusCode::OK, "{}", next.body);
    let shown = fixture.import(first).await;
    assert_eq!(shown["status"], "failed");
    assert!(shown["finished"].is_string(), "the death is recorded");
    assert!(shown["died"].is_string(), "the wire says it died: {shown}");
    let row = JobRepo::new(&fixture.pool).get(job).await.unwrap().unwrap();
    assert_eq!(row.status.as_deref(), Some("error"));
    assert!(row.ended.is_some(), "the job is ended");
}

#[tokio::test]
async fn an_import_whose_job_died_can_be_undone() {
    let fixture = fixture().await;
    let id = fixture.draft().await;
    fixture.running(id).await;
    fixture.died(id).await;

    let undo = fixture
        .send("POST", &format!("/api/romeka/imports/{id}/undo"), json!({}))
        .await;

    assert_eq!(undo.status, StatusCode::OK, "{}", undo.body);
    assert_eq!(fixture.import(id).await["status"], "undoing");
}

#[tokio::test]
async fn a_live_job_with_a_fresh_heartbeat_keeps_blocking() {
    let fixture = fixture().await;
    let first = fixture.draft().await;
    let second = fixture.draft().await;
    fixture.running(first).await;

    let busy = fixture
        .send(
            "POST",
            &format!("/api/romeka/imports/{second}/run"),
            json!({}),
        )
        .await;

    assert_eq!(busy.status, StatusCode::CONFLICT, "{}", busy.body);
    let shown = fixture.import(first).await;
    assert_eq!(shown["status"], "running");
    assert_eq!(shown["died"], Value::Null);
    assert!(
        shown["summary"].get("heartbeat").is_none(),
        "the heartbeat stays off the wire: {shown}"
    );
}

impl Fixture {
    async fn delete_job(&self, job: JobId) -> Answer {
        self.send("DELETE", &format!("/api/jobs/{}", job.0), json!({}))
            .await
    }
}

const JOB_HELD: &str = r#"{"errors":{"error":"This job belongs to an archive import that is running or undoing. Delete it once the import has ended."}}"#;

#[tokio::test]
async fn deleting_the_job_of_a_running_import_is_409_and_changes_nothing() {
    let fixture = fixture().await;
    let id = fixture.draft().await;
    let job = fixture.running(id).await;

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
    let shown = fixture.import(id).await;
    assert_eq!(shown["status"], "running");
    assert_eq!(shown["jobId"], job.0);
}

#[tokio::test]
async fn deleting_the_job_of_an_undoing_import_is_409() {
    let fixture = fixture().await;
    let id = fixture.draft().await;
    fixture.running(id).await;
    ImportRepo::new(&fixture.pool)
        .finish(ImportId(id), omeka_db::rows::ImportStatus::Done)
        .await
        .unwrap();
    let undo = fixture
        .send("POST", &format!("/api/romeka/imports/{id}/undo"), json!({}))
        .await;
    assert_eq!(undo.status, StatusCode::OK, "{}", undo.body);
    let job = JobId(undo.json()["jobId"].as_u64().unwrap());

    let refused = fixture.delete_job(job).await;

    assert_eq!(refused.status, StatusCode::CONFLICT, "{}", refused.body);
    assert_eq!(refused.body, JOB_HELD);
}

#[tokio::test]
async fn the_job_of_an_ended_or_dead_import_can_be_deleted() {
    let fixture = fixture().await;
    let ended = fixture.draft().await;
    let ended_job = fixture.running(ended).await;
    ImportRepo::new(&fixture.pool)
        .finish(ImportId(ended), omeka_db::rows::ImportStatus::Done)
        .await
        .unwrap();
    assert_eq!(fixture.delete_job(ended_job).await.status, StatusCode::OK);

    let dead = fixture.draft().await;
    let dead_job = fixture.running(dead).await;
    fixture.died(dead).await;
    let answer = fixture.delete_job(dead_job).await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    assert_eq!(fixture.import(dead).await["status"], "failed");
}
