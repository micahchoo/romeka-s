//! Migration 0006 on a database at 0005 that holds v1 import rows: the v1 import tables are
//! replaced, their rows go, and every other table keeps every row. A fresh database gets the same
//! v2 tables. SQLite always; MySQL with the `mysql` feature and a server.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::path::{Path, PathBuf};

use sqlx::migrate::Migrator;
use sqlx::{Row, SqlitePool};

const V2_TABLES: [&str; 6] = [
    "romeka_import",
    "romeka_import_before",
    "romeka_import_entry",
    "romeka_import_resource",
    "romeka_import_scan",
    "romeka_import_upload",
];

/// The tables migrations after 0006 add, empty on an upgrade.
const LATER_TABLES: [&str; 5] = [
    "romeka_media_annotation",
    "romeka_media_derivative",
    "romeka_media_extent",
    "romeka_site_page_version",
    "romeka_user_upload",
];

/// A folder holding the migrations of `dir` numbered below 0006, for a [`Migrator`] that stops at
/// 0005.
fn migrations_before_0006(dir: &str, name: &str) -> PathBuf {
    let target = std::env::temp_dir().join(format!(
        "romeka-db-{}-{name}-migrations",
        std::process::id()
    ));
    std::fs::create_dir_all(&target).unwrap();
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join(dir);
    for entry in std::fs::read_dir(source).unwrap() {
        let path = entry.unwrap().path();
        let file = path.file_name().unwrap().to_string_lossy().to_string();
        if file.as_str() < "0006" {
            std::fs::copy(&path, target.join(&file)).unwrap();
        }
    }
    target
}

const ITEM_TYPE: &str = "Omeka\\Entity\\Item";

/// The v1 rows 0006 drops, and one item with a value it must keep. A `?` takes [`ITEM_TYPE`],
/// bound because the two engines read a backslash in a literal differently.
const V1_ROWS: [&str; 7] = [
    "INSERT INTO vocabulary (id, namespace_uri, prefix, label) VALUES (1, 'http://purl.org/dc/terms/', 'dcterms', 'Dublin Core')",
    "INSERT INTO property (id, vocabulary_id, local_name, label) VALUES (1, 1, 'title', 'Title')",
    "INSERT INTO resource (id, is_public, created, modified, resource_type) VALUES (1, 1, '2026-10-01 00:00:00', '2026-10-01 00:00:00', ?)",
    "INSERT INTO item (id) VALUES (1)",
    "INSERT INTO value (resource_id, property_id, type, value, is_public) VALUES (1, 1, 'literal', 'Kept', 1)",
    "INSERT INTO romeka_import (id, folder, plan, status, undoable, created) VALUES (1, 'Box', '{}', 'done', 1, '2026-10-01 00:00:00')",
    "INSERT INTO romeka_import_resource (import_id, kind, target_id) VALUES (1, 'item', 1)",
];

/// Every row of every table but the import ones and the sqlx bookkeeping, sorted.
async fn sqlite_contents(pool: &SqlitePool) -> Vec<(String, Vec<String>)> {
    let tables: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM sqlite_master WHERE type = 'table' AND name != '_sqlx_migrations'
         AND name NOT LIKE 'romeka_import%' AND name NOT LIKE 'romeka_media_annotation_fts%'
         AND sql NOT LIKE 'CREATE VIRTUAL TABLE%'
         ORDER BY name",
    )
    .fetch_all(pool)
    .await
    .unwrap();
    let mut dump = Vec::new();
    for table in tables {
        let columns: Vec<String> = sqlx::query(&format!("PRAGMA table_info(\"{table}\")"))
            .fetch_all(pool)
            .await
            .unwrap()
            .iter()
            .map(|row| format!("quote(\"{}\")", row.get::<String, _>("name")))
            .collect();
        let mut rows: Vec<String> = sqlx::query_scalar(&format!(
            "SELECT {} FROM \"{table}\"",
            columns.join(" || ',' || ")
        ))
        .fetch_all(pool)
        .await
        .unwrap();
        // The drop removes the v1 import table's rowid counter with the table.
        rows.retain(|row| !row.starts_with("'romeka_import"));
        rows.sort();
        dump.push((table, rows));
    }
    dump
}

