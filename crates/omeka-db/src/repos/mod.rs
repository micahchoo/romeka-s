//! Repositories: the only place in the workspace where SQL lives.
//!
//! Each repository is generic over [`Backend`] and borrows a `Pool<DB>`. One implementation serves
//! SQLite and MySQL; the statements that cannot be shared come from the backend
//! ([`crate::backend`]). A caller that names no backend gets SQLite, so
//! `ItemRepo::new(&sqlite_pool)` is unchanged and `ItemRepo::<MySql>::new(&mysql_pool)` is the
//! MySQL repository.
//!
//! Repositories speak in row structs. List filters mirror the query parameters of the matching
//! upstream adapter (`application/src/Api/Adapter/*.php`); where an adapter filter needs a join or
//! full-text search the port leaves it to a later unit and says so in the repository's
//! documentation.
//!
//! Timestamp policy mirrors upstream `AbstractEntityAdapter::updateTimestamps`: a create writes the
//! same instant into `created` and `modified`, an update rewrites `modified`, and both ignore a
//! caller-supplied `modified`.

mod assets;
mod attachments;
mod collation;
mod custom_vocabs;
mod database;
mod fulltext;
mod identifiers;
mod import_evidence;
mod import_index;
mod import_ledger;
mod import_lifecycle;
mod import_made;
mod import_state;
mod import_touched;
mod import_writes;
mod imports;
mod item_sites;
mod jobs;
mod media;
mod media_annotations;
mod media_extents;
mod modules;
mod page_versions;
mod passwords;
mod publications;
mod query;
mod relations;
mod resource_reads;
mod resources;
mod rewrites;
mod sessions;
mod settings;
mod sites;
mod templates;
mod terms;
mod user_uploads;
mod users;
mod values;
mod vocabularies;

pub use assets::{AssetFilter, AssetOwner, AssetRepo};
pub use attachments::SiteBlockAttachmentRepo;
pub use collation::{compare_opt_text_ci, compare_text_ci};
pub use custom_vocabs::CustomVocabRepo;
pub use database::DatabaseRepo;
pub use fulltext::{FulltextMatch, FulltextRepo, has_word, match_expression};
pub use identifiers::IdentifierRepo;
pub use import_evidence::{EntryRepo, UploadAdvance, UploadRepo};
pub use import_ledger::{BeforeImageRepo, ImportResourceRepo};
pub use import_lifecycle::{DEAD_AFTER, DIED, HEARTBEAT, HEARTBEAT_EVERY, StartOutcome};
pub use import_touched::{ImportTouchRepo, Pointer, Presence, PropertyUsers};
pub use import_writes::ImportWrites;
pub use imports::{ImportRepo, PlanSave};
pub use item_sites::ItemSiteRepo;
pub use jobs::{JobFilter, JobRepo};
pub use media::{MediaFileOwner, MediaFilter, MediaRepo};
pub use media_annotations::{
    ChangeOutcome, Cue, FtsQuery, InsertOutcome, MediaAnnotationRepo, MediaAnnotationRow,
    MediaAnnotationWrite, SearchScope, TrackOutcome,
};
pub use media_extents::{ExtentRow, MediaExtentRepo};
pub use modules::ModuleRepo;
pub use page_versions::{KEPT_VERSIONS, PageVersionRepo, Snapshot};
pub use passwords::PasswordCreationRepo;
pub use publications::{NewPublicationVersion, PublicationRepo};
pub use query::{Joiner, MatchKind, MediaKindFilter, MediaVisibility, QueryFilter, ValueQuery};
pub use relations::RelationRepo;
pub use resource_reads::{ItemPage, ItemWindow};
pub use resources::{ItemFilter, ItemRepo, ItemSetFilter, ItemSetRepo, ResourceRepo};
pub use rewrites::ValueRewriteRepo;
pub use sessions::SessionRepo;
pub use settings::{SettingRepo, SiteSettingRepo, UserSettingRepo};
pub use sites::{SiteFilter, SiteRepo};
pub use templates::TemplateRepo;
pub use terms::{ClassTerm, PropertyTerm, TermRepo, TermScope};
pub use user_uploads::UserUploadRepo;
pub use users::{ApiKeyRepo, UserFilter, UserRepo};
pub use values::ValueRepo;
pub use vocabularies::{
    ClassFilter, ClassRepo, PropertyFilter, PropertyRepo, VocabularyFilter, VocabularyRepo,
};

use serde_json::Value;
use sqlx::query::Query;
use sqlx::{Arguments, Encode, Executor, IntoArguments, Pool, Type};

use crate::DbError;
use crate::backend::Backend;

/// A list window: upstream's `page` and `per_page`, already turned into `LIMIT` and `OFFSET`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Paging {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

