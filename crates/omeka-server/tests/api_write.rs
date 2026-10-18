//! Write-endpoint integration tests: create and update through the router,
//! asserting the same document the read layer serves, plus one failure per
//! error shape (malformed body, missing member, unknown property, permission).
//!
//! Every write answers with the read representation of the row it just wrote,
//! so a create and a following `GET` must return the same body.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::http::StatusCode;
use common::{append_key, json_request, seeded_pool, seeded_router, send};
use serde_json::json;

fn keyed(uri: &str) -> String {
    append_key(uri)
}

#[tokio::test]
async fn create_item_then_read_back() {
    let (app, seed) = seeded_router().await;
    let body = format!(
        r#"{{"o:item_set":[{{"o:id":{}}}],"o:is_public":true,
           "dcterms:title":[{{"property_id":"auto","type":"literal","@value":"Written item","@language":"en"}}]}}"#,
        seed.item_set.0
    );
    let created = send(
        app.clone(),
        json_request("POST", &keyed("/api/items"), &body),
    )
    .await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.body);
    let document = created.json();
    assert_eq!(document["dcterms:title"][0]["@value"], "Written item");
    assert_eq!(document["dcterms:title"][0]["@language"], "en");
    assert_eq!(document["o:item_set"][0]["o:id"], seed.item_set.0);
    assert_eq!(document["o:owner"]["o:id"], seed.user.0);
    let id = document["o:id"].as_u64().unwrap();

    let read = send(app, common::get(&format!("/api/items/{id}"), false)).await;
    assert_eq!(read.body, created.body, "create and read differ");
}

#[tokio::test]
async fn update_item_replaces_values_and_reads_back() {
    let (app, seed) = seeded_router().await;
    let body = r#"{"o:is_public":true,
        "dcterms:title":[{"property_id":"auto","type":"literal","@value":"Updated item"}]}"#;
    let updated = send(
        app.clone(),
        json_request("PUT", &keyed(&format!("/api/items/{}", seed.item.0)), body),
    )
    .await;
    assert_eq!(updated.status, StatusCode::OK, "{}", updated.body);
    let document = updated.json();
    assert_eq!(document["dcterms:title"][0]["@value"], "Updated item");
    assert_eq!(document["o:item_set"], json!([]));

    let read = send(
        app,
        common::get(&format!("/api/items/{}", seed.item.0), false),
    )
    .await;
    assert_eq!(read.body, updated.body, "update and read differ");
}

#[tokio::test]
async fn create_and_update_item_set() {
    let (app, _seed) = seeded_router().await;
    let body = r#"{"o:is_public":true,"o:is_open":true,
        "dcterms:title":[{"property_id":"auto","type":"literal","@value":"A new set"}]}"#;
    let created = send(
        app.clone(),
        json_request("POST", &keyed("/api/item_sets"), body),
    )
    .await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.body);
    assert_eq!(created.json()["o:is_open"], true);
    let id = created.json()["o:id"].as_u64().unwrap();

    let update = r#"{"o:is_public":true,"o:is_open":false,
        "dcterms:title":[{"property_id":"auto","type":"literal","@value":"Renamed set"}]}"#;
    let updated = send(
        app,
        json_request("PUT", &keyed(&format!("/api/item_sets/{id}")), update),
    )
    .await;
    assert_eq!(updated.status, StatusCode::OK, "{}", updated.body);
    assert_eq!(updated.json()["o:is_open"], false);
    assert_eq!(updated.json()["dcterms:title"][0]["@value"], "Renamed set");
}

#[tokio::test]
async fn create_media_purifies_an_html_ingester_body() {
    let (app, seed) = seeded_router().await;
    let body = format!(
        r#"{{"o:ingester":"html","o:item":{{"o:id":{}}},"o:is_public":true,
           "data":{{"html":"<p>ok</p><script>alert(1)</script>"}}}}"#,
        seed.item.0
    );
    let created = send(
        app.clone(),
        json_request("POST", &keyed("/api/media"), &body),
    )
    .await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.body);
    let chars = created.json()["o-cnt:chars"].as_str().unwrap().to_string();
    assert!(!chars.contains("<script"), "{chars}");
    assert!(chars.contains("<p>ok</p>"), "{chars}");

    let id = created.json()["o:id"].as_u64().unwrap();
    let read = send(app, common::get(&format!("/api/media/{id}"), false)).await;
    assert_eq!(read.json()["o-cnt:chars"], chars, "{}", read.body);
}

