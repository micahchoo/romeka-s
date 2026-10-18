//! `POST /s/:site-slug/preview`: what the editor's live preview renders, what
//! it refuses, and what it leaves alone.
//!
//! Every test drives the router over HTTP against a migrated in-memory
//! database, so it asserts what the admin SPA sees. The row-count tests are the
//! write guarantee: a preview of an edit that would replace every block leaves
//! `site_page`, `site_page_block` and `site_block_attachment` untouched.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::{
    Answer, OWNER_KEY_CREDENTIAL, OWNER_KEY_IDENTITY, VIEWER_KEY_CREDENTIAL, VIEWER_KEY_IDENTITY,
    VISIBILITY_PASSWORD, header, json_request, login_cookie, router, send, visibility_pool,
};
use omeka_db::ids::PageId;
use omeka_db::repos::SiteRepo;
use omeka_db::rows::NewSiteBlock;
use serde_json::{Value, json};
use sqlx::SqlitePool;

/// A preview POST carrying one key pair in the query string, the way the admin
/// SPA attaches its key to an `/api` URL.
fn keyed_preview(uri: &str, identity: &str, credential: &str, body: &Value) -> Request<Body> {
    let separator = if uri.contains('?') { '&' } else { '?' };
    json_request(
        "POST",
        &format!("{uri}{separator}key_identity={identity}&key_credential={credential}"),
        &body.to_string(),
    )
}

/// The body a `PUT /api/site_pages/:id` takes, plus `o:id` for an edit.
fn page_body(id: Option<u64>, slug: &str, title: &str, blocks: Value) -> Value {
    let mut body = json!({
        "o:title": title,
        "o:slug": slug,
        "o:is_public": true,
        "o:layout": "",
        "o:layout_data": {},
        "o:block": blocks,
    });
    if let Some(id) = id {
        body["o:id"] = json!(id);
    }
    body
}

fn errors(answer: &Answer) -> Value {
    answer.json().get("errors").cloned().unwrap_or(Value::Null)
}

/// The three tables a page write touches.
async fn counts(pool: &SqlitePool) -> (i64, i64, i64) {
    let mut out = [0_i64; 3];
    for (index, table) in ["site_page", "site_page_block", "site_block_attachment"]
        .into_iter()
        .enumerate()
    {
        out[index] = sqlx::query_scalar(&format!("SELECT count(*) FROM {table}"))
            .fetch_one(pool)
            .await
            .unwrap();
    }
    (out[0], out[1], out[2])
}

async fn add_stored_html_block(pool: &SqlitePool, page: u64, html: &str) {
    SiteRepo::new(pool)
        .add_block(&NewSiteBlock {
            page_id: PageId::from(page),
            layout: "html".to_string(),
            data: json!({ "html": html }),
            layout_data: None,
            position: 1,
        })
        .await
        .unwrap();
}

/// The unsaved body renders in place of the stored one, and the page grid
/// applies.
#[tokio::test]
async fn preview_renders_unsaved_blocks_and_the_page_grid() {
    let (pool, seed) = visibility_pool().await;
    add_stored_html_block(&pool, seed.public_page, "<p>Stored copy</p>").await;
    let app = router(pool);

    let body = json!({
        "o:id": seed.public_page,
        "o:title": "Vis public page",
        "o:slug": "vis-pub-public",
        "o:is_public": true,
        "o:layout": "grid",
        "o:layout_data": {"grid_columns": 3, "grid_column_gap": 7, "grid_row_gap": 9},
        "o:block": [
            {"o:layout": "html", "o:data": {"html": "<p>Unsaved copy</p>"},
             "o:layout_data": {"grid_column_span": "2"}},
            {"o:layout": "lineBreak", "o:data": {"break_type": "opaque"}}
        ]
    });
    let answer = send(
        app,
        keyed_preview(
            "/s/vis-pub/preview",
            OWNER_KEY_IDENTITY,
            OWNER_KEY_CREDENTIAL,
            &body,
        ),
    )
    .await;

    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    assert_eq!(
        header(&answer, "content-type"),
        Some("text/html; charset=utf-8")
    );
    assert!(
        answer.body.contains("<p>Unsaved copy</p>"),
        "the edited block is missing"
    );
    assert!(
        !answer.body.contains("Stored copy"),
        "the removed block still renders"
    );
    assert!(
        answer.body.contains("page-layout-grid")
            && answer.body.contains("grid-template-columns-3")
            && answer.body.contains("column-gap: 7px;"),
        "the grid layout did not apply"
    );
    assert!(
        answer.body.contains("grid-span-2"),
        "the block span did not apply"
    );
    assert!(
        answer.body.contains("<h1 class=\"site-title\"><a href=")
            && answer.body.contains(">Vis public</a></h1>"),
        "the site frame is missing"
    );
    assert_eq!(header(&answer, "cache-control"), Some("no-store"));
    assert_eq!(header(&answer, "x-robots-tag"), Some("noindex"));
    let csp = header(&answer, "content-security-policy").unwrap_or("");
    assert!(csp.contains("script-src 'none'"), "csp is {csp:?}");
    assert!(csp.contains("frame-ancestors 'self'"), "csp is {csp:?}");
}

