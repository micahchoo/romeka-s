//! `media` repository.
//!
//! A media file is a `resource` row plus the media columns, sharing the id. The ingester and
//! renderer names are upstream class names (`application/src/Media/*`); this crate stores them and
//! never interprets them.
//!
//! Filters mirror `MediaAdapter::buildQuery`: `item_id`, `media_type`, `ingester`, `renderer`, plus
//! the parent adapter's `owner_id` and `is_public`. The port adds `kinds`, the `romeka_kind` filter.

use sqlx::{ColumnIndex, Decode, Encode, Executor, IntoArguments, Pool, Sqlite, Type};

use super::resources::{insert_resource, update_resource_columns};
use super::{Paging, QueryFilter, Statement, bind_id, fetch_rows, push_paging};
use crate::DbError;
use crate::backend::Backend;
use crate::ids::{ClassId, ItemId, MediaId, TemplateId, UserId};
use crate::rows::{MediaRow, NewMedia, resource_type};

/// A list filter for `media`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MediaFilter {
    pub item_id: Option<ItemId>,
    pub ingester: Option<String>,
    pub renderer: Option<String>,
    pub media_type: Option<String>,
    /// `romeka_kind`: media of one of these port kind names
    /// (`MediaKind::name`). `None` adds no filter; an empty list matches no row.
    pub kinds: Option<Vec<String>>,
    pub owner_id: Option<UserId>,
    pub resource_class_id: Option<ClassId>,
    pub resource_template_id: Option<TemplateId>,
    pub is_public: Option<bool>,
}

const MEDIA_COLUMNS: &str = "SELECT resource.*, media.item_id, media.ingester, media.renderer,
     media.data, media.source, media.media_type, media.storage_id, media.extension, media.sha256,
     media.size, media.has_original, media.has_thumbnails, media.position, media.lang,
     media.alt_text
     FROM media JOIN resource ON resource.id = media.id";

/// What decides who may fetch a media's stored files: the media's visibility and its item's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaFileOwner {
    pub media_id: MediaId,
    /// `media.extension`, the suffix of the original's storage path.
    pub extension: Option<String>,
    /// `media.media_type`, the type the file answers with.
    pub media_type: Option<String>,
    /// `media.source`, the name a download is saved under.
    pub source: Option<String>,
    pub media_is_public: bool,
    pub media_owner_id: Option<UserId>,
    pub item_is_public: bool,
    pub item_owner_id: Option<UserId>,
}

/// `media` rows.
pub struct MediaRepo<'a, DB: Backend = Sqlite> {
    pool: &'a Pool<DB>,
}

