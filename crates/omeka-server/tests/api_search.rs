//! Search through the router: the `fulltext_search` parameter, the advanced
//! query parameters, and the index a write maintains.
//!
//! Upstream sources: `Module::searchFulltext` (the fulltext match, the default
//! relevance order and the visibility constraint), `Module::saveFulltext` and
//! `::deleteFulltext` (the write hooks),
//! `AbstractResourceEntityAdapter::buildPropertyQuery` (the property rows and
//! the `search` parameter) and `ItemAdapter::buildQuery` (`item_set_id`,
//! `not_item_set_id`, `site_id`, `in_sites`, `has_media`).
//!
//! Every assertion is about membership in the answer: which resources the
//! query returned, in which order, never an internal.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::{Answer, append_key, get, json_request, router, seeded_pool, send};
use omeka_db::ids::{ItemId, PropertyId, ResourceId, SiteId};
use omeka_db::repos::{
    FulltextRepo, ItemRepo, ItemSetRepo, MediaRepo, Paging, PropertyFilter, PropertyRepo, SiteRepo,
    ValueRepo,
};
use omeka_db::rows::{
    FulltextSearchRow, NewItem, NewItemSet, NewMedia, NewProperty, NewResource, NewSiteBlock,
    NewSitePage, NewValue,
};
use sqlx::SqlitePool;

/// The `dcterms:title` property of the seeded database.
async fn title_property(pool: &SqlitePool) -> PropertyId {
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
}

/// A `dcterms:description` property on the seeded vocabulary.
async fn description_property(pool: &SqlitePool, vocabulary: u64) -> PropertyId {
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
}

/// An item with one literal value on `property`.
async fn item_with_value(
    pool: &SqlitePool,
    property: PropertyId,
    title: &str,
    text: &str,
) -> ItemId {
    let item = ItemRepo::new(pool)
        .create(&NewItem {
            resource: NewResource {
                title: Some(title.to_string()),
                is_public: true,
                ..NewResource::default()
            },
            primary_media_id: None,
        })
        .await
        .unwrap();
    ValueRepo::new(pool)
        .create(&NewValue {
            resource_id: item.resource.id,
            property_id: property,
            value_resource_id: None,
            value_annotation_id: None,
            r#type: "literal".to_string(),
            lang: None,
            value: Some(text.to_string()),
            uri: None,
            is_public: true,
        })
        .await
        .unwrap();
    item.id()
}

/// Index one resource by hand, the way `IndexFulltextSearch` would.
async fn index(pool: &SqlitePool, id: ResourceId, resource: &str, title: &str, text: &str) {
    FulltextRepo::new(pool)
        .save(&FulltextSearchRow {
            id,
            resource: resource.to_string(),
            owner_id: None,
            is_public: true,
            title: Some(title.to_string()),
            text: Some(text.to_string()),
        })
        .await
        .unwrap();
}

fn ids(answer: &Answer) -> Vec<u64> {
    answer
        .json()
        .as_array()
        .expect("a listing is an array")
        .iter()
        .map(|document| document["o:id"].as_u64().expect("every document has o:id"))
        .collect()
}

#[tokio::test]
async fn items_fulltext_search_returns_only_the_matching_item() {
    let (pool, seed) = seeded_pool().await;
    let title = title_property(&pool).await;
    let helmet = item_with_value(&pool, title, "Bronze helmet", "Bronze helmet").await;
    let cup = item_with_value(&pool, title, "Silver cup", "Silver cup").await;
    index(
        &pool,
        helmet.as_resource(),
        "items",
        "Bronze helmet",
        "Bronze helmet",
    )
    .await;
    index(
        &pool,
        cup.as_resource(),
        "items",
        "Silver cup",
        "Silver cup",
    )
    .await;

    let app = router(pool);
    let answer = send(app.clone(), get("/api/items?fulltext_search=helmet", false)).await;
    assert_eq!(answer.status, reqwest_status_ok());
    assert_eq!(ids(&answer), vec![helmet.0], "only the helmet matches");

    let answer = send(app, get("/api/items?fulltext_search=absent", false)).await;
    assert!(ids(&answer).is_empty(), "no item holds that word");
    let _ = seed;
}

