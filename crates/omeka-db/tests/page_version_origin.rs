//! Migration 0015 on a database at 0014 that holds page versions: version 1 with no user becomes
//! the state before history began, every other row a save, and the author each row names does
//! not change. A row the backfill cannot make is refused by the table.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::path::{Path, PathBuf};

use omeka_db::ids::PageId;
use omeka_db::repos::PageVersionRepo;
use omeka_db::rows::{PageVersionOrigin, VersionAuthor};
use sqlx::migrate::Migrator;

/// A folder holding the SQLite migrations numbered below 0015.
fn migrations_before_0015() -> PathBuf {
    let target = std::env::temp_dir().join(format!(
        "romeka-db-{}-origin-migrations",
        std::process::id()
    ));
    std::fs::create_dir_all(&target).unwrap();
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
    for entry in std::fs::read_dir(source).unwrap() {
        let path = entry.unwrap().path();
        let file = path.file_name().unwrap().to_string_lossy().to_string();
        if file.as_str() < "0015" {
            std::fs::copy(&path, target.join(&file)).unwrap();
        }
    }
    target
}

/// Page 1 kept its first state (version 1, no user) and two saves by user 1, the second user
/// since deleted (no user). Page 2 kept only versions 6 and 7: the trim took 1 to 5, and the
/// saver of 6 was deleted.
const PRE_0015_ROWS: [&str; 9] = [
    "INSERT INTO user (id, email, name, created, role, is_active) VALUES (1, 'ana@example.com', 'Ana', '2026-10-01 00:00:00', 'editor', 1)",
    "INSERT INTO site (id, slug, theme, title, navigation, item_pool, created, is_public, assign_new_items) VALUES (1, 'history', 'default', 'History', '[]', '[]', '2026-10-01 00:00:00', 1, 0)",
    "INSERT INTO site_page (id, site_id, slug, title, is_public, created) VALUES (1, 1, 'intro', 'Intro', 1, '2026-10-01 00:00:00')",
    "INSERT INTO site_page (id, site_id, slug, title, is_public, created) VALUES (2, 1, 'about', 'About', 1, '2026-10-01 00:00:00')",
    "INSERT INTO romeka_site_page_version (page_id, version, created, user_id, snapshot) VALUES (1, 1, '2026-10-01 00:00:00', NULL, '{}')",
    "INSERT INTO romeka_site_page_version (page_id, version, created, user_id, snapshot) VALUES (1, 2, '2026-10-02 00:00:00', 1, '{}')",
    "INSERT INTO romeka_site_page_version (page_id, version, created, user_id, snapshot) VALUES (1, 3, '2026-10-03 00:00:00', NULL, '{}')",
    "INSERT INTO romeka_site_page_version (page_id, version, created, user_id, snapshot) VALUES (2, 6, '2026-10-06 00:00:00', NULL, '{}')",
    "INSERT INTO romeka_site_page_version (page_id, version, created, user_id, snapshot) VALUES (2, 7, '2026-10-07 00:00:00', 1, '{}')",
];

#[tokio::test]
async fn sqlite_0015_backfills_each_kept_version_with_what_made_it() {
    let migrations = migrations_before_0015();
    let path = common::test_path("origin-0015");
    common::remove_db(&path);
    let pool = omeka_db::open_file(&path).await.unwrap();
    Migrator::new(migrations.as_path())
        .await
        .unwrap()
        .run(&pool)
        .await
        .unwrap();
    std::fs::remove_dir_all(&migrations).unwrap();
    for statement in PRE_0015_ROWS {
        sqlx::query(statement).execute(&pool).await.unwrap();
    }
    // A deploy migrates in a new process. A connection that read the old schema would answer a
    // `SELECT *` it prepared before the new column.
    pool.close().await;
    let pool = omeka_db::open_file(&path).await.unwrap();

    omeka_db::migrate(&pool).await.unwrap();

    let repo = PageVersionRepo::new(&pool);
    let mut kept = Vec::new();
    for page in [PageId(1), PageId(2)] {
        for row in repo.list(page).await.unwrap() {
            kept.push((page.0, row.version, row.origin, row.author()));
        }
    }
    let ana = VersionAuthor::User(omeka_db::ids::UserId(1));
    assert_eq!(
        kept,
        [
            (1, 3, PageVersionOrigin::Save, VersionAuthor::Deleted),
            (1, 2, PageVersionOrigin::Save, ana),
            (
                1,
                1,
                PageVersionOrigin::BeforeHistory,
                VersionAuthor::NotRecorded
            ),
            (2, 7, PageVersionOrigin::Save, ana),
            (2, 6, PageVersionOrigin::Save, VersionAuthor::Deleted),
        ]
    );
    pool.close().await;
    common::remove_db(&path);
}

#[tokio::test]
async fn a_version_row_that_contradicts_its_origin_is_refused() {
    let pool = common::memory().await;
    for statement in &PRE_0015_ROWS[..4] {
        sqlx::query(statement).execute(&pool).await.unwrap();
    }
    let insert = |version: i64, user: Option<i64>, origin: &'static str| {
        sqlx::query(
            "INSERT INTO romeka_site_page_version (page_id, version, created, user_id, snapshot, origin)
             VALUES (1, ?, '2026-10-01 00:00:00', ?, '{}', ?)",
        )
        .bind(version)
        .bind(user)
        .bind(origin)
        .execute(&pool)
    };

    assert!(
        insert(2, Some(1), "create").await.is_err(),
        "a create is version 1"
    );
    assert!(
        insert(2, None, "before_history").await.is_err(),
        "so is the state before history"
    );
    assert!(
        insert(1, Some(1), "before_history").await.is_err(),
        "which has no user"
    );
    assert!(
        insert(1, Some(1), "restore").await.is_err(),
        "no other origin exists"
    );
    insert(1, Some(1), "create").await.unwrap();
}
