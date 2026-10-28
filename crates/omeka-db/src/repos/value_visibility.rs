//! Upstream `ValueVisibilityFilter` (`application/src/Db/Filter/ValueVisibilityFilter.php`) as one
//! SQL condition on a `value` alias.
//!
//! Upstream enables that filter on the entity manager (`EntityManagerFactory`), so it joins every
//! statement that reads `value`: documents, reverse references, the property search and every count.
//! A caller with `view-all` sees every value, a signed-in caller also sees every value on the
//! resources they own, and everyone else sees public values only. Every statement here that reads
//! `value` for a caller renders this condition, so the rule has one spelling.
//!
//! A resource's stored `title` is a copy of its first title value (`ResourceTitleHydrator`), so a
//! statement that reads `resource.title` for a caller asks [`bind_title_visible`] whether that copy
//! came from a value the caller may see.

use sqlx::{Encode, Type};

use super::{MediaVisibility, Statement};
use crate::DbError;
use crate::backend::Backend;

/// The condition, led by ` AND `, and the owner id its one placeholder binds; `None` for
/// `view-all`, which sees every value.
fn clause(visibility: MediaVisibility, alias: &str) -> Option<(String, Option<i64>)> {
    match visibility {
        MediaVisibility::All => None,
        MediaVisibility::Actor(None) => Some((format!(" AND {alias}.is_public = 1"), None)),
        MediaVisibility::Actor(Some(user)) => Some((
            format!(
                " AND ({alias}.is_public = 1 OR EXISTS (SELECT 1 FROM resource value_owner \
                 WHERE value_owner.id = {alias}.resource_id AND value_owner.owner_id = ?))"
            ),
            Some(user.0 as i64),
        )),
    }
}

/// Append the condition to SQL whose binds are collected beside it, in clause order.
pub(crate) fn push_value_visibility(
    sql: &mut String,
    binds: &mut Vec<i64>,
    visibility: MediaVisibility,
    alias: &str,
) {
    if let Some((text, owner)) = clause(visibility, alias) {
        sql.push_str(&text);
        binds.extend(owner);
    }
}

/// Append the condition to a [`Statement`].
pub(crate) fn bind_value_visibility<'q, DB: Backend>(
    statement: &mut Statement<'q, DB>,
    visibility: MediaVisibility,
    alias: &str,
) -> Result<(), DbError>
where
    for<'x> i64: Encode<'x, DB> + Type<DB>,
{
    let Some((text, owner)) = clause(visibility, alias) else {
        return Ok(());
    };
    match (text.split_once('?'), owner) {
        (Some((head, tail)), Some(owner)) => {
            statement.push(head).bind(owner)?.push(tail);
        }
        _ => {
            statement.push(&text);
        }
    }
    Ok(())
}

/// The title property of the resource alias: its template's, else `dcterms:title`
/// (`ResourceTitleHydrator::hydrate`).
pub(crate) fn title_property(resource: &str) -> String {
    format!(
        "COALESCE((SELECT title_template.title_property_id FROM resource_template title_template \
         WHERE title_template.id = {resource}.resource_template_id), \
         (SELECT p.id FROM property p JOIN vocabulary vocab ON vocab.id = p.vocabulary_id \
         WHERE vocab.prefix = 'dcterms' AND p.local_name = 'title'))"
    )
}

/// Append ` AND <the resource alias's stored title may be shown to the caller>`: no value of the
/// title property holds it (so it did not come from a value), or one the caller may see does.
pub(crate) fn bind_title_visible<'q, DB: Backend>(
    statement: &mut Statement<'q, DB>,
    visibility: MediaVisibility,
    resource: &str,
) -> Result<(), DbError>
where
    for<'x> i64: Encode<'x, DB> + Type<DB>,
{
    if visibility == MediaVisibility::All {
        return Ok(());
    }
    let property = title_property(resource);
    statement.push(&format!(
        " AND (NOT EXISTS (SELECT 1 FROM value title_any WHERE title_any.resource_id = {resource}.id \
         AND title_any.value = {resource}.title AND title_any.property_id = {property}) \
         OR EXISTS (SELECT 1 FROM value title_seen WHERE title_seen.resource_id = {resource}.id \
         AND title_seen.value = {resource}.title AND title_seen.property_id = {property}"
    ));
    bind_value_visibility(statement, visibility, "title_seen")?;
    statement.push("))");
    Ok(())
}
