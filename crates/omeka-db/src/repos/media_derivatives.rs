//! `romeka_media_derivative`: the queue a derivative worker drains.
//!
//! A row is queued by the media write that needs it (`omeka_api::derivatives`), claimed by one
//! worker, and settled by the same worker. Every transition is a conditional `UPDATE`, so two
//! workers cannot both own a row and a settled row is never reopened by a late claim. The claim
//! also counts `attempts`, so a row that fails twice is visible without the job log.
//!
//! An `access_copy` row is written by the create with its decision already in it. A worker claims
//! one whose tool is among the commands its install has (`Tools::derivative_commands`), so two
//! workers on one queue, a main image with `exiftool` and a tools image with `vips`, each claim the
//! rows they can make. A row waiting for a command no current worker has stays queued.
//!
//! A media delete cascades from `media`, so no path here removes a row.

use sqlx::{ColumnIndex, Decode, Encode, Executor, IntoArguments, Pool, Sqlite, Type};

use crate::DbError;
use crate::backend::Backend;
use crate::ids::MediaId;
use crate::rows::{DerivativeKind, DerivativeRow, DerivativeState, NewDerivative};
use crate::time;

/// `romeka_media_derivative` rows.
pub struct MediaDerivativeRepo<'a, DB: Backend = Sqlite> {
    pool: &'a Pool<DB>,
}

