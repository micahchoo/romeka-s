//! Tags and list merges on a published term list (`omeka_api::tagging`, `docs/compat.md`): a
//! merge or a promotion may add and deprecate published terms, never remove one.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;
mod tag_fixtures;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::{append_key, json_request, router, send};
use serde_json::{Value, json};
use sqlx::SqlitePool;
use tag_fixtures::{
    Archive, Caller, archive, item, item_as, literal, pairs, post, read, values_of,
};

async fn list(pool: &SqlitePool, label: &str, terms: &[&str]) -> u64 {
    let created = post(
        pool,
        "/api/custom_vocabs",
        &json!({"o:label": label, "o:lang": "en", "o:terms": terms}),
    )
    .await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.body);
    created.json()["o:id"].as_u64().unwrap()
}

async fn publish(pool: &SqlitePool, id: u64) -> Value {
    let answer = post(
        pool,
        &format!("/api/custom_vocabs/{id}/publication"),
        &json!({"action": "publish"}),
    )
    .await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    answer.json()
}

async fn turtle(pool: &SqlitePool, path: &str) -> String {
    let answer = send(
        router(pool.clone()),
        Request::builder()
            .uri(path)
            .header("accept", "text/turtle")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(answer.status, StatusCode::OK, "{path}: {}", answer.body);
    answer.body
}

fn term<'v>(view: &'v Value, key: &str) -> &'v Value {
    view["terms"]
        .as_array()
        .unwrap()
        .iter()
        .find(|term| term["key"] == key)
        .unwrap_or_else(|| panic!("no term {key} in {view}"))
}

/// An item whose `dcterms:spatial` holds these words of the list.
async fn placed(archive: &Archive, list: u64, words: &[&str]) -> u64 {
    let values: Vec<Value> = words
        .iter()
        .map(|word| {
            json!({"type": format!("customvocab:{list}"), "property_id": archive.spatial.0, "@value": word})
        })
        .collect();
    item_as(
        &archive.pool,
        Caller::Admin,
        "Map",
        vec![],
        json!({"dcterms:spatial": values}),
    )
    .await
}

/// Merging away a published word moves the values to the kept spelling, but the word stays in
/// the list, deprecated and replaced by the kept one, so its URI keeps resolving.
#[tokio::test]
async fn a_merge_keeps_a_published_word_as_deprecated_and_replaced() {
    let archive = archive().await;
    let pool = &archive.pool;
    let places = list(pool, "Places", &["Cork", "Cork City", "Bantry"]).await;
    publish(pool, places).await;
    let map = placed(&archive, places, &["Cork City"]).await;
    let name = format!("customvocab:{places}");
    let merge = format!("/api/custom_vocabs/{places}/merge");

    let done = post(pool, &merge, &json!({"from": ["Cork City"], "to": "Cork"})).await;
    assert_eq!(done.status, StatusCode::OK, "{}", done.body);
    assert_eq!(
        read(pool, &format!("/api/custom_vocabs/{places}")).await["o:terms"],
        json!(["Cork", "Cork City", "Bantry"])
    );
    assert_eq!(
        values_of(pool, map, "dcterms:spatial").await,
        pairs(&[(&name, "Cork")])
    );
    let pending = read(pool, &format!("/api/custom_vocabs/{places}/publication")).await;
    assert_eq!(pending["changed"], true);
    assert_eq!(term(&pending, "Cork City")["deprecated"], true);
    assert_eq!(term(&pending, "Cork City")["replaced_by"], "Cork");
    assert_eq!(term(&pending, "Cork")["replaced_by"], Value::Null);

    let second = publish(pool, places).await;
    assert_eq!(second["versions"].as_array().unwrap().len(), 2);
    let served = turtle(pool, "/vocab/places/2").await;
    assert!(served.contains("owl:deprecated true"), "{served}");
    assert!(
        served.contains("dcterms:isReplacedBy <http://localhost/vocab/places/1>"),
        "{served}"
    );
}

/// The dry run the admin's prompt reads names the published words a merge would deprecate, and
/// writes nothing; on a draft list it names none.
#[tokio::test]
async fn a_dry_run_names_the_words_a_merge_would_deprecate() {
    let archive = archive().await;
    let pool = &archive.pool;
    let places = list(pool, "Places", &["Cork", "Cork City"]).await;
    let draft = list(pool, "Towns", &["Cork", "Cork City"]).await;
    publish(pool, places).await;
    placed(&archive, places, &["Cork City"]).await;
    let body = json!({"from": ["Cork City"], "to": "Cork", "dry_run": true});

    let preview = post(pool, &format!("/api/custom_vocabs/{places}/merge"), &body).await;
    assert_eq!(
        preview.json(),
        json!({"changed": 1, "skipped": 0, "deprecated": ["Cork City"]})
    );
    let pending = read(pool, &format!("/api/custom_vocabs/{places}/publication")).await;
    assert_eq!(pending["changed"], false);
    let on_draft = post(pool, &format!("/api/custom_vocabs/{draft}/merge"), &body).await;
    assert_eq!(on_draft.json(), json!({"changed": 0, "skipped": 0}));
}