#[tokio::test]
async fn create_and_update_media() {
    let (app, seed) = seeded_router().await;
    let body = format!(
        r#"{{"o:ingester":"html","o:item":{{"o:id":{}}},"o:is_public":true,
           "data":{{"html":"<p>Written media</p>"}}}}"#,
        seed.item.0
    );
    let created = send(
        app.clone(),
        json_request("POST", &keyed("/api/media"), &body),
    )
    .await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.body);
    let document = created.json();
    assert_eq!(document["o:ingester"], "html");
    assert_eq!(document["o:renderer"], "html");
    assert_eq!(document["@type"], "o-cnt:ContentAsText");
    assert_eq!(document["o:item"]["o:id"], seed.item.0);
    assert_eq!(document["o-cnt:chars"], "<p>Written media</p>");
    let id = document["o:id"].as_u64().unwrap();

    let read = send(app.clone(), common::get(&format!("/api/media/{id}"), false)).await;
    assert_eq!(read.body, created.body, "create and read differ");

    let update = r#"{"o:is_public":true,"o:lang":"en","o:alt_text":"A caption"}"#;
    let updated = send(
        app,
        json_request("PUT", &keyed(&format!("/api/media/{id}")), update),
    )
    .await;
    assert_eq!(updated.status, StatusCode::OK, "{}", updated.body);
    assert_eq!(updated.json()["o:alt_text"], "A caption");
    assert_eq!(updated.json()["o:lang"], "en");
}

#[tokio::test]
async fn create_and_update_site() {
    let (app, _seed) = seeded_router().await;
    let body = r#"{"o:slug":"written-site","o:title":"Written site",
        "o:theme":"default","o:is_public":true}"#;
    let created = send(
        app.clone(),
        json_request("POST", &keyed("/api/sites"), body),
    )
    .await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.body);
    let document = created.json();
    assert_eq!(document["o:slug"], "written-site");
    assert_eq!(document["o:page"].as_array().unwrap().len(), 1);
    assert_eq!(document["o:site_permission"][0]["o:role"], "admin");
    assert_eq!(document["o:owner"]["o:id"], 1);
    let id = document["o:id"].as_u64().unwrap();

    let read = send(app.clone(), common::get(&format!("/api/sites/{id}"), false)).await;
    assert_eq!(read.body, created.body, "create and read differ");

    let update = r#"{"o:slug":"written-site","o:title":"Renamed site","o:theme":"default"}"#;
    let updated = send(
        app,
        json_request("PUT", &keyed(&format!("/api/sites/{id}")), update),
    )
    .await;
    assert_eq!(updated.status, StatusCode::OK, "{}", updated.body);
    assert_eq!(updated.json()["o:title"], "Renamed site");
}

#[tokio::test]
async fn update_site_page_replaces_blocks() {
    let (app, _seed) = seeded_router().await;
    let site = r#"{"o:slug":"page-site","o:title":"Page site","o:theme":"default"}"#;
    let created = send(
        app.clone(),
        json_request("POST", &keyed("/api/sites"), site),
    )
    .await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.body);
    let page_id = created.json()["o:page"][0]["o:id"].as_u64().unwrap();

    let body = r#"{"o:title":"Scenarios page","o:slug":"scenarios-page","o:is_public":true,
        "o:block":[{"o:layout":"html","o:data":{"html":"<p>First scenario block.</p>"}},
                   {"o:layout":"pageTitle","o:data":{}}]}"#;
    let updated = send(
        app.clone(),
        json_request("PUT", &keyed(&format!("/api/site_pages/{page_id}")), body),
    )
    .await;
    assert_eq!(updated.status, StatusCode::OK, "{}", updated.body);
    let document = updated.json();
    assert_eq!(document["o:title"], "Scenarios page");
    assert_eq!(document["o:slug"], "scenarios-page");
    assert_eq!(document["o:block"][0]["o:layout"], "html");
    assert_eq!(document["o:block"][1]["o:layout"], "pageTitle");
    // PHP `json_decode('{}', true)` yields an empty array, which re-encodes as [].
    assert_eq!(document["o:block"][1]["o:data"], json!([]));

    let read = send(
        app,
        common::get(&format!("/api/site_pages/{page_id}"), false),
    )
    .await;
    assert_eq!(read.body, updated.body, "update and read differ");
}

