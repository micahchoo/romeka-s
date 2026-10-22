//! The steps before a run starts, shared by the run route and `romeka import
//! run` (`docs/import-v2.md`, "Lifecycle and the ledger", "Command line").
//!
//! [`assess`] rescans the sources, derives the model and checks it. [`begin`]
//! refuses unless the checked digest is the one the caller saw, no blocking
//! flag stands, something would be written, and a delete is confirmed with its
//! count; then it moves the import to `running` under the revision the
//! derive read. The route and the command differ only in where the digest
//! comes from: the admin saw one in Check, the command has none to compare.

use std::sync::Arc;

use omeka_db::Backend;
use omeka_db::DbError;
use omeka_db::ids::ImportId;
use omeka_db::repos::{ImportRepo, StartOutcome};
use omeka_db::rows::{ImportRow, ImportStatus};
use omeka_media::store::Store;
use omeka_media::validation::FileValidation;
use sqlx::{ColumnIndex, Decode, Encode, Executor, IntoArguments, Pool, Type};

use super::archive::ArchiveRoots;
use super::catalog_load::Owners;
use super::check::{Check, check};
use super::model::Flag;
use super::plan::{Action, ImportPlan};
use super::preview::{Caches, Derived, PreviewError, derive_import};
use super::scan::{self, ScanError};

/// A plan derived against a fresh index, and what Check says of it.
pub struct Assessed {
    pub derived: Derived,
    pub check: Check,
}

#[derive(Debug, thiserror::Error)]
pub enum AssessError {
    #[error(transparent)]
    Scan(#[from] ScanError),
    #[error(transparent)]
    Derive(#[from] PreviewError),
}

/// What the caller says it expects.
#[derive(Debug, Clone, Copy, Default)]
pub struct Go<'a> {
    /// The digest the admin saw; `None` when the caller derived the model
    /// itself and has nothing older to compare.
    pub digest: Option<&'a str>,
    /// The count of resources a delete removes.
    pub confirm: Option<u64>,
    /// The server goes on with a run whose worker went without the person
    /// asking (`ImportRepo::start_resume`); nobody pressed Run.
    pub resume: bool,
}

/// Why a run did not start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// The model changed since the caller looked.
    DigestChanged {
        digest: String,
    },
    Blocked(Vec<Flag>),
    Nothing,
    Confirm {
        deletes: usize,
    },
    Missing,
    /// The import's state does not allow a run.
    CannotRun(ImportStatus),
    Busy(ImportId),
    /// The plan was saved again since the derive read it.
    RevisionChanged {
        revision: u64,
    },
    Deleted,
}

#[derive(Debug, thiserror::Error)]
pub enum StartError {
    #[error("the run did not start: {0:?}")]
    Refused(Refusal),
    #[error(transparent)]
    Db(#[from] DbError),
}

impl From<Refusal> for StartError {
    fn from(refusal: Refusal) -> Self {
        Self::Refused(refusal)
    }
}

/// Rescan `plan`'s sources into `import`'s index, derive, and check.
///
/// # Errors
/// [`AssessError`] when the index cannot be written or read.
#[allow(clippy::too_many_arguments)]
pub async fn assess<DB>(
    pool: &Pool<DB>,
    roots: &ArchiveRoots,
    store: &Arc<dyn Store>,
    caches: &Arc<Caches>,
    files: FileValidation,
    owners: Owners,
    import: ImportId,
    plan: ImportPlan,
) -> Result<Assessed, AssessError>
where
    DB: Backend,
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
    usize: ColumnIndex<DB::Row>,
    for<'x> bool: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> f64: Decode<'x, DB> + Type<DB>,
    for<'x> i64: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Option<i64>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> String: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Option<String>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Vec<u8>: Decode<'x, DB> + Type<DB>,
    for<'x> Option<Vec<u8>>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> &'x str: Encode<'x, DB> + Type<DB> + ColumnIndex<DB::Row>,
{
    scan::scan(pool, roots, store, import, &plan.sources).await?;
    let derived = derive_import(
        pool,
        roots,
        store,
        caches,
        import,
        Arc::new(plan),
        owners,
        files,
    )
    .await?;
    let found = check(&derived.plan, &derived.model);
    Ok(Assessed {
        derived,
        check: found,
    })
}

/// Start the run of `row` if `found` allows it.
///
/// # Errors
/// [`StartError::Refused`] with the first refusal, in the order the contract
/// lists them; [`StartError::Db`] when the start transaction fails.
pub async fn begin<DB>(
    pool: &Pool<DB>,
    row: &ImportRow,
    found: &Check,
    go: Go<'_>,
) -> Result<(), StartError>
where
    DB: Backend,
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
    usize: ColumnIndex<DB::Row>,
    for<'x> bool: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> f64: Decode<'x, DB> + Type<DB>,
    for<'x> i64: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Option<i64>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> String: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Option<String>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Vec<u8>: Decode<'x, DB> + Type<DB>,
    for<'x> Option<Vec<u8>>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> &'x str: Encode<'x, DB> + Type<DB> + ColumnIndex<DB::Row>,
{
    if go.digest.is_some_and(|seen| seen != found.digest) {
        return Err(Refusal::DigestChanged {
            digest: found.digest.clone(),
        }
        .into());
    }
    if !found.blocking.is_empty() {
        return Err(Refusal::Blocked(found.blocking.clone()).into());
    }
    if !found.can_run {
        return Err(Refusal::Nothing.into());
    }
    if found.action == Action::Delete && go.confirm != Some(found.deletes as u64) {
        return Err(Refusal::Confirm {
            deletes: found.deletes,
        }
        .into());
    }
    let imports = ImportRepo::new(pool);
    let outcome = if go.resume {
        imports.start_resume(row.id, row.revision).await?
    } else {
        imports.start_run(row.id, row.revision).await?
    };
    match outcome {
        StartOutcome::Started => Ok(()),
        StartOutcome::Missing => Err(Refusal::Missing.into()),
        StartOutcome::Refused(status) => Err(Refusal::CannotRun(status).into()),
        StartOutcome::Busy(other) => Err(Refusal::Busy(other).into()),
        StartOutcome::Changed { revision } => Err(Refusal::RevisionChanged { revision }.into()),
        StartOutcome::Deleted => Err(Refusal::Deleted.into()),
    }
}
