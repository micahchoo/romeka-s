//! Shared helpers for the integration tests.
#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use omeka_db::{migrate, open_file, open_memory};
use sqlx::{Row, SqlitePool};

/// A migrated in-memory database.
pub async fn memory() -> SqlitePool {
    let pool = open_memory().await.expect("open in-memory database");
    migrate(&pool).await.expect("apply migrations");
    pool
}

/// A unique file path for one test, under the system temp directory.
///
/// The process id alone is not enough. A pid can be reused, and one binary can
/// ask for several databases, so a counter and a clock reading make the name
/// unique across processes and within one.
pub fn test_path(name: &str) -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let seq = NEXT.fetch_add(1, Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.subsec_nanos());
    std::env::temp_dir().join(format!(
        "romeka-db-{}-{seq}-{nanos}-{name}.sqlite",
        std::process::id()
    ))
}

/// A migrated file database and the path holding it.
pub async fn file(name: &str) -> (SqlitePool, PathBuf) {
    let path = test_path(name);
    remove_db(&path);
    let pool = open_file(&path).await.expect("open file database");
    migrate(&pool).await.expect("apply migrations");
    (pool, path)
}

/// Remove a test database and its write-ahead log and shared-memory files.
pub fn remove_db(path: &Path) {
    for suffix in ["", "-wal", "-shm"] {
        let mut name = OsString::from(path.as_os_str());
        name.push(suffix);
        let candidate = PathBuf::from(name);
        if candidate.exists() {
            match std::fs::remove_file(&candidate) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => panic!("remove test database file {}: {error}", candidate.display()),
            }
        }
    }
}

/// One index sqlite reports for a table: its name, its uniqueness and its columns in order.
pub struct PortIndex {
    pub name: String,
    pub unique: bool,
    pub columns: Vec<String>,
}

/// One foreign key sqlite reports for a table.
pub struct PortForeignKey {
    pub column: String,
    pub parent: String,
    pub parent_column: String,
    pub on_delete: String,
}

/// The primary key columns of a table, in key order.
///
/// A single `INTEGER PRIMARY KEY` is the rowid alias and gets no index entry, so this reads
/// `PRAGMA table_info` rather than `PRAGMA index_list`.
pub async fn primary_key(pool: &SqlitePool, table: &str) -> Vec<String> {
    let info = sqlx::query(&format!("PRAGMA table_info({table})"))
        .fetch_all(pool)
        .await
        .expect("read table_info");
    let mut keyed: Vec<(i64, String)> = info
        .iter()
        .filter_map(|row| {
            let position: i64 = row.get("pk");
            if position > 0 {
                Some((position, row.get("name")))
            } else {
                None
            }
        })
        .collect();
    keyed.sort_by_key(|(position, _)| *position);
    keyed.into_iter().map(|(_, column)| column).collect()
}

/// Every index sqlite reports for a table, with its columns in order.
pub async fn indexes(pool: &SqlitePool, table: &str) -> Vec<PortIndex> {
    let list = sqlx::query(&format!("PRAGMA index_list({table})"))
        .fetch_all(pool)
        .await
        .expect("read index_list");
    let mut found = Vec::new();
    for entry in &list {
        let name: String = entry.get("name");
        let unique: i64 = entry.get("unique");
        let info = sqlx::query(&format!("PRAGMA index_info({name})"))
            .fetch_all(pool)
            .await
            .expect("read index_info");
        let mut columns: Vec<(i64, String)> = info
            .iter()
            .map(|row| (row.get("seqno"), row.get("name")))
            .collect();
        columns.sort_by_key(|(seqno, _)| *seqno);
        found.push(PortIndex {
            name,
            unique: unique == 1,
            columns: columns.into_iter().map(|(_, column)| column).collect(),
        });
    }
    found
}

/// Every foreign key sqlite reports for a table.
pub async fn foreign_keys(pool: &SqlitePool, table: &str) -> Vec<PortForeignKey> {
    let rows = sqlx::query(&format!("PRAGMA foreign_key_list({table})"))
        .fetch_all(pool)
        .await
        .expect("read foreign_key_list");
    rows.iter()
        .map(|row| {
            let to: Option<String> = row.get("to");
            PortForeignKey {
                column: row.get("from"),
                parent: row.get("table"),
                // SQLite omits `to` when the key references the parent's primary key, which is
                // `id` on every parent table in this schema.
                parent_column: to.unwrap_or_else(|| "id".to_string()),
                on_delete: row.get("on_delete"),
            }
        })
        .collect()
}

/// Assert the table's primary key columns exactly, in order.
pub async fn assert_primary_key(pool: &SqlitePool, table: &str, expected: &[&str]) {
    let found = primary_key(pool, table).await;
    assert_eq!(found, expected, "primary key of {table}");
}

