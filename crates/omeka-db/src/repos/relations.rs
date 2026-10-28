//! The rows a page of resource documents reads, one statement per relation over the page's ids.
//!
//! Building one document at a time issued a query per relation per row, about a hundred for a
//! 25-row page, which the perf harness measured as the list endpoint's limiter
//! (`verification/perf/README.md`). Each method here reads one relation for every id at once, so a
//! page costs one statement per relation however many rows it holds. The API's `read/relations.rs`
//! groups the rows by id and walks the annotation levels; `tests/relations_batch.rs` there pins the
//! statement count. An empty id list returns no rows and runs no statement.
//!
//! The orders match the per-row repository reads each method batches: media as `MediaRepo::list`,
//! memberships as `ItemRepo::list_item_sets` and `list_sites`, values as `ValueRepo::list`, and
//! template properties in position order.

use sqlx::{ColumnIndex, Decode, Encode, Executor, IntoArguments, Pool, Sqlite, Type};

use super::value_visibility::{bind_title_visible, bind_value_visibility, title_property};
use super::{MediaVisibility, Statement, bind_id, fetch_rows};
use crate::DbError;
use crate::backend::Backend;
use crate::ids::{ItemId, ItemSetId, MediaId, PropertyId, ResourceId, SiteId, TemplateId};
use crate::rows::{AssetRow, MediaRow, ResourceRow, ValueRow};

/// The relations of a page of resources.
pub struct RelationRepo<'a, DB: Backend = Sqlite> {
    pool: &'a Pool<DB>,
}

