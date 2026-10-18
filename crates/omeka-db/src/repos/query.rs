//! The advanced search filters every resource kind shares.
//!
//! `AbstractResourceEntityAdapter::buildPropertyQuery` turns
//! `property[i][property|type|text|joiner]` into a `WHERE` over the `value`
//! table; `ItemAdapter::buildQuery` adds `item_set_id`, `not_item_set_id`,
//! `site_id`, `in_sites` and `has_media`; `ItemSetAdapter` and `MediaAdapter`
//! add their own `site_id` join. [`QueryFilter`] carries those parameters as
//! typed rows, and each kind's repository renders the ones that kind accepts
//! into SQL.
//!
//! The per-kind repositories keep their own filter structs for the parameters
//! that predate this module; a list call that passes `QueryFilter::default()`
//! behaves exactly as before.
//!
//! The clause text is identical on both backends except the `LIKE ... ESCAPE`
//! literal, which the caller's backend supplies.

use sqlx::{Encode, Type};

use super::Statement;
use super::bind_id;
use crate::DbError;
use crate::backend::Backend;
use crate::ids::{ItemSetId, PropertyId, SiteId, UserId};

/// How one property row joins the row before it.
///
/// `AbstractResourceEntityAdapter::buildPropertyQuery` builds the clause as
/// `$where = row1; $where .= " OR row2"` / `" AND row3"`, so the operators
/// apply left to right with SQL's own precedence (`AND` binds tighter than
/// `OR`) and no grouping parentheses. The rendered SQL keeps that shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Joiner {
    #[default]
    And,
    Or,
}

/// The comparison one property row applies.
///
/// The negative types (`neq`, `nin`, `nsw`, `new`, `nres`, `nex`, `ndt`) are
/// the same comparison with [`ValueQuery::negated`] set, which is how
/// `buildPropertyQuery` strips the leading `n` before its switch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchKind {
    /// `eq`: the literal, the URI or a referenced resource title equals the text.
    Eq,
    /// `in`: contains.
    In,
    /// `sw`: starts with.
    StartsWith,
    /// `ew`: ends with.
    EndsWith,
    /// `res`: the value references this resource id.
    Resource,
    /// `ex`: the resource has any value for the property.
    Exists,
    /// `dt`: the value's data type equals the text.
    DataType,
}

/// One row of `property[i][...]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValueQuery {
    /// The property id, `None` for no property narrowing. An unknown
    /// `prefix:localName` term becomes `PropertyId(0)`, which is what
    /// `buildPropertyQuery` stores when `getPropertyByTerm` returns null.
    pub property: Option<PropertyId>,
    pub kind: MatchKind,
    pub negated: bool,
    pub text: String,
    pub joiner: Joiner,
}

/// Which media rows a `has_media` test counts.
///
/// `ItemAdapter::buildQuery` joins `omeka_root.media` and the SQL visibility
/// filter turns private media into a NULL alias for a caller who may not see
/// them, so `has_media` counts only the media the caller can see.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MediaVisibility {
    /// Every media row, for a caller with `view-all`.
    #[default]
    All,
    /// Public media, or media the given user owns.
    Actor(Option<UserId>),
}

/// The advanced search parameters shared by items, item sets and media.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct QueryFilter {
    /// The property rows, in the order the query carried them.
    pub value: Vec<ValueQuery>,
    /// `site_id`: items attached to this site, item sets assigned to it, or
    /// media of an item attached to it.
    pub site_id: Option<SiteId>,
    /// `in_sites`: items that are attached to any site (true) or to none (false).
    pub in_sites: Option<bool>,
    /// `has_media`: items with media (true) or without (false).
    pub has_media: Option<bool>,
    /// `site_attachments_only`: items attached to a block on a page of the
    /// `site_id` site (`ItemAdapter::buildQuery`). Only meaningful with
    /// `site_id`.
    pub site_attachments_only: bool,
    /// The media rows the `has_media` test counts.
    pub media_visibility: MediaVisibility,
    /// `item_set_id`: items in any of these sets.
    pub item_set_ids: Vec<ItemSetId>,
    /// `not_item_set_id`: items in none of these sets.
    pub not_item_set_ids: Vec<ItemSetId>,
}

