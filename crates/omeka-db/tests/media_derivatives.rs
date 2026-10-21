//! `romeka_media_derivative`: one row per media and kind, claimed once, settled
//! once, and removed with its media.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use omeka_db::repos::{ItemRepo, JobRepo, MediaDerivativeRepo, MediaRepo};
use omeka_db::rows::{DerivativeKind, DerivativeState, NewDerivative, NewItem, NewJob, NewMedia};
use sqlx::SqlitePool;

async fn media(pool: &SqlitePool, storage_id: &str) -> omeka_db::ids::MediaId {
    let item = ItemRepo::new(pool)
        .create(&NewItem::default())
        .await
        .expect("item");
    MediaRepo::new(pool)
        .create(&NewMedia {
            item_id: item.id(),
            ingester: "upload".to_string(),
            renderer: "file".to_string(),
            storage_id: Some(storage_id.to_string()),
            extension: Some("mp4".to_string()),
            media_type: Some("video/mp4".to_string()),
            source: Some("reel.mp4".to_string()),
            has_original: true,
            ..NewMedia::default()
        })
        .await
        .expect("media")
        .id()
}

fn poster(id: omeka_db::ids::MediaId) -> NewDerivative {
    NewDerivative {
        media_id: id,
        kind: DerivativeKind::Poster,
    }
}

#[tokio::test]
async fn one_media_and_kind_has_one_row() {
    let pool = common::memory().await;
    let id = media(&pool, "queue-1").await;
    let repo = MediaDerivativeRepo::new(&pool);

    assert!(repo.enqueue(&poster(id)).await.unwrap());
    assert!(
        !repo.enqueue(&poster(id)).await.unwrap(),
        "a second queue of the same pair keeps the first row"
    );
    assert_eq!(repo.for_media(id).await.unwrap().len(), 1);
}

#[tokio::test]
async fn a_claim_takes_the_row_once_and_counts_the_attempt() {
    let pool = common::memory().await;
    let first = media(&pool, "queue-2").await;
    let second = media(&pool, "queue-3").await;
    let repo = MediaDerivativeRepo::new(&pool);
    repo.enqueue(&poster(second)).await.unwrap();
    repo.enqueue(&poster(first)).await.unwrap();
    assert_eq!(repo.pending().await.unwrap(), 2);

    let claimed = repo.claim_next().await.unwrap().expect("a row");
    assert_eq!(claimed.state, DerivativeState::Running);
    assert_eq!(claimed.attempts, 1);
    assert!(claimed.started.is_some());

    let next = repo.claim_next().await.unwrap().expect("the second row");
    assert_ne!(claimed.media_id, next.media_id, "one claim per row");
    assert_eq!(repo.pending().await.unwrap(), 0);
    assert!(repo.claim_next().await.unwrap().is_none());
}

#[tokio::test]
async fn an_access_copy_waits_for_the_worker_that_makes_it() {
    let pool = common::memory().await;
    let id = media(&pool, "queue-access").await;
    let repo = MediaDerivativeRepo::new(&pool);
    assert!(
        repo.plan(
            &NewDerivative {
                media_id: id,
                kind: DerivativeKind::AccessCopy,
            },
            DerivativeState::Pending,
            None,
            Some("ffmpeg"),
        )
        .await
        .unwrap()
    );
    assert_eq!(
        repo.pending().await.unwrap(),
        0,
        "the preview worker counts only its own work"
    );
    assert!(
        repo.claim_next().await.unwrap().is_none(),
        "an access copy is not a still the preview worker can make"
    );
    let row = repo
        .get(id, DerivativeKind::AccessCopy)
        .await
        .unwrap()
        .expect("the row");
    assert_eq!(row.state, DerivativeState::Pending);
    assert_eq!(row.tool.as_deref(), Some("ffmpeg"));
    assert!(row.finished.is_none());

    // A preview row queued behind it is still claimed first.
    repo.enqueue(&poster(id)).await.unwrap();
    assert_eq!(repo.pending().await.unwrap(), 1);
    let claimed = repo.claim_next().await.unwrap().expect("the poster");
    assert_eq!(claimed.kind, DerivativeKind::Poster);
}

#[tokio::test]
async fn a_planned_decision_is_settled_with_its_words() {
    let pool = common::memory().await;
    let id = media(&pool, "queue-original").await;
    let repo = MediaDerivativeRepo::new(&pool);
    assert!(
        repo.plan(
            &NewDerivative {
                media_id: id,
                kind: DerivativeKind::AccessCopy,
            },
            DerivativeState::None,
            Some("a browser shows the original"),
            None,
        )
        .await
        .unwrap()
    );
    let row = repo
        .get(id, DerivativeKind::AccessCopy)
        .await
        .unwrap()
        .expect("the row");
    assert_eq!(row.state, DerivativeState::None);
    assert_eq!(row.reason.as_deref(), Some("a browser shows the original"));
    assert!(row.finished.is_some(), "a settled row says when");
    assert_eq!(row.attempts, 0);
}

