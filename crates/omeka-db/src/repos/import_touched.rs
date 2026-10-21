//! What undo reads to tell whether anything but an import changed a resource after the import
//! wrote it (`docs/import-v2.md`, "Lifecycle and the ledger", "Touched").
//!
//! Each read answers one fact; the rule that combines them lives with undo in the api, which also
//! knows which resources are the import's own.

use chrono::NaiveDateTime;
use sqlx::{ColumnIndex, Decode, Encode, Executor, IntoArguments, Pool, Sqlite, Type};

use super::bind_id;
use crate::DbError;
use crate::backend::Backend;
use crate::ids::{ImportId, ItemId, ItemSetId, MediaId, PropertyId, ResourceId, TemplateId};
use crate::rows::{ImportTarget, resource_type};
use crate::time;

/// A resource that points at another one, with its own last change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pointer {
    pub resource: ResourceId,
    pub modified: Option<NaiveDateTime>,
}

/// Whether a recorded target is still there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Presence {
    Gone,
    /// There; `modified` is the resource's, `None` for a template or a property.
    Here {
        modified: Option<NaiveDateTime>,
    },
}

/// What names a property.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PropertyUsers {
    pub resources: Vec<ResourceId>,
    pub templates: Vec<TemplateId>,
}

/// The reads of the touched test.
pub struct ImportTouchRepo<'a, DB: Backend = Sqlite> {
    pool: &'a Pool<DB>,
}

impl<'a, DB: Backend> ImportTouchRepo<'a, DB>
where
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
    usize: ColumnIndex<DB::Row>,
    for<'x> i64: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> String: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> &'x str: Encode<'x, DB> + Type<DB> + ColumnIndex<DB::Row>,
{
    pub fn new(pool: &'a Pool<DB>) -> Self {
        Self { pool }
    }

    /// Whether `target` exists, as a resource of its kind or as a template or property row.
    pub async fn presence(&self, target: ImportTarget) -> Result<Presence, DbError> {
        let (id, kind) = match target {
            ImportTarget::Item(id) => (id.0, resource_type::ITEM),
            ImportTarget::Media(id) => (id.0, resource_type::MEDIA),
            ImportTarget::ItemSet(id) => (id.0, resource_type::ITEM_SET),
            ImportTarget::ResourceTemplate(id) => {
                return self.row_presence("resource_template", id.0).await;
            }
            ImportTarget::Property(id) => return self.row_presence("property", id.0).await,
        };
        let row = sqlx::query("SELECT resource_type, modified FROM resource WHERE id = ?")
            .bind(bind_id(id))
            .fetch_optional(self.pool)
            .await?;
        let Some(row) = row else {
            return Ok(Presence::Gone);
        };
        if DB::text(&row, "resource_type")? != kind {
            return Ok(Presence::Gone);
        }
        Ok(Presence::Here {
            modified: DB::opt_datetime(&row, "modified")?,
        })
    }

    async fn row_presence(&self, table: &'static str, id: u64) -> Result<Presence, DbError> {
        let found = sqlx::query(&format!("SELECT 1 FROM {table} WHERE id = ?"))
            .bind(bind_id(id))
            .fetch_optional(self.pool)
            .await?;
        Ok(match found {
            Some(_) => Presence::Here { modified: None },
            None => Presence::Gone,
        })
    }

    /// The resources whose values link to `target`, each once.
    pub async fn linkers(&self, target: ResourceId) -> Result<Vec<Pointer>, DbError> {
        self.pointers(
            "SELECT DISTINCT value.resource_id AS id, resource.modified AS modified FROM value
             JOIN resource ON resource.id = value.resource_id
             WHERE value.value_resource_id = ? ORDER BY value.resource_id",
            target.0,
        )
        .await
    }

    /// The items in an item set.
    pub async fn set_members(&self, set: ItemSetId) -> Result<Vec<Pointer>, DbError> {
        self.pointers(
            "SELECT item_item_set.item_id AS id, resource.modified AS modified FROM item_item_set
             JOIN resource ON resource.id = item_item_set.item_id
             WHERE item_item_set.item_set_id = ? ORDER BY item_item_set.item_id",
            set.0,
        )
        .await
    }

    async fn pointers(&self, sql: &'static str, id: u64) -> Result<Vec<Pointer>, DbError> {
        let rows = sqlx::query(sql)
            .bind(bind_id(id))
            .fetch_all(self.pool)
            .await?;
        rows.iter()
            .map(|row| {
                Ok(Pointer {
                    resource: DB::id(row, "id")?,
                    modified: DB::opt_datetime(row, "modified")?,
                })
            })
            .collect()
    }

    /// An item's media.
    pub async fn media_of(&self, item: ItemId) -> Result<Vec<MediaId>, DbError> {
        let rows = sqlx::query("SELECT id FROM media WHERE item_id = ? ORDER BY id")
            .bind(bind_id(item))
            .fetch_all(self.pool)
            .await?;
        rows.iter().map(|row| DB::id(row, "id")).collect()
    }

    /// The other imports whose ledger holds `target` with a written time after `after`.
    pub async fn later_imports(
        &self,
        import: ImportId,
        target: ImportTarget,
        after: NaiveDateTime,
    ) -> Result<Vec<ImportId>, DbError> {
        let rows = sqlx::query(
            "SELECT import_id FROM romeka_import_resource
             WHERE kind = ? AND target_id = ? AND import_id <> ? AND written > ?
             ORDER BY import_id",
        )
        .bind(target.kind().as_str())
        .bind(target.raw_id())
        .bind(bind_id(import))
        .bind(time::format(after))
        .fetch_all(self.pool)
        .await?;
        rows.iter().map(|row| DB::id(row, "import_id")).collect()
    }

    /// The resources that name the template.
    pub async fn template_users(&self, template: TemplateId) -> Result<Vec<ResourceId>, DbError> {
        let rows =
            sqlx::query("SELECT id FROM resource WHERE resource_template_id = ? ORDER BY id")
                .bind(bind_id(template))
                .fetch_all(self.pool)
                .await?;
        rows.iter().map(|row| DB::id(row, "id")).collect()
    }

    /// What names the property: resources through their values, and templates through their
    /// fields or their title and description.
    pub async fn property_users(&self, property: PropertyId) -> Result<PropertyUsers, DbError> {
        let valued = sqlx::query(
            "SELECT DISTINCT resource_id AS id FROM value WHERE property_id = ? ORDER BY resource_id",
        )
        .bind(bind_id(property))
        .fetch_all(self.pool)
        .await?;
        let templates = sqlx::query(
            "SELECT DISTINCT id FROM (
                 SELECT resource_template_id AS id FROM resource_template_property
                 WHERE property_id = ?
                 UNION SELECT id FROM resource_template WHERE title_property_id = ?
                 UNION SELECT id FROM resource_template WHERE description_property_id = ?
             ) AS users ORDER BY id",
        )
        .bind(bind_id(property))
        .bind(bind_id(property))
        .bind(bind_id(property))
        .fetch_all(self.pool)
        .await?;
        Ok(PropertyUsers {
            resources: valued
                .iter()
                .map(|row| DB::id(row, "id"))
                .collect::<Result<_, _>>()?,
            templates: templates
                .iter()
                .map(|row| DB::id(row, "id"))
                .collect::<Result<_, _>>()?,
        })
    }
}