impl<'a, DB: Backend> MediaRepo<'a, DB>
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

    pub async fn create(&self, new: &NewMedia) -> Result<MediaRow, DbError> {
        let mut tx = self.pool.begin().await?;
        let row = Self::insert(&mut tx, new).await?;
        tx.commit().await?;
        Ok(row)
    }

    /// Insert a media on the caller's connection, so it can join a larger transaction.
    pub(crate) async fn insert(
        conn: &mut DB::Connection,
        new: &NewMedia,
    ) -> Result<MediaRow, DbError> {
        let resource = insert_resource(conn, &new.resource, resource_type::MEDIA).await?;
        sqlx::query(
            "INSERT INTO media
                 (id, item_id, ingester, renderer, data, source, media_type, storage_id, extension,
                  sha256, size, has_original, has_thumbnails, position, lang, alt_text)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(bind_id(resource.id))
        .bind(bind_id(new.item_id))
        .bind(&new.ingester)
        .bind(&new.renderer)
        .bind(
            new.data
                .as_ref()
                .map(serde_json::to_string)
                .transpose()
                .map_err(database_json)?,
        )
        .bind(&new.source)
        .bind(&new.media_type)
        .bind(&new.storage_id)
        .bind(&new.extension)
        .bind(&new.sha256)
        .bind(new.size)
        .bind(new.has_original)
        .bind(new.has_thumbnails)
        .bind(new.position)
        .bind(&new.lang)
        .bind(&new.alt_text)
        .execute(&mut *conn)
        .await?;
        Ok(MediaRow {
            resource,
            item_id: new.item_id,
            ingester: new.ingester.clone(),
            renderer: new.renderer.clone(),
            data: new.data.clone(),
            source: new.source.clone(),
            media_type: new.media_type.clone(),
            storage_id: new.storage_id.clone(),
            extension: new.extension.clone(),
            sha256: new.sha256.clone(),
            size: new.size,
            has_original: new.has_original,
            has_thumbnails: new.has_thumbnails,
            position: new.position,
            lang: new.lang.clone(),
            alt_text: new.alt_text.clone(),
        })
    }

    pub async fn get(&self, id: MediaId) -> Result<Option<MediaRow>, DbError> {
        let sql = format!("{MEDIA_COLUMNS} WHERE media.id = ?");
        let row = sqlx::query(&sql)
            .bind(bind_id(id))
            .fetch_optional(self.pool)
            .await?;
        row.as_ref().map(MediaRow::from_db_row::<DB>).transpose()
    }

    pub async fn update(&self, row: &MediaRow) -> Result<bool, DbError> {
        let mut tx = self.pool.begin().await?;
        if !update_resource_columns(&mut tx, &row.resource).await? {
            return Ok(false);
        }
        let data = row
            .data
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(database_json)?;
        sqlx::query(
            "UPDATE media
             SET item_id = ?, ingester = ?, renderer = ?, data = ?, source = ?, media_type = ?,
                 storage_id = ?, extension = ?, sha256 = ?, size = ?, has_original = ?,
                 has_thumbnails = ?, position = ?, lang = ?, alt_text = ?
             WHERE id = ?",
        )
        .bind(bind_id(row.item_id))
        .bind(&row.ingester)
        .bind(&row.renderer)
        .bind(data)
        .bind(&row.source)
        .bind(&row.media_type)
        .bind(&row.storage_id)
        .bind(&row.extension)
        .bind(&row.sha256)
        .bind(row.size)
        .bind(row.has_original)
        .bind(row.has_thumbnails)
        .bind(row.position)
        .bind(&row.lang)
        .bind(&row.alt_text)
        .bind(bind_id(row.resource.id))
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(true)
    }

    /// Delete the media row, its values and its resource row. Site blocks and items that point at
    /// the media keep their rows and lose the reference (`ON DELETE SET NULL`).
    pub async fn delete(&self, id: MediaId) -> Result<bool, DbError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM value WHERE resource_id = ?")
            .bind(bind_id(id))
            .execute(&mut *tx)
            .await?;
        let done = sqlx::query("DELETE FROM resource WHERE id = ?")
            .bind(bind_id(id))
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(DB::rows_affected(&done) > 0)
    }

    /// The `(storage_id, extension)` of every media row that names a stored file. A row with no
    /// `storage_id` (an oEmbed or HTML media) has no file and is left out.
    pub async fn stored_names(&self) -> Result<Vec<(String, Option<String>)>, DbError> {
        let rows =
            sqlx::query("SELECT storage_id, extension FROM media WHERE storage_id IS NOT NULL")
                .fetch_all(self.pool)
                .await?;
        rows.iter()
            .map(|row| {
                Ok((
                    DB::text(row, "storage_id")?,
                    DB::opt_text(row, "extension")?,
                ))
            })
            .collect()
    }

    /// The media that stores its files under `storage_id`, with its own visibility and its item's,
    /// so a file request can be answered without loading the whole row. The comparison follows the
    /// column's collation, as the unique key does.
    pub async fn file_owner(&self, storage_id: &str) -> Result<Option<MediaFileOwner>, DbError> {
        let row = sqlx::query(
            "SELECT media.id, media.extension, media.media_type, media.source,
                 media_resource.is_public AS media_is_public,
                 media_resource.owner_id AS media_owner_id,
                 item_resource.is_public AS item_is_public,
                 item_resource.owner_id AS item_owner_id
             FROM media
             JOIN resource media_resource ON media_resource.id = media.id
             JOIN resource item_resource ON item_resource.id = media.item_id
             WHERE media.storage_id = ?",
        )
        .bind(storage_id)
        .fetch_optional(self.pool)
        .await?;
        row.map(|row| {
            Ok(MediaFileOwner {
                media_id: DB::id(&row, "id")?,
                extension: DB::opt_text(&row, "extension")?,
                media_type: DB::opt_text(&row, "media_type")?,
                source: DB::opt_text(&row, "source")?,
                media_is_public: DB::boolean(&row, "media_is_public")?,
                media_owner_id: DB::opt_id(&row, "media_owner_id")?,
                item_is_public: DB::boolean(&row, "item_is_public")?,
                item_owner_id: DB::opt_id(&row, "item_owner_id")?,
            })
        })
        .transpose()
    }

    pub async fn list(
        &self,
        filter: &MediaFilter,
        paging: &Paging,
    ) -> Result<Vec<MediaRow>, DbError> {
        self.list_filtered(filter, &QueryFilter::default(), paging)
            .await
    }

    /// `list` with the advanced search parameters of `MediaAdapter::buildQuery`
    /// and its parent on top ([`QueryFilter`]).
    pub async fn list_filtered(
        &self,
        filter: &MediaFilter,
        query: &QueryFilter,
        paging: &Paging,
    ) -> Result<Vec<MediaRow>, DbError> {
        let mut statement = Statement::<DB>::new(MEDIA_COLUMNS);
        statement.push(" WHERE 1 = 1");
        if let Some(item_id) = filter.item_id {
            statement.push(" AND media.item_id = ");
            statement.bind(bind_id(item_id))?;
        }
        if let Some(ingester) = &filter.ingester {
            statement.push(" AND media.ingester = ");
            statement.bind(ingester)?;
        }
        if let Some(renderer) = &filter.renderer {
            statement.push(" AND media.renderer = ");
            statement.bind(renderer)?;
        }
        if let Some(media_type) = &filter.media_type {
            statement.push(" AND media.media_type = ");
            statement.bind(media_type)?;
        }
        if let Some(kinds) = &filter.kinds {
            super::query::push_media_kind(&mut statement, kinds)?;
        }
        if let Some(owner_id) = filter.owner_id {
            statement.push(" AND resource.owner_id = ");
            statement.bind(bind_id(owner_id))?;
        }
        if let Some(class_id) = filter.resource_class_id {
            statement.push(" AND resource.resource_class_id = ");
            statement.bind(bind_id(class_id))?;
        }
        if let Some(template_id) = filter.resource_template_id {
            statement.push(" AND resource.resource_template_id = ");
            statement.bind(bind_id(template_id))?;
        }
        if let Some(is_public) = filter.is_public {
            statement.push(" AND resource.is_public = ");
            statement.bind(is_public)?;
        }
        query.push_media(&mut statement)?;
        // Upstream orders media within an item by `position` (`Entity/Item.php`, `@OrderBy`); MySQL
        // sorts NULL first in ascending order, and so does SQLite, so a plain `position` keeps the
        // same order. The id breaks ties.
        statement.push(" ORDER BY media.item_id, media.position, media.id");
        push_paging(&mut statement, paging)?;
        let (sql, arguments) = statement.into_parts();
        let rows: Vec<DB::Row> = fetch_rows(&sql, arguments, self.pool).await?;
        rows.iter().map(MediaRow::from_db_row::<DB>).collect()
    }
}

fn database_json(error: serde_json::Error) -> DbError {
    DbError::Column {
        column: "data",
        reason: format!("cannot store as JSON: {error}"),
    }
}
