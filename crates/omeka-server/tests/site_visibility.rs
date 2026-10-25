//! Site visibility through the public seams: the `/api` page and site reads and
//! the `/s` public pages.
//!
//! One test per problem from the audit. Upstream's rules, measured against the
//! oracle: a page is visible when it is public on a public site, or the caller
//! owns the site, or the caller holds a site permission (`Db\Filter\SitePageVisibilityFilter`);
//! a private resource is visible to its owner (`Db\Filter\ResourceVisibilityFilter`);
//! a hidden site, page or resource is a 404, not a 403.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::http::StatusCode;
use common::{
    OWNER_KEY_CREDENTIAL, OWNER_KEY_IDENTITY, VIEWER_KEY_CREDENTIAL, VIEWER_KEY_IDENTITY,
    VISIBILITY_PASSWORD, get, get_cookie, get_keyed, json_request, login_cookie, router, send,
    visibility_pool,
};

/// Problem 1: an anonymous `GET /api/site_pages/{id}` must not answer with a
/// public page of a private site.
#[tokio::test]
async fn api_site_page_read_hides_a_page_of_a_private_site_from_anonymous() {
    let (pool, vis) = visibility_pool().await;
    let app = router(pool);
    let path = format!("/api/site_pages/{}", vis.private_site_public_page);
    let answer = send(app, get(&path, false)).await;
    assert_eq!(answer.status, StatusCode::NOT_FOUND, "{}", answer.body);
}

/// Problem 1: a private page on a public site is also hidden from an anonymous
/// caller.
#[tokio::test]
async fn api_site_page_read_hides_a_private_page_from_anonymous() {
    let (pool, vis) = visibility_pool().await;
    let app = router(pool);
    let path = format!("/api/site_pages/{}", vis.private_page);
    let answer = send(app, get(&path, false)).await;
    assert_eq!(answer.status, StatusCode::NOT_FOUND, "{}", answer.body);
}

/// Problem 1: the site owner and a user holding a site permission see both a
/// private page of the public site and a public page of the private site,
/// matching `SitePageVisibilityFilter`'s owner and permission branches.
#[tokio::test]
async fn api_site_page_read_shows_hidden_pages_to_the_owner_and_a_site_viewer() {
    let (pool, vis) = visibility_pool().await;
    let app = router(pool);
    for (page, title) in [
        (vis.private_page, "Vis private page"),
        (vis.private_site_public_page, "Vis private site page"),
    ] {
        let path = format!("/api/site_pages/{page}");
        for (identity, credential, who) in [
            (OWNER_KEY_IDENTITY, OWNER_KEY_CREDENTIAL, "owner"),
            (VIEWER_KEY_IDENTITY, VIEWER_KEY_CREDENTIAL, "viewer"),
        ] {
            let answer = send(app.clone(), get_keyed(&path, identity, credential)).await;
            assert_eq!(answer.status, StatusCode::OK, "{who}: {}", answer.body);
            assert_eq!(answer.json()["o:title"], serde_json::json!(title), "{who}");
        }
    }
}

/// Problem 1: the page list with `site_id` must not carry a page of a private
/// site to an anonymous caller, and must not carry a private page of a public
/// site either.
#[tokio::test]
async fn api_site_page_list_hides_pages_the_actor_cannot_see() {
    let (pool, vis) = visibility_pool().await;
    let app = router(pool);
    for (site, hidden) in [
        (vis.private_site, vis.private_site_public_page),
        (vis.public_site, vis.private_page),
    ] {
        let answer = send(
            app.clone(),
            get(&format!("/api/site_pages?site_id={site}"), false),
        )
        .await;
        assert_eq!(answer.status, StatusCode::OK);
        let ids: Vec<u64> = answer
            .json()
            .as_array()
            .unwrap()
            .iter()
            .map(|doc| doc["o:id"].as_u64().unwrap())
            .collect();
        assert!(!ids.contains(&hidden), "site {site}: {ids:?}");
    }
    let answer = send(
        app,
        get(
            &format!("/api/site_pages?site_id={}", vis.public_site),
            false,
        ),
    )
    .await;
    let ids: Vec<u64> = answer
        .json()
        .as_array()
        .unwrap()
        .iter()
        .map(|doc| doc["o:id"].as_u64().unwrap())
        .collect();
    assert!(ids.contains(&vis.public_page), "{ids:?}");
}

/// Problem 1 and problem 6: a private site read answers the site to its owner
/// and to a viewer permission holder; the site list shows it to both.
#[tokio::test]
async fn api_site_read_shows_a_private_site_to_the_owner_and_a_site_viewer() {
    let (pool, vis) = visibility_pool().await;
    let app = router(pool);
    let path = format!("/api/sites/{}", vis.private_site);
    assert_eq!(
        send(app.clone(), get(&path, false)).await.status,
        StatusCode::NOT_FOUND
    );
    for (identity, credential, who) in [
        (OWNER_KEY_IDENTITY, OWNER_KEY_CREDENTIAL, "owner"),
        (VIEWER_KEY_IDENTITY, VIEWER_KEY_CREDENTIAL, "viewer"),
    ] {
        let answer = send(app.clone(), get_keyed(&path, identity, credential)).await;
        assert_eq!(answer.status, StatusCode::OK, "{who}: {}", answer.body);
    }
}

