//! The import routes that read evidence or start a job: scan, preview, run
//! (its digest, blocking and confirm refusals), stop, undo and left.csv.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;
mod imports_fixture;

use axum::http::StatusCode;
use imports_fixture::{Fixture, JPEG, fixture};
use omeka_db::ids::ImportId;
use omeka_db::repos::{EntryRepo, ImportRepo, ItemRepo, PropertyRepo, ValueRepo, VocabularyRepo};
use omeka_db::rows::{ImportEnding, ImportStatus, NewEntry, NewItem, NewProperty, NewValue};
use omeka_db::with_db;
use omeka_media::store::Store;
use serde_json::{Value, json};

const PATTERN: &str = r"^(?:.*/)?(?<identifier>[A-Z]\d+)(?:_(?<part>[a-z0-9]+))?\.jpg$";

/// A draft over `A1.jpg`, `A1_back.jpg` and `B2.jpg`, scanned, with a
/// pattern saved.
async fn grouped(fixture: &Fixture) -> Value {
    fixture.write_objects(&[("A1.jpg", JPEG), ("A1_back.jpg", JPEG), ("B2.jpg", JPEG)]);
    let draft = fixture.draft().await;
    let saved = fixture
        .save(&draft, |plan| plan["grouping"]["pattern"] = json!(PATTERN))
        .await;
    assert_eq!(saved.status, StatusCode::OK, "{}", saved.body);
    let scanned = fixture
        .call(
            "POST",
            &format!("/api/romeka/imports/{}/scan", draft["id"]),
            None,
        )
        .await;
    assert_eq!(scanned.status, StatusCode::OK, "{}", scanned.body);
    saved.json()
}

async fn checked(fixture: &Fixture, id: &Value) -> Value {
    let answer = fixture.preview(id, &json!({ "include": ["check"] })).await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    answer.json()["check"].clone()
}

async fn run(fixture: &Fixture, id: &Value, body: Value) -> common::Answer {
    fixture
        .call(
            "POST",
            &format!("/api/romeka/imports/{id}/run"),
            Some(&body),
        )
        .await
}

#[tokio::test]
async fn a_scan_in_the_request_answers_each_source() {
    let fixture = fixture().await;
    fixture.write(&[
        ("A1.jpg", JPEG),
        ("register.csv", b"Id\nA1\n"),
        (".hidden", b"x"),
    ]);
    let draft = fixture.draft().await;
    let scanned = fixture
        .call(
            "POST",
            &format!("/api/romeka/imports/{}/scan", draft["id"]),
            None,
        )
        .await;
    assert_eq!(scanned.status, StatusCode::OK, "{}", scanned.body);
    let source = &scanned.json()["sources"][0];
    assert_eq!(source["source"], "s1");
    assert_eq!(source["files"], 1);
    assert_eq!(source["sheetFiles"], json!(["register.csv"]));
    assert!(source["scanned"].is_string());
    assert!(fixture.jobs.started.lock().unwrap().is_empty());
}

#[tokio::test]
async fn a_scan_past_twenty_thousand_entries_is_a_job() {
    let fixture = fixture().await;
    let draft = fixture.draft().await;
    let id = ImportId(draft["id"].as_u64().unwrap());
    let rows: Vec<NewEntry> = (0..20_001)
        .map(|n| NewEntry {
            source: "s1".into(),
            path: format!("f{n}.jpg"),
            size: 1,
            modified: Default::default(),
            media_type: Some("image/jpeg".into()),
            flags: json!({}),
        })
        .collect();
    with_db!(&fixture.pool, |db| EntryRepo::new(db)
        .replace(id, &[], &rows)
        .await)
    .unwrap();

    let scanned = fixture
        .call("POST", &format!("/api/romeka/imports/{}/scan", id.0), None)
        .await;
    assert_eq!(scanned.status, StatusCode::ACCEPTED, "{}", scanned.body);
    let job = scanned.json()["jobId"].as_u64().unwrap();
    assert_eq!(fixture.jobs.started.lock().unwrap().len(), 1);
    let row = fixture.job(job).await;
    assert_eq!(row.class, "Romeka\\Job\\ImportScan");
    assert_eq!(row.args, Some(json!({ "importId": id.0 })));
    assert_eq!(fixture.row(&draft["id"]).await.status, ImportStatus::Draft);
}

