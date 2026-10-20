//! The crate error type.

/// Everything that can go wrong between a caller and SQLite.
#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("sqlite error: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("migration error: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),
    /// A migration left this many rows referencing a row that does not exist.
    #[error("migration left {0} broken foreign key references")]
    BrokenReferences(i64),
    /// A stored column holds a value the row type cannot represent. This is the row-parsing
    /// boundary: the database file is external data, so a bad value is an error, not a panic.
    #[error("column `{column}`: {reason}")]
    Column {
        column: &'static str,
        reason: String,
    },
    /// A value could not be encoded for the database before the statement ran.
    #[error("binding a value: {reason}")]
    Bind { reason: String },
    /// The operation has no counterpart on this engine, so the port refuses it rather than
    /// running a partial version.
    #[error("{engine} does not support {operation}: {reason}")]
    Unsupported {
        engine: &'static str,
        operation: &'static str,
        reason: &'static str,
    },
}

impl DbError {
    /// True for a unique-constraint violation (upstream rejects these during validation, so a
    /// caller that sees this raced another writer or skipped validation).
    pub fn is_unique_violation(&self) -> bool {
        matches!(self, Self::Sqlx(sqlx::Error::Database(e)) if e.is_unique_violation())
    }

    /// True for a foreign-key violation, such as deleting a row another table still references.
    pub fn is_foreign_key_violation(&self) -> bool {
        matches!(self, Self::Sqlx(sqlx::Error::Database(e)) if e.is_foreign_key_violation())
    }
}
