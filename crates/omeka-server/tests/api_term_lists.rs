//! Term lists through the router: the `custom_vocabs` resource, the
//! `customvocab:<id>` data types it registers, the values written with them
//! and the public site's rendering of those values.
//!
//! The upstream rule each test freezes is named in its doc comment. The source
//! is the Custom Vocab module v2.1.0 (`omeka-s-modules/CustomVocab`).

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::http::StatusCode;
use common::{Answer, append_key, get, json_request, router, seeded_pool, send, site_pool};
use omeka_db::ids::{PropertyId, VocabularyId};
use omeka_db::repos::{PropertyRepo, TemplateRepo};
use omeka_db::rows::{NewProperty, NewTemplateProperty};
use serde_json::{Value, json};
use sqlx::SqlitePool;

async fn post(pool: &SqlitePool, path: &str, body: &Value) -> Answer {
    send(
        router(pool.clone()),
        json_request("POST", &append_key(path), &body.to_string()),
    )
    .await
}

async fn write(pool: &SqlitePool, method: &str, path: &str, body: &Value) -> Answer {
    send(
        router(pool.clone()),
        json_request(method, &append_key(path), &body.to_string()),
    )
    .await
}

async fn read(pool: &SqlitePool, path: &str) -> Answer {
    send(router(pool.clone()), get(path, false)).await
}

/// Create a list and return its id.
async fn list(pool: &SqlitePool, body: Value) -> u64 {
    let answer = post(pool, "/api/custom_vocabs", &body).await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    answer.json()["o:id"].as_u64().unwrap()
}

/// A `dcterms:spatial` property beside the seeded `dcterms:title`.
async fn spatial(pool: &SqlitePool, vocabulary: VocabularyId) -> PropertyId {
    PropertyRepo::new(pool)
        .create(&NewProperty {
            owner_id: None,
            vocabulary_id: vocabulary,
            local_name: "spatial".to_string(),
            label: "Spatial Coverage".to_string(),
            comment: None,
        })
        .await
        .unwrap()
        .id
}

/// An item carrying one `dcterms:spatial` value object; returns the answer.
async fn item_with(pool: &SqlitePool, property: PropertyId, value: Value) -> Answer {
    let mut object = value;
    object["property_id"] = json!(property.0);
    post(
        pool,
        "/api/items",
        &json!({
            "dcterms:title": [{"type": "literal", "property_id": "auto", "@value": "A map"}],
            "dcterms:spatial": [object],
        }),
    )
    .await
}

/// `CustomVocabRepresentation::getJsonLd` after the entity envelope: label,
/// language, terms, URIs, item set, owner, in that order. A create answers the
/// document a read returns.
#[tokio::test]
async fn a_created_list_reads_back_in_the_module_shape() {
    let (pool, _seed) = seeded_pool().await;
    let created = post(
        &pool,
        "/api/custom_vocabs",
        &json!({"o:label": "Places", "o:lang": " en ", "o:terms": "Cork\nDublin\n\nCork"}),
    )
    .await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.body);
    let id = created.json()["o:id"].as_u64().unwrap();
    assert_eq!(
        created.body,
        format!(
            r#"{{"@context":"http://localhost/api-context","@id":"http://localhost/api/custom_vocabs/{id}","@type":"o:CustomVocab","o:id":{id},"o:label":"Places","o:lang":"en","o:terms":["Cork","Dublin"],"o:uris":null,"o:item_set":null,"o:owner":{{"@id":"http://localhost/api/users/1","o:id":1}}}}"#
        )
    );
    assert_eq!(
        read(&pool, &format!("/api/custom_vocabs/{id}")).await.body,
        created.body
    );

    let listed = read(&pool, "/api/custom_vocabs").await;
    assert_eq!(listed.status, StatusCode::OK);
    assert_eq!(common::header(&listed, "omeka-s-total-results"), Some("1"));
    assert_eq!(listed.json()[0]["o:label"], "Places");
}

