//! `job`: the lifecycle writes the runner makes, the list filter, and the owner cascade.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use omeka_db::ids::UserId;
use omeka_db::repos::{JobFilter, JobRepo, Paging, UserRepo};
use omeka_db::rows::{NewJob, NewUser};
use serde_json::json;
use sqlx::SqlitePool;

async fn a_user(pool: &SqlitePool, email: &str) -> UserId {
    UserRepo::new(pool)
        .create(&NewUser {
            email: email.to_string(),
            name: email.to_string(),
            password_hash: None,
            role: "editor".to_string(),
            is_active: true,
            created: None,
        })
        .await
        .unwrap()
        .id
}

fn batch_delete(owner: Option<UserId>, status: &str) -> NewJob {
    NewJob {
        owner_id: owner,
        pid: None,
        status: Some(status.to_string()),
        class: "Omeka\\Job\\BatchDelete".to_string(),
        args: Some(json!({"resource": "items", "ids": [1, 2]})),
        log: None,
        started: None,
    }
}

#[tokio::test]
async fn a_job_runs_through_start_stop_log_and_finish() {
    let pool = common::memory().await;
    let owner = a_user(&pool, "ada@example.org").await;
    let jobs = JobRepo::new(&pool);

    let job = jobs
        .create(&batch_delete(Some(owner), "starting"))
        .await
        .unwrap();
    assert_eq!(job.status.as_deref(), Some("starting"));
    assert_eq!(jobs.get(job.id).await.unwrap(), Some(job.clone()));
    assert_eq!(
        jobs.status(job.id).await.unwrap().as_deref(),
        Some("starting")
    );

    assert!(jobs.start(job.id, "4242").await.unwrap());
    let running = jobs.get(job.id).await.unwrap().unwrap();
    assert_eq!(running.pid.as_deref(), Some("4242"));
    assert_eq!(running.status.as_deref(), Some("in_progress"));

    assert!(jobs.request_stop(job.id).await.unwrap());
    assert_eq!(
        jobs.status(job.id).await.unwrap().as_deref(),
        Some("stopping")
    );
    assert!(
        !jobs.request_stop(job.id).await.unwrap(),
        "a job that is not in_progress cannot be stopped again"
    );

    assert!(jobs.append_log(job.id, "deleting item 1").await.unwrap());
    assert!(jobs.append_log(job.id, "deleting item 2").await.unwrap());
    assert_eq!(
        jobs.get(job.id).await.unwrap().unwrap().log.as_deref(),
        Some("deleting item 1\ndeleting item 2\n"),
        "each line is appended after the stored log with a newline"
    );

    assert!(jobs.clear_pid(job.id).await.unwrap());
    assert!(jobs.finish(job.id, "completed").await.unwrap());
    let finished = jobs.get(job.id).await.unwrap().unwrap();
    assert_eq!(finished.pid, None);
    assert_eq!(finished.status.as_deref(), Some("completed"));
    assert!(
        finished.ended.unwrap() >= finished.started,
        "finish records an end at or after the start"
    );
}

#[tokio::test]
async fn jobs_list_in_id_order_and_filter_by_class_status_and_owner() {
    let pool = common::memory().await;
    let ada = a_user(&pool, "ada@example.org").await;
    let grace = a_user(&pool, "grace@example.org").await;
    let jobs = JobRepo::new(&pool);

    let first = jobs
        .create(&batch_delete(Some(ada), "starting"))
        .await
        .unwrap();
    let second = jobs
        .create(&batch_delete(Some(ada), "completed"))
        .await
        .unwrap();
    let mut other = batch_delete(Some(grace), "starting");
    other.class = "Omeka\\Job\\UpdateFulltext".to_string();
    let third = jobs.create(&other).await.unwrap();

    let all = jobs
        .list(&JobFilter::default(), &Paging::default())
        .await
        .unwrap();
    assert_eq!(
        all.iter().map(|row| row.id).collect::<Vec<_>>(),
        vec![first.id, second.id, third.id],
        "list is in id order"
    );

    let completed = jobs
        .list(
            &JobFilter {
                status: Some("completed".to_string()),
                ..Default::default()
            },
            &Paging::default(),
        )
        .await
        .unwrap();
    assert_eq!(
        completed.iter().map(|row| row.id).collect::<Vec<_>>(),
        vec![second.id]
    );

    let fulltext = jobs
        .list(
            &JobFilter {
                class: Some("Omeka\\Job\\UpdateFulltext".to_string()),
                ..Default::default()
            },
            &Paging::default(),
        )
        .await
        .unwrap();
    assert_eq!(
        fulltext.iter().map(|row| row.id).collect::<Vec<_>>(),
        vec![third.id]
    );

    let adas = jobs
        .list(
            &JobFilter {
                owner_id: Some(ada),
                ..Default::default()
            },
            &Paging::default(),
        )
        .await
        .unwrap();
    assert_eq!(
        adas.iter().map(|row| row.id).collect::<Vec<_>>(),
        vec![first.id, second.id]
    );
}

#[tokio::test]
async fn a_job_owner_is_cleared_when_the_user_goes() {
    let pool = common::memory().await;
    let owner = a_user(&pool, "ada@example.org").await;
    let jobs = JobRepo::new(&pool);
    let job = jobs
        .create(&batch_delete(Some(owner), "starting"))
        .await
        .unwrap();

    assert!(UserRepo::new(&pool).delete(owner).await.unwrap());

    let kept = jobs.get(job.id).await.unwrap().unwrap();
    assert_eq!(kept.owner_id, None, "the job stays and loses its owner");
}

#[tokio::test]
async fn a_job_needs_an_existing_owner() {
    let pool = common::memory().await;
    let error = JobRepo::new(&pool)
        .create(&batch_delete(Some(UserId(404)), "starting"))
        .await
        .unwrap_err();
    assert!(
        error.is_foreign_key_violation(),
        "job.owner_id references user: {error}"
    );
}

#[tokio::test]
async fn a_deleted_job_is_gone() {
    let pool = common::memory().await;
    let jobs = JobRepo::new(&pool);
    let job = jobs.create(&batch_delete(None, "completed")).await.unwrap();

    assert!(jobs.delete(job.id).await.unwrap());
    assert_eq!(jobs.get(job.id).await.unwrap(), None);
    assert!(!jobs.delete(job.id).await.unwrap());
    assert_eq!(jobs.status(job.id).await.unwrap(), None);
}
