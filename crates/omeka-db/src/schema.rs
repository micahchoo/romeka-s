//! Embedded migrations, one set per backend.
//!
//! `migrations/0001_init.sql` is the SQLite translation of the upstream install schema
//! (`application/data/install/schema.sql` at the frozen SHA). Its effects already include every
//! DDL migration in `application/data/migrations/`; see `SCHEMA-NOTES.md`.
//!
//! `migrations-mysql/0001_init.sql` is upstream's `schema.sql` itself, with `IF NOT EXISTS` added
//! to every `CREATE TABLE`. That is the point of the MySQL backend: the operator points the port at
//! an Omeka S database that already exists, and the migration leaves its tables alone.
//!
//! `sqlx` records applied versions in `_sqlx_migrations`, so running a migration set twice applies
//! the second run's nothing and leaves the schema unchanged. The upstream `migration` table stays
//! empty and exists for import/export fidelity only.

use sqlx::SqlitePool;
use sqlx::migrate::Migrator;

use crate::error::DbError;

/// The embedded SQLite migration set, resolved from `crates/omeka-db/migrations` at compile time.
pub static MIGRATOR: Migrator = sqlx::migrate!();

/// The embedded MySQL migration set, resolved from `crates/omeka-db/migrations-mysql`.
#[cfg(feature = "mysql")]
pub static MYSQL_MIGRATOR: Migrator = sqlx::migrate!("./migrations-mysql");

/// Apply every embedded SQLite migration that has not been applied yet.
pub async fn migrate(pool: &SqlitePool) -> Result<(), DbError> {
    MIGRATOR.run(pool).await?;
    Ok(())
}

/// Apply every embedded MySQL migration that has not been applied yet.
#[cfg(feature = "mysql")]
pub async fn migrate_mysql(pool: &sqlx::MySqlPool) -> Result<(), DbError> {
    MYSQL_MIGRATOR.run(pool).await?;
    Ok(())
}