#[tokio::test]
async fn items_fulltext_search_ignores_an_unindexed_item() {
    let (pool, _seed) = seeded_pool().await;
    let title = title_property(&pool).await;
    let indexed = item_with_value(&pool, title, "Bronze helmet", "Bronze helmet").await;
    item_with_value(&pool, title, "Bronze helmet two", "Bronze helmet two").await;
    index(
        &pool,
        indexed.as_resource(),
        "items",
        "Bronze helmet",
        "Bronze helmet",
    )
    .await;

    let answer = send(
        router(pool),
        get("/api/items?fulltext_search=helmet", false),
    )
    .await;
    assert_eq!(
        ids(&answer),
        vec![indexed.0],
        "the index decides membership"
    );
}

#[tokio::test]
async fn items_fulltext_search_orders_by_relevance_until_a_sort_is_given() {
    let (pool, _seed) = seeded_pool().await;
    let title = title_property(&pool).await;
    // The closer match carries the later id, so an id order and a relevance
    // order disagree.
    let weak = item_with_value(&pool, title, "Aaa notes", "one mention").await;
    let strong = item_with_value(&pool, title, "Zzz notes", "bronze bronze bronze bronze").await;
    index(
        &pool,
        weak.as_resource(),
        "items",
        "Aaa notes",
        "one bronze mention",
    )
    .await;
    index(
        &pool,
        strong.as_resource(),
        "items",
        "Zzz notes",
        "bronze bronze bronze bronze",
    )
    .await;

    let app = router(pool);
    let default = send(app.clone(), get("/api/items?fulltext_search=bronze", false)).await;
    assert_eq!(
        ids(&default),
        vec![strong.0, weak.0],
        "relevance puts the closer match first"
    );

    let sorted = send(
        app,
        get("/api/items?fulltext_search=bronze&sort_by=title", false),
    )
    .await;
    assert_eq!(
        ids(&sorted),
        vec![weak.0, strong.0],
        "an explicit sort replaces the relevance order"
    );
}

#[tokio::test]
async fn items_search_matches_a_value_not_only_the_title() {
    let (pool, seed) = seeded_pool().await;
    let description = description_property(&pool, seed.vocabulary.0).await;
    let item = item_with_value(&pool, description, "Untitled", "Oil on canvas").await;

    let answer = send(router(pool), get("/api/items?search=canvas", false)).await;
    assert!(
        ids(&answer).contains(&item.0),
        "the description value matches: {:?}",
        ids(&answer)
    );
}

#[tokio::test]
async fn items_property_filter_narrows_to_one_property() {
    let (pool, seed) = seeded_pool().await;
    let title = title_property(&pool).await;
    let description = description_property(&pool, seed.vocabulary.0).await;
    let item = item_with_value(&pool, description, "Untitled", "Oil on canvas").await;
    let app = router(pool);

    let hit = send(
        app.clone(),
        get(
            &format!(
                "/api/items?property[0][property]={}&property[0][type]=in&property[0][text]=canvas",
                description.0
            ),
            false,
        ),
    )
    .await;
    assert_eq!(ids(&hit), vec![item.0]);

    let miss = send(
        app,
        get(
            &format!(
                "/api/items?property[0][property]={}&property[0][type]=in&property[0][text]=canvas",
                title.0
            ),
            false,
        ),
    )
    .await;
    assert!(ids(&miss).is_empty(), "the title does not hold the word");
}

#[tokio::test]
async fn items_property_filter_with_a_negative_type_excludes_a_value() {
    let (pool, seed) = seeded_pool().await;
    let title = title_property(&pool).await;
    let description = description_property(&pool, seed.vocabulary.0).await;
    let plain = item_with_value(&pool, title, "Silver cup", "Silver cup").await;
    let bronze = item_with_value(&pool, description, "Bronze helmet", "Bronze helmet").await;

    let answer = send(
        router(pool),
        get(
            &format!(
                "/api/items?property[0][property]={}&property[0][type]=nin&property[0][text]=bronze",
                description.0
            ),
            false,
        ),
    )
    .await;
    let returned = ids(&answer);
    assert!(
        returned.contains(&plain.0),
        "the item without a bronze description stays: {returned:?}"
    );
    assert!(
        !returned.contains(&bronze.0),
        "the item with a bronze description is excluded: {returned:?}"
    );
}

