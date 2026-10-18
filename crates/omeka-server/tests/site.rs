//! Public site route tests: the `/s/:site-slug/...` paths served through the
//! axum router against a seeded SQLite database.
//!
//! One test per route family; the names cite the upstream controller or view
//! each route reaches where one exists.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::http::{StatusCode, header};
use common::{get, header as header_value, request, router, seeded_pool, send, site_pool};
use omeka_db::ids::UserId;
use omeka_db::repos::{ItemRepo, SiteRepo};
use omeka_db::rows::{NewItem, NewResource, NewSite};

/// `Site\IndexController::indexAction`: a site with pages and no homepage
/// redirects to the first page its navigation links.
#[tokio::test]
async fn site_home_redirects_to_the_first_page() {
    let (pool, _seed) = site_pool().await;
    let answer = send(router(pool), get("/s/museum", false)).await;
    assert_eq!(answer.status, StatusCode::FOUND);
    assert_eq!(
        header_value(&answer, "location"),
        Some("/s/museum/page/welcome")
    );
}

/// `view/omeka/site/index/index.phtml`: a site with no pages renders the index.
#[tokio::test]
async fn site_home_renders_the_index_when_the_site_has_no_pages() {
    let (pool, _seed) = seeded_pool().await;
    SiteRepo::new(&pool)
        .create(&NewSite {
            slug: "empty".to_string(),
            theme: "default".to_string(),
            title: "Empty".to_string(),
            ..NewSite::default()
        })
        .await
        .unwrap();
    let answer = send(router(pool), get("/s/empty", false)).await;
    assert_eq!(answer.status, StatusCode::OK);
    assert_eq!(
        header_value(&answer, "content-type"),
        Some("text/html; charset=utf-8")
    );
    assert!(
        answer.body.contains("<p>This site has no pages.</p>"),
        "{}",
        answer.body
    );
}

/// `view/omeka/site/page/content.phtml`: the page's blocks render in position
/// order, inside the layout.
#[tokio::test]
async fn page_show_renders_every_block_in_position_order() {
    let (pool, _seed) = site_pool().await;
    let answer = send(router(pool), get("/s/museum/page/welcome", false)).await;
    assert_eq!(answer.status, StatusCode::OK);
    assert!(
        answer.body.starts_with("<!DOCTYPE html>"),
        "{}",
        answer.body
    );
    assert!(
        answer.body.contains("<title>Welcome · Omeka S</title>"),
        "{}",
        answer.body
    );
    let title_at = answer
        .body
        .find("<h2>Welcome</h2>")
        .expect("page title block");
    let html_at = answer.body.find("<p>Body copy</p>").expect("html block");
    let break_at = answer
        .body
        .find("<div class='break opaque'></div>")
        .expect("line break block");
    assert!(
        title_at < html_at && html_at < break_at,
        "blocks are out of order: {}",
        answer.body
    );
}

/// `Laminas\View\Helper\EscapeHtml`: a stored title carrying markup renders
/// escaped.
#[tokio::test]
async fn page_show_escapes_a_value_carrying_markup() {
    let (pool, _seed) = site_pool().await;
    let answer = send(router(pool), get("/s/museum/page/markup", false)).await;
    assert_eq!(answer.status, StatusCode::OK);
    assert!(!answer.body.contains("<script>alert"), "{}", answer.body);
    assert!(answer.body.contains("&lt;script&gt;"), "{}", answer.body);
}

/// `Mvc\ExceptionListener` over `view/error/404.phtml`: an unknown site slug is
/// a 404 inside the site layout.
#[tokio::test]
async fn unknown_slug_is_404_with_the_error_page() {
    let (pool, _seed) = site_pool().await;
    let answer = send(router(pool), get("/s/nope", false)).await;
    assert_eq!(answer.status, StatusCode::NOT_FOUND);
    assert_eq!(
        header_value(&answer, "content-type"),
        Some("text/html; charset=utf-8")
    );
    assert!(
        answer.body.contains("<h1>Page not found</h1>"),
        "{}",
        answer.body
    );
    assert!(
        answer
            .body
            .contains("The page you are trying to view does not exist."),
        "{}",
        answer.body
    );
}

/// `view/error/403.phtml`: a page that exists but is not public is a 403.
#[tokio::test]
async fn non_public_page_is_403() {
    let (pool, _seed) = site_pool().await;
    let answer = send(router(pool), get("/s/museum/page/notes", false)).await;
    assert_eq!(answer.status, StatusCode::FORBIDDEN);
    assert!(
        answer.body.contains("<h1>Forbidden</h1>"),
        "{}",
        answer.body
    );
}

