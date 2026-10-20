//! Items by their `dcterms:identifier`: what an import's match looks up.
//!
//! One query answers a whole archive, not one per record. The text is trimmed here, after the
//! read, because SQL's `TRIM` removes spaces only and a cell keeps tabs and line ends too.

use std::collections::BTreeMap;

use sqlx::{ColumnIndex, Decode, Encode, Executor, IntoArguments, Pool, Sqlite, Type};

use super::{Statement, bind_id, fetch_rows};
use crate::DbError;
use crate::backend::Backend;
use crate::ids::{ItemId, ItemSetId};

/// Item lookups by identifier.
pub struct IdentifierRepo<'a, DB: Backend = Sqlite> {
    pool: &'a Pool<DB>,
}

impl<'a, DB: Backend> IdentifierRepo<'a, DB>
where
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
    usize: ColumnIndex<DB::Row>,
    for<'x> i64: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> String: Decode<'x, DB> + Type<DB>,
    for<'x> &'x str: ColumnIndex<DB::Row>,
{
    pub fn new(pool: &'a Pool<DB>) -> Self {
        Self { pool }
    }

    /// Every item's trimmed `dcterms:identifier` literals, each mapped to the lowest item id
    /// that holds it. `Some(sets)` keeps only items in one of `sets`; an empty list finds
    /// nothing. Blank identifiers are left out.
    pub async fn items(
        &self,
        sets: Option<&[ItemSetId]>,
    ) -> Result<BTreeMap<String, ItemId>, DbError> {
        if sets.is_some_and(<[ItemSetId]>::is_empty) {
            return Ok(BTreeMap::new());
        }
        let mut statement = Statement::<DB>::new(&format!(
            "SELECT v.{value} AS identifier, v.resource_id AS item_id FROM value v
             JOIN item i ON i.id = v.resource_id
             JOIN property p ON p.id = v.property_id
             JOIN vocabulary w ON w.id = p.vocabulary_id
             WHERE w.prefix = 'dcterms' AND p.local_name = 'identifier'
               AND v.type = 'literal' AND v.{value} IS NOT NULL",
            value = DB::quote_identifier("value"),
        ));
        if let Some(sets) = sets {
            statement.push(
                " AND EXISTS (SELECT 1 FROM item_item_set s
                              WHERE s.item_id = v.resource_id AND s.item_set_id IN (",
            );
            for (index, set) in sets.iter().enumerate() {
                if index > 0 {
                    statement.push(", ");
                }
                statement.bind(bind_id(*set))?;
            }
            statement.push("))");
        }
        statement.push(" ORDER BY v.resource_id");
        let (sql, arguments) = statement.into_parts();
        let mut found = BTreeMap::new();
        for row in fetch_rows(&sql, arguments, self.pool).await? {
            let identifier = DB::text(&row, "identifier")?;
            let identifier = identifier.trim();
            if !identifier.is_empty() {
                found
                    .entry(identifier.to_string())
                    .or_insert(DB::id(&row, "item_id")?);
            }
        }
        Ok(found)
    }
}