#[tokio::test]
async fn items_item_set_filter_selects_the_members() {
    let (pool, _seed) = seeded_pool().await;
    let title = title_property(&pool).await;
    let member = item_with_value(&pool, title, "Member", "Member").await;
    let loose = item_with_value(&pool, title, "Loose", "Loose").await;
    let set = ItemSetRepo::new(&pool)
        .create(&NewItemSet::default())
        .await
        .unwrap();
    ItemRepo::new(&pool)
        .add_to_item_set(member, set.id())
        .await
        .unwrap();
    let app = router(pool);

    let inside = send(
        app.clone(),
        get(&format!("/api/items?item_set_id={}", set.id().0), false),
    )
    .await;
    assert_eq!(ids(&inside), vec![member.0]);

    let outside = send(
        app,
        get(&format!("/api/items?not_item_set_id={}", set.id().0), false),
    )
    .await;
    let outside_ids = ids(&outside);
    assert!(outside_ids.contains(&loose.0), "ids were {outside_ids:?}");
    assert!(!outside_ids.contains(&member.0), "ids were {outside_ids:?}");
}

#[tokio::test]
async fn items_has_media_filter_selects_items_with_media() {
    let (pool, _seed) = seeded_pool().await;
    let title = title_property(&pool).await;
    let with = item_with_value(&pool, title, "With media", "With media").await;
    let without = item_with_value(&pool, title, "Without media", "Without media").await;
    MediaRepo::new(&pool)
        .create(&NewMedia {
            resource: NewResource::default(),
            item_id: with,
            ..NewMedia::default()
        })
        .await
        .unwrap();
    let app = router(pool);

    let answer = send(app.clone(), get("/api/items?has_media=1", false)).await;
    let returned = ids(&answer);
    assert!(returned.contains(&with.0), "ids were {returned:?}");
    assert!(!returned.contains(&without.0), "ids were {returned:?}");

    let answer = send(app, get("/api/items?has_media=0", false)).await;
    let returned = ids(&answer);
    assert!(returned.contains(&without.0), "ids were {returned:?}");
    assert!(!returned.contains(&with.0), "ids were {returned:?}");
}

#[tokio::test]
async fn items_has_media_counts_only_visible_media() {
    let (pool, _seed) = seeded_pool().await;
    let title = title_property(&pool).await;
    let item = item_with_value(&pool, title, "Private media", "Private media").await;
    MediaRepo::new(&pool)
        .create(&NewMedia {
            resource: NewResource {
                is_public: false,
                ..NewResource::default()
            },
            item_id: item,
            ..NewMedia::default()
        })
        .await
        .unwrap();
    let app = router(pool);

    let anonymous = send(app.clone(), get("/api/items?has_media=1", false)).await;
    assert!(
        !ids(&anonymous).contains(&item.0),
        "a private media does not count for a visitor: {:?}",
        ids(&anonymous)
    );

    let admin = send(app, get("/api/items?has_media=1", true)).await;
    assert!(
        ids(&admin).contains(&item.0),
        "view-all counts every media row: {:?}",
        ids(&admin)
    );
}

#[tokio::test]
async fn items_site_filter_selects_attached_items() {
    let (pool, seed) = seeded_pool().await;
    let title = title_property(&pool).await;
    let attached = item_with_value(&pool, title, "Attached", "Attached").await;
    let loose = item_with_value(&pool, title, "Loose", "Loose").await;
    sqlx::query("INSERT INTO item_site (item_id, site_id) VALUES (?, ?)")
        .bind(i64::try_from(attached.0).unwrap())
        .bind(i64::try_from(seed.site).unwrap())
        .execute(&pool)
        .await
        .unwrap();
    let app = router(pool);

    let answer = send(
        app.clone(),
        get(&format!("/api/items?site_id={}", seed.site), false),
    )
    .await;
    assert_eq!(ids(&answer), vec![attached.0]);

    let answer = send(app, get("/api/items?in_sites=0", false)).await;
    let returned = ids(&answer);
    assert!(returned.contains(&loose.0), "ids were {returned:?}");
    assert!(!returned.contains(&attached.0), "ids were {returned:?}");
}

