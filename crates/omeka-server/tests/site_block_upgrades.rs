//! The Romeka settings of four upstream blocks, rendered on the public site:
//! a browse preview's sort, card style and empty state, and a list of pages
//! drawn as cards.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::http::StatusCode;
use common::{Seed, get, router, send, site_pool};
use omeka_db::ids::{ItemId, PropertyId, SiteId};
use omeka_db::repos::{ItemRepo, PropertyRepo, SiteRepo, ValueRepo};
use omeka_db::rows::{NewItem, NewProperty, NewResource, NewSiteBlock, NewSitePage, NewValue};
use serde_json::{Value, json};
use sqlx::SqlitePool;

async fn date_property(pool: &SqlitePool, seed: &Seed) -> PropertyId {
    PropertyRepo::new(pool)
        .create(&NewProperty {
            owner_id: None,
            vocabulary_id: seed.vocabulary,
            local_name: "date".to_string(),
            label: "Date".to_string(),
            comment: None,
        })
        .await
        .unwrap()
        .id
}

/// A public item of the site with a title and, when given, a date value.
async fn dated_item(
    pool: &SqlitePool,
    seed: &Seed,
    title: &str,
    date: Option<(PropertyId, &str)>,
) -> ItemId {
    let id = ItemRepo::new(pool)
        .create(&NewItem {
            resource: NewResource {
                owner_id: Some(seed.user),
                resource_class_id: None,
                resource_template_id: None,
                thumbnail_id: None,
                title: Some(title.to_string()),
                is_public: true,
                created: None,
            },
            primary_media_id: None,
        })
        .await
        .unwrap()
        .id();
    if let Some((property, text)) = date {
        ValueRepo::new(pool)
            .create(&NewValue {
                resource_id: id.as_resource(),
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
    }
    sqlx::query("INSERT INTO item_site (item_id, site_id) VALUES (?, ?)")
        .bind(i64::try_from(id.0).unwrap())
        .bind(i64::try_from(seed.site).unwrap())
        .execute(pool)
        .await
        .unwrap();
    id
}

/// A public page of the seeded site holding one block.
async fn page_with(
    pool: &SqlitePool,
    seed: &Seed,
    slug: &str,
    layout: &str,
    data: Value,
    layout_data: Option<Value>,
) {
    let repo = SiteRepo::new(pool);
    let page = repo
        .add_page(&NewSitePage {
            site_id: SiteId::from(seed.site),
            slug: slug.to_string(),
            title: slug.to_string(),
            is_public: true,
            ..NewSitePage::default()
        })
        .await
        .unwrap();
    repo.add_block(&NewSiteBlock {
        page_id: page.id,
        layout: layout.to_string(),
        data,
        layout_data,
        position: 1,
    })
    .await
    .unwrap();
}

/// Where each of `names` first appears in `body`.
fn positions(body: &str, names: &[&str]) -> Vec<usize> {
    names
        .iter()
        .map(|name| {
            body.find(name)
                .unwrap_or_else(|| panic!("{name} missing:\n{body}"))
        })
        .collect()
}

/// A browse preview orders its items by the property its stored query names,
/// as upstream's `sortQuery` does; an item with no value sorts first.
#[tokio::test]
async fn a_browse_preview_sorts_by_a_property() {
    let (pool, seed) = site_pool().await;
    let date = date_property(&pool, &seed).await;
    dated_item(&pool, &seed, "Middle chart", Some((date, "1850"))).await;
    dated_item(&pool, &seed, "Oldest chart", Some((date, "1700"))).await;
    dated_item(&pool, &seed, "Newest chart", Some((date, "1990"))).await;
    page_with(
        &pool,
        &seed,
        "by-date",
        "browsePreview",
        json!({"resource_type": "items", "query": "sort_by=dcterms:date&sort_order=asc"}),
        None,
    )
    .await;
    page_with(
        &pool,
        &seed,
        "by-title",
        "browsePreview",
        json!({"resource_type": "items", "query": "sort_by=title&sort_order=desc"}),
        None,
    )
    .await;
    let app = router(pool);

    let page = send(app.clone(), get("/s/museum/page/by-date", false)).await;
    assert_eq!(page.status, StatusCode::OK);
    let order = positions(
        &page.body,
        &[
            "A picture of a cat",
            "Oldest chart",
            "Middle chart",
            "Newest chart",
        ],
    );
    assert!(order.windows(2).all(|pair| pair[0] < pair[1]), "{order:?}");

    let page = send(app, get("/s/museum/page/by-title", false)).await;
    let order = positions(
        &page.body,
        &[
            "Oldest chart",
            "Newest chart",
            "Middle chart",
            "A picture of a cat",
        ],
    );
    assert!(order.windows(2).all(|pair| pair[0] < pair[1]), "{order:?}");
}

/// A browse preview searches with every filter the items endpoint reads, as
/// upstream's `$this->api()->search('items', $query)` does: the class, the
/// owner and the `id` list narrow it, not only the advanced-search rows. The
/// editor's count asks the endpoint with the same stored query, so the two
/// agree.
#[tokio::test]
async fn a_browse_preview_applies_the_class_owner_and_id_filters() {
    let (pool, seed) = site_pool().await;
    let chart = dated_item(&pool, &seed, "Unclassed chart", None).await;
    let class = seed.class.0;
    let user = seed.user.0;
    for (slug, query) in [
        ("by-class", format!("resource_class_id={class}")),
        (
            "by-owner",
            format!("owner_id={user}&resource_class_id={class}"),
        ),
        ("by-id", format!("id[]={}", chart.0)),
    ] {
        page_with(
            &pool,
            &seed,
            slug,
            "browsePreview",
            json!({"resource_type": "items", "query": query}),
            None,
        )
        .await;
    }
    let app = router(pool);

    for slug in ["by-class", "by-owner"] {
        let page = send(app.clone(), get(&format!("/s/museum/page/{slug}"), false)).await;
        assert_eq!(page.status, StatusCode::OK);
        assert!(
            page.body.contains("A picture of a cat"),
            "{slug}: {}",
            page.body
        );
        assert!(
            !page.body.contains("Unclassed chart"),
            "{slug}: {}",
            page.body
        );
    }
    let page = send(app, get("/s/museum/page/by-id", false)).await;
    assert!(page.body.contains("Unclassed chart"), "{}", page.body);
    assert!(!page.body.contains("A picture of a cat"), "{}", page.body);
}

/// A preview whose search matches nothing, on a site that holds items, says
/// nothing matches rather than that the site is empty.
#[tokio::test]
async fn an_empty_preview_tells_no_match_from_an_empty_site() {
    let (pool, seed) = site_pool().await;
    page_with(
        &pool,
        &seed,
        "none",
        "browsePreview",
        json!({"resource_type": "items", "query": "fulltext_search=zzzz-no-such-word"}),
        None,
    )
    .await;
    let page = send(router(pool), get("/s/museum/page/none", false)).await;
    assert!(
        page.body
            .contains("<p class=\"preview-empty\">Nothing here matches yet.</p>"),
        "{}",
        page.body
    );
}

/// A carousel draws its region and loads the script that steps it; a page
/// with no carousel does not load it.
#[tokio::test]
async fn a_carousel_loads_its_script() {
    let (pool, seed) = site_pool().await;
    page_with(
        &pool,
        &seed,
        "carousel",
        "browsePreview",
        json!({"resource_type": "items"}),
        Some(json!({"romeka": {"block": "browsePreview", "v": 2,
            "settings": {"cards": "carousel", "columns": "3"}}})),
    )
    .await;
    let app = router(pool);
    let page = send(app.clone(), get("/s/museum/page/carousel", false)).await;
    assert!(
        page.body.contains("<div class=\"preview-carousel\""),
        "{}",
        page.body
    );
    assert!(
        page.body.contains("/assets/js/carousel.js?v="),
        "{}",
        page.body
    );
    let plain = send(app.clone(), get("/s/museum/page/preview", false)).await;
    assert!(!plain.body.contains("carousel.js"), "{}", plain.body);
    let script = send(app, get("/assets/js/carousel.js", false)).await;
    assert_eq!(script.status, StatusCode::OK);
}

/// A list of pages in card mode shows each visible page with the first
/// paragraph of its first HTML block; a private page has no card for a
/// visitor.
#[tokio::test]
async fn a_list_of_pages_draws_cards_with_summaries() {
    let (pool, seed) = site_pool().await;
    let pages = SiteRepo::new(&pool)
        .list_pages(SiteId::from(seed.site))
        .await
        .unwrap();
    let id = |slug: &str| pages.iter().find(|page| page.slug == slug).unwrap().id.0;
    let pagelist = json!([
        {"text": "Welcome", "data": {"id": id("welcome")}},
        {"text": "Notes", "data": {"id": id("notes")}}
    ]);
    page_with(
        &pool,
        &seed,
        "contents",
        "listOfPages",
        json!({"pagelist": pagelist.to_string()}),
        Some(json!({"romeka": {"block": "listOfPages", "v": 2, "settings": {"display": "cards"}}})),
    )
    .await;
    let page = send(router(pool), get("/s/museum/page/contents", false)).await;
    assert!(
        page.body.contains(
            "<li class=\"page-card\"><a href=\"/s/museum/page/welcome\" class=\"page-card-link\">\
             <span class=\"page-card-thumb page-card-thumb-empty\"></span>\
             <span class=\"page-card-title\">Welcome</span>\
             <span class=\"page-card-summary\">Body copy</span></a></li>"
        ),
        "{}",
        page.body
    );
    assert!(!page.body.contains("page/notes"), "{}", page.body);
}
