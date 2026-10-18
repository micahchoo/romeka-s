//! Schema fidelity: the columns of every upstream table, in upstream order.
//!
//! The expected lists are read from `source/omeka-s @ 5e413f0`,
//! `application/data/install/schema.sql`. A column that appears, disappears, or moves fails here.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use sqlx::{Row, SqlitePool};

/// Every upstream table with its column list, in `schema.sql` order.
const TABLES: &[(&str, &[&str])] = &[
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
        "asset",
        &[
            "id",
            "owner_id",
            "name",
            "media_type",
            "storage_id",
            "extension",
            "alt_text",
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
    ("item_set", &["id", "is_open"]),
    ("item", &["id", "primary_media_id"]),
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
    ("value_annotation", &["id"]),
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
    ("item_item_set", &["item_id", "item_set_id"]),
    ("item_site", &["item_id", "site_id"]),
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
    (
        "site_block_attachment",
        &[
            "id", "block_id", "item_id", "media_id", "caption", "position",
        ],
    ),
    (
        "site_item_set",
        &["id", "site_id", "item_set_id", "position"],
    ),
    ("site_permission", &["id", "site_id", "user_id", "role"]),
    ("site_setting", &["id", "site_id", "value"]),
    ("user_setting", &["id", "user_id", "value"]),
    ("setting", &["id", "value"]),
    ("module", &["id", "is_active", "version"]),
    (
        "job",
        &[
            "id", "owner_id", "pid", "status", "class", "args", "log", "started", "ended",
        ],
    ),
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
    (
        "password_creation",
        &["id", "user_id", "created", "activate"],
    ),
    ("migration", &["version"]),
    ("session", &["id", "data", "modified"]),
    (
        "fulltext_search",
        &["id", "resource", "owner_id", "is_public", "title", "text"],
    ),
];

async fn columns(pool: &SqlitePool, table: &str) -> Vec<String> {
    let rows = sqlx::query(&format!("PRAGMA table_info({table})"))
        .fetch_all(pool)
        .await
        .unwrap();
    rows.iter().map(|row| row.get("name")).collect()
}

#[tokio::test]
async fn every_upstream_table_has_its_upstream_columns_in_order() {
    let pool = common::memory().await;
    for (table, expected) in TABLES {
        assert_eq!(&columns(&pool, table).await, expected, "table {table}");
    }
}

#[tokio::test]
async fn no_table_beyond_the_upstream_set_and_the_port_infrastructure() {
    let pool = common::memory().await;
    let rows = sqlx::query("SELECT name FROM sqlite_master WHERE type = 'table'")
        .fetch_all(&pool)
        .await
        .unwrap();
    let mut found: Vec<String> = rows.iter().map(|row| row.get("name")).collect();
    found.retain(|name| !name.starts_with("sqlite_"));
    found.retain(|name| name != "_sqlx_migrations");
    found.retain(|name| name != "fulltext_search_fts");
    found.retain(|name| !name.starts_with("fulltext_search_fts_"));
    let mut expected: Vec<String> = TABLES.iter().map(|(table, _)| table.to_string()).collect();
    found.sort();
    expected.sort();
    assert_eq!(found, expected);
}

#[tokio::test]
async fn column_types_are_integer_text_or_blob() {
    let pool = common::memory().await;
    for (table, _) in TABLES {
        let rows = sqlx::query(&format!("PRAGMA table_info({table})"))
            .fetch_all(&pool)
            .await
            .unwrap();
        for row in &rows {
            let name: String = row.get("name");
            let declared: String = row.get("type");
            assert!(
                matches!(declared.as_str(), "INTEGER" | "TEXT" | "BLOB"),
                "{table}.{name} is {declared}"
            );
        }
    }
}

#[tokio::test]
async fn fulltext_search_index_tracks_insert_update_and_delete() {
    let pool = common::memory().await;
    sqlx::query(
        "INSERT INTO fulltext_search (id, resource, owner_id, is_public, title, text)
         VALUES (1, 'items', NULL, 1, 'Bronze helmet', 'archaeology of the Bronze Age')",
    )
    .execute(&pool)
    .await
    .unwrap();

    let hits = search(&pool, "archaeology").await;
    assert_eq!(hits, vec![1]);

    sqlx::query("UPDATE fulltext_search SET text = 'conservation notes' WHERE id = 1")
        .execute(&pool)
        .await
        .unwrap();
    assert!(search(&pool, "archaeology").await.is_empty());
    assert_eq!(search(&pool, "conservation").await, vec![1]);

    sqlx::query("DELETE FROM fulltext_search WHERE id = 1")
        .execute(&pool)
        .await
        .unwrap();
    assert!(search(&pool, "conservation").await.is_empty());
}

async fn search(pool: &SqlitePool, term: &str) -> Vec<i64> {
    let rows = sqlx::query(
        "SELECT fulltext_search.id FROM fulltext_search_fts
         JOIN fulltext_search ON fulltext_search.rowid = fulltext_search_fts.rowid
         WHERE fulltext_search_fts MATCH ? ORDER BY fulltext_search.id",
    )
    .bind(term)
    .fetch_all(pool)
    .await
    .unwrap();
    rows.iter().map(|row| row.get("id")).collect()
}

#[tokio::test]
async fn boolean_columns_reject_values_beyond_zero_and_one() {
    let pool = common::memory().await;
    let error = sqlx::query("INSERT INTO module (id, is_active, version) VALUES ('M', 2, '1.0')")
        .execute(&pool)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("CHECK"), "{error}");
}