/// A preview of an edit that would replace every block writes nothing, and the
/// stored page keeps rendering its stored content.
#[tokio::test]
async fn preview_writes_nothing() {
    let (pool, seed) = visibility_pool().await;
    add_stored_html_block(&pool, seed.public_page, "<p>Stored copy</p>").await;
    let app = router(pool.clone());
    let before = counts(&pool).await;

    let body = page_body(
        Some(seed.public_page),
        "vis-pub-public",
        "Vis public page",
        json!([
            {"o:layout": "html", "o:data": {"html": "<p>Unsaved copy</p>"}}
        ]),
    );
    let answer = send(
        app.clone(),
        keyed_preview(
            "/s/vis-pub/preview",
            OWNER_KEY_IDENTITY,
            OWNER_KEY_CREDENTIAL,
            &body,
        ),
    )
    .await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);

    assert_eq!(counts(&pool).await, before, "the preview wrote rows");

    let stored = SiteRepo::new(&pool)
        .list_blocks(omeka_db::ids::PageId::from(seed.public_page))
        .await
        .unwrap();
    assert_eq!(stored.len(), 1, "the stored blocks changed");
    assert_eq!(stored[0].data, json!({"html": "<p>Stored copy</p>"}));

    let page = send(app, common::get("/s/vis-pub/page/vis-pub-public", false)).await;
    assert_eq!(page.status, StatusCode::OK);
    assert!(
        page.body.contains("Stored copy"),
        "the stored page lost its block"
    );
    assert!(!page.body.contains("Unsaved copy"));
}

/// A new page previews without being created, authorized as an add-page.
#[tokio::test]
async fn preview_of_a_new_page_creates_nothing() {
    let (pool, seed) = visibility_pool().await;
    let app = router(pool.clone());
    let before = counts(&pool).await;

    let body = page_body(
        None,
        "draft-page",
        "Draft page",
        json!([{"o:layout": "pageTitle", "o:data": {}}]),
    );
    let answer = send(
        app,
        keyed_preview(
            "/s/vis-pub/preview",
            OWNER_KEY_IDENTITY,
            OWNER_KEY_CREDENTIAL,
            &body,
        ),
    )
    .await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    assert!(
        answer.body.contains("Draft page"),
        "the draft title is missing"
    );
    assert_eq!(counts(&pool).await, before, "the preview wrote rows");

    let pages = SiteRepo::new(&pool)
        .list_pages(omeka_db::ids::SiteId::from(seed.public_site))
        .await
        .unwrap();
    assert!(
        pages.iter().all(|page| page.slug != "draft-page"),
        "the preview created the page"
    );
}

/// A body with no `o:block` leaves the blocks alone, the way the update does,
/// so the preview renders the stored blocks.
#[tokio::test]
async fn preview_without_o_block_renders_the_stored_blocks() {
    let (pool, seed) = visibility_pool().await;
    add_stored_html_block(&pool, seed.public_page, "<p>Stored copy</p>").await;
    let app = router(pool);

    let body = json!({
        "o:id": seed.public_page,
        "o:title": "Vis public page",
        "o:slug": "vis-pub-public",
        "o:is_public": true,
    });
    let answer = send(
        app,
        keyed_preview(
            "/s/vis-pub/preview",
            OWNER_KEY_IDENTITY,
            OWNER_KEY_CREDENTIAL,
            &body,
        ),
    )
    .await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    assert!(answer.body.contains("Stored copy"), "{}", answer.body);
}

