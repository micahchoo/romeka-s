//! The backend seam: every dialect difference between SQLite and MySQL, one impl per backend.
//!
//! The repositories are generic over [`Backend`], so one implementation serves both engines. A
//! repository never asks which backend it runs on; it asks the backend for the small pieces that
//! differ. Those pieces are:
//!
//! - the id the last `INSERT` assigned (`rowid` against `LAST_INSERT_ID()`),
//! - the `LIMIT` value that means "no limit" (`-1` against a very large number),
//! - how a column is read. SQLite stores the upstream `datetime` text in a `TEXT` column; MySQL has
//!   a native `datetime`. Every other read goes through sqlx's per-driver `Decode`, which is why
//!   the row layer asks the backend instead of calling `try_get` itself,
//! - the `LIKE ... ESCAPE` literal (SQLite takes a bare backslash, MySQL needs it doubled),
//! - the four upsert statements (`ON CONFLICT` against `ON DUPLICATE KEY UPDATE`, `INSERT OR
//!   IGNORE` against `INSERT IGNORE`),
//! - the full-text query (`MATCH` on the FTS5 mirror against MySQL's `MATCH ... AGAINST`).
//! - how a transaction serializes against other writers (`BEGIN IMMEDIATE` against `FOR UPDATE`).
//!
//! Everything else is shared: table and column names, `?` placeholders, the join and filter shapes
//! of the repositories, and the row structs. `DESIGN.md` fixes the port on one storage engine per
//! deployment, so the repositories stay non-generic at the call site: `Sqlite` is the default type
//! parameter and `ItemRepo::new(&pool)` still names a SQLite repository.
//!
//! Rust does not expose a type parameter's `Encode` and `Decode` impls through a trait bound, so a
//! generic item that binds or reads restates the value bounds it needs, as in
//! `for<'x> i64: Decode<'x, DB> + Encode<'x, DB> + Type<DB>`. `Sqlite` and `MySql` are the only
//! instantiations and both satisfy those bounds, so the restatement is a compile-time obligation
//! and never a runtime branch.

use std::borrow::Cow;

use chrono::NaiveDateTime;
use serde_json::Value;
use sqlx::{ColumnIndex, Database, Decode, Row, Type};

use crate::DbError;

/// One SQL dialect, as the handful of shapes the repositories cannot share.
///
/// The read methods are required rather than provided so that each one decodes through the
/// concrete driver: a provided method with a `Decode` bound would push that bound onto every caller.
pub trait Backend: Database {
    /// The engine name, for errors and the README.
    const NAME: &'static str;

    /// The id the last `INSERT` on this connection assigned.
    ///
    /// SQLite reports the `rowid` it just inserted; MySQL reports `LAST_INSERT_ID()`. Both are
    /// per-connection, so a create that runs inside a transaction reads back its own id.
    fn inserted_id(result: &Self::QueryResult) -> Result<u64, DbError>;

    /// How many rows the last statement changed.
    fn rows_affected(result: &Self::QueryResult) -> u64;

    /// The `LIMIT` value that means "no limit".
    ///
    /// SQLite spells it `-1`. MySQL rejects a negative `LIMIT`, so it gets the largest signed
    /// value a MySQL `LIMIT` accepts.
    fn no_limit() -> i64;

