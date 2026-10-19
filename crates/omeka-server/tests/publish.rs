//! Publishing through the router: the admin's `/api/<resource>/<id>/publication`, the public
//! `/vocab/<slug>/` it serves, the negotiation of each representation, and the rule that a
//! published term can only be added or deprecated (`docs/compat.md`, Romeka's own rule).

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::{Answer, append_key, header, json_request, router, seeded_pool, send};
use serde_json::{Value, json};
use sqlx::SqlitePool;

async fn call(pool: &SqlitePool, method: &str, path: &str, body: &Value) -> Answer {
    send(
        router(pool.clone()),
        json_request(method, &append_key(path), &body.to_string()),
    )
    .await
}

async fn fetch(pool: &SqlitePool, path: &str, accept: Option<&str>) -> Answer {
    let mut request = Request::builder().method("GET").uri(path);
    if let Some(accept) = accept {
        request = request.header("accept", accept);
    }
    send(router(pool.clone()), request.body(Body::empty()).unwrap()).await
}

async fn view(pool: &SqlitePool, path: &str) -> Value {
    let answer = send(
        router(pool.clone()),
        Request::builder()
            .method("GET")
            .uri(append_key(path))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    answer.json()
}

/// A term list of three places, published once; returns its id.
async fn published_places(pool: &SqlitePool) -> u64 {
    let created = call(
        pool,
        "POST",
        "/api/custom_vocabs",
        &json!({"o:label": "Irish places", "o:lang": "en", "o:terms": "Cork\nDublin\nGalway"}),
    )
    .await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.body);
    let id = created.json()["o:id"].as_u64().unwrap();
    let published = publish(pool, id, json!({"action": "publish"})).await;
    assert_eq!(published.status, StatusCode::OK, "{}", published.body);
    id
}

async fn publish(pool: &SqlitePool, id: u64, body: Value) -> Answer {
    call(
        pool,
        "POST",
        &format!("/api/custom_vocabs/{id}/publication"),
        &body,
    )
    .await
}

fn errors(answer: &Answer) -> Vec<String> {
    answer.json()["errors"]
        .as_object()
        .unwrap()
        .values()
        .flat_map(|messages| messages.as_array().unwrap().clone())
        .map(|message| message.as_str().unwrap().to_string())
        .collect()
}

