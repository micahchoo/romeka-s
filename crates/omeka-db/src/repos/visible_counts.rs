//! Counts that stand for a list: the items of each item set, the resources whose values point at
//! one resource, and the items of one template that fill each property, each kept to the rows a
//! caller may see.
//!
//! A count here must equal the `totalResults` of the list it opens for the same caller, so every
//! counted row passes the same visibility the list applies ([`super::resource_reads`]'s
//! `push_visibility`, which is `Actor::sees_resource`), and every counted value passes
//! `ValueVisibilityFilter` ([`super::value_visibility`]). `omeka-server/tests/place_counts.rs` holds
//! each count against its list's total.

use sqlx::{ColumnIndex, Decode, Encode, Executor, IntoArguments, Type};

use super::resource_reads::{push_list, push_visibility};
use super::resources::ITEM_FROM;
use super::value_visibility::push_value_visibility;
use super::{ItemRepo, ItemSetRepo, MediaVisibility, ValueRepo};
use crate::DbError;
use crate::backend::Backend;
use crate::ids::{ItemSetId, PropertyId, ResourceId, TemplateId};

/// The items one item set holds, as the caller may see them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetItemCount {
    pub item_set: ItemSetId,
    pub items: i64,
}

/// The resources of one kind whose values on one property point at a resource.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkCount {
    /// `resource.resource_type` of the linking resources (`rows::resource_type`).
    pub resource_type: String,
    pub property: PropertyId,
    /// Distinct linking resources, not values: a list counts each resource once.
    pub resources: i64,
}

/// The items of one template, and how many of them fill each property.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldCounts {
    pub total: i64,
    /// One row per property at least one counted item has a value for, in property id order. A
    /// property absent here is filled by none.
    pub fields: Vec<FieldCount>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FieldCount {
    pub property: PropertyId,
    /// Distinct items, not values: a list counts each item once.
    pub items: i64,
}

impl<DB: Backend> ItemRepo<'_, DB>
where
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
    usize: ColumnIndex<DB::Row>,
    for<'x> i64: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> &'x str: ColumnIndex<DB::Row>,
{
    /// The items of `template` that `visibility` keeps, and per property how many of them have a
    /// value for it.
    ///
    /// A property's count is the `ex` property search (`query.rs`, `MatchKind::Exists`) grouped: it
    /// counts only the values `visibility` may see, as that search does.
    pub async fn field_counts(
        &self,
        template: TemplateId,
        visibility: MediaVisibility,
    ) -> Result<FieldCounts, DbError> {
        let mut sql =
            format!("SELECT COUNT(*) AS total{ITEM_FROM} WHERE resource.resource_template_id = ?");
        let mut binds = vec![template.0 as i64];
        push_visibility(&mut sql, &mut binds, visibility, "resource");
        let mut query = sqlx::query::<DB>(&sql);
        for bind in &binds {
            query = query.bind(*bind);
        }
        let total = DB::int(&query.fetch_one(self.pool).await?, "total")?;

        let table = DB::quote_identifier("value");
        let mut sql = format!(
            "SELECT v.property_id AS property_id, COUNT(DISTINCT resource.id) AS items \
             FROM {table} v INNER JOIN item ON item.id = v.resource_id \
             INNER JOIN resource ON resource.id = item.id \
             WHERE resource.resource_template_id = ?"
        );
        let mut binds = vec![template.0 as i64];
        push_visibility(&mut sql, &mut binds, visibility, "resource");
        push_value_visibility(&mut sql, &mut binds, visibility, "v");
        sql.push_str(" GROUP BY v.property_id ORDER BY v.property_id");
        let mut query = sqlx::query::<DB>(&sql);
        for bind in &binds {
            query = query.bind(*bind);
        }
        let fields = query
            .fetch_all(self.pool)
            .await?
            .iter()
            .map(|row| {
                Ok(FieldCount {
                    property: DB::id(row, "property_id")?,
                    items: DB::int(row, "items")?,
                })
            })
            .collect::<Result<_, DbError>>()?;
        Ok(FieldCounts { total, fields })
    }
}

impl<DB: Backend> ItemSetRepo<'_, DB>
where
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
    usize: ColumnIndex<DB::Row>,
    for<'x> i64: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> &'x str: ColumnIndex<DB::Row>,
{
    /// The item count of each named set that `visibility` keeps, counting only the items it keeps,
    /// in set id order. A set it does not keep, or that does not exist, is absent.
    pub async fn item_counts(
        &self,
        ids: &[ItemSetId],
        sets: MediaVisibility,
        items: MediaVisibility,
    ) -> Result<Vec<SetItemCount>, DbError> {
        let mut sql = String::from(
            "SELECT item_set.id AS id, (SELECT COUNT(*) FROM item_item_set \
             INNER JOIN resource ON resource.id = item_item_set.item_id \
             WHERE item_item_set.item_set_id = item_set.id",
        );
        let mut binds = Vec::new();
        push_visibility(&mut sql, &mut binds, items, "resource");
        sql.push_str(
            ") AS items FROM item_set INNER JOIN resource set_resource \
             ON set_resource.id = item_set.id WHERE item_set.id IN (",
        );
        push_list(&mut sql, &mut binds, ids.iter().map(|id| id.0 as i64));
        sql.push(')');
        push_visibility(&mut sql, &mut binds, sets, "set_resource");
        sql.push_str(" ORDER BY item_set.id");
        let mut query = sqlx::query::<DB>(&sql);
        for bind in &binds {
            query = query.bind(*bind);
        }
        let rows = query.fetch_all(self.pool).await?;
        rows.iter()
            .map(|row| {
                Ok(SetItemCount {
                    item_set: DB::id(row, "id")?,
                    items: DB::int(row, "items")?,
                })
            })
            .collect()
    }
}

impl<DB: Backend> ValueRepo<'_, DB>
where
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
    usize: ColumnIndex<DB::Row>,
    for<'x> i64: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> String: Decode<'x, DB> + Type<DB>,
    for<'x> &'x str: ColumnIndex<DB::Row>,
{
    /// The resources that `visibility` keeps whose values point at `target`, counted per
    /// (resource type, property), ordered by both.
    ///
    /// This is the `res` property search (`query.rs`, `MatchKind::Resource`) grouped: it counts
    /// only the values `visibility` may see, as that search does.
    pub async fn link_counts(
        &self,
        target: ResourceId,
        visibility: MediaVisibility,
    ) -> Result<Vec<LinkCount>, DbError> {
        let table = DB::quote_identifier("value");
        let mut sql = format!(
            "SELECT resource.resource_type AS resource_type, v.property_id AS property_id, \
             COUNT(DISTINCT resource.id) AS resources FROM {table} v \
             INNER JOIN resource ON resource.id = v.resource_id WHERE v.value_resource_id = ?"
        );
        let mut binds = vec![target.0 as i64];
        push_visibility(&mut sql, &mut binds, visibility, "resource");
        push_value_visibility(&mut sql, &mut binds, visibility, "v");
        sql.push_str(
            " GROUP BY resource.resource_type, v.property_id \
             ORDER BY resource.resource_type, v.property_id",
        );
        let mut query = sqlx::query::<DB>(&sql);
        for bind in &binds {
            query = query.bind(*bind);
        }
        let rows = query.fetch_all(self.pool).await?;
        rows.iter()
            .map(|row| {
                Ok(LinkCount {
                    resource_type: DB::text(row, "resource_type")?,
                    property: DB::id(row, "property_id")?,
                    resources: DB::int(row, "resources")?,
                })
            })
            .collect()
    }
}
