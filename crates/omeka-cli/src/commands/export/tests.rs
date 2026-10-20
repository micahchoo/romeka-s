//! Behaviour tests for `romeka export`.

use super::*;
use crate::testing::seeded_db;
use omeka_db::ids::{ItemSetId, PropertyId};
use omeka_db::repos::{ItemRepo, ItemSetRepo, ValueRepo};
use omeka_db::rows::{NewItem, NewItemSet, NewResource, NewValue};
use sqlx::SqlitePool;

async fn property(pool: &SqlitePool, term: &str) -> PropertyId {
    terms::resolve_property(&omeka_db::DbPool::from(pool.clone()), term)
        .await
        .expect(term)
}

/// One item in `item_set` with the given `(term, text)` values.
async fn add_item(pool: &SqlitePool, item_set: ItemSetId, values: &[(&str, &str)]) -> u64 {
    let title = values
        .iter()
        .find(|(term, _)| *term == "dcterms:title")
        .map(|(_, text)| text.to_string());
    let item = ItemRepo::new(pool)
        .create(&NewItem {
            resource: NewResource {
                title,
                ..NewResource::default()
            },
            primary_media_id: None,
        })
        .await
        .expect("create item");
    ItemRepo::new(pool)
        .add_to_item_set(item.id(), item_set)
        .await
        .expect("add to set");
    for (term, text) in values {
        ValueRepo::new(pool)
            .create(&NewValue {
                resource_id: item.resource.id,
                property_id: property(pool, term).await,
                value_resource_id: None,
                value_annotation_id: None,
                r#type: "literal".to_string(),
                lang: None,
                value: Some((*text).to_string()),
                uri: None,
                is_public: true,
            })
            .await
            .expect("create value");
    }
    item.id().0
}

async fn new_item_set(pool: &SqlitePool, title: &str) -> ItemSetId {
    ItemSetRepo::new(pool)
        .create(&NewItemSet {
            resource: NewResource {
                title: Some(title.to_string()),
                ..NewResource::default()
            },
            is_open: false,
        })
        .await
        .expect("create item set")
        .id()
}

#[tokio::test]
async fn jsonld_export_writes_the_documents_the_read_layer_serves() {
    let db = seeded_db().await;
    let set = new_item_set(&db.pool, "Set").await;
    add_item(&db.pool, set, &[("dcterms:title", "Alpha")]).await;
    add_item(&db.pool, set, &[("dcterms:title", "Beta")]).await;

    let out = db.path.parent().expect("dir").join("items.json");
    let report = run_jsonld(&JsonldOptions {
        database: db.path.clone().into(),
        out: out.clone(),
        resource: "items".to_string(),
        item_set: Some(set.0),
        base_url: "http://example.org".to_string(),
    })
    .await
    .expect("export jsonld");
    assert_eq!(report.documents, 2);
    assert!(!report.per_file);

    let text = std::fs::read_to_string(&out).expect("read export");
    let documents: Vec<serde_json::Value> = serde_json::from_str(&text).expect("parse export");
    assert_eq!(documents.len(), 2);
    // Every element parses as the read layer's wire type.
    for document in &documents {
        let item: ItemDocument = serde_json::from_value(document.clone()).expect("wire item");
        assert_eq!(item.o_id, document["o:id"].as_u64().expect("o:id"));
        assert_eq!(item.context, "http://example.org/api-context");
    }
    assert_eq!(documents[0]["o:title"], "Alpha");
}

#[tokio::test]
async fn jsonld_export_to_a_directory_writes_one_file_per_document() {
    let db = seeded_db().await;
    let set = new_item_set(&db.pool, "Set").await;
    let first = add_item(&db.pool, set, &[("dcterms:title", "Alpha")]).await;
    add_item(&db.pool, set, &[("dcterms:title", "Beta")]).await;

    let directory = db.path.parent().expect("dir").join("documents");
    std::fs::create_dir(&directory).expect("create dir");
    let report = run_jsonld(&JsonldOptions {
        database: db.path.clone().into(),
        out: directory.clone(),
        resource: "items".to_string(),
        item_set: Some(set.0),
        base_url: "http://example.org".to_string(),
    })
    .await
    .expect("export jsonld");
    assert!(report.per_file);
    let path = directory.join(format!("items-{first}.json"));
    assert!(path.is_file(), "expected {path:?}");
    let document: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).expect("read")).expect("parse");
    assert_eq!(document["o:title"], "Alpha");
}