    /// An identifier as this engine needs it written in a statement.
    ///
    /// SQLite accepts the bare name, and so does MySQL except where MariaDB reserves it. MariaDB
    /// reserves `VALUE`, and its parser reads `INSERT INTO value` as the row-list keyword and
    /// rejects the statement with error 1064. The MySQL spelling therefore backticks a name, the
    /// way upstream's own `schema.sql` does. Only the `value` table's `INSERT INTO` position needs
    /// it; every read and update the port writes with the bare name is accepted by MariaDB.
    fn quote_identifier(name: &'static str) -> Cow<'static, str> {
        Cow::Borrowed(name)
    }

    /// A `NOT NULL` character column.
    fn text(row: &Self::Row, column: &'static str) -> Result<String, DbError>;

    /// A nullable character column.
    fn opt_text(row: &Self::Row, column: &'static str) -> Result<Option<String>, DbError>;

    /// A `NOT NULL` integer column that is not an id (`size`, `position`, `session.modified`).
    fn int(row: &Self::Row, column: &'static str) -> Result<i64, DbError>;

    /// A nullable integer column that is not an id.
    fn opt_int(row: &Self::Row, column: &'static str) -> Result<Option<i64>, DbError>;

    /// A `NOT NULL` boolean column, stored as `tinyint(1)` or SQLite's `INTEGER` 0/1.
    fn boolean(row: &Self::Row, column: &'static str) -> Result<bool, DbError>;

    /// A `NOT NULL` blob (`session.data`).
    fn blob(row: &Self::Row, column: &'static str) -> Result<Vec<u8>, DbError>;

    /// A nullable blob (`api_key.last_ip`).
    fn opt_blob(row: &Self::Row, column: &'static str) -> Result<Option<Vec<u8>>, DbError>;

    /// A `NOT NULL` `datetime` column as upstream stores it.
    fn datetime(row: &Self::Row, column: &'static str) -> Result<NaiveDateTime, DbError>;

    /// A nullable `datetime` column.
    fn opt_datetime(
        row: &Self::Row,
        column: &'static str,
    ) -> Result<Option<NaiveDateTime>, DbError>;

    /// A `NOT NULL` JSON column.
    fn json(row: &Self::Row, column: &'static str) -> Result<Value, DbError> {
        json_from(&Self::text(row, column)?, column)
    }

    /// A nullable JSON column.
    fn opt_json(row: &Self::Row, column: &'static str) -> Result<Option<Value>, DbError> {
        match Self::opt_text(row, column)? {
            Some(stored) => Ok(Some(json_from(&stored, column)?)),
            None => Ok(None),
        }
    }

    /// A nullable JSON column that upstream types as a JSON array of strings
    /// (`resource_template_property.data_type`).
    fn opt_string_list(
        row: &Self::Row,
        column: &'static str,
    ) -> Result<Option<Vec<String>>, DbError> {
        match Self::opt_json(row, column)? {
            Some(value) => {
                serde_json::from_value(value)
                    .map(Some)
                    .map_err(|error| DbError::Column {
                        column,
                        reason: format!("not a JSON array of strings: {error}"),
                    })
            }
            None => Ok(None),
        }
    }

    /// A `NOT NULL` id column.
    fn id<T: From<u64>>(row: &Self::Row, column: &'static str) -> Result<T, DbError> {
        id_from(Self::int(row, column)?, column)
    }

    /// A nullable id column.
    fn opt_id<T: From<u64>>(row: &Self::Row, column: &'static str) -> Result<Option<T>, DbError> {
        match Self::opt_int(row, column)? {
            Some(raw) => Ok(Some(id_from(raw, column)?)),
            None => Ok(None),
        }
    }

    /// The `LIKE ... ESCAPE` clause, including its leading space.
    fn like_escape() -> &'static str;

    /// `item_item_set`: attach an item to an item set unless the row is already there.
    fn insert_item_item_set() -> &'static str;

    /// `item_site`: attach items to a site unless the row is already there. The text ends at
    /// `VALUES `; the caller appends one `(?, ?)` per row, so one statement can carry a batch.
    fn insert_item_site() -> &'static str;

    /// `site_item_set`: attach an item set to a site, replacing the position on a repeat.
    fn upsert_site_item_set() -> &'static str;

    /// `site_permission`: grant a role, replacing it on a repeat.
    fn upsert_site_permission() -> &'static str;

    /// `fulltext_search`: write the index row for one resource, replacing an existing one.
    fn upsert_fulltext_search() -> &'static str;

    /// `setting`: write the global setting named `id`, replacing an existing value.
    fn upsert_setting() -> &'static str;

    /// `user_setting`: write one user's setting, replacing an existing value.
    fn upsert_user_setting() -> &'static str;

    /// `site_setting`: write one site's setting, replacing an existing value.
    fn upsert_site_setting() -> &'static str;

    /// `session`: write a session row, replacing the data and time on a repeat id.
    fn upsert_session() -> &'static str;

    /// `job`: append a line to the log. SQLite joins strings with `||`; MySQL's `||` is a logical
    /// OR, so it needs `CONCAT`.
    fn append_job_log() -> &'static str;

    /// The statement that opens a transaction which must not interleave with another writer.
    ///
    /// SQLite's plain `BEGIN` takes the write lock at the first write, so two transactions can
    /// both read and then fail to upgrade with `SQLITE_BUSY`. `BEGIN IMMEDIATE` takes the lock
    /// first and makes the second wait. MySQL begins plainly and locks rows with [`Self::for_update`].
    fn begin_write() -> &'static str;

    /// The suffix that makes a read lock the rows it reads until the transaction ends. SQLite
    /// has no row locks and needs none inside `BEGIN IMMEDIATE`.
    fn for_update() -> &'static str;

    /// The full-text select. Two columns: the resource id and the match score.
    fn fulltext_select() -> &'static str;

    /// The parameter for the full-text match, or `None` when the query holds no word.
    fn fulltext_needle(needle: &str) -> Option<String>;

    /// The score as the port reports it, where higher is a better match.
    fn fulltext_score(raw: f64) -> f64;
}

/// Decode one column, naming it in the error rather than reporting a bare column index.
fn decode<'r, DB, T>(row: &'r DB::Row, column: &'static str) -> Result<T, DbError>
where
    DB: Database,
    T: Decode<'r, DB> + Type<DB>,
    &'r str: ColumnIndex<DB::Row>,
{
    row.try_get(column).map_err(|error| DbError::Column {
        column,
        reason: error.to_string(),
    })
}

fn json_from(stored: &str, column: &'static str) -> Result<Value, DbError> {
    serde_json::from_str(stored).map_err(|error| DbError::Column {
        column,
        reason: format!("`{stored}` is not JSON: {error}"),
    })
}

/// A binary-collated text column as the UTF-8 text it holds.
#[cfg(feature = "mysql")]
fn utf8(bytes: Vec<u8>, column: &'static str) -> Result<String, DbError> {
    String::from_utf8(bytes).map_err(|error| DbError::Column {
        column,
        reason: format!("binary-collated text is not UTF-8: {error}"),
    })
}

fn id_from<T: From<u64>>(raw: i64, column: &'static str) -> Result<T, DbError> {
    u64::try_from(raw)
        .map(T::from)
        .map_err(|_| DbError::Column {
            column,
            reason: format!("id {raw} is negative"),
        })
}

impl Backend for sqlx::Sqlite {
    const NAME: &'static str = "sqlite";

    fn inserted_id(result: &<Self as Database>::QueryResult) -> Result<u64, DbError> {
        let raw = result.last_insert_rowid();
        u64::try_from(raw).map_err(|_| DbError::Column {
            column: "id",
            reason: format!("insert returned rowid {raw}"),
        })
    }

    fn rows_affected(result: &<Self as Database>::QueryResult) -> u64 {
        result.rows_affected()
    }

    fn no_limit() -> i64 {
        -1
    }

    fn text(row: &Self::Row, column: &'static str) -> Result<String, DbError> {
        decode::<Self, String>(row, column)
    }

    fn opt_text(row: &Self::Row, column: &'static str) -> Result<Option<String>, DbError> {
        decode::<Self, Option<String>>(row, column)
    }

    fn int(row: &Self::Row, column: &'static str) -> Result<i64, DbError> {
        decode::<Self, i64>(row, column)
    }

    fn opt_int(row: &Self::Row, column: &'static str) -> Result<Option<i64>, DbError> {
        decode::<Self, Option<i64>>(row, column)
    }

    fn boolean(row: &Self::Row, column: &'static str) -> Result<bool, DbError> {
        decode::<Self, bool>(row, column)
    }

    fn blob(row: &Self::Row, column: &'static str) -> Result<Vec<u8>, DbError> {
        decode::<Self, Vec<u8>>(row, column)
    }

    fn opt_blob(row: &Self::Row, column: &'static str) -> Result<Option<Vec<u8>>, DbError> {
        decode::<Self, Option<Vec<u8>>>(row, column)
    }

    fn datetime(row: &Self::Row, column: &'static str) -> Result<NaiveDateTime, DbError> {
        let stored: String = decode::<Self, String>(row, column)?;
        crate::time::parse(&stored).map_err(|reason| DbError::Column { column, reason })
    }

    fn opt_datetime(
        row: &Self::Row,
        column: &'static str,
    ) -> Result<Option<NaiveDateTime>, DbError> {
        match decode::<Self, Option<String>>(row, column)? {
            Some(stored) => crate::time::parse(&stored)
                .map(Some)
                .map_err(|reason| DbError::Column { column, reason }),
            None => Ok(None),
        }
    }

    fn like_escape() -> &'static str {
        r" ESCAPE '\'"
    }

    fn insert_item_item_set() -> &'static str {
        "INSERT OR IGNORE INTO item_item_set (item_id, item_set_id) VALUES (?, ?)"
    }

    fn insert_item_site() -> &'static str {
        "INSERT OR IGNORE INTO item_site (item_id, site_id) VALUES "
    }

    fn upsert_site_item_set() -> &'static str {
        "INSERT INTO site_item_set (site_id, item_set_id, position) VALUES (?, ?, ?)
         ON CONFLICT (site_id, item_set_id) DO UPDATE SET position = excluded.position"
    }

    fn upsert_site_permission() -> &'static str {
        "INSERT INTO site_permission (site_id, user_id, role) VALUES (?, ?, ?)
         ON CONFLICT (site_id, user_id) DO UPDATE SET role = excluded.role"
    }

    fn upsert_fulltext_search() -> &'static str {
        "INSERT INTO fulltext_search (id, resource, owner_id, is_public, title, text)
         VALUES (?, ?, ?, ?, ?, ?)
         ON CONFLICT(id, resource) DO UPDATE SET
             owner_id = excluded.owner_id,
             is_public = excluded.is_public,
             title = excluded.title,
             text = excluded.text"
    }

    fn upsert_setting() -> &'static str {
        "INSERT INTO setting (id, value) VALUES (?, ?)
         ON CONFLICT (id) DO UPDATE SET value = excluded.value"
    }

    fn upsert_user_setting() -> &'static str {
        "INSERT INTO user_setting (id, user_id, value) VALUES (?, ?, ?)
         ON CONFLICT (id, user_id) DO UPDATE SET value = excluded.value"
    }

    fn upsert_site_setting() -> &'static str {
        "INSERT INTO site_setting (id, site_id, value) VALUES (?, ?, ?)
         ON CONFLICT (id, site_id) DO UPDATE SET value = excluded.value"
    }