/// `SitePageAdapter::hydrateAttachments`: a media block's `o:attachment`
/// entries are stored in order and read back as `o:item`, `o:media` and
/// `o:caption`. A missing item keeps a null reference; the media is stored only
/// when it belongs to the item.
#[tokio::test]
async fn update_site_page_stores_block_attachments() {
    let (app, seed) = seeded_router().await;
    let site = r#"{"o:slug":"attach-site","o:title":"Attach site","o:theme":"default"}"#;
    let created = send(
        app.clone(),
        json_request("POST", &keyed("/api/sites"), site),
    )
    .await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.body);
    let page_id = created.json()["o:page"][0]["o:id"].as_u64().unwrap();

    let body = format!(
        r#"{{"o:title":"Attachments","o:slug":"attachments","o:is_public":true,
            "o:block":[{{"o:layout":"media","o:data":{{}},
              "o:attachment":[
                {{"o:item":{{"o:id":{item}}}}},
                {{"o:item":{{"o:id":{item}}},"o:media":{{"o:id":{media}}},"o:caption":"A caption"}},
                {{"o:item":{{"o:id":999999}},"o:caption":"Ghost"}}
              ]}}]}}"#,
        item = seed.item.0,
        media = seed.media.0,
    );
    let updated = send(
        app.clone(),
        json_request("PUT", &keyed(&format!("/api/site_pages/{page_id}")), &body),
    )
    .await;
    assert_eq!(updated.status, StatusCode::OK, "{}", updated.body);
    let document = updated.json();
    let attachments = document["o:block"][0]["o:attachment"].as_array().unwrap();
    assert_eq!(attachments.len(), 3, "{}", updated.body);
    // One item-only attachment: the item is present, the media null.
    assert_eq!(attachments[0]["o:item"]["o:id"], seed.item.0);
    assert!(attachments[0]["o:media"].is_null());
    assert_eq!(attachments[0]["o:caption"], "");
    // One attachment with the item's media and a caption.
    assert_eq!(attachments[1]["o:item"]["o:id"], seed.item.0);
    assert_eq!(attachments[1]["o:media"]["o:id"], seed.media.0);
    assert_eq!(attachments[1]["o:caption"], "A caption");
    // A missing item is stored as a null reference, not an error.
    assert!(attachments[2]["o:item"].is_null());
    assert!(attachments[2]["o:media"].is_null());
    assert_eq!(attachments[2]["o:caption"], "Ghost");

    let read = send(
        app,
        common::get(&format!("/api/site_pages/{page_id}"), false),
    )
    .await;
    assert_eq!(read.body, updated.body, "update and read differ");
}

