//! Write-endpoint tests for the vocabulary family and resource templates:
//! `vocabularies`, `resource_classes`, `properties` and `resource_templates`.
//!
//! Every test drives the router, so the answer is what a caller sees. The
//! upstream rule each test freezes is named in its doc comment.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::http::StatusCode;
use common::{append_key, get, json_request, seeded_pool, seeded_router, send};
use omeka_db::ids::VocabularyId;
use omeka_db::repos::{Paging, PropertyFilter, PropertyRepo};
use omeka_db::rows::NewProperty;
use sqlx::SqlitePool;

fn keyed(uri: &str) -> String {
    append_key(uri)
}

/// The vocabulary a create names, read back through its list endpoints.
#[tokio::test]
async fn create_a_vocabulary_with_its_classes_and_properties() {
    let (app, _seed) = seeded_router().await;
    let body = r#"{"o:namespace_uri":"http://example.org/ns#","o:prefix":"ex",
        "o:label":"Example","o:comment":"An example vocabulary.",
        "o:class":[{"o:local_name":"Thing","o:label":"Thing"}],
        "o:property":[{"o:local_name":"name","o:label":"Name","o:comment":"A name."}]}"#;
    let created = send(
        app.clone(),
        json_request("POST", &keyed("/api/vocabularies"), body),
    )
    .await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.body);
    let document = created.json();
    assert_eq!(document["o:namespace_uri"], "http://example.org/ns#");
    assert_eq!(document["o:prefix"], "ex");
    assert_eq!(document["o:label"], "Example");
    assert_eq!(document["o:owner"]["o:id"], 1);
    let id = document["o:id"].as_u64().unwrap();

    let read = send(
        app.clone(),
        common::get(&format!("/api/vocabularies/{id}"), false),
    )
    .await;
    assert_eq!(read.body, created.body, "create and read differ");

    let classes = send(
        app.clone(),
        get(&format!("/api/resource_classes?vocabulary_id={id}"), false),
    )
    .await;
    assert_eq!(classes.status, StatusCode::OK, "{}", classes.body);
    let classes = classes.json();
    assert_eq!(classes.as_array().unwrap().len(), 1);
    assert_eq!(classes[0]["o:local_name"], "Thing");
    assert_eq!(classes[0]["o:label"], "Thing");

    let properties = send(
        app,
        get(&format!("/api/properties?vocabulary_id={id}"), false),
    )
    .await;
    assert_eq!(properties.status, StatusCode::OK, "{}", properties.body);
    let properties = properties.json();
    assert_eq!(properties.as_array().unwrap().len(), 1);
    assert_eq!(properties[0]["o:local_name"], "name");
    assert_eq!(properties[0]["o:comment"], "A name.");
}

/// `VocabularyAdapter::hydrate`: an `o:class` entry with an id keeps that class,
/// an entry without one is created, and a member the list omits is removed
/// (`orphanRemoval` deletes the row). The same holds for `o:property`.
#[tokio::test]
async fn updating_a_vocabulary_replaces_its_member_lists() {
    let (app, _seed) = seeded_router().await;
    let body = r#"{"o:namespace_uri":"http://example.org/ns#","o:prefix":"ex2",
        "o:label":"Example two",
        "o:class":[{"o:local_name":"Thing","o:label":"Thing"}],
        "o:property":[{"o:local_name":"name","o:label":"Name"}]}"#;
    let created = send(
        app.clone(),
        json_request("POST", &keyed("/api/vocabularies"), body),
    )
    .await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.body);
    let id = created.json()["o:id"].as_u64().unwrap();
    let class_id = send(
        app.clone(),
        get(&format!("/api/resource_classes?vocabulary_id={id}"), false),
    )
    .await
    .json()[0]["o:id"]
        .as_u64()
        .unwrap();
    let property_id = send(
        app.clone(),
        get(&format!("/api/properties?vocabulary_id={id}"), false),
    )
    .await
    .json()[0]["o:id"]
        .as_u64()
        .unwrap();

    // Keep the created class, add a second, and drop every property.
    let update = format!(
        r#"{{"o:namespace_uri":"http://example.org/ns#","o:prefix":"ex2",
            "o:label":"Example two","o:class":[{{"o:id":{class_id}}},
            {{"o:local_name":"Agent","o:label":"Agent"}}],"o:property":[]}}"#
    );
    let updated = send(
        app.clone(),
        json_request("PUT", &keyed(&format!("/api/vocabularies/{id}")), &update),
    )
    .await;
    assert_eq!(updated.status, StatusCode::OK, "{}", updated.body);

    let classes = send(
        app.clone(),
        get(&format!("/api/resource_classes?vocabulary_id={id}"), false),
    )
    .await
    .json();
    let local_names: Vec<&str> = classes
        .as_array()
        .unwrap()
        .iter()
        .map(|document| document["o:local_name"].as_str().unwrap())
        .collect();
    assert_eq!(local_names, ["Thing", "Agent"]);
    assert_eq!(
        classes[0]["o:id"].as_u64().unwrap(),
        class_id,
        "the kept class changed id"
    );

    let properties = send(
        app.clone(),
        get(&format!("/api/properties?vocabulary_id={id}"), false),
    )
    .await
    .json();
    assert!(properties.as_array().unwrap().is_empty());
    // The removed property row is gone, not just unlisted.
    let read = send(app, get(&format!("/api/properties/{property_id}"), false)).await;
    assert_eq!(read.status, StatusCode::NOT_FOUND);
}

