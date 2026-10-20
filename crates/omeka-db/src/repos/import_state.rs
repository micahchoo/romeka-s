//! The before-image of a resource an import changes, and the restore that puts it back
//! (`docs/import-v2.md`, "Lifecycle and the ledger", "Undo").
//!
//! A restore writes the image's state over what is there now. It never makes a media again: a
//! media the image lists that is gone since stays gone, and a media the image does not list (one
//! the import added) is left for undo to delete. A value, set, site, template, class or owner the
//! image names that is gone since is skipped, as the database cascade would have removed it.

use sqlx::{ColumnIndex, Decode, Encode, Executor, IntoArguments, Type};

use super::import_writes::ImportWrites;
use super::resources::{insert_resource, update_resource_columns};
use super::values::insert_value;
use super::{bind_id, new_id};
use crate::DbError;
use crate::backend::Backend;
use crate::ids::{AnnotationId, ItemId, ItemSetId, MediaId, ResourceId};
use crate::rows::{
    BeforeImage, ImageMemberships, ImageValue, ImportResource, NewResource, ValueContent,
    resource_type,
};

impl<'t, DB: Backend> ImportWrites<'t, DB>
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
    /// The resource's stored state, or `None` when no resource of that kind has the id.
    pub async fn before_image(
        &mut self,
        target: ImportResource,
    ) -> Result<Option<BeforeImage>, DbError> {
        let Some(row) = self.resource_row(target.id()).await? else {
            return Ok(None);
        };
        if row.resource_type != kind_of(target) {
            return Ok(None);
        }
        let memberships = match target {
            ImportResource::Item(id) => ImageMemberships::Item {
                item_sets: self.ids("SELECT item_set_id FROM item_item_set WHERE item_id = ? ORDER BY item_set_id", id.0).await?,
                sites: self.ids("SELECT site_id FROM item_site WHERE item_id = ? ORDER BY site_id", id.0).await?,
                media_order: self.media_order(id).await?,
                primary_media: self.primary_media(id).await?,
            },
            ImportResource::Media(_) => ImageMemberships::Media,
            ImportResource::ItemSet(id) => ImageMemberships::ItemSet {
                sites: self.set_positions(id).await?,
            },
        };
        Ok(Some(BeforeImage {
            values: self.image_values(target.id()).await?,
            template: row.resource_template_id,
            class: row.resource_class_id,
            owner: row.owner_id,
            is_public: row.is_public,
            memberships,
        }))
    }

    /// Put the resource back as `image` says. False when the resource is gone. An image of
    /// another kind is refused: the ledger stored it under this target, so it is corrupt.
    pub async fn restore(
        &mut self,
        target: ImportResource,
        image: &BeforeImage,
    ) -> Result<bool, DbError> {
        let fits = matches!(
            (target, &image.memberships),
            (ImportResource::Item(_), ImageMemberships::Item { .. })
                | (ImportResource::Media(_), ImageMemberships::Media)
                | (ImportResource::ItemSet(_), ImageMemberships::ItemSet { .. })
        );
        if !fits {
            return Err(DbError::Column {
                column: "image",
                reason: format!("before-image of another kind than {target:?}"),
            });
        }
        let Some(mut row) = self.resource_row(target.id()).await? else {
            return Ok(false);
        };
        if row.resource_type != kind_of(target) {
            return Ok(false);
        }
        self.delete_values(target.id(), None).await?;
        self.write_image_values(target.id(), &image.values).await?;
        row.resource_template_id = self.kept("resource_template", image.template).await?;
        row.resource_class_id = self.kept("resource_class", image.class).await?;
        row.owner_id = self.kept("user", image.owner).await?;
        row.is_public = image.is_public;
        update_resource_columns(self.conn, &row).await?;
        match (target, &image.memberships) {
            (
                ImportResource::Item(id),
                ImageMemberships::Item {
                    item_sets,
                    sites,
                    media_order,
                    primary_media,
                },
            ) => {
                let item_sets = self.kept_all("item_set", item_sets).await?;
                self.retain_item_sets(id, &item_sets).await?;
                for set in &item_sets {
                    sqlx::query(DB::insert_item_item_set())
                        .bind(bind_id(id))
                        .bind(bind_id(*set))
                        .execute(&mut *self.conn)
                        .await?;
                }
                let sites = self.kept_all("site", sites).await?;
                self.retain_item_sites(id, &sites).await?;
                self.add_item_sites(id, &sites).await?;
                self.restore_media_order(id, media_order).await?;
                let current = self.media_order(id).await?;
                let primary = primary_media.filter(|media| current.contains(media));
                self.set_primary_media(id, primary).await?;
            }
            (ImportResource::ItemSet(id), ImageMemberships::ItemSet { sites }) => {
                let mut kept = Vec::new();
                for (site, position) in sites {
                    if self.exists("site", site.0).await? {
                        kept.push((*site, *position));
                    }
                }
                let keep: Vec<_> = kept.iter().map(|(site, _)| *site).collect();
                self.retain_set_sites(id, &keep).await?;
                for (site, position) in kept {
                    self.put_set_in_site(id, site, position).await?;
                }
            }
            _ => {}
        }
        self.settle(target).await?;
        Ok(true)
    }

    /// Recompute the title from the values and refresh the index rows the write changed. The
    /// title follows `ResourceTitleHydrator`: the first non-empty text of the template's title
    /// property, else of `dcterms:title`. A link has no text.
    pub(super) async fn settle(&mut self, target: ImportResource) -> Result<(), DbError> {
        let id = target.id();
        let template_title: Option<i64> = sqlx::query_scalar::<DB, Option<i64>>(
            "SELECT resource_template.title_property_id FROM resource
             JOIN resource_template ON resource_template.id = resource.resource_template_id
             WHERE resource.id = ?",
        )
        .bind(bind_id(id))
        .fetch_optional(&mut *self.conn)
        .await?
        .flatten();
        let property = match template_title {
            Some(property) => Some(property),
            None => {
                sqlx::query_scalar::<DB, i64>(
                    "SELECT property.id FROM property
                 JOIN vocabulary ON vocabulary.id = property.vocabulary_id
                 WHERE vocabulary.prefix = 'dcterms' AND property.local_name = 'title'",
                )
                .fetch_optional(&mut *self.conn)
                .await?
            }
        };
        let title: Option<String> = match property {
            Some(property) => sqlx::query_scalar::<DB, Option<String>>(
                "SELECT value FROM value
                     WHERE resource_id = ? AND property_id = ? AND type NOT LIKE 'resource%'
                       AND value IS NOT NULL AND value <> ''
                     ORDER BY id LIMIT 1",
            )
            .bind(bind_id(id))
            .bind(property)
            .fetch_optional(&mut *self.conn)
            .await?
            .flatten(),
            None => None,
        };
        sqlx::query("UPDATE resource SET title = ? WHERE id = ?")
            .bind(title)
            .bind(bind_id(id))
            .execute(&mut *self.conn)
            .await?;
        self.reindex(target).await
    }

    /// A resource's values as an image keeps them, each with its annotation's values.
    async fn image_values(&mut self, id: ResourceId) -> Result<Vec<ImageValue>, DbError> {
        let mut values = Vec::new();
        for row in self.values_of(id).await? {
            let annotation = match row.value_annotation_id {
                Some(annotation) => Box::pin(self.image_values(annotation.as_resource())).await?,
                None => Vec::new(),
            };
            values.push(ImageValue {
                content: ValueContent {
                    property_id: row.property_id,
                    value_resource_id: row.value_resource_id,
                    value_annotation_id: None,
                    r#type: row.r#type,
                    lang: row.lang,
                    value: row.value,
                    uri: row.uri,
                    is_public: row.is_public,
                },
                annotation,
            });
        }
        Ok(values)
    }

    /// Write an image's values in order, making each annotation again. A value whose property or
    /// linked resource is gone is skipped.
    async fn write_image_values(
        &mut self,
        id: ResourceId,
        values: &[ImageValue],
    ) -> Result<(), DbError> {
        for value in values {
            let content = &value.content;
            if !self.exists("property", content.property_id.0).await? {
                continue;
            }
            if let Some(linked) = content.value_resource_id
                && !self.exists("resource", linked.0).await?
            {
                continue;
            }
            let mut new = content.on(id);
            if !value.annotation.is_empty() {
                let annotation = insert_resource::<DB>(
                    self.conn,
                    &NewResource::default(),
                    resource_type::VALUE_ANNOTATION,
                )
                .await?
                .id;
                sqlx::query("INSERT INTO value_annotation (id) VALUES (?)")
                    .bind(bind_id(annotation))
                    .execute(&mut *self.conn)
                    .await?;
                Box::pin(self.write_image_values(annotation, &value.annotation)).await?;
                new.value_annotation_id = Some(AnnotationId(annotation.0));
            }
            insert_value(self.conn, &new).await?;
        }
        Ok(())
    }

    /// The image's media keep their order, numbered from 1; a media the image does not list
    /// follows them in its current order.
    async fn restore_media_order(
        &mut self,
        item: ItemId,
        order: &[MediaId],
    ) -> Result<(), DbError> {
        let current = self.media_order(item).await?;
        let listed = order.iter().filter(|media| current.contains(media));
        let rest = current.iter().filter(|media| !order.contains(media));
        for (index, media) in listed.chain(rest).enumerate() {
            sqlx::query("UPDATE media SET position = ? WHERE id = ?")
                .bind(index as i64 + 1)
                .bind(bind_id(*media))
                .execute(&mut *self.conn)
                .await?;
        }
        Ok(())
    }

    pub(super) async fn media_order(&mut self, item: ItemId) -> Result<Vec<MediaId>, DbError> {
        self.ids(
            "SELECT id FROM media WHERE item_id = ? ORDER BY position, id",
            item.0,
        )
        .await
    }

    async fn primary_media(&mut self, item: ItemId) -> Result<Option<MediaId>, DbError> {
        let raw: Option<i64> =
            sqlx::query_scalar::<DB, Option<i64>>("SELECT primary_media_id FROM item WHERE id = ?")
                .bind(bind_id(item))
                .fetch_optional(&mut *self.conn)
                .await?
                .flatten();
        raw.map(|raw| Ok(MediaId(new_id(raw)?))).transpose()
    }

    async fn set_positions(
        &mut self,
        set: ItemSetId,
    ) -> Result<Vec<(crate::ids::SiteId, Option<i64>)>, DbError> {
        let rows: Vec<(i64, Option<i64>)> = sqlx::query_as::<DB, (i64, Option<i64>)>(
            "SELECT site_id, position FROM site_item_set WHERE item_set_id = ? ORDER BY site_id",
        )
        .bind(bind_id(set))
        .fetch_all(&mut *self.conn)
        .await?;
        rows.into_iter()
            .map(|(site, position)| Ok((crate::ids::SiteId(new_id(site)?), position)))
            .collect()
    }

    /// One id column of the rows a statement reads, binding one id.
    async fn ids<T: From<u64>>(&mut self, sql: &'static str, id: u64) -> Result<Vec<T>, DbError> {
        let raw: Vec<i64> = sqlx::query_scalar::<DB, i64>(sql)
            .bind(bind_id(id))
            .fetch_all(&mut *self.conn)
            .await?;
        raw.into_iter()
            .map(|raw| Ok(T::from(new_id(raw)?)))
            .collect()
    }

    async fn exists(&mut self, table: &'static str, id: u64) -> Result<bool, DbError> {
        let sql = format!("SELECT 1 FROM {table} WHERE id = ?");
        let found = sqlx::query(&sql)
            .bind(bind_id(id))
            .fetch_optional(&mut *self.conn)
            .await?;
        Ok(found.is_some())
    }

    /// The id when its row still exists, else none, as `ON DELETE SET NULL` would leave it.
    async fn kept<T: Copy + Into<u64>>(
        &mut self,
        table: &'static str,
        id: Option<T>,
    ) -> Result<Option<T>, DbError> {
        match id {
            Some(id) if self.exists(table, id.into()).await? => Ok(Some(id)),
            _ => Ok(None),
        }
    }

    async fn kept_all<T: Copy + Into<u64>>(
        &mut self,
        table: &'static str,
        ids: &[T],
    ) -> Result<Vec<T>, DbError> {
        let mut kept = Vec::new();
        for id in ids {
            if self.exists(table, (*id).into()).await? {
                kept.push(*id);
            }
        }
        Ok(kept)
    }
}

fn kind_of(target: ImportResource) -> &'static str {
    match target {
        ImportResource::Item(_) => resource_type::ITEM,
        ImportResource::Media(_) => resource_type::MEDIA,
        ImportResource::ItemSet(_) => resource_type::ITEM_SET,
    }
}