/// One test per upstream index and foreign key, named after its ledger row. Each ledger row cites
/// its own proof name, so deleting or renaming a test breaks that cite instead of silently
/// reusing another assertion. Each assertion reads the live port schema through the `common`
/// helpers (`sqlite_master` and the SQLite pragmas), never a copy of the DDL.
///
/// Five upstream indexes have no port index on exactly their columns and stay excluded in the
/// ledger. Each is the leading column of a composite `UNIQUE` key on the same table, so the
/// lookup is still indexed; the standalone key upstream declares has no port object to assert.
macro_rules! primary_key_proofs {
    ($(($name:ident, $table:literal, [$($column:literal),+ $(,)?])),+ $(,)?) => {
        $(
            #[tokio::test]
            async fn $name() {
                let pool = common::memory().await;
                common::assert_primary_key(&pool, $table, &[$($column),+]).await;
            }
        )+
    };
}

macro_rules! index_proofs {
    ($(($name:ident, $table:literal, $unique:literal, [$($column:literal),+ $(,)?])),+ $(,)?) => {
        $(
            #[tokio::test]
            async fn $name() {
                let pool = common::memory().await;
                common::assert_index(&pool, $table, $unique, &[$($column),+]).await;
            }
        )+
    };
}

macro_rules! foreign_key_proofs {
    ($(($name:ident, $table:literal, $column:literal, $parent:literal, $parent_column:literal, $action:literal)),+ $(,)?) => {
        $(
            #[tokio::test]
            async fn $name() {
                let pool = common::memory().await;
                common::assert_foreign_key(&pool, $table, $column, $parent, $parent_column, $action)
                    .await;
            }
        )+
    };
}

primary_key_proofs! {
    (schema_index_api_key_primary, "api_key", ["id"]),
    (schema_index_asset_primary, "asset", ["id"]),
    (schema_index_fulltext_search_primary, "fulltext_search", ["id", "resource"]),
    (schema_index_item_primary, "item", ["id"]),
    (schema_index_item_item_set_primary, "item_item_set", ["item_id", "item_set_id"]),
    (schema_index_item_set_primary, "item_set", ["id"]),
    (schema_index_item_site_primary, "item_site", ["item_id", "site_id"]),
    (schema_index_job_primary, "job", ["id"]),
    (schema_index_media_primary, "media", ["id"]),
    (schema_index_migration_primary, "migration", ["version"]),
    (schema_index_module_primary, "module", ["id"]),
    (schema_index_password_creation_primary, "password_creation", ["id"]),
    (schema_index_property_primary, "property", ["id"]),
    (schema_index_resource_primary, "resource", ["id"]),
    (schema_index_resource_class_primary, "resource_class", ["id"]),
    (schema_index_resource_template_primary, "resource_template", ["id"]),
    (schema_index_resource_template_property_primary, "resource_template_property", ["id"]),
    (schema_index_session_primary, "session", ["id"]),
    (schema_index_setting_primary, "setting", ["id"]),
    (schema_index_site_primary, "site", ["id"]),
    (schema_index_site_block_attachment_primary, "site_block_attachment", ["id"]),
    (schema_index_site_item_set_primary, "site_item_set", ["id"]),
    (schema_index_site_page_primary, "site_page", ["id"]),
    (schema_index_site_page_block_primary, "site_page_block", ["id"]),
    (schema_index_site_permission_primary, "site_permission", ["id"]),
    (schema_index_site_setting_primary, "site_setting", ["id", "site_id"]),
    (schema_index_user_primary, "user", ["id"]),
    (schema_index_user_setting_primary, "user_setting", ["id", "user_id"]),
    (schema_index_value_primary, "value", ["id"]),
    (schema_index_value_annotation_primary, "value_annotation", ["id"]),
    (schema_index_vocabulary_primary, "vocabulary", ["id"]),
}