/// `VocabularyAdapter::hydrate` reads `o:class` on a full `PUT` even when the
/// body omits it, so every class goes; `shouldHydrate` is false on a `PATCH`
/// that omits it, so the list is left alone.
#[tokio::test]
async fn a_full_update_without_the_class_list_removes_every_class() {
    let (app, _seed) = seeded_router().await;
    let body = r#"{"o:namespace_uri":"http://example.org/ns#","o:prefix":"ex3",
        "o:label":"Example three","o:class":[{"o:local_name":"Thing","o:label":"Thing"}]}"#;
    let created = send(
        app.clone(),
        json_request("POST", &keyed("/api/vocabularies"), body),
    )
    .await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.body);
    let id = created.json()["o:id"].as_u64().unwrap();

    let put = r#"{"o:namespace_uri":"http://example.org/ns#","o:prefix":"ex3",
        "o:label":"Example three"}"#;
    let updated = send(
        app.clone(),
        json_request("PUT", &keyed(&format!("/api/vocabularies/{id}")), put),
    )
    .await;
    assert_eq!(updated.status, StatusCode::OK, "{}", updated.body);
    let classes = send(
        app.clone(),
        get(&format!("/api/resource_classes?vocabulary_id={id}"), false),
    )
    .await
    .json();
    assert!(
        classes.as_array().unwrap().is_empty(),
        "a full PUT without o:class kept a class"
    );

    // The same vocabulary, patched without the member, keeps its class.
    let body = r#"{"o:namespace_uri":"http://example.org/other#","o:prefix":"ex4",
        "o:label":"Example four","o:class":[{"o:local_name":"Thing","o:label":"Thing"}]}"#;
    let created = send(
        app.clone(),
        json_request("POST", &keyed("/api/vocabularies"), body),
    )
    .await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.body);
    let id = created.json()["o:id"].as_u64().unwrap();
    let patched = send(
        app.clone(),
        json_request(
            "PATCH",
            &keyed(&format!("/api/vocabularies/{id}")),
            r#"{"o:label":"Renamed four"}"#,
        ),
    )
    .await;
    assert_eq!(patched.status, StatusCode::OK, "{}", patched.body);
    assert_eq!(patched.json()["o:label"], "Renamed four");
    let classes = send(
        app,
        get(&format!("/api/resource_classes?vocabulary_id={id}"), false),
    )
    .await
    .json();
    assert_eq!(
        classes.as_array().unwrap().len(),
        1,
        "a PATCH removed a class"
    );
}

/// `VocabularyAdapter::validateRequest` files both list members under
/// `o:item_set` (`VocabularyAdapter.php:65,71`), and `validateEntity` refuses a
/// missing namespace URI, prefix and label in one answer.
#[tokio::test]
async fn a_vocabulary_create_reports_every_field_error() {
    let (app, _seed) = seeded_router().await;
    let body = r#"{"o:class":"not a list"}"#;
    let answer = send(
        app.clone(),
        json_request("POST", &keyed("/api/vocabularies"), body),
    )
    .await;
    assert_eq!(answer.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        answer.body,
        r#"{"errors":{"o:item_set":["Classes must be an array"]}}"#
    );

    let answer = send(app, json_request("POST", &keyed("/api/vocabularies"), "{}")).await;
    assert_eq!(answer.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        answer.json()["errors"]["o:label"][0],
        "The label cannot be empty."
    );
    assert_eq!(
        answer.json()["errors"]["o:namespace_uri"][0],
        "The namespace URI cannot be empty."
    );
    assert_eq!(
        answer.json()["errors"]["o:prefix"][0],
        "The prefix cannot be empty."
    );
}