#[tokio::test]
async fn media_site_filter_follows_the_item_attachment() {
    let (pool, seed) = seeded_pool().await;
    let title = title_property(&pool).await;
    let item = item_with_value(&pool, title, "Item", "Item").await;
    let media = MediaRepo::new(&pool)
        .create(&NewMedia {
            resource: NewResource::default(),
            item_id: item,
            ..NewMedia::default()
        })
        .await
        .unwrap();
    sqlx::query("INSERT INTO item_site (item_id, site_id) VALUES (?, ?)")
        .bind(i64::try_from(item.0).unwrap())
        .bind(i64::try_from(seed.site).unwrap())
        .execute(&pool)
        .await
        .unwrap();

    let answer = send(
        router(pool),
        get(&format!("/api/media?site_id={}", seed.site), false),
    )
    .await;
    assert_eq!(ids(&answer), vec![media.id().0]);
}

#[tokio::test]
async fn media_fulltext_search_matches_an_indexed_media() {
    let (pool, _seed) = seeded_pool().await;
    let title = title_property(&pool).await;
    let item = item_with_value(&pool, title, "Item", "Item").await;
    let media = MediaRepo::new(&pool)
        .create(&NewMedia {
            resource: NewResource::default(),
            item_id: item,
            ingester: "html".to_string(),
            renderer: "html".to_string(),
            data: Some(serde_json::json!({"html": "<p>A bronze plate</p>"})),
            ..NewMedia::default()
        })
        .await
        .unwrap();
    index(
        &pool,
        media.resource.id,
        "media",
        "A bronze plate",
        "A bronze plate",
    )
    .await;

    let answer = send(
        router(pool),
        get("/api/media?fulltext_search=bronze", false),
    )
    .await;
    assert_eq!(ids(&answer), vec![media.id().0]);
}

#[tokio::test]
async fn create_then_fulltext_search_finds_the_new_item() {
    let (pool, _seed) = seeded_pool().await;
    let app = router(pool);
    let body =
        r#"{"dcterms:title":[{"property_id":"auto","type":"literal","@value":"A bronze helmet"}]}"#;
    let created = send(
        app.clone(),
        json_request("POST", &append_key("/api/items"), body),
    )
    .await;
    assert_eq!(created.status, reqwest_status_ok());
    let id = created.json()["o:id"].as_u64().unwrap();

    let answer = send(app, get("/api/items?fulltext_search=helmet", false)).await;
    assert_eq!(ids(&answer), vec![id], "the write indexed the item");
}

#[tokio::test]
async fn create_indexes_the_html_media_text_for_its_item() {
    let (pool, _seed) = seeded_pool().await;
    let app = router(pool);
    let item = send(
        app.clone(),
        json_request(
            "POST",
            &append_key("/api/items"),
            r#"{"dcterms:title":[{"property_id":"auto","type":"literal","@value":"An illustrated plate"}]}"#,
        ),
    )
    .await
    .json()["o:id"]
        .as_u64()
        .unwrap();
    let media = send(
        app.clone(),
        json_request(
            "POST",
            &append_key("/api/media"),
            &format!(
                r#"{{"o:ingester":"html","o:item":{{"o:id":{item}}},
                    "data":{{"html":"<p>Vindolanda excavation notes</p>"}}}}"#
            ),
        ),
    )
    .await;
    assert_eq!(media.status, reqwest_status_ok());

    // The item's own index row carries the media text (`ItemAdapter::getFulltextText`).
    let answer = send(app, get("/api/items?fulltext_search=Vindolanda", false)).await;
    assert_eq!(ids(&answer), vec![item], "the item carries its media text");
}

