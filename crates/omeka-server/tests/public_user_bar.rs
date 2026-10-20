//! The public site's user bar end to end: Logout ends the session and returns
//! the reader to the page, the edit links follow the caller's roles, and the
//! resource page blocks read the class and the image files from the database.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use omeka_db::ids::SiteId;
use omeka_db::repos::{SiteRepo, UserRepo};
use omeka_db::rows::NewUser;
use serde_json::json;
use sqlx::SqlitePool;

use common::{Seed, get, get_cookie, header, login_cookie, router, send, site_pool};

const PASSWORD: &str = "user-bar-pass-1";
const WELCOME: &str = "/s/museum/page/welcome";

/// A user with a global role and, when given, a role on the seeded site.
async fn user(pool: &SqlitePool, seed: &Seed, email: &str, role: &str, on_site: Option<&str>) {
    let row = UserRepo::new(pool)
        .create(&NewUser {
            email: email.to_string(),
            name: "Bar Person".to_string(),
            password_hash: Some(omeka_api::password::hash(PASSWORD).unwrap()),
            role: role.to_string(),
            is_active: true,
            created: None,
        })
        .await
        .unwrap();
    if let Some(site_role) = on_site {
        SiteRepo::new(pool)
            .set_permission(SiteId::from(seed.site), row.id, site_role)
            .await
            .unwrap();
    }
}

/// The Logout form's submission: a POST with the session cookie and the
/// `Origin` a browser sends.
fn logout(redirect: &str, cookie: &str, origin: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(format!("/logout?redirect={redirect}"))
        .header("cookie", cookie)
        .header("origin", origin)
        .body(Body::empty())
        .unwrap()
}

/// Whether the cookie still names a live session.
async fn signed_in(app: &Router, cookie: &str) -> bool {
    send(app.clone(), get_cookie("/api/me", cookie))
        .await
        .status
        == StatusCode::OK
}

/// The welcome page's id, for the editor route.
async fn welcome_id(pool: &SqlitePool, seed: &Seed) -> u64 {
    SiteRepo::new(pool)
        .list_pages(SiteId::from(seed.site))
        .await
        .unwrap()
        .into_iter()
        .find(|page| page.slug == "welcome")
        .expect("the welcome page")
        .id
        .0
}

#[tokio::test]
async fn logout_ends_the_session_and_returns_to_the_page() {
    let (pool, seed) = site_pool().await;
    user(&pool, &seed, "editor@example.com", "editor", None).await;
    let app = router(pool);
    let cookie = login_cookie(app.clone(), "editor@example.com", PASSWORD).await;
    let before = send(app.clone(), get_cookie(WELCOME, &cookie)).await;
    assert!(before.body.contains("class=\"logout\""), "{}", before.body);

    let answer = send(
        app.clone(),
        logout(
            "%2Fs%2Fmuseum%2Fpage%2Fwelcome",
            &cookie,
            "http://localhost",
        ),
    )
    .await;
    assert_eq!(answer.status, StatusCode::SEE_OTHER, "{}", answer.body);
    assert_eq!(header(&answer, "location"), Some(WELCOME));
    let cleared = header(&answer, "set-cookie").expect("the cookie is cleared");
    assert!(cleared.contains("Max-Age=0"), "{cleared}");

    assert!(!signed_in(&app, &cookie).await, "the session row is gone");
    let after = send(app, get_cookie(WELCOME, &cookie)).await;
    assert!(!after.body.contains("id=\"user-bar\""), "{}", after.body);
}

#[tokio::test]
async fn logout_returns_only_to_a_path_on_this_site() {
    let (pool, seed) = site_pool().await;
    user(&pool, &seed, "editor@example.com", "editor", None).await;
    let app = router(pool);
    for redirect in [
        "https%3A%2F%2Fevil.example%2F",
        "%2F%2Fevil.example%2Fpath",
        "%2F%5Cevil.example",
        "evil.example",
        "",
    ] {
        let cookie = login_cookie(app.clone(), "editor@example.com", PASSWORD).await;
        let answer = send(app.clone(), logout(redirect, &cookie, "http://localhost")).await;
        assert_eq!(answer.status, StatusCode::SEE_OTHER, "{redirect}");
        assert_eq!(header(&answer, "location"), Some("/"), "{redirect}");
        assert!(!signed_in(&app, &cookie).await, "{redirect}");
    }
}

