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
//!
//! SQLite changes a column's collation or constraints only by rebuilding the table (new table,
//! copy, drop, rename), and the drop runs every `ON DELETE` action of the tables that reference
//! it unless foreign key enforcement is off. The pragma cannot change inside a transaction, and
//! sqlx runs each SQLite migration inside one, so [`migrate`] turns enforcement off on its own
//! connection around the whole run, and refuses to finish if any reference is then broken.

use sqlx::migrate::Migrator;
use sqlx::{Connection, SqlitePool};

use crate::DbPool;
use crate::error::DbError;

/// The embedded SQLite migration set, resolved from `crates/omeka-db/migrations` at compile time.
pub static MIGRATOR: Migrator = sqlx::migrate!();

/// The embedded MySQL migration set, resolved from `crates/omeka-db/migrations-mysql`.
#[cfg(feature = "mysql")]
pub static MYSQL_MIGRATOR: Migrator = sqlx::migrate!("./migrations-mysql");

/// Apply every embedded migration of the pool's engine that has not been applied yet. `pool` is
/// a SQLite pool, a MySQL pool, or a [`DbPool`] holding either.
///
/// A version below one already recorded is applied too: a deploy may record 0007 before a later
/// build brings 0006.
///
/// # Errors
/// A [`DbError`] for a failed migration, or, on SQLite, [`DbError::BrokenReferences`] when the run
/// left a row pointing at a row that does not exist.
pub async fn migrate<P: Clone + Into<DbPool>>(pool: &P) -> Result<(), DbError> {
    match pool.clone().into() {
        DbPool::Sqlite(pool) => migrate_sqlite(&pool).await,
        #[cfg(feature = "mysql")]
        DbPool::MySql(pool) => {
            MYSQL_MIGRATOR.run(&pool).await?;
            Ok(())
        }
    }
}

async fn migrate_sqlite(pool: &SqlitePool) -> Result<(), DbError> {
    let mut connection = pool.acquire().await?;
    sqlx::query("PRAGMA foreign_keys = OFF")
        .execute(&mut *connection)
        .await?;
    let outcome = run_unenforced(&mut connection).await;
    if outcome.is_ok() {
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&mut *connection)
            .await?;
    } else {
        // The connection may hold enforcement off; it must never serve another query.
        connection.detach().close().await.ok();
    }
    outcome
}

async fn run_unenforced(connection: &mut sqlx::SqliteConnection) -> Result<(), DbError> {
    MIGRATOR.run(&mut *connection).await?;
    let broken: i64 = sqlx::query_scalar("SELECT count(*) FROM pragma_foreign_key_check")
        .fetch_one(&mut *connection)
        .await?;
    if broken > 0 {
        return Err(DbError::BrokenReferences(broken));
    }
    Ok(())
}
