//! The list endpoints batch their per-row relations.
//!
//! Before this change a document asked the database for its media, its
//! item-set memberships, its values and its reverse references itself, so a
//! page's query count scaled with the page size (`verification/perf/README.md`
//! measured that as the list endpoint's limiter). These tests pin the two
//! properties that matter: the query count per page does not grow with the row
//! count, and a document read through a list equals the one read through the
//! member endpoint.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod support;

use std::sync::Once;
use std::sync::atomic::{AtomicUsize, Ordering};

use log::LevelFilter;
use omeka_api::read::response::ApiResponse;
use omeka_api::read::{Api, Params};
use omeka_db::migrate;
use sqlx::ConnectOptions;
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use support::{admin, base, open_seeded, seed, seed_plain};

fn documents(body: &str) -> Vec<serde_json::Value> {
    match serde_json::from_str(body).unwrap() {
        serde_json::Value::Array(items) => items,
        other => panic!("a list body is an array, got {other}"),
    }
}

fn document_with_id(body: &str, id: u64) -> serde_json::Value {
    documents(body)
        .into_iter()
        .find(|doc| doc["o:id"] == id)
        .unwrap_or_else(|| panic!("no document with o:id {id} in {body}"))
}

// ------------------------------------------------------------- the queries

static QUERIES: AtomicUsize = AtomicUsize::new(0);
static COUNTER: QueryCounter = QueryCounter;
static INSTALL: Once = Once::new();
/// The counter is process-wide, so the tests that read it run one at a time.
static GATE: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Counts the statements sqlx logs at debug level.
struct QueryCounter;

