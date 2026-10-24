//! `romeka_bulk` and `romeka_bulk_target`: a bulk run's life and the reads around it.
//!
//! A run is written whole at preview, its selection frozen into one target row per item. A start
//! moves the state with one guarded statement, so two starts cannot both win. The job proves it
//! is alive with [`BulkRepo::pulse`], which also hands it the state a stop request left; a run
//! whose heartbeat is older than [`DEAD_AFTER`] is settled by [`BulkRepo::settle_dead`]. The
//! per-item transactions live in `bulk_writes`.

use std::collections::{BTreeMap, BTreeSet};

use chrono::NaiveDateTime;
use sqlx::{ColumnIndex, Decode, Encode, Executor, IntoArguments, Pool, Sqlite, Type};

use super::{DEAD_AFTER, Presence, Statement, bind_id, insert_id, json_text, new_id};
use crate::DbError;
use crate::backend::Backend;
use crate::ids::{BulkId, ItemId, JobId, UserId};
use crate::rows::{
    BulkEnding, BulkPhase, BulkRow, BulkStatus, BulkTargetRow, Described, NewBulk, TargetCounts,
    TargetState, resource_type,
};
use crate::time;

/// Rows per insert or `IN (...)` list.
const CHUNK: usize = 500;

/// A target an undo would deal with, and what its item looks like now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UndoCandidate {
    pub item: ItemId,
    pub state: TargetState,
    pub written: Option<NaiveDateTime>,
    pub now: Presence,
}

/// An item's describing fields, with its title for a preview line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DescribedItem {
    pub item: ItemId,
    pub title: Option<String>,
    pub fields: Described,
}

/// `romeka_bulk` rows and their targets.
pub struct BulkRepo<'a, DB: Backend = Sqlite> {
    pub(super) pool: &'a Pool<DB>,
}

