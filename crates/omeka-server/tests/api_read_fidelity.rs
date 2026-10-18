//! The read surface honours upstream's query parameters.
//!
//! Six behaviours the port answered differently from Omeka S at `5e413f0`, each
//! found by the 82-step oracle probe of 2026-10-17. Every test asserts what an
//! HTTP caller observes, so each fails on the port before the fix and passes
//! after.
//!
//! Upstream sources, named per test: `AbstractEntityAdapter::buildBaseQuery`
//! (the `id` filter), `AbstractEntityAdapter::limitQuery` (a non-numeric
//! `page`), `PropertyAdapter::buildQuery` and `ResourceClassAdapter::buildQuery`
//! (`term`, `vocabulary_prefix`, `vocabulary_namespace_uri`), the
//! `DataTypeAdapter` / `ApiResourceAdapter` / `ModuleAdapter` plain `Response`
//! (no `Omeka-S-Total-Results`) and `Laminas\Uri\Http::setQuery` with
//! `Laminas\Uri\Uri::normalizeQuery` (the `Link` query encoding).

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::{Answer, get, header, router, seeded_pool, send};
use omeka_db::ids::UserId;
use omeka_db::repos::{ItemRepo, ItemSetRepo, Paging, PropertyFilter, PropertyRepo};
use omeka_db::rows::{NewItem, NewItemSet, NewProperty, NewResource};
use serde_json::Value;
use sqlx::SqlitePool;

/// A public item with a title, owned by `owner`.
async fn picture_item(pool: &SqlitePool, title: &str, owner: UserId) -> u64 {
    ItemRepo::new(pool)
        .create(&NewItem {
            resource: NewResource {
                owner_id: Some(owner),
                title: Some(title.to_string()),
                is_public: true,
                ..NewResource::default()
            },
            primary_media_id: None,
        })
        .await
        .unwrap()
        .id()
        .0
}

/// A public item set holding every id in `items`, owned by `owner`.
async fn item_set_holding(pool: &SqlitePool, title: &str, owner: UserId, items: &[u64]) -> u64 {
    let set = ItemSetRepo::new(pool)
        .create(&NewItemSet {
            resource: NewResource {
                owner_id: Some(owner),
                title: Some(title.to_string()),
                is_public: true,
                ..NewResource::default()
            },
            is_open: false,
        })
        .await
        .unwrap();
    for item in items {
        ItemRepo::new(pool)
            .add_to_item_set(omeka_db::ids::ItemId(*item), set.id())
            .await
            .unwrap();
    }
    set.id().0
}

/// The `dcterms:title` property of the seeded database.
async fn title_property(pool: &SqlitePool) -> u64 {
    PropertyRepo::new(pool)
        .list(
            &PropertyFilter {
                local_name: Some("title".to_string()),
                ..PropertyFilter::default()
            },
            &Paging::default(),
        )
        .await
        .unwrap()
        .first()
        .expect("the seed creates dcterms:title")
        .id
        .0
}

/// A property on the seeded `dcterms` vocabulary, so a search sees two.
async fn a_second_property(pool: &SqlitePool, vocabulary: u64) -> u64 {
    PropertyRepo::new(pool)
        .create(&NewProperty {
            owner_id: None,
            vocabulary_id: omeka_db::ids::VocabularyId(vocabulary),
            local_name: "description".to_string(),
            label: "Description".to_string(),
            comment: None,
        })
        .await
        .unwrap()
        .id
        .0
}

/// The documents of a list body, in order.
fn documents(answer: &Answer) -> Vec<Value> {
    answer
        .json()
        .as_array()
        .cloned()
        .expect("a list body is an array")
}

/// The `o:id` of every document in a list body, in order.
fn ids(answer: &Answer) -> Vec<u64> {
    documents(answer)
        .iter()
        .map(|document| document["o:id"].as_u64().expect("o:id is a number"))
        .collect()
}

// ------------------------------------------------------------------ id filter

/// `AbstractEntityAdapter::buildBaseQuery`: `id[]` narrows every resource
/// search to the listed ids. The port parsed the parameter into a list and
/// never applied it, so it answered every row (`x_ids_filter`: probe 2, port 8).
#[tokio::test]
async fn id_array_filter_narrows_the_item_search() {
    let (pool, seed) = seeded_pool().await;
    let second = picture_item(&pool, "A second picture", seed.user).await;
    let _third = picture_item(&pool, "A third picture", seed.user).await;

    let uri = format!("/api/items?id[]={}&id[]={}", seed.item.0, second);
    let answer = send(router(pool), get(&uri, true)).await;

    assert_eq!(answer.status, axum::http::StatusCode::OK);
    assert_eq!(ids(&answer), vec![seed.item.0, second]);
    assert_eq!(header(&answer, "omeka-s-total-results"), Some("2"));
}