    fn upsert_session() -> &'static str {
        "INSERT INTO session (id, modified, data) VALUES (?, ?, ?)
         ON CONFLICT (id) DO UPDATE SET modified = excluded.modified, data = excluded.data"
    }

    fn append_job_log() -> &'static str {
        "UPDATE job SET log = COALESCE(log, '') || ? || char(10) WHERE id = ?"
    }

    fn begin_write() -> &'static str {
        "BEGIN IMMEDIATE"
    }

    fn for_update() -> &'static str {
        ""
    }

    fn fulltext_select() -> &'static str {
        "SELECT fulltext_search.id, bm25(fulltext_search_fts) AS rank
         FROM fulltext_search_fts
         JOIN fulltext_search ON fulltext_search.rowid = fulltext_search_fts.rowid
         WHERE fulltext_search_fts MATCH ? AND fulltext_search.resource = ?
         ORDER BY rank, fulltext_search.id"
    }

    fn fulltext_needle(needle: &str) -> Option<String> {
        crate::repos::match_expression(needle)
    }

    fn fulltext_score(raw: f64) -> f64 {
        -raw
    }
}

#[cfg(feature = "mysql")]
impl Backend for sqlx::MySql {
    const NAME: &'static str = "mysql";

    fn inserted_id(result: &<Self as Database>::QueryResult) -> Result<u64, DbError> {
        Ok(result.last_insert_id())
    }