/// `VocabularyAdapter::validateEntity`: a prefix is unique, and `o` or an `o-`
/// prefix is reserved.
#[tokio::test]
async fn a_vocabulary_refuses_a_taken_or_reserved_prefix() {
    let (app, _seed) = seeded_router().await;
    let taken = r#"{"o:namespace_uri":"http://example.org/a#","o:prefix":"dcterms",
        "o:label":"Taken"}"#;
    let answer = send(
        app.clone(),
        json_request("POST", &keyed("/api/vocabularies"), taken),
    )
    .await;
    assert_eq!(answer.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        answer.json()["errors"]["o:prefix"][0],
        "The prefix \"dcterms\" is already taken."
    );

    let reserved = r#"{"o:namespace_uri":"http://example.org/b#","o:prefix":"o-x",
        "o:label":"Reserved"}"#;
    let answer = send(
        app,
        json_request("POST", &keyed("/api/vocabularies"), reserved),
    )
    .await;
    assert_eq!(answer.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        answer.json()["errors"]["o:prefix"][0],
        "The prefix \"o-x\" is reserved."
    );
}

/// A vocabulary delete answers the representation and takes its classes and
/// properties with it (`Entity/Vocabulary.php:54,65`).
#[tokio::test]
async fn deleting_a_vocabulary_removes_its_members() {
    let (pool, _seed) = seeded_pool().await;
    let app = common::router(pool.clone());
    let body = r#"{"o:namespace_uri":"http://example.org/ns#","o:prefix":"gone",
        "o:label":"Gone","o:class":[{"o:local_name":"Thing","o:label":"Thing"}],
        "o:property":[{"o:local_name":"name","o:label":"Name"}]}"#;
    let created = send(
        app.clone(),
        json_request("POST", &keyed("/api/vocabularies"), body),
    )
    .await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.body);
    let id = created.json()["o:id"].as_u64().unwrap();

    let deleted = send(
        app.clone(),
        common::request("DELETE", &keyed(&format!("/api/vocabularies/{id}"))),
    )
    .await;
    assert_eq!(deleted.status, StatusCode::OK, "{}", deleted.body);
    assert_eq!(deleted.body, created.body, "delete and create differ");

    let read = send(app, get(&format!("/api/vocabularies/{id}"), false)).await;
    assert_eq!(read.status, StatusCode::NOT_FOUND);
    let classes: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM resource_class WHERE vocabulary_id = ?")
            .bind(id as i64)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(classes, 0, "a class survived");
    let properties: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM property WHERE vocabulary_id = ?")
            .bind(id as i64)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(properties, 0, "a property survived");
}

/// `ResourceClassAdapter::hydrate` and `::validateEntity` for a standalone
/// create, update and delete. `o:vocabulary` is read on create only.
#[tokio::test]
async fn create_update_and_delete_a_resource_class() {
    let (app, seed) = seeded_router().await;
    let vocabulary = seed.vocabulary.0;
    let body = format!(
        r#"{{"o:vocabulary":{{"o:id":{vocabulary}}},"o:local_name":"Painting",
            "o:label":"Painting","o:comment":"A painting."}}"#
    );
    let created = send(
        app.clone(),
        json_request("POST", &keyed("/api/resource_classes"), &body),
    )
    .await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.body);
    let document = created.json();
    assert_eq!(document["o:local_name"], "Painting");
    assert_eq!(document["o:vocabulary"]["o:id"], vocabulary);
    assert_eq!(document["o:term"], "dcterms:Painting");
    let id = document["o:id"].as_u64().unwrap();

    let read = send(
        app.clone(),
        get(&format!("/api/resource_classes/{id}"), false),
    )
    .await;
    assert_eq!(read.body, created.body, "create and read differ");

    let update = format!(
        r#"{{"o:vocabulary":{{"o:id":{vocabulary}}},"o:local_name":"Painting",
            "o:label":"A painting","o:comment":"An oil painting."}}"#
    );
    let updated = send(
        app.clone(),
        json_request(
            "PUT",
            &keyed(&format!("/api/resource_classes/{id}")),
            &update,
        ),
    )
    .await;
    assert_eq!(updated.status, StatusCode::OK, "{}", updated.body);
    assert_eq!(updated.json()["o:label"], "A painting");
    assert_eq!(updated.json()["o:comment"], "An oil painting.");

    let deleted = send(
        app.clone(),
        common::request("DELETE", &keyed(&format!("/api/resource_classes/{id}"))),
    )
    .await;
    assert_eq!(deleted.status, StatusCode::OK, "{}", deleted.body);
    assert_eq!(deleted.body, updated.body, "delete and update differ");
    let read = send(app, get(&format!("/api/resource_classes/{id}"), false)).await;
    assert_eq!(read.status, StatusCode::NOT_FOUND);
}