index_proofs! {
    (schema_index_api_key_idx_c912ed9d7e3c61f9, "api_key", false, ["owner_id"]),
    (schema_index_asset_uniq_2af5a5c5cc5db90, "asset", true, ["storage_id"]),
    (schema_index_asset_idx_2af5a5c7e3c61f9, "asset", false, ["owner_id"]),
    (schema_index_fulltext_search_idx_aa31fe4a7e3c61f9, "fulltext_search", false, ["owner_id"]),
    (schema_index_item_idx_1f1b251ecbe0b084, "item", false, ["primary_media_id"]),
    (schema_index_item_item_set_idx_6d0c9625960278d7, "item_item_set", false, ["item_set_id"]),
    (schema_index_item_site_idx_a1734d1ff6bd1646, "item_site", false, ["site_id"]),
    (schema_index_job_idx_fbd8e0f87e3c61f9, "job", false, ["owner_id"]),
    (schema_index_media_uniq_6a2ca10c5cc5db90, "media", true, ["storage_id"]),
    (schema_index_media_idx_6a2ca10c126f525e, "media", false, ["item_id"]),
    (schema_index_media_item_position, "media", false, ["item_id", "position"]),
    (schema_index_media_media_type, "media", false, ["media_type"]),
    (schema_index_password_creation_uniq_c77917b4a76ed395, "password_creation", true, ["user_id"]),
    (schema_index_property_uniq_8bf21cdead0e05f6623c14d5, "property", true, ["vocabulary_id", "local_name"]),
    (schema_index_property_idx_8bf21cde7e3c61f9, "property", false, ["owner_id"]),
    (schema_index_property_idx_8bf21cdead0e05f6, "property", false, ["vocabulary_id"]),
    (schema_index_resource_idx_bc91f4167e3c61f9, "resource", false, ["owner_id"]),
    (schema_index_resource_idx_bc91f416448cc1bd, "resource", false, ["resource_class_id"]),
    (schema_index_resource_idx_bc91f41616131ea, "resource", false, ["resource_template_id"]),
    (schema_index_resource_idx_bc91f416fdff2e92, "resource", false, ["thumbnail_id"]),
    (schema_index_resource_title, "resource", false, ["title"]),
    (schema_index_resource_is_public, "resource", false, ["is_public"]),
    (schema_index_resource_class_uniq_c6f063adad0e05f6623c14d5, "resource_class", true, ["vocabulary_id", "local_name"]),
    (schema_index_resource_class_idx_c6f063ad7e3c61f9, "resource_class", false, ["owner_id"]),
    (schema_index_resource_class_idx_c6f063adad0e05f6, "resource_class", false, ["vocabulary_id"]),
    (schema_index_resource_template_uniq_39ecd52eea750e8, "resource_template", true, ["label"]),
    (schema_index_resource_template_idx_39ecd52e7e3c61f9, "resource_template", false, ["owner_id"]),
    (schema_index_resource_template_idx_39ecd52e448cc1bd, "resource_template", false, ["resource_class_id"]),
    (schema_index_resource_template_idx_39ecd52e724734a3, "resource_template", false, ["title_property_id"]),
    (schema_index_resource_template_idx_39ecd52eb84e0d1d, "resource_template", false, ["description_property_id"]),
    (schema_index_resource_template_property_uniq_4689e2f116131ea549213ec, "resource_template_property", true, ["resource_template_id", "property_id"]),
    (schema_index_resource_template_property_idx_4689e2f1549213ec, "resource_template_property", false, ["property_id"]),
    (schema_index_site_uniq_694309e4989d9b62, "site", true, ["slug"]),
    (schema_index_site_uniq_694309e4571edda, "site", true, ["homepage_id"]),
    (schema_index_site_idx_694309e4fdff2e92, "site", false, ["thumbnail_id"]),
    (schema_index_site_idx_694309e47e3c61f9, "site", false, ["owner_id"]),
    (schema_index_site_block_attachment_idx_236473fee9ed820c, "site_block_attachment", false, ["block_id"]),
    (schema_index_site_block_attachment_idx_236473fe126f525e, "site_block_attachment", false, ["item_id"]),
    (schema_index_site_block_attachment_idx_236473feea9fdd75, "site_block_attachment", false, ["media_id"]),
    (schema_index_site_block_attachment_block_position, "site_block_attachment", false, ["block_id", "position"]),
    (schema_index_site_item_set_uniq_d4ce134f6bd1646960278d7, "site_item_set", true, ["site_id", "item_set_id"]),
    (schema_index_site_item_set_idx_d4ce134960278d7, "site_item_set", false, ["item_set_id"]),
    (schema_index_site_item_set_position, "site_item_set", false, ["position"]),
    (schema_index_site_page_is_public, "site_page", false, ["is_public"]),
    (schema_index_site_page_uniq_2f900bd9f6bd1646989d9b62, "site_page", true, ["site_id", "slug"]),
    (schema_index_site_page_idx_2f900bd9f6bd1646, "site_page", false, ["site_id"]),
    (schema_index_site_page_block_idx_c593e731c4663e4, "site_page_block", false, ["page_id"]),
    (schema_index_site_page_block_page_position, "site_page_block", false, ["page_id", "position"]),
    (schema_index_site_permission_uniq_c0401d6ff6bd1646a76ed395, "site_permission", true, ["site_id", "user_id"]),
    (schema_index_site_permission_idx_c0401d6fa76ed395, "site_permission", false, ["user_id"]),
    (schema_index_site_setting_idx_64d05a53f6bd1646, "site_setting", false, ["site_id"]),
    (schema_index_user_uniq_8d93d649e7927c74, "user", true, ["email"]),
    (schema_index_user_setting_idx_c779a692a76ed395, "user_setting", false, ["user_id"]),
    (schema_index_value_uniq_1d7758349b66727e, "value", true, ["value_annotation_id"]),
    (schema_index_value_idx_1d77583489329d25, "value", false, ["resource_id"]),
    (schema_index_value_idx_1d775834549213ec, "value", false, ["property_id"]),
    (schema_index_value_idx_1d7758344bc72506, "value", false, ["value_resource_id"]),
    (schema_index_value_value, "value", false, ["value"]),
    (schema_index_value_uri, "value", false, ["uri"]),
    (schema_index_value_is_public, "value", false, ["is_public"]),
    (schema_index_vocabulary_uniq_9099c97b9b267fdf, "vocabulary", true, ["namespace_uri"]),
    (schema_index_vocabulary_uniq_9099c97b93b1868e, "vocabulary", true, ["prefix"]),
    (schema_index_vocabulary_idx_9099c97b7e3c61f9, "vocabulary", false, ["owner_id"]),
}