#[tokio::test]
async fn a_preview_answers_the_sections_asked_for() {
    let fixture = fixture().await;
    let draft = grouped(&fixture).await;
    let answer = fixture
        .preview(
            &draft["id"],
            &json!({
                "include": ["records", "entries", "flags", "anomalies", "orphans", "check", "sheets", "tags"],
                "records": { "kind": "item", "perPage": 1 },
                "entries": { "page": 1, "perPage": 10 }
            }),
        )
        .await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    let preview = answer.json();
    assert_eq!(preview["records"]["total"], 2);
    assert_eq!(preview["records"]["items"].as_array().unwrap().len(), 1);
    assert_eq!(preview["entries"]["total"], 3);
    assert!(preview["check"]["digest"].as_str().unwrap().len() == 64);
    assert!(preview["check"]["canRun"].as_bool().unwrap());
    assert!(
        preview["tags"]
            .as_array()
            .unwrap()
            .iter()
            .any(|tag| tag["tag"] == "File name" && tag["samples"][0]["value"] == "A1.jpg")
    );
    assert_eq!(preview["sources"][0]["files"], 3);
    let only = fixture
        .preview(&draft["id"], &json!({ "include": ["flags"] }))
        .await
        .json();
    assert!(only.get("records").is_none());
}

#[tokio::test]
async fn a_preview_of_an_unsaved_plan_leaves_the_stored_one() {
    let fixture = fixture().await;
    let draft = grouped(&fixture).await;
    let mut plan = draft["plan"].clone();
    plan["grouping"]["pattern"] = json!(r"^(?<identifier>A\d+)\.jpg$");
    let answer = fixture
        .preview(
            &draft["id"],
            &json!({ "include": ["records"], "plan": plan }),
        )
        .await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    assert_eq!(answer.json()["records"]["total"], 1);
    assert_eq!(fixture.row(&draft["id"]).await.revision, 1);
    let bad = fixture
        .preview(
            &draft["id"],
            &json!({ "include": [], "plan": { "version": 3 } }),
        )
        .await;
    assert_eq!(bad.status, StatusCode::BAD_REQUEST);
    let unknown = fixture
        .preview(&draft["id"], &json!({ "include": ["everything"] }))
        .await;
    assert_eq!(unknown.status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn a_run_starts_the_job_once_the_digest_is_the_checked_one() {
    let fixture = fixture().await;
    let draft = grouped(&fixture).await;
    let check = checked(&fixture, &draft["id"]).await;

    let started = run(&fixture, &draft["id"], json!({ "digest": check["digest"] })).await;
    assert_eq!(started.status, StatusCode::OK, "{}", started.body);
    let job = started.json()["jobId"].as_u64().unwrap();
    let row = fixture.job(job).await;
    assert_eq!(row.class, "Romeka\\Job\\ImportRun");
    assert_eq!(row.status.as_deref(), Some("starting"));
    assert_eq!(row.args, Some(json!({ "importId": draft["id"] })));
    let import = fixture.row(&draft["id"]).await;
    assert_eq!(import.status, ImportStatus::Running);
    assert_eq!(
        import.ran_plan.as_ref(),
        Some(&import.plan),
        "the plan is frozen"
    );
    assert_eq!(import.job_id.map(|job| job.0), Some(job));
    assert_eq!(
        *fixture.jobs.started.lock().unwrap(),
        [import.job_id.unwrap()]
    );

    let again = run(&fixture, &draft["id"], json!({ "digest": check["digest"] })).await;
    assert_eq!(again.status, StatusCode::CONFLICT);
    assert_eq!(again.json()["code"], "cannotRun");
}

#[tokio::test]
async fn a_run_whose_evidence_changed_since_the_check_answers_changed() {
    let fixture = fixture().await;
    let draft = grouped(&fixture).await;
    let check = checked(&fixture, &draft["id"]).await;
    fixture.write_objects(&[("C3.jpg", JPEG)]);

    let refused = run(&fixture, &draft["id"], json!({ "digest": check["digest"] })).await;

    assert_eq!(refused.status, StatusCode::CONFLICT, "{}", refused.body);
    let body = refused.json();
    assert_eq!(body["code"], "changed");
    assert_ne!(body["digest"], check["digest"]);
    assert_eq!(fixture.row(&draft["id"]).await.status, ImportStatus::Draft);
    let rechecked = checked(&fixture, &draft["id"]).await;
    assert_eq!(rechecked["digest"], body["digest"], "the run rescanned");
}

#[tokio::test]
async fn a_run_with_a_blocking_flag_answers_the_reasons() {
    let fixture = fixture().await;
    let draft = grouped(&fixture).await;
    let mut draft = fixture.row(&draft["id"]).await;
    let mut plan = draft.plan.clone();
    plan["slots"]["item"]["dcterms:nothing"] =
        json!({ "sources": [{ "from": "constant", "value": "x" }], "combine": "first" });
    let saved = fixture
        .call(
            "PUT",
            &format!("/api/romeka/imports/{}", draft.id.0),
            Some(&json!({ "plan": plan, "revision": draft.revision })),
        )
        .await;
    assert_eq!(saved.status, StatusCode::OK, "{}", saved.body);
    draft.plan = plan;
    let id = json!(draft.id.0);
    let check = checked(&fixture, &id).await;
    assert!(!check["canRun"].as_bool().unwrap());

    let refused = run(&fixture, &id, json!({ "digest": check["digest"] })).await;

    assert_eq!(refused.status, StatusCode::CONFLICT);
    let body = refused.json();
    assert_eq!(body["code"], "blocked");
    assert_eq!(body["blocking"][0]["kind"], "unknown-term");
    assert_eq!(body["blocking"][0]["detail"], "dcterms:nothing");
}

#[tokio::test]
async fn a_run_with_nothing_to_write_is_refused() {
    let fixture = fixture().await;
    let draft = fixture.draft().await;
    let check = checked(&fixture, &draft["id"]).await;
    let refused = run(&fixture, &draft["id"], json!({ "digest": check["digest"] })).await;
    assert_eq!(refused.status, StatusCode::CONFLICT);
    assert_eq!(refused.json()["code"], "nothing");
}

#[tokio::test]
async fn a_second_import_cannot_run_while_one_runs() {
    let fixture = fixture().await;
    let first = grouped(&fixture).await;
    let check = checked(&fixture, &first["id"]).await;
    assert_eq!(
        run(&fixture, &first["id"], json!({ "digest": check["digest"] }))
            .await
            .status,
        StatusCode::OK
    );
    let second = fixture.draft().await;
    let second = fixture
        .save(&second, |plan| plan["grouping"]["pattern"] = json!(PATTERN))
        .await
        .json();
    fixture
        .call(
            "POST",
            &format!("/api/romeka/imports/{}/scan", second["id"]),
            None,
        )
        .await;
    let check = checked(&fixture, &second["id"]).await;
    let refused = run(
        &fixture,
        &second["id"],
        json!({ "digest": check["digest"] }),
    )
    .await;
    assert_eq!(refused.status, StatusCode::CONFLICT, "{}", refused.body);
    assert_eq!(refused.json()["code"], "busy");
    assert_eq!(refused.json()["importId"], first["id"]);
}

#[tokio::test]
async fn a_run_the_runner_refuses_ends_its_job_in_error() {
    let fixture = fixture().await;
    let draft = grouped(&fixture).await;
    let check = checked(&fixture, &draft["id"]).await;
    *fixture.jobs.refuse.lock().unwrap() = true;
    let refused = run(&fixture, &draft["id"], json!({ "digest": check["digest"] })).await;
    assert_eq!(refused.status, StatusCode::INTERNAL_SERVER_ERROR);
    let job = fixture.row(&draft["id"]).await.job_id.unwrap();
    assert_eq!(fixture.job(job.0).await.status.as_deref(), Some("error"));
    let shown = fixture
        .call("GET", &format!("/api/romeka/imports/{}", draft["id"]), None)
        .await
        .json();
    assert_eq!(
        shown["status"], "run_failed",
        "the next read records it failed"
    );
}

/// An item with `dcterms:identifier` `A1`, for a delete to match.
async fn existing_a1(fixture: &Fixture) {
    let dcterms = with_db!(&fixture.pool, |db| VocabularyRepo::new(db)
        .get_by_prefix("dcterms")
        .await)
    .unwrap()
    .unwrap();
    let identifier = with_db!(&fixture.pool, |db| PropertyRepo::new(db)
        .create(&NewProperty {
            owner_id: None,
            vocabulary_id: dcterms.id,
            local_name: "identifier".into(),
            label: "Identifier".into(),
            comment: None,
        })
        .await)
    .unwrap();
    let item = with_db!(&fixture.pool, |db| ItemRepo::new(db)
        .create(&NewItem::default())
        .await)
    .unwrap();
    with_db!(&fixture.pool, |db| ValueRepo::new(db)
        .create(&NewValue {
            resource_id: item.resource.id,
            property_id: identifier.id,
            value_resource_id: None,
            value_annotation_id: None,
            r#type: "literal".into(),
            lang: None,
            value: Some("A1".into()),
            uri: None,
            is_public: true,
        })
        .await)
    .unwrap();
}

#[tokio::test]
async fn a_delete_run_needs_the_count_it_deletes() {
    let fixture = fixture().await;
    existing_a1(&fixture).await;
    let draft = grouped(&fixture).await;
    let saved = fixture
        .save(&draft, |plan| {
            plan["action"]["action"] = json!("delete");
            plan["action"]["match"]["item"] =
                json!({ "key": "dcterms:identifier", "scope": "all" });
        })
        .await;
    assert_eq!(saved.status, StatusCode::OK, "{}", saved.body);
    let check = checked(&fixture, &draft["id"]).await;
    assert_eq!(check["deletes"], 1, "{check}");

    for confirm in [None, Some(2)] {
        let mut body = json!({ "digest": check["digest"] });
        if let Some(confirm) = confirm {
            body["confirm"] = json!(confirm);
        }
        let refused = run(&fixture, &draft["id"], body).await;
        assert_eq!(refused.status, StatusCode::CONFLICT, "{}", refused.body);
        assert_eq!(refused.json()["code"], "confirm");
        assert_eq!(refused.json()["deletes"], 1);
    }
    let confirmed = run(
        &fixture,
        &draft["id"],
        json!({ "digest": check["digest"], "confirm": 1 }),
    )
    .await;
    assert_eq!(confirmed.status, StatusCode::OK, "{}", confirmed.body);
}

#[tokio::test]
async fn stop_asks_a_running_import_and_refuses_any_other() {
    let fixture = fixture().await;
    let draft = grouped(&fixture).await;
    let uri = format!("/api/romeka/imports/{}/stop", draft["id"]);
    let idle = fixture.call("POST", &uri, None).await;
    assert_eq!(idle.status, StatusCode::CONFLICT);
    assert_eq!(idle.json()["code"], "notRunning");

    let check = checked(&fixture, &draft["id"]).await;
    run(&fixture, &draft["id"], json!({ "digest": check["digest"] })).await;
    let asked = fixture.call("POST", &uri, None).await;
    assert_eq!(asked.status, StatusCode::ACCEPTED, "{}", asked.body);
    let row = fixture.row(&draft["id"]).await;
    assert!(row.stop_requested);
    assert!(
        row.summary.unwrap().get("stop").is_none(),
        "the request is not a summary member"
    );
    let shown = fixture
        .call("GET", &format!("/api/romeka/imports/{}", draft["id"]), None)
        .await
        .json();
    assert_eq!(shown["stopRequested"], true);
    assert_eq!(shown["status"], "running");
}

#[tokio::test]
async fn a_done_import_runs_again_only_with_failures_listed() {
    let fixture = fixture().await;
    let draft = grouped(&fixture).await;
    let id = ImportId(draft["id"].as_u64().unwrap());
    let check = checked(&fixture, &draft["id"]).await;
    run(&fixture, &draft["id"], json!({ "digest": check["digest"] })).await;
    with_db!(&fixture.pool, |db| ImportRepo::new(db)
        .set_summary(id, &json!({ "failures": [] }))
        .await)
    .unwrap();
    with_db!(&fixture.pool, |db| ImportRepo::new(db)
        .finish(id, ImportEnding::Done)
        .await)
    .unwrap();

    let refused = run(&fixture, &draft["id"], json!({ "digest": check["digest"] })).await;
    assert_eq!(refused.status, StatusCode::CONFLICT, "{}", refused.body);
    assert_eq!(refused.json()["code"], "cannotRun");
    assert_eq!(refused.json()["status"], "done");

    with_db!(&fixture.pool, |db| ImportRepo::new(db)
        .set_summary(
            id,
            &json!({ "failures": [{ "record": "A1", "key": "x", "detail": "d" }] })
        )
        .await)
    .unwrap();
    let again = checked(&fixture, &draft["id"]).await;
    let retried = run(&fixture, &draft["id"], json!({ "digest": again["digest"] })).await;
    assert_eq!(retried.status, StatusCode::OK, "{}", retried.body);
    assert_eq!(
        fixture.row(&draft["id"]).await.status,
        ImportStatus::Running
    );
}

#[tokio::test]
async fn undo_starts_a_job_for_an_ended_import_only() {
    let fixture = fixture().await;
    let draft = fixture.draft().await;
    let uri = format!("/api/romeka/imports/{}/undo", draft["id"]);
    let refused = fixture.call("POST", &uri, Some(&json!({}))).await;
    assert_eq!(refused.status, StatusCode::CONFLICT);
    assert_eq!(refused.json()["code"], "cannotUndo");

    let id = ImportId(draft["id"].as_u64().unwrap());
    with_db!(&fixture.pool, |db| ImportRepo::new(db)
        .start_run(id, 0)
        .await)
    .unwrap();
    with_db!(&fixture.pool, |db| ImportRepo::new(db)
        .finish(id, ImportEnding::Done)
        .await)
    .unwrap();
    let bad = fixture
        .call("POST", &uri, Some(&json!({ "force": "some" })))
        .await;
    assert_eq!(bad.status, StatusCode::BAD_REQUEST);

    let started = fixture
        .call(
            "POST",
            &uri,
            Some(&json!({ "force": [{ "kind": "item", "id": 7 }] })),
        )
        .await;
    assert_eq!(started.status, StatusCode::OK, "{}", started.body);
    let job = fixture.job(started.json()["jobId"].as_u64().unwrap()).await;
    assert_eq!(job.class, "Romeka\\Job\\ImportUndo");
    assert_eq!(
        job.args,
        Some(json!({ "importId": id.0, "force": [{ "kind": "item", "id": 7 }] }))
    );
    assert_eq!(
        fixture.row(&draft["id"]).await.status,
        ImportStatus::Undoing
    );
}

#[tokio::test]
async fn left_csv_is_the_stored_file() {
    let fixture = fixture().await;
    let draft = fixture.draft().await;
    let uri = format!("/api/romeka/imports/{}/left.csv", draft["id"]);
    let missing = fixture.call("GET", &uri, None).await;
    assert_eq!(missing.status, StatusCode::NOT_FOUND);
    let store = omeka_media::store::FileStore::new(fixture.files.path(), "/files");
    store
        .put(
            b"#reason,path\nnot-media,_scratch/Thumbs.db\n",
            &format!("imports/{}/left.csv", draft["id"]),
            Some("text/csv"),
        )
        .unwrap();
    let found = fixture.call("GET", &uri, None).await;
    assert_eq!(found.status, StatusCode::OK);
    assert!(found.body.contains("Thumbs.db"));
    assert_eq!(
        common::header(&found, "content-type"),
        Some("text/csv; charset=utf-8")
    );
}

#[tokio::test]
async fn a_scan_while_the_import_runs_is_refused() {
    let fixture = fixture().await;
    let draft = grouped(&fixture).await;
    let check = checked(&fixture, &draft["id"]).await;
    run(&fixture, &draft["id"], json!({ "digest": check["digest"] })).await;
    let refused = fixture
        .call(
            "POST",
            &format!("/api/romeka/imports/{}/scan", draft["id"]),
            None,
        )
        .await;
    assert_eq!(refused.status, StatusCode::CONFLICT);
    assert_eq!(refused.json()["code"], "busy");
}