/// Assert the table has an index on exactly these columns in order, with this uniqueness.
pub async fn assert_index(pool: &SqlitePool, table: &str, unique: bool, expected: &[&str]) {
    let found = indexes(pool, table).await;
    let matched = found
        .iter()
        .any(|index| index.unique == unique && index.columns == expected);
    let seen: Vec<(&str, bool, &[String])> = found
        .iter()
        .map(|index| (index.name.as_str(), index.unique, index.columns.as_slice()))
        .collect();
    assert!(
        matched,
        "no index on {table} ({expected:?}, unique {unique}); port has {seen:?}"
    );
}

/// Assert a foreign key exists from the child column to the parent column with this ON DELETE.
pub async fn assert_foreign_key(
    pool: &SqlitePool,
    table: &str,
    column: &str,
    parent: &str,
    parent_column: &str,
    action: &str,
) {
    let found = foreign_keys(pool, table).await;
    let matched = found.iter().any(|key| {
        key.column == column
            && key.parent == parent
            && key.parent_column == parent_column
            && key.on_delete == action
    });
    let seen: Vec<(&str, &str, &str, &str)> = found
        .iter()
        .map(|key| {
            (
                key.column.as_str(),
                key.parent.as_str(),
                key.parent_column.as_str(),
                key.on_delete.as_str(),
            )
        })
        .collect();
    assert!(
        matched,
        "no foreign key {table}.{column} -> {parent}.{parent_column} ON DELETE {action}; port has {seen:?}"
    );
}

/// One test's MySQL database, or `None` when no server answers.
///
/// The suite reads `ROMEKA_MYSQL_URL`, a server URL with no database
/// (`mysql://root:secret@127.0.0.1:3306`). Each call creates its own database, named from the
/// process id and a counter, so tests never share state. [`MysqlTest::cleanup`] drops it again.
/// With no variable set, or a server that does not answer or refuses `CREATE DATABASE`, the helper
/// prints why and returns `None`, and the test returns: the suite skips loudly.
#[cfg(feature = "mysql")]
pub struct MysqlTest {
    pub pool: sqlx::MySqlPool,
    base: String,
    name: String,
}

#[cfg(feature = "mysql")]
impl MysqlTest {
    /// Drop the test's database. A test that panics before this leaves its database behind; the
    /// name carries the process id and a counter, so the next run never collides with it.
    pub async fn cleanup(&self) {
        if let Ok(admin) = omeka_db::open_mysql(&self.base).await {
            let _ = sqlx::query(&format!("DROP DATABASE IF EXISTS `{}`", self.name))
                .execute(&admin)
                .await;
        }
    }
}

/// The server URL for the MySQL suite, or `None` when the variable is unset.
#[cfg(feature = "mysql")]
pub fn mysql_url() -> Option<String> {
    match std::env::var("ROMEKA_MYSQL_URL") {
        Ok(url) if !url.trim().is_empty() => Some(url.trim().to_string()),
        _ => {
            eprintln!("SKIP: ROMEKA_MYSQL_URL is not set, so the MySQL suite did not run");
            None
        }
    }
}

/// Replace a URL's database with `name`, keeping the authority and any query string.
#[cfg(feature = "mysql")]
pub fn with_database(base: &str, name: &str) -> String {
    let (head, query) = match base.split_once('?') {
        Some((head, query)) => (head, Some(query)),
        None => (base, None),
    };
    let authority_end = head.find("://").map_or(0, |index| index + 3);
    let path_end = head[authority_end..]
        .find('/')
        .map_or(head.len(), |index| authority_end + index);
    let mut url = format!("{}/{name}", &head[..path_end]);
    if let Some(query) = query {
        url.push('?');
        url.push_str(query);
    }
    url
}

/// A migrated MySQL database, or `None` when the suite should skip.
#[cfg(feature = "mysql")]
pub async fn mysql() -> Option<MysqlTest> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let base = mysql_url()?;
    let seq = NEXT.fetch_add(1, Ordering::Relaxed);
    let name = format!("romeka_test_{}_{seq}", std::process::id());

    let admin = match omeka_db::open_mysql(&base).await {
        Ok(pool) => pool,
        Err(error) => {
            eprintln!("SKIP: cannot reach {base}: {error}");
            return None;
        }
    };
    let _ = sqlx::query(&format!("DROP DATABASE IF EXISTS `{name}`"))
        .execute(&admin)
        .await;
    let create = sqlx::query(&format!(
        "CREATE DATABASE `{name}` CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci"
    ))
    .execute(&admin)
    .await;
    if let Err(error) = create {
        eprintln!("SKIP: cannot create database {name}: {error}");
        return None;
    }

    let url = with_database(&base, &name);
    let pool = match omeka_db::open_mysql(&url).await {
        Ok(pool) => pool,
        Err(error) => {
            eprintln!("SKIP: cannot open {url}: {error}");
            return None;
        }
    };
    if let Err(error) = omeka_db::migrate_mysql(&pool).await {
        eprintln!("SKIP: cannot migrate {name}: {error}");
        return None;
    }
    Some(MysqlTest { pool, base, name })
}