/// Problem 1: the site document's `o:page` list is filtered by the same rule,
/// so an anonymous caller does not learn the private page's id.
#[tokio::test]
async fn api_site_document_lists_only_the_pages_the_actor_can_see() {
    let (pool, vis) = visibility_pool().await;
    let app = router(pool);
    let path = format!("/api/sites/{}", vis.public_site);

    let anonymous = send(app.clone(), get(&path, false)).await;
    let anon_pages = page_ids(&anonymous);
    assert!(anon_pages.contains(&vis.public_page), "{anon_pages:?}");
    assert!(!anon_pages.contains(&vis.private_page), "{anon_pages:?}");

    let owner = send(
        app,
        get_keyed(&path, OWNER_KEY_IDENTITY, OWNER_KEY_CREDENTIAL),
    )
    .await;
    let owner_pages = page_ids(&owner);
    assert!(owner_pages.contains(&vis.private_page), "{owner_pages:?}");
}

/// Problem 5 and problem 6: a private site is a 404 for an anonymous caller and
/// a 200 for its owner and a site viewer, read through the session cookie.
#[tokio::test]
async fn private_site_is_404_for_anonymous_and_200_for_the_owner_and_viewer() {
    let (pool, vis) = visibility_pool().await;
    let app = router(pool);

    let anonymous = send(app.clone(), get("/s/vis-priv", false)).await;
    assert_eq!(
        anonymous.status,
        StatusCode::NOT_FOUND,
        "{}",
        anonymous.body
    );

    let owner = login_cookie(app.clone(), vis.owner_email, VISIBILITY_PASSWORD).await;
    let answer = send(app.clone(), get_cookie("/s/vis-priv", &owner)).await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);

    let viewer = login_cookie(app.clone(), vis.viewer_email, VISIBILITY_PASSWORD).await;
    let answer = send(app, get_cookie("/s/vis-priv", &viewer)).await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
}

/// Problem 5 and problem 6: a private page is a 404 for an anonymous caller and
/// a 200 for the site owner and a site viewer.
#[tokio::test]
async fn private_page_is_404_for_anonymous_and_200_for_the_owner_and_viewer() {
    let (pool, vis) = visibility_pool().await;
    let app = router(pool);
    let path = "/s/vis-pub/page/vis-pub-private";

    let anonymous = send(app.clone(), get(path, false)).await;
    assert_eq!(
        anonymous.status,
        StatusCode::NOT_FOUND,
        "{}",
        anonymous.body
    );

    let owner = login_cookie(app.clone(), vis.owner_email, VISIBILITY_PASSWORD).await;
    let answer = send(app.clone(), get_cookie(path, &owner)).await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);

    let viewer = login_cookie(app.clone(), vis.viewer_email, VISIBILITY_PASSWORD).await;
    let answer = send(app, get_cookie(path, &viewer)).await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
}

/// Problem 2: a private page linked in the navigation is not rendered for an
/// anonymous visitor, and is rendered for the owner.
#[tokio::test]
async fn navigation_omits_a_private_page_for_anonymous_and_links_it_for_the_owner() {
    let (pool, vis) = visibility_pool().await;
    let app = router(pool);
    let path = "/s/vis-pub/page/vis-pub-public";
    let link = "/s/vis-pub/page/vis-pub-private";

    let anonymous = send(app.clone(), get(path, false)).await;
    assert_eq!(anonymous.status, StatusCode::OK);
    assert!(
        !anonymous.body.contains(link),
        "anonymous nav linked the private page: {}",
        anonymous.body
    );

    let owner = login_cookie(app.clone(), vis.owner_email, VISIBILITY_PASSWORD).await;
    let answer = send(app, get_cookie(path, &owner)).await;
    assert!(
        answer.body.contains(link),
        "owner nav omitted the private page: {}",
        answer.body
    );
}

/// Problem 3: the prev/next pagination follows navigation order, not page id
/// order, and a hidden page is not a neighbour. The private page is created
/// before the attach page, so id order would link the attach page where
/// navigation order links the private page.
#[tokio::test]
async fn pagination_follows_navigation_order_and_skips_a_hidden_page() {
    let (pool, vis) = visibility_pool().await;
    let app = router(pool);
    let path = "/s/vis-pub/page/vis-pub-public";

    let anonymous = send(app.clone(), get(path, false)).await;
    let anon_next = "/s/vis-pub/page/vis-attach";
    assert!(
        anonymous.body.contains(anon_next),
        "anonymous next was not the attach page: {}",
        anonymous.body
    );
    let hidden = "/s/vis-pub/page/vis-pub-private";
    assert!(!anonymous.body.contains(hidden), "{}", anonymous.body);

    let owner = login_cookie(app.clone(), vis.owner_email, VISIBILITY_PASSWORD).await;
    let answer = send(app, get_cookie(path, &owner)).await;
    assert!(
        answer.body.contains(hidden),
        "owner next was not the navigation neighbour: {}",
        answer.body
    );
}