/// `PropertyAdapter::validateEntity`: `o:vocabulary` is required, and a local
/// name is unique within its vocabulary.
#[tokio::test]
async fn a_property_create_refuses_a_missing_vocabulary_and_a_taken_name() {
    let (app, seed) = seeded_router().await;
    let body = r#"{"o:local_name":"name","o:label":"Name"}"#;
    let answer = send(
        app.clone(),
        json_request("POST", &keyed("/api/properties"), body),
    )
    .await;
    assert_eq!(answer.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        answer.body,
        r#"{"errors":{"o:vocabulary":["A vocabulary must be set."]}}"#
    );

    let body = format!(
        r#"{{"o:vocabulary":{{"o:id":{}}},"o:local_name":"title","o:label":"Title"}}"#,
        seed.vocabulary.0
    );
    let answer = send(app, json_request("POST", &keyed("/api/properties"), &body)).await;
    assert_eq!(answer.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        answer.body,
        format!(
            r#"{{"errors":{{"o:local_name":["The local name \"title\" is already taken."]}}}}"#
        )
    );
}

/// `ResourceTemplateAdapter::hydrate`: every `o:resource_template_property`
/// entry takes its one-based position from the list, and its data types are
/// trimmed, de-duplicated and emptied of blanks.
#[tokio::test]
async fn create_update_and_delete_a_resource_template() {
    let (pool, seed) = seeded_pool().await;
    let app = common::router(pool.clone());
    let title = seeded_title(&pool).await;
    let description = property(&pool, seed.vocabulary.0, "description", "Description").await;
    let body = format!(
        r#"{{"o:label":"Written template","o:resource_class":{{"o:id":{}}},
            "o:title_property":{{"o:id":{title}}},
            "o:resource_template_property":[
              {{"o:property":{{"o:id":{title}}},"o:alternate_label":"Heading",
                "o:data_type":[" literal ","literal",""],"o:is_required":true}},
              {{"o:property":{{"o:id":{description}}},"o:default_lang":"en"}}]}}"#,
        seed.class.0
    );
    let created = send(
        app.clone(),
        json_request("POST", &keyed("/api/resource_templates"), &body),
    )
    .await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.body);
    let document = created.json();
    assert_eq!(document["o:label"], "Written template");
    assert_eq!(document["o:resource_class"]["o:id"], seed.class.0);
    assert_eq!(document["o:title_property"]["o:id"], title);
    let properties = document["o:resource_template_property"].as_array().unwrap();
    assert_eq!(properties.len(), 2);
    assert_eq!(properties[0]["o:property"]["o:id"], title);
    assert_eq!(properties[0]["o:alternate_label"], "Heading");
    assert_eq!(properties[0]["o:data_type"], serde_json::json!(["literal"]));
    assert_eq!(properties[0]["o:is_required"], true);
    assert_eq!(properties[1]["o:property"]["o:id"], description);
    assert_eq!(properties[1]["o:default_lang"], "en");
    let id = document["o:id"].as_u64().unwrap();

    let read = send(
        app.clone(),
        get(&format!("/api/resource_templates/{id}"), false),
    )
    .await;
    assert_eq!(read.body, created.body, "create and read differ");

    // One entry left: the description row is removed.
    let update = format!(
        r#"{{"o:label":"Written template",
            "o:resource_template_property":[{{"o:property":{{"o:id":{title}}}}}]}}"#
    );
    let updated = send(
        app.clone(),
        json_request(
            "PUT",
            &keyed(&format!("/api/resource_templates/{id}")),
            &update,
        ),
    )
    .await;
    assert_eq!(updated.status, StatusCode::OK, "{}", updated.body);
    let properties = updated.json();
    let properties = properties["o:resource_template_property"]
        .as_array()
        .unwrap();
    assert_eq!(properties.len(), 1);
    assert_eq!(properties[0]["o:property"]["o:id"], title);

    let deleted = send(
        app.clone(),
        common::request("DELETE", &keyed(&format!("/api/resource_templates/{id}"))),
    )
    .await;
    assert_eq!(deleted.status, StatusCode::OK, "{}", deleted.body);
    assert_eq!(deleted.body, updated.body, "delete and update differ");
    let read = send(app, get(&format!("/api/resource_templates/{id}"), false)).await;
    assert_eq!(read.status, StatusCode::NOT_FOUND);
}

