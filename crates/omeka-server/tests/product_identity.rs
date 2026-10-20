//! The product name and the wire facts it must not reach, in one file.
//!
//! Every test here fails if a later rename eats the compatibility surface. The
//! `o:` prefix, the version header and the upstream class name in an error
//! document are what a client or a database depends on. The title and the
//! footer are what the rename owns.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::{
    empty_router, get, header as header_value, request, router, seeded_pool, send, site_pool,
};

/// The upstream vocabulary origin, spelled out so a rename of the constant that
/// holds it still fails this test.
const OMEKA_VOCABULARY_IRI: &str = "http://omeka.org/s/vocabs/o#";
/// The version the wire reports, spelled out for the same reason.
const OMEKA_VERSION: &str = "4.3.0-alpha";

/// The `o:` prefix and the reported version are upstream's, not the product's.
#[tokio::test]
async fn an_api_document_keeps_the_o_prefix_and_the_version_header() {
    let (pool, seed) = seeded_pool().await;
    let answer = send(
        router(pool),
        get(&format!("/api/items/{}", seed.item.0), false),
    )
    .await;

    assert_eq!(
        header_value(&answer, "omeka-s-version"),
        Some(OMEKA_VERSION),
        "the version header is a compatibility fact"
    );
    assert_eq!(
        header_value(&answer, "omeka-s-total-results"),
        None,
        "a member answer carries no listing header"
    );
    let document = answer.json();
    assert_eq!(
        document["@context"], "http://localhost/api-context",
        "a member document points at the context path"
    );
    assert_eq!(document["o:id"], seed.item.0, "o: members carry the id");

    let (pool, _seed) = seeded_pool().await;
    let context = send(router(pool), get("/api-context", false)).await.json();
    assert_eq!(
        context["@context"]["o"], OMEKA_VOCABULARY_IRI,
        "the JSON-LD prefix maps to upstream's vocabulary"
    );
}

/// A permission error names the upstream class a client may match byte for byte.
#[tokio::test]
async fn a_permission_error_keeps_the_upstream_class_name() {
    let answer = send(empty_router().await, request("DELETE", "/api/items/1")).await;

    assert_eq!(
        answer.body,
        r#"{"errors":{"error":"Permission denied for the current user to delete the Omeka\\Api\\Adapter\\ItemAdapter resource."}}"#
    );
}

/// The product's own name reaches the page, in the title and the footer.
#[tokio::test]
async fn a_rendered_page_carries_the_product_name() {
    let (pool, _seed) = site_pool().await;
    let answer = send(router(pool), get("/s/museum/page/welcome", false)).await;

    assert!(
        answer.body.contains("<title>Welcome · Romeka S</title>"),
        "{}",
        answer.body
    );
    assert!(
        answer
            .body
            .contains("<p class=\"powered-by\">Powered by Romeka S</p>"),
        "{}",
        answer.body
    );
}