/// `view/error/403.phtml`: a site that is not public is a 403.
#[tokio::test]
async fn non_public_site_is_403() {
    let (pool, _seed) = seeded_pool().await;
    SiteRepo::new(&pool)
        .create(&NewSite {
            slug: "private".to_string(),
            theme: "default".to_string(),
            title: "Private".to_string(),
            is_public: false,
            ..NewSite::default()
        })
        .await
        .unwrap();
    let answer = send(router(pool), get("/s/private", false)).await;
    assert_eq!(answer.status, StatusCode::FORBIDDEN);
    assert!(
        answer.body.contains("<h1>Forbidden</h1>"),
        "{}",
        answer.body
    );
}

/// `view/error/403.phtml`: an item that is not public is a 403.
#[tokio::test]
async fn non_public_item_is_403() {
    let (pool, seed) = site_pool().await;
    let item = ItemRepo::new(&pool)
        .create(&NewItem {
            resource: NewResource {
                owner_id: Some(UserId(seed.user.0)),
                title: Some("A private item".to_string()),
                is_public: false,
                ..NewResource::default()
            },
            primary_media_id: None,
        })
        .await
        .unwrap();
    let path = format!("/s/museum/item/{}", item.id().0);
    let answer = send(router(pool), get(&path, false)).await;
    assert_eq!(answer.status, StatusCode::FORBIDDEN);
    assert!(
        answer.body.contains("<h1>Forbidden</h1>"),
        "{}",
        answer.body
    );
}

/// A stored page slug matches the request case-insensitively, as the column's
/// `COLLATE NOCASE` declares.
#[tokio::test]
async fn page_slug_matches_case_insensitively() {
    let (pool, _seed) = site_pool().await;
    let answer = send(router(pool), get("/s/museum/page/WELCOME", false)).await;
    assert_eq!(answer.status, StatusCode::OK);
    assert!(answer.body.contains("<h2>Welcome</h2>"), "{}", answer.body);
}

/// `Site\PageController::showAction`: a slug no page of the site carries is a
/// 404.
#[tokio::test]
async fn unknown_page_slug_is_404() {
    let (pool, _seed) = site_pool().await;
    let answer = send(router(pool), get("/s/museum/page/missing", false)).await;
    assert_eq!(answer.status, StatusCode::NOT_FOUND);
}

/// `view/omeka/site/item/show.phtml`: the item title, the `Item` label and the
/// values.
#[tokio::test]
async fn item_show_renders_the_item_values() {
    let (pool, seed) = site_pool().await;
    let path = format!("/s/museum/item/{}", seed.item.0);
    let answer = send(router(pool), get(&path, false)).await;
    assert_eq!(answer.status, StatusCode::OK);
    assert!(
        answer.body.contains("<h2>A picture of a cat</h2>"),
        "{}",
        answer.body
    );
    assert!(answer.body.contains("<h3>Item</h3>"), "{}", answer.body);
    assert!(answer.body.contains("<dt>Title</dt>"), "{}", answer.body);
    assert!(
        answer
            .body
            .contains("<dd class=\"value\">A picture of a cat</dd>"),
        "{}",
        answer.body
    );
}

/// `view/omeka/site/item/browse.phtml`: the public item list.
#[tokio::test]
async fn item_browse_lists_the_site_items() {
    let (pool, seed) = site_pool().await;
    let answer = send(router(pool), get("/s/museum/item", false)).await;
    assert_eq!(answer.status, StatusCode::OK);
    assert!(answer.body.contains("<h2>Items</h2>"), "{}", answer.body);
    assert!(
        answer.body.contains("<li class=\"item resource\">"),
        "{}",
        answer.body
    );
    assert!(
        answer
            .body
            .contains(&format!("/s/museum/item/{}", seed.item.0)),
        "{}",
        answer.body
    );
    assert!(
        answer.body.contains("A picture of a cat"),
        "{}",
        answer.body
    );
}

/// `view/omeka/site/item-set/browse.phtml`: the public item-set list.
#[tokio::test]
async fn item_set_browse_lists_the_sets() {
    let (pool, _seed) = site_pool().await;
    let answer = send(router(pool), get("/s/museum/item-set", false)).await;
    assert_eq!(answer.status, StatusCode::OK);
    assert!(
        answer.body.contains("<h2>Item sets</h2>"),
        "{}",
        answer.body
    );
    assert!(
        answer.body.contains("<li class=\"item-set resource\">"),
        "{}",
        answer.body
    );
    assert!(
        answer.body.contains("Featured photographs"),
        "{}",
        answer.body
    );
}

