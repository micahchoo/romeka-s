//! End-to-end tests for the `job` commands over a real database file and a real
//! child process. Each starts the built `romeka` binary, so the process boundary
//! and the row the child writes are exercised, not a stand-in.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

use sqlx::SqlitePool;
use tempfile::TempDir;

use omeka_cli::jobs::{BatchDelete, Dispatcher, Job};
use omeka_cli::store;
use omeka_db::ids::JobId;
use omeka_db::repos::{ItemRepo, Paging};
use omeka_db::rows::{NewItem, NewResource};
use omeka_db::{migrate, open_file};

/// A migrated SQLite file that lives as long as the value.
struct TempDb {
    _directory: TempDir,
    path: PathBuf,
    pool: SqlitePool,
}

async fn temp_db() -> TempDb {
    let directory = tempfile::tempdir().expect("temp dir");
    let path = directory.path().join("romeka.db");
    let pool = open_file(&path).await.expect("open temp database");
    migrate(&pool).await.expect("migrate temp database");
    TempDb {
        _directory: directory,
        path,
        pool,
    }
}

fn romeka() -> &'static str {
    env!("CARGO_BIN_EXE_romeka")
}

/// Poll the row until its status is terminal, or fail after 30 seconds.
async fn wait_for_terminal(pool: &SqlitePool, id: JobId) -> Option<String> {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let status = store::job_get(pool, id)
            .await
            .unwrap()
            .and_then(|row| row.status);
        if matches!(status.as_deref(), Some("completed" | "stopped" | "error")) {
            return status;
        }
        assert!(
            Instant::now() < deadline,
            "job {id} did not end; last status {status:?}"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[tokio::test]
async fn the_job_command_runs_a_stored_row() {
    let db = temp_db().await;
    let job_id = store::job_create(
        &db.pool,
        BatchDelete.class(),
        &serde_json::json!({"resource": "items"}),
        None,
    )
    .await
    .unwrap();

    let output = Command::new(romeka())
        .args(["job", "run", &job_id.0.to_string(), "--database"])
        .arg(&db.path)
        .output()
        .expect("start romeka job run");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let row = store::job_get(&db.pool, job_id)
        .await
        .unwrap()
        .expect("row");
    assert_eq!(row.status.as_deref(), Some("completed"));
    assert!(row.ended.is_some(), "the child writes the ending");
    assert_eq!(row.pid, None, "the child clears the pid");
}

#[tokio::test]
async fn a_failed_job_records_the_reason_and_show_prints_it() {
    let db = temp_db().await;
    let job_id = store::job_create(
        &db.pool,
        BatchDelete.class(),
        &serde_json::json!({"resource": "widgets"}),
        None,
    )
    .await
    .unwrap();

    let run = Command::new(romeka())
        .args(["job", "run", &job_id.0.to_string(), "--database"])
        .arg(&db.path)
        .output()
        .expect("start romeka job run");
    assert!(!run.status.success(), "a failed job exits nonzero");

    let row = store::job_get(&db.pool, job_id)
        .await
        .unwrap()
        .expect("row");
    assert_eq!(row.status.as_deref(), Some("error"));
    assert!(
        row.log.unwrap_or_default().contains("unsupported resource"),
        "the failure reason is stored"
    );

    let show = Command::new(romeka())
        .args(["job", "show", &job_id.0.to_string(), "--database"])
        .arg(&db.path)
        .output()
        .expect("start romeka job show");
    let stdout = String::from_utf8_lossy(&show.stdout);
    assert!(
        show.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&show.stderr)
    );
    assert!(stdout.contains("status: error"), "show printed {stdout:?}");
    assert!(
        stdout.contains("unsupported resource"),
        "show printed the log: {stdout:?}"
    );
}

#[tokio::test]
async fn a_dispatched_job_finishes_after_the_client_goes_away() {
    let db = temp_db().await;
    let items = ItemRepo::new(&db.pool);
    for index in 0..50 {
        items
            .create(&NewItem {
                resource: NewResource {
                    title: Some(format!("item {index}")),
                    ..NewResource::default()
                },
                primary_media_id: None,
            })
            .await
            .unwrap();
    }

    let id = Dispatcher::with_program(&db.pool, &db.path, romeka())
        .dispatch(&BatchDelete, serde_json::json!({"resource": "items"}))
        .await
        .expect("dispatch");

    // The client is done: it drops its pool and never runs the job itself.
    let path = db.path.clone();
    drop(db.pool);
    let reopened = open_file(&path).await.expect("reopen database");

    let status = wait_for_terminal(&reopened, id).await;
    assert_eq!(status.as_deref(), Some("completed"));
    let remaining = ItemRepo::new(&reopened)
        .list(&Default::default(), &Paging::default())
        .await
        .unwrap();
    assert!(remaining.is_empty(), "the child did the work");
}

#[tokio::test]
async fn the_stop_command_refuses_a_job_that_is_not_running() {
    let db = temp_db().await;
    let job_id = store::job_create(
        &db.pool,
        BatchDelete.class(),
        &serde_json::json!({"resource": "items"}),
        None,
    )
    .await
    .unwrap();

    let output = Command::new(romeka())
        .args(["job", "stop", &job_id.0.to_string(), "--database"])
        .arg(&db.path)
        .output()
        .expect("start romeka job stop");

    assert!(
        !output.status.success(),
        "a job in `starting` cannot be stopped"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("not in progress"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let row = store::job_get(&db.pool, job_id)
        .await
        .unwrap()
        .expect("row");
    assert_eq!(
        row.status.as_deref(),
        Some("starting"),
        "the row is untouched"
    );
}
