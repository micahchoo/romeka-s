//! 0011 rebuilds `romeka_media_annotation` so a `rect` row may hold a time range (a box on a
//! video's frames over a span). The claim under test: every note made before the upgrade comes
//! through with its id, its text stays searchable, a deleted id stays unused, and the table then
//! takes a box with a range while still refusing a polygon with one.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::path::PathBuf;

use omeka_db::repos::{ItemRepo, MediaRepo};
use omeka_db::rows::{NewItem, NewMedia, NewResource};
use omeka_db::{migrate, open_file};
use sqlx::migrate::Migrator;
use sqlx::{Row, SqlitePool};

/// A database as every migration before 0011 left it.
async fn before_upgrade(name: &str) -> (SqlitePool, PathBuf) {
    let migrations = std::env::temp_dir().join(format!(
        "romeka-db-{}-{name}-migrations",
        std::process::id()
    ));
    std::fs::create_dir_all(&migrations).unwrap();
    let source = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/migrations"));
    for entry in std::fs::read_dir(&source).unwrap() {
        let file = entry.unwrap().file_name();
        if file.to_string_lossy().as_ref() < "0011" {
            std::fs::copy(source.join(&file), migrations.join(&file)).unwrap();
        }
    }
    let path = common::test_path(name);
    common::remove_db(&path);
    let pool = open_file(&path).await.unwrap();
    Migrator::new(migrations.as_path())
        .await
        .unwrap()
        .run(&pool)
        .await
        .unwrap();
    std::fs::remove_dir_all(&migrations).unwrap();
    (pool, path)
}

async fn a_media(pool: &SqlitePool) -> i64 {
    let item = ItemRepo::new(pool)
        .create(&NewItem {
            resource: NewResource::default(),
            primary_media_id: None,
        })
        .await
        .unwrap();
    let media = MediaRepo::new(pool)
        .create(&NewMedia {
            item_id: item.id(),
            ingester: "upload".to_string(),
            renderer: "file".to_string(),
            ..NewMedia::default()
        })
        .await
        .unwrap();
    i64::try_from(media.id().0).unwrap()
}

async fn insert(
    pool: &SqlitePool,
    media: i64,
    kind: &str,
    selector: Option<&str>,
    time: Option<(i64, i64)>,
    text: &str,
) -> Result<i64, sqlx::Error> {
    let row = sqlx::query(
        "INSERT INTO romeka_media_annotation
             (media_id, created, modified, motivation, selector_kind, selector,
              t_start_ms, t_end_ms, body_json, body_text)
         VALUES (?, '2026-10-21 10:00:00', '2026-10-21 10:00:00', 'commenting', ?, ?, ?, ?, '[]', ?)
         RETURNING id",
    )
    .bind(media)
    .bind(kind)
    .bind(selector)
    .bind(time.map(|(start, _)| start))
    .bind(time.map(|(_, end)| end))
    .bind(text)
    .fetch_one(pool)
    .await?;
    Ok(row.get("id"))
}

async fn rows(pool: &SqlitePool) -> Vec<(i64, String, Option<String>, Option<i64>, String)> {
    sqlx::query(
        "SELECT id, selector_kind, selector, t_start_ms, body_text
         FROM romeka_media_annotation ORDER BY id",
    )
    .fetch_all(pool)
    .await
    .unwrap()
    .iter()
    .map(|row| {
        (
            row.get("id"),
            row.get("selector_kind"),
            row.get("selector"),
            row.get("t_start_ms"),
            row.get("body_text"),
        )
    })
    .collect()
}

async fn found(pool: &SqlitePool, word: &str) -> Vec<i64> {
    sqlx::query_scalar(
        "SELECT rowid FROM romeka_media_annotation_fts
         WHERE romeka_media_annotation_fts MATCH ? ORDER BY rowid",
    )
    .bind(word)
    .fetch_all(pool)
    .await
    .unwrap()
}

#[tokio::test]
async fn the_upgrade_keeps_every_note_and_then_takes_a_box_over_a_span() {
    let (pool, path) = before_upgrade("annotation-box-time").await;
    let media = a_media(&pool).await;
    let region = insert(
        &pool,
        media,
        "rect",
        Some("1,2,3,4"),
        None,
        "a crowned shield",
    )
    .await
    .unwrap();
    let moment = insert(&pool, media, "time", None, Some((1000, 2000)), "a footstep")
        .await
        .unwrap();
    let last = insert(&pool, media, "whole", None, None, "deleted later")
        .await
        .unwrap();
    sqlx::query("DELETE FROM romeka_media_annotation WHERE id = ?")
        .bind(last)
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        insert(
            &pool,
            media,
            "rect",
            Some("1,2,3,4"),
            Some((0, 500)),
            "too early"
        )
        .await
        .is_err(),
        "before 0011 a box cannot hold a time range"
    );
    let before = rows(&pool).await;

    migrate(&pool).await.unwrap();

    assert_eq!(
        rows(&pool).await,
        before,
        "every note keeps its id and columns"
    );
    assert_eq!(found(&pool, "shield").await, vec![region]);
    assert_eq!(found(&pool, "footstep").await, vec![moment]);

    let boxed = insert(
        &pool,
        media,
        "rect",
        Some("10,20,30,40"),
        Some((0, 500)),
        "the hands",
    )
    .await
    .unwrap();
    assert!(boxed > last, "a deleted id stays unused");
    assert_eq!(
        found(&pool, "hands").await,
        vec![boxed],
        "the triggers index new notes"
    );
    assert!(
        insert(
            &pool,
            media,
            "rect",
            Some("1,2,3,4"),
            Some((500, 500)),
            "empty span"
        )
        .await
        .is_err(),
        "a box's range must end after it starts"
    );
    assert!(
        insert(
            &pool,
            media,
            "polygon",
            Some("0,0 1,0 1,1"),
            Some((0, 500)),
            "no"
        )
        .await
        .is_err(),
        "a polygon still holds no time range"
    );
    sqlx::query("DELETE FROM romeka_media_annotation WHERE id = ?")
        .bind(region)
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        found(&pool, "shield").await.is_empty(),
        "a delete leaves the index"
    );

    pool.close().await;
    common::remove_db(&path);
}
