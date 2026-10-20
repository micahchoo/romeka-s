//! `fulltext_search`: the MATCH queries and the write-path upsert and delete.
//!
//! Upstream `Module::searchFulltext` filters a resource list with
//! `MATCH(fulltext_search.title, fulltext_search.text) AGAINST (?)` and keeps rows whose score is
//! above zero (`application/Module.php`). MySQL runs that statement as written. SQLite has no
//! `MATCH` operator over a plain table, so its migration mirrors `fulltext_search` into the FTS5
//! virtual table `fulltext_search_fts`, keeps it in sync with triggers, and this module queries
//! that. The statement and the score direction per backend come from [`Backend`]; the write path
//! and the row shape are shared.
//!
//! `FulltextSearch::save` and `::delete` are the write half
//! (`application/src/Stdlib/FulltextSearch.php`). `Module::saveFulltext` calls `save` after a
//! create or update and `deleteFulltext` after a delete; the caller builds the row because the text
//! comes from the data types and the block layouts, which do not live in this crate.

use sqlx::{ColumnIndex, Decode, Encode, Executor, IntoArguments, Pool, Sqlite, Type};

use crate::DbError;
use crate::backend::Backend;
use crate::ids::ResourceId;
use crate::rows::FulltextSearchRow;

/// One index match: the resource id and its relevance score.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FulltextMatch {
    pub id: ResourceId,
    /// Higher is a better match, the direction MySQL's `MATCH` score runs in.
    pub score: f64,
}

/// Whether a query holds a word, which is what MySQL's `AGAINST` and FTS5 both need to match
/// anything.
pub fn has_word(needle: &str) -> bool {
    needle
        .split(|c: char| !c.is_alphanumeric())
        .any(|word| !word.is_empty())
}

/// The FTS5 `MATCH` expression for a user query.
///
/// MySQL's natural-language mode splits the query on non-word characters and matches a row that
/// contains any of the words. FTS5 reads `MATCH` as a query expression, where a bare `"`, `*`, `-`
/// or `OR` changes or breaks the query, so every word is quoted and the words are joined with `OR`
/// to keep the any-word membership MySQL gives. MySQL binds the query unchanged, so this function
/// is the SQLite spelling of the same parameter.
///
/// `None` when the query holds no word, which matches nothing either way. Ranking still differs
/// from MySQL (`innodb_ft_min_token_size` drops short words there and FTS5 keeps them);
/// `DESIGN.md` excludes ranking particulars from differential matching.
pub fn match_expression(needle: &str) -> Option<String> {
    let words: Vec<String> = needle
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(|word| format!("\"{}\"", word.replace('"', "\"\"")))
        .collect();
    if words.is_empty() {
        None
    } else {
        Some(words.join(" OR "))
    }
}

/// `fulltext_search` rows.
pub struct FulltextRepo<'a, DB: Backend = Sqlite> {
    pool: &'a Pool<DB>,
}

impl<'a, DB: Backend> FulltextRepo<'a, DB>
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

    /// The index rows of one resource name that match `needle`, best first.
    ///
    /// `Module::searchFulltext` inner-joins the table and keeps rows whose score is above zero, so
    /// a row with no matching word is absent rather than present with a zero score. The statement
    /// takes two parameters in order: the match query, then the resource name.
    pub async fn search(
        &self,
        resource: &str,
        needle: &str,
    ) -> Result<Vec<FulltextMatch>, DbError> {
        let Some(parameter) = DB::fulltext_needle(needle) else {
            return Ok(Vec::new());
        };
        let rows: Vec<(i64, f64)> = sqlx::query_as::<DB, (i64, f64)>(DB::fulltext_select())
            .bind(parameter)
            .bind(resource)
            .fetch_all(self.pool)
            .await?;
        rows.iter()
            .map(|(id, raw)| {
                Ok(FulltextMatch {
                    id: ResourceId(crate::repos::new_id(*id)?),
                    score: DB::fulltext_score(*raw),
                })
            })
            .collect()
    }

    /// `FulltextSearch::save`: insert or overwrite the row for one resource.
    pub async fn save(&self, row: &FulltextSearchRow) -> Result<(), DbError> {
        sqlx::query(DB::upsert_fulltext_search())
            .bind(crate::repos::bind_id(row.id))
            .bind(&row.resource)
            .bind(crate::repos::bind_opt_id(row.owner_id))
            .bind(row.is_public)
            .bind(&row.title)
            .bind(&row.text)
            .execute(self.pool)
            .await?;
        Ok(())
    }

    /// `FulltextSearch::delete`: drop the row for one resource.
    pub async fn delete(&self, id: ResourceId, resource: &str) -> Result<bool, DbError> {
        let done = sqlx::query("DELETE FROM fulltext_search WHERE id = ? AND resource = ?")
            .bind(crate::repos::bind_id(id))
            .bind(resource)
            .execute(self.pool)
            .await?;
        Ok(DB::rows_affected(&done) > 0)
    }

    /// Every index row, by resource name then id.
    pub async fn list(&self) -> Result<Vec<FulltextSearchRow>, DbError> {
        let rows = sqlx::query::<DB>("SELECT * FROM fulltext_search ORDER BY resource, id")
            .fetch_all(self.pool)
            .await?;
        rows.iter()
            .map(FulltextSearchRow::from_db_row::<DB>)
            .collect()
    }

    /// `IndexFulltextSearch::perform`'s first statement: empty the index.
    pub async fn clear(&self) -> Result<(), DbError> {
        sqlx::query("DELETE FROM fulltext_search")
            .execute(self.pool)
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn match_expression_quotes_every_word() {
        assert_eq!(
            match_expression("oil canvas").as_deref(),
            Some("\"oil\" OR \"canvas\"")
        );
        assert_eq!(
            match_expression("d'Angelo").as_deref(),
            Some("\"d\" OR \"Angelo\"")
        );
        assert_eq!(
            match_expression("\"quoted\"").as_deref(),
            Some("\"quoted\"")
        );
        assert_eq!(
            match_expression("AND OR NEAR").as_deref(),
            Some("\"AND\" OR \"OR\" OR \"NEAR\"")
        );
    }

    #[test]
    fn match_expression_without_a_word_is_empty() {
        assert_eq!(match_expression("   "), None);
        assert_eq!(match_expression("!!! ***"), None);
    }

    #[test]
    fn a_word_test_matches_what_the_expression_keeps() {
        assert!(has_word("oil canvas"));
        assert!(has_word("d'Angelo"));
        assert!(!has_word("   "));
        assert!(!has_word("!!! ***"));
    }
}
