//! Site-scoped and cross-site search through the public routes.
//!
//! Upstream sources: `Site\ItemController::browseAction` and
//! `Site\ItemSetController::browseAction` (the site browse, which sets
//! `site_id` on the query and serves the `fulltext_search` results),
//! `Site\IndexController::searchAction` (`/s/:site/search`) and
//! `Site\CrossSiteSearchController` (`/s/:site/cross-site-search`, which sets
//! `in_sites` instead of `site_id`).
//!
//! The assertions are about the rendered document: which resources appear, in
//! which order, and the status code.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::http::StatusCode;
use common::{Answer, get, router, send, site_pool};
use omeka_db::ids::{ItemId, ResourceId, SiteId};
use omeka_db::repos::{FulltextRepo, ItemRepo, SiteRepo, ValueRepo};
use omeka_db::rows::{FulltextSearchRow, NewItem, NewResource, NewSite, NewValue};
use sqlx::SqlitePool;

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

/// A public item with a title value, optionally attached to a site.
async fn item(pool: &SqlitePool, title: &str, site: Option<SiteId>) -> ItemId {
    let row = ItemRepo::new(pool)
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
    let property: i64 = sqlx::query_scalar(
        "SELECT id FROM property WHERE local_name = 'title' ORDER BY id LIMIT 1",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    ValueRepo::new(pool)
        .create(&NewValue {
            resource_id: row.resource.id,
            property_id: omeka_db::ids::PropertyId(u64::try_from(property).unwrap()),
            value_resource_id: None,
            value_annotation_id: None,
            r#type: "literal".to_string(),
            lang: None,
            value: Some(title.to_string()),
            uri: None,
            is_public: true,
        })
        .await
        .unwrap();
    if let Some(site) = site {
        sqlx::query("INSERT INTO item_site (item_id, site_id) VALUES (?, ?)")
            .bind(i64::try_from(row.id().0).unwrap())
            .bind(i64::try_from(site.0).unwrap())
            .execute(pool)
            .await
            .unwrap();
    }
    row.id()
}

fn body_has(answer: &Answer, needle: &str) -> bool {
    answer.body.contains(needle)
}

#[tokio::test]
async fn site_item_browse_filters_by_fulltext_search() {
    let (pool, seed) = site_pool().await;
    index(
        &pool,
        ResourceId(seed.item.0),
        "items",
        "A picture of a cat",
        "A picture of a cat",
    )
    .await;
    let app = router(pool);

    let hit = send(
        app.clone(),
        get("/s/museum/item?fulltext_search=cat", false),
    )
    .await;
    assert_eq!(hit.status, StatusCode::OK);
    assert!(
        body_has(
            &hit,
            "<span class=\"resource-name\">A picture of a cat</span>"
        ),
        "the indexed item is listed: {}",
        hit.body
    );

    let miss = send(app, get("/s/museum/item?fulltext_search=absent", false)).await;
    assert_eq!(miss.status, StatusCode::OK);
    assert!(
        !body_has(&miss, "A picture of a cat"),
        "an unmatched query lists nothing: {}",
        miss.body
    );
}

#[tokio::test]
async fn site_item_browse_lists_only_the_items_the_site_holds() {
    let (pool, seed) = site_pool().await;
    let loose = item(&pool, "A loose item", None).await;
    index(
        &pool,
        ResourceId(seed.item.0),
        "items",
        "A picture of a cat",
        "A picture of a cat",
    )
    .await;
    index(
        &pool,
        loose.as_resource(),
        "items",
        "A loose item",
        "A loose item",
    )
    .await;

    let answer = send(router(pool), get("/s/museum/item", false)).await;
    assert_eq!(answer.status, StatusCode::OK);
    assert!(body_has(&answer, "A picture of a cat"), "{}", answer.body);
    assert!(
        !body_has(&answer, "A loose item"),
        "an item attached to no site stays out of the site browse: {}",
        answer.body
    );
}

#[tokio::test]
async fn site_item_browse_orders_by_relevance_for_a_fulltext_search() {
    let (pool, seed) = site_pool().await;
    // The seeded item is the weaker match and carries the lower id, so an id
    // order and a relevance order disagree.
    index(
        &pool,
        ResourceId(seed.item.0),
        "items",
        "A picture of a cat",
        "one bronze mention",
    )
    .await;
    let strong = item(&pool, "A bronze plate", Some(SiteId(seed.site))).await;
    index(
        &pool,
        strong.as_resource(),
        "items",
        "A bronze plate",
        "bronze bronze bronze bronze",
    )
    .await;

    let answer = send(
        router(pool),
        get("/s/museum/item?fulltext_search=bronze", false),
    )
    .await;
    let strong_at = answer
        .body
        .find("A bronze plate")
        .expect("the strong match");
    let weak_at = answer
        .body
        .find("A picture of a cat")
        .expect("the weak match");
    assert!(
        strong_at < weak_at,
        "relevance puts the closer match first: {}",
        answer.body
    );
}

#[tokio::test]
async fn site_search_page_serves_the_item_results() {
    let (pool, seed) = site_pool().await;
    index(
        &pool,
        ResourceId(seed.item.0),
        "items",
        "A picture of a cat",
        "A picture of a cat",
    )
    .await;
    let app = router(pool);

    let answer = send(
        app.clone(),
        get("/s/museum/search?fulltext_search=cat", false),
    )
    .await;
    assert_eq!(answer.status, StatusCode::OK);
    assert!(
        body_has(&answer, "<h1>Search results for “cat”</h1>"),
        "{}",
        answer.body
    );
    assert!(
        body_has(&answer, "<h2>Items</h2>"),
        "the items section: {}",
        answer.body
    );
    assert!(body_has(&answer, "A picture of a cat"), "{}", answer.body);
    assert!(
        body_has(
            &answer,
            "<a href=\"/s/museum/item?fulltext_search=cat\">View all results (1 total)</a>"
        ),
        "the section's link: {}",
        answer.body
    );
    // The default `search_resource_names` also covers site pages, and the index
    // holds none.
    assert!(
        !body_has(&answer, "<h2>Site pages</h2>"),
        "a resource with no match is not a section: {}",
        answer.body
    );

    let miss = send(app, get("/s/museum/search?fulltext_search=absent", false)).await;
    assert_eq!(miss.status, StatusCode::OK);
    assert!(body_has(&miss, "<p>No result found</p>"), "{}", miss.body);
    assert!(!body_has(&miss, "A picture of a cat"), "{}", miss.body);
}

#[tokio::test]
async fn site_search_page_lists_the_site_pages_the_index_holds() {
    let (pool, seed) = site_pool().await;
    let page: i64 = sqlx::query_scalar("SELECT id FROM site_page WHERE slug = 'welcome'")
        .fetch_one(&pool)
        .await
        .unwrap();
    index(
        &pool,
        ResourceId(u64::try_from(page).unwrap()),
        "site_pages",
        "Welcome",
        "Welcome",
    )
    .await;

    let answer = send(
        router(pool),
        get("/s/museum/search?fulltext_search=Welcome", false),
    )
    .await;
    assert_eq!(answer.status, StatusCode::OK);
    assert!(body_has(&answer, "<h2>Site pages</h2>"), "{}", answer.body);
    assert!(
        body_has(&answer, "<a href=\"/s/museum/page/welcome\">Welcome</a>"),
        "a site-page hit links to its page: {}",
        answer.body
    );
    assert!(
        body_has(
            &answer,
            "<a href=\"/s/museum/page?fulltext_search=Welcome\">View all results (1 total)</a>"
        ),
        "{}",
        answer.body
    );
    let _ = seed;
}

/// `/s/:site/index/search` is the path `common/search-form.phtml` posts to; the
/// port keeps `/s/:site/search` too.
#[tokio::test]
async fn the_upstream_search_path_serves_the_same_page() {
    let (pool, seed) = site_pool().await;
    index(
        &pool,
        ResourceId(seed.item.0),
        "items",
        "A picture of a cat",
        "A picture of a cat",
    )
    .await;

    let answer = send(
        router(pool),
        get("/s/museum/index/search?fulltext_search=cat", false),
    )
    .await;
    assert_eq!(answer.status, StatusCode::OK);
    assert!(body_has(&answer, "A picture of a cat"), "{}", answer.body);
}

/// `Site\PageController::browseAction`: the site's public pages, and none of its
/// private ones.
#[tokio::test]
async fn page_browse_lists_the_public_pages_of_the_site() {
    let (pool, _seed) = site_pool().await;
    let answer = send(router(pool), get("/s/museum/page", false)).await;
    assert_eq!(answer.status, StatusCode::OK);
    assert!(body_has(&answer, "<h2>Pages</h2>"), "{}", answer.body);
    // Upstream's `page/browse.phtml` prints each page title in an `h4`, which
    // skips a level under the `Pages` `h2`. The port's theme prints `h3` so the
    // outline stays contiguous; the deviation is recorded in
    // `crates/omeka-site/README.md` and in `crates/omeka-site/src/theme/search.rs`.
    assert!(
        body_has(
            &answer,
            "<li class=\"page resource\"><h3><a href=\"/s/museum/page/welcome\">Welcome</a></h3></li>"
        ),
        "{}",
        answer.body
    );
    assert!(body_has(&answer, ">Preview</a>"), "{}", answer.body);
    assert!(
        !body_has(&answer, "Notes"),
        "a private page: {}",
        answer.body
    );
}

/// A needle narrows the page browse through the `site_pages` fulltext index.
#[tokio::test]
async fn page_browse_filters_by_the_fulltext_index() {
    let (pool, _seed) = site_pool().await;
    let welcome: i64 = sqlx::query_scalar("SELECT id FROM site_page WHERE slug = 'welcome'")
        .fetch_one(&pool)
        .await
        .unwrap();
    index(
        &pool,
        ResourceId(u64::try_from(welcome).unwrap()),
        "site_pages",
        "Welcome",
        "Welcome",
    )
    .await;
    let app = router(pool);

    let hit = send(
        app.clone(),
        get("/s/museum/page?fulltext_search=Welcome", false),
    )
    .await;
    assert!(body_has(&hit, ">Welcome</a>"), "{}", hit.body);
    assert!(!body_has(&hit, ">Preview</a>"), "{}", hit.body);

    let miss = send(app, get("/s/museum/page?fulltext_search=absent", false)).await;
    assert!(body_has(&miss, "<p>No pages found</p>"), "{}", miss.body);
}

#[tokio::test]
async fn site_item_search_renders_the_advanced_form() {
    let (pool, _seed) = site_pool().await;
    let app = router(pool);

    let answer = send(app.clone(), get("/s/museum/item/search", false)).await;
    assert_eq!(answer.status, StatusCode::OK);
    assert!(
        body_has(&answer, "<h2>Advanced item search</h2>"),
        "{}",
        answer.body
    );
    assert!(
        body_has(
            &answer,
            "<form id=\"advanced-search\" method=\"get\" action=\"/s/museum/item\">"
        ),
        "the form posts to the item browse: {}",
        answer.body
    );
    for name in [
        "name=\"sort_by\"",
        "name=\"sort_order\"",
        "name=\"fulltext_search\"",
        "name=\"property[0][joiner]\"",
        "name=\"property[0][property]\"",
        "name=\"property[0][type]\"",
        "name=\"property[0][text]\"",
        "name=\"resource_class_id[]\"",
        "name=\"item_set_id[]\"",
        "name=\"has_media\"",
        "name=\"id\"",
    ] {
        assert!(body_has(&answer, name), "{name}: {}", answer.body);
    }

    // The form keeps the values the request carried, including a property row.
    let filled = send(
        app,
        get(
            "/s/museum/item/search?fulltext_search=cat&sort_by=title&has_media=1&property[0][property]=1&property[0][type]=in&property[0][text]=cat",
            false,
        ),
    )
    .await;
    assert!(
        body_has(
            &filled,
            "name=\"fulltext_search\" id=\"fulltext_search\" value=\"cat\""
        ),
        "{}",
        filled.body
    );
    assert!(
        body_has(
            &filled,
            "<option value=\"title\" selected=\"selected\">Title</option>"
        ),
        "{}",
        filled.body
    );
    assert!(
        body_has(
            &filled,
            "<input type=\"text\" class=\"query-text\" name=\"property[0][text]\" value=\"cat\""
        ),
        "{}",
        filled.body
    );
}

#[tokio::test]
async fn site_item_set_search_renders_the_item_set_form() {
    let (pool, _seed) = site_pool().await;
    let answer = send(router(pool), get("/s/museum/item-set/search", false)).await;
    assert_eq!(answer.status, StatusCode::OK);
    assert!(
        body_has(
            &answer,
            "<form id=\"advanced-search\" method=\"get\" action=\"/s/museum/item-set\">"
        ),
        "{}",
        answer.body
    );
    assert!(
        !body_has(&answer, "name=\"has_media\""),
        "the item-set form carries no media filter: {}",
        answer.body
    );
    assert!(
        !body_has(&answer, "item_set_id[]"),
        "the item-set form carries no item-set filter: {}",
        answer.body
    );
}

#[tokio::test]
async fn cross_site_search_index_renders_the_fulltext_form() {
    let (pool, _seed) = site_pool().await;
    let answer = send(router(pool), get("/s/museum/cross-site-search", false)).await;
    assert_eq!(answer.status, StatusCode::OK);
    assert!(
        body_has(
            &answer,
            "<form action=\"/s/museum/cross-site-search/results\" class=\"sitewide-search-form\" role=\"search\">"
        ),
        "{}",
        answer.body
    );
    assert!(
        !body_has(&answer, "results\"><ul>"),
        "the index action renders the form alone: {}",
        answer.body
    );
}

/// The frame's sitewide form posts to `search_type`'s target, which defaults to
/// the site search.
#[tokio::test]
async fn the_frame_renders_the_sitewide_search_form() {
    let (pool, _seed) = site_pool().await;
    let answer = send(router(pool), get("/s/museum/page/welcome", false)).await;
    assert_eq!(answer.status, StatusCode::OK);
    assert!(
        body_has(
            &answer,
            "<form action=\"/s/museum/index/search\" id=\"search-form\""
        ),
        "{}",
        answer.body
    );
}

#[tokio::test]
async fn cross_site_search_spans_the_sites() {
    let (pool, _seed) = site_pool().await;
    let other_site = SiteRepo::new(&pool)
        .create(&NewSite {
            slug: "archive".to_string(),
            theme: "default".to_string(),
            title: "Archive".to_string(),
            ..NewSite::default()
        })
        .await
        .unwrap();
    let elsewhere = item(&pool, "A bronze plate", Some(other_site.id)).await;
    index(
        &pool,
        elsewhere.as_resource(),
        "items",
        "A bronze plate",
        "A bronze plate",
    )
    .await;

    let answer = send(
        router(pool),
        get(
            "/s/museum/cross-site-search/results?fulltext_search=bronze",
            false,
        ),
    )
    .await;
    assert_eq!(answer.status, StatusCode::OK);
    assert!(
        body_has(&answer, "<h2>Search results for “bronze”</h2>"),
        "{}",
        answer.body
    );
    assert!(
        body_has(&answer, "A bronze plate"),
        "an item attached to another site is a cross-site result: {}",
        answer.body
    );
    assert!(
        body_has(
            &answer,
            "<a href=\"/s/museum/cross-site-search/items?fulltext_search=bronze\">View all results (1 total)</a>"
        ),
        "the section links to the action: {}",
        answer.body
    );
}

/// The `items` action lists the resources across sites, with the advance form
/// link upstream prints.
#[tokio::test]
async fn cross_site_items_action_lists_the_items() {
    let (pool, _seed) = site_pool().await;
    let other_site = SiteRepo::new(&pool)
        .create(&NewSite {
            slug: "archive".to_string(),
            theme: "default".to_string(),
            title: "Archive".to_string(),
            ..NewSite::default()
        })
        .await
        .unwrap();
    let elsewhere = item(&pool, "A bronze plate", Some(other_site.id)).await;
    index(
        &pool,
        elsewhere.as_resource(),
        "items",
        "A bronze plate",
        "A bronze plate",
    )
    .await;

    let answer = send(
        router(pool),
        get(
            "/s/museum/cross-site-search/items?fulltext_search=bronze",
            false,
        ),
    )
    .await;
    assert_eq!(answer.status, StatusCode::OK);
    assert!(
        body_has(&answer, "<h2>Item results for &quot;bronze&quot;</h2>"),
        "{}",
        answer.body
    );
    assert!(
        body_has(
            &answer,
            "<a href=\"/s/museum/cross-site-search/items-advanced?fulltext_search=bronze\" class=\"advanced-search\">Advanced item search</a>"
        ),
        "{}",
        answer.body
    );
    assert!(body_has(&answer, "A bronze plate"), "{}", answer.body);
}

#[tokio::test]
async fn cross_site_search_skips_an_item_attached_to_no_site() {
    let (pool, _seed) = site_pool().await;
    let loose = item(&pool, "A loose bronze item", None).await;
    index(
        &pool,
        loose.as_resource(),
        "items",
        "A loose bronze item",
        "A loose bronze item",
    )
    .await;

    let answer = send(
        router(pool),
        get(
            "/s/museum/cross-site-search/results?fulltext_search=bronze",
            false,
        ),
    )
    .await;
    assert_eq!(answer.status, StatusCode::OK);
    assert!(
        !body_has(&answer, "A loose bronze item"),
        "`in_sites` keeps an unattached item out: {}",
        answer.body
    );
}

#[tokio::test]
async fn cross_site_search_item_sets_action_lists_the_sets() {
    let (pool, seed) = site_pool().await;
    index(
        &pool,
        ResourceId(seed.item_set.0),
        "item_sets",
        "Featured photographs",
        "Featured photographs",
    )
    .await;

    let answer = send(
        router(pool),
        get("/s/museum/cross-site-search/item-sets", false),
    )
    .await;
    assert_eq!(answer.status, StatusCode::OK);
    assert!(body_has(&answer, "Featured photographs"), "{}", answer.body);
    assert!(
        body_has(&answer, "<h2>Item set results for &quot;&quot;</h2>"),
        "{}",
        answer.body
    );
}

/// `site-pages` lists the pages of every public site, each naming its site.
#[tokio::test]
async fn cross_site_site_pages_action_lists_the_pages() {
    let (pool, _seed) = site_pool().await;
    let answer = send(
        router(pool),
        get("/s/museum/cross-site-search/site-pages", false),
    )
    .await;
    assert_eq!(answer.status, StatusCode::OK);
    assert!(
        body_has(&answer, "<h2>Site page results for &quot;&quot;</h2>"),
        "{}",
        answer.body
    );
    assert!(
        body_has(
            &answer,
            "<li><span class=\"result-title\"><a href=\"/s/museum/page/welcome\" class=\"result-title\">Welcome</a></span><span class=\"result-site\"><a href=\"/s/museum\">Museum</a></span></li>"
        ),
        "a page hit names its site: {}",
        answer.body
    );
}

#[tokio::test]
async fn cross_site_advanced_forms_post_to_the_action() {
    let (pool, _seed) = site_pool().await;
    let app = router(pool);

    let items = send(
        app.clone(),
        get("/s/museum/cross-site-search/items-advanced", false),
    )
    .await;
    assert_eq!(items.status, StatusCode::OK);
    assert!(
        body_has(
            &items,
            "<form id=\"advanced-search\" method=\"get\" action=\"/s/museum/cross-site-search/items\">"
        ),
        "{}",
        items.body
    );

    let sets = send(
        app,
        get("/s/museum/cross-site-search/item-sets-advanced", false),
    )
    .await;
    assert_eq!(sets.status, StatusCode::OK);
    assert!(
        body_has(
            &sets,
            "<form id=\"advanced-search\" method=\"get\" action=\"/s/museum/cross-site-search/item-sets\">"
        ),
        "{}",
        sets.body
    );
}

#[tokio::test]
async fn cross_site_unknown_action_is_not_found() {
    let (pool, _seed) = site_pool().await;
    let answer = send(
        router(pool),
        get("/s/museum/cross-site-search/no-such-action", false),
    )
    .await;
    assert_eq!(answer.status, StatusCode::NOT_FOUND);
}

/// A parameter the search does not know is ignored, not an error: the API's
/// parser reads the names it supports and drops the rest.
#[tokio::test]
async fn an_unknown_parameter_is_ignored() {
    let (pool, seed) = site_pool().await;
    index(
        &pool,
        ResourceId(seed.item.0),
        "items",
        "A picture of a cat",
        "A picture of a cat",
    )
    .await;
    let app = router(pool);

    for path in [
        "/s/museum/search?fulltext_search=cat&bogus=1",
        "/s/museum/item?fulltext_search=cat&bogus=1",
        "/s/museum/page?bogus=1",
        "/s/museum/cross-site-search/results?fulltext_search=cat&bogus=1",
        "/s/museum/item/search?bogus=1",
    ] {
        let answer = send(app.clone(), get(path, false)).await;
        assert_eq!(answer.status, StatusCode::OK, "{path}");
    }

    let answer = send(
        app,
        get("/s/museum/search?fulltext_search=cat&bogus=1", false),
    )
    .await;
    assert!(body_has(&answer, "A picture of a cat"), "{}", answer.body);
}

#[tokio::test]
async fn site_item_set_browse_filters_by_fulltext_search() {
    let (pool, seed) = site_pool().await;
    index(
        &pool,
        ResourceId(seed.item_set.0),
        "item_sets",
        "Featured photographs",
        "Featured photographs",
    )
    .await;
    let app = router(pool);

    let hit = send(
        app.clone(),
        get("/s/museum/item-set?fulltext_search=photographs", false),
    )
    .await;
    assert_eq!(hit.status, StatusCode::OK);
    assert!(body_has(&hit, "Featured photographs"), "{}", hit.body);

    let miss = send(app, get("/s/museum/item-set?fulltext_search=absent", false)).await;
    assert!(!body_has(&miss, "Featured photographs"), "{}", miss.body);
}

/// A property row narrows a site browse the same way it narrows the API list.
#[tokio::test]
async fn site_item_browse_accepts_a_property_row() {
    let (pool, _seed) = site_pool().await;
    let title: i64 = sqlx::query_scalar(
        "SELECT id FROM property WHERE local_name = 'title' ORDER BY id LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let app = router(pool);

    let hit = send(
        app.clone(),
        get(
            &format!(
                "/s/museum/item?property[0][property]={title}&property[0][type]=in&property[0][text]=cat"
            ),
            false,
        ),
    )
    .await;
    assert_eq!(hit.status, StatusCode::OK);
    assert!(body_has(&hit, "A picture of a cat"), "{}", hit.body);

    let miss = send(
        app,
        get(
            &format!(
                "/s/museum/item?property[0][property]={title}&property[0][type]=in&property[0][text]=absent"
            ),
            false,
        ),
    )
    .await;
    assert!(!body_has(&miss, "A picture of a cat"), "{}", miss.body);
}

/// A site browse with no search parameters still lists the site's rows.
#[tokio::test]
async fn site_browse_without_search_parameters_is_unchanged() {
    let (pool, _seed) = site_pool().await;
    let app = router(pool);

    let items = send(app.clone(), get("/s/museum/item", false)).await;
    assert_eq!(items.status, StatusCode::OK);
    assert!(body_has(&items, "<h2>Items</h2>"), "{}", items.body);
    assert!(body_has(&items, "A picture of a cat"), "{}", items.body);

    let sets = send(app, get("/s/museum/item-set", false)).await;
    assert_eq!(sets.status, StatusCode::OK);
    assert!(body_has(&sets, "Featured photographs"), "{}", sets.body);
}
