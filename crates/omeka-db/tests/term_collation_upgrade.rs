//! The upgrade that makes `property.local_name` and `resource_class.local_name` case-sensitive,
//! as upstream's `utf8mb4_bin` is, on a database made before it.
//!
//! The rebuild drops and recreates two tables that `value`, `resource` and the template tables
//! reference, so the claim under test is that every row of every table comes through unchanged.
//! `a_real_database_upgrades_with_every_row_kept` runs that claim on a copy of a real archive
//! when `ROMEKA_UPGRADE_DB` names one; the file itself is only read.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::path::{Path, PathBuf};

use omeka_db::repos::{ClassRepo, ItemRepo, PropertyRepo, TemplateRepo, ValueRepo, VocabularyRepo};
use omeka_db::rows::{
    NewClass, NewItem, NewProperty, NewResource, NewTemplate, NewTemplateProperty, NewValue,
    NewVocabulary,
};
use omeka_db::{migrate, open_file};
use sqlx::migrate::Migrator;
use sqlx::{Row, SqlitePool};

/// A database as `0001_init.sql` alone left it: the schema every install before the upgrade has.
async fn before_upgrade(name: &str) -> (SqlitePool, PathBuf) {
    let migrations = std::env::temp_dir().join(format!(
        "romeka-db-{}-{name}-migrations",
        std::process::id()
    ));
    std::fs::create_dir_all(&migrations).unwrap();
    std::fs::copy(
        concat!(env!("CARGO_MANIFEST_DIR"), "/migrations/0001_init.sql"),
        migrations.join("0001_init.sql"),
    )
    .unwrap();
    let path = common::test_path(name);
    common::remove_db(&path);
    let pool = open_file(&path).await.unwrap();
    Migrator::new(migrations.as_path())
        .await
        .unwrap()
        .run(&pool)
        .await
        .unwrap();
    std::fs::remove_dir_all(&migrations).unwrap();
    (pool, path)
}

/// Every table definition, index and trigger, as `sqlite_master` holds them.
async fn schema(pool: &SqlitePool) -> Vec<(String, String, Option<String>)> {
    sqlx::query(
        "SELECT type, name, sql FROM sqlite_master
         WHERE name NOT LIKE 'sqlite_%' ORDER BY type, name",
    )
    .fetch_all(pool)
    .await
    .unwrap()
    .iter()
    .map(|row| (row.get("type"), row.get("name"), row.get("sql")))
    .collect()
}

/// Every row of every stored table, each value through `quote()`, sorted. The sqlx
/// bookkeeping is left out, since the upgrade adds a row there by design.
async fn contents(pool: &SqlitePool) -> Vec<(String, Vec<String>)> {
    let tables: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM sqlite_master WHERE type = 'table' AND name != '_sqlx_migrations'
         AND name NOT LIKE 'romeka_media_annotation_fts%'
         AND sql NOT LIKE 'CREATE VIRTUAL TABLE%' ORDER BY name",
    )
    .fetch_all(pool)
    .await
    .unwrap();
    let mut dump = Vec::new();
    for table in tables {
        let columns: Vec<String> = sqlx::query(&format!("PRAGMA table_info(\"{table}\")"))
            .fetch_all(pool)
            .await
            .unwrap()
            .iter()
            .map(|row| format!("quote(\"{}\")", row.get::<String, _>("name")))
            .collect();
        let mut rows: Vec<String> = sqlx::query_scalar(&format!(
            "SELECT {} FROM \"{table}\"",
            columns.join(" || ',' || ")
        ))
        .fetch_all(pool)
        .await
        .unwrap();
        rows.sort();
        dump.push((table, rows));
    }
    dump
}

/// Asserts the upgrade kept every row of every table the archive had, and that
/// each table a later migration added starts empty.
fn assert_kept(before: &[(String, Vec<String>)], after: &[(String, Vec<String>)]) {
    for (table, rows) in after {
        match before.iter().find(|(name, _)| name == table) {
            Some((_, kept)) => assert_eq!(rows, kept, "{table} keeps every row"),
            None => assert!(rows.is_empty(), "{table} is new and starts empty"),
        }
    }
    for (table, _) in before {
        assert!(
            after.iter().any(|(name, _)| name == table),
            "{table} still exists"
        );
    }
}

