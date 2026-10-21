//! `romeka_media_annotation`: the W3C Web Annotations of one media.
//!
//! One row is one live annotation. The target IRI is not stored; it follows from `media.item_id`
//! and `media_id` when a document is rendered. Geometry is canonical: a rectangle or a polygon in
//! upright original pixels in `selector`, a time range only in `t_start_ms`/`t_end_ms`. The
//! `CHECK`s on the table refuse a caption with no time and a track row that is not a caption.
//!
//! `revision` is the strong ETag `"r<revision>"`. A replace or a delete names the revision it read,
//! so a writer that lost a race gets the current row back instead of overwriting it. `client_key` is
//! the create's `Idempotency-Key`; `UNIQUE (media_id, client_key)` lets both engines store many
//! `NULL`s, so a write without a key needs no second index.
//!
//! Text search reads the SQLite FTS5 mirror `romeka_media_annotation_fts`, kept in step by triggers,
//! or MySQL's `FULLTEXT` index. MySQL does not index a word under `innodb_ft_min_token_size`, so a
//! short word is matched with `LIKE` inside [`MediaAnnotationRepo::search`] only ([`Backend`]).
//!
//! Rows are written here and by nothing else. Reads for a page, a manifest and a search all start
//! from [`MediaAnnotationRow`]; `omeka_api::annotation` alone turns a row into a domain type.

use std::collections::HashMap;

use sqlx::{ColumnIndex, Decode, Encode, Executor, IntoArguments, Pool, Sqlite, Transaction, Type};

use super::{Statement, bind_id, fetch_rows};
use crate::DbError;
use crate::backend::Backend;
use crate::ids::{ItemId, MediaId, UserId};
use crate::time;

/// A stored annotation as the table holds it. Only `omeka_api::annotation` builds a domain type
/// from it, in one place (`model::Annotation::from_row`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaAnnotationRow {
    pub id: i64,
    pub media_id: MediaId,
    pub owner_id: Option<UserId>,
    pub revision: u32,
    pub created: chrono::NaiveDateTime,
    pub modified: chrono::NaiveDateTime,
    pub motivation: String,
    pub origin: String,
    pub lang: Option<String>,
    pub selector_kind: String,
    pub selector: Option<String>,
    pub t_start_ms: Option<i64>,
    pub t_end_ms: Option<i64>,
    pub body_json: String,
    pub extensions: Option<String>,
}

/// What a write stores. `body_text` and `lang` are derived by the one caller function
/// Which rows a read covers: every row, or the notes a reader sees. A caption
/// cue is a row (a `supplementing` annotation with a time range and a track
/// origin), not a note; a reader's list and its counts leave the cues out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowScope {
    All,
    Notes,
}

/// What a write stores. `body_text` and `lang` are derived by the one caller function
/// (`omeka_api::annotation`), so a write cannot forget them.
#[derive(Debug, Clone, Copy)]
pub struct MediaAnnotationWrite<'a> {
    pub motivation: &'a str,
    pub selector_kind: &'a str,
    pub selector: Option<&'a str>,
    pub time_ms: Option<(i64, i64)>,
    pub body_json: &'a str,
    pub body_text: &'a str,
    pub lang: Option<&'a str>,
    pub extensions: Option<&'a str>,
}

/// The result of a create that named a key: the new row, or the row the key already names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InsertOutcome {
    Inserted(MediaAnnotationRow),
    KeyTaken(MediaAnnotationRow),
}

/// The result of a replace or a delete that expected a revision.
///
/// `Stale` carries the current row itself, not a box: it is the answer the caller renders on a
/// conflict, and a pointer would allocate on that one path for nothing.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChangeOutcome {
    Changed { revision: u32 },
    Stale { current: MediaAnnotationRow },
    Missing,
}

/// Who is searching. Built only by `omeka_api::annotation::access` from the actor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SearchScope {
    pub user: Option<UserId>,
    pub privileged: bool,
    pub hide_notes: bool,
}

/// A user's words made safe for FTS5 `MATCH` or MySQL boolean mode: each word quoted, operators
/// dropped. The only way raw input reaches `MATCH`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FtsQuery(String);

impl FtsQuery {
    /// `None` when the input holds no word, which matches nothing on either engine.
    pub fn from_words(input: &str) -> Option<Self> {
        let words: Vec<String> = input
            .split(|c: char| !c.is_alphanumeric())
            .filter(|word| !word.is_empty())
            .map(|word| format!("\"{word}\""))
            .collect();
        if words.is_empty() {
            None
        } else {
            Some(Self(words.join(" ")))
        }
    }