/// `ResourceTemplateAdapter::validateRequest`: one property may appear once.
#[tokio::test]
async fn a_template_refuses_a_duplicate_property_and_a_missing_one() {
    let (pool, _seed) = seeded_pool().await;
    let app = common::router(pool.clone());
    let title = seeded_title(&pool).await;
    let body = format!(
        r#"{{"o:label":"Duplicate","o:resource_template_property":[
            {{"o:property":{{"o:id":{title}}}}},
            {{"o:property":{{"o:id":{title}}}}}]}}"#
    );
    let answer = send(
        app.clone(),
        json_request("POST", &keyed("/api/resource_templates"), &body),
    )
    .await;
    assert_eq!(answer.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        answer.body,
        format!(
            r#"{{"errors":{{"o:property":["Attempting to add duplicate property with ID {title}"]}}}}"#
        )
    );

    // `findEntity` throws for a property id that does not exist.
    let body = r#"{"o:label":"Missing","o:resource_template_property":[
        {"o:property":{"o:id":999999}}]}"#;
    let answer = send(
        app,
        json_request("POST", &keyed("/api/resource_templates"), body),
    )
    .await;
    assert_eq!(answer.status, StatusCode::NOT_FOUND);
    assert_eq!(
        answer.body,
        r#"{"errors":{"error":"Omeka\\Entity\\Property entity with criteria {\"id\":999999} not found"}}"#
    );
}

/// A reviewer carries no `resource_templates` write grant
/// (`AclFactory::addRulesForReviewer` omits the adapter) and no
/// `vocabularies` grant either.
#[tokio::test]
async fn a_reviewer_cannot_write_a_template_or_vocabulary() {
    let (pool, _seed) = seeded_pool().await;
    let (_, identity, credential) = keyed_user(&pool, "reviewer").await;
    let app = common::router(pool);
    let uri =
        format!("/api/resource_templates?key_identity={identity}&key_credential={credential}");
    let answer = send(app, json_request("POST", &uri, r#"{"o:label":"Nope"}"#)).await;
    assert_eq!(answer.status, StatusCode::FORBIDDEN);
    assert_eq!(
        answer.body,
        r#"{"errors":{"error":"Permission denied for the current user to create the Omeka\\Api\\Adapter\\ResourceTemplateAdapter resource."}}"#
    );
}

/// Create one user holding the given role and an active API key for it.
async fn keyed_user(pool: &SqlitePool, role: &str) -> (u64, String, String) {
    let user = omeka_db::repos::UserRepo::new(pool)
        .create(&omeka_db::rows::NewUser {
            email: format!("{role}@example.com"),
            name: role.to_string(),
            password_hash: None,
            role: role.to_string(),
            is_active: true,
            created: None,
        })
        .await
        .unwrap();
    let mut identity: String = role.chars().filter(char::is_ascii_alphanumeric).collect();
    identity.truncate(32);
    while identity.len() < 32 {
        identity.push('x');
    }
    let credential = format!("{role}-credential");
    omeka_db::repos::ApiKeyRepo::new(pool)
        .create(&omeka_db::rows::NewApiKey {
            id: identity.clone(),
            owner_id: user.id,
            label: format!("{role} key"),
            credential_hash: bcrypt::hash(&credential, 4).unwrap(),
            last_accessed: None,
            last_ip: None,
            created: None,
        })
        .await
        .unwrap();
    (user.id.0, identity, credential)
}

/// The seeded `dcterms:title` property.
async fn seeded_title(pool: &SqlitePool) -> u64 {
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

/// One property in a vocabulary, for a template to name.
async fn property(pool: &SqlitePool, vocabulary: u64, local_name: &str, label: &str) -> u64 {
    PropertyRepo::new(pool)
        .create(&NewProperty {
            owner_id: None,
            vocabulary_id: VocabularyId(vocabulary),
            local_name: local_name.to_string(),
            label: label.to_string(),
            comment: None,
        })
        .await
        .unwrap()
        .id
        .0
}