async fn fresh_schema() -> Vec<(String, String, Option<String>)> {
    schema(&common::memory().await).await
}

/// The schema with the one difference the upgrade makes written out of it: the two `local_name`
/// collations, and the quotes SQLite puts around a renamed table's name.
fn without_term_collation(
    schema: Vec<(String, String, Option<String>)>,
) -> Vec<(String, String, Option<String>)> {
    schema
        .into_iter()
        .map(|(kind, name, sql)| {
            let sql = sql.map(|sql| {
                sql.replace(
                    "local_name TEXT NOT NULL COLLATE NOCASE",
                    "local_name TEXT NOT NULL",
                )
                .replace("CREATE TABLE \"property\"", "CREATE TABLE property")
                .replace(
                    "CREATE TABLE \"resource_class\"",
                    "CREATE TABLE resource_class",
                )
            });
            (kind, name, sql)
        })
        .collect()
}

/// Store FOAF's two given-name properties, which upstream keeps side by side.
async fn store_both_given_names(pool: &SqlitePool) {
    let foaf = VocabularyRepo::new(pool)
        .create(&NewVocabulary {
            owner_id: None,
            namespace_uri: "http://xmlns.com/foaf/0.1/".to_string(),
            prefix: "foaf".to_string(),
            label: "Friend of a Friend".to_string(),
            comment: None,
        })
        .await
        .unwrap();
    for local_name in ["givenName", "givenname"] {
        PropertyRepo::new(pool)
            .create(&NewProperty {
                owner_id: None,
                vocabulary_id: foaf.id,
                local_name: local_name.to_string(),
                label: "Given name".to_string(),
                comment: None,
            })
            .await
            .unwrap_or_else(|error| panic!("foaf:{local_name} is stored: {error}"));
    }
}

/// The rows the rebuilt tables are referenced from: a class on an item, a value on a property,
/// a template that names a title property and lists the property.
#[tokio::test]
async fn an_archive_from_before_the_upgrade_keeps_every_row() {
    let (pool, path) = before_upgrade("term-upgrade").await;
    let dcterms = VocabularyRepo::new(&pool)
        .create(&NewVocabulary {
            owner_id: None,
            namespace_uri: "http://purl.org/dc/terms/".to_string(),
            prefix: "dcterms".to_string(),
            label: "Dublin Core".to_string(),
            comment: None,
        })
        .await
        .unwrap();
    let properties = PropertyRepo::new(&pool);
    let property = |local_name: &str| NewProperty {
        owner_id: None,
        vocabulary_id: dcterms.id,
        local_name: local_name.to_string(),
        label: local_name.to_string(),
        comment: Some("A comment.".to_string()),
    };
    let title = properties.create(&property("title")).await.unwrap();
    let removed = properties.create(&property("removed")).await.unwrap();
    properties.delete(removed.id).await.unwrap();
    let class = ClassRepo::new(&pool)
        .create(&NewClass {
            owner_id: None,
            vocabulary_id: dcterms.id,
            local_name: "PhysicalObject".to_string(),
            label: "Physical Object".to_string(),
            comment: None,
        })
        .await
        .unwrap();
    let templates = TemplateRepo::new(&pool);
    let template = templates
        .create(&NewTemplate {
            owner_id: None,
            resource_class_id: Some(class.id),
            title_property_id: Some(title.id),
            description_property_id: None,
            label: "Object".to_string(),
        })
        .await
        .unwrap();
    templates
        .add_property(&NewTemplateProperty {
            resource_template_id: template.id,
            property_id: title.id,
            alternate_label: Some("Name".to_string()),
            alternate_comment: None,
            position: Some(1),
            data_type: None,
            is_required: true,
            is_private: false,
            default_lang: None,
        })
        .await
        .unwrap();
    let item = ItemRepo::new(&pool)
        .create(&NewItem {
            resource: NewResource {
                resource_class_id: Some(class.id),
                resource_template_id: Some(template.id),
                title: Some("Bronze helmet".to_string()),
                ..NewResource::default()
            },
            primary_media_id: None,
        })
        .await
        .unwrap();
    ValueRepo::new(&pool)
        .create(&NewValue {
            resource_id: item.resource.id,
            property_id: title.id,
            value_resource_id: None,
            value_annotation_id: None,
            r#type: "literal".to_string(),
            lang: None,
            value: Some("Bronze helmet".to_string()),
            uri: None,
            is_public: true,
        })
        .await
        .unwrap();
    let before = contents(&pool).await;
    let old_schema = schema(&pool).await;
    assert!(
        before
            .iter()
            .any(|(table, rows)| table == "value" && rows.len() == 1),
        "the archive holds the value the rebuild must keep"
    );

    migrate(&pool).await.unwrap();

    assert_kept(&before, &contents(&pool).await);
    let new_schema = schema(&pool).await;
    assert_ne!(
        new_schema, old_schema,
        "the upgrade changed the term tables"
    );
    let kept_objects: Vec<_> = new_schema
        .iter()
        .filter(|entry| old_schema.iter().any(|old| old.1 == entry.1))
        .cloned()
        .collect();
    assert_eq!(
        without_term_collation(kept_objects),
        without_term_collation(old_schema),
        "every table, index, key and trigger is kept but the collation"
    );
    assert_eq!(new_schema, fresh_schema().await);
    let next = properties.create(&property("abstract")).await.unwrap();
    assert!(
        next.id > removed.id,
        "the id of a deleted property stays unused: {next:?}"
    );
    let next_class = ClassRepo::new(&pool)
        .create(&NewClass {
            owner_id: None,
            vocabulary_id: dcterms.id,
            local_name: "physicalObject".to_string(),
            label: "lower case".to_string(),
            comment: None,
        })
        .await
        .expect("a class differing only in case is a second class");
    assert!(next_class.id > class.id);
    store_both_given_names(&pool).await;
    let foreign_keys: i64 = sqlx::query_scalar("PRAGMA foreign_keys")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(foreign_keys, 1, "enforcement is back on after the upgrade");
    pool.close().await;
    common::remove_db(&path);
}