    /// The words, without the quotes that keep them from reading as operators.
    fn words(&self) -> impl Iterator<Item = &str> {
        self.0.split("\" \"").map(|word| word.trim_matches('"'))
    }
}

/// One cue of a caption import.
#[derive(Debug, Clone, Copy)]
pub struct Cue<'a> {
    pub start_ms: i64,
    pub end_ms: i64,
    pub body_json: &'a str,
    pub body_text: &'a str,
}

/// The result of a caption import.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackOutcome {
    Unchanged { cues: u32 },
    Replaced { removed: u32, inserted: u32 },
}

/// The columns every read lists, under the alias `a`.
const ROW_COLUMNS: &str = "a.id, a.media_id, a.owner_id, a.revision, a.created, a.modified,
     a.motivation, a.origin, a.lang, a.selector_kind, a.selector, a.t_start_ms, a.t_end_ms,
     a.body_json, a.extensions";

/// `romeka_media_annotation` rows.
pub struct MediaAnnotationRepo<'a, DB: Backend = Sqlite> {
    pool: &'a Pool<DB>,
}

impl<'a, DB: Backend> MediaAnnotationRepo<'a, DB>
where
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
    usize: ColumnIndex<DB::Row>,
    for<'x> i64: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Option<i64>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> bool: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> String: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Option<String>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Option<&'x str>: Encode<'x, DB> + Type<DB>,
    for<'x> chrono::NaiveDateTime: Encode<'x, DB> + Type<DB>,
    for<'x> Vec<u8>: Decode<'x, DB> + Type<DB>,
    for<'x> Option<Vec<u8>>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> &'x str: Encode<'x, DB> + Type<DB> + ColumnIndex<DB::Row>,
    for<'x> f64: Decode<'x, DB> + Type<DB>,
{
    pub fn new(pool: &'a Pool<DB>) -> Self {
        Self { pool }
    }

    /// Keyset page in id order: rows with id > `after`, at most `limit + 1`.
    ///
    /// `motivations` filters (None = all); the switch passes `Some(&["supplementing"])`. The caller
    /// keeps the extra row as the sign that another page exists.
    pub async fn page(
        &self,
        media: MediaId,
        after: i64,
        limit: u32,
        motivations: Option<&[&str]>,
        scope: RowScope,
    ) -> Result<Vec<MediaAnnotationRow>, DbError> {
        let mut statement = Statement::<DB>::new(PAGE_COLUMNS);
        push_page(&mut statement, media, after, limit, motivations, scope)?;
        let (sql, arguments) = statement.into_parts();
        fetch_rows(&sql, arguments, self.pool)
            .await?
            .iter()
            .map(row_from::<DB>)
            .collect()
    }

    pub async fn total(
        &self,
        media: MediaId,
        motivations: Option<&[&str]>,
        scope: RowScope,
    ) -> Result<u32, DbError> {
        let mut statement =
            Statement::<DB>::new("SELECT COUNT(*) FROM romeka_media_annotation WHERE media_id = ");
        statement.bind(bind_id(media))?;
        push_motivations(&mut statement, motivations)?;
        push_scope(&mut statement, scope);
        let (sql, arguments) = statement.into_parts();
        let raw: i64 = sqlx::query_scalar_with::<DB, i64, _>(&sql, arguments)
            .fetch_one(self.pool)
            .await?;
        count_of(raw)
    }

    /// First `per_media` rows of each media, time order then id. One query.
    pub async fn firsts(
        &self,
        media: &[MediaId],
        per_media: u32,
        motivations: Option<&[&str]>,
        scope: RowScope,
    ) -> Result<Vec<MediaAnnotationRow>, DbError> {
        if media.is_empty() {
            return Ok(Vec::new());
        }
        let mut statement = Statement::<DB>::new(
            "SELECT id, media_id, owner_id, revision, created, modified, motivation, origin,
             lang, selector_kind, selector, t_start_ms, t_end_ms, body_json, extensions
             FROM (SELECT a.id, a.media_id, a.owner_id, a.revision, a.created, a.modified,
                 a.motivation, a.origin, a.lang, a.selector_kind, a.selector, a.t_start_ms,
                 a.t_end_ms, a.body_json, a.extensions,
                 ROW_NUMBER() OVER (PARTITION BY a.media_id
                     ORDER BY a.t_start_ms IS NULL, a.t_start_ms, a.id) AS position
                 FROM romeka_media_annotation a WHERE a.media_id IN (",
        );
        push_ids(&mut statement, media)?;
        statement.push(")");
        push_motivations(&mut statement, motivations)?;
        push_scope(&mut statement, scope);
        statement.push(") ranked WHERE ranked.position <= ");
        statement.bind(i64::from(per_media))?;
        statement.push(" ORDER BY ranked.media_id, ranked.position");
        let (sql, arguments) = statement.into_parts();
        fetch_rows(&sql, arguments, self.pool)
            .await?
            .iter()
            .map(row_from::<DB>)
            .collect()
    }

    pub async fn counts(
        &self,
        media: &[MediaId],
        motivations: Option<&[&str]>,
        scope: RowScope,
    ) -> Result<HashMap<MediaId, u32>, DbError> {
        if media.is_empty() {
            return Ok(HashMap::new());
        }
        let mut statement = Statement::<DB>::new(
            "SELECT media_id, COUNT(*) AS total FROM romeka_media_annotation
             WHERE media_id IN (",
        );
        push_ids(&mut statement, media)?;
        statement.push(")");
        push_motivations(&mut statement, motivations)?;
        push_scope(&mut statement, scope);
        statement.push(" GROUP BY media_id");
        let (sql, arguments) = statement.into_parts();
        let rows = fetch_rows(&sql, arguments, self.pool).await?;
        rows.iter()
            .map(|row| Ok((DB::id(row, "media_id")?, count_of(DB::int(row, "total")?)?)))
            .collect()
    }

    pub async fn get(&self, id: i64) -> Result<Option<MediaAnnotationRow>, DbError> {
        let sql = format!("SELECT {ROW_COLUMNS} FROM romeka_media_annotation a WHERE a.id = ?");
        let row = sqlx::query::<DB>(&sql)
            .bind(id)
            .fetch_optional(self.pool)
            .await?;
        row.as_ref().map(row_from::<DB>).transpose()
    }

    pub async fn insert(
        &self,
        media: MediaId,
        owner: Option<UserId>,
        client_key: Option<&str>,
        write: &MediaAnnotationWrite<'_>,
    ) -> Result<InsertOutcome, DbError> {
        let now = time::now();
        let (start, end) = match write.time_ms {
            Some((start, end)) => (Some(start), Some(end)),
            None => (None, None),
        };
        let result = sqlx::query::<DB>(
            "INSERT INTO romeka_media_annotation
                 (media_id, owner_id, revision, created, modified, motivation, origin, lang,
                  selector_kind, selector, t_start_ms, t_end_ms, body_json, body_text, extensions,
                  client_key)
             VALUES (?, ?, 1, ?, ?, ?, 'author', ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(bind_id(media))
        .bind(super::bind_opt_id(owner))
        .bind(now)
        .bind(now)
        .bind(write.motivation)
        .bind(write.lang)
        .bind(write.selector_kind)
        .bind(write.selector)
        .bind(start)
        .bind(end)
        .bind(write.body_json)
        .bind(write.body_text)
        .bind(write.extensions)
        .bind(client_key)
        .execute(self.pool)
        .await
        .map_err(DbError::from);
        match result {
            Ok(done) => {
                let inserted = DB::inserted_id(&done)?;
                let id = i64::try_from(inserted).map_err(|_| DbError::Column {
                    column: "id",
                    reason: format!("insert returned {inserted}, beyond the signed range"),
                })?;
                let row = self.get(id).await?.ok_or(DbError::Column {
                    column: "id",
                    reason: format!("the row just inserted as {id} is gone"),
                })?;
                Ok(InsertOutcome::Inserted(row))
            }
            Err(error) if error.is_unique_violation() => {
                let Some(key) = client_key else {
                    return Err(error);
                };
                let taken = self.by_key(media, key).await?;
                match taken {
                    Some(row) => Ok(InsertOutcome::KeyTaken(row)),
                    None => Err(error),
                }
            }
            Err(error) => Err(error),
        }
    }

    /// Never overwrites a newer revision. Origin is never changed by a replace.
    pub async fn replace(
        &self,
        id: i64,
        expected: u32,
        write: &MediaAnnotationWrite<'_>,
    ) -> Result<ChangeOutcome, DbError> {
        let now = time::now();
        let (start, end) = match write.time_ms {
            Some((start, end)) => (Some(start), Some(end)),
            None => (None, None),
        };
        let done = sqlx::query::<DB>(
            "UPDATE romeka_media_annotation
             SET revision = revision + 1, modified = ?, motivation = ?, selector_kind = ?,
                 selector = ?, t_start_ms = ?, t_end_ms = ?, body_json = ?, body_text = ?,
                 lang = ?, extensions = ?
             WHERE id = ? AND revision = ?",
        )
        .bind(now)
        .bind(write.motivation)
        .bind(write.selector_kind)
        .bind(write.selector)
        .bind(start)
        .bind(end)
        .bind(write.body_json)
        .bind(write.body_text)
        .bind(write.lang)
        .bind(write.extensions)
        .bind(id)
        .bind(i64::from(expected))
        .execute(self.pool)
        .await?;
        if DB::rows_affected(&done) == 0 {
            return Ok(match self.get(id).await? {
                Some(current) => ChangeOutcome::Stale { current },
                None => ChangeOutcome::Missing,
            });
        }
        let revision = self.get(id).await?.map_or(expected + 1, |row| row.revision);
        Ok(ChangeOutcome::Changed { revision })
    }

    pub async fn delete(&self, id: i64, expected: u32) -> Result<ChangeOutcome, DbError> {
        let done =
            sqlx::query::<DB>("DELETE FROM romeka_media_annotation WHERE id = ? AND revision = ?")
                .bind(id)
                .bind(i64::from(expected))
                .execute(self.pool)
                .await?;
        if DB::rows_affected(&done) > 0 {
            return Ok(ChangeOutcome::Changed {
                revision: expected + 1,
            });
        }
        Ok(match self.get(id).await? {
            Some(current) => ChangeOutcome::Stale { current },
            None => ChangeOutcome::Missing,
        })
    }

    /// Replace the track rows of (`media`, `lang`) with `cues`, one transaction.
    ///
    /// Equal cues (same times, text and order) change nothing and bump no revision, so importing
    /// the same file twice is a no-op. Hand-written rows, including hand-written `supplementing`
    /// ones, are left alone.
    pub async fn replace_track(
        &self,
        media: MediaId,
        lang: Option<&str>,
        cues: &[Cue<'_>],
    ) -> Result<TrackOutcome, DbError> {
        let mut tx = self.pool.begin_with(DB::begin_write()).await?;
        let existing = track_rows::<DB>(&mut tx, media, lang, DB::for_update()).await?;
        if equal_cues(&existing, cues) {
            tx.commit().await?;
            return Ok(TrackOutcome::Unchanged {
                cues: u32::try_from(cues.len()).unwrap_or(u32::MAX),
            });
        }
        let now = time::now();
        let mut delete =
            Statement::<DB>::new("DELETE FROM romeka_media_annotation WHERE media_id = ");
        delete.bind(bind_id(media))?;
        delete.push(" AND motivation = 'supplementing' AND origin = 'track' AND ");
        push_lang(&mut delete, lang)?;
        let (sql, arguments) = delete.into_parts();
        let removed = sqlx::query_with::<DB, _>(&sql, arguments)
            .execute(&mut *tx)
            .await?;
        for cue in cues {
            sqlx::query::<DB>(
                "INSERT INTO romeka_media_annotation
                     (media_id, owner_id, revision, created, modified, motivation, origin, lang,
                      selector_kind, selector, t_start_ms, t_end_ms, body_json, body_text,
                      extensions, client_key)
                 VALUES (?, NULL, 1, ?, ?, 'supplementing', 'track', ?, 'time', NULL, ?, ?, ?, ?,
                         NULL, NULL)",
            )
            .bind(bind_id(media))
            .bind(now)
            .bind(now)
            .bind(lang)
            .bind(cue.start_ms)
            .bind(cue.end_ms)
            .bind(cue.body_json)
            .bind(cue.body_text)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(TrackOutcome::Replaced {
            removed: u32::try_from(DB::rows_affected(&removed)).unwrap_or(u32::MAX),
            inserted: u32::try_from(cues.len()).unwrap_or(u32::MAX),
        })
    }

    /// Every `supplementing` row of `media` in `lang` (author and track), time order.
    pub async fn cues(
        &self,
        media: MediaId,
        lang: Option<&str>,
    ) -> Result<Vec<MediaAnnotationRow>, DbError> {
        let mut statement = Statement::<DB>::new(
            "SELECT a.id, a.media_id, a.owner_id, a.revision, a.created, a.modified,
             a.motivation, a.origin, a.lang, a.selector_kind, a.selector, a.t_start_ms,
             a.t_end_ms, a.body_json, a.extensions
             FROM romeka_media_annotation a WHERE a.media_id = ",
        );
        statement.bind(bind_id(media))?;
        statement.push(" AND a.motivation = 'supplementing' AND ");
        push_lang(&mut statement, lang)?;
        statement.push(" ORDER BY a.t_start_ms, a.id");
        let (sql, arguments) = statement.into_parts();
        fetch_rows(&sql, arguments, self.pool)
            .await?
            .iter()
            .map(row_from::<DB>)
            .collect()
    }

    /// The `supplementing` languages of each media and their cue counts.
    pub async fn track_langs(
        &self,
        media: &[MediaId],
    ) -> Result<Vec<(MediaId, Option<String>, u32)>, DbError> {
        if media.is_empty() {
            return Ok(Vec::new());
        }
        let mut statement = Statement::<DB>::new(
            "SELECT media_id, lang, COUNT(*) AS cues FROM romeka_media_annotation
             WHERE motivation = 'supplementing' AND origin = 'track' AND media_id IN (",
        );
        push_ids(&mut statement, media)?;
        statement.push(") GROUP BY media_id, lang ORDER BY media_id, lang");
        let (sql, arguments) = statement.into_parts();
        let rows = fetch_rows(&sql, arguments, self.pool).await?;
        rows.iter()
            .map(|row| {
                Ok((
                    DB::id(row, "media_id")?,
                    DB::opt_text(row, "lang")?,
                    count_of(DB::int(row, "cues")?)?,
                ))
            })
            .collect()
    }

    /// Text search over the visible media of `item`.
    ///
    /// The visibility predicate is SQL: media and item resource each public, or one of them owned
    /// by `scope.user`, or `scope.privileged`. `scope.hide_notes` is the switch, which leaves only
    /// captions. Paging is keyset by annotation id.
    pub async fn search(
        &self,
        scope: &SearchScope,
        item: ItemId,
        query: &FtsQuery,
        after: i64,
        limit: u32,
    ) -> Result<Vec<MediaAnnotationRow>, DbError> {
        let words: Vec<&str> = query.words().collect();
        if words.is_empty() {
            return Ok(Vec::new());
        }
        let mut statement = Statement::<DB>::new(&format!(
            "SELECT a.id, a.media_id, a.owner_id, a.revision, a.created, a.modified,
             a.motivation, a.origin, a.lang, a.selector_kind, a.selector, a.t_start_ms,
             a.t_end_ms, a.body_json, a.extensions
             FROM romeka_media_annotation a{} JOIN media ON media.id = a.media_id
             JOIN resource media_resource ON media_resource.id = media.id
             JOIN resource item_resource ON item_resource.id = media.item_id
             WHERE a.id > ",
            DB::annotation_search_join()
        ));
        statement.bind(after)?;
        statement.push(" AND media.item_id = ");
        statement.bind(bind_id(item))?;
        if !scope.privileged {
            statement.push(" AND ((media_resource.is_public = 1 AND item_resource.is_public = 1)");
            if let Some(user) = scope.user {
                statement.push(" OR media_resource.owner_id = ");
                statement.bind(bind_id(user))?;
                statement.push(" OR item_resource.owner_id = ");
                statement.bind(bind_id(user))?;
            }
            statement.push(")");
        }
        if scope.hide_notes {
            statement.push(" AND a.motivation = 'supplementing'");
        }
        statement.push(" AND ");
        let (predicate, values) = DB::annotation_match(&words);
        push_with_values(&mut statement, &predicate, values)?;
        statement.push(" ORDER BY a.id LIMIT ");
        statement.bind(i64::from(limit) + 1)?;
        let (sql, arguments) = statement.into_parts();
        fetch_rows(&sql, arguments, self.pool)
            .await?
            .iter()
            .map(row_from::<DB>)
            .collect()
    }

    /// The row a create's key already names, if any.
    async fn by_key(
        &self,
        media: MediaId,
        client_key: &str,
    ) -> Result<Option<MediaAnnotationRow>, DbError> {
        let sql = format!(
            "SELECT {ROW_COLUMNS} FROM romeka_media_annotation a
             WHERE a.media_id = ? AND a.client_key = ?"
        );
        let row = sqlx::query::<DB>(&sql)
            .bind(bind_id(media))
            .bind(client_key)
            .fetch_optional(self.pool)
            .await?;
        row.as_ref().map(row_from::<DB>).transpose()
    }
}

/// The columns a keyset page lists, under the alias `a`.
const PAGE_COLUMNS: &str = "SELECT a.id, a.media_id, a.owner_id, a.revision, a.created, a.modified,
     a.motivation, a.origin, a.lang, a.selector_kind, a.selector, a.t_start_ms, a.t_end_ms,
     a.body_json, a.extensions
     FROM romeka_media_annotation a WHERE a.media_id = ";

/// The keyset page: the cursor, the motivation filter and `limit + 1`, never an offset. The unit
/// test reads this statement's text, so the property that makes page 30 cost what page 0 costs is
/// measured on the statement [`MediaAnnotationRepo::page`] runs.
fn push_page<'q, DB: Backend>(
    statement: &mut Statement<'q, DB>,
    media: MediaId,
    after: i64,
    limit: u32,
    motivations: Option<&[&'q str]>,
    scope: RowScope,
) -> Result<(), DbError>
where
    for<'x> i64: Encode<'x, DB> + Type<DB>,
    for<'x> &'x str: Encode<'x, DB> + Type<DB>,
{
    statement.bind(bind_id(media))?;
    statement.push(" AND a.id > ");
    statement.bind(after)?;
    push_motivations(statement, motivations)?;
    push_scope(statement, scope);
    statement.push(" ORDER BY a.id LIMIT ");
    statement.bind(i64::from(limit) + 1)?;
    Ok(())
}

/// `origin <> 'track'` for the notes scope: a cue imported from a caption file
/// is a row, and the list of notes is not the transcript.
fn push_scope<DB: Backend>(statement: &mut Statement<'_, DB>, scope: RowScope) {
    if scope == RowScope::Notes {
        statement.push(" AND origin <> 'track'");
    }
}

/// Append one `(?, ?)` per id, inside an `IN (` the caller opened.
fn push_ids<DB: Backend>(statement: &mut Statement<'_, DB>, ids: &[MediaId]) -> Result<(), DbError>
where
    for<'x> i64: Encode<'x, DB> + Type<DB>,
{
    for (index, id) in ids.iter().enumerate() {
        if index > 0 {
            statement.push(", ");
        }
        statement.bind(bind_id(*id))?;
    }
    Ok(())
}

/// A motivation filter, or nothing. An empty list selects no row rather than every row.
fn push_motivations<'q, DB: Backend>(
    statement: &mut Statement<'q, DB>,
    motivations: Option<&[&'q str]>,
) -> Result<(), DbError>
where
    for<'x> &'x str: Encode<'x, DB> + Type<DB>,
{
    let Some(motivations) = motivations else {
        return Ok(());
    };
    if motivations.is_empty() {
        statement.push(" AND 1 = 0");
        return Ok(());
    }
    statement.push(" AND motivation IN (");
    for (index, motivation) in motivations.iter().enumerate() {
        if index > 0 {
            statement.push(", ");
        }
        statement.bind(*motivation)?;
    }
    statement.push(")");
    Ok(())
}

/// `lang = ?`, or `lang IS NULL`. Spelled in the SQL rather than with MySQL's `<=>` so one
/// statement serves both engines.
fn push_lang<'q, DB: Backend>(
    statement: &mut Statement<'q, DB>,
    lang: Option<&'q str>,
) -> Result<(), DbError>
where
    for<'x> &'x str: Encode<'x, DB> + Type<DB>,
{
    match lang {
        Some(lang) => {
            statement.push("lang = ");
            statement.bind(lang)?;
        }
        None => {
            statement.push("lang IS NULL");
        }
    }
    Ok(())
}

/// Append a predicate whose `?` marks one value of `values`, in order. The backend writes the
/// predicate this way because [`Statement::bind`] appends a placeholder itself.
fn push_with_values<DB: Backend>(
    statement: &mut Statement<'_, DB>,
    predicate: &str,
    values: Vec<String>,
) -> Result<(), DbError>
where
    for<'x> String: Encode<'x, DB> + Type<DB>,
{
    let mut parts = predicate.split('?');
    if let Some(first) = parts.next() {
        statement.push(first);
    }
    for (value, rest) in values.into_iter().zip(parts) {
        statement.bind(value)?;
        statement.push(rest);
    }
    Ok(())
}

/// The track rows of one language, in id order, locked when the caller asked for it.
async fn track_rows<DB: Backend>(
    tx: &mut Transaction<'static, DB>,
    media: MediaId,
    lang: Option<&str>,
    lock: &str,
) -> Result<Vec<MediaAnnotationRow>, DbError>
where
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
    usize: ColumnIndex<DB::Row>,
    for<'x> i64: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Option<i64>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> String: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Option<String>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> &'x str: Encode<'x, DB> + Type<DB>,
{
    let mut statement = Statement::<DB>::new(
        "SELECT a.id, a.media_id, a.owner_id, a.revision, a.created, a.modified,
         a.motivation, a.origin, a.lang, a.selector_kind, a.selector, a.t_start_ms, a.t_end_ms,
         a.body_json, a.extensions
         FROM romeka_media_annotation a WHERE a.media_id = ",
    );
    statement.bind(bind_id(media))?;
    statement.push(" AND a.motivation = 'supplementing' AND a.origin = 'track' AND ");
    push_lang(&mut statement, lang)?;
    statement.push(" ORDER BY a.id");
    statement.push(lock);
    let (sql, arguments) = statement.into_parts();
    let rows = sqlx::query_with::<DB, _>(&sql, arguments)
        .fetch_all(&mut **tx)
        .await?;
    rows.iter().map(row_from::<DB>).collect()
}

fn equal_cues(existing: &[MediaAnnotationRow], cues: &[Cue<'_>]) -> bool {
    existing.len() == cues.len()
        && existing.iter().zip(cues).all(|(row, cue)| {
            row.t_start_ms == Some(cue.start_ms)
                && row.t_end_ms == Some(cue.end_ms)
                && row.body_json == cue.body_json
        })
}

fn count_of(raw: i64) -> Result<u32, DbError> {
    u32::try_from(raw).map_err(|_| DbError::Column {
        column: "count",
        reason: format!("{raw} rows does not fit a count"),
    })
}

fn row_from<DB: Backend>(row: &DB::Row) -> Result<MediaAnnotationRow, DbError> {
    let revision = DB::int(row, "revision")?;
    Ok(MediaAnnotationRow {
        id: DB::int(row, "id")?,
        media_id: DB::id(row, "media_id")?,
        owner_id: DB::opt_id(row, "owner_id")?,
        revision: u32::try_from(revision).map_err(|_| DbError::Column {
            column: "revision",
            reason: format!("{revision} is not a revision"),
        })?,
        created: DB::datetime(row, "created")?,
        modified: DB::datetime(row, "modified")?,
        motivation: DB::text(row, "motivation")?,
        origin: DB::text(row, "origin")?,
        lang: DB::opt_text(row, "lang")?,
        selector_kind: DB::text(row, "selector_kind")?,
        selector: DB::opt_text(row, "selector")?,
        t_start_ms: DB::opt_int(row, "t_start_ms")?,
        t_end_ms: DB::opt_int(row, "t_end_ms")?,
        body_json: DB::text(row, "body_json")?,
        extensions: DB::opt_text(row, "extensions")?,
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn a_page_statement_has_no_offset() {
        let mut statement = Statement::<Sqlite>::new(PAGE_COLUMNS);
        push_page(
            &mut statement,
            MediaId(7),
            0,
            3,
            Some(&["supplementing"]),
            RowScope::All,
        )
        .unwrap();
        let sql = statement.sql();
        assert!(!sql.contains("OFFSET"), "{sql}");
        assert!(sql.contains("a.id > "), "{sql}");
        assert!(
            sql.contains("LIMIT ?"),
            "the statement binds its limit: {sql}"
        );
    }

    #[test]
    fn fts_words_are_quoted_and_operators_dropped() {
        let query = FtsQuery::from_words("shield* \"wall\" NEAR").unwrap();
        assert_eq!(
            query.words().collect::<Vec<_>>(),
            ["shield", "wall", "NEAR"]
        );
        assert_eq!(FtsQuery::from_words(" *** "), None);
        assert_eq!(FtsQuery::from_words("\""), None);
    }
}
