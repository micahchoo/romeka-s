//! The welcome page a site created through the API starts with: the port's
//! starter (a banner, an introduction, the latest items) in place of
//! upstream's style sampler, stored in blocks upstream can render.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::http::StatusCode;
use common::{append_key, json_request, seeded_router, send};

#[tokio::test]
async fn a_new_site_welcome_page_is_the_starter() {
    let (app, _seed) = seeded_router().await;
    let body = r#"{"o:slug":"starter","o:title":"Starter","o:theme":"default","o:is_public":true}"#;
    let created = send(
        app.clone(),
        json_request("POST", &append_key("/api/sites"), body),
    )
    .await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.body);
    let page_id = created.json()["o:page"][0]["o:id"].as_u64().unwrap();

    let stored = send(
        app.clone(),
        common::get(&format!("/api/site_pages/{page_id}"), false),
    )
    .await;
    let document = stored.json();
    let layouts: Vec<&str> = document["o:block"]
        .as_array()
        .unwrap()
        .iter()
        .map(|block| block["o:layout"].as_str().unwrap())
        .collect();
    assert_eq!(layouts, ["html", "html", "browsePreview"]);
    // The banner's carrier holds its plain rendering, which is what upstream
    // shows, and a hash that vouches for it.
    let banner = &document["o:block"][0];
    assert_eq!(
        banner["o:data"]["html"],
        "<h2>Welcome</h2><p>A new collection, ready for its first visitors.</p>\
         <a href=\"../item\" rel=\"noreferrer noopener\">Explore the collection</a>"
    );
    assert!(banner["o:layout_data"]["romeka"]["plain_hash"].is_string());

    let page = send(app, common::get("/s/starter/page/welcome", false)).await;
    assert_eq!(page.status, StatusCode::OK);
    assert!(
        page.body
            .contains("<h2 class=\"romeka-hero-heading\">Welcome</h2>"),
        "{}",
        page.body
    );
    assert!(
        page.body
            .contains("<a href=\"../item\" class=\"romeka-hero-cta\">"),
        "{}",
        page.body
    );
    assert!(page.body.contains("This is the first page of your site."));
    assert!(
        page.body.contains("<h2>Recently added</h2>"),
        "{}",
        page.body
    );
    assert!(!page.body.contains("Special Container"));
}

/// A port default: a site created with no `o:navigation` links its welcome
/// page first, so the site root redirects to the starter
/// (`Site\IndexController::indexAction` follows the first linked page).
/// Upstream's default navigation holds the browse link alone.
#[tokio::test]
async fn a_new_site_opens_on_its_welcome_page() {
    let (app, _seed) = seeded_router().await;
    let body = r#"{"o:slug":"opens","o:title":"Opens","o:theme":"default","o:is_public":true}"#;
    let created = send(
        app.clone(),
        json_request("POST", &append_key("/api/sites"), body),
    )
    .await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.body);
    let site = created.json();
    let welcome = site["o:page"][0]["o:id"].as_u64().unwrap();
    assert_eq!(
        site["o:navigation"],
        serde_json::json!([
            { "type": "page", "data": { "label": null, "id": welcome }, "links": [] },
            { "type": "browse", "data": { "label": "Browse", "query": "" }, "links": [] },
        ])
    );
    assert!(site["o:homepage"].is_null());

    let root = send(app.clone(), common::get("/s/opens", false)).await;
    assert!(root.status.is_redirection(), "{}", root.status);
    let location = root.headers["location"].to_str().unwrap();
    assert!(location.ends_with("/s/opens/page/welcome"), "{location}");
    let page = send(app, common::get(location, false)).await;
    assert!(
        page.body
            .contains("<h2 class=\"romeka-hero-heading\">Welcome</h2>"),
        "{}",
        page.body
    );
}

/// `SiteAdapter::hydrate`: a create that sends `o:navigation` stores exactly
/// what it sent, an empty tree included, and the welcome page stays unlinked.
#[tokio::test]
async fn a_new_site_keeps_the_navigation_it_was_sent() {
    let (app, _seed) = seeded_router().await;
    let body = r#"{"o:slug":"kept","o:title":"Kept","o:theme":"default","o:navigation":[]}"#;
    let created = send(
        app.clone(),
        json_request("POST", &append_key("/api/sites"), body),
    )
    .await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.body);
    assert_eq!(created.json()["o:navigation"], serde_json::json!([]));
    let root = send(app, common::get("/s/kept", false)).await;
    assert_eq!(root.status, StatusCode::OK);
    assert!(
        root.body.contains("<p>This site has no pages.</p>"),
        "{}",
        root.body
    );
}
