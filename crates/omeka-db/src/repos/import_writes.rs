//! The archive import's writes: each target and its `romeka_import_resource` row in one
//! transaction, so a rollback leaves neither and undo finds every committed target.
//!
//! The in-use checks are the questions undo asks before it removes a set, a template or a field
//! the import made: one `LIMIT 1` probe each, so a large archive answers as fast as an empty one.

use sqlx::{ColumnIndex, Decode, Encode, Executor, IntoArguments, Pool, Sqlite, Type};

use super::{ImportResourceRepo, ItemRepo, ItemSetRepo, MediaRepo, PropertyRepo, TemplateRepo};
use super::{bind_id, values::insert_value};
use crate::DbError;
use crate::backend::Backend;
use crate::ids::{ImportId, ItemSetId, PropertyId, TemplateId};
use crate::rows::{
    ImportTarget, ItemRow, ItemSetRow, MediaRow, NewItemSet, NewProperty, NewTemplate,
    NewTemplateProperty, NewWholeItem, PropertyRow, TemplateRow, ValueContent,
};

/// One media of an imported item with the values it carries. `media.item_id` is ignored: the
/// item is created in the same transaction.
#[derive(Debug, Clone, PartialEq)]
pub struct NewImportMedia {
    pub media: crate::rows::NewMedia,
    pub values: Vec<ValueContent>,
}

/// An imported item as committed.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportedItem {
    pub item: ItemRow,
    pub media: Vec<MediaRow>,
}

/// Writes for one import.
pub struct ImportWriteRepo<'a, DB: Backend = Sqlite> {
    pool: &'a Pool<DB>,
    import: ImportId,
}

impl<'a, DB: Backend> ImportWriteRepo<'a, DB>
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
    pub fn new(pool: &'a Pool<DB>, import: ImportId) -> Self {
        Self { pool, import }
    }

    /// An item set with its values.
    pub async fn create_item_set(
        &self,
        new: &NewItemSet,
        values: &[ValueContent],
    ) -> Result<ItemSetRow, DbError> {
        let mut tx = self.pool.begin().await?;
        let row = ItemSetRepo::<DB>::insert(&mut tx, new).await?;
        for content in values {
            insert_value(&mut tx, &content.on(row.resource.id)).await?;
        }
        ImportResourceRepo::<DB>::record(&mut *tx, self.import, ImportTarget::ItemSet(row.id()))
            .await?;
        tx.commit().await?;
        Ok(row)
    }

    /// A template with its fields. Each field's `resource_template_id` is ignored.
    pub async fn create_template(
        &self,
        new: &NewTemplate,
        fields: &[NewTemplateProperty],
    ) -> Result<TemplateRow, DbError> {
        let mut tx = self.pool.begin().await?;
        let row = TemplateRepo::<DB>::insert(&mut tx, new).await?;
        for field in fields {
            let field = NewTemplateProperty {
                resource_template_id: row.id,
                ..field.clone()
            };
            TemplateRepo::<DB>::insert_property(&mut tx, &field).await?;
        }
        ImportResourceRepo::<DB>::record(
            &mut *tx,
            self.import,
            ImportTarget::ResourceTemplate(row.id),
        )
        .await?;
        tx.commit().await?;
        Ok(row)
    }

    pub async fn create_property(&self, new: &NewProperty) -> Result<PropertyRow, DbError> {
        let mut tx = self.pool.begin().await?;
        let row = PropertyRepo::<DB>::insert(&mut tx, new).await?;
        ImportResourceRepo::<DB>::record(&mut *tx, self.import, ImportTarget::Property(row.id))
            .await?;
        tx.commit().await?;
        Ok(row)
    }

    /// An item with its set links and values, then its media with theirs, all recorded.
    pub async fn create_item(
        &self,
        item: &NewWholeItem,
        media: &[NewImportMedia],
    ) -> Result<ImportedItem, DbError> {
        let mut tx = self.pool.begin().await?;
        let row = ItemRepo::<DB>::insert_whole(&mut tx, item).await?;
        ImportResourceRepo::<DB>::record(&mut *tx, self.import, ImportTarget::Item(row.id()))
            .await?;
        let mut created = Vec::with_capacity(media.len());
        for entry in media {
            let new = crate::rows::NewMedia {
                item_id: row.id(),
                ..entry.media.clone()
            };
            let media_row = MediaRepo::<DB>::insert(&mut tx, &new).await?;
            for content in &entry.values {
                insert_value(&mut tx, &content.on(media_row.resource.id)).await?;
            }
            ImportResourceRepo::<DB>::record(
                &mut *tx,
                self.import,
                ImportTarget::Media(media_row.id()),
            )
            .await?;
            created.push(media_row);
        }
        tx.commit().await?;
        Ok(ImportedItem {
            item: row,
            media: created,
        })
    }

    /// Whether any item belongs to the set.
    pub async fn item_set_has_items(&self, id: ItemSetId) -> Result<bool, DbError> {
        self.any(
            "SELECT 1 FROM item_item_set WHERE item_set_id = ? LIMIT 1",
            bind_id(id),
        )
        .await
    }

    /// Whether any resource names the template.
    pub async fn template_in_use(&self, id: TemplateId) -> Result<bool, DbError> {
        self.any(
            "SELECT 1 FROM resource WHERE resource_template_id = ? LIMIT 1",
            bind_id(id),
        )
        .await
    }

    /// Whether any value is in the property, or any template lists it. Deleting it would cascade
    /// to both.
    pub async fn property_in_use(&self, id: PropertyId) -> Result<bool, DbError> {
        if self
            .any(
                "SELECT 1 FROM value WHERE property_id = ? LIMIT 1",
                bind_id(id),
            )
            .await?
        {
            return Ok(true);
        }
        self.any(
            "SELECT 1 FROM resource_template_property WHERE property_id = ? LIMIT 1",
            bind_id(id),
        )
        .await
    }

    async fn any(&self, sql: &'static str, id: i64) -> Result<bool, DbError> {
        let row = sqlx::query(sql).bind(id).fetch_optional(self.pool).await?;
        Ok(row.is_some())
    }
}
