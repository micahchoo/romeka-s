//! List reads over resources that the API serves straight from SQL: one window of items with the
//! total its filter matches, and every resource of one kind.
//!
//! Upstream `AbstractAdapter::search` counts the filter and then selects the page. The window
//! does both in one statement: `COUNT(*) OVER ()` counts the matching rows before `LIMIT`
//! applies, so a page stays one statement however many rows match. Both engines run window
//! functions (MySQL 8.0, MariaDB 10.2).

use sqlx::{ColumnIndex, Decode, Encode, Executor, IntoArguments, Type};

use super::resources::{ITEM_FROM, ITEM_PROJECTION};
use super::{ItemFilter, ItemRepo, MediaVisibility, ResourceRepo};
use crate::DbError;
use crate::backend::Backend;
use crate::ids::ItemSetId;
use crate::rows::{ItemRow, ResourceRow};

/// The filter of one item window.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ItemWindow {
    /// `id`, the base filter `AbstractEntityAdapter::buildBaseQuery` renders first. `None` adds
    /// nothing; an empty list matches no item.
    pub ids: Option<Vec<i64>>,
    pub filter: ItemFilter,
    /// `item_set_id[]`: items in any of these sets.
    pub item_set_ids: Vec<ItemSetId>,
    /// Who reads: every row for `view-all`, else public rows and the caller's own, the rule
    /// `Actor::sees_resource` applies.
    pub visibility: MediaVisibility,
}

/// One window of items and the number of items the filter matches.
#[derive(Debug, Clone, PartialEq)]
pub struct ItemPage {
    pub total: i64,
    pub rows: Vec<ItemRow>,
}

impl<DB: Backend> ItemRepo<'_, DB>
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
    /// The items of one window in id order, and the total the filter matches.
    ///
    /// A window past the last row returns no row to carry the total, so only that case runs a
    /// second statement to count. An empty first window means the filter matched nothing.
    pub async fn window(
        &self,
        window: &ItemWindow,
        limit: i64,
        offset: i64,
    ) -> Result<ItemPage, DbError> {
        let (where_sql, binds) = predicate(window);
        let sql = format!(
            "{ITEM_PROJECTION}, COUNT(*) OVER () AS total{ITEM_FROM}{where_sql} \
             ORDER BY resource.id LIMIT ? OFFSET ?"
        );
        let mut query = sqlx::query::<DB>(&sql);
        for bind in &binds {
            query = query.bind(*bind);
        }
        let rows = query.bind(limit).bind(offset).fetch_all(self.pool).await?;
        let total = match rows.first() {
            Some(row) => DB::int(row, "total")?,
            None if offset == 0 => 0,
            None => {
                let sql = format!("SELECT COUNT(*){ITEM_FROM}{where_sql}");
                let mut query = sqlx::query_scalar::<DB, i64>(&sql);
                for bind in &binds {
                    query = query.bind(*bind);
                }
                query.fetch_one(self.pool).await?
            }
        };
        let rows = rows
            .iter()
            .map(ItemRow::from_db_row::<DB>)
            .collect::<Result<_, _>>()?;
        Ok(ItemPage { total, rows })
    }
}

impl<DB: Backend> ResourceRepo<'_, DB>
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
    /// Every resource of one kind (`rows::resource_type`), in id order.
    pub async fn list_of_type(&self, resource_type: &str) -> Result<Vec<ResourceRow>, DbError> {
        let rows = sqlx::query::<DB>("SELECT * FROM resource WHERE resource_type = ? ORDER BY id")
            .bind(resource_type)
            .fetch_all(self.pool)
            .await?;
        rows.iter().map(ResourceRow::from_db_row::<DB>).collect()
    }
}

/// The window's `WHERE`, and the values it binds in clause order.
fn predicate(window: &ItemWindow) -> (String, Vec<i64>) {
    let mut sql = String::from(" WHERE 1 = 1");
    let mut binds: Vec<i64> = Vec::new();
    if let Some(ids) = &window.ids {
        sql.push_str(" AND resource.id IN (");
        push_list(&mut sql, &mut binds, ids.iter().copied());
        sql.push(')');
    }
    let filter = &window.filter;
    if let Some(owner_id) = filter.owner_id {
        sql.push_str(" AND resource.owner_id = ?");
        binds.push(owner_id.0 as i64);
    }
    if let Some(class_id) = filter.resource_class_id {
        sql.push_str(" AND resource.resource_class_id = ?");
        binds.push(class_id.0 as i64);
    }
    if let Some(template_id) = filter.resource_template_id {
        sql.push_str(" AND resource.resource_template_id = ?");
        binds.push(template_id.0 as i64);
    }
    if let Some(item_set_id) = filter.item_set_id {
        sql.push_str(
            " AND EXISTS (SELECT 1 FROM item_item_set WHERE item_item_set.item_id = resource.id \
             AND item_item_set.item_set_id = ?)",
        );
        binds.push(item_set_id.0 as i64);
    }
    if let Some(is_public) = filter.is_public {
        sql.push_str(" AND resource.is_public = ?");
        binds.push(i64::from(is_public));
    }
    if !window.item_set_ids.is_empty() {
        sql.push_str(
            " AND EXISTS (SELECT 1 FROM item_item_set WHERE item_item_set.item_id = resource.id \
             AND item_item_set.item_set_id IN (",
        );
        push_list(
            &mut sql,
            &mut binds,
            window.item_set_ids.iter().map(|id| id.0 as i64),
        );
        sql.push_str("))");
    }
    push_visibility(&mut sql, &mut binds, window.visibility, "resource");
    (sql, binds)
}