impl<'a, DB: Backend> BulkRepo<'a, DB>
where
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
    usize: ColumnIndex<DB::Row>,
    for<'x> i64: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Option<i64>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> bool: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> String: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Option<String>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Vec<u8>: Decode<'x, DB> + Type<DB>,
    for<'x> Option<Vec<u8>>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> &'x str: Encode<'x, DB> + Type<DB> + ColumnIndex<DB::Row>,
    for<'x> f64: Decode<'x, DB> + Type<DB>,
{
    pub fn new(pool: &'a Pool<DB>) -> Self {
        Self { pool }
    }

    /// Write a previewed run and one pending target per item, in one transaction.
    pub async fn create(&self, new: &NewBulk) -> Result<BulkRow, DbError> {
        let created = time::now();
        let total = new.items.len() as u64;
        let mut tx = self.pool.begin().await?;
        let query = sqlx::query(
            "INSERT INTO romeka_bulk (owner_id, kind, query, change_json, status, total, created)
             VALUES (?, ?, ?, ?, 'previewed', ?, ?)",
        )
        .bind(super::bind_opt_id(new.owner_id))
        .bind(new.kind.as_str())
        .bind(json_text(&new.query)?)
        .bind(json_text(&new.change)?)
        .bind(total as i64)
        .bind(time::format(created));
        let id = BulkId(insert_id(query, &mut *tx).await?);
        for chunk in new.items.chunks(CHUNK) {
            let mut statement = Statement::<DB>::new(
                "INSERT INTO romeka_bulk_target (bulk_id, item_id, state) VALUES ",
            );
            for (index, item) in chunk.iter().enumerate() {
                if index > 0 {
                    statement.push(", ");
                }
                statement.push("(");
                statement.bind(bind_id(id))?;
                statement.push(", ");
                statement.bind(bind_id(*item))?;
                statement.push(", 'pending')");
            }
            let (sql, arguments) = statement.into_parts();
            sqlx::query_with::<DB, _>(&sql, arguments)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(BulkRow {
            id,
            owner_id: new.owner_id,
            kind: new.kind,
            query: new.query.clone(),
            change: new.change.clone(),
            status: BulkStatus::Previewed,
            total,
            done: 0,
            failed: 0,
            job_id: None,
            error: None,
            created,
            started: None,
            ended: None,
            heartbeat: None,
        })
    }

    pub async fn get(&self, id: BulkId) -> Result<Option<BulkRow>, DbError> {
        let row = sqlx::query("SELECT * FROM romeka_bulk WHERE id = ?")
            .bind(bind_id(id))
            .fetch_optional(self.pool)
            .await?;
        row.as_ref().map(BulkRow::from_db_row::<DB>).transpose()
    }

    /// The newest runs first; those of `owner` only, or everyone's for `None`.
    pub async fn recent(&self, owner: Option<UserId>, limit: i64) -> Result<Vec<BulkRow>, DbError> {
        let rows = match owner {
            Some(owner) => {
                sqlx::query("SELECT * FROM romeka_bulk WHERE owner_id = ? ORDER BY id DESC LIMIT ?")
                    .bind(bind_id(owner))
                    .bind(limit)
                    .fetch_all(self.pool)
                    .await?
            }
            None => {
                sqlx::query("SELECT * FROM romeka_bulk ORDER BY id DESC LIMIT ?")
                    .bind(limit)
                    .fetch_all(self.pool)
                    .await?
            }
        };
        rows.iter().map(BulkRow::from_db_row::<DB>).collect()
    }

    /// Move the run into `phase` when its state allows it, with a fresh heartbeat and no job
    /// yet. False when another start won, or the state does not allow it.
    pub async fn start(&self, id: BulkId, phase: BulkPhase) -> Result<bool, DbError> {
        let from = BulkStatus::ALL.iter().filter(|status| match phase {
            BulkPhase::Run => status.may_run(),
            BulkPhase::Undo => status.may_undo(),
        });
        let now = time::format(time::now());
        let mut statement = Statement::<DB>::new("UPDATE romeka_bulk SET status = ");
        statement.bind(phase.working().as_str())?;
        statement.push(", heartbeat = ");
        statement.bind(now.clone())?;
        statement.push(", started = COALESCE(started, ");
        statement.bind(now)?;
        statement.push("), ended = NULL, error = NULL, job_id = NULL WHERE id = ");
        statement.bind(bind_id(id))?;
        push_names(
            &mut statement,
            " AND status IN (",
            from.map(|status| status.as_str()),
        )?;
        let (sql, arguments) = statement.into_parts();
        let done = sqlx::query_with::<DB, _>(&sql, arguments)
            .execute(self.pool)
            .await?;
        Ok(DB::rows_affected(&done) > 0)
    }

    pub async fn set_job(&self, id: BulkId, job: JobId) -> Result<(), DbError> {
        sqlx::query("UPDATE romeka_bulk SET job_id = ? WHERE id = ?")
            .bind(bind_id(job))
            .bind(bind_id(id))
            .execute(self.pool)
            .await?;
        Ok(())
    }

    /// Ask the job to stop after its chunk. False when nothing works on the run.
    pub async fn request_stop(&self, id: BulkId) -> Result<bool, DbError> {
        let done = sqlx::query(
            "UPDATE romeka_bulk
             SET status = CASE status WHEN 'running' THEN 'stopping' ELSE 'undo_stopping' END
             WHERE id = ? AND status IN ('running', 'undoing')",
        )
        .bind(bind_id(id))
        .execute(self.pool)
        .await?;
        Ok(DB::rows_affected(&done) > 0)
    }

    /// Write the heartbeat and the counts of `job`, the worker of `id`, and answer the run's
    /// state, a stop request's included. `None` when the run no longer works under this job.
    pub async fn pulse(
        &self,
        id: BulkId,
        job: JobId,
        phase: BulkPhase,
        counts: &TargetCounts,
    ) -> Result<Option<BulkStatus>, DbError> {
        let mut statement = Statement::<DB>::new("UPDATE romeka_bulk SET heartbeat = ");
        statement.bind(time::format(time::now()))?;
        statement.push(", done = ");
        statement.bind((counts.done + counts.unchanged) as i64)?;
        statement.push(", failed = ");
        statement.bind(counts.failed as i64)?;
        statement.push(" WHERE id = ");
        statement.bind(bind_id(id))?;
        statement.push(" AND job_id = ");
        statement.bind(bind_id(job))?;
        push_names(
            &mut statement,
            " AND status IN (",
            phase.live().map(BulkStatus::as_str),
        )?;
        let (sql, arguments) = statement.into_parts();
        let done = sqlx::query_with::<DB, _>(&sql, arguments)
            .execute(self.pool)
            .await?;
        if DB::rows_affected(&done) == 0 {
            return Ok(None);
        }
        Ok(self.get(id).await?.map(|row| row.status))
    }

    /// End the run as `ending` says. False when the run no longer works under this job.
    pub async fn finish(
        &self,
        id: BulkId,
        job: JobId,
        ending: BulkEnding,
        error: Option<&str>,
    ) -> Result<bool, DbError> {
        let mut statement = Statement::<DB>::new("UPDATE romeka_bulk SET status = ");
        statement.bind(ending.status().as_str())?;
        statement.push(", ended = ");
        statement.bind(time::format(time::now()))?;
        statement.push(", error = ");
        statement.bind(error.map(str::to_string))?;
        statement.push(" WHERE id = ");
        statement.bind(bind_id(id))?;
        statement.push(" AND job_id = ");
        statement.bind(bind_id(job))?;
        push_names(
            &mut statement,
            " AND status IN (",
            ending.phase().live().map(BulkStatus::as_str),
        )?;
        let (sql, arguments) = statement.into_parts();
        let done = sqlx::query_with::<DB, _>(&sql, arguments)
            .execute(self.pool)
            .await?;
        Ok(DB::rows_affected(&done) > 0)
    }

    /// Move every live run whose heartbeat is older than [`DEAD_AFTER`] to the state a dead
    /// worker leaves. Each move is guarded by the heartbeat it judged, so a worker that beats in
    /// between keeps its run. Answers the runs it moved.
    pub async fn settle_dead(&self) -> Result<Vec<BulkId>, DbError> {
        let now = time::now();
        let mut statement = Statement::<DB>::new("SELECT * FROM romeka_bulk");
        push_names(
            &mut statement,
            " WHERE status IN (",
            BulkStatus::LIVE.map(BulkStatus::as_str),
        )?;
        let (sql, arguments) = statement.into_parts();
        let rows = sqlx::query_with::<DB, _>(&sql, arguments)
            .fetch_all(self.pool)
            .await?;
        let mut settled = Vec::new();
        for row in rows.iter().map(BulkRow::from_db_row::<DB>) {
            let row = row?;
            let dead = row.heartbeat.is_none_or(|beat| now - beat > DEAD_AFTER);
            let Some(end) = row.status.when_dead().filter(|_| dead) else {
                continue;
            };
            let done = sqlx::query(
                "UPDATE romeka_bulk SET status = ?, ended = ?
                 WHERE id = ? AND status = ? AND (heartbeat = ? OR heartbeat IS NULL)",
            )
            .bind(end.as_str())
            .bind(time::format(now))
            .bind(bind_id(row.id))
            .bind(row.status.as_str())
            .bind(row.heartbeat.map(time::format))
            .execute(self.pool)
            .await?;
            if DB::rows_affected(&done) > 0 {
                settled.push(row.id);
            }
        }
        Ok(settled)
    }

    /// The next `limit` items after `after` that `phase` still has work on, in id order.
    pub async fn open_targets(
        &self,
        id: BulkId,
        phase: BulkPhase,
        after: Option<ItemId>,
        limit: i64,
    ) -> Result<Vec<ItemId>, DbError> {
        let mut statement =
            Statement::<DB>::new("SELECT item_id FROM romeka_bulk_target WHERE bulk_id = ");
        statement.bind(bind_id(id))?;
        statement.push(" AND item_id > ");
        statement.bind(after.map_or(0, bind_id))?;
        push_names(
            &mut statement,
            " AND state IN (",
            phase.open_targets().iter().map(|state| state.as_str()),
        )?;
        statement.push(" ORDER BY item_id LIMIT ");
        statement.bind(limit)?;
        let (sql, arguments) = statement.into_parts();
        let raw: Vec<i64> = sqlx::query_scalar_with::<DB, i64, _>(&sql, arguments)
            .fetch_all(self.pool)
            .await?;
        raw.into_iter()
            .map(|raw| Ok(ItemId(new_id(raw)?)))
            .collect()
    }

    pub async fn counts(&self, id: BulkId) -> Result<TargetCounts, DbError> {
        let rows = sqlx::query(
            "SELECT state, COUNT(*) AS n FROM romeka_bulk_target WHERE bulk_id = ? GROUP BY state",
        )
        .bind(bind_id(id))
        .fetch_all(self.pool)
        .await?;
        let mut counts = TargetCounts::default();
        for row in &rows {
            let state = TargetState::parse(&DB::text(row, "state")?)?;
            counts.add(state, DB::id(row, "n")?);
        }
        Ok(counts)
    }

    /// The first `limit` targets in `state`, in id order.
    pub async fn targets(
        &self,
        id: BulkId,
        state: TargetState,
        limit: i64,
    ) -> Result<Vec<BulkTargetRow>, DbError> {
        let rows = sqlx::query(
            "SELECT * FROM romeka_bulk_target WHERE bulk_id = ? AND state = ?
             ORDER BY item_id LIMIT ?",
        )
        .bind(bind_id(id))
        .bind(state.as_str())
        .bind(limit)
        .fetch_all(self.pool)
        .await?;
        rows.iter().map(BulkTargetRow::from_db_row::<DB>).collect()
    }

    /// Every target an undo would deal with, with its item's state now.
    pub async fn undo_candidates(&self, id: BulkId) -> Result<Vec<UndoCandidate>, DbError> {
        let rows = sqlx::query(
            "SELECT t.item_id, t.state, t.written, r.modified, r.resource_type
             FROM romeka_bulk_target t LEFT JOIN resource r ON r.id = t.item_id
             WHERE t.bulk_id = ? AND t.state IN ('done', 'kept')
             ORDER BY t.item_id",
        )
        .bind(bind_id(id))
        .fetch_all(self.pool)
        .await?;
        rows.iter()
            .map(|row| {
                let here =
                    DB::opt_text(row, "resource_type")?.as_deref() == Some(resource_type::ITEM);
                Ok(UndoCandidate {
                    item: DB::id(row, "item_id")?,
                    state: TargetState::parse(&DB::text(row, "state")?)?,
                    written: DB::opt_datetime(row, "written")?,
                    now: if here {
                        Presence::Here {
                            modified: DB::opt_datetime(row, "modified")?,
                        }
                    } else {
                        Presence::Gone
                    },
                })
            })
            .collect()
    }

    /// The describing fields of `items` that still exist, in id order.
    pub async fn described(&self, items: &[ItemId]) -> Result<Vec<DescribedItem>, DbError> {
        let mut found = Vec::new();
        for chunk in items.chunks(CHUNK) {
            let mut statement = Statement::<DB>::new(
                "SELECT r.id, r.title, r.is_public, r.owner_id, r.resource_class_id,
                        r.resource_template_id
                 FROM resource r JOIN item i ON i.id = r.id",
            );
            push_ids(&mut statement, " WHERE r.id IN (", chunk)?;
            statement.push(" ORDER BY r.id");
            let (sql, arguments) = statement.into_parts();
            let rows = sqlx::query_with::<DB, _>(&sql, arguments)
                .fetch_all(self.pool)
                .await?;
            let sets = self.members(chunk, "item_item_set", "item_set_id").await?;
            let sites = self.members(chunk, "item_site", "site_id").await?;
            for row in &rows {
                let item: ItemId = DB::id(row, "id")?;
                found.push(DescribedItem {
                    item,
                    title: DB::opt_text(row, "title")?,
                    fields: Described {
                        is_public: DB::boolean(row, "is_public")?,
                        owner: DB::opt_id(row, "owner_id")?,
                        class: DB::opt_id(row, "resource_class_id")?,
                        template: DB::opt_id(row, "resource_template_id")?,
                        item_sets: members_of(&sets, item),
                        sites: members_of(&sites, item),
                    },
                });
            }
        }
        Ok(found)
    }

    /// `(item_id, member)` of a membership table, for the items of one chunk.
    async fn members(
        &self,
        items: &[ItemId],
        table: &'static str,
        member: &'static str,
    ) -> Result<BTreeMap<ItemId, Vec<u64>>, DbError> {
        let mut statement =
            Statement::<DB>::new(&format!("SELECT item_id, {member} AS member FROM {table}"));
        push_ids(&mut statement, " WHERE item_id IN (", items)?;
        let (sql, arguments) = statement.into_parts();
        let rows = sqlx::query_with::<DB, _>(&sql, arguments)
            .fetch_all(self.pool)
            .await?;
        let mut members: BTreeMap<ItemId, Vec<u64>> = BTreeMap::new();
        for row in &rows {
            members
                .entry(DB::id(row, "item_id")?)
                .or_default()
                .push(DB::id(row, "member")?);
        }
        Ok(members)
    }
}

fn members_of<T: From<u64> + Ord>(
    members: &BTreeMap<ItemId, Vec<u64>>,
    item: ItemId,
) -> BTreeSet<T> {
    members
        .get(&item)
        .into_iter()
        .flatten()
        .map(|id| T::from(*id))
        .collect()
}

/// ` IN (a, b, c)` after `open`, over stored state names.
fn push_names<DB: Backend>(
    statement: &mut Statement<'_, DB>,
    open: &str,
    names: impl IntoIterator<Item = &'static str>,
) -> Result<(), DbError>
where
    for<'x> &'x str: Encode<'x, DB> + Type<DB>,
{
    statement.push(open);
    for (index, name) in names.into_iter().enumerate() {
        if index > 0 {
            statement.push(", ");
        }
        statement.bind(name)?;
    }
    statement.push(")");
    Ok(())
}

/// `open` and an id list closed by `)`.
fn push_ids<DB: Backend>(
    statement: &mut Statement<'_, DB>,
    open: &str,
    items: &[ItemId],
) -> Result<(), DbError>
where
    for<'x> i64: Encode<'x, DB> + Type<DB>,
{
    statement.push(open);
    for (index, item) in items.iter().enumerate() {
        if index > 0 {
            statement.push(", ");
        }
        statement.bind(bind_id(*item))?;
    }
    statement.push(")");
    Ok(())
}