#[tokio::test]
async fn update_reindexes_an_item_and_delete_drops_it() {
    let (pool, _seed) = seeded_pool().await;
    let app = router(pool);
    let id = send(
        app.clone(),
        json_request(
            "POST",
            &append_key("/api/items"),
            r#"{"dcterms:title":[{"property_id":"auto","type":"literal","@value":"First name"}]}"#,
        ),
    )
    .await
    .json()["o:id"]
        .as_u64()
        .unwrap();

    let updated = send(
        app.clone(),
        json_request(
            "PUT",
            &append_key(&format!("/api/items/{id}")),
            r#"{"dcterms:title":[{"property_id":"auto","type":"literal","@value":"Second name"}]}"#,
        ),
    )
    .await;
    assert_eq!(updated.status, reqwest_status_ok());

    let answer = send(app.clone(), get("/api/items?fulltext_search=First", false)).await;
    assert!(ids(&answer).is_empty(), "the old text is gone");
    let answer = send(app.clone(), get("/api/items?fulltext_search=Second", false)).await;
    assert_eq!(ids(&answer), vec![id], "the new text is indexed");

    let deleted = send(
        app.clone(),
        common::request("DELETE", &append_key(&format!("/api/items/{id}"))),
    )
    .await;
    assert_eq!(deleted.status, reqwest_status_ok());
    let answer = send(app, get("/api/items?fulltext_search=Second", false)).await;
    assert!(ids(&answer).is_empty(), "the delete dropped the index row");
}

#[tokio::test]
async fn site_page_fulltext_search_matches_a_block_body() {
    let (pool, seed) = seeded_pool().await;
    let app = router(pool);
    let body = format!(
        r#"{{"o:site":{{"o:id":{}}},
            "o:title":"Notes",
            "o:slug":"notes",
            "o:block":[{{"o:layout":"html","o:data":{{"html":"<p>Vindolanda excavation notes</p>"}}}}]}}"#,
        seed.site
    );
    let created = send(
        app.clone(),
        json_request("POST", &append_key("/api/site_pages"), &body),
    )
    .await;
    assert_eq!(created.status, reqwest_status_ok());
    let id = created.json()["o:id"].as_u64().unwrap();

    let answer = send(
        app.clone(),
        get("/api/site_pages?fulltext_search=Vindolanda", false),
    )
    .await;
    assert_eq!(ids(&answer), vec![id], "the block body is indexed");

    let deleted = send(
        app.clone(),
        common::request("DELETE", &append_key(&format!("/api/site_pages/{id}"))),
    )
    .await;
    assert_eq!(deleted.status, reqwest_status_ok());
    let answer = send(
        app,
        get("/api/site_pages?fulltext_search=Vindolanda", false),
    )
    .await;
    assert!(ids(&answer).is_empty(), "the delete dropped the index row");
}

#[tokio::test]
async fn site_page_index_carries_the_site_owner_and_visibility() {
    let (pool, seed) = seeded_pool().await;
    let page = SiteRepo::new(&pool)
        .add_page(&NewSitePage {
            site_id: SiteId(seed.site),
            slug: "notes".to_string(),
            title: "Notes".to_string(),
            is_public: true,
            ..NewSitePage::default()
        })
        .await
        .unwrap();
    SiteRepo::new(&pool)
        .add_block(&NewSiteBlock {
            page_id: page.id,
            layout: "html".to_string(),
            data: serde_json::json!({"html": "<p>Bronze plate</p>"}),
            layout_data: None,
            position: 0,
        })
        .await
        .unwrap();

    let app = router(pool.clone());
    let body = r#"{"o:title":"Notes","o:is_public":true,
        "o:block":[{"o:layout":"html","o:data":{"html":"<p>Bronze plate</p>"}}]}"#;
    let updated = send(
        app.clone(),
        json_request(
            "PUT",
            &append_key(&format!("/api/site_pages/{}", page.id.0)),
            body,
        ),
    )
    .await;
    assert_eq!(updated.status, reqwest_status_ok());

    let rows: Vec<(i64, Option<i64>, i64)> = sqlx::query_as(
        "SELECT id, owner_id, is_public FROM fulltext_search WHERE resource = 'site_pages'",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(rows.len(), 1, "one site page row");
    assert_eq!(rows[0].0, i64::try_from(page.id.0).unwrap());
    assert_eq!(rows[0].2, 1, "the page and its site are public");
}