/// A site viewer and an anonymous caller may not preview: both are 403, the
/// status the page write answers.
#[tokio::test]
async fn preview_refuses_a_viewer_and_an_anonymous_caller() {
    let (pool, seed) = visibility_pool().await;
    let app = router(pool);
    let body = page_body(
        Some(seed.public_page),
        "vis-pub-public",
        "Vis public page",
        json!([{"o:layout": "pageTitle", "o:data": {}}]),
    );

    let viewer = send(
        app.clone(),
        keyed_preview(
            "/s/vis-pub/preview",
            VIEWER_KEY_IDENTITY,
            VIEWER_KEY_CREDENTIAL,
            &body,
        ),
    )
    .await;
    assert_eq!(viewer.status, StatusCode::FORBIDDEN, "{}", viewer.body);
    assert_eq!(
        errors(&viewer),
        json!({"error":
            "Permission denied for the current user to update the \
             Omeka\\Entity\\SitePage resource."
        })
    );

    let anonymous = send(
        app,
        json_request("POST", "/s/vis-pub/preview", &body.to_string()),
    )
    .await;
    assert_eq!(
        anonymous.status,
        StatusCode::FORBIDDEN,
        "{}",
        anonymous.body
    );
    assert_eq!(
        errors(&anonymous),
        json!({"error":
            "Permission denied for the current user to update the \
             Omeka\\Entity\\SitePage resource."
        })
    );
}

/// A private site does not turn an anonymous preview into a 404; the access
/// check is the answer, as it is for the write.
#[tokio::test]
async fn preview_of_a_private_site_is_forbidden_not_hidden() {
    let (pool, seed) = visibility_pool().await;
    let app = router(pool);
    let body = page_body(
        Some(seed.private_site_public_page),
        "vis-priv-public",
        "Vis private site page",
        json!([{"o:layout": "pageTitle", "o:data": {}}]),
    );
    let anonymous = send(
        app,
        json_request("POST", "/s/vis-priv/preview", &body.to_string()),
    )
    .await;
    assert_eq!(
        anonymous.status,
        StatusCode::FORBIDDEN,
        "{}",
        anonymous.body
    );
}

/// An unknown site slug is the API's 404 document.
#[tokio::test]
async fn preview_unknown_site_is_404() {
    let (pool, _seed) = visibility_pool().await;
    let app = router(pool);
    let body = page_body(None, "draft-page", "Draft page", json!([]));
    let answer = send(
        app,
        keyed_preview(
            "/s/no-such-site/preview",
            OWNER_KEY_IDENTITY,
            OWNER_KEY_CREDENTIAL,
            &body,
        ),
    )
    .await;
    assert_eq!(answer.status, StatusCode::NOT_FOUND, "{}", answer.body);
    assert_eq!(errors(&answer), json!({"error": "Not Found"}));
}

/// A block the write refuses is a 422 with the write's error document.
#[tokio::test]
async fn preview_refuses_invalid_blocks() {
    let (pool, seed) = visibility_pool().await;
    let app = router(pool);

    let scalar = page_body(
        Some(seed.public_page),
        "vis-pub-public",
        "Vis public page",
        json!([{"o:layout": "html", "o:data": "not an object"}]),
    );
    let answer = send(
        app.clone(),
        keyed_preview(
            "/s/vis-pub/preview",
            OWNER_KEY_IDENTITY,
            OWNER_KEY_CREDENTIAL,
            &scalar,
        ),
    )
    .await;
    assert_eq!(
        answer.status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "{}",
        answer.body
    );
    assert_eq!(
        errors(&answer),
        json!({"o:block": ["Block data must not be a scalar value."]})
    );

    let no_layout = page_body(
        Some(seed.public_page),
        "vis-pub-public",
        "Vis public page",
        json!([{"o:data": {"html": "<p>x</p>"}}]),
    );
    let answer = send(
        app,
        keyed_preview(
            "/s/vis-pub/preview",
            OWNER_KEY_IDENTITY,
            OWNER_KEY_CREDENTIAL,
            &no_layout,
        ),
    )
    .await;
    assert_eq!(
        answer.status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "{}",
        answer.body
    );
    assert_eq!(
        errors(&answer),
        json!({"o:block": ["All blocks must have a layout."]})
    );
}

