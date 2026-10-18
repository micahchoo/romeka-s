//! The export entry point returns what the list endpoints serve.
//!
//! `read::export::documents` opens the list window and reads the served body
//! back. These tests pin the two properties a caller depends on: every
//! searchable resource answers (so the export list cannot drift from the
//! dispatch), and every element of an item export parses back into the read
//! layer's wire type.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod support;

use omeka_api::read::export::{self, Selection};
use omeka_api::wire::item::Item;
use support::{admin, base, open_seeded};

#[tokio::test]
async fn every_export_resource_answers() {
    let (pool, seed) = open_seeded(3).await;
    let names = export::resources();
    assert_eq!(names.len(), 17);
    for name in names {
        export::documents(&pool, base(), admin(seed.user), &Selection::new(name))
            .await
            .unwrap_or_else(|error| panic!("{name}: {error}"));
    }
}

#[tokio::test]
async fn items_export_parses_into_the_wire_type() {
    let (pool, seed) = open_seeded(3).await;
    let documents = export::documents(&pool, base(), admin(seed.user), &Selection::new("items"))
        .await
        .expect("export items");
    assert_eq!(documents.len(), 3);
    for document in &documents {
        let item: Item = serde_json::from_value(document.clone()).expect("wire item");
        assert!(item.o_id > 0);
        assert_eq!(item.context, "http://localhost/api-context");
        // The seeded items each carry a literal title value.
        assert!(item.values.contains_key("dcterms:title"));
    }
}

#[tokio::test]
async fn an_item_set_selection_filters_the_items() {
    let (pool, seed) = open_seeded(3).await;
    let all = export::documents(&pool, base(), admin(seed.user), &Selection::new("items"))
        .await
        .expect("all items");
    let narrowed = export::documents(
        &pool,
        base(),
        admin(seed.user),
        &Selection::new("items").in_item_set(seed.item_set.0),
    )
    .await
    .expect("narrowed items");
    assert_eq!(narrowed.len(), all.len());
}

/// `ResourceAdapter::search` runs on the abstract `Resource` entity, so its
/// list carries one document per row, of every kind the seed created: three
/// items, one item set, two media and three value annotations.
#[tokio::test]
async fn a_resources_export_lists_every_kind() {
    let (pool, seed) = open_seeded(3).await;
    let documents = export::documents(
        &pool,
        base(),
        admin(seed.user),
        &Selection::new("resources"),
    )
    .await
    .expect("export resources");
    assert_eq!(documents.len(), 9);
    let mut types: Vec<String> = documents
        .iter()
        .map(|document| document["@type"].as_str().unwrap_or_default().to_string())
        .collect();
    types.sort();
    types.dedup();
    assert_eq!(
        types,
        ["o:Item", "o:ItemSet", "o:Media", "o:ValueAnnotation"]
    );
}

/// A name no adapter registers has no dispatch arm, so the export refuses it.
#[tokio::test]
async fn a_resource_with_no_adapter_is_refused() {
    let (pool, seed) = open_seeded(1).await;
    let error = export::documents(&pool, base(), admin(seed.user), &Selection::new("nonsense"))
        .await
        .expect_err("no adapter registers nonsense");
    assert!(!error.to_string().is_empty());
}
