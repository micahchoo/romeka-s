//! A release-mode probe that splits one list request into the stages it runs.
//!
//! `cargo test --release -p omeka-api --lib -- --ignored --nocapture list_cost`
//! seeds the perf harness's dataset shape (one item set, 200 items, six values
//! each, no media, template, annotation or resource value) into a file-backed
//! SQLite database, then times the whole `search_items` call and each stage it
//! is built from. It is the lever behind the limiter claim in
//! `verification/perf/README.md`: a reviewer reruns it instead of trusting a
//! prose split.
//!
//! The database is file-backed because that is what the harness serves. The
//! numbers are per-request medians inside this process; the HTTP round trip,
//! routing and authentication are not in them, so the end-to-end stage reads
//! lower than the harness's `items_list`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;
use std::time::{Duration, Instant};

use omeka_db::ids::{ItemId, ItemSetId, PropertyId, UserId};
use omeka_db::repos::{
    ClassRepo, ItemFilter, ItemRepo, ItemSetRepo, Paging, PropertyRepo, UserRepo, ValueRepo,
    VocabularyRepo,
};
use omeka_db::rows::{
    ItemRow, NewClass, NewItem, NewItemSet, NewProperty, NewResource, NewUser, NewValue,
    NewVocabulary,
};
use omeka_db::{migrate, open_file};
use sqlx::SqlitePool;

use super::catalog::Catalog;
use super::params::{Params, Window, sorted_page};
use super::query::SearchQuery;
use super::relations::{Page, Relations};
use super::{Api, resources};
use crate::auth::Actor;
use crate::jsonld::BaseUrl;

/// Requests timed per stage. The median of an odd count is the middle sample.
const RUNS: usize = 21;
/// The item set the probe's request filters on.
const ITEMS: usize = 200;
/// The page the probe's request reads, the harness's `per_page`.
const PER_PAGE: usize = 25;

struct Props {
    title: PropertyId,
    creator: PropertyId,
    description: PropertyId,
    date: PropertyId,
    subject: PropertyId,
    source: PropertyId,
}

fn median(mut samples: Vec<Duration>) -> Duration {
    samples.sort();
    samples[samples.len() / 2]
}

fn micros(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1_000_000.0
}

fn report(name: &str, duration: Duration) {
    println!("{name:<34} {:>9.1} us", micros(duration));
}

async fn open_probe_db() -> (SqlitePool, PathBuf) {
    let dir = PathBuf::from("/tmp/opencode");
    std::fs::create_dir_all(&dir).expect("probe temp dir");
    let path = dir.join(format!("listperf2-probe-{}.db", std::process::id()));
    let _ = std::fs::remove_file(&path);
    let pool = open_file(&path).await.expect("probe database");
    migrate(&pool).await.expect("probe migration");
    (pool, path)
}

/// The harness's `bench.py` `item_body`: five properties, one value each, plus a
/// URI source value.
async fn seed(pool: &SqlitePool) -> (ItemSetId, UserId) {
    let user = UserRepo::new(pool)
        .create(&NewUser {
            email: "probe@example.com".to_string(),
            name: "Probe".to_string(),
            password_hash: None,
            role: "global_admin".to_string(),
            is_active: true,
            created: None,
        })
        .await
        .expect("probe user")
        .id;
    let terms = install_vocabularies(pool).await;
    let find = |term: &str| -> PropertyId {
        terms
            .iter()
            .find(|(name, _)| name == term)
            .map(|(_, id)| *id)
            .expect("a default property")
    };
    let props = Props {
        title: find("dcterms:title"),
        creator: find("dcterms:creator"),
        description: find("dcterms:description"),
        date: find("dcterms:date"),
        subject: find("dcterms:subject"),
        source: find("dcterms:source"),
    };
    let set = ItemSetRepo::new(pool)
        .create(&NewItemSet {
            resource: NewResource {
                owner_id: Some(user),
                title: Some("Perf harness item set".to_string()),
                ..NewResource::default()
            },
            is_open: false,
        })
        .await
        .expect("probe item set")
        .id();
    for index in 1..=ITEMS {
        let item = ItemRepo::new(pool)
            .create(&NewItem {
                resource: NewResource {
                    owner_id: Some(user),
                    title: Some(format!("Perf item {index:04}")),
                    ..NewResource::default()
                },
                primary_media_id: None,
            })
            .await
            .expect("probe item")
            .id();
        ItemRepo::new(pool)
            .add_to_item_set(item, set)
            .await
            .expect("probe membership");
        let texts = [
            (props.title, "Perf item".to_string()),
            (props.creator, format!("Creator {}", index % 10)),
            (
                props.description,
                format!("Deterministic description for item {index}. ").repeat(3),
            ),
            (props.date, format!("2026-01-{:02}", ((index - 1) % 28) + 1)),
            (props.subject, format!("Subject {}", index % 7)),
        ];
        for (property, value) in texts {
            add_value(
                pool,
                item,
                property,
                "literal",
                Some(value),
                None,
                Some("en"),
            )
            .await;
        }
        add_value(
            pool,
            item,
            props.source,
            "uri",
            None,
            Some(format!("https://example.org/sources/perf-item-{index:04}")),
            None,
        )
        .await;
    }
    (set, user)
}