/// A slug another page of the site holds is the write's 422.
#[tokio::test]
async fn preview_refuses_a_taken_slug() {
    let (pool, seed) = visibility_pool().await;
    let app = router(pool);
    let body = page_body(
        Some(seed.public_page),
        "vis-attach",
        "Vis public page",
        json!([]),
    );
    let answer = send(
        app,
        keyed_preview(
            "/s/vis-pub/preview",
            OWNER_KEY_IDENTITY,
            OWNER_KEY_CREDENTIAL,
            &body,
        ),
    )
    .await;
    assert_eq!(
        answer.status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "{}",
        answer.body
    );
    assert_eq!(
        errors(&answer),
        json!({"o:slug": ["The slug \"vis-attach\" is already taken."]})
    );
}

/// A `<script>` in an html block reaches the document purified, because the
/// preview runs the write's purifier.
#[tokio::test]
async fn preview_purifies_an_html_block() {
    let (pool, seed) = visibility_pool().await;
    let app = router(pool);
    let body = page_body(
        Some(seed.public_page),
        "vis-pub-public",
        "Vis public page",
        json!([{
            "o:layout": "html",
            "o:data": {"html": "<p>Safe</p><script>alert('x')</script><a href=\"/ok\" onclick=\"boom()\">link</a>"}
        }]),
    );
    let answer = send(
        app,
        keyed_preview(
            "/s/vis-pub/preview",
            OWNER_KEY_IDENTITY,
            OWNER_KEY_CREDENTIAL,
            &body,
        ),
    )
    .await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    assert!(answer.body.contains("<p>Safe</p>"));
    assert!(
        !answer.body.contains("<script>alert('x')</script>"),
        "the script survived"
    );
    assert!(!answer.body.contains("onclick"), "the handler survived");
    assert!(
        answer.body.contains("href=\"/ok\""),
        "the safe link is gone"
    );
}

/// The existing same-origin guard covers a cookie-authenticated preview: a
/// foreign origin is refused before the route runs, and the same origin passes.
#[tokio::test]
async fn preview_requires_the_same_origin_for_a_cookie_post() {
    let (pool, seed) = visibility_pool().await;
    let app = router(pool);
    let cookie = login_cookie(app.clone(), "vis-owner@example.com", VISIBILITY_PASSWORD).await;
    let body = page_body(
        Some(seed.public_page),
        "vis-pub-public",
        "Vis public page",
        json!([{"o:layout": "pageTitle", "o:data": {}}]),
    )
    .to_string();

    let foreign = Request::builder()
        .method("POST")
        .uri("/s/vis-pub/preview")
        .header("content-type", "application/json")
        .header("cookie", &cookie)
        .header("origin", "http://evil.example")
        .body(Body::from(body.clone()))
        .unwrap();
    let answer = send(app.clone(), foreign).await;
    assert_eq!(answer.status, StatusCode::FORBIDDEN, "{}", answer.body);
    assert_eq!(
        errors(&answer),
        json!({"error":
            "A session-authenticated write must come from the configured base URL."
        })
    );

    let same_origin = Request::builder()
        .method("POST")
        .uri("/s/vis-pub/preview")
        .header("content-type", "application/json")
        .header("cookie", &cookie)
        .header("origin", "http://localhost")
        .body(Body::from(body))
        .unwrap();
    let answer = send(app, same_origin).await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    assert!(answer.body.contains("Vis public page"));
}

/// The preview path is a `POST`; the same path as a `GET` stays the public
/// route it was, with no controller behind it.
#[tokio::test]
async fn preview_answers_only_to_post() {
    let (pool, _seed) = visibility_pool().await;
    let app = router(pool);
    let answer = send(app, common::get("/s/vis-pub/preview", false)).await;
    assert_eq!(answer.status, StatusCode::NOT_FOUND);
    assert_eq!(
        header(&answer, "content-type"),
        Some("text/html; charset=utf-8")
    );
}

