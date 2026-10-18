//! The migration error type.

use std::path::PathBuf;

/// Everything that can stop a migration.
///
/// The variants separate the three boundaries a run crosses: the source server, the source schema,
/// and the target file. A caller can match on the shape without reading a message.
#[derive(Debug, thiserror::Error)]
pub enum MigrateError {
    /// The source server, its schema, or a read through `omeka-db` failed.
    #[error("the source database: {0}")]
    Source(#[from] omeka_db::DbError),
    /// Reading the source's own column list failed.
    #[error("reading the source schema: {0}")]
    Schema(#[from] sqlx::Error),
    /// The source predates the port's schema: a column the port reads is not there.
    ///
    /// Upstream's install schema is the frozen release, and the port does not replay upstream's
    /// `application/data/migrations/`, so a source older than the frozen SHA keeps its older
    /// columns. The migrator refuses it by name rather than migrating a subset silently.
    #[error(
        "the source is missing column `{table}.{column}`; it predates the port's schema, so migrate upstream first"
    )]
    MissingColumn { table: String, column: String },
    /// The target already holds rows. Writing into it would collide with the unique keys.
    #[error("the target `{path}` already holds `{table}` rows; migrate into an empty database")]
    TargetNotEmpty { path: PathBuf, table: &'static str },
    /// Copying an asset file between the two stores failed.
    #[error("the file store: {0}")]
    Store(#[from] omeka_media::store::StoreError),
    /// The source store has no file for an asset row.
    #[error("the source store has no file for asset `{storage_id}` at `{path}`")]
    MissingAssetFile { storage_id: String, path: PathBuf },
}