/// `sanitizeUris` keeps the map order and trims each label; `sortFields` has
/// `label`.
#[tokio::test]
async fn uris_keep_their_order_and_lists_sort_by_label() {
    let (pool, _seed) = seeded_pool().await;
    let links = list(
        &pool,
        json!({"o:label": "Subjects", "o:uris": {
            "http://id.loc.gov/z": " Zinc ", "http://id.loc.gov/a": ""}}),
    )
    .await;
    list(
        &pool,
        json!({"o:label": "Archive boxes", "o:terms": ["Box 1"]}),
    )
    .await;

    let document = read(&pool, &format!("/api/custom_vocabs/{links}")).await;
    assert!(
        document
            .body
            .contains(r#""o:uris":{"http://id.loc.gov/z":"Zinc","http://id.loc.gov/a":""}"#),
        "{}",
        document.body
    );
    let sorted = read(&pool, "/api/custom_vocabs?sort_by=label").await.json();
    let labels: Vec<&str> = sorted
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["o:label"].as_str().unwrap())
        .collect();
    assert_eq!(labels, ["Archive boxes", "Subjects"]);
}

/// `CustomVocabAdapter::validateEntity`: the label must be set and unused, and
/// one of the three sources must hold something.
#[tokio::test]
async fn validation_refuses_an_empty_or_taken_label_and_an_empty_list() {
    let (pool, _seed) = seeded_pool().await;
    list(&pool, json!({"o:label": "Places", "o:terms": ["Cork"]})).await;

    let empty = post(
        &pool,
        "/api/custom_vocabs",
        &json!({"o:label": " ", "o:terms": "\n"}),
    )
    .await;
    assert_eq!(empty.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        empty.body,
        r#"{"errors":{"o:label":["The label cannot be empty."],"o:terms":["The item set, terms, and URIs cannot all be empty."]}}"#
    );

    let taken = post(
        &pool,
        "/api/custom_vocabs",
        &json!({"o:label": "PLACES", "o:terms": ["Kerry"]}),
    )
    .await;
    assert_eq!(taken.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        taken.body,
        r#"{"errors":{"o:label":["The label is already taken."]}}"#
    );
}

/// `shouldHydrate`: a `PATCH` writes the members it carries, a `PUT` writes all
/// of them, so a `PUT` without `o:terms` clears the terms.
#[tokio::test]
async fn patch_keeps_absent_members_and_put_clears_them() {
    let (pool, _seed) = seeded_pool().await;
    let id = list(
        &pool,
        json!({"o:label": "Places", "o:lang": "en", "o:terms": ["Cork"]}),
    )
    .await;
    let path = format!("/api/custom_vocabs/{id}");

    let patched = write(&pool, "PATCH", &path, &json!({"o:label": "Irish places"})).await;
    assert_eq!(patched.status, StatusCode::OK, "{}", patched.body);
    let patched = patched.json();
    assert_eq!(patched["o:label"], "Irish places");
    assert_eq!(patched["o:lang"], "en");
    assert_eq!(patched["o:terms"], json!(["Cork"]));

    let put = write(&pool, "PUT", &path, &json!({"o:label": "Irish places"})).await;
    assert_eq!(put.status, StatusCode::UNPROCESSABLE_ENTITY, "{}", put.body);

    let put = write(
        &pool,
        "PUT",
        &path,
        &json!({"o:label": "Irish places", "o:terms": ["Galway"]}),
    )
    .await;
    assert_eq!(put.status, StatusCode::OK, "{}", put.body);
    assert_eq!(put.json()["o:lang"], Value::Null);
}

/// `Module::addVocabularyServices` appends `customvocab:<id>` to the registered
/// names, and `DataType\CustomVocab::getLabel` is the list's label.
#[tokio::test]
async fn data_types_list_each_term_list_after_the_core_types() {
    let (pool, _seed) = seeded_pool().await;
    let id = list(&pool, json!({"o:label": "Places", "o:terms": ["Cork"]})).await;

    let types = read(&pool, "/api/data_types").await.json();
    let names: Vec<&str> = types
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["o:id"].as_str().unwrap())
        .collect();
    let name = format!("customvocab:{id}");
    assert_eq!(
        names,
        [
            "literal",
            "uri",
            "resource",
            "resource:item",
            "resource:itemset",
            "resource:media",
            &name
        ]
    );
    let one = read(&pool, &format!("/api/data_types/{name}")).await;
    assert_eq!(one.status, StatusCode::OK, "{}", one.body);
    assert_eq!(one.json()["o:label"], "Places");
    assert_eq!(
        read(&pool, "/api/data_types/customvocab:999").await.status,
        StatusCode::NOT_FOUND
    );
}