#[tokio::test]
async fn a_draft_list_shows_its_suggested_address_and_serves_nothing() {
    let (pool, _) = seeded_pool().await;
    let created = call(
        &pool,
        "POST",
        "/api/custom_vocabs",
        &json!({"o:label": "Irish places", "o:lang": "en", "o:terms": "Cork\nDublin"}),
    )
    .await;
    let id = created.json()["o:id"].as_u64().unwrap();

    let draft = view(&pool, &format!("/api/custom_vocabs/{id}/publication")).await;

    assert_eq!(draft["state"], "draft");
    assert_eq!(draft["url"], "http://localhost/vocab/irish-places/");
    assert_eq!(
        draft["terms"][1]["uri"],
        "http://localhost/vocab/irish-places/2"
    );
    assert_eq!(draft["changed"], true);
    assert!(
        draft["preview"]
            .as_str()
            .unwrap()
            .contains("<http://localhost/vocab/irish-places/1> a skos:Concept"),
        "{}",
        draft["preview"]
    );
    assert_eq!(
        fetch(&pool, "/vocab/irish-places/", None).await.status,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn a_published_list_negotiates_each_representation() {
    let (pool, _) = seeded_pool().await;
    published_places(&pool).await;

    let turtle = fetch(&pool, "/vocab/irish-places/", Some("text/turtle")).await;
    assert_eq!(turtle.status, StatusCode::OK);
    assert_eq!(
        header(&turtle, "content-type"),
        Some("text/turtle; charset=utf-8")
    );
    assert_eq!(header(&turtle, "vary"), Some("Accept"));
    assert_eq!(header(&turtle, "access-control-allow-origin"), Some("*"));
    assert!(
        turtle.body.contains("skos:ConceptScheme"),
        "{}",
        turtle.body
    );
    assert!(
        turtle.body.contains("skos:prefLabel \"Galway\"@en"),
        "{}",
        turtle.body
    );

    let json_ld = fetch(&pool, "/vocab/irish-places", Some("application/ld+json")).await;
    assert_eq!(
        header(&json_ld, "content-type"),
        Some("application/ld+json")
    );
    let ids: Vec<String> = json_ld
        .json()
        .as_array()
        .unwrap()
        .iter()
        .map(|node| node["@id"].as_str().unwrap().to_string())
        .collect();
    assert!(
        ids.contains(&"http://localhost/vocab/irish-places/3".to_string()),
        "{ids:?}"
    );

    let xml = fetch(&pool, "/vocab/irish-places/", Some("application/rdf+xml")).await;
    assert_eq!(header(&xml, "content-type"), Some("application/rdf+xml"));
    assert!(xml.body.contains("<rdf:RDF"), "{}", xml.body);

    let browser = "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8";
    let page = fetch(&pool, "/vocab/irish-places/", Some(browser)).await;
    assert_eq!(
        header(&page, "content-type"),
        Some("text/html; charset=utf-8")
    );
    assert!(page.body.contains("<h1>Irish places</h1>"), "{}", page.body);
    assert!(
        page.body
            .contains("href=\"/vocab/irish-places/2\">Dublin</a>"),
        "{}",
        page.body
    );

    let curl = fetch(&pool, "/vocab/irish-places/", Some("*/*")).await;
    assert_eq!(
        header(&curl, "content-type"),
        Some("text/turtle; charset=utf-8")
    );

    let named = fetch(&pool, "/vocab/irish-places/?format=nt", Some("text/html")).await;
    assert_eq!(
        header(&named, "content-type"),
        Some("application/n-triples; charset=utf-8")
    );

    let refused = fetch(&pool, "/vocab/irish-places/", Some("image/png")).await;
    assert_eq!(refused.status, StatusCode::NOT_ACCEPTABLE);
}

#[tokio::test]
async fn a_term_uri_serves_that_term() {
    let (pool, _) = seeded_pool().await;
    published_places(&pool).await;

    let turtle = fetch(&pool, "/vocab/irish-places/2", Some("text/turtle")).await;
    assert_eq!(turtle.status, StatusCode::OK);
    assert!(
        turtle
            .body
            .contains("<http://localhost/vocab/irish-places/2> a skos:Concept")
    );
    assert!(!turtle.body.contains("\"Cork\""), "{}", turtle.body);

    let page = fetch(&pool, "/vocab/irish-places/2", Some("text/html")).await;
    assert!(page.body.contains("<h1>Dublin</h1>"), "{}", page.body);

    assert_eq!(
        fetch(&pool, "/vocab/irish-places/9", None).await.status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        fetch(&pool, "/vocab/nowhere/", None).await.status,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn a_published_term_cannot_be_removed_or_renamed() {
    let (pool, _) = seeded_pool().await;
    let id = published_places(&pool).await;

    let renamed = call(
        &pool,
        "PATCH",
        &format!("/api/custom_vocabs/{id}"),
        &json!({"o:terms": "Cork\nDubln"}),
    )
    .await;

    assert_eq!(renamed.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        errors(&renamed),
        [
            "A published term cannot be removed or renamed: \"Dublin\", \"Galway\". Deprecate it instead."
        ]
    );
    let stored = view(&pool, &format!("/api/custom_vocabs/{id}")).await;
    assert_eq!(stored["o:terms"], json!(["Cork", "Dublin", "Galway"]));

    let to_items = call(
        &pool,
        "PUT",
        &format!("/api/custom_vocabs/{id}"),
        &json!({"o:label": "Irish places", "o:item_set": {"o:id": 1}}),
    )
    .await;
    assert_eq!(
        to_items.status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "{}",
        to_items.body
    );

    let deleted = call(
        &pool,
        "DELETE",
        &format!("/api/custom_vocabs/{id}"),
        &json!({}),
    )
    .await;
    assert_eq!(deleted.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        errors(&deleted),
        ["A published term list or vocabulary cannot be deleted. Deprecate its terms instead."]
    );
}

#[tokio::test]
async fn an_added_term_gets_the_next_uri_and_a_new_version() {
    let (pool, _) = seeded_pool().await;
    let id = published_places(&pool).await;

    let added = call(
        &pool,
        "PATCH",
        &format!("/api/custom_vocabs/{id}"),
        &json!({"o:terms": "Ennis\nCork\nDublin\nGalway"}),
    )
    .await;
    assert_eq!(added.status, StatusCode::OK, "{}", added.body);
    let pending = view(&pool, &format!("/api/custom_vocabs/{id}/publication")).await;
    assert_eq!(pending["changed"], true);
    assert_eq!(
        pending["terms"][0]["uri"],
        "http://localhost/vocab/irish-places/4"
    );
    assert_eq!(pending["terms"][0]["published"], false);

    let second = publish(&pool, id, json!({"action": "publish"}))
        .await
        .json();
    assert_eq!(second["versions"].as_array().unwrap().len(), 2);
    assert_eq!(second["changed"], false);
    let again = publish(&pool, id, json!({"action": "publish"}))
        .await
        .json();
    assert_eq!(
        again["versions"].as_array().unwrap().len(),
        2,
        "nothing changed"
    );

    let latest = fetch(&pool, "/vocab/irish-places/4", Some("text/turtle")).await;
    assert!(
        latest.body.contains("skos:prefLabel \"Ennis\"@en"),
        "{}",
        latest.body
    );
    let first = fetch(&pool, "/vocab/irish-places/?version=1", Some("text/turtle")).await;
    assert!(!first.body.contains("Ennis"), "version 1 is frozen");
    assert!(
        first.body.contains("owl:versionInfo \"1\""),
        "{}",
        first.body
    );
}

#[tokio::test]
async fn a_deprecation_is_published_and_then_stays() {
    let (pool, _) = seeded_pool().await;
    let id = published_places(&pool).await;

    let deprecated = publish(&pool, id, json!({"action": "deprecate", "key": "Galway"})).await;
    assert_eq!(deprecated.status, StatusCode::OK, "{}", deprecated.body);
    assert_eq!(deprecated.json()["terms"][2]["deprecated"], true);
    publish(&pool, id, json!({"action": "publish"})).await;

    let galway = fetch(&pool, "/vocab/irish-places/3", Some("text/turtle")).await;
    assert!(
        galway.body.contains("owl:deprecated true"),
        "{}",
        galway.body
    );
    let undone = publish(&pool, id, json!({"action": "undeprecate", "key": "Galway"})).await;
    assert_eq!(undone.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        errors(&undone),
        ["The deprecation of \"Galway\" is published and cannot be taken back."]
    );
}

#[tokio::test]
async fn a_draft_term_is_removed_rather_than_deprecated() {
    let (pool, _) = seeded_pool().await;
    let id = published_places(&pool).await;
    call(
        &pool,
        "PATCH",
        &format!("/api/custom_vocabs/{id}"),
        &json!({"o:terms": "Cork\nDublin\nGalway\nEnnis"}),
    )
    .await;

    let refused = publish(&pool, id, json!({"action": "deprecate", "key": "Ennis"})).await;
    assert_eq!(refused.status, StatusCode::UNPROCESSABLE_ENTITY);
    let removed = call(
        &pool,
        "PATCH",
        &format!("/api/custom_vocabs/{id}"),
        &json!({"o:terms": "Cork\nDublin\nGalway"}),
    )
    .await;
    assert_eq!(removed.status, StatusCode::OK, "{}", removed.body);
}

#[tokio::test]
async fn a_term_links_to_an_authority() {
    let (pool, _) = seeded_pool().await;
    let id = published_places(&pool).await;

    let linked = publish(
        &pool,
        id,
        json!({
            "action": "link",
            "key": "Cork",
            "exact_match": "https://sws.geonames.org/2965140/",
            "close_match": " "
        }),
    )
    .await;
    assert_eq!(linked.status, StatusCode::OK, "{}", linked.body);
    assert_eq!(
        linked.json()["terms"][0]["exact_match"],
        "https://sws.geonames.org/2965140/"
    );
    assert_eq!(linked.json()["terms"][0]["close_match"], Value::Null);
    publish(&pool, id, json!({"action": "publish"})).await;
    let cork = fetch(&pool, "/vocab/irish-places/1", Some("text/turtle")).await;
    assert!(
        cork.body
            .contains("skos:exactMatch <https://sws.geonames.org/2965140/>"),
        "{}",
        cork.body
    );

    let bad = publish(
        &pool,
        id,
        json!({"action": "link", "key": "Cork", "exact_match": "geonames 2965140"}),
    )
    .await;
    assert_eq!(bad.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        errors(&bad),
        ["A match must be an absolute http or https URI."]
    );
}

#[tokio::test]
async fn a_persistent_namespace_mints_the_term_uris_and_the_address_stays() {
    let (pool, _) = seeded_pool().await;
    let created = call(
        &pool,
        "POST",
        "/api/custom_vocabs",
        &json!({"o:label": "Places", "o:terms": "Cork"}),
    )
    .await;
    let id = created.json()["o:id"].as_u64().unwrap();

    let bad = publish(
        &pool,
        id,
        json!({"action": "publish", "slug": "places", "namespace": "https://example.org/places/"}),
    )
    .await;
    assert_eq!(bad.status, StatusCode::UNPROCESSABLE_ENTITY);

    let done = publish(
        &pool,
        id,
        json!({"action": "publish", "slug": "places", "namespace": "https://w3id.org/my-archive/places/"}),
    )
    .await;
    assert_eq!(done.status, StatusCode::OK, "{}", done.body);
    assert_eq!(done.json()["url"], "http://localhost/vocab/places/");
    let served = fetch(&pool, "/vocab/places/1", Some("text/turtle")).await;
    assert!(
        served
            .body
            .contains("<https://w3id.org/my-archive/places/1> a skos:Concept")
    );

    let moved = publish(&pool, id, json!({"action": "publish", "slug": "elsewhere"})).await;
    assert_eq!(
        errors(&moved),
        ["A published term list or vocabulary keeps the address it was first published at."]
    );

    let other = call(
        &pool,
        "POST",
        "/api/custom_vocabs",
        &json!({"o:label": "More places", "o:terms": "Ennis"}),
    )
    .await;
    let other = other.json()["o:id"].as_u64().unwrap();
    let taken = publish(&pool, other, json!({"action": "publish", "slug": "places"})).await;
    assert_eq!(
        errors(&taken),
        ["The address /vocab/places/ is already taken."]
    );
}

#[tokio::test]
async fn publishing_needs_the_right_to_change_the_list() {
    let (pool, _) = seeded_pool().await;
    let created = call(
        &pool,
        "POST",
        "/api/custom_vocabs",
        &json!({"o:label": "Places", "o:terms": "Cork"}),
    )
    .await;
    let id = created.json()["o:id"].as_u64().unwrap();

    let anonymous = send(
        router(pool.clone()),
        json_request(
            "POST",
            &format!("/api/custom_vocabs/{id}/publication"),
            &json!({"action": "publish"}).to_string(),
        ),
    )
    .await;

    assert_eq!(
        anonymous.status,
        StatusCode::FORBIDDEN,
        "{}",
        anonymous.body
    );
}

/// The local vocabulary "Create a new field" makes, with one property.
async fn local_vocabulary(pool: &SqlitePool) -> (u64, u64) {
    let created = call(
        pool,
        "POST",
        "/api/vocabularies",
        &json!({
            "o:namespace_uri": "http://localhost/vocab/local/",
            "o:prefix": "local",
            "o:label": "Local fields",
            "o:comment": "Fields this archive made for itself.",
        }),
    )
    .await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.body);
    let vocabulary = created.json()["o:id"].as_u64().unwrap();
    let property = call(
        pool,
        "POST",
        "/api/properties",
        &json!({
            "o:local_name": "numberOfPages",
            "o:label": "Number of pages",
            "o:vocabulary": {"o:id": vocabulary},
        }),
    )
    .await;
    assert_eq!(property.status, StatusCode::OK, "{}", property.body);
    (vocabulary, property.json()["o:id"].as_u64().unwrap())
}

#[tokio::test]
async fn the_local_vocabulary_publishes_at_its_own_namespace() {
    let (pool, _) = seeded_pool().await;
    let (vocabulary, _) = local_vocabulary(&pool).await;

    let draft = view(
        &pool,
        &format!("/api/vocabularies/{vocabulary}/publication"),
    )
    .await;
    assert_eq!(draft["slug"], "local");
    assert_eq!(draft["slug_fixed"], true);
    let published = call(
        &pool,
        "POST",
        &format!("/api/vocabularies/{vocabulary}/publication"),
        &json!({"action": "publish"}),
    )
    .await;
    assert_eq!(published.status, StatusCode::OK, "{}", published.body);

    let term = fetch(&pool, "/vocab/local/numberOfPages", Some("text/turtle")).await;
    assert_eq!(term.status, StatusCode::OK);
    assert!(
        term.body
            .contains("<http://localhost/vocab/local/numberOfPages> a rdf:Property"),
        "{}",
        term.body
    );
    assert!(
        term.body
            .contains("rdfs:isDefinedBy <http://localhost/vocab/local/>")
    );
    let scheme = fetch(&pool, "/vocab/local/", Some("text/turtle")).await;
    assert!(scheme.body.contains("a owl:Ontology"), "{}", scheme.body);
}

#[tokio::test]
async fn a_published_vocabulary_keeps_its_terms_and_namespace() {
    let (pool, _) = seeded_pool().await;
    let (vocabulary, property) = local_vocabulary(&pool).await;
    call(
        &pool,
        "POST",
        &format!("/api/vocabularies/{vocabulary}/publication"),
        &json!({"action": "publish"}),
    )
    .await;

    let renamed = call(
        &pool,
        "PATCH",
        &format!("/api/properties/{property}"),
        &json!({"o:local_name": "pageCount"}),
    )
    .await;
    assert_eq!(
        renamed.status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "{}",
        renamed.body
    );
    assert_eq!(
        errors(&renamed),
        ["A published term cannot be removed or renamed: \"numberOfPages\". Deprecate it instead."]
    );

    let relabelled = call(
        &pool,
        "PATCH",
        &format!("/api/properties/{property}"),
        &json!({"o:label": "Page count"}),
    )
    .await;
    assert_eq!(relabelled.status, StatusCode::OK, "{}", relabelled.body);

    let deleted = call(
        &pool,
        "DELETE",
        &format!("/api/properties/{property}"),
        &json!({}),
    )
    .await;
    assert_eq!(deleted.status, StatusCode::UNPROCESSABLE_ENTITY);

    let moved = call(
        &pool,
        "PATCH",
        &format!("/api/vocabularies/{vocabulary}"),
        &json!({"o:namespace_uri": "http://localhost/vocab/fields/"}),
    )
    .await;
    assert_eq!(
        errors(&moved),
        ["A published vocabulary keeps its namespace, so its terms keep their URIs."]
    );

    let gone = call(
        &pool,
        "DELETE",
        &format!("/api/vocabularies/{vocabulary}"),
        &json!({}),
    )
    .await;
    assert_eq!(gone.status, StatusCode::UNPROCESSABLE_ENTITY);

    let added = call(
        &pool,
        "POST",
        "/api/properties",
        &json!({
            "o:local_name": "binding",
            "o:label": "Binding",
            "o:vocabulary": {"o:id": vocabulary},
        }),
    )
    .await;
    assert_eq!(added.status, StatusCode::OK, "{}", added.body);
}

#[tokio::test]
async fn a_bundled_vocabulary_is_not_this_archive_to_publish() {
    let (pool, seed) = seeded_pool().await;
    let path = format!("/api/vocabularies/{}/publication", seed.vocabulary.0);

    let shown = view(&pool, &path).await;
    let refused = call(&pool, "POST", &path, &json!({"action": "publish"})).await;

    assert!(
        shown["refusal"]
            .as_str()
            .unwrap()
            .starts_with("The namespace http://purl.org/dc/terms/ is not this archive's."),
        "{shown}"
    );
    assert_eq!(refused.status, StatusCode::UNPROCESSABLE_ENTITY);
}