    fn rows_affected(result: &<Self as Database>::QueryResult) -> u64 {
        result.rows_affected()
    }

    fn no_limit() -> i64 {
        i64::MAX
    }

    fn quote_identifier(name: &'static str) -> Cow<'static, str> {
        Cow::Owned(format!("`{name}`"))
    }

    /// A binary collation makes sqlx report a text column as `VARBINARY`, so the `String` decode is
    /// refused. `password_creation.id`, `property.local_name` and `resource_class.local_name` are
    /// `utf8mb4_bin`, yet their bytes are the UTF-8 text they hold, so decode those bytes instead.
    fn text(row: &Self::Row, column: &'static str) -> Result<String, DbError> {
        match decode::<Self, String>(row, column) {
            Ok(text) => Ok(text),
            Err(text_error) => match decode::<Self, Vec<u8>>(row, column) {
                Ok(bytes) => utf8(bytes, column),
                Err(_) => Err(text_error),
            },
        }
    }

    fn opt_text(row: &Self::Row, column: &'static str) -> Result<Option<String>, DbError> {
        match decode::<Self, Option<String>>(row, column) {
            Ok(text) => Ok(text),
            Err(text_error) => match decode::<Self, Option<Vec<u8>>>(row, column) {
                Ok(Some(bytes)) => utf8(bytes, column).map(Some),
                Ok(None) => Ok(None),
                Err(_) => Err(text_error),
            },
        }
    }

    fn int(row: &Self::Row, column: &'static str) -> Result<i64, DbError> {
        decode::<Self, i64>(row, column)
    }

    fn opt_int(row: &Self::Row, column: &'static str) -> Result<Option<i64>, DbError> {
        decode::<Self, Option<i64>>(row, column)
    }

    fn boolean(row: &Self::Row, column: &'static str) -> Result<bool, DbError> {
        decode::<Self, bool>(row, column)
    }

    fn blob(row: &Self::Row, column: &'static str) -> Result<Vec<u8>, DbError> {
        decode::<Self, Vec<u8>>(row, column)
    }

    fn opt_blob(row: &Self::Row, column: &'static str) -> Result<Option<Vec<u8>>, DbError> {
        decode::<Self, Option<Vec<u8>>>(row, column)
    }

    /// MySQL hands back a `NaiveDateTime`; SQLite hands back the text form. This is the one read
    /// whose representation differs between the engines.
    fn datetime(row: &Self::Row, column: &'static str) -> Result<NaiveDateTime, DbError> {
        decode::<Self, NaiveDateTime>(row, column)
    }

    fn opt_datetime(
        row: &Self::Row,
        column: &'static str,
    ) -> Result<Option<NaiveDateTime>, DbError> {
        decode::<Self, Option<NaiveDateTime>>(row, column)
    }

    /// MySQL's default `sql_mode` reads a backslash inside a string literal as an escape, so the
    /// escape character is written twice here and reaches the server as one backslash.
    fn like_escape() -> &'static str {
        r" ESCAPE '\\'"
    }