impl Paging {
    pub fn new(limit: i64, offset: i64) -> Self {
        Self {
            limit: Some(limit),
            offset: Some(offset),
        }
    }
}

/// A statement under construction: the SQL text plus the values bound so far.
///
/// This replaces `sqlx::QueryBuilder`, which cannot run when the database is a type parameter:
/// `build()` ties the query's argument lifetime to the borrow of the builder, and for a projection
/// such as `DB::Arguments` the borrow checker rejects that. The text here is owned by the caller's
/// local, so the query borrows that local and nothing else.
pub(crate) struct Statement<'q, DB: Backend> {
    sql: String,
    arguments: DB::Arguments<'q>,
}

impl<'q, DB: Backend> Statement<'q, DB> {
    pub(crate) fn new(sql: &str) -> Self {
        Self {
            sql: sql.to_string(),
            arguments: DB::Arguments::default(),
        }
    }

    /// Append SQL text.
    pub(crate) fn push(&mut self, sql: &str) -> &mut Self {
        self.sql.push_str(sql);
        self
    }

    /// Append a `?` placeholder and the value it stands for.
    pub(crate) fn bind<T>(&mut self, value: T) -> Result<&mut Self, DbError>
    where
        T: 'q + Encode<'q, DB> + Type<DB>,
    {
        self.sql.push('?');
        self.arguments.add(value).map_err(|error| DbError::Bind {
            reason: error.to_string(),
        })?;
        Ok(self)
    }

    /// The SQL text as built so far, for the unit tests.
    #[cfg(test)]
    pub(crate) fn sql(&self) -> &str {
        &self.sql
    }

    /// The finished statement, ready for [`fetch_all`].
    pub(crate) fn into_parts(self) -> (String, DB::Arguments<'q>) {
        (self.sql, self.arguments)
    }
}

/// Bind a list window onto a statement.
///
/// An absent limit becomes the backend's "no limit" value: SQLite reads `-1`, MySQL has no such
/// value and gets a very large number.
pub(crate) fn push_paging<'q, DB: Backend>(
    statement: &mut Statement<'q, DB>,
    paging: &Paging,
) -> Result<(), DbError>
where
    for<'x> i64: Encode<'x, DB> + Type<DB>,
{
    statement.push(" LIMIT ");
    statement.bind(paging.limit.unwrap_or_else(DB::no_limit))?;
    statement.push(" OFFSET ");
    statement.bind(paging.offset.unwrap_or(0))?;
    Ok(())
}

/// Run a finished statement and return its raw rows.
///
/// The caller maps each row with the row struct's own `from_db_row`, which is the storage boundary.
pub(crate) async fn fetch_rows<'q, DB>(
    sql: &'q str,
    arguments: DB::Arguments<'q>,
    pool: &Pool<DB>,
) -> Result<Vec<DB::Row>, DbError>
where
    DB: Backend,
    for<'a> DB::Arguments<'a>: IntoArguments<'a, DB>,
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
{
    let rows = sqlx::query_with::<DB, _>(sql, arguments)
        .fetch_all(pool)
        .await?;
    Ok(rows)
}

/// Bind an id as the signed integer both engines store. Every id in this crate came out of an
/// integer column, so the cast preserves the value.
pub(crate) fn bind_id(id: impl Into<u64>) -> i64 {
    id.into() as i64
}

pub(crate) fn bind_opt_id<T: Into<u64>>(id: Option<T>) -> Option<i64> {
    id.map(bind_id)
}

/// Serialise a required JSON column. A value that cannot be encoded returns a `DbError` rather
/// than panicking, because the value came from a caller that only this crate writes.
pub(crate) fn json_text(value: &Value) -> Result<String, DbError> {
    serde_json::to_string(value).map_err(|error| DbError::Column {
        column: "json",
        reason: format!("cannot store as JSON: {error}"),
    })
}

/// Check an id that came back from an insert or a scalar select before it becomes a typed id.
pub(crate) fn new_id(raw: i64) -> Result<u64, DbError> {
    u64::try_from(raw).map_err(|_| DbError::Column {
        column: "id",
        reason: format!("insert returned id {raw}"),
    })
}

/// Run an insert and return the id the backend assigned.
///
/// Both engines report the id on the statement result, so no statement in this crate needs
/// `RETURNING` (SQLite only) or a second `SELECT LAST_INSERT_ID()` (MySQL only).
pub(crate) async fn insert_id<'q, DB, E>(
    query: Query<'q, DB, DB::Arguments<'q>>,
    executor: E,
) -> Result<u64, DbError>
where
    DB: Backend,
    for<'a> DB::Arguments<'a>: IntoArguments<'a, DB>,
    E: Executor<'q, Database = DB>,
{
    let result = query.execute(executor).await?;
    DB::inserted_id(&result)
}