/// `DataType\CustomVocab::hydrate` for a list of words: the value keeps the
/// list's type name, and its language is the list's (`@language` from the
/// body is replaced). `isValid` does not check membership, so a word outside
/// the list is stored as well.
#[tokio::test]
async fn a_term_value_keeps_the_list_type_and_takes_the_list_language() {
    let (pool, seed) = seeded_pool().await;
    let property = spatial(&pool, seed.vocabulary).await;
    let id = list(
        &pool,
        json!({"o:label": "Places", "o:lang": "en", "o:terms": ["Cork"]}),
    )
    .await;
    let name = format!("customvocab:{id}");

    let item = item_with(
        &pool,
        property,
        json!({"type": name, "@value": "Cork", "@language": "ga"}),
    )
    .await;
    assert_eq!(item.status, StatusCode::OK, "{}", item.body);
    let value = &item.json()["dcterms:spatial"][0];
    assert_eq!(value["type"], name);
    assert_eq!(value["@value"], "Cork");
    assert_eq!(value["@language"], "en");

    let outside = item_with(&pool, property, json!({"type": name, "@value": "Paris"})).await;
    assert_eq!(outside.status, StatusCode::OK, "{}", outside.body);
    assert_eq!(outside.json()["dcterms:spatial"][0]["@value"], "Paris");
}

/// `DataType\CustomVocab::hydrate` for links: `o:label` is the list's label for
/// the URI whatever the body says, and `getJsonLd` writes no `o:lang`.
#[tokio::test]
async fn a_uri_value_takes_its_label_from_the_list() {
    let (pool, seed) = seeded_pool().await;
    let property = spatial(&pool, seed.vocabulary).await;
    let id = list(
        &pool,
        json!({"o:label": "Places", "o:uris": {"http://www.wikidata.org/entity/Q36405": "Cork"}}),
    )
    .await;
    let name = format!("customvocab:{id}");

    let item = item_with(
        &pool,
        property,
        json!({"type": name, "@id": "http://www.wikidata.org/entity/Q36405",
               "o:label": "Typed by hand", "o:lang": "en"}),
    )
    .await;
    assert_eq!(item.status, StatusCode::OK, "{}", item.body);
    let value = &item.json()["dcterms:spatial"][0];
    assert_eq!(value["type"], name);
    assert_eq!(value["@id"], "http://www.wikidata.org/entity/Q36405");
    assert_eq!(value["o:label"], "Cork");
    assert!(value.get("o:lang").is_none(), "{value}");
}

/// `DataType\CustomVocab::hydrate` hands a `value_resource_id` to
/// `resource:item`, which refuses anything but an item.
#[tokio::test]
async fn an_item_value_must_name_an_item() {
    let (pool, seed) = seeded_pool().await;
    let property = spatial(&pool, seed.vocabulary).await;
    let id = list(
        &pool,
        json!({"o:label": "Places", "o:item_set": {"o:id": seed.item_set.0}}),
    )
    .await;
    let name = format!("customvocab:{id}");

    let item = item_with(
        &pool,
        property,
        json!({"type": name, "value_resource_id": seed.item.0}),
    )
    .await;
    assert_eq!(item.status, StatusCode::OK, "{}", item.body);
    let value = &item.json()["dcterms:spatial"][0];
    assert_eq!(value["type"], name);
    assert_eq!(value["value_resource_id"], seed.item.0);
    assert_eq!(value["value_resource_name"], "items");

    let set = item_with(
        &pool,
        property,
        json!({"type": name, "value_resource_id": seed.item_set.0}),
    )
    .await;
    assert_eq!(set.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        set.body,
        format!(
            r#"{{"errors":{{"value":["Invalid value resource Omeka\\Entity\\ItemSet for type {name}"]}}}}"#
        )
    );
}