/// Insert the install's four default vocabularies, so the catalog the probe
/// loads holds the same rows the served database holds. Returns each property's
/// `prefix:localName` with its id.
async fn install_vocabularies(pool: &SqlitePool) -> Vec<(String, PropertyId)> {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../omeka-cli/data/default-vocabularies.json"
    );
    let raw = std::fs::read_to_string(path).expect("default vocabularies file");
    let data: serde_json::Value = serde_json::from_str(&raw).expect("default vocabularies json");
    let mut terms = Vec::new();
    for vocabulary in data["vocabularies"].as_array().expect("vocabularies array") {
        let prefix = vocabulary["prefix"].as_str().expect("prefix").to_string();
        let row = VocabularyRepo::new(pool)
            .create(&NewVocabulary {
                owner_id: None,
                namespace_uri: vocabulary["namespace_uri"]
                    .as_str()
                    .expect("namespace")
                    .to_string(),
                prefix: prefix.clone(),
                label: vocabulary["label"].as_str().expect("label").to_string(),
                comment: vocabulary["comment"].as_str().map(str::to_string),
            })
            .await
            .expect("probe vocabulary");
        for class in vocabulary["classes"].as_array().expect("classes") {
            ClassRepo::new(pool)
                .create(&NewClass {
                    owner_id: None,
                    vocabulary_id: row.id,
                    local_name: class["local_name"]
                        .as_str()
                        .expect("local name")
                        .to_string(),
                    label: class["label"].as_str().expect("label").to_string(),
                    comment: class["comment"].as_str().map(str::to_string),
                })
                .await
                .expect("probe class");
        }
        for property in vocabulary["properties"].as_array().expect("properties") {
            let local_name = property["local_name"].as_str().expect("local name");
            let id = PropertyRepo::new(pool)
                .create(&NewProperty {
                    owner_id: None,
                    vocabulary_id: row.id,
                    local_name: local_name.to_string(),
                    label: property["label"].as_str().expect("label").to_string(),
                    comment: property["comment"].as_str().map(str::to_string),
                })
                .await
                .expect("probe property")
                .id;
            terms.push((format!("{prefix}:{local_name}"), id));
        }
    }
    terms
}

async fn add_value(
    pool: &SqlitePool,
    item: ItemId,
    property: PropertyId,
    kind: &str,
    value: Option<String>,
    uri: Option<String>,
    lang: Option<&str>,
) {
    ValueRepo::new(pool)
        .create(&NewValue {
            resource_id: item.as_resource(),
            property_id: property,
            value_resource_id: None,
            value_annotation_id: None,
            r#type: kind.to_string(),
            lang: lang.map(str::to_string),
            value,
            uri,
            is_public: true,
        })
        .await
        .expect("probe value");
}

fn item_filter() -> ItemFilter {
    ItemFilter::default()
}

fn id_cmp(a: &ItemRow, b: &ItemRow) -> std::cmp::Ordering {
    a.resource.id.cmp(&b.resource.id)
}