impl QueryFilter {
    /// Whether the filter adds nothing to a query.
    pub fn is_empty(&self) -> bool {
        self.value.is_empty()
            && self.site_id.is_none()
            && self.in_sites.is_none()
            && self.has_media.is_none()
            && !self.site_attachments_only
            && self.item_set_ids.is_empty()
            && self.not_item_set_ids.is_empty()
    }

    /// The additions for `ItemAdapter::buildQuery`.
    pub(crate) fn push_item<'q, DB: Backend>(
        &self,
        statement: &mut Statement<'q, DB>,
    ) -> Result<(), DbError>
    where
        for<'x> i64: Encode<'x, DB> + Type<DB>,
        for<'x> String: Encode<'x, DB> + Type<DB>,
    {
        self.push_value(statement)?;
        if let Some(site_id) = self.site_id {
            statement.push(
                " AND EXISTS (SELECT 1 FROM item_site
                   WHERE item_site.item_id = resource.id AND item_site.site_id = ",
            );
            statement.bind(bind_id(site_id))?;
            statement.push(")");
            if self.site_attachments_only {
                // `ItemAdapter::buildQuery`: the item is attached to a block on
                // a page of this site.
                statement.push(
                    " AND EXISTS (SELECT 1 FROM site_block_attachment
                       JOIN site_page_block ON site_page_block.id = site_block_attachment.block_id
                       JOIN site_page ON site_page.id = site_page_block.page_id
                       WHERE site_block_attachment.item_id = resource.id
                         AND site_page.site_id = ",
                );
                statement.bind(bind_id(site_id))?;
                statement.push(")");
            }
        } else if let Some(in_sites) = self.in_sites {
            statement.push(" AND ").push(if in_sites {
                "EXISTS (SELECT 1 FROM item_site WHERE item_site.item_id = resource.id)"
            } else {
                "NOT EXISTS (SELECT 1 FROM item_site WHERE item_site.item_id = resource.id)"
            });
        }
        if let Some(has_media) = self.has_media {
            statement
                .push(" AND ")
                .push(if has_media { "EXISTS" } else { "NOT EXISTS" })
                .push(
                    " (SELECT 1 FROM media
                       JOIN resource mr ON mr.id = media.id
                       WHERE media.item_id = resource.id",
                );
            self.push_media_visibility(statement, "mr")?;
            statement.push(")");
        }
        push_item_sets(statement, "item_item_set.item_id = resource.id", self)
    }

    /// The additions for `ItemSetAdapter::buildQuery`.
    pub(crate) fn push_item_set<'q, DB: Backend>(
        &self,
        statement: &mut Statement<'q, DB>,
    ) -> Result<(), DbError>
    where
        for<'x> i64: Encode<'x, DB> + Type<DB>,
    {
        if let Some(site_id) = self.site_id {
            statement.push(
                " AND EXISTS (SELECT 1 FROM site_item_set
                   WHERE site_item_set.item_set_id = resource.id
                     AND site_item_set.site_id = ",
            );
            statement.bind(bind_id(site_id))?;
            statement.push(")");
        }
        Ok(())
    }

    /// The additions for `MediaAdapter::buildQuery`. Media carries no property
    /// rows of its own, but `AbstractResourceEntityAdapter::buildQuery` gives
    /// media the same `search` and property handling its parent has.
    pub(crate) fn push_media<'q, DB: Backend>(
        &self,
        statement: &mut Statement<'q, DB>,
    ) -> Result<(), DbError>
    where
        for<'x> i64: Encode<'x, DB> + Type<DB>,
        for<'x> String: Encode<'x, DB> + Type<DB>,
    {
        self.push_value(statement)?;
        if let Some(site_id) = self.site_id {
            statement.push(
                " AND EXISTS (SELECT 1 FROM item_site
                   WHERE item_site.item_id = media.item_id AND item_site.site_id = ",
            );
            statement.bind(bind_id(site_id))?;
            statement.push(")");
        }
        Ok(())
    }

    /// The visibility test on a media alias, mirroring the SQL visibility
    /// filter `ItemAdapter::buildQuery` relies on.
    fn push_media_visibility<'q, DB: Backend>(
        &self,
        statement: &mut Statement<'q, DB>,
        alias: &str,
    ) -> Result<(), DbError>
    where
        for<'x> i64: Encode<'x, DB> + Type<DB>,
    {
        match self.media_visibility {
            MediaVisibility::All => {}
            MediaVisibility::Actor(owner) => {
                statement.push(" AND (").push(alias).push(".is_public = 1");
                if let Some(owner) = owner {
                    statement
                        .push(" OR ")
                        .push(alias)
                        .push(".owner_id = ")
                        .bind(bind_id(owner))?;
                }
                statement.push(")");
            }
        }
        Ok(())
    }

    /// The `value` table clause from the property rows.
    fn push_value<'q, DB: Backend>(&self, statement: &mut Statement<'q, DB>) -> Result<(), DbError>
    where
        for<'x> i64: Encode<'x, DB> + Type<DB>,
        for<'x> String: Encode<'x, DB> + Type<DB>,
    {
        for (index, row) in self.value.iter().enumerate() {
            if index == 0 {
                statement.push(" AND ");
            } else if row.joiner == Joiner::Or {
                statement.push(" OR ");
            } else {
                statement.push(" AND ");
            }
            // A positive row keeps the comparison in the subquery's `WHERE`; a
            // negative row puts the same comparison there under `NOT EXISTS`,
            // which is `buildPropertyQuery`'s `LEFT JOIN ... WHERE alias.id IS
            // NULL` for a row that must not have such a value.
            statement.push(if row.negated {
                "NOT EXISTS (SELECT 1 FROM value v WHERE v.resource_id = resource.id"
            } else {
                "EXISTS (SELECT 1 FROM value v WHERE v.resource_id = resource.id"
            });
            if let Some(property) = row.property {
                statement.push(" AND v.property_id = ");
                statement.bind(bind_id(property))?;
            }
            push_predicate(statement, row)?;
            statement.push(")");
        }
        Ok(())
    }
}

