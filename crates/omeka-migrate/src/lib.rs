//! omeka-migrate: read a live upstream Omeka S database and write the port's storage.
//!
//! An Omeka S install holds its data in MySQL, in upstream's own schema
//! (`application/data/install/schema.sql` at the frozen SHA). The port stores the same tables in
//! SQLite. This crate is the adoption path between them: it reads the source through `omeka-db`'s
//! MySQL backend and writes the port's SQLite storage through `omeka-db`'s repositories.
//!
//! Shape of a run:
//!
//! 1. A column check reads the source's own column list and fails by name on the first column the
//!    port needs and the source lacks ([`MigrateError::MissingColumn`]). The port does not replay
//!    upstream's `application/data/migrations/`, so a source older than the frozen SHA keeps older
//!    columns, and the migrator refuses it rather than dropping a field silently.
//! 2. A read pass reads every table the port serves, through the same row structs a SQLite read
//!    uses, so MySQL datetimes, JSON columns and booleans decode through one boundary.
//! 3. A write pass writes them in dependency order. `omeka-db` assigns the target its own ids, so
//!    every reference is remapped through an id map, including the page ids inside a site's
//!    navigation tree.
//!
//! `--dry-run` stops after the mapping and reports what a run would write, with the target
//! untouched.
//!
//! What does not move is listed in `README.md`. The largest losses are upstream ids (the target
//! gets its own), `modified` timestamps (a repository create writes `created` into both), assets
//! (no `asset` repository), value annotations (no repository read) and `site_block_attachment`
//! (no repository).
//!
//! Runtime `sqlx` queries only, and only through `omeka-db`'s repositories. The one query this
//! crate writes itself reads `INFORMATION_SCHEMA.COLUMNS` from the source, which is schema
//! introspection of a foreign database, not port storage SQL.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod columns;
mod error;
mod map;
mod read;
mod report;
mod write;

pub use error::MigrateError;
pub use report::{MigrateOptions, MigrationReport, TableReport};

use std::path::Path;

/// Read a live upstream Omeka S database and write the port's storage.
///
/// `source_url` is a MySQL URL naming the source database
/// (`mysql://user:password@host:port/omeka`). `target_path` is the SQLite file to create; a file
/// that already holds rows is refused. With [`MigrateOptions::dry_run`] set, `target_path` is not
/// opened and nothing is written.
///
/// Returns a per-table [`MigrationReport`] of rows read, written and skipped.
pub async fn migrate_from_upstream(
    source_url: &str,
    target_path: &Path,
    options: &MigrateOptions,
) -> Result<MigrationReport, MigrateError> {
    let source = omeka_db::open_mysql(source_url).await?;
    columns::check_columns(&source).await?;
    let data = read::read_source(&source).await?;

    if options.dry_run {
        return write::Writer::dry().run(&data).await;
    }

    let target = omeka_db::open_file(target_path).await?;
    omeka_db::migrate(&target).await?;
    write::ensure_empty(&target, target_path).await?;
    write::Writer::live(&target).run(&data).await
}