/// `Actor::sees_resource` as SQL on one resource alias: every row for `view-all`, else public rows
/// and the caller's own. A count that must equal a list total keeps its rows through this.
pub(super) fn push_visibility(
    sql: &mut String,
    binds: &mut Vec<i64>,
    visibility: MediaVisibility,
    alias: &str,
) {
    match visibility {
        MediaVisibility::All => {}
        MediaVisibility::Actor(Some(user)) => {
            sql.push_str(&format!(
                " AND ({alias}.is_public = 1 OR {alias}.owner_id = ?)"
            ));
            binds.push(user.0 as i64);
        }
        MediaVisibility::Actor(None) => sql.push_str(&format!(" AND {alias}.is_public = 1")),
    }
}

/// An empty list renders `NULL`, which no id equals, so `IN (NULL)` matches no row on both engines
/// where `IN ()` is a MySQL syntax error.
pub(super) fn push_list(sql: &mut String, binds: &mut Vec<i64>, values: impl Iterator<Item = i64>) {
    let before = binds.len();
    for (index, value) in values.enumerate() {
        if index > 0 {
            sql.push_str(", ");
        }
        sql.push('?');
        binds.push(value);
    }
    if binds.len() == before {
        sql.push_str("NULL");
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use crate::ids::{ClassId, TemplateId, UserId};

    #[test]
    fn a_predicate_renders_the_fields_it_carries_in_order() {
        let window = ItemWindow {
            filter: ItemFilter {
                owner_id: Some(UserId(3)),
                resource_class_id: Some(ClassId(4)),
                resource_template_id: Some(TemplateId(5)),
                item_set_id: None,
                is_public: Some(true),
            },
            item_set_ids: vec![ItemSetId(6), ItemSetId(7)],
            visibility: MediaVisibility::Actor(None),
            ..ItemWindow::default()
        };
        let (sql, binds) = predicate(&window);
        assert_eq!(
            sql,
            " WHERE 1 = 1 AND resource.owner_id = ? AND resource.resource_class_id = ? \
             AND resource.resource_template_id = ? AND resource.is_public = ? \
             AND EXISTS (SELECT 1 FROM item_item_set WHERE item_item_set.item_id = resource.id \
             AND item_item_set.item_set_id IN (?, ?)) AND resource.is_public = 1"
        );
        assert_eq!(binds, vec![3, 4, 5, 1, 6, 7]);
    }

    /// `buildBaseQuery` renders the `id` list first, before the adapter's own filters.
    #[test]
    fn a_predicate_renders_the_id_list_first() {
        let window = ItemWindow {
            ids: Some(vec![149, 150]),
            visibility: MediaVisibility::Actor(None),
            ..ItemWindow::default()
        };
        let (sql, binds) = predicate(&window);
        assert_eq!(
            sql,
            " WHERE 1 = 1 AND resource.id IN (?, ?) AND resource.is_public = 1"
        );
        assert_eq!(binds, vec![149, 150]);
    }

    #[test]
    fn a_privileged_caller_adds_no_visibility_clause() {
        let (sql, binds) = predicate(&ItemWindow::default());
        assert_eq!(sql, " WHERE 1 = 1");
        assert!(binds.is_empty());
    }

    #[test]
    fn a_signed_in_caller_sees_public_rows_and_their_own() {
        let window = ItemWindow {
            visibility: MediaVisibility::Actor(Some(UserId(9))),
            ..ItemWindow::default()
        };
        let (sql, binds) = predicate(&window);
        assert_eq!(
            sql,
            " WHERE 1 = 1 AND (resource.is_public = 1 OR resource.owner_id = ?)"
        );
        assert_eq!(binds, vec![9]);
    }

    #[test]
    fn an_anonymous_caller_sees_public_rows() {
        let window = ItemWindow {
            visibility: MediaVisibility::Actor(None),
            ..ItemWindow::default()
        };
        let (sql, binds) = predicate(&window);
        assert_eq!(sql, " WHERE 1 = 1 AND resource.is_public = 1");
        assert!(binds.is_empty());
    }
}
