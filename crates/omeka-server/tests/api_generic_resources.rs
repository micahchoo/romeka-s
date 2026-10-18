//! The generic `resources` GETs: `ResourceAdapter::search` and `::read`.
//!
//! `ResourceAdapter::getRepresentationClass` is null and every method proxies
//! to the adapter named by the row's `getResourceName`, so a list carries
//! documents of every kind and a member answers the document its own endpoint
//! answers.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::http::StatusCode;
use common::{get, router, seeded_pool, send};

/// The seed creates one item set, one item, one media and one value annotation:
/// four `resource` rows. An anonymous caller sees every one of them, because
/// each is public.
#[tokio::test]
async fn the_collection_lists_every_resource_kind() {
    let (pool, seed) = seeded_pool().await;
    let answer = send(router(pool), get("/api/resources", false)).await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    let documents = answer.json();
    let documents = documents.as_array().unwrap();
    assert_eq!(documents.len(), 4);

    let by_id: Vec<u64> = documents
        .iter()
        .map(|document| document["o:id"].as_u64().unwrap())
        .collect();
    let mut sorted = by_id.clone();
    sorted.sort_unstable();
    assert_eq!(by_id, sorted, "the page is ordered by id");

    let at = |id: u64| {
        documents
            .iter()
            .find(|document| document["o:id"].as_u64() == Some(id))
            .unwrap()
    };
    assert_eq!(
        at(seed.item.0)["@id"],
        format!("http://localhost/api/items/{}", seed.item.0)
    );
    assert_eq!(
        at(seed.item_set.0)["@id"],
        format!("http://localhost/api/item_sets/{}", seed.item_set.0)
    );
    assert_eq!(
        at(seed.media.0)["@id"],
        format!("http://localhost/api/media/{}", seed.media.0)
    );
    assert_eq!(
        at(seed.annotation)["@id"],
        format!("http://localhost/api/value_annotations/{}", seed.annotation)
    );
    assert_eq!(at(seed.item.0)["o:title"], "A picture of a cat");
    assert_eq!(at(seed.item_set.0)["o:is_open"], true);
    assert_eq!(at(seed.media.0)["o:ingester"], "upload");
    assert_eq!(at(seed.annotation)["o:title"], "Editorial note");
}

/// A member answers the document its own kind's endpoint answers, drawn from
/// the same row: `ResourceAdapter::getRepresentation` proxies to the target
/// adapter. The per-kind adapter access check runs too, so a value annotation
/// read through the generic path is as privileged as its own endpoint.
#[tokio::test]
async fn a_member_answers_the_document_its_kind_owns() {
    let (pool, seed) = seeded_pool().await;
    for (id, resource) in [
        (seed.item.0, "items"),
        (seed.item_set.0, "item_sets"),
        (seed.media.0, "media"),
        (seed.annotation, "value_annotations"),
    ] {
        let generic = send(
            router(pool.clone()),
            get(&format!("/api/resources/{id}"), true),
        )
        .await;
        let specific = send(
            router(pool.clone()),
            get(&format!("/api/{resource}/{id}"), true),
        )
        .await;
        assert_eq!(generic.status, StatusCode::OK, "{}", generic.body);
        assert_eq!(
            generic.body, specific.body,
            "the {resource} document differs from /api/resources/{id}"
        );
    }

    // `JobAdapter`, `ModuleAdapter` and `ValueAnnotationAdapter` carry no
    // unauthenticated rule, so the annotation stays a privileged read here.
    let answer = send(
        router(pool),
        get(&format!("/api/resources/{}", seed.annotation), false),
    )
    .await;
    assert_eq!(answer.status, StatusCode::FORBIDDEN);
    assert_eq!(
        answer.body,
        r#"{"errors":{"error":"Permission denied for the current user to read the Omeka\\Api\\Adapter\\ValueAnnotationAdapter resource."}}"#
    );
}

/// The base adapter's `owner_id` and `resource_class_id` filters narrow the
/// page, and `search` matches the title.
#[tokio::test]
async fn the_collection_applies_the_base_filters() {
    let (pool, seed) = seeded_pool().await;
    let answer = send(
        router(pool.clone()),
        get(&format!("/api/resources?owner_id={}", seed.user.0), false),
    )
    .await;
    assert_eq!(answer.json().as_array().unwrap().len(), 4);

    let answer = send(
        router(pool.clone()),
        get(
            &format!("/api/resources?resource_class_id={}", seed.class.0),
            false,
        ),
    )
    .await;
    let documents = answer.json();
    let documents = documents.as_array().unwrap();
    assert_eq!(documents.len(), 1, "only the item carries the class");
    assert_eq!(documents[0]["o:id"], seed.item.0);

    let answer = send(
        router(pool.clone()),
        get("/api/resources?search=editorial", false),
    )
    .await;
    let documents = answer.json();
    let documents = documents.as_array().unwrap();
    assert_eq!(documents.len(), 1);
    assert_eq!(documents[0]["o:id"], seed.annotation);
}

/// `ResourceAdapter::findEntity` runs on the abstract `Resource` entity, so a
/// missing id is the `Omeka\Entity\Resource` 404 whatever the id would name.
#[tokio::test]
async fn a_missing_member_is_the_resource_404() {
    let (pool, _seed) = seeded_pool().await;
    let answer = send(router(pool), get("/api/resources/999999", false)).await;
    assert_eq!(answer.status, StatusCode::NOT_FOUND);
    assert_eq!(
        answer.body,
        r#"{"errors":{"error":"Omeka\\Entity\\Resource entity with criteria {\"id\":\"999999\"} not found"}}"#
    );
}