#[tokio::test]
async fn a_settled_row_keeps_its_reason_and_is_not_reclaimed() {
    let pool = common::memory().await;
    let id = media(&pool, "queue-4").await;
    let repo = MediaDerivativeRepo::new(&pool);
    repo.enqueue(&poster(id)).await.unwrap();
    let claimed = repo.claim_next().await.unwrap().expect("a row");

    assert!(
        repo.finish(
            id,
            DerivativeKind::Poster,
            DerivativeState::Failed,
            Some("the ffmpeg command failed: exit status 1".to_string()),
            Some("ffmpeg".to_string()),
        )
        .await
        .unwrap()
    );
    let row = repo
        .get(id, DerivativeKind::Poster)
        .await
        .unwrap()
        .expect("row");
    assert_eq!(row.state, DerivativeState::Failed);
    assert_eq!(row.tool.as_deref(), Some("ffmpeg"));
    assert_eq!(
        row.reason.as_deref(),
        Some("the ffmpeg command failed: exit status 1")
    );
    assert!(row.finished.is_some());
    assert!(repo.claim_next().await.unwrap().is_none());
    assert!(
        !repo
            .finish(
                id,
                DerivativeKind::Poster,
                DerivativeState::Ready,
                None,
                None
            )
            .await
            .unwrap(),
        "a settled row is not settled twice"
    );
    assert_eq!(claimed.media_id, id);
}

#[tokio::test]
async fn retry_reopens_failed_and_skipped_rows_only() {
    let pool = common::memory().await;
    let failed = media(&pool, "queue-5").await;
    let ready = media(&pool, "queue-6").await;
    let repo = MediaDerivativeRepo::new(&pool);
    for id in [failed, ready] {
        repo.enqueue(&poster(id)).await.unwrap();
        repo.claim_next().await.unwrap().expect("claimed");
    }
    repo.finish(
        failed,
        DerivativeKind::Poster,
        DerivativeState::Failed,
        Some("no".to_string()),
        None,
    )
    .await
    .unwrap();
    repo.finish(
        ready,
        DerivativeKind::Poster,
        DerivativeState::Ready,
        None,
        None,
    )
    .await
    .unwrap();

    assert!(repo.retry(failed, DerivativeKind::Poster).await.unwrap());
    assert!(!repo.retry(ready, DerivativeKind::Poster).await.unwrap());
    let row = repo
        .get(failed, DerivativeKind::Poster)
        .await
        .unwrap()
        .expect("row");
    assert_eq!(row.state, DerivativeState::Pending);
    assert!(row.reason.is_none());
    assert!(row.started.is_none());
}

#[tokio::test]
async fn deleting_the_media_removes_its_rows() {
    let pool = common::memory().await;
    let id = media(&pool, "queue-7").await;
    let repo = MediaDerivativeRepo::new(&pool);
    repo.enqueue(&poster(id)).await.unwrap();
    MediaRepo::new(&pool)
        .delete(id)
        .await
        .expect("delete media");
    assert!(repo.for_media(id).await.unwrap().is_empty());
}

#[test]
fn every_kind_and_state_round_trips_its_name() {
    for kind in DerivativeKind::ALL {
        assert_eq!(DerivativeKind::parse(kind.name()), Some(kind));
    }
    for state in DerivativeState::ALL {
        assert_eq!(DerivativeState::parse(state.name()).unwrap(), state);
    }
    assert_eq!(
        DerivativeKind::parse("poster"),
        Some(DerivativeKind::Poster)
    );
    assert!(DerivativeState::parse("making").is_err());
}

#[tokio::test]
async fn a_worker_of_this_class_is_active_until_it_ends() {
    let pool = common::memory().await;
    let repo = JobRepo::new(&pool);
    assert!(!repo.active_of_class(omeka_api_class()).await.unwrap());
    let job = repo
        .create(&NewJob {
            owner_id: None,
            pid: None,
            status: Some("starting".to_string()),
            class: omeka_api_class().to_string(),
            args: Some(serde_json::json!({})),
            log: None,
            started: None,
        })
        .await
        .unwrap();
    assert!(repo.active_of_class(omeka_api_class()).await.unwrap());
    repo.finish(job.id, "completed").await.unwrap();
    assert!(!repo.active_of_class(omeka_api_class()).await.unwrap());
}

/// The class string a worker row carries: the one
/// `omeka_api::derivatives::DERIVATIVES_JOB` holds, written here because
/// `omeka-db` does not depend on `omeka-api`.
fn omeka_api_class() -> &'static str {
    "Romeka\\Job\\MediaDerivatives"
}
