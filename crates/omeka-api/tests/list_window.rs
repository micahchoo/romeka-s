//! The list window read from SQL answers what the in-memory window answers.
//!
//! `search_items` reads the page and the total from one query when the request's
//! filter is one `read::window` renders, and keeps the in-memory window
//! (`params::sorted_page`) otherwise. These tests run both paths over the same
//! seeded database and assert they answer the same page and the same
//! `Omeka-S-Total-Results`, so a divergence in the SQL the fast path copies from
//! `omeka_db::repos` fails here.
//!
//! The fallback is forced with `not_item_set_id` naming a set that holds no
//! item: the clause excludes nothing, and a `not_item_set_id` is one of the
//! shapes the fast path declines.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use omeka_api::auth::Actor;
use omeka_api::jsonld::BaseUrl;
use omeka_api::read::response::TOTAL_RESULTS_HEADER;
use omeka_api::read::{Api, Params};
use omeka_db::ids::{ClassId, ItemId, ItemSetId, PropertyId, TemplateId, UserId};
use omeka_db::repos::{
    ClassRepo, ItemRepo, ItemSetRepo, PropertyRepo, TemplateRepo, UserRepo, ValueRepo,
    VocabularyRepo,
};
use omeka_db::rows::{
    NewClass, NewItem, NewItemSet, NewProperty, NewResource, NewTemplate, NewUser, NewValue,
    NewVocabulary,
};
use omeka_db::{migrate, open_memory};
use serde_json::Value;
use sqlx::SqlitePool;

/// A set id no item belongs to, so `not_item_set_id` excludes nothing.
const ABSENT_SET: u64 = 999_999;

struct Seed {
    admin: UserId,
    author: UserId,
    set_a: ItemSetId,
    set_b: ItemSetId,
    class: ClassId,
    template: TemplateId,
    /// Eight items in set A: six public, two private and owned by the author.
    in_a: Vec<ItemId>,
    /// One public item in set B.
    in_b: ItemId,
}

fn base() -> BaseUrl {
    BaseUrl::parse("http://localhost").unwrap()
}

async fn user(pool: &SqlitePool, email: &str, role: &str) -> UserId {
    UserRepo::new(pool)
        .create(&NewUser {
            email: email.to_string(),
            name: email.to_string(),
            password_hash: None,
            role: role.to_string(),
            is_active: true,
            created: None,
        })
        .await
        .unwrap()
        .id
}

async fn item_set(pool: &SqlitePool, owner: UserId, title: &str) -> ItemSetId {
    ItemSetRepo::new(pool)
        .create(&NewItemSet {
            resource: NewResource {
                owner_id: Some(owner),
                title: Some(title.to_string()),
                ..NewResource::default()
            },
            is_open: false,
        })
        .await
        .unwrap()
        .id()
}

#[allow(clippy::too_many_arguments)]
async fn item(
    pool: &SqlitePool,
    owner: UserId,
    set: ItemSetId,
    class: ClassId,
    template: TemplateId,
    property: PropertyId,
    title: &str,
    is_public: bool,
) -> ItemId {
    let id = ItemRepo::new(pool)
        .create(&NewItem {
            resource: NewResource {
                owner_id: Some(owner),
                resource_class_id: Some(class),
                resource_template_id: Some(template),
                title: Some(title.to_string()),
                is_public,
                ..NewResource::default()
            },
            primary_media_id: None,
        })
        .await
        .unwrap()
        .id();
    ItemRepo::new(pool).add_to_item_set(id, set).await.unwrap();
    ValueRepo::new(pool)
        .create(&NewValue {
            resource_id: id.as_resource(),
            property_id: property,
            value_resource_id: None,
            value_annotation_id: None,
            r#type: "literal".to_string(),
            lang: None,
            value: Some(format!("value for {title}")),
            uri: None,
            is_public: true,
        })
        .await
        .unwrap();
    id
}