/// The predicate of one property row, inside its `EXISTS` subquery.
fn push_predicate<'q, DB: Backend>(
    statement: &mut Statement<'q, DB>,
    row: &ValueQuery,
) -> Result<(), DbError>
where
    for<'x> i64: Encode<'x, DB> + Type<DB>,
    for<'x> String: Encode<'x, DB> + Type<DB>,
{
    match row.kind {
        // `ex`: the join itself is the test, so no extra condition.
        MatchKind::Exists => {}
        MatchKind::DataType => {
            statement.push(" AND v.type = ");
            statement.bind(row.text.clone())?;
        }
        MatchKind::Resource => match row.text.trim().parse::<i64>() {
            Ok(id) => {
                statement.push(" AND v.value_resource_id = ");
                statement.bind(id)?;
            }
            // `$qb->createNamedParameter('abc')` against an integer column
            // matches nothing in MySQL, which is what this renders.
            Err(_) => {
                statement.push(" AND 1 = 0");
            }
        },
        MatchKind::Eq => {
            statement
                .push(" AND (v.value_resource_id IN (SELECT id FROM resource r2 WHERE r2.title = ");
            statement.bind(row.text.clone())?;
            statement.push(") OR v.value = ");
            statement.bind(row.text.clone())?;
            statement.push(" OR v.uri = ");
            statement.bind(row.text.clone())?;
            statement.push(")");
        }
        MatchKind::In | MatchKind::StartsWith | MatchKind::EndsWith => {
            let pattern = like_pattern(row.kind, &row.text);
            let escape = DB::like_escape();
            statement.push(
                " AND (v.value_resource_id IN (SELECT id FROM resource r2 WHERE r2.title LIKE ",
            );
            statement.bind(pattern.clone())?;
            statement.push(escape);
            statement.push(") OR v.value LIKE ");
            statement.bind(pattern.clone())?;
            statement.push(escape);
            statement.push(" OR v.uri LIKE ");
            statement.bind(pattern)?;
            statement.push(escape);
            statement.push(")");
        }
    }
    Ok(())
}

/// `buildPropertyQuery`'s `$escapeSqlLike` plus the surrounding wildcards.
fn like_pattern(kind: MatchKind, text: &str) -> String {
    let escaped = text
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    match kind {
        MatchKind::StartsWith => format!("{escaped}%"),
        MatchKind::EndsWith => format!("%{escaped}"),
        _ => format!("%{escaped}%"),
    }
}

