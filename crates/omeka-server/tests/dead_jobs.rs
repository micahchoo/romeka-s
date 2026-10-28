//! `settle_dead_jobs`, which `romeka serve` runs as it starts: the jobs a stopped server left
//! behind end as `error`, and a job that may still run keeps its status.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use chrono::{Duration, Utc};
use omeka_db::ids::JobId;
use omeka_db::repos::JobRepo;
use omeka_db::rows::NewJob;
use sqlx::SqlitePool;

async fn job(pool: &SqlitePool, status: &str, pid: Option<String>, minutes_ago: i64) -> JobId {
    JobRepo::new(pool)
        .create(&NewJob {
            owner_id: None,
            pid,
            status: Some(status.to_string()),
            class: "Omeka\\Job\\BatchUpdate".to_string(),
            args: None,
            log: None,
            started: Some((Utc::now() - Duration::minutes(minutes_ago)).naive_utc()),
        })
        .await
        .unwrap()
        .id
}

async fn status_and_log(pool: &SqlitePool, id: JobId) -> (String, Option<String>) {
    let row = JobRepo::new(pool).get(id).await.unwrap().unwrap();
    (row.status.unwrap(), row.log)
}

#[tokio::test]
async fn a_restart_ends_the_jobs_that_cannot_finish_and_keeps_the_rest() {
    let pool = omeka_db::open_memory().await.unwrap();
    omeka_db::migrate(&pool).await.unwrap();
    let alive = Some(std::process::id().to_string());
    // Past Linux's largest `pid_max` (2^22), so no process ever has it.
    let gone = Some("4194305".to_string());

    let crashed = job(&pool, "in_progress", gone.clone(), 30).await;
    let stopping = job(&pool, "stopping", gone, 30).await;
    let queued = job(&pool, "starting", None, 30).await;
    let running = job(&pool, "in_progress", alive, 30).await;
    let just_composed = job(&pool, "starting", None, 0).await;
    let finished = job(&pool, "completed", Some("4194305".to_string()), 30).await;

    omeka_server::settle_dead_jobs(&pool.clone().into()).await;

    let died = Some("the job's process 4194305 ended before the job did\n".to_string());
    assert_eq!(
        status_and_log(&pool, crashed).await,
        ("error".to_string(), died.clone())
    );
    assert_eq!(
        status_and_log(&pool, stopping).await,
        ("error".to_string(), died)
    );
    assert_eq!(
        status_and_log(&pool, queued).await,
        (
            "error".to_string(),
            Some("the server stopped before the job started\n".to_string())
        )
    );
    assert_eq!(status_and_log(&pool, running).await.0, "in_progress");
    assert_eq!(
        status_and_log(&pool, just_composed).await.0,
        "starting",
        "a job another command composed a moment ago is about to run"
    );
    assert_eq!(
        status_and_log(&pool, finished).await,
        ("completed".to_string(), None)
    );

    omeka_server::settle_dead_jobs(&pool.clone().into()).await;
    let (_, log) = status_and_log(&pool, crashed).await;
    assert_eq!(
        log.unwrap().lines().count(),
        1,
        "a second run finds nothing left to do"
    );
}