async fn seed(pool: &SqlitePool) -> Seed {
    let admin = user(pool, "admin@example.com", "global_admin").await;
    let author = user(pool, "author@example.com", "author").await;
    let vocabulary = VocabularyRepo::new(pool)
        .create(&NewVocabulary {
            owner_id: None,
            namespace_uri: "http://purl.org/dc/terms/".to_string(),
            prefix: "dcterms".to_string(),
            label: "Dublin Core".to_string(),
            comment: None,
        })
        .await
        .unwrap();
    let property = PropertyRepo::new(pool)
        .create(&NewProperty {
            owner_id: None,
            vocabulary_id: vocabulary.id,
            local_name: "title".to_string(),
            label: "Title".to_string(),
            comment: None,
        })
        .await
        .unwrap()
        .id;
    let class: ClassId = ClassRepo::new(pool)
        .create(&NewClass {
            owner_id: None,
            vocabulary_id: vocabulary.id,
            local_name: "Thing".to_string(),
            label: "Thing".to_string(),
            comment: None,
        })
        .await
        .unwrap()
        .id;
    let template = TemplateRepo::new(pool)
        .create(&NewTemplate {
            owner_id: Some(admin),
            resource_class_id: Some(class),
            title_property_id: Some(property),
            description_property_id: None,
            label: "Thing template".to_string(),
        })
        .await
        .unwrap()
        .id;
    let set_a = item_set(pool, admin, "Set A").await;
    let set_b = item_set(pool, admin, "Set B").await;
    let mut in_a = Vec::new();
    for index in 1..=6 {
        in_a.push(
            item(
                pool,
                admin,
                set_a,
                class,
                template,
                property,
                &format!("Public A{index}"),
                true,
            )
            .await,
        );
    }
    for index in 1..=2 {
        in_a.push(
            item(
                pool,
                author,
                set_a,
                class,
                template,
                property,
                &format!("Private A{index}"),
                false,
            )
            .await,
        );
    }
    let in_b = item(
        pool,
        admin,
        set_b,
        class,
        template,
        property,
        "Public B1",
        true,
    )
    .await;
    Seed {
        admin,
        author,
        set_a,
        set_b,
        class,
        template,
        in_a,
        in_b,
    }
}

async fn open_seeded() -> (SqlitePool, Seed) {
    let pool = open_memory().await.unwrap();
    migrate(&pool).await.unwrap();
    let seed = seed(&pool).await;
    (pool, seed)
}

fn documents(body: &str) -> Vec<Value> {
    match serde_json::from_str(body).unwrap() {
        Value::Array(documents) => documents,
        other => panic!("a list body is an array, got {other}"),
    }
}

fn ids(documents: &[Value]) -> Vec<u64> {
    documents
        .iter()
        .map(|document| document["o:id"].as_u64().unwrap())
        .collect()
}

/// One request's documents and its `Omeka-S-Total-Results`.
async fn page(api: &Api<'_>, query: &str) -> (Vec<Value>, u64) {
    let response = api.search("items", &Params::parse(query)).await.unwrap();
    let total = response
        .headers
        .get(TOTAL_RESULTS_HEADER)
        .unwrap()
        .to_str()
        .unwrap()
        .parse()
        .unwrap();
    (documents(&response.body), total)
}

/// The fast path's answer equals the in-memory fallback's for the same filter.
async fn same_page(api: &Api<'_>, fast: &str) {
    let fallback = format!("{fast}&not_item_set_id={ABSENT_SET}");
    let (fast_documents, fast_total) = page(api, fast).await;
    let (slow_documents, slow_total) = page(api, &fallback).await;
    assert_eq!(
        ids(&fast_documents),
        ids(&slow_documents),
        "the page differs for `{fast}` against `{fallback}`"
    );
    assert_eq!(
        fast_total, slow_total,
        "the total differs for `{fast}` against `{fallback}`"
    );
}

#[tokio::test]
async fn the_window_and_the_in_memory_fallback_answer_the_same_page() {
    let (pool, seed) = open_seeded().await;
    let api = Api::load(&pool, base(), Actor::new(seed.admin, "global_admin"))
        .await
        .unwrap();

    for query in [
        format!("item_set_id={}&per_page=2", seed.set_a.0),
        format!("item_set_id={}&per_page=2&page=2", seed.set_a.0),
        format!("item_set_id={}&per_page=2&page=5", seed.set_a.0),
        format!("item_set_id={}&per_page=100", seed.set_a.0),
        format!("item_set_id={}&per_page=3", seed.set_b.0),
        "per_page=4".to_string(),
        "per_page=4&page=3".to_string(),
        format!("is_public=1&per_page=3&item_set_id={}", seed.set_a.0),
        "is_public=0&per_page=3".to_string(),
        format!("owner_id={}&per_page=3", seed.author.0),
        format!("resource_class_id={}&per_page=3", seed.class.0),
        format!("resource_template_id={}&per_page=3", seed.template.0),
        format!("item_set_id={}&per_page=0", seed.set_a.0),
    ] {
        same_page(&api, &query).await;
    }
    pool.close().await;
}