impl<'a, DB: Backend> MediaDerivativeRepo<'a, DB>
where
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
    usize: ColumnIndex<DB::Row>,
    for<'x> i64: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Option<i64>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> chrono::NaiveDateTime: Encode<'x, DB> + Type<DB>,
    for<'x> Option<chrono::NaiveDateTime>: Encode<'x, DB> + Type<DB>,
    for<'x> String: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Option<String>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> &'x str: Encode<'x, DB> + Type<DB> + ColumnIndex<DB::Row>,
{
    pub fn new(pool: &'a Pool<DB>) -> Self {
        Self { pool }
    }

    /// Queue one derivative. `false` when a row already exists for the pair, whatever its state:
    /// a create never overwrites the state an earlier attempt reached.
    pub async fn enqueue(&self, new: &NewDerivative) -> Result<bool, DbError> {
        self.plan(new, DerivativeState::Pending, None, None).await
    }

    /// Record one create-time decision: a row that is already `pending` for an access copy, or
    /// already settled. A preview kind is queued with [`Self::enqueue`]. `tool` names the command
    /// the row waits for, and `reason` the words a reader shows when there is nothing to make.
    /// `false` when a row already exists for the pair, as [`Self::enqueue`] answers.
    pub async fn plan(
        &self,
        new: &NewDerivative,
        state: DerivativeState,
        reason: Option<&str>,
        tool: Option<&str>,
    ) -> Result<bool, DbError> {
        let queued = time::now();
        let finished = (!matches!(state, DerivativeState::Pending | DerivativeState::Running))
            .then_some(queued);
        let planned = self
            .insert(new, state, reason, tool, queued, finished)
            .await;
        match planned {
            Ok(()) => Ok(true),
            Err(error) if error.is_unique_violation() => Ok(false),
            Err(error) => Err(error),
        }
    }

    async fn insert(
        &self,
        new: &NewDerivative,
        state: DerivativeState,
        reason: Option<&str>,
        tool: Option<&str>,
        queued: chrono::NaiveDateTime,
        finished: Option<chrono::NaiveDateTime>,
    ) -> Result<(), DbError> {
        sqlx::query::<DB>(
            "INSERT INTO romeka_media_derivative
                 (media_id, kind, state, reason, progress, tool, queued, started, finished, attempts)
             VALUES (?, ?, ?, ?, NULL, ?, ?, NULL, ?, 0)",
        )
        .bind(super::bind_id(new.media_id))
        .bind(new.kind.name())
        .bind(state.name())
        .bind(reason.map(str::to_string))
        .bind(tool.map(str::to_string))
        .bind(queued)
        .bind(finished)
        .execute(self.pool)
        .await?;
        Ok(())
    }

    /// Put a failed or skipped row back in the queue. `false` when the row is ready, pending or
    /// running, which is when there is nothing to retry.
    ///
    /// A preview row's tool is the command that failed, and it is cleared. An access copy's tool is
    /// the command its plan named, which is what a worker claims it by, and it stays: a row settled
    /// `none` because this install lacked the command is retried on the install that has it.
    pub async fn retry(&self, media: MediaId, kind: DerivativeKind) -> Result<bool, DbError> {
        let done = sqlx::query::<DB>(
            "UPDATE romeka_media_derivative
             SET state = 'pending', reason = NULL, progress = NULL,
                 tool = CASE WHEN kind = 'access_copy' THEN tool ELSE NULL END,
                 queued = ?, started = NULL, finished = NULL
             WHERE media_id = ? AND kind = ? AND state IN ('failed', 'none')",
        )
        .bind(time::now())
        .bind(super::bind_id(media))
        .bind(kind.name())
        .execute(self.pool)
        .await?;
        Ok(DB::rows_affected(&done) > 0)
    }

    /// Every row of one media, in kind order.
    pub async fn for_media(&self, media: MediaId) -> Result<Vec<DerivativeRow>, DbError> {
        self.for_media_ids(std::slice::from_ref(&media)).await
    }

    /// Every row of a page of media, in media and kind order.
    pub async fn for_media_ids(&self, media: &[MediaId]) -> Result<Vec<DerivativeRow>, DbError> {
        if media.is_empty() {
            return Ok(Vec::new());
        }
        let mut statement = super::Statement::<DB>::new("SELECT * FROM romeka_media_derivative");
        statement.push(" WHERE media_id IN (");
        for (index, id) in media.iter().enumerate() {
            if index > 0 {
                statement.push(", ");
            }
            statement.bind(super::bind_id(*id))?;
        }
        statement.push(") ORDER BY media_id, kind");
        let (sql, arguments) = statement.into_parts();
        let rows: Vec<DB::Row> = super::fetch_rows(&sql, arguments, self.pool).await?;
        rows.iter().map(DerivativeRow::from_db_row::<DB>).collect()
    }

    /// The oldest queued row this worker can do, claimed for one worker. `None` when nothing is
    /// queued.
    ///
    /// A preview kind is always its own. An `access_copy` row is claimed only when its tool is one
    /// of `commands`, the commands this install has (`Tools::derivative_commands`): `ffmpeg` for an
    /// audio or video copy, and the still-master commands for a JPEG master. Two workers on one
    /// queue therefore claim disjoint sets, and neither settles a row it cannot make.
    ///
    /// The claim is the `UPDATE ... WHERE state = 'pending'`; a candidate another worker took first
    /// leaves zero rows affected, and the next candidate is tried.
    pub async fn claim_next(&self, commands: &[&str]) -> Result<Option<DerivativeRow>, DbError> {
        if commands.is_empty() {
            return Ok(None);
        }
        let mut statement = super::Statement::<DB>::new("SELECT * FROM romeka_media_derivative");
        push_claimable(&mut statement, commands)?;
        statement.push(" ORDER BY queued, media_id, kind LIMIT 8");
        let (sql, arguments) = statement.into_parts();
        let rows: Vec<DB::Row> = super::fetch_rows(&sql, arguments, self.pool).await?;
        for row in &rows {
            let candidate = DerivativeRow::from_db_row::<DB>(row)?;
            if let Some(claimed) = self.claim(&candidate).await? {
                return Ok(Some(claimed));
            }
        }
        Ok(None)
    }

    /// Claim one queued row of one kind. `None` when another worker took it first.
    async fn claim(&self, candidate: &DerivativeRow) -> Result<Option<DerivativeRow>, DbError> {
        let claimed = sqlx::query::<DB>(
            "UPDATE romeka_media_derivative
             SET state = 'running', started = ?, progress = 0, attempts = attempts + 1
             WHERE media_id = ? AND kind = ? AND state = 'pending'",
        )
        .bind(time::now())
        .bind(super::bind_id(candidate.media_id))
        .bind(candidate.kind.name())
        .execute(self.pool)
        .await?;
        if DB::rows_affected(&claimed) == 0 {
            return Ok(None);
        }
        self.get(candidate.media_id, candidate.kind).await
    }

    /// One row, or `None` when it does not exist.
    pub async fn get(
        &self,
        media: MediaId,
        kind: DerivativeKind,
    ) -> Result<Option<DerivativeRow>, DbError> {
        let row = sqlx::query::<DB>(
            "SELECT * FROM romeka_media_derivative WHERE media_id = ? AND kind = ?",
        )
        .bind(super::bind_id(media))
        .bind(kind.name())
        .fetch_optional(self.pool)
        .await?;
        row.as_ref()
            .map(DerivativeRow::from_db_row::<DB>)
            .transpose()
    }

    /// Record how far a long derivative has come. `false` when the row is no longer running.
    pub async fn set_progress(
        &self,
        media: MediaId,
        kind: DerivativeKind,
        progress: u8,
    ) -> Result<bool, DbError> {
        let done = sqlx::query::<DB>(
            "UPDATE romeka_media_derivative SET progress = ?
             WHERE media_id = ? AND kind = ? AND state = 'running'",
        )
        .bind(i64::from(progress.min(100)))
        .bind(super::bind_id(media))
        .bind(kind.name())
        .execute(self.pool)
        .await?;
        Ok(DB::rows_affected(&done) > 0)
    }

    /// Settle a running row. `ready` marks progress complete; every other state keeps the last
    /// progress and records `reason` and `tool`. `false` when the row was not running.
    pub async fn finish(
        &self,
        media: MediaId,
        kind: DerivativeKind,
        state: DerivativeState,
        reason: Option<String>,
        tool: Option<String>,
    ) -> Result<bool, DbError> {
        let progress: Option<i64> = match state {
            DerivativeState::Ready => Some(100),
            _ => None,
        };
        let done = sqlx::query::<DB>(
            "UPDATE romeka_media_derivative
             SET state = ?, reason = ?, tool = ?, finished = ?,
                 progress = COALESCE(?, progress)
             WHERE media_id = ? AND kind = ? AND state = 'running'",
        )
        .bind(state.name())
        .bind(reason)
        .bind(tool)
        .bind(time::now())
        .bind(progress)
        .bind(super::bind_id(media))
        .bind(kind.name())
        .execute(self.pool)
        .await?;
        Ok(DB::rows_affected(&done) > 0)
    }

    /// How many rows a worker with `commands` can claim: every preview row, and an `access_copy`
    /// row whose tool is one of them. A row waiting for a command this install does not have is not
    /// counted, because starting the worker for it would only have it leave the row alone.
    pub async fn pending(&self, commands: &[&str]) -> Result<u64, DbError> {
        if commands.is_empty() {
            return Ok(0);
        }
        let mut statement =
            super::Statement::<DB>::new("SELECT count(*) FROM romeka_media_derivative");
        push_claimable(&mut statement, commands)?;
        let (sql, arguments) = statement.into_parts();
        let count: i64 = sqlx::query_scalar_with::<DB, i64, _>(&sql, arguments)
            .fetch_one(self.pool)
            .await?;
        Ok(count.max(0) as u64)
    }

    /// Put a running row back in the queue, as a worker that stopped a long
    /// derivative does: nothing it wrote is kept and the next worker starts
    /// again. `false` when the row was not running.
    pub async fn requeue(&self, media: MediaId, kind: DerivativeKind) -> Result<bool, DbError> {
        let done = sqlx::query::<DB>(
            "UPDATE romeka_media_derivative
             SET state = 'pending', progress = NULL, started = NULL
             WHERE media_id = ? AND kind = ? AND state = 'running'",
        )
        .bind(super::bind_id(media))
        .bind(kind.name())
        .execute(self.pool)
        .await?;
        Ok(DB::rows_affected(&done) > 0)
    }
}

/// The `WHERE` a derivative worker claims by: every preview row, and an
/// `access_copy` row whose command is one of `commands`.
fn push_claimable<'q, DB: Backend>(
    statement: &mut super::Statement<'q, DB>,
    commands: &[&str],
) -> Result<(), DbError>
where
    for<'x> String: Encode<'x, DB> + Type<DB>,
{
    statement.push(" WHERE state = 'pending' AND (kind <> 'access_copy' OR tool IN (");
    for (index, command) in commands.iter().enumerate() {
        if index > 0 {
            statement.push(", ");
        }
        statement.bind((*command).to_string())?;
    }
    statement.push("))");
    Ok(())
}
