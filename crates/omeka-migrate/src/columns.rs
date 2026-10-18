//! The column check: every source column the port reads, verified before a run writes anything.
//!
//! The port reads a source through `omeka-db`'s row structs, which parse each table by column name.
//! A source that predates the frozen schema would be missing a column, and a row struct would fail
//! mid-run with a bare column error. This check asks the server for its own column list first, so a
//! stale source fails by name before the first write.
//!
//! The list below is the mapping the README documents. It names the columns `omeka-db`'s
//! `rows::from_db_row` reads for each table the port serves, plus the `resource` columns the
//! `item`, `item_set` and `media` joins read.

use sqlx::MySqlPool;

use crate::error::MigrateError;

/// Source tables the port reads, each with the columns the port needs.
pub(crate) const REQUIRED_COLUMNS: &[(&str, &[&str])] = &[
    (
        "api_key",
        &[
            "id",
            "owner_id",
            "label",
            "credential_hash",
            "last_ip",
            "last_accessed",
            "created",
        ],
    ),
    ("item", &["id", "primary_media_id"]),
    ("item_item_set", &["item_id", "item_set_id"]),
    ("item_set", &["id", "is_open"]),
    (
        "media",
        &[
            "id",
            "item_id",
            "ingester",
            "renderer",
            "data",
            "source",
            "media_type",
            "storage_id",
            "extension",
            "sha256",
            "size",
            "has_original",
            "has_thumbnails",
            "position",
            "lang",
            "alt_text",
        ],
    ),
    (
        "property",
        &[
            "id",
            "owner_id",
            "vocabulary_id",
            "local_name",
            "label",
            "comment",
        ],
    ),
    (
        "resource",
        &[
            "id",
            "owner_id",
            "resource_class_id",
            "resource_template_id",
            "thumbnail_id",
            "title",
            "is_public",
            "created",
            "modified",
            "resource_type",
        ],
    ),
    (
        "resource_class",
        &[
            "id",
            "owner_id",
            "vocabulary_id",
            "local_name",
            "label",
            "comment",
        ],
    ),
    (
        "resource_template",
        &[
            "id",
            "owner_id",
            "resource_class_id",
            "title_property_id",
            "description_property_id",
            "label",
        ],
    ),
    (
        "resource_template_property",
        &[
            "id",
            "resource_template_id",
            "property_id",
            "alternate_label",
            "alternate_comment",
            "position",
            "data_type",
            "is_required",
            "is_private",
            "default_lang",
        ],
    ),
    (
        "site",
        &[
            "id",
            "thumbnail_id",
            "homepage_id",
            "owner_id",
            "slug",
            "theme",
            "title",
            "summary",
            "navigation",
            "item_pool",
            "created",
            "modified",
            "is_public",
            "assign_new_items",
        ],
    ),
    (
        "site_item_set",
        &["id", "site_id", "item_set_id", "position"],
    ),
    (
        "site_page",
        &[
            "id",
            "site_id",
            "slug",
            "title",
            "is_public",
            "layout",
            "layout_data",
            "created",
            "modified",
        ],
    ),
    (
        "site_page_block",
        &["id", "page_id", "layout", "data", "layout_data", "position"],
    ),
    ("site_permission", &["id", "site_id", "user_id", "role"]),
    (
        "user",
        &[
            "id",
            "email",
            "name",
            "created",
            "modified",
            "password_hash",
            "role",
            "is_active",
        ],
    ),
    (
        "value",
        &[
            "id",
            "resource_id",
            "property_id",
            "value_resource_id",
            "value_annotation_id",
            "type",
            "lang",
            "value",
            "uri",
            "is_public",
        ],
    ),
    (
        "vocabulary",
        &[
            "id",
            "owner_id",
            "namespace_uri",
            "prefix",
            "label",
            "comment",
        ],
    ),
];

/// Read the source's column list and fail on the first column the port needs and the source lacks.
pub(crate) async fn check_columns(pool: &MySqlPool) -> Result<(), MigrateError> {
    // MySQL 8 reports `INFORMATION_SCHEMA.COLUMNS` names as `VARBINARY` through the data
    // dictionary, so the cast makes them the text they name.
    let present: Vec<(String, String)> = sqlx::query_as(
        "SELECT CAST(TABLE_NAME AS CHAR) AS table_name, CAST(COLUMN_NAME AS CHAR) AS column_name
         FROM INFORMATION_SCHEMA.COLUMNS
         WHERE TABLE_SCHEMA = DATABASE()",
    )
    .fetch_all(pool)
    .await?;

    for (table, columns) in REQUIRED_COLUMNS {
        for column in *columns {
            let found = present
                .iter()
                .any(|(found_table, found_column)| found_table == table && found_column == column);
            if !found {
                return Err(MigrateError::MissingColumn {
                    table: (*table).to_string(),
                    column: (*column).to_string(),
                });
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_port_table_is_checked() {
        // The check covers each table the migrator reads, with no duplicate entries.
        let mut names: Vec<&str> = REQUIRED_COLUMNS.iter().map(|(table, _)| *table).collect();
        let count = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), count, "one entry per table");
        for table in ["resource", "item", "value", "site_page_block", "api_key"] {
            assert!(names.contains(&table), "{table} is checked");
        }
    }
}