async fn sqlite_import_tables(pool: &SqlitePool) -> Vec<String> {
    sqlx::query_scalar(
        "SELECT name FROM sqlite_master WHERE type = 'table' AND name LIKE 'romeka_import%'
         ORDER BY name",
    )
    .fetch_all(pool)
    .await
    .unwrap()
}

#[tokio::test]
async fn sqlite_0006_replaces_the_v1_tables_and_keeps_every_other_row() {
    let migrations = migrations_before_0006("migrations", "sqlite-upgrade");
    let path = common::test_path("upgrade-0006");
    common::remove_db(&path);
    let pool = omeka_db::open_file(&path).await.unwrap();
    Migrator::new(migrations.as_path())
        .await
        .unwrap()
        .run(&pool)
        .await
        .unwrap();
    std::fs::remove_dir_all(&migrations).unwrap();
    for statement in V1_ROWS {
        let query = sqlx::query(statement);
        let query = if statement.contains('?') {
            query.bind(ITEM_TYPE)
        } else {
            query
        };
        query.execute(&pool).await.unwrap();
    }
    let before = sqlite_contents(&pool).await;

    omeka_db::migrate(&pool).await.unwrap();

    let (later, after): (Vec<_>, Vec<_>) = sqlite_contents(&pool)
        .await
        .into_iter()
        .partition(|(table, _)| LATER_TABLES.contains(&table.as_str()));
    assert_eq!(after, before);
    assert_eq!(
        later,
        LATER_TABLES.map(|table| (table.to_string(), Vec::new()))
    );
    assert_eq!(sqlite_import_tables(&pool).await, V2_TABLES);
    let imports: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM romeka_import")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(imports, 0, "the v1 rows are dropped");
    let kept: String = sqlx::query_scalar("SELECT value FROM value")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(kept, "Kept");
    pool.close().await;
    common::remove_db(&path);
}

#[tokio::test]
async fn sqlite_a_fresh_database_gets_the_v2_tables_and_keys() {
    let pool = common::memory().await;
    assert_eq!(sqlite_import_tables(&pool).await, V2_TABLES);
    for (table, key) in [
        (
            "romeka_import_resource",
            &["import_id", "kind", "target_id"][..],
        ),
        (
            "romeka_import_before",
            &["import_id", "kind", "target_id"][..],
        ),
        ("romeka_import_upload", &["import_id", "id"][..]),
        (
            "romeka_import_entry",
            &["import_id", "source", "path_hash"][..],
        ),
        ("romeka_import_scan", &["import_id", "source"][..]),
    ] {
        common::assert_primary_key(&pool, table, key).await;
        common::assert_foreign_key(&pool, table, "import_id", "romeka_import", "id", "CASCADE")
            .await;
    }
    for column in ["owner_id", "job_id", "copied_from"] {
        let parent = match column {
            "owner_id" => "user",
            "job_id" => "job",
            _ => "romeka_import",
        };
        common::assert_foreign_key(&pool, "romeka_import", column, parent, "id", "SET NULL").await;
    }
}

#[cfg(feature = "mysql")]
mod mysql {
    use super::*;

