//! A database that took 0007 before 0006 (main shipped the page versions before import v2 landed)
//! migrates to the full set through `omeka_db::migrate`, and ends with the schema a fresh database
//! gets. SQLite always; MySQL with the `mysql` feature and a server (`ROMEKA_MYSQL_URL`), which is
//! how the MySQL 8.0 and MariaDB 11 runs are made.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::path::{Path, PathBuf};

use sqlx::SqlitePool;
use sqlx::migrate::Migrator;

/// The version the out-of-order database lacks.
const LATE: &str = "0006";

/// Every version an embedded set holds, in order.
fn all_versions(set: &Migrator) -> Vec<i64> {
    set.iter().map(|migration| migration.version).collect()
}

/// A folder holding the migrations of `dir` except [`LATE`], for a [`Migrator`] that leaves a
/// gap below the newest version.
fn migrations_without_late(dir: &str, name: &str) -> PathBuf {
    let target = std::env::temp_dir().join(format!(
        "romeka-db-{}-{name}-migrations",
        std::process::id()
    ));
    std::fs::create_dir_all(&target).unwrap();
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join(dir);
    for entry in std::fs::read_dir(source).unwrap() {
        let path = entry.unwrap().path();
        let file = path.file_name().unwrap().to_string_lossy().to_string();
        if !file.starts_with(LATE) {
            std::fs::copy(&path, target.join(&file)).unwrap();
        }
    }
    target
}

async fn sqlite_versions(pool: &SqlitePool) -> Vec<i64> {
    sqlx::query_scalar("SELECT version FROM _sqlx_migrations ORDER BY version")
        .fetch_all(pool)
        .await
        .unwrap()
}

/// Every table, index and trigger with its SQL, sorted, without the sqlx bookkeeping.
async fn sqlite_schema(pool: &SqlitePool) -> Vec<(String, String, Option<String>)> {
    sqlx::query_as(
        "SELECT type, name, sql FROM sqlite_master WHERE name != '_sqlx_migrations'
         ORDER BY type, name",
    )
    .fetch_all(pool)
    .await
    .unwrap()
}

#[tokio::test]
async fn sqlite_0006_after_0007_reaches_the_fresh_schema() {
    let migrations = migrations_without_late("migrations", "sqlite-order");
    let path = common::test_path("order-0006-after-0007");
    common::remove_db(&path);
    let pool = omeka_db::open_file(&path).await.unwrap();
    let partial = Migrator::new(migrations.as_path()).await.unwrap();
    std::fs::remove_dir_all(&migrations).unwrap();
    partial.run(&pool).await.unwrap();
    let mut gap = all_versions(&omeka_db::MIGRATOR);
    gap.retain(|version| *version != 6);
    assert_eq!(sqlite_versions(&pool).await, gap, "the gap is in place");

    omeka_db::migrate(&pool).await.unwrap();

    assert_eq!(
        sqlite_versions(&pool).await,
        all_versions(&omeka_db::MIGRATOR)
    );
    let fresh = common::memory().await;
    assert_eq!(sqlite_schema(&pool).await, sqlite_schema(&fresh).await);
    pool.close().await;
    common::remove_db(&path);
}

#[cfg(feature = "mysql")]
mod mysql {
    use super::*;

    async fn create(base: &str, name: &str) -> Option<sqlx::MySqlPool> {
        let admin = match omeka_db::open_mysql(base).await {
            Ok(pool) => pool,
            Err(error) => {
                eprintln!("SKIP: cannot reach {base}: {error}");
                return None;
            }
        };
        for statement in [
            format!("DROP DATABASE IF EXISTS `{name}`"),
            format!("CREATE DATABASE `{name}` CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci"),
        ] {
            sqlx::query(&statement).execute(&admin).await.unwrap();
        }
        Some(
            omeka_db::open_mysql(&common::with_database(base, name))
                .await
                .unwrap(),
        )
    }

    async fn drop_database(base: &str, name: &str) {
        let admin = omeka_db::open_mysql(base).await.unwrap();
        sqlx::query(&format!("DROP DATABASE IF EXISTS `{name}`"))
            .execute(&admin)
            .await
            .unwrap();
    }

    async fn versions(pool: &sqlx::MySqlPool) -> Vec<i64> {
        sqlx::query_scalar("SELECT version FROM _sqlx_migrations ORDER BY version")
            .fetch_all(pool)
            .await
            .unwrap()
    }

    /// Every column with its type, nullability, default and collation, every index column, and
    /// every foreign key, sorted. The column position is left out: two migrations that add
    /// columns to one table in the other order give the same table.
    async fn schema(pool: &sqlx::MySqlPool) -> Vec<String> {
        let queries = [
            "SELECT CAST(CONCAT_WS('|', 'column', table_name, column_name, column_type,
                    is_nullable, IFNULL(column_default, 'NULL'), IFNULL(collation_name, ''), extra)
                    AS CHAR)
             FROM information_schema.columns
             WHERE table_schema = DATABASE() AND table_name != '_sqlx_migrations'",
            "SELECT CAST(CONCAT_WS('|', 'index', table_name, index_name, non_unique, seq_in_index,
                    column_name, IFNULL(sub_part, '')) AS CHAR)
             FROM information_schema.statistics
             WHERE table_schema = DATABASE() AND table_name != '_sqlx_migrations'",
            "SELECT CAST(CONCAT_WS('|', 'key', k.table_name, k.constraint_name, k.column_name,
                    k.referenced_table_name, k.referenced_column_name, r.delete_rule) AS CHAR)
             FROM information_schema.key_column_usage k
             JOIN information_schema.referential_constraints r
               ON r.constraint_schema = k.constraint_schema
              AND r.constraint_name = k.constraint_name
             WHERE k.table_schema = DATABASE()",
        ];
        let mut rows = Vec::new();
        for query in queries {
            let found: Vec<String> = sqlx::query_scalar(query).fetch_all(pool).await.unwrap();
            rows.extend(found);
        }
        rows.sort();
        rows
    }

    #[tokio::test]
    async fn mysql_0006_after_0007_reaches_the_fresh_schema() {
        let Some(base) = common::mysql_url() else {
            return;
        };
        let (late, fresh) = (
            format!("romeka_order_{}", std::process::id()),
            format!("romeka_order_fresh_{}", std::process::id()),
        );
        let Some(pool) = create(&base, &late).await else {
            return;
        };
        let migrations = migrations_without_late("migrations-mysql", "mysql-order");
        let partial = Migrator::new(migrations.as_path()).await.unwrap();
        std::fs::remove_dir_all(&migrations).unwrap();
        partial.run(&pool).await.unwrap();
        let mut gap = all_versions(&omeka_db::MYSQL_MIGRATOR);
        gap.retain(|version| *version != 6);
        assert_eq!(versions(&pool).await, gap, "the gap is in place");

        omeka_db::migrate_mysql(&pool).await.unwrap();

        assert_eq!(
            versions(&pool).await,
            all_versions(&omeka_db::MYSQL_MIGRATOR)
        );
        let fresh_pool = create(&base, &fresh).await.unwrap();
        omeka_db::migrate_mysql(&fresh_pool).await.unwrap();
        let (got, want) = (schema(&pool).await, schema(&fresh_pool).await);
        pool.close().await;
        fresh_pool.close().await;
        drop_database(&base, &late).await;
        drop_database(&base, &fresh).await;
        assert!(!want.is_empty());
        assert_eq!(got, want);
    }
}