/// `ValueHydrator` stores `$dataType->getName()` in `value.type`, so a core
/// resource type round-trips as sent, and a term list name as its own.
#[tokio::test]
async fn the_stored_type_is_the_name_the_value_was_sent_with() {
    let (pool, seed) = seeded_pool().await;
    let property = spatial(&pool, seed.vocabulary).await;
    let id = list(&pool, json!({"o:label": "Places", "o:terms": ["Cork"]})).await;
    let name = format!("customvocab:{id}");
    let answer = post(
        &pool,
        "/api/items",
        &json!({
            "dcterms:spatial": [
                {"type": "resource:item", "property_id": property.0, "value_resource_id": seed.item.0},
                {"type": "resource", "property_id": property.0, "value_resource_id": seed.item.0},
                {"type": name, "property_id": property.0, "@value": "Cork"},
            ],
        }),
    )
    .await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    let item = answer.json()["o:id"].as_u64().unwrap();

    let stored: Vec<String> =
        sqlx::query_scalar("SELECT type FROM value WHERE resource_id = ? ORDER BY id")
            .bind(item as i64)
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(stored, ["resource:item", "resource", name.as_str()]);
    let read = read(&pool, &format!("/api/items/{item}")).await.json();
    let types: Vec<&str> = read["dcterms:spatial"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value["type"].as_str().unwrap())
        .collect();
    assert_eq!(types, ["resource:item", "resource", name.as_str()]);
}

/// `ValueHydrator` skips a value whose data type the manager cannot create, and
/// `CustomVocabFactory::canCreate` refuses an id with no list.
#[tokio::test]
async fn a_value_naming_a_missing_list_is_ignored() {
    let (pool, seed) = seeded_pool().await;
    let property = spatial(&pool, seed.vocabulary).await;
    let item = item_with(
        &pool,
        property,
        json!({"type": "customvocab:999", "@value": "Cork"}),
    )
    .await;
    assert_eq!(item.status, StatusCode::OK, "{}", item.body);
    assert!(
        item.json().get("dcterms:spatial").is_none(),
        "{}",
        item.body
    );
}

/// `Module::setVocabTypeToDefaultState`: deleting a list sets its values back
/// to `literal`. The port also takes the name out of the template rows, where
/// upstream's `data_type = ?` no longer matches the JSON list it stores.
#[tokio::test]
async fn deleting_a_list_turns_its_values_into_text_and_frees_the_template() {
    let (pool, seed) = seeded_pool().await;
    let property = spatial(&pool, seed.vocabulary).await;
    let id = list(&pool, json!({"o:label": "Places", "o:terms": ["Cork"]})).await;
    let name = format!("customvocab:{id}");
    let item = item_with(&pool, property, json!({"type": name, "@value": "Cork"})).await;
    let item_id = item.json()["o:id"].as_u64().unwrap();
    TemplateRepo::new(&pool)
        .add_property(&NewTemplateProperty {
            resource_template_id: seed.template,
            property_id: property,
            alternate_label: None,
            alternate_comment: None,
            position: Some(9),
            data_type: Some(vec![name.clone(), "literal".to_string()]),
            is_required: false,
            is_private: false,
            default_lang: None,
        })
        .await
        .unwrap();

    let deleted = send(
        router(pool.clone()),
        common::request("DELETE", &append_key(&format!("/api/custom_vocabs/{id}"))),
    )
    .await;
    assert_eq!(deleted.status, StatusCode::OK, "{}", deleted.body);
    let deleted = deleted.json();
    assert_eq!(deleted["o:id"], Value::Null);
    assert_eq!(deleted["@id"], "http://localhost/api/custom_vocabs");
    assert_eq!(deleted["o:label"], "Places");

    let value = &read(&pool, &format!("/api/items/{item_id}")).await.json()["dcterms:spatial"][0];
    assert_eq!(value["type"], "literal");
    assert_eq!(value["@value"], "Cork");
    let template = read(
        &pool,
        &format!("/api/resource_templates/{}", seed.template.0),
    )
    .await
    .json();
    let row = template["o:resource_template_property"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["o:property"]["o:id"] == property.0)
        .unwrap()
        .clone();
    assert_eq!(row["o:data_type"], json!(["literal"]));
}

