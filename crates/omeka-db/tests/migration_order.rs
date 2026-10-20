//! A database that recorded 0007 before 0006 migrates to every version through `migrate`.
//!
//! The 8124 deploy ran a build that carried 0007 but not 0006, so its `_sqlx_migrations` holds
//! 1 to 5 and 7. The next build carries 0006, and `migrate` must apply it below a version that is
//! already recorded, on SQLite and on both MySQL engines. SQLite always; MySQL with the `mysql`
//! feature and a server.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::path::{Path, PathBuf};

use omeka_db::repos::UserRepo;
use omeka_db::rows::NewUser;
use sqlx::migrate::Migrator;

/// A folder holding every migration of `dir` but 0006.
fn migrations_without_0006(dir: &str, name: &str) -> PathBuf {
    let target = common::test_path(name).with_extension("migrations");
    std::fs::create_dir_all(&target).unwrap();
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join(dir);
    for entry in std::fs::read_dir(source).unwrap() {
        let path = entry.unwrap().path();
        let file = path.file_name().unwrap().to_string_lossy().to_string();
        if !file.starts_with("0006") {
            std::fs::copy(&path, target.join(&file)).unwrap();
        }
    }
    target
}

fn ada() -> NewUser {
    NewUser {
        email: "ada@example.com".to_string(),
        name: "Ada".to_string(),
        password_hash: None,
        role: "global_admin".to_string(),
        is_active: true,
        created: None,
    }
}

#[tokio::test]
async fn sqlite_a_database_at_0007_without_0006_migrates_to_every_version() {
    let path = common::test_path("order");
    let pool = omeka_db::open_file(&path).await.unwrap();
    let migrations = migrations_without_0006("migrations", "sqlite-order");
    // An empty database, so the 0004 rebuild has no row its cascade could reach.
    Migrator::new(migrations.as_path())
        .await
        .unwrap()
        .run(&pool)
        .await
        .unwrap();
    std::fs::remove_dir_all(&migrations).unwrap();
    let user = UserRepo::new(&pool).create(&ada()).await.unwrap();

    omeka_db::migrate(&pool).await.unwrap();

    let applied: Vec<i64> =
        sqlx::query_scalar("SELECT version FROM _sqlx_migrations ORDER BY version")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(applied, [1, 2, 3, 4, 5, 6, 7]);
    let imports: i64 = sqlx::query_scalar("SELECT count(*) FROM romeka_import")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(imports, 0, "0006 made the import table");
    assert_eq!(UserRepo::new(&pool).get(user.id).await.unwrap(), Some(user));
    pool.close().await;
    common::remove_db(&path);
}

#[cfg(feature = "mysql")]
#[tokio::test]
async fn mysql_a_database_at_0007_without_0006_migrates_to_every_version() {
    let Some(db) = common::mysql_unmigrated().await else {
        return;
    };
    let migrations = migrations_without_0006("migrations-mysql", "mysql-order");
    Migrator::new(migrations.as_path())
        .await
        .unwrap()
        .run(&db.pool)
        .await
        .unwrap();
    std::fs::remove_dir_all(&migrations).unwrap();
    let user = UserRepo::<sqlx::MySql>::new(&db.pool)
        .create(&ada())
        .await
        .unwrap();

    omeka_db::migrate(&db.pool).await.unwrap();

    let applied: Vec<i64> =
        sqlx::query_scalar("SELECT version FROM _sqlx_migrations ORDER BY version")
            .fetch_all(&db.pool)
            .await
            .unwrap();
    assert_eq!(applied, [1, 2, 3, 5, 6, 7]);
    let imports: i64 = sqlx::query_scalar("SELECT count(*) FROM romeka_import")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(imports, 0, "0006 made the import table");
    assert_eq!(
        UserRepo::<sqlx::MySql>::new(&db.pool)
            .get(user.id)
            .await
            .unwrap(),
        Some(user)
    );
    db.cleanup().await;
}