impl log::Log for QueryCounter {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.target() == "sqlx::query" && metadata.level() == log::Level::Debug
    }

    fn log(&self, record: &log::Record<'_>) {
        if self.enabled(record.metadata()) {
            QUERIES.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn flush(&self) {}
}

fn install_counter() {
    INSTALL.call_once(|| {
        // The test binary has no logger, so this only affects this process.
        let _ = log::set_logger(&COUNTER);
        log::set_max_level(LevelFilter::Debug);
    });
}

/// A pool that logs every statement sqlx executes.
async fn logged_pool() -> SqlitePool {
    install_counter();
    let options = SqliteConnectOptions::new()
        .in_memory(true)
        .foreign_keys(true)
        .log_statements(LevelFilter::Debug);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .unwrap();
    migrate(&pool).await.unwrap();
    pool
}

/// Run one `search` and return its answer and the statements it ran.
async fn counted_search(api: &Api<'_>, resource: &str, query: &str) -> (ApiResponse, usize) {
    QUERIES.store(0, Ordering::Relaxed);
    let response = api.search(resource, &Params::parse(query)).await.unwrap();
    let count = QUERIES.load(Ordering::Relaxed);
    (response, count)
}

/// The list path issues the same number of queries for a small page and a large
/// one, and that number is a property of the relations, not of the rows.
#[tokio::test]
async fn items_list_query_count_is_bounded_by_the_page_size() {
    let _gate = GATE.lock().await;
    let pool = logged_pool().await;
    let seed = seed(&pool, 40).await;
    let api = Api::load(&pool, base(), admin(seed.user)).await.unwrap();

    let (small, small_count) = counted_search(&api, "items", "per_page=2").await;
    let (large, large_count) = counted_search(&api, "items", "per_page=20").await;

    assert_eq!(documents(&small.body).len(), 2);
    assert_eq!(documents(&large.body).len(), 20);
    // The page's queries: the filtered list, then one per relation (media,
    // item-set memberships, site memberships, values, the annotations' values,
    // the reverse references, the referenced resources, the thumbnail assets,
    // the template properties).
    assert_eq!(
        small_count, large_count,
        "the query count grows with the page: {small_count} at 2 rows, {large_count} at 20"
    );
    assert!(
        large_count <= 10,
        "a page of 20 rows ran {large_count} queries; the batched path runs a fixed few"
    );
    eprintln!("items list: 2 rows {small_count} queries, 20 rows {large_count} queries");
    pool.close().await;
}

/// The other list endpoints that repeat the shape batch the same way.
#[tokio::test]
async fn every_list_query_count_is_bounded() {
    let _gate = GATE.lock().await;
    let pool = logged_pool().await;
    let seed = seed(&pool, 20).await;
    let api = Api::load(&pool, base(), admin(seed.user)).await.unwrap();

    for resource in ["items", "item_sets", "media", "value_annotations"] {
        let (small, small_count) = counted_search(&api, resource, "per_page=1").await;
        let (_large, large_count) = counted_search(&api, resource, "per_page=10").await;
        assert_eq!(
            small_count, large_count,
            "{resource}: {small_count} queries at 1 row, {large_count} at 10"
        );
        assert!(
            large_count <= 10,
            "{resource}: a page of 10 rows ran {large_count} queries"
        );
        assert!(
            small.status.is_success(),
            "{resource} answered {}",
            small.status
        );
        eprintln!("{resource}: 1 row {small_count} queries, 10 rows {large_count} queries");
    }
    pool.close().await;
}

/// The perf harness's dataset shape. Its page runs the list query and one per
/// relation, and the count does not move with the page size.
#[tokio::test]
async fn the_perf_dataset_page_runs_a_fixed_few_queries() {
    let _gate = GATE.lock().await;
    let pool = logged_pool().await;
    let seed = seed_plain(&pool, 40).await;
    let api = Api::load(&pool, base(), admin(seed.user)).await.unwrap();

    let (_, one) = counted_search(&api, "items", "per_page=1").await;
    let (large, twenty_five) = counted_search(&api, "items", "per_page=25").await;

    assert_eq!(documents(&large.body).len(), 25);
    assert_eq!(
        one, twenty_five,
        "the perf-shaped page count moves with the page: {one} at 1 row, {twenty_five} at 25"
    );
    // The filtered list, then media, item-set memberships, site memberships,
    // values, the reverse references and the thumbnail assets. No media,
    // annotation, template, resource value or custom thumbnail is in the
    // dataset, so no further query runs (the asset and site queries return no
    // row but still run).
    assert_eq!(
        twenty_five, 7,
        "the perf-shaped page should run seven queries"
    );
    eprintln!("perf shape: 1 row {one} queries, 25 rows {twenty_five} queries");
    pool.close().await;
}

// --------------------------------------------------------- the same document

/// A listed item carries the same document as the member endpoint, with media,
/// memberships, a resource value, an annotation, a reverse reference and a
/// template in play.
#[tokio::test]
async fn a_listed_item_document_equals_its_single_read() {
    let _gate = GATE.lock().await;
    let (pool, seed) = open_seeded(4).await;
    let api = Api::load(&pool, base(), admin(seed.user)).await.unwrap();

    let listed = api
        .search("items", &Params::parse("per_page=25"))
        .await
        .unwrap();
    for item in &seed.items {
        let single = api.read_member("items", &item.0.to_string()).await.unwrap();
        assert_eq!(
            document_with_id(&listed.body, item.0),
            serde_json::from_str::<serde_json::Value>(&single.body).unwrap(),
            "item {} differs between the list and the member read",
            item.0
        );
    }
    pool.close().await;
}

/// The item set's document reads the first item's primary media, which the
/// batched path loads with the page's other media.
#[tokio::test]
async fn a_listed_item_set_document_equals_its_single_read() {
    let _gate = GATE.lock().await;
    let (pool, seed) = open_seeded(4).await;
    let api = Api::load(&pool, base(), admin(seed.user)).await.unwrap();

    let listed = api
        .search("item_sets", &Params::parse("per_page=25"))
        .await
        .unwrap();
    let single = api
        .read_member("item_sets", &seed.item_set.0.to_string())
        .await
        .unwrap();
    assert_eq!(
        document_with_id(&listed.body, seed.item_set.0),
        serde_json::from_str::<serde_json::Value>(&single.body).unwrap()
    );
    pool.close().await;
}

/// Every media and annotation document matches its member read.
#[tokio::test]
async fn listed_media_and_annotations_equal_their_single_reads() {
    let _gate = GATE.lock().await;
    let (pool, seed) = open_seeded(4).await;
    let api = Api::load(&pool, base(), admin(seed.user)).await.unwrap();

    let listed = api
        .search("media", &Params::parse("per_page=25"))
        .await
        .unwrap();
    for media in &seed.media {
        let single = api
            .read_member("media", &media.0.to_string())
            .await
            .unwrap();
        assert_eq!(
            document_with_id(&listed.body, media.0),
            serde_json::from_str::<serde_json::Value>(&single.body).unwrap(),
            "media {} differs between the list and the member read",
            media.0
        );
    }

    let listed = api
        .search("value_annotations", &Params::parse("per_page=25"))
        .await
        .unwrap();
    let single = api
        .read_member("value_annotations", &seed.annotation.0.to_string())
        .await
        .unwrap();
    assert_eq!(
        document_with_id(&listed.body, seed.annotation.0),
        serde_json::from_str::<serde_json::Value>(&single.body).unwrap(),
        "annotation {} differs between the list and the member read",
        seed.annotation.0
    );
    pool.close().await;
}