/// A lookup by local name is exact, as upstream's binary collation makes it.
#[tokio::test]
async fn a_local_name_lookup_matches_case_exactly() {
    let pool = common::memory().await;
    store_both_given_names(&pool).await;
    let foaf = VocabularyRepo::new(&pool)
        .get_by_prefix("foaf")
        .await
        .unwrap()
        .unwrap();
    let properties = PropertyRepo::new(&pool);
    let upper = properties
        .find(foaf.id, "givenName")
        .await
        .unwrap()
        .unwrap();
    let lower = properties
        .find(foaf.id, "givenname")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(upper.local_name, "givenName");
    assert_eq!(lower.local_name, "givenname");
    assert_ne!(upper.id, lower.id);
    assert_eq!(properties.find(foaf.id, "GIVENNAME").await.unwrap(), None);
    assert_eq!(
        ClassRepo::new(&pool).find(foaf.id, "person").await.unwrap(),
        None
    );
}

/// The same claim on a copy of a real archive: `ROMEKA_UPGRADE_DB` names a database file from
/// before the upgrade. With none set the test says so and returns.
#[tokio::test]
async fn a_real_database_upgrades_with_every_row_kept() {
    let Some(source) = std::env::var_os("ROMEKA_UPGRADE_DB") else {
        eprintln!("ROMEKA_UPGRADE_DB is not set; skipping the real-database upgrade");
        return;
    };
    let copy = common::test_path("real-upgrade");
    common::remove_db(&copy);
    std::fs::write(&copy, std::fs::read(Path::new(&source)).unwrap()).unwrap();
    let pool = open_file(&copy).await.unwrap();
    let applied: Vec<i64> = sqlx::query_scalar("SELECT version FROM _sqlx_migrations")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert!(
        !applied.contains(&4),
        "the copy predates the upgrade: {applied:?}"
    );
    let before = contents(&pool).await;

    migrate(&pool).await.unwrap();

    let after = contents(&pool).await;
    assert_kept(&before, &after);
    assert_eq!(schema(&pool).await, fresh_schema().await);
    let rows: usize = after.iter().map(|(_, rows)| rows.len()).sum();
    eprintln!(
        "upgraded a copy of {}: {} tables, {rows} rows, all unchanged",
        Path::new(&source).display(),
        after.len()
    );
    pool.close().await;
    common::remove_db(&copy);
}