#[tokio::test]
async fn a_logout_posted_from_another_origin_keeps_the_session() {
    let (pool, seed) = site_pool().await;
    user(&pool, &seed, "editor@example.com", "editor", None).await;
    let app = router(pool);
    let cookie = login_cookie(app.clone(), "editor@example.com", PASSWORD).await;

    let answer = send(
        app.clone(),
        logout("%2Fs%2Fmuseum", &cookie, "https://evil.example"),
    )
    .await;
    assert_eq!(answer.status, StatusCode::FORBIDDEN, "{}", answer.body);
    assert!(signed_in(&app, &cookie).await);
}

#[tokio::test]
async fn edit_this_page_shows_only_to_callers_who_may_edit_the_page() {
    let (pool, seed) = site_pool().await;
    user(
        &pool,
        &seed,
        "site-editor@example.com",
        "researcher",
        Some("editor"),
    )
    .await;
    user(
        &pool,
        &seed,
        "site-viewer@example.com",
        "researcher",
        Some("viewer"),
    )
    .await;
    let link = format!(
        "<a href=\"/#/sites/{}/pages/{}\" class=\"admin-edit\">Edit this page</a>",
        seed.site,
        welcome_id(&pool, &seed).await
    );
    let app = router(pool.clone());

    let editor = login_cookie(app.clone(), "site-editor@example.com", PASSWORD).await;
    let page = send(app.clone(), get_cookie(WELCOME, &editor)).await;
    assert!(page.body.contains(&link), "{}", page.body);

    let viewer = login_cookie(app.clone(), "site-viewer@example.com", PASSWORD).await;
    let page = send(app.clone(), get_cookie(WELCOME, &viewer)).await;
    assert!(page.body.contains("Admin dashboard"), "{}", page.body);
    assert!(!page.body.contains("admin-edit"), "{}", page.body);

    omeka_db::repos::SiteSettingRepo::new(&pool)
        .set(SiteId::from(seed.site), "show_user_bar", &json!(1))
        .await
        .unwrap();
    let anonymous = send(app, get(WELCOME, false)).await;
    assert!(
        anonymous.body.contains("class=\"login\""),
        "{}",
        anonymous.body
    );
    assert!(!anonymous.body.contains("admin-edit"), "{}", anonymous.body);
}

#[tokio::test]
async fn an_item_page_links_its_admin_page_for_callers_who_may_edit_it() {
    let (pool, seed) = site_pool().await;
    user(&pool, &seed, "editor@example.com", "editor", None).await;
    user(&pool, &seed, "author@example.com", "author", Some("admin")).await;
    let app = router(pool);
    let path = format!("/s/museum/item/{}", seed.item.0);
    let link = format!(
        "<a href=\"/#/items/{}\" class=\"admin-edit\">Edit item</a>",
        seed.item.0
    );

    let editor = login_cookie(app.clone(), "editor@example.com", PASSWORD).await;
    let page = send(app.clone(), get_cookie(&path, &editor)).await;
    assert!(page.body.contains(&link), "{}", page.body);

    // The seeded item belongs to the admin; an author edits only their own.
    let author = login_cookie(app.clone(), "author@example.com", PASSWORD).await;
    let page = send(app, get_cookie(&path, &author)).await;
    assert!(page.body.contains("id=\"user-bar\""), "{}", page.body);
    assert!(!page.body.contains("admin-edit"), "{}", page.body);
}

#[tokio::test]
async fn the_item_page_shows_its_class_and_a_gallery_of_its_images() {
    let (pool, seed) = site_pool().await;
    let db = omeka_db::DbPool::from(pool.clone());
    omeka_api::theme_settings::store(
        &db,
        SiteId::from(seed.site),
        "default",
        Some(&json!({
            "resource_page_blocks": {
                "items": { "main": ["resourceClass", "lightboxGallery"] }
            }
        })),
    )
    .await
    .unwrap();
    let app = router(pool);

    let page = send(app, get(&format!("/s/museum/item/{}", seed.item.0), false)).await;
    assert_eq!(page.status, StatusCode::OK, "{}", page.body);
    assert!(
        page.body.contains(
            "<dl class=\"resource-class\"><div class=\"property\"><dt>Resource class</dt>\
             <dd class=\"value\">Image</dd></div></dl>"
        ),
        "{}",
        page.body
    );
    let gallery = page
        .body
        .split("<div class=\"lightbox-gallery\">")
        .nth(1)
        .expect("a gallery");
    assert!(
        gallery.starts_with(
            "<a href=\"http://localhost/files/original/4b1e5b9e0f9f4f5a8f6b2c3d4e5f6a7b8c9d0e1f.jpg\" \
             class=\"resource-link\"><img src=\"http://localhost/files/square/"
        ),
        "{gallery}"
    );
}
