//! An import's media get the same playable-copy decision an API create gets
//! (`omeka_api::derivatives::enqueue_access`): one `access_copy` row for each
//! created audio, video or stored image, none for a PDF. The decision never
//! fails the run, a retry decides nothing twice, and an undo leaves no row.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod import_site;

use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use import_site::{Site, site};
use omeka_db::ids::{ImportId, JobId, MediaId};
use omeka_db::repos::{JobRepo, MediaDerivativeRepo};
use omeka_db::rows::{DerivativeKind, DerivativeRow, DerivativeState, ImportStatus, MediaRow};
use omeka_media::store::Store;

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/media-kinds")
            .join(name),
    )
    .unwrap()
}

/// One video, one image and one PDF, each its own item.
fn three_kinds(site: &Site) {
    site.write_object("reel.mkv", &fixture("video.mkv"));
    site.write_object("photo.jpg", &fixture("image.jpg"));
    site.write_object("letter.pdf", &fixture("doc.pdf"));
}

/// Start the run and wait for its job, not only its import, to end: the
/// decisions are made after the import is done.
async fn run_to_the_end(site: &Site, id: u64) -> (ImportStatus, String) {
    let job = JobId::from(site.start(id).await);
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        let row = JobRepo::new(&site.pool).get(job).await.unwrap().unwrap();
        if matches!(
            row.status.as_deref(),
            Some("completed" | "error" | "stopped")
        ) {
            assert_eq!(row.status.as_deref(), Some("completed"), "{:?}", row.log);
            return (site.row(id).await.status, row.log.unwrap_or_default());
        }
        assert!(Instant::now() < deadline, "job {} did not end", job.0);
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

async fn media_typed(site: &Site, prefix: &str) -> MediaRow {
    site.media()
        .await
        .into_iter()
        .find(|media| {
            media
                .media_type
                .as_deref()
                .is_some_and(|t| t.starts_with(prefix))
        })
        .unwrap_or_else(|| panic!("no {prefix} media"))
}

async fn access_row(site: &Site, media: MediaId) -> Option<DerivativeRow> {
    MediaDerivativeRepo::new(&site.pool)
        .for_media(media)
        .await
        .unwrap()
        .into_iter()
        .find(|row| row.kind == DerivativeKind::AccessCopy)
}

#[tokio::test(flavor = "multi_thread")]
async fn a_run_records_a_playable_copy_decision_for_its_video_and_image_only() {
    let site = site().await;
    three_kinds(&site);
    let id = site.draft(|_| {}).await;

    let (status, _) = run_to_the_end(&site, id).await;

    assert_eq!(status, ImportStatus::Done);
    for prefix in ["video/", "image/"] {
        let media = media_typed(&site, prefix).await;
        let row = access_row(&site, media.id())
            .await
            .unwrap_or_else(|| panic!("the {prefix} media has an access_copy row"));
        match row.state {
            DerivativeState::Pending => assert!(row.tool.is_some(), "{row:?}"),
            DerivativeState::None => assert!(row.reason.is_some(), "{row:?}"),
            other => panic!("{prefix} access_copy is {other:?}"),
        }
    }
    let pdf = media_typed(&site, "application/pdf").await;
    assert!(
        access_row(&site, pdf.id()).await.is_none(),
        "a PDF gets no access_copy row"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_second_pass_over_a_done_import_decides_nothing() {
    let site = site().await;
    three_kinds(&site);
    let id = site.draft(|_| {}).await;
    run_to_the_end(&site, id).await;
    let store: Arc<dyn Store> = site.store.clone();
    let pool = omeka_db::DbPool::from(site.pool.clone());

    let again = omeka_api::import::access_copies::queue(&pool, &store, ImportId(id))
        .await
        .unwrap();

    assert_eq!(again.decided, 0, "each media was decided once");
    assert!(again.failures.is_empty(), "{:?}", again.failures);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_run_whose_decisions_fail_still_ends_done_and_logs_each() {
    let site = site().await;
    three_kinds(&site);
    let id = site.draft(|_| {}).await;
    sqlx::query("ALTER TABLE romeka_media_derivative RENAME TO romeka_media_derivative_gone")
        .execute(&site.pool)
        .await
        .unwrap();

    let (status, log) = run_to_the_end(&site, id).await;

    assert_eq!(status, ImportStatus::Done);
    assert_eq!(site.media().await.len(), 3);
    assert!(log.contains("playable cop"), "{log}");
}

#[tokio::test(flavor = "multi_thread")]
async fn an_undone_run_leaves_no_access_copy_row() {
    let site = site().await;
    three_kinds(&site);
    let id = site.draft(|_| {}).await;
    run_to_the_end(&site, id).await;
    let media: Vec<MediaId> = site.media().await.iter().map(MediaRow::id).collect();
    assert!(
        !MediaDerivativeRepo::new(&site.pool)
            .for_media_ids(&media)
            .await
            .unwrap()
            .is_empty()
    );

    let started = site
        .send("POST", &format!("/api/romeka/imports/{id}/undo"), None)
        .await;
    assert_eq!(started.status, 200, "{}", started.body);
    let deadline = Instant::now() + Duration::from_secs(60);
    while site.row(id).await.status == ImportStatus::Undoing {
        assert!(Instant::now() < deadline, "import {id} stayed undoing");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    assert_eq!(site.row(id).await.status, ImportStatus::Undone);
    assert!(site.media().await.is_empty());
    assert!(
        MediaDerivativeRepo::new(&site.pool)
            .for_media_ids(&media)
            .await
            .unwrap()
            .is_empty(),
        "the cascade removed every derivative row"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_video_queued_for_ffmpeg_gives_the_worker_a_job() {
    let site = site().await;
    three_kinds(&site);
    let id = site.draft(|_| {}).await;
    run_to_the_end(&site, id).await;
    let video = media_typed(&site, "video/").await;
    let row = access_row(&site, video.id()).await.unwrap();
    if row.state != DerivativeState::Pending {
        // No ffmpeg on this machine: the decision is none, with its reason.
        assert!(row.reason.is_some(), "{row:?}");
        return;
    }
    assert_eq!(row.tool.as_deref(), Some("ffmpeg"), "{row:?}");

    let pool = omeka_db::DbPool::from(site.pool.clone());
    let job = omeka_api::derivatives::queue_worker(&pool, None)
        .await
        .unwrap();

    assert!(job.is_some(), "the worker claims a pending ffmpeg access_copy row");
}