/// The `item_set_id` and `not_item_set_id` membership clauses for items.
fn push_item_sets<'q, DB: Backend>(
    statement: &mut Statement<'q, DB>,
    join: &str,
    filter: &QueryFilter,
) -> Result<(), DbError>
where
    for<'x> i64: Encode<'x, DB> + Type<DB>,
{
    if !filter.item_set_ids.is_empty() {
        statement
            .push(" AND EXISTS (SELECT 1 FROM item_item_set WHERE ")
            .push(join)
            .push(" AND item_item_set.item_set_id IN (");
        for (index, id) in filter.item_set_ids.iter().enumerate() {
            if index > 0 {
                statement.push(", ");
            }
            statement.bind(bind_id(*id))?;
        }
        statement.push("))");
    }
    if !filter.not_item_set_ids.is_empty() {
        statement
            .push(" AND NOT EXISTS (SELECT 1 FROM item_item_set WHERE ")
            .push(join)
            .push(" AND item_item_set.item_set_id IN (");
        for (index, id) in filter.not_item_set_ids.iter().enumerate() {
            if index > 0 {
                statement.push(", ");
            }
            statement.bind(bind_id(*id))?;
        }
        statement.push("))");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use sqlx::Sqlite;

    fn item_sql(filter: &QueryFilter) -> String {
        let mut statement = Statement::<Sqlite>::new("SELECT 1");
        filter.push_item(&mut statement).unwrap();
        statement.sql().to_string()
    }

    #[test]
    fn a_search_row_is_one_contains_exists() {
        let filter = QueryFilter {
            value: vec![ValueQuery {
                property: None,
                kind: MatchKind::In,
                negated: false,
                text: "cat".to_string(),
                joiner: Joiner::And,
            }],
            ..Default::default()
        };
        let sql = item_sql(&filter);
        assert!(
            sql.contains("EXISTS (SELECT 1 FROM value v WHERE v.resource_id = resource.id"),
            "sql was {sql}"
        );
        assert!(sql.contains("v.value LIKE ? ESCAPE '\\'"), "sql was {sql}");
    }

    #[test]
    fn an_unknown_property_narrows_to_id_zero() {
        let filter = QueryFilter {
            value: vec![ValueQuery {
                property: Some(PropertyId(0)),
                kind: MatchKind::Exists,
                negated: false,
                text: String::new(),
                joiner: Joiner::And,
            }],
            ..Default::default()
        };
        let sql = item_sql(&filter);
        assert!(sql.contains("v.property_id = ?"), "sql was {sql}");
    }

    #[test]
    fn negative_rows_render_not_exists() {
        let filter = QueryFilter {
            value: vec![ValueQuery {
                property: None,
                kind: MatchKind::Eq,
                negated: true,
                text: "draft".to_string(),
                joiner: Joiner::And,
            }],
            ..Default::default()
        };
        let sql = item_sql(&filter);
        assert!(
            sql.contains("NOT EXISTS (SELECT 1 FROM value v"),
            "sql was {sql}"
        );
    }

    #[test]
    fn joiners_render_between_rows_only() {
        let row = |joiner| ValueQuery {
            property: None,
            kind: MatchKind::In,
            negated: false,
            text: "x".to_string(),
            joiner,
        };
        let filter = QueryFilter {
            value: vec![row(Joiner::And), row(Joiner::Or), row(Joiner::And)],
            ..Default::default()
        };
        let sql = item_sql(&filter);
        assert_eq!(
            sql.matches("EXISTS (SELECT 1 FROM value").count(),
            3,
            "sql was {sql}"
        );
        assert_eq!(sql.matches(") OR EXISTS").count(), 1, "sql was {sql}");
        assert_eq!(sql.matches(") AND EXISTS").count(), 1, "sql was {sql}");
    }

    #[test]
    fn like_text_escapes_the_wildcards() {
        assert_eq!(like_pattern(MatchKind::In, "50%_off"), "%50\\%\\_off%");
        assert_eq!(like_pattern(MatchKind::StartsWith, "a\\b"), "a\\\\b%");
    }
}