foreign_key_proofs! {
    (schema_foreign_key_api_key_fk_c912ed9d7e3c61f9, "api_key", "owner_id", "user", "id", "NO ACTION"),
    (schema_foreign_key_asset_fk_2af5a5c7e3c61f9, "asset", "owner_id", "user", "id", "SET NULL"),
    (schema_foreign_key_fulltext_search_fk_aa31fe4a7e3c61f9, "fulltext_search", "owner_id", "user", "id", "SET NULL"),
    (schema_foreign_key_item_fk_1f1b251ebf396750, "item", "id", "resource", "id", "CASCADE"),
    (schema_foreign_key_item_fk_1f1b251ecbe0b084, "item", "primary_media_id", "media", "id", "SET NULL"),
    (schema_foreign_key_item_item_set_fk_6d0c9625126f525e, "item_item_set", "item_id", "item", "id", "CASCADE"),
    (schema_foreign_key_item_item_set_fk_6d0c9625960278d7, "item_item_set", "item_set_id", "item_set", "id", "CASCADE"),
    (schema_foreign_key_item_set_fk_1015eeebf396750, "item_set", "id", "resource", "id", "CASCADE"),
    (schema_foreign_key_item_site_fk_a1734d1f126f525e, "item_site", "item_id", "item", "id", "CASCADE"),
    (schema_foreign_key_item_site_fk_a1734d1ff6bd1646, "item_site", "site_id", "site", "id", "CASCADE"),
    (schema_foreign_key_job_fk_fbd8e0f87e3c61f9, "job", "owner_id", "user", "id", "SET NULL"),
    (schema_foreign_key_media_fk_6a2ca10c126f525e, "media", "item_id", "item", "id", "NO ACTION"),
    (schema_foreign_key_media_fk_6a2ca10cbf396750, "media", "id", "resource", "id", "CASCADE"),
    (schema_foreign_key_password_creation_fk_c77917b4a76ed395, "password_creation", "user_id", "user", "id", "CASCADE"),
    (schema_foreign_key_property_fk_8bf21cde7e3c61f9, "property", "owner_id", "user", "id", "SET NULL"),
    (schema_foreign_key_property_fk_8bf21cdead0e05f6, "property", "vocabulary_id", "vocabulary", "id", "NO ACTION"),
    (schema_foreign_key_resource_fk_bc91f41616131ea, "resource", "resource_template_id", "resource_template", "id", "SET NULL"),
    (schema_foreign_key_resource_fk_bc91f416448cc1bd, "resource", "resource_class_id", "resource_class", "id", "SET NULL"),
    (schema_foreign_key_resource_fk_bc91f4167e3c61f9, "resource", "owner_id", "user", "id", "SET NULL"),
    (schema_foreign_key_resource_fk_bc91f416fdff2e92, "resource", "thumbnail_id", "asset", "id", "SET NULL"),
    (schema_foreign_key_resource_class_fk_c6f063ad7e3c61f9, "resource_class", "owner_id", "user", "id", "SET NULL"),
    (schema_foreign_key_resource_class_fk_c6f063adad0e05f6, "resource_class", "vocabulary_id", "vocabulary", "id", "NO ACTION"),
    (schema_foreign_key_resource_template_fk_39ecd52e448cc1bd, "resource_template", "resource_class_id", "resource_class", "id", "SET NULL"),
    (schema_foreign_key_resource_template_fk_39ecd52e724734a3, "resource_template", "title_property_id", "property", "id", "SET NULL"),
    (schema_foreign_key_resource_template_fk_39ecd52e7e3c61f9, "resource_template", "owner_id", "user", "id", "SET NULL"),
    (schema_foreign_key_resource_template_fk_39ecd52eb84e0d1d, "resource_template", "description_property_id", "property", "id", "SET NULL"),
    (schema_foreign_key_resource_template_property_fk_4689e2f116131ea, "resource_template_property", "resource_template_id", "resource_template", "id", "NO ACTION"),
    (schema_foreign_key_resource_template_property_fk_4689e2f1549213ec, "resource_template_property", "property_id", "property", "id", "CASCADE"),
    (schema_foreign_key_site_fk_694309e4571edda, "site", "homepage_id", "site_page", "id", "SET NULL"),
    (schema_foreign_key_site_fk_694309e47e3c61f9, "site", "owner_id", "user", "id", "SET NULL"),
    (schema_foreign_key_site_fk_694309e4fdff2e92, "site", "thumbnail_id", "asset", "id", "SET NULL"),
    (schema_foreign_key_site_block_attachment_fk_236473fe126f525e, "site_block_attachment", "item_id", "item", "id", "SET NULL"),
    (schema_foreign_key_site_block_attachment_fk_236473fee9ed820c, "site_block_attachment", "block_id", "site_page_block", "id", "NO ACTION"),
    (schema_foreign_key_site_block_attachment_fk_236473feea9fdd75, "site_block_attachment", "media_id", "media", "id", "SET NULL"),
    (schema_foreign_key_site_item_set_fk_d4ce134960278d7, "site_item_set", "item_set_id", "item_set", "id", "CASCADE"),
    (schema_foreign_key_site_item_set_fk_d4ce134f6bd1646, "site_item_set", "site_id", "site", "id", "CASCADE"),
    (schema_foreign_key_site_page_fk_2f900bd9f6bd1646, "site_page", "site_id", "site", "id", "NO ACTION"),
    (schema_foreign_key_site_page_block_fk_c593e731c4663e4, "site_page_block", "page_id", "site_page", "id", "NO ACTION"),
    (schema_foreign_key_site_permission_fk_c0401d6fa76ed395, "site_permission", "user_id", "user", "id", "CASCADE"),
    (schema_foreign_key_site_permission_fk_c0401d6ff6bd1646, "site_permission", "site_id", "site", "id", "CASCADE"),
    (schema_foreign_key_site_setting_fk_64d05a53f6bd1646, "site_setting", "site_id", "site", "id", "CASCADE"),
    (schema_foreign_key_user_setting_fk_c779a692a76ed395, "user_setting", "user_id", "user", "id", "CASCADE"),
    (schema_foreign_key_value_fk_1d7758344bc72506, "value", "value_resource_id", "resource", "id", "CASCADE"),
    (schema_foreign_key_value_fk_1d775834549213ec, "value", "property_id", "property", "id", "CASCADE"),
    (schema_foreign_key_value_fk_1d77583489329d25, "value", "resource_id", "resource", "id", "NO ACTION"),
    (schema_foreign_key_value_fk_1d7758349b66727e, "value", "value_annotation_id", "value_annotation", "id", "SET NULL"),
    (schema_foreign_key_value_annotation_fk_c03ba4ebf396750, "value_annotation", "id", "resource", "id", "CASCADE"),
    (schema_foreign_key_vocabulary_fk_9099c97b7e3c61f9, "vocabulary", "owner_id", "user", "id", "SET NULL"),
}
