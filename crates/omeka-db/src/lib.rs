//! `omeka-db`: the storage layer for Romeka S, over SQLite or MySQL.
//!
//! Upstream is PHP with Doctrine and MySQL (`application/data/install/schema.sql` at the frozen
//! SHA). This crate keeps the upstream table and column names, the upstream foreign-key delete
//! actions, and the upstream datetime format (`YYYY-MM-DD HH:MM:SS`, UTC). The SQLite schema stores
//! that datetime form in `TEXT`; the MySQL schema is upstream's own DDL, so an existing Omeka S
//! database is a deployment target. Every deliberate divergence is listed in
//! `crates/omeka-db/SCHEMA-NOTES.md`.
//!
//! Shape of the crate:
//!
//! - [`open_memory`] and [`open_file`] build a SQLite pool with `PRAGMA foreign_keys=ON` (per
//!   connection), a 5 second busy timeout, and WAL journaling for file databases. [`open_mysql`]
//!   builds a MySQL pool from a URL.
//! - [`migrate`] applies the embedded SQLite DDL and [`migrate_mysql`] the embedded MySQL DDL;
//!   running either twice is a no-op.
//! - [`rows`] holds one struct per upstream table with a `from_row` that is the storage boundary.
//! - [`repos`] holds repositories over those rows. They are generic over [`Backend`], so one
//!   implementation serves both engines and the dialect differences live in [`backend`]. SQL
//!   strings live here and nowhere else in the workspace.
//!
//! Runtime `sqlx` queries only. No compile-time macros, no `DATABASE_URL` at build time. The MySQL
//! driver is behind the `mysql` feature; `open_mysql`, `migrate_mysql` and the MySQL
//! [`Backend`] impl exist only with it on.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod error;
mod time;

pub mod backend;
pub mod ids;
pub mod repos;
pub mod rows;
pub mod schema;

pub use backend::Backend;
pub use error::DbError;
pub use schema::{MIGRATOR, migrate};
#[cfg(feature = "mysql")]
pub use schema::{MYSQL_MIGRATOR, migrate_mysql};

use std::path::Path;
use std::time::Duration;

use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};

/// How long a connection waits on a locked database before returning `SQLITE_BUSY`.
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

/// Connections a file-backed pool keeps open. SQLite serialises writers anyway; readers benefit.
const FILE_POOL_CONNECTIONS: u32 = 5;

fn base_options() -> SqliteConnectOptions {
    SqliteConnectOptions::new()
        .foreign_keys(true)
        .busy_timeout(BUSY_TIMEOUT)
        .synchronous(SqliteSynchronous::Normal)
}

/// Open a private in-memory SQLite database, ready for [`migrate`].
///
/// One connection: SQLite gives every `:memory:` connection its own database, so a pool of two
/// would expose an empty schema on the second connection.
pub async fn open_memory() -> Result<SqlitePool, DbError> {
    let options = base_options().in_memory(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await?;
    Ok(pool)
}

/// Open a SQLite file database, creating the file if it does not exist, ready for [`migrate`].
///
/// WAL journaling keeps readers from blocking the writer, which matters once the server holds a
/// pool while a job writes.
pub async fn open_file(path: impl AsRef<Path>) -> Result<SqlitePool, DbError> {
    let options = base_options()
        .filename(path)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal);
    let pool = SqlitePoolOptions::new()
        .max_connections(FILE_POOL_CONNECTIONS)
        .connect_with(options)
        .await?;
    Ok(pool)
}

/// Connections a MySQL pool keeps open, matching [`FILE_POOL_CONNECTIONS`] so both backends hand
/// the server the same concurrency.
#[cfg(feature = "mysql")]
const MYSQL_POOL_CONNECTIONS: u32 = 5;

/// Open a MySQL pool from a URL (`mysql://user:password@host:port/database`), ready for
/// [`migrate_mysql`].
///
/// The URL names an existing Omeka S database or an empty one. `sqlx` reads every other option
/// (`ssl-mode`, `timezone`, `charset`) from the query string, so the caller has the full
/// [`sqlx::mysql::MySqlConnectOptions`] surface through the URL.
#[cfg(feature = "mysql")]
pub async fn open_mysql(url: &str) -> Result<sqlx::MySqlPool, DbError> {
    use std::str::FromStr;

    use sqlx::mysql::{MySqlConnectOptions, MySqlPoolOptions};

    let options = MySqlConnectOptions::from_str(url)?;
    let pool = MySqlPoolOptions::new()
        .max_connections(MYSQL_POOL_CONNECTIONS)
        .connect_with(options)
        .await?;
    Ok(pool)
}
