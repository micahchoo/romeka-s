//! Resolving a `prefix:local_name` term to a property id.
//!
//! Shared by `import csv` and the batch jobs. The caller maps the returned
//! message onto its own error type, so this stays a plain `String` failure.

use omeka_db::DbError;
use omeka_db::DbPool;
use omeka_db::ids::PropertyId;
use omeka_db::repos::{PropertyRepo, VocabularyRepo};
use omeka_db::with_db;

/// Resolve one term (`dcterms:title`) to the stored property.
///
/// # Errors
/// A message naming the unknown term, prefix or property, or the db error text.
pub async fn resolve_property(pool: &DbPool, term: &str) -> Result<PropertyId, String> {
    let (prefix, local_name) = term
        .split_once(':')
        .ok_or_else(|| format!("\"{term}\" is not a property term like dcterms:title"))?;
    let vocabulary = with_db!(pool, |db| VocabularyRepo::new(db)
        .get_by_prefix(prefix)
        .await)
    .map_err(db_message)?
    .ok_or_else(|| format!("unknown vocabulary prefix \"{prefix}\""))?;
    let property = with_db!(pool, |db| PropertyRepo::new(db)
        .find(vocabulary.id, local_name)
        .await)
    .map_err(db_message)?
    .ok_or_else(|| format!("unknown property \"{term}\""))?;
    Ok(property.id)
}

fn db_message(error: DbError) -> String {
    error.to_string()
}
