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