    fn insert_item_item_set() -> &'static str {
        "INSERT IGNORE INTO item_item_set (item_id, item_set_id) VALUES (?, ?)"
    }

    fn insert_item_site() -> &'static str {
        "INSERT IGNORE INTO item_site (item_id, site_id) VALUES "
    }

    fn upsert_site_item_set() -> &'static str {
        "INSERT INTO site_item_set (site_id, item_set_id, position) VALUES (?, ?, ?)
         ON DUPLICATE KEY UPDATE position = VALUES(position)"
    }

    fn upsert_site_permission() -> &'static str {
        "INSERT INTO site_permission (site_id, user_id, role) VALUES (?, ?, ?)
         ON DUPLICATE KEY UPDATE role = VALUES(role)"
    }

    fn upsert_fulltext_search() -> &'static str {
        "INSERT INTO fulltext_search (id, resource, owner_id, is_public, title, text)
         VALUES (?, ?, ?, ?, ?, ?)
         ON DUPLICATE KEY UPDATE
             owner_id = VALUES(owner_id),
             is_public = VALUES(is_public),
             title = VALUES(title),
             text = VALUES(text)"
    }

    fn upsert_setting() -> &'static str {
        "INSERT INTO setting (id, value) VALUES (?, ?)
         ON DUPLICATE KEY UPDATE value = VALUES(value)"
    }

    fn upsert_user_setting() -> &'static str {
        "INSERT INTO user_setting (id, user_id, value) VALUES (?, ?, ?)
         ON DUPLICATE KEY UPDATE value = VALUES(value)"
    }

    fn upsert_site_setting() -> &'static str {
        "INSERT INTO site_setting (id, site_id, value) VALUES (?, ?, ?)
         ON DUPLICATE KEY UPDATE value = VALUES(value)"
    }

    fn upsert_session() -> &'static str {
        "INSERT INTO session (id, modified, data) VALUES (?, ?, ?)
         ON DUPLICATE KEY UPDATE modified = VALUES(modified), data = VALUES(data)"
    }

    fn append_job_log() -> &'static str {
        "UPDATE job SET log = CONCAT(COALESCE(log, ''), ?, char(10)) WHERE id = ?"
    }

    fn begin_write() -> &'static str {
        "BEGIN"
    }

    fn for_update() -> &'static str {
        " FOR UPDATE"
    }

    fn fulltext_select() -> &'static str {
        "SELECT matched.id, matched.score FROM (
             SELECT fulltext_search.id AS id,
                    MATCH(fulltext_search.title, fulltext_search.text) AGAINST (?) AS score
             FROM fulltext_search
             WHERE fulltext_search.resource = ?
         ) AS matched
         WHERE matched.score > 0
         ORDER BY matched.score DESC, matched.id"
    }

    /// MySQL takes the user query itself; `AGAINST` splits it on word boundaries the same way
    /// [`crate::repos::match_expression`] does for FTS5.
    fn fulltext_needle(needle: &str) -> Option<String> {
        crate::repos::has_word(needle).then(|| needle.to_string())
    }

    fn fulltext_score(raw: f64) -> f64 {
        raw
    }
}
