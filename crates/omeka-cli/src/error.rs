//! The command-line error type.

/// Everything a command can fail with. Repository and migration failures arrive
/// as [`omeka_db::DbError`]; anything else is a message written for the operator.
#[derive(Debug, thiserror::Error)]
pub enum CliError {
    #[error(transparent)]
    Db(#[from] omeka_db::DbError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Api(#[from] omeka_api::errors::ApiError),
    #[error(transparent)]
    Password(#[from] omeka_api::password::HashError),
    #[error(transparent)]
    Store(#[from] omeka_media::store::StoreError),
    #[error(transparent)]
    Tar(#[from] crate::tar::TarError),
    #[error(transparent)]
    Job(#[from] crate::jobs::JobError),
    /// A malformed input file, reported with the 1-based line it broke on.
    #[error("line {line}: {message}")]
    Parse { line: usize, message: String },
    /// A command-level failure described in plain words.
    #[error("{0}")]
    Message(String),
}

impl CliError {
    /// Build a [`CliError::Message`] from anything displayable.
    pub fn message(text: impl Into<String>) -> Self {
        Self::Message(text.into())
    }
}