/// The page is the filter's window in `resource.id` order, and the total is the
/// whole filter, not the page.
#[tokio::test]
async fn a_window_reads_the_requested_page_and_counts_the_whole_filter() {
    let (pool, seed) = open_seeded().await;
    let api = Api::load(&pool, base(), Actor::new(seed.admin, "global_admin"))
        .await
        .unwrap();

    let (first, total) = page(&api, &format!("item_set_id={}&per_page=2", seed.set_a.0)).await;
    assert_eq!(total, 8);
    assert_eq!(ids(&first), vec![seed.in_a[0].0, seed.in_a[1].0]);

    let (second, total) = page(
        &api,
        &format!("item_set_id={}&per_page=2&page=2", seed.set_a.0),
    )
    .await;
    assert_eq!(total, 8);
    assert_eq!(ids(&second), vec![seed.in_a[2].0, seed.in_a[3].0]);

    let (past, total) = page(
        &api,
        &format!("item_set_id={}&per_page=2&page=9", seed.set_a.0),
    )
    .await;
    assert_eq!(
        total, 8,
        "an offset past the last row still counts the filter"
    );
    assert!(past.is_empty());

    let (only_b, total) = page(&api, &format!("item_set_id={}&per_page=25", seed.set_b.0)).await;
    assert_eq!(total, 1);
    assert_eq!(ids(&only_b), vec![seed.in_b.0]);
    pool.close().await;
}

/// A signed-in caller who is not the owner sees the public rows only, and the
/// total counts those rows.
#[tokio::test]
async fn a_reader_counts_only_the_rows_it_may_see() {
    let (pool, seed) = open_seeded().await;
    let api = Api::load(&pool, base(), Actor::new(seed.admin, "editor"))
        .await
        .unwrap();
    let (visible, total) = page(&api, &format!("item_set_id={}&per_page=100", seed.set_a.0)).await;
    assert_eq!(total, 8, "an editor holds view-all");

    let api = Api::load(&pool, base(), Actor::anonymous()).await.unwrap();
    let (public, total) = page(&api, &format!("item_set_id={}&per_page=100", seed.set_a.0)).await;
    assert_eq!(total, 6, "anonymous sees the public rows of the set");
    assert_eq!(ids(&public), ids(&visible)[..6].to_vec());
    assert!(
        public
            .iter()
            .all(|document| document["o:is_public"] == true)
    );

    let api = Api::load(&pool, base(), Actor::new(seed.author, "author"))
        .await
        .unwrap();
    let (own, total) = page(&api, &format!("item_set_id={}&per_page=100", seed.set_a.0)).await;
    assert_eq!(total, 8, "the author sees the public rows and their own");
    assert_eq!(ids(&own).len(), 8);
    pool.close().await;
}

/// A sort the window does not render keeps the in-memory page, which orders by
/// the requested field.
#[tokio::test]
async fn a_sort_the_window_does_not_render_keeps_the_in_memory_page() {
    let (pool, seed) = open_seeded().await;
    let api = Api::load(&pool, base(), Actor::new(seed.admin, "global_admin"))
        .await
        .unwrap();

    let (by_title, total) = page(
        &api,
        &format!("item_set_id={}&per_page=3&sort_by=title", seed.set_a.0),
    )
    .await;
    assert_eq!(total, 8);
    let titles: Vec<&str> = by_title
        .iter()
        .map(|document| document["o:title"].as_str().unwrap())
        .collect();
    assert_eq!(titles, vec!["Private A1", "Private A2", "Public A1"]);

    let (by_id, _) = page(
        &api,
        &format!(
            "item_set_id={}&per_page=3&sort_by=id&sort_order=desc",
            seed.set_a.0
        ),
    )
    .await;
    assert_eq!(
        ids(&by_id),
        vec![seed.in_a[7].0, seed.in_a[6].0, seed.in_a[5].0]
    );
    pool.close().await;
}
