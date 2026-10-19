//! The archive the tag tests share: the seeded install plus `dcterms:subject`, `dcterms:spatial`
//! and an author with a key, and the request helpers that read results back.

#![allow(clippy::unwrap_used, clippy::expect_used, dead_code)]

use crate::common::{Answer, append_key, get, json_request, router, seeded_pool, send};
use axum::http::StatusCode;
use omeka_db::ids::{PropertyId, TemplateId, UserId, VocabularyId};
use omeka_db::repos::{ApiKeyRepo, PropertyRepo, TemplateRepo, UserRepo};
use omeka_db::rows::{NewApiKey, NewProperty, NewTemplate, NewTemplateProperty, NewUser};
use serde_json::{Value, json};
use sqlx::SqlitePool;

pub const AUTHOR_IDENTITY: &str = "cccccccccccccccccccccccccccccc00";
pub const AUTHOR_CREDENTIAL: &str = "cccccccccccccccccccccccccccccc01";

/// Who sends a request.
#[derive(Clone, Copy)]
pub enum Caller {
    Admin,
    Author,
}

pub fn keyed(path: &str, caller: Caller) -> String {
    match caller {
        Caller::Admin => append_key(path),
        Caller::Author => {
            let separator = if path.contains('?') { '&' } else { '?' };
            format!(
                "{path}{separator}key_identity={AUTHOR_IDENTITY}&key_credential={AUTHOR_CREDENTIAL}"
            )
        }
    }
}

pub async fn post_as(pool: &SqlitePool, caller: Caller, path: &str, body: &Value) -> Answer {
    send(
        router(pool.clone()),
        json_request("POST", &keyed(path, caller), &body.to_string()),
    )
    .await
}

pub async fn post(pool: &SqlitePool, path: &str, body: &Value) -> Answer {
    post_as(pool, Caller::Admin, path, body).await
}

pub async fn read(pool: &SqlitePool, path: &str) -> Value {
    let answer = send(router(pool.clone()), get(path, true)).await;
    assert_eq!(answer.status, StatusCode::OK, "{path}: {}", answer.body);
    answer.json()
}

pub async fn property(pool: &SqlitePool, vocabulary: VocabularyId, name: &str) -> PropertyId {
    PropertyRepo::new(pool)
        .create(&NewProperty {
            owner_id: None,
            vocabulary_id: vocabulary,
            local_name: name.to_string(),
            label: name.to_string(),
            comment: None,
        })
        .await
        .unwrap()
        .id
}

/// The seeded install plus `dcterms:subject`, `dcterms:spatial` and an author with a key.
pub struct Archive {
    pub pool: SqlitePool,
    pub subject: PropertyId,
    pub spatial: PropertyId,
    pub author: UserId,
}

pub async fn archive() -> Archive {
    let (pool, seed) = seeded_pool().await;
    let subject = property(&pool, seed.vocabulary, "subject").await;
    let spatial = property(&pool, seed.vocabulary, "spatial").await;
    let author = UserRepo::new(&pool)
        .create(&NewUser {
            email: "author@example.com".to_string(),
            name: "Author".to_string(),
            password_hash: None,
            role: "author".to_string(),
            is_active: true,
            created: None,
        })
        .await
        .unwrap()
        .id;
    ApiKeyRepo::new(&pool)
        .create(&NewApiKey {
            id: AUTHOR_IDENTITY.to_string(),
            owner_id: author,
            label: "Author key".to_string(),
            credential_hash: bcrypt::hash(AUTHOR_CREDENTIAL, 4).unwrap(),
            last_ip: None,
            last_accessed: None,
            created: None,
        })
        .await
        .unwrap();
    Archive {
        pool,
        subject,
        spatial,
        author,
    }
}

pub fn literal(text: &str) -> Value {
    json!({"type": "literal", "property_id": "auto", "@value": text})
}

/// Create an item as `caller` with a title, the given subjects and optional extra members.
pub async fn item_as(
    pool: &SqlitePool,
    caller: Caller,
    title: &str,
    subjects: Vec<Value>,
    extra: Value,
) -> u64 {
    let mut body = json!({
        "dcterms:title": [literal(title)],
        "dcterms:subject": subjects,
    });
    if let Value::Object(members) = extra {
        for (key, value) in members {
            body[key] = value;
        }
    }
    let answer = post_as(pool, caller, "/api/items", &body).await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    answer.json()["o:id"].as_u64().unwrap()
}

pub async fn item(pool: &SqlitePool, title: &str, subjects: Vec<Value>) -> u64 {
    item_as(pool, Caller::Admin, title, subjects, json!({})).await
}

/// The `(type, @value or @id)` pairs of one property on an item.
pub async fn values_of(pool: &SqlitePool, item: u64, term: &str) -> Vec<(String, String)> {
    let document = read(pool, &format!("/api/items/{item}")).await;
    document[term]
        .as_array()
        .map(|values| {
            values
                .iter()
                .map(|value| {
                    let text = value
                        .get("@value")
                        .or_else(|| value.get("@id"))
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    (
                        value["type"].as_str().unwrap().to_string(),
                        text.to_string(),
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn pairs(raw: &[(&str, &str)]) -> Vec<(String, String)> {
    raw.iter()
        .map(|(kind, text)| ((*kind).to_string(), (*text).to_string()))
        .collect()
}

pub async fn template(
    pool: &SqlitePool,
    label: &str,
    subject_row: Option<PropertyId>,
) -> TemplateId {
    let repo = TemplateRepo::new(pool);
    let template = repo
        .create(&NewTemplate {
            owner_id: None,
            resource_class_id: None,
            title_property_id: None,
            description_property_id: None,
            label: label.to_string(),
        })
        .await
        .unwrap();
    if let Some(property) = subject_row {
        repo.add_property(&NewTemplateProperty {
            resource_template_id: template.id,
            property_id: property,
            alternate_label: Some("Topic".to_string()),
            alternate_comment: None,
            position: Some(1),
            data_type: None,
            is_required: false,
            is_private: false,
            default_lang: None,
        })
        .await
        .unwrap();
    }
    template.id
}

/// The data types each template row offers, by property id.
pub async fn rows_of(pool: &SqlitePool, template: TemplateId) -> Vec<(u64, Option<Vec<String>>)> {
    TemplateRepo::new(pool)
        .list_properties(template)
        .await
        .unwrap()
        .into_iter()
        .map(|row| (row.property_id.0, row.data_type))
        .collect()
}