/// Time the whole list request and each stage it runs, then print the table.
#[tokio::test]
#[ignore = "a release-mode probe, not a behavioural test"]
async fn list_cost_split() {
    let (pool, path) = open_probe_db().await;
    let (set, user) = seed(&pool).await;
    let base = BaseUrl::parse("http://localhost").expect("base url");
    let actor = Actor::new(user, "global_admin");
    let api = Api::load(&pool, base, actor.clone())
        .await
        .expect("probe api");
    let query = format!("item_set_id={}&per_page={PER_PAGE}", set.0);
    let params = Params::parse(&query);
    let window = Window::from_params(&params);
    let search = SearchQuery::parse(
        &params,
        api.catalog(),
        super::query::media_visibility(api.actor()),
    );
    let filter = item_filter();

    println!("== list_cost: {ITEMS} items, page of {PER_PAGE}, {RUNS} runs per stage");

    let mut samples = Vec::with_capacity(RUNS);
    for _ in 0..RUNS {
        let start = Instant::now();
        let _ = Api::load(
            &pool,
            BaseUrl::parse("http://localhost").expect("base url"),
            actor.clone(),
        )
        .await
        .expect("api load");
        samples.push(start.elapsed());
    }
    report("api load (catalog, narrow)", median(samples));

    let mut samples = Vec::with_capacity(RUNS);
    for _ in 0..RUNS {
        let start = Instant::now();
        let rows = ItemRepo::new(&pool)
            .list_filtered(&filter, &search.filter, &Paging::default())
            .await
            .expect("list_filtered");
        assert_eq!(rows.len(), ITEMS);
        samples.push(start.elapsed());
    }
    report("list_filtered (all 200 rows)", median(samples));

    // The window's SQL shapes against the whole-set load.
    let predicate = " WHERE 1 = 1 AND EXISTS (SELECT 1 FROM item_item_set \
                     WHERE item_item_set.item_id = resource.id \
                     AND item_item_set.item_set_id IN (?))";
    assert!(super::window::eligible(
        &filter,
        &search,
        &super::params::Sort::from_params(&params)
    ));

    let mut samples = Vec::with_capacity(RUNS);
    for _ in 0..RUNS {
        let start = Instant::now();
        let sql = format!(
            "SELECT resource.*, item.primary_media_id FROM item JOIN resource ON resource.id = item.id\
             {predicate} ORDER BY resource.id LIMIT ? OFFSET ?"
        );
        let rows: Vec<sqlx::sqlite::SqliteRow> = sqlx::query(&sql)
            .bind(set.0 as i64)
            .bind(PER_PAGE as i64)
            .bind(0i64)
            .fetch_all(&pool)
            .await
            .expect("page");
        assert_eq!(rows.len(), PER_PAGE);
        let parsed: Vec<ItemRow> = rows
            .iter()
            .map(|row| ItemRow::from_row(row).expect("item row"))
            .collect();
        assert_eq!(parsed.len(), PER_PAGE);
        samples.push(start.elapsed());
    }
    report("  page only (25 rows, 1 query)", median(samples));

    let mut samples = Vec::with_capacity(RUNS);
    for _ in 0..RUNS {
        let start = Instant::now();
        let sql =
            format!("SELECT COUNT(*) FROM item JOIN resource ON resource.id = item.id{predicate}");
        let total: i64 = sqlx::query_scalar(&sql)
            .bind(set.0 as i64)
            .fetch_one(&pool)
            .await
            .expect("count");
        assert_eq!(total as usize, ITEMS);
        samples.push(start.elapsed());
    }
    report("  count only (1 query)", median(samples));

    let mut samples = Vec::with_capacity(RUNS);
    for _ in 0..RUNS {
        let start = Instant::now();
        let page = super::window::read(&api, &filter, &search, &window)
            .await
            .expect("window");
        assert_eq!(page.total as usize, ITEMS);
        assert_eq!(page.rows.len(), PER_PAGE);
        samples.push(start.elapsed());
    }
    report("  window read (COUNT OVER, 1 query)", median(samples));

    let rows = ItemRepo::new(&pool)
        .list_filtered(&filter, &search.filter, &Paging::default())
        .await
        .expect("list_filtered");
    let mut samples = Vec::with_capacity(RUNS);
    for _ in 0..RUNS {
        let start = Instant::now();
        let (total, page) = sorted_page(
            rows.clone(),
            &super::params::Sort::from_params(&params),
            &window,
            id_cmp,
        );
        assert_eq!(total as usize, ITEMS);
        assert_eq!(page.len(), PER_PAGE);
        samples.push(start.elapsed());
    }
    report("sort + slice window", median(samples));

    let (_, page) = sorted_page(
        rows,
        &super::params::Sort::from_params(&params),
        &window,
        id_cmp,
    );
    let targets: Vec<(u64, Option<omeka_db::ids::TemplateId>)> = page
        .iter()
        .map(|row| (row.resource.id.0, row.resource.resource_template_id))
        .collect();
    let mut samples = Vec::with_capacity(RUNS);
    for _ in 0..RUNS {
        let start = Instant::now();
        let relations = Relations::load(&api, Page::Items, targets.iter().copied())
            .await
            .expect("relations");
        assert_eq!(relations.values_of(page[0].resource.id.0).len(), 6);
        samples.push(start.elapsed());
    }
    report("relations load (5 queries)", median(samples));

    let ids: Vec<u64> = targets.iter().map(|(id, _)| *id).collect();
    let mut samples = Vec::with_capacity(RUNS);
    for _ in 0..RUNS {
        let start = Instant::now();
        let rows = super::relations::media_for(&pool, &ids)
            .await
            .expect("media_for");
        assert!(rows.is_empty());
        samples.push(start.elapsed());
    }
    report("  media_for (25 ids, 0 rows)", median(samples));

    let mut samples = Vec::with_capacity(RUNS);
    for _ in 0..RUNS {
        let start = Instant::now();
        let rows = super::relations::item_sets_for(&pool, &ids)
            .await
            .expect("item_sets_for");
        assert_eq!(rows.len(), PER_PAGE);
        samples.push(start.elapsed());
    }
    report("  item_sets_for (25 ids, 25 rows)", median(samples));

    let mut samples = Vec::with_capacity(RUNS);
    for _ in 0..RUNS {
        let start = Instant::now();
        let rows = super::relations::values_for(&pool, &ids)
            .await
            .expect("values_for");
        assert_eq!(rows.len(), PER_PAGE);
        samples.push(start.elapsed());
    }
    report("  values_for (25 ids, 150 rows)", median(samples));

    let mut samples = Vec::with_capacity(RUNS);
    for _ in 0..RUNS {
        let start = Instant::now();
        let rows = super::relations::pointing_at(&pool, &ids)
            .await
            .expect("pointing_at");
        assert!(rows.is_empty());
        samples.push(start.elapsed());
    }
    report("  pointing_at (25 ids, 0 rows)", median(samples));

    let relations = Relations::load(&api, Page::Items, targets.iter().copied())
        .await
        .expect("relations");
    let mut samples = Vec::with_capacity(RUNS);
    let mut documents = Vec::new();
    for _ in 0..RUNS {
        let start = Instant::now();
        documents = page
            .iter()
            .map(|row| resources::item_doc(&api, row, &relations))
            .collect();
        samples.push(start.elapsed());
    }
    report("build 25 documents", median(samples));

    let mut samples = Vec::with_capacity(RUNS);
    for _ in 0..RUNS {
        let start = Instant::now();
        let body = super::response::serialize(&documents).expect("serialize");
        assert!(body.len() > 30_000);
        samples.push(start.elapsed());
    }
    report("serialize page (39 KB)", median(samples));

    let mut samples = Vec::with_capacity(RUNS);
    for _ in 0..RUNS {
        let start = Instant::now();
        let response = resources::search_items(&api, &params)
            .await
            .expect("search");
        assert!(response.body.len() > 30_000);
        samples.push(start.elapsed());
    }
    report("search_items end to end", median(samples));

    let mut samples = Vec::with_capacity(RUNS);
    for _ in 0..RUNS {
        let start = Instant::now();
        let _ = Catalog::load(&pool).await.expect("catalog");
        samples.push(start.elapsed());
    }
    report("catalog load (full, as served before)", median(samples));

    let mut samples = Vec::with_capacity(RUNS);
    for _ in 0..RUNS {
        let start = Instant::now();
        let _ = Catalog::load_for_read(&pool).await.expect("catalog");
        samples.push(start.elapsed());
    }
    report("catalog load_for_read (as served now)", median(samples));

    pool.close().await;
    let _ = std::fs::remove_file(&path);
}