/// `SitePageAdapter::hydrateAttachments`: a media that does not belong to the
/// item named beside it is dropped (`$item->getMedia()->get($id)` returns null),
/// with no error document.
#[tokio::test]
async fn site_page_attachment_media_must_belong_to_the_item() {
    let (app, seed) = seeded_router().await;
    let site = r#"{"o:slug":"attach-site","o:title":"Attach site","o:theme":"default"}"#;
    let created = send(
        app.clone(),
        json_request("POST", &keyed("/api/sites"), site),
    )
    .await;
    let page_id = created.json()["o:page"][0]["o:id"].as_u64().unwrap();

    // A second item that does not own the seeded media.
    let other = send(
        app.clone(),
        json_request("POST", &keyed("/api/items"), r#"{"o:is_public":true}"#),
    )
    .await;
    let other_item = other.json()["o:id"].as_u64().unwrap();
    assert_ne!(other_item, seed.item.0);

    let body = format!(
        r#"{{"o:title":"Attachments","o:slug":"attachments","o:is_public":true,
            "o:block":[{{"o:layout":"media","o:data":{{}},
              "o:attachment":[{{"o:item":{{"o:id":{other}}},"o:media":{{"o:id":{media}}}}}]}}]}}"#,
        other = other_item,
        media = seed.media.0,
    );
    let updated = send(
        app,
        json_request("PUT", &keyed(&format!("/api/site_pages/{page_id}")), &body),
    )
    .await;
    assert_eq!(updated.status, StatusCode::OK, "{}", updated.body);
    let attachment = &updated.json()["o:block"][0]["o:attachment"][0];
    assert_eq!(attachment["o:item"]["o:id"], other_item);
    assert!(attachment["o:media"].is_null(), "{}", updated.body);
}

#[tokio::test]
async fn create_and_update_user() {
    let (app, _seed) = seeded_router().await;
    let body = r#"{"o:name":"New User","o:email":"new@example.com","o:role":"researcher"}"#;
    let created = send(
        app.clone(),
        json_request("POST", &keyed("/api/users"), body),
    )
    .await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.body);
    let document = created.json();
    assert_eq!(document["o:is_active"], false);
    assert_eq!(document["o:role"], "researcher");
    let id = document["o:id"].as_u64().unwrap();

    let read = send(app.clone(), common::get(&format!("/api/users/{id}"), true)).await;
    assert_eq!(read.body, created.body, "create and read differ");

    let update = r#"{"o:name":"Renamed User","o:email":"new@example.com",
        "o:role":"author","o:is_active":true}"#;
    let updated = send(
        app,
        json_request("PUT", &keyed(&format!("/api/users/{id}")), update),
    )
    .await;
    assert_eq!(updated.status, StatusCode::OK, "{}", updated.body);
    assert_eq!(updated.json()["o:name"], "Renamed User");
    assert_eq!(updated.json()["o:role"], "author");
    assert_eq!(updated.json()["o:is_active"], true);
}