#[tokio::test]
async fn fulltext_search_respects_visibility() {
    let (pool, _seed) = seeded_pool().await;
    let title = title_property(&pool).await;
    let private = ItemRepo::new(&pool)
        .create(&NewItem {
            resource: NewResource {
                title: Some("Private bronze".to_string()),
                is_public: false,
                ..NewResource::default()
            },
            primary_media_id: None,
        })
        .await
        .unwrap();
    ValueRepo::new(&pool)
        .create(&NewValue {
            resource_id: private.resource.id,
            property_id: title,
            value_resource_id: None,
            value_annotation_id: None,
            r#type: "literal".to_string(),
            lang: None,
            value: Some("Private bronze".to_string()),
            uri: None,
            is_public: true,
        })
        .await
        .unwrap();
    index(
        &pool,
        private.resource.id,
        "items",
        "Private bronze",
        "Private bronze",
    )
    .await;

    let answer = send(
        router(pool),
        get("/api/items?fulltext_search=bronze", false),
    )
    .await;
    assert!(
        ids(&answer).is_empty(),
        "an anonymous visitor sees no private item"
    );
}

#[tokio::test]
async fn fulltext_search_returns_nothing_for_a_query_without_words() {
    let (pool, _seed) = seeded_pool().await;
    let title = title_property(&pool).await;
    let item = item_with_value(&pool, title, "Bronze helmet", "Bronze helmet").await;
    index(
        &pool,
        item.as_resource(),
        "items",
        "Bronze helmet",
        "Bronze helmet",
    )
    .await;

    let answer = send(
        router(pool),
        get("/api/items?fulltext_search=%22%22", false),
    )
    .await;
    assert!(ids(&answer).is_empty(), "quotes hold no word");
}

/// The status code the router answers a create and a read with.
fn reqwest_status_ok() -> axum::http::StatusCode {
    axum::http::StatusCode::OK
}

/// `PropertyAdapter::buildQuery` and `ResourceClassAdapter::buildQuery` filter on
/// `vocabulary_id` with an `innerJoin` on the vocabulary. The seeded database
/// holds `dcterms:title` in the Dublin Core vocabulary and `dctype:Image` in the
/// Dublin Core Type vocabulary, so each filter keeps one row and drops the other.
#[tokio::test]
async fn the_vocabulary_id_filter_selects_one_vocabularys_members() {
    let (pool, seed) = seeded_pool().await;
    let dcterms = seed.vocabulary.0;
    let dctype = 2;

    let properties = send(
        router(pool.clone()),
        get(&format!("/api/properties?vocabulary_id={dcterms}"), false),
    )
    .await;
    assert_eq!(
        properties.status,
        axum::http::StatusCode::OK,
        "{}",
        properties.body
    );
    let properties_body = properties.json();
    let terms: Vec<&str> = properties_body
        .as_array()
        .unwrap()
        .iter()
        .map(|document| document["o:local_name"].as_str().unwrap())
        .collect();
    assert_eq!(terms, ["title"], "dcterms holds title only");

    let none = send(
        router(pool.clone()),
        get(&format!("/api/properties?vocabulary_id={dctype}"), false),
    )
    .await;
    assert!(
        none.json().as_array().unwrap().is_empty(),
        "dctype holds no property"
    );

    let classes = send(
        router(pool.clone()),
        get(
            &format!("/api/resource_classes?vocabulary_id={dctype}"),
            false,
        ),
    )
    .await;
    assert_eq!(
        classes.status,
        axum::http::StatusCode::OK,
        "{}",
        classes.body
    );
    let classes_body = classes.json();
    let local_names: Vec<&str> = classes_body
        .as_array()
        .unwrap()
        .iter()
        .map(|document| document["o:local_name"].as_str().unwrap())
        .collect();
    assert_eq!(local_names, ["Image"], "dctype holds Image only");

    let none = send(
        router(pool),
        get(
            &format!("/api/resource_classes?vocabulary_id={dcterms}"),
            false,
        ),
    )
    .await;
    assert!(
        none.json().as_array().unwrap().is_empty(),
        "dcterms holds no resource class"
    );
}