/// `view/omeka/site/item-set/browse.phtml` for a single set.
#[tokio::test]
async fn item_set_show_renders_the_item_set() {
    let (pool, seed) = site_pool().await;
    let path = format!("/s/museum/item-set/{}", seed.item_set.0);
    let answer = send(router(pool), get(&path, false)).await;
    assert_eq!(answer.status, StatusCode::OK);
    assert!(answer.body.contains("<h3>Item set</h3>"), "{}", answer.body);
    assert!(
        answer.body.contains("<h2>Featured photographs</h2>"),
        "{}",
        answer.body
    );
}

/// `view/omeka/site/media/show.phtml`: the media view names its parent item.
#[tokio::test]
async fn media_show_names_the_parent_item() {
    let (pool, seed) = site_pool().await;
    let path = format!("/s/museum/media/{}", seed.media.0);
    let answer = send(router(pool), get(&path, false)).await;
    assert_eq!(answer.status, StatusCode::OK);
    assert!(answer.body.contains("<h3>Media</h3>"), "{}", answer.body);
    assert!(
        answer.body.contains(&format!(
            "Part of <a href=\"/s/museum/item/{}\">A picture of a cat</a>",
            seed.item.0
        )),
        "{}",
        answer.body
    );
}

/// `Mvc\ExceptionListener`: a missing resource id is a 404.
#[tokio::test]
async fn missing_item_is_404() {
    let (pool, _seed) = site_pool().await;
    let answer = send(router(pool), get("/s/museum/item/999999", false)).await;
    assert_eq!(answer.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn missing_item_set_is_404() {
    let (pool, _seed) = site_pool().await;
    let answer = send(router(pool), get("/s/museum/item-set/999999", false)).await;
    assert_eq!(answer.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn missing_media_is_404() {
    let (pool, _seed) = site_pool().await;
    let answer = send(router(pool), get("/s/museum/media/999999", false)).await;
    assert_eq!(answer.status, StatusCode::NOT_FOUND);
}

/// `routes.config.php`: a controller segment that is not `item`, `item-set` or
/// `media` names no resource controller.
#[tokio::test]
async fn unknown_controller_is_404() {
    let (pool, _seed) = site_pool().await;
    let app = router(pool);
    for path in ["/s/museum/nonsense", "/s/museum/nonsense/3"] {
        let answer = send(app.clone(), get(path, false)).await;
        assert_eq!(answer.status, StatusCode::NOT_FOUND, "path {path}");
    }
}

/// `view/common/block-layout/browse-preview.phtml`: the block renders the items
/// its search returns.
#[tokio::test]
async fn browse_preview_block_renders_its_items() {
    let (pool, _seed) = site_pool().await;
    let answer = send(router(pool), get("/s/museum/page/preview", false)).await;
    assert_eq!(answer.status, StatusCode::OK);
    assert!(
        answer.body.contains("<div class=\"preview-block\">"),
        "{}",
        answer.body
    );
    assert!(
        answer
            .body
            .contains("<span class=\"resource-name\">A picture of a cat</span>"),
        "{}",
        answer.body
    );
}

/// Only `GET` reaches the site routes; another method is a 405 with `Allow`.
#[tokio::test]
async fn post_to_a_site_is_405() {
    let (pool, _seed) = site_pool().await;
    let answer = send(router(pool), request("POST", "/s/museum")).await;
    assert_eq!(answer.status, StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(answer.headers.get(header::ALLOW).unwrap(), "GET");
}

/// Every route family's status in one place.
#[tokio::test]
async fn site_route_table() {
    let (pool, seed) = site_pool().await;
    let app = router(pool);
    let cases: Vec<(String, StatusCode)> = vec![
        ("/s/museum".to_string(), StatusCode::FOUND),
        ("/s/museum/page/welcome".to_string(), StatusCode::OK),
        ("/s/museum/page/WELCOME".to_string(), StatusCode::OK),
        ("/s/museum/page/notes".to_string(), StatusCode::FORBIDDEN),
        ("/s/museum/page/missing".to_string(), StatusCode::NOT_FOUND),
        (format!("/s/museum/item/{}", seed.item.0), StatusCode::OK),
        ("/s/museum/item".to_string(), StatusCode::OK),
        ("/s/museum/item-set".to_string(), StatusCode::OK),
        (
            format!("/s/museum/item-set/{}", seed.item_set.0),
            StatusCode::OK,
        ),
        (format!("/s/museum/media/{}", seed.media.0), StatusCode::OK),
        ("/s/museum/nonsense".to_string(), StatusCode::NOT_FOUND),
        ("/s/nope".to_string(), StatusCode::NOT_FOUND),
        ("/s".to_string(), StatusCode::NOT_FOUND),
    ];
    for (path, status) in cases {
        let answer = send(app.clone(), get(&path, false)).await;
        assert_eq!(answer.status, status, "path {path}");
    }
}