/// A hero block in the preview renders the rich banner. The preview runs the
/// write's carrier step, so the plain rendering is generated and hashed before
/// the render, exactly as a save would store it. Without that step the block
/// would carry no hash and render its empty stored html.
#[tokio::test]
async fn preview_renders_the_romeka_hero() {
    let (pool, seed) = visibility_pool().await;
    let app = router(pool);

    let body = page_body(
        Some(seed.public_page),
        "vis-pub-public",
        "Vis public page",
        json!([{
            "o:layout": "html",
            "o:data": {},
            "o:layout_data": {
                "romeka": {
                    "block": "hero",
                    "v": 1,
                    "settings": {
                        "heading": "Preview heading",
                        "subheading": "Preview subheading",
                        "cta_label": "Go",
                        "cta_url": "https://example.org/go"
                    }
                }
            }
        }]),
    );
    let answer = send(
        app,
        keyed_preview(
            "/s/vis-pub/preview",
            OWNER_KEY_IDENTITY,
            OWNER_KEY_CREDENTIAL,
            &body,
        ),
    )
    .await;

    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    assert!(
        answer
            .body
            .contains("class=\"romeka-hero romeka-hero-align-left romeka-hero-overlay-medium"),
        "{}",
        answer.body
    );
    assert!(
        answer
            .body
            .contains("<h2 class=\"romeka-hero-heading\">Preview heading</h2>"),
        "{}",
        answer.body
    );
    assert!(
        answer
            .body
            .contains("<a href=\"https://example.org/go\" class=\"romeka-hero-cta\">Go</a>"),
        "{}",
        answer.body
    );
}

/// A preview carrying `o:theme_settings` renders with the body's design tokens
/// and stores nothing: the site keeps its own map, and the public page renders
/// the stored tokens.
#[tokio::test]
async fn preview_renders_the_theme_override_without_storing_it() {
    let (pool, seed) = visibility_pool().await;
    let app = router(pool.clone());
    let before: i64 = sqlx::query_scalar("SELECT count(*) FROM site_setting")
        .fetch_one(&pool)
        .await
        .unwrap();

    let mut body = page_body(
        Some(seed.public_page),
        "vis-pub-public",
        "Vis public page",
        json!([{"o:layout": "pageTitle", "o:data": {}}]),
    );
    body["o:theme_settings"] = json!({
        "brand_color": "#cc3300",
        "heading_font": "geometric",
        "footer_text": "<p>Custom footer</p>",
        "banner_text": "Open daily"
    });
    let answer = send(
        app.clone(),
        keyed_preview(
            "/s/vis-pub/preview",
            OWNER_KEY_IDENTITY,
            OWNER_KEY_CREDENTIAL,
            &body,
        ),
    )
    .await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    assert!(
        answer.body.contains("--color-brand: #cc3300;"),
        "the override did not reach the sheet"
    );
    assert!(
        answer.body.contains("--font-heading: Futura"),
        "the heading font override did not apply"
    );
    assert!(
        answer
            .body
            .contains("<div class=\"footer-text\"><p>Custom footer</p></div>"),
        "the footer override did not render"
    );
    assert!(
        answer
            .body
            .contains("<p class=\"site-banner-text\">Open daily</p>"),
        "the banner override did not render"
    );

    let after: i64 = sqlx::query_scalar("SELECT count(*) FROM site_setting")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(after, before, "the preview stored the override");

    let page = send(app, common::get("/s/vis-pub/page/vis-pub-public", false)).await;
    assert_eq!(page.status, StatusCode::OK);
    assert!(
        page.body.contains("--color-brand: #6aa9d6;"),
        "the public page should keep the default brand"
    );
    assert!(
        !page.body.contains("#cc3300"),
        "the override leaked into the public page"
    );
}

/// A hostile `o:theme_settings` value takes its token's default, so a preview
/// body cannot inject CSS the way a stored value cannot.
#[tokio::test]
async fn preview_theme_override_validates_the_tokens() {
    let (pool, seed) = visibility_pool().await;
    let app = router(pool);

    let mut body = page_body(
        Some(seed.public_page),
        "vis-pub-public",
        "Vis public page",
        json!([{"o:layout": "pageTitle", "o:data": {}}]),
    );
    body["o:theme_settings"] = json!({"brand_color": "}</style><script>alert(1)</script>"});
    let answer = send(
        app,
        keyed_preview(
            "/s/vis-pub/preview",
            OWNER_KEY_IDENTITY,
            OWNER_KEY_CREDENTIAL,
            &body,
        ),
    )
    .await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    assert!(
        answer.body.contains("--color-brand: #6aa9d6;"),
        "the hostile colour did not fall back"
    );
    assert!(
        !answer.body.contains("<script>alert(1)</script>"),
        "the hostile value reached the document"
    );
}