/// The `id` filter reaches the other entity collections too: upstream runs
/// `buildBaseQuery` before every adapter's own `buildQuery`.
#[tokio::test]
async fn id_array_filter_narrows_the_property_search() {
    let (pool, seed) = seeded_pool().await;
    let title = title_property(&pool).await;
    let other = a_second_property(&pool, seed.vocabulary.0).await;
    let uri = format!("/api/properties?id[]={title}");
    let answer = send(router(pool), get(&uri, true)).await;

    assert_eq!(answer.status, axum::http::StatusCode::OK);
    assert_eq!(header(&answer, "omeka-s-total-results"), Some("1"));
    assert_eq!(ids(&answer), vec![title]);
    assert_ne!(title, other);
}

// ------------------------------------------------------- vocabulary members

/// `PropertyAdapter::buildQuery`: `term` is `prefix:localName` and narrows to
/// one property. The port answered all 183 properties (`x_properties_by_term`).
#[tokio::test]
async fn property_term_filter_matches_upstream() {
    let (pool, seed) = seeded_pool().await;
    let _other = a_second_property(&pool, seed.vocabulary.0).await;
    let answer = send(
        router(pool),
        get("/api/properties?term=dcterms:title", true),
    )
    .await;

    assert_eq!(answer.status, axum::http::StatusCode::OK);
    assert_eq!(header(&answer, "omeka-s-total-results"), Some("1"));
    let documents = documents(&answer);
    assert_eq!(documents.len(), 1);
    assert_eq!(documents[0]["o:term"], "dcterms:title");
}

/// `ResourceClassAdapter::buildQuery`: `vocabulary_prefix` and `local_name`
/// narrow together. The port ignored the prefix, so `dcterms` + `Agent` matched
/// the `dctype` class as well (`x_classes_filter`: probe 1, port 2).
#[tokio::test]
async fn class_vocabulary_prefix_filter_matches_upstream() {
    let (pool, seed) = seeded_pool().await;
    let class = seed.class.0;
    let answer = send(
        router(pool.clone()),
        get(
            "/api/resource_classes?vocabulary_prefix=dctype&local_name=Image",
            true,
        ),
    )
    .await;
    assert_eq!(answer.status, axum::http::StatusCode::OK);
    assert_eq!(header(&answer, "omeka-s-total-results"), Some("1"));
    assert_eq!(ids(&answer), vec![class]);

    // The other vocabulary's prefix matches no class, so the list is empty.
    let mismatch = send(
        router(pool),
        get(
            "/api/resource_classes?vocabulary_prefix=dcterms&local_name=Image",
            true,
        ),
    )
    .await;
    assert_eq!(header(&mismatch, "omeka-s-total-results"), Some("0"));
    assert!(ids(&mismatch).is_empty());
}

/// `PropertyAdapter::buildQuery` reads `vocabulary_namespace_uri` the same way.
#[tokio::test]
async fn class_vocabulary_namespace_uri_filter_matches_upstream() {
    let (pool, seed) = seeded_pool().await;
    let answer = send(
        router(pool.clone()),
        get(
            "/api/resource_classes?vocabulary_namespace_uri=http://purl.org/dc/dcmitype/\
             &local_name=Image",
            true,
        ),
    )
    .await;
    assert_eq!(header(&answer, "omeka-s-total-results"), Some("1"));
    assert_eq!(ids(&answer), vec![seed.class.0]);

    let mismatch = send(
        router(pool),
        get(
            "/api/resource_classes?vocabulary_namespace_uri=http://example.invalid/\
             &local_name=Image",
            true,
        ),
    )
    .await;
    assert_eq!(header(&mismatch, "omeka-s-total-results"), Some("0"));
    assert!(ids(&mismatch).is_empty());
}

// ----------------------------------------------------------------- the page