#[tokio::test]
async fn malformed_json_body_is_400() {
    let (app, _seed) = seeded_router().await;
    let answer = send(
        app,
        json_request("POST", &keyed("/api/items"), "{\"o:is_public\":"),
    )
    .await;
    assert_eq!(answer.status, StatusCode::BAD_REQUEST);
    assert_eq!(answer.body, r#"{"errors":{"error":"JSON: Syntax error"}}"#);
}

#[tokio::test]
async fn scalar_json_body_is_400() {
    let (app, _seed) = seeded_router().await;
    let answer = send(app, json_request("POST", &keyed("/api/items"), "12")).await;
    assert_eq!(answer.status, StatusCode::BAD_REQUEST);
    assert_eq!(
        answer.body,
        r#"{"errors":{"error":"JSON: Content must be an object or array."}}"#
    );
}

#[tokio::test]
async fn missing_required_member_is_422() {
    let (app, seed) = seeded_router().await;
    let body = format!(r#"{{"o:item":{{"o:id":{}}}}}"#, seed.item.0);
    let answer = send(app, json_request("POST", &keyed("/api/media"), &body)).await;
    assert_eq!(answer.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        answer.body,
        r#"{"errors":{"o:ingester":["Media must set an ingester."]}}"#
    );
}

#[tokio::test]
async fn unknown_property_is_422() {
    let (app, _seed) = seeded_router().await;
    let body = r#"{"dcterms:nonsense":[{"property_id":"auto","@value":"x"}]}"#;
    let answer = send(app, json_request("POST", &keyed("/api/items"), body)).await;
    assert_eq!(answer.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        answer.body,
        r#"{"errors":{"dcterms:nonsense":["The property \"dcterms:nonsense\" is not recognized."]}}"#
    );
}

#[tokio::test]
async fn anonymous_create_is_403() {
    let (app, _seed) = seeded_router().await;
    let body = r#"{"o:is_public":true}"#;
    let answer = send(app, json_request("POST", "/api/items", body)).await;
    assert_eq!(answer.status, StatusCode::FORBIDDEN);
    assert_eq!(
        answer.body,
        r#"{"errors":{"error":"Permission denied for the current user to create the Omeka\\Api\\Adapter\\ItemAdapter resource."}}"#
    );
}

#[tokio::test]
async fn anonymous_update_is_403() {
    let (app, seed) = seeded_router().await;
    let body = r#"{"o:is_public":true}"#;
    let answer = send(
        app,
        json_request("PUT", &format!("/api/items/{}", seed.item.0), body),
    )
    .await;
    assert_eq!(answer.status, StatusCode::FORBIDDEN);
    assert_eq!(
        answer.body,
        r#"{"errors":{"error":"Permission denied for the current user to update the Omeka\\Api\\Adapter\\ItemAdapter resource."}}"#
    );
}

/// `AbstractEntityAdapter::hydrateOwner`: an `o:owner` member that names no
/// `o:id` keeps the stored owner, while an explicit `{"o:id": null}` clears it.
#[tokio::test]
async fn an_owner_member_without_an_id_keeps_the_stored_owner() {
    let (app, seed) = seeded_router().await;
    let update = r#"{"o:is_public":true,"o:owner":{}}"#;
    let updated = send(
        app.clone(),
        json_request(
            "PUT",
            &keyed(&format!("/api/items/{}", seed.item.0)),
            update,
        ),
    )
    .await;
    assert_eq!(updated.status, StatusCode::OK, "{}", updated.body);
    assert_eq!(
        updated.json()["o:owner"]["o:id"],
        seed.user.0,
        "an empty o:owner cleared the stored owner"
    );

    let cleared = send(
        app,
        json_request(
            "PUT",
            &keyed(&format!("/api/items/{}", seed.item.0)),
            r#"{"o:is_public":true,"o:owner":{"o:id":null}}"#,
        ),
    )
    .await;
    assert_eq!(cleared.status, StatusCode::OK, "{}", cleared.body);
    assert!(cleared.json()["o:owner"].is_null(), "{}", cleared.body);
}

#[tokio::test]
async fn update_of_a_missing_row_is_404() {
    let (app, _seed) = seeded_router().await;
    let answer = send(
        app,
        json_request(
            "PUT",
            &keyed("/api/items/999999"),
            r#"{"o:is_public":true}"#,
        ),
    )
    .await;
    assert_eq!(answer.status, StatusCode::NOT_FOUND);
    assert_eq!(
        answer.body,
        r#"{"errors":{"error":"Omeka\\Entity\\Item entity with criteria {\"id\":\"999999\"} not found"}}"#
    );
}

#[tokio::test]
async fn an_author_cannot_update_another_users_item() {
    let (pool, seed) = seeded_pool().await;
    let author = omeka_db::repos::UserRepo::new(&pool)
        .create(&omeka_db::rows::NewUser {
            email: "author@example.com".to_string(),
            name: "Author".to_string(),
            password_hash: None,
            role: "author".to_string(),
            is_active: true,
            created: None,
        })
        .await
        .unwrap();
    let identity = "authorauthorauthorauthorauthor12";
    let credential = "author-credential";
    omeka_db::repos::ApiKeyRepo::new(&pool)
        .create(&omeka_db::rows::NewApiKey {
            id: identity.to_string(),
            owner_id: author.id,
            label: "Author key".to_string(),
            credential_hash: bcrypt::hash(credential, 4).unwrap(),
            last_ip: None,
            last_accessed: None,
            created: None,
        })
        .await
        .unwrap();
    let app = common::router(pool);
    let uri = format!(
        "/api/items/{}?key_identity={identity}&key_credential={credential}",
        seed.item.0
    );
    let answer = send(app, json_request("PUT", &uri, r#"{"o:is_public":true}"#)).await;
    assert_eq!(answer.status, StatusCode::FORBIDDEN);
    assert_eq!(
        answer.body,
        r#"{"errors":{"error":"Permission denied for the current user to update the Omeka\\Entity\\Item resource."}}"#
    );
}