impl<'a, DB: Backend> RelationRepo<'a, DB>
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
{
    pub fn new(pool: &'a Pool<DB>) -> Self {
        Self { pool }
    }

    /// Every media row of the items, by item, then position, then id.
    pub async fn media_of_items(&self, items: &[ItemId]) -> Result<Vec<MediaRow>, DbError> {
        let rows = self
            .rows_in(
                "SELECT resource.*, media.item_id, media.ingester, media.renderer, media.data, \
                 media.source, media.media_type, media.storage_id, media.extension, media.sha256, \
                 media.size, media.has_original, media.has_thumbnails, media.position, media.lang, \
                 media.alt_text \
                 FROM media JOIN resource ON resource.id = media.id \
                 WHERE media.item_id IN (",
                ") ORDER BY media.item_id, media.position, media.id",
                items.iter().copied().map(bind_id),
            )
            .await?;
        rows.iter().map(MediaRow::from_db_row::<DB>).collect()
    }

    /// The `(item, item set)` memberships of the items, by item then item set.
    pub async fn item_sets_of_items(
        &self,
        items: &[ItemId],
    ) -> Result<Vec<(ItemId, ItemSetId)>, DbError> {
        let rows = self
            .rows_in(
                "SELECT item_id, item_set_id FROM item_item_set WHERE item_id IN (",
                ") ORDER BY item_id, item_set_id",
                items.iter().copied().map(bind_id),
            )
            .await?;
        rows.iter()
            .map(|row| Ok((DB::id(row, "item_id")?, DB::id(row, "item_set_id")?)))
            .collect()
    }

    /// The `(item, site)` memberships of the items, by item then site.
    pub async fn sites_of_items(&self, items: &[ItemId]) -> Result<Vec<(ItemId, SiteId)>, DbError> {
        let rows = self
            .rows_in(
                "SELECT item_id, site_id FROM item_site WHERE item_id IN (",
                ") ORDER BY item_id, site_id",
                items.iter().copied().map(bind_id),
            )
            .await?;
        rows.iter()
            .map(|row| Ok((DB::id(row, "item_id")?, DB::id(row, "site_id")?)))
            .collect()
    }

    /// The first item of each item set by id, with that item's stored primary media.
    ///
    /// `ItemSetRepresentation::primaryMedia` reads the set's first item through `ItemRepo::list`,
    /// which orders by `resource.id` and applies no visibility filter, so the smallest member id
    /// is that item. A set with no member has no row.
    pub async fn first_items(
        &self,
        sets: &[ItemSetId],
    ) -> Result<Vec<(ItemSetId, ItemId, Option<MediaId>)>, DbError> {
        let rows = self
            .rows_in(
                "SELECT member.item_set_id AS item_set_id, item.id AS item_id, \
                 item.primary_media_id AS primary_media_id \
                 FROM (SELECT item_set_id, MIN(item_id) AS item_id FROM item_item_set \
                       WHERE item_set_id IN (",
                ") GROUP BY item_set_id) member \
                 JOIN item ON item.id = member.item_id \
                 ORDER BY member.item_set_id",
                sets.iter().copied().map(bind_id),
            )
            .await?;
        rows.iter()
            .map(|row| {
                Ok((
                    DB::id(row, "item_set_id")?,
                    DB::id(row, "item_id")?,
                    DB::opt_id(row, "primary_media_id")?,
                ))
            })
            .collect()
    }

    /// The assets the resources' `thumbnail_id` names.
    pub async fn thumbnail_assets(
        &self,
        resources: &[ResourceId],
    ) -> Result<Vec<AssetRow>, DbError> {
        let rows = self
            .rows_in(
                "SELECT * FROM asset WHERE id IN (SELECT thumbnail_id FROM resource \
                 WHERE thumbnail_id IS NOT NULL AND id IN (",
                "))",
                resources.iter().copied().map(bind_id),
            )
            .await?;
        rows.iter().map(AssetRow::from_db_row::<DB>).collect()
    }

    /// Every value on the resources that `visibility` may see, by resource then id.
    pub async fn values_of(
        &self,
        resources: &[ResourceId],
        visibility: MediaVisibility,
    ) -> Result<Vec<ValueRow>, DbError> {
        self.values_where(
            "resource_id",
            resources,
            visibility,
            " ORDER BY v.resource_id, v.id",
        )
        .await
    }

    /// Every value whose `value_resource_id` is one of the resources and that `visibility` may
    /// see, by id.
    pub async fn values_pointing_at(
        &self,
        resources: &[ResourceId],
        visibility: MediaVisibility,
    ) -> Result<Vec<ValueRow>, DbError> {
        self.values_where("value_resource_id", resources, visibility, " ORDER BY v.id")
            .await
    }

    async fn values_where(
        &self,
        column: &str,
        resources: &[ResourceId],
        visibility: MediaVisibility,
        order: &str,
    ) -> Result<Vec<ValueRow>, DbError> {
        if resources.is_empty() {
            return Ok(Vec::new());
        }
        let mut statement =
            Statement::<DB>::new(&format!("SELECT v.* FROM value v WHERE v.{column} IN ("));
        for (index, id) in resources.iter().enumerate() {
            if index > 0 {
                statement.push(", ");
            }
            statement.bind(bind_id(*id))?;
        }
        statement.push(")");
        bind_value_visibility(&mut statement, visibility, "v")?;
        statement.push(order);
        let (sql, arguments) = statement.into_parts();
        let rows: Vec<DB::Row> = fetch_rows(&sql, arguments, self.pool).await?;
        rows.iter().map(ValueRow::from_db_row::<DB>).collect()
    }

    /// The `resource` rows with these ids, in no particular order.
    pub async fn resources(&self, ids: &[ResourceId]) -> Result<Vec<ResourceRow>, DbError> {
        let rows = self
            .rows_in(
                "SELECT * FROM resource WHERE id IN (",
                ")",
                ids.iter().copied().map(bind_id),
            )
            .await?;
        rows.iter().map(ResourceRow::from_db_row::<DB>).collect()
    }

    /// The title each resource shows to `visibility` where its stored `title` came from a value
    /// `visibility` may not see: the first non-empty value of the title property it may see, or
    /// `None`. A resource whose stored title the caller may see is absent.
    ///
    /// `ResourceTitleHydrator` stores the first non-empty value of the template's title property
    /// (`dcterms:title` without one) whatever that value's visibility, and every upstream read
    /// prints that column. The port prints a title only from a value the caller may read.
    pub async fn hidden_titles(
        &self,
        ids: &[ResourceId],
        visibility: MediaVisibility,
    ) -> Result<Vec<(ResourceId, Option<String>)>, DbError> {
        if visibility == MediaVisibility::All {
            return Ok(Vec::new());
        }
        // The public site asks about every resource a browse filters, more ids than one
        // statement may bind.
        const IDS_PER_STATEMENT: usize = 1000;
        let mut shown = Vec::new();
        for chunk in ids.chunks(IDS_PER_STATEMENT) {
            let mut statement = Statement::<DB>::new(&format!(
                "SELECT r.id AS id, (SELECT v.value FROM value v WHERE v.resource_id = r.id \
                 AND v.value IS NOT NULL AND v.value <> '' AND v.property_id = {}",
                title_property("r")
            ));
            bind_value_visibility(&mut statement, visibility, "v")?;
            statement.push(
                " ORDER BY v.id LIMIT 1) AS shown FROM resource r \
                 WHERE r.title IS NOT NULL AND r.id IN (",
            );
            for (index, id) in chunk.iter().enumerate() {
                if index > 0 {
                    statement.push(", ");
                }
                statement.bind(bind_id(*id))?;
            }
            statement.push(") AND NOT (1 = 1");
            bind_title_visible(&mut statement, visibility, "r")?;
            statement.push(")");
            let (sql, arguments) = statement.into_parts();
            let rows: Vec<DB::Row> = fetch_rows(&sql, arguments, self.pool).await?;
            for row in &rows {
                shown.push((DB::id(row, "id")?, DB::opt_text(row, "shown")?));
            }
        }
        Ok(shown)
    }

    /// The title a resource shows to anyone: its stored title, unless that came from a private
    /// value. The fulltext index answers every caller, so it holds this title.
    pub async fn public_title(&self, row: &ResourceRow) -> Result<Option<String>, DbError> {
        let hidden = self
            .hidden_titles(&[row.id], MediaVisibility::Actor(None))
            .await?;
        Ok(match hidden.into_iter().next() {
            Some((_, shown)) => shown,
            None => row.title.clone(),
        })
    }

    /// The `(template, property)` pairs of the templates, by template, then position, then id.
    pub async fn template_properties(
        &self,
        templates: &[TemplateId],
    ) -> Result<Vec<(TemplateId, PropertyId)>, DbError> {
        let rows = self
            .rows_in(
                "SELECT resource_template_id, property_id FROM resource_template_property \
                 WHERE resource_template_id IN (",
                ") ORDER BY resource_template_id, position, id",
                templates.iter().copied().map(bind_id),
            )
            .await?;
        rows.iter()
            .map(|row| {
                Ok((
                    DB::id(row, "resource_template_id")?,
                    DB::id(row, "property_id")?,
                ))
            })
            .collect()
    }

    /// Run `head`, the ids as a `?` list, then `tail`. No ids, no statement.
    async fn rows_in(
        &self,
        head: &str,
        tail: &str,
        ids: impl ExactSizeIterator<Item = i64>,
    ) -> Result<Vec<DB::Row>, DbError> {
        if ids.len() == 0 {
            return Ok(Vec::new());
        }
        let mut statement = Statement::<DB>::new(head);
        for (index, id) in ids.enumerate() {
            if index > 0 {
                statement.push(", ");
            }
            statement.bind(id)?;
        }
        statement.push(tail);
        let (sql, arguments) = statement.into_parts();
        fetch_rows(&sql, arguments, self.pool).await
    }
}