/// `AbstractEntityAdapter::limitQuery`: the page window is applied only when
/// `page` is numeric. `page=abc` leaves the query unbounded, so upstream
/// answers every row and the paginator still reads `(int) 'abc'` as page 1 for
/// the links (`x_page_text`: probe 7, port 2).
#[tokio::test]
async fn a_non_numeric_page_does_not_open_a_window() {
    let (pool, seed) = seeded_pool().await;
    let second = picture_item(&pool, "A second picture", seed.user).await;
    let third = picture_item(&pool, "A third picture", seed.user).await;
    let set = item_set_holding(
        &pool,
        "Fidelity set",
        seed.user,
        &[seed.item.0, second, third],
    )
    .await;

    let unbounded = send(
        router(pool.clone()),
        get(
            &format!("/api/items?item_set_id={set}&page=abc&per_page=1"),
            true,
        ),
    )
    .await;
    assert_eq!(unbounded.status, axum::http::StatusCode::OK);
    assert_eq!(header(&unbounded, "omeka-s-total-results"), Some("3"));
    assert_eq!(ids(&unbounded), vec![seed.item.0, second, third]);
    let link = header(&unbounded, "link").unwrap();
    assert!(link.contains("page=1"), "link was {link}");

    // A numeric page keeps the window; `page=0` and a negative page clamp to 1.
    for query in [
        "page=1&per_page=1",
        "page=0&per_page=1",
        "page=-1&per_page=1",
    ] {
        let bounded = send(
            router(pool.clone()),
            get(&format!("/api/items?item_set_id={set}&{query}"), true),
        )
        .await;
        assert_eq!(
            ids(&bounded),
            vec![seed.item.0],
            "query was {query}, total {:?}",
            header(&bounded, "omeka-s-total-results")
        );
    }
}

// ------------------------------------------------------------- total header

/// `DataTypeAdapter::search` builds a plain `Response`, whose
/// `getTotalResults()` is null, so `ApiController::getList` adds no
/// `Omeka-S-Total-Results` (`x_data_types`). `api_resources` and `modules` are
/// the other two non-entity adapters, and like `data_types` they answer every
/// registered row whatever `per_page` says.
#[tokio::test]
async fn collections_without_a_total_carry_no_total_header() {
    let (pool, _seed) = seeded_pool().await;

    let data_types = send(router(pool.clone()), get("/api/data_types", false)).await;
    assert_eq!(data_types.status, axum::http::StatusCode::OK);
    assert_eq!(header(&data_types, "omeka-s-total-results"), None);
    assert_eq!(documents(&data_types).len(), 6);

    let paged = send(
        router(pool.clone()),
        get("/api/data_types?per_page=2", false),
    )
    .await;
    assert_eq!(header(&paged, "omeka-s-total-results"), None);
    assert_eq!(documents(&paged).len(), 6);

    let api_resources = send(router(pool.clone()), get("/api/api_resources", false)).await;
    assert_eq!(header(&api_resources, "omeka-s-total-results"), None);
    assert!(documents(&api_resources).len() > 6);

    let modules = send(router(pool), get("/api/modules", true)).await;
    assert_eq!(modules.status, axum::http::StatusCode::OK);
    assert_eq!(header(&modules, "omeka-s-total-results"), None);
}

// ------------------------------------------------------------- link encoding

/// `Laminas\Uri\Http::setQuery` runs `http_build_query` and then
/// `Uri::normalizeQuery`, so an array parameter keeps its bracket notation with
/// both brackets percent-encoded and each element indexed from zero
/// (`x_ids_filter`: upstream `id%5B0%5D=149&id%5B1%5D=150`).
#[tokio::test]
async fn link_header_indexes_and_encodes_array_parameters() {
    let (pool, seed) = seeded_pool().await;
    let second = picture_item(&pool, "A second picture", seed.user).await;
    let uri = format!(
        "/api/items?id[]={}&id[]={}&sort_by=id&sort_order=asc",
        seed.item.0, second
    );
    let answer = send(router(pool), get(&uri, true)).await;

    let link = header(&answer, "link").unwrap();
    let (_, query) = link.split_once("/api/items?").expect("a link query");
    assert!(
        query.starts_with(&format!(
            "id%5B0%5D={}&id%5B1%5D={}&sort_by=id&sort_order=asc&",
            seed.item.0, second
        )),
        "link was {link}"
    );
    assert!(!query.contains("id[]="), "link was {link}");
    assert!(query.contains("page=1"), "link was {link}");
}

/// The same encoding for a nested array and for a value that needs escaping:
/// upstream `property%5B0%5D%5Bproperty%5D=1` and `URI%20value%20item%20updated`
/// (`x_prop_search_eq`, `x_prop_search_in`, `x_prop_search_ex`,
/// `x_prop_search_nin`, `x_owner_filter`).
#[tokio::test]
async fn link_header_encodes_nested_parameters_and_spaces() {
    let (pool, seed) = seeded_pool().await;
    let uri = format!(
        "/api/items?item_set_id={}&property[0][property]=dcterms:title&property[0][type]=in\
         &property[0][text]=URI%20value",
        seed.item_set.0
    );
    let answer = send(router(pool), get(&uri, true)).await;

    let link = header(&answer, "link").unwrap();
    assert!(
        link.contains("property%5B0%5D%5Bproperty%5D=dcterms:title"),
        "link was {link}"
    );
    assert!(
        link.contains("property%5B0%5D%5Btext%5D=URI%20value"),
        "link was {link}"
    );
    assert!(!link.contains("property[0]"), "link was {link}");
}