    /// A MySQL database migrated to 0005 only, or `None` when no server answers.
    async fn at_0005() -> Option<(sqlx::MySqlPool, String, String)> {
        let base = common::mysql_url()?;
        let name = format!("romeka_upgrade_{}", std::process::id());
        let admin = match omeka_db::open_mysql(&base).await {
            Ok(pool) => pool,
            Err(error) => {
                eprintln!("SKIP: cannot reach {base}: {error}");
                return None;
            }
        };
        sqlx::query(&format!("DROP DATABASE IF EXISTS `{name}`"))
            .execute(&admin)
            .await
            .unwrap();
        sqlx::query(&format!(
            "CREATE DATABASE `{name}` CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci"
        ))
        .execute(&admin)
        .await
        .unwrap();
        let pool = omeka_db::open_mysql(&common::with_database(&base, &name))
            .await
            .unwrap();
        let migrations = migrations_before_0006("migrations-mysql", "mysql-upgrade");
        Migrator::new(migrations.as_path())
            .await
            .unwrap()
            .run(&pool)
            .await
            .unwrap();
        std::fs::remove_dir_all(&migrations).unwrap();
        Some((pool, base, name))
    }

    async fn tables(pool: &sqlx::MySqlPool) -> Vec<String> {
        sqlx::query_scalar(
            "SELECT CAST(table_name AS CHAR) FROM information_schema.tables
             WHERE table_schema = DATABASE() ORDER BY table_name",
        )
        .fetch_all(pool)
        .await
        .unwrap()
    }

    async fn kept_rows(pool: &sqlx::MySqlPool) -> Vec<String> {
        sqlx::query_scalar(
            "SELECT CONCAT(i.id, ':', v.property_id, ':', v.value)
             FROM item i JOIN `value` v ON v.resource_id = i.id ORDER BY v.id",
        )
        .fetch_all(pool)
        .await
        .unwrap()
    }

    #[tokio::test]
    async fn mysql_0006_replaces_the_v1_tables_and_keeps_every_other_row() {
        let Some((pool, base, name)) = at_0005().await else {
            return;
        };
        for statement in V1_ROWS {
            let statement = statement.replace("INSERT INTO value ", "INSERT INTO `value` ");
            let query = sqlx::query(&statement);
            let query = if statement.contains('?') {
                query.bind(ITEM_TYPE)
            } else {
                query
            };
            query.execute(&pool).await.unwrap();
        }
        let before_tables: Vec<String> = tables(&pool)
            .await
            .into_iter()
            .filter(|table| !table.starts_with("romeka_import"))
            .collect();
        let before_rows = kept_rows(&pool).await;

        omeka_db::migrate(&pool).await.unwrap();

        let after = tables(&pool).await;
        let (imports, others): (Vec<String>, Vec<String>) = after
            .into_iter()
            .partition(|table| table.starts_with("romeka_import"));
        assert_eq!(imports, V2_TABLES);
        let others: Vec<String> = others
            .into_iter()
            .filter(|table| !LATER_TABLES.contains(&table.as_str()))
            .collect();
        assert_eq!(others, before_tables);
        assert_eq!(kept_rows(&pool).await, before_rows);
        assert_eq!(before_rows, ["1:1:Kept"]);
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM romeka_import")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 0, "the v1 rows are dropped");

        pool.close().await;
        let admin = omeka_db::open_mysql(&base).await.unwrap();
        sqlx::query(&format!("DROP DATABASE IF EXISTS `{name}`"))
            .execute(&admin)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn mysql_key_text_is_binary_so_case_makes_two_keys() {
        let Some(db) = common::mysql().await else {
            return;
        };
        let collations: Vec<(String, String, String)> = sqlx::query_as(
            "SELECT CAST(table_name AS CHAR), CAST(column_name AS CHAR),
                    CAST(collation_name AS CHAR)
             FROM information_schema.columns
             WHERE table_schema = DATABASE() AND table_name LIKE 'romeka_import%'
               AND column_name IN ('source', 'path', 'path_hash', 'record', 'id')
               AND collation_name IS NOT NULL
             ORDER BY table_name, column_name",
        )
        .fetch_all(&db.pool)
        .await
        .unwrap();
        assert!(!collations.is_empty());
        for (table, column, collation) in collations {
            assert_eq!(collation, "utf8mb4_bin", "{table}.{column}");
        }
        db.cleanup().await;
    }
}