/// A word added since the last publish is not a promise yet, so a merge removes it as on a
/// draft; the kept spelling joins the list as a new term.
#[tokio::test]
async fn a_merge_removes_unpublished_words_and_adds_the_kept_spelling() {
    let archive = archive().await;
    let pool = &archive.pool;
    let places = list(pool, "Places", &["Cork City", "Bantry"]).await;
    publish(pool, places).await;
    let patched = send(
        router(pool.clone()),
        json_request(
            "PATCH",
            &append_key(&format!("/api/custom_vocabs/{places}")),
            &json!({"o:terms": ["Cork City", "Bantry", "Co. Cork"]}).to_string(),
        ),
    )
    .await;
    assert_eq!(patched.status, StatusCode::OK, "{}", patched.body);

    let done = post(
        pool,
        &format!("/api/custom_vocabs/{places}/merge"),
        &json!({"from": ["Cork City", "Co. Cork"], "to": "Cork"}),
    )
    .await;
    assert_eq!(done.status, StatusCode::OK, "{}", done.body);
    assert_eq!(done.json()["deprecated"], json!(["Cork City"]));
    assert_eq!(
        read(pool, &format!("/api/custom_vocabs/{places}")).await["o:terms"],
        json!(["Cork", "Cork City", "Bantry"])
    );
    let published = publish(pool, places).await;
    assert_eq!(term(&published, "Cork")["published"], true);
    assert_eq!(term(&published, "Cork City")["deprecation_published"], true);
}

/// A later merge of the replacement moves the earlier replacement along, so a deprecated term
/// never points at another deprecated term.
#[tokio::test]
async fn a_second_merge_moves_the_earlier_replacement_along() {
    let archive = archive().await;
    let pool = &archive.pool;
    let places = list(pool, "Places", &["Cork", "Cork City", "Corcaigh"]).await;
    publish(pool, places).await;
    let merge = format!("/api/custom_vocabs/{places}/merge");
    post(pool, &merge, &json!({"from": ["Cork City"], "to": "Cork"})).await;
    post(pool, &merge, &json!({"from": ["Cork"], "to": "Corcaigh"})).await;

    let pending = read(pool, &format!("/api/custom_vocabs/{places}/publication")).await;
    assert_eq!(term(&pending, "Cork City")["replaced_by"], "Corcaigh");
    assert_eq!(term(&pending, "Cork")["replaced_by"], "Corcaigh");
    assert_eq!(term(&pending, "Corcaigh")["deprecated"], false);
}

/// Merging back the other way never leaves a term replaced by itself.
#[tokio::test]
async fn a_merge_back_clears_the_replacement_of_the_kept_spelling() {
    let archive = archive().await;
    let pool = &archive.pool;
    let places = list(pool, "Places", &["Cork", "Cork City"]).await;
    publish(pool, places).await;
    let merge = format!("/api/custom_vocabs/{places}/merge");
    post(pool, &merge, &json!({"from": ["Cork City"], "to": "Cork"})).await;
    post(pool, &merge, &json!({"from": ["Cork"], "to": "Cork City"})).await;

    let pending = read(pool, &format!("/api/custom_vocabs/{places}/publication")).await;
    assert_eq!(term(&pending, "Cork")["replaced_by"], "Cork City");
    assert_eq!(term(&pending, "Cork City")["replaced_by"], Value::Null);
}

/// The page a person reads links a deprecated term to its replacement and counts terms in
/// English.
#[tokio::test]
async fn the_pages_link_a_replaced_term_and_count_terms() {
    let archive = archive().await;
    let pool = &archive.pool;
    let places = list(pool, "Places", &["Cork", "Cork City"]).await;
    let towns = list(pool, "Towns", &["Bantry"]).await;
    publish(pool, places).await;
    publish(pool, towns).await;
    post(
        pool,
        &format!("/api/custom_vocabs/{places}/merge"),
        &json!({"from": ["Cork City"], "to": "Cork"}),
    )
    .await;
    publish(pool, places).await;

    let page = |path: &'static str| async move {
        send(
            router(pool.clone()),
            Request::builder()
                .uri(path)
                .header("accept", "text/html")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .body
    };
    let term_page = page("/vocab/places/2").await;
    assert!(
        term_page.contains("Replaced by <a href=\"/vocab/places/1\">Cork</a>."),
        "{term_page}"
    );
    assert!(page("/vocab/places/").await.contains(">2 terms</caption>"));
    assert!(page("/vocab/towns/").await.contains(">1 term</caption>"));
}

/// Promoting tags into a published list only adds words: the published ones keep their URIs.
#[tokio::test]
async fn a_promotion_into_a_published_list_adds_terms() {
    let archive = archive().await;
    let pool = &archive.pool;
    let places = list(pool, "Places", &["Cork", "Bantry"]).await;
    publish(pool, places).await;
    item(pool, "Harbour", vec![literal("Kinsale"), literal("Cork")]).await;

    let promoted = post(
        pool,
        "/api/tags/promote",
        &json!({"tags": ["Kinsale", "Cork"], "list": "Places"}),
    )
    .await;
    assert_eq!(promoted.status, StatusCode::OK, "{}", promoted.body);
    assert_eq!(
        read(pool, &format!("/api/custom_vocabs/{places}")).await["o:terms"],
        json!(["Cork", "Bantry", "Kinsale"])
    );
    let second = publish(pool, places).await;
    assert_eq!(
        term(&second, "Cork")["uri"],
        "http://localhost/vocab/places/1"
    );
    assert_eq!(
        term(&second, "Kinsale")["uri"],
        "http://localhost/vocab/places/3"
    );
}