/// `Module::onBootstrap`: everyone may search and read, an editor may create
/// and change a list they own, and only the admin roles may change another's.
#[tokio::test]
async fn editors_write_their_own_lists_and_everyone_reads() {
    let (pool, _seed) = seeded_pool().await;
    let admin_list = list(&pool, json!({"o:label": "Places", "o:terms": ["Cork"]})).await;
    let (_, editor, editor_secret) = keyed_user(&pool, "editor").await;
    let (_, author, author_secret) = keyed_user(&pool, "author").await;
    let as_user = |identity: &str, secret: &str, path: &str| {
        format!("{path}?key_identity={identity}&key_credential={secret}")
    };

    let refused = send(
        router(pool.clone()),
        json_request(
            "POST",
            &as_user(&author, &author_secret, "/api/custom_vocabs"),
            r#"{"o:label":"Mine","o:terms":["a"]}"#,
        ),
    )
    .await;
    assert_eq!(refused.status, StatusCode::FORBIDDEN, "{}", refused.body);

    let own = send(
        router(pool.clone()),
        json_request(
            "POST",
            &as_user(&editor, &editor_secret, "/api/custom_vocabs"),
            r#"{"o:label":"Counties","o:terms":["Kerry"]}"#,
        ),
    )
    .await;
    assert_eq!(own.status, StatusCode::OK, "{}", own.body);
    let own_id = own.json()["o:id"].as_u64().unwrap();
    let renamed = send(
        router(pool.clone()),
        json_request(
            "PATCH",
            &as_user(
                &editor,
                &editor_secret,
                &format!("/api/custom_vocabs/{own_id}"),
            ),
            r#"{"o:label":"Irish counties"}"#,
        ),
    )
    .await;
    assert_eq!(renamed.status, StatusCode::OK, "{}", renamed.body);

    let other = send(
        router(pool.clone()),
        json_request(
            "PATCH",
            &as_user(
                &editor,
                &editor_secret,
                &format!("/api/custom_vocabs/{admin_list}"),
            ),
            r#"{"o:label":"Taken over"}"#,
        ),
    )
    .await;
    assert_eq!(other.status, StatusCode::FORBIDDEN, "{}", other.body);

    assert_eq!(
        read(&pool, "/api/custom_vocabs").await.status,
        StatusCode::OK
    );
    assert_eq!(
        read(&pool, &format!("/api/custom_vocabs/{admin_list}"))
            .await
            .status,
        StatusCode::OK
    );
}

/// `DataType\CustomVocab::render` in `resource-values.phtml`: a word as text, a
/// URI as a new-tab link with its label, and the `<dd>` classed by the type
/// name with `:` turned into `-`.
#[tokio::test]
async fn the_public_item_page_renders_term_list_values() {
    let (pool, seed) = site_pool().await;
    let property = spatial(&pool, seed.vocabulary).await;
    let words = list(&pool, json!({"o:label": "Places", "o:terms": ["Cork"]})).await;
    let links = list(
        &pool,
        json!({"o:label": "Links", "o:uris": {"http://www.wikidata.org/entity/Q36405": "Cork city"}}),
    )
    .await;
    let path = format!("/api/items/{}", seed.item.0);
    let patched = write(
        &pool,
        "PATCH",
        &path,
        &json!({
            "dcterms:title": [{"type": "literal", "property_id": "auto", "@value": "A picture of a cat"}],
            "dcterms:spatial": [
                {"type": format!("customvocab:{words}"), "property_id": property.0, "@value": "Cork"},
                {"type": format!("customvocab:{links}"), "property_id": property.0,
                 "@id": "http://www.wikidata.org/entity/Q36405"},
            ],
        }),
    )
    .await;
    assert_eq!(patched.status, StatusCode::OK, "{}", patched.body);

    let page = read(&pool, &format!("/s/museum/item/{}", seed.item.0)).await;
    assert_eq!(page.status, StatusCode::OK);
    assert!(
        page.body.contains(&format!(
            r#"<dd class="value customvocab-{words}" lang=""><span class="value-content">Cork</span>"#
        )),
        "{}",
        page.body
    );
    assert!(
        page.body.contains(&format!(
            r#"<dd class="value customvocab-{links}" lang=""><span class="value-content"><a href="http://www.wikidata.org/entity/Q36405" target="_blank">Cork city</a></span>"#
        )),
        "{}",
        page.body
    );
}

/// One user holding `role` and an active API key for it.
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