#[tokio::test]
async fn jsonld_export_rejects_an_unknown_resource() {
    let db = seeded_db().await;
    let error = run_jsonld(&JsonldOptions {
        database: db.path.clone().into(),
        out: db.path.parent().expect("dir").join("x.json"),
        resource: "widgets".to_string(),
        item_set: None,
        base_url: "http://example.org".to_string(),
    })
    .await
    .expect_err("unknown resource");
    assert!(error.to_string().contains("unknown --resource"));
}

#[tokio::test]
async fn csv_export_writes_one_row_per_item_with_a_column_per_value() {
    let db = seeded_db().await;
    let source = new_item_set(&db.pool, "Source").await;
    // The title leads; two creators force the header to repeat that term.
    add_item(
        &db.pool,
        source,
        &[
            ("dcterms:title", "Alpha"),
            ("dcterms:creator", "Ada"),
            ("dcterms:creator", "Alan"),
            ("dcterms:description", "first"),
        ],
    )
    .await;
    add_item(
        &db.pool,
        source,
        &[("dcterms:title", "Beta"), ("dcterms:creator", "Grace")],
    )
    .await;

    let out = db.path.parent().expect("dir").join("items.csv");
    let report = run_csv(&CsvOptions {
        database: db.path.clone().into(),
        out: out.clone(),
        item_set: Some(source.0),
        columns: Vec::new(),
    })
    .await
    .expect("export csv");
    assert_eq!(report.items, 2);
    assert_eq!(
        report.columns,
        [
            "dcterms:title",
            "dcterms:creator",
            "dcterms:creator",
            "dcterms:description"
        ]
    );

    assert_eq!(
        std::fs::read_to_string(&out).expect("read"),
        "dcterms:title,dcterms:creator,dcterms:creator,dcterms:description\n\
         Alpha,Ada,Alan,first\n\
         Beta,Grace,,\n"
    );
}

#[tokio::test]
async fn csv_export_honours_an_explicit_column_list() {
    let db = seeded_db().await;
    let set = new_item_set(&db.pool, "Set").await;
    add_item(
        &db.pool,
        set,
        &[("dcterms:title", "Alpha"), ("dcterms:creator", "Ada")],
    )
    .await;

    let out = db.path.parent().expect("dir").join("one.csv");
    let report = run_csv(&CsvOptions {
        database: db.path.clone().into(),
        out: out.clone(),
        item_set: Some(set.0),
        columns: vec!["dcterms:title".to_string()],
    })
    .await
    .expect("export csv");
    assert_eq!(report.columns, ["dcterms:title"]);
    let text = std::fs::read_to_string(&out).expect("read");
    assert_eq!(text, "dcterms:title\nAlpha\n");
}

#[tokio::test]
async fn csv_export_rejects_an_unknown_column() {
    let db = seeded_db().await;
    let error = run_csv(&CsvOptions {
        database: db.path.clone().into(),
        out: db.path.parent().expect("dir").join("x.csv"),
        item_set: None,
        columns: vec!["nope:title".to_string()],
    })
    .await
    .expect_err("unknown term");
    assert!(error.to_string().contains("unknown vocabulary prefix"));
}

#[tokio::test]
async fn csv_export_writes_the_derived_header_before_any_row() {
    let db = seeded_db().await;
    let set = new_item_set(&db.pool, "Set").await;
    add_item(&db.pool, set, &[("dcterms:title", "Alpha")]).await;
    let out = db.path.parent().expect("dir").join("empty.csv");
    let report = run_csv(&CsvOptions {
        database: db.path.clone().into(),
        out: out.clone(),
        item_set: Some(set.0),
        columns: Vec::new(),
    })
    .await
    .expect("export csv");
    assert_eq!(report.columns, ["dcterms:title"]);
    assert_eq!(
        std::fs::read_to_string(&out).expect("read"),
        "dcterms:title\nAlpha\n"
    );
}

/// A file path whose parent does not exist is created, so a caller does not
/// have to `mkdir` first.
#[tokio::test]
async fn csv_export_creates_a_missing_parent_directory() {
    let db = seeded_db().await;
    let set = new_item_set(&db.pool, "Set").await;
    add_item(&db.pool, set, &[("dcterms:title", "Alpha")]).await;
    let out = db
        .path
        .parent()
        .expect("dir")
        .join("nested")
        .join("out.csv");
    run_csv(&CsvOptions {
        database: db.path.clone().into(),
        out: out.clone(),
        item_set: Some(set.0),
        columns: Vec::new(),
    })
    .await
    .expect("export csv");
    assert!(out.is_file());
}