/// Problem 4: the item-with-metadata block does not carry a private media title
/// or thumbnail to an anonymous caller, and does to the owner.
#[tokio::test]
async fn item_media_list_hides_a_private_media_from_anonymous_and_shows_it_to_the_owner() {
    let (pool, vis) = visibility_pool().await;
    let app = router(pool);
    let path = "/s/vis-pub/page/vis-attach";

    let anonymous = send(app.clone(), get(path, false)).await;
    assert_eq!(anonymous.status, StatusCode::OK);
    assert!(
        anonymous.body.contains("Vis media public"),
        "{}",
        anonymous.body
    );
    assert!(
        !anonymous.body.contains("Vis media private"),
        "anonymous saw the private media title: {}",
        anonymous.body
    );
    assert!(
        !anonymous.body.contains("ffffffff"),
        "anonymous saw the private media thumbnail: {}",
        anonymous.body
    );

    let owner = login_cookie(app.clone(), vis.owner_email, VISIBILITY_PASSWORD).await;
    let answer = send(app, get_cookie(path, &owner)).await;
    assert!(
        answer.body.contains("Vis media private"),
        "owner did not see the private media: {}",
        answer.body
    );
}

/// Problem 5 and problem 6: a private item is a 404 for an anonymous caller and
/// a 200 for its owner.
#[tokio::test]
async fn private_item_is_404_for_anonymous_and_200_for_its_owner() {
    let (pool, vis) = visibility_pool().await;
    let app = router(pool);
    let path = format!("/s/vis-pub/item/{}", vis.private_item.0);

    let anonymous = send(app.clone(), get(&path, false)).await;
    assert_eq!(
        anonymous.status,
        StatusCode::NOT_FOUND,
        "{}",
        anonymous.body
    );

    let owner = login_cookie(app.clone(), vis.owner_email, VISIBILITY_PASSWORD).await;
    let answer = send(app, get_cookie(&path, &owner)).await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
}

/// `item-with-metadata.phtml` lists `$item->itemSets()`, which loads through
/// `ResourceVisibilityFilter`: measured on the oracle, an anonymous reader of
/// a public item in a private and a public set sees only the public set. The
/// set's owner sees both.
#[tokio::test]
async fn item_page_lists_only_the_item_sets_the_reader_can_see() {
    let (pool, vis) = visibility_pool().await;
    let app = router(pool);
    let keyed = |path: &str| {
        format!("{path}?key_identity={OWNER_KEY_IDENTITY}&key_credential={OWNER_KEY_CREDENTIAL}")
    };
    let mut sets = Vec::new();
    for (title, is_public) in [("Vis set private", false), ("Vis set public", true)] {
        let body = serde_json::json!({
            "o:is_public": is_public,
            "dcterms:title": [{"property_id": "auto", "type": "literal", "@value": title}],
        });
        let answer = send(
            app.clone(),
            json_request("POST", &keyed("/api/item_sets"), &body.to_string()),
        )
        .await;
        assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
        sets.push(serde_json::json!({"o:id": answer.json()["o:id"]}));
    }
    let linked = send(
        app.clone(),
        json_request(
            "PATCH",
            &keyed(&format!("/api/items/{}", vis.item.0)),
            &serde_json::json!({"o:item_set": sets}).to_string(),
        ),
    )
    .await;
    assert_eq!(linked.status, StatusCode::OK, "{}", linked.body);
    let path = format!("/s/vis-pub/item/{}", vis.item.0);

    let anonymous = send(app.clone(), get(&path, false)).await;
    assert_eq!(anonymous.status, StatusCode::OK, "{}", anonymous.body);
    assert!(
        anonymous.body.contains("Vis set public"),
        "{}",
        anonymous.body
    );
    assert!(
        !anonymous.body.contains("Vis set private"),
        "anonymous saw the private item set: {}",
        anonymous.body
    );

    let owner = login_cookie(app.clone(), vis.owner_email, VISIBILITY_PASSWORD).await;
    let answer = send(app, get_cookie(&path, &owner)).await;
    assert!(
        answer.body.contains("Vis set private"),
        "the owner did not see the private item set: {}",
        answer.body
    );
}

/// The `o:page` ids of a `Site` document.
fn page_ids(answer: &common::Answer) -> Vec<u64> {
    answer.json()["o:page"]
        .as_array()
        .unwrap()
        .iter()
        .map(|page| page["o:id"].as_u64().unwrap())
        .collect()
}
