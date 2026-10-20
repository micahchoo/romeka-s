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
    Answer, KEY_CREDENTIAL, KEY_IDENTITY, OWNER_KEY_CREDENTIAL, OWNER_KEY_IDENTITY,
    VIEWER_KEY_CREDENTIAL, VIEWER_KEY_IDENTITY, VISIBILITY_PASSWORD, get, header, json_request,
    login_cookie, router, send, site_pool, visibility_pool,
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

/// The preview frame is the page as a visitor sees it, so the user bar the
/// signed-in owner gets on the stored page is left out of the preview.
#[tokio::test]
async fn preview_renders_without_the_user_bar() {
    let (pool, seed) = visibility_pool().await;
    let app = router(pool);
    let cookie = login_cookie(app.clone(), "vis-owner@example.com", VISIBILITY_PASSWORD).await;
    let stored = send(
        app.clone(),
        Request::builder()
            .method("GET")
            .uri("/s/vis-pub/page/vis-pub-public")
            .header("cookie", &cookie)
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(stored.status, StatusCode::OK, "{}", stored.body);
    assert!(
        stored.body.contains("id=\"user-bar\""),
        "the stored page shows the signed-in caller the user bar"
    );

    let body = page_body(
        Some(seed.public_page),
        "vis-pub-public",
        "Vis public page",
        json!([{"o:layout": "html", "o:data": {"html": "<p>Draft</p>"}}]),
    );
    let preview = send(
        app,
        keyed_preview(
            "/s/vis-pub/preview",
            OWNER_KEY_IDENTITY,
            OWNER_KEY_CREDENTIAL,
            &body,
        ),
    )
    .await;
    assert_eq!(preview.status, StatusCode::OK, "{}", preview.body);
    assert!(preview.body.contains("<p>Draft</p>"), "{}", preview.body);
    assert!(
        !preview.body.contains("id=\"user-bar\""),
        "the preview carries the user bar"
    );
}

/// Draft custom CSS reaches the preview after the theme sheet, and CSS the
/// sanitiser refuses is a 422 naming the line, the answer the site write gives.
#[tokio::test]
async fn preview_applies_draft_custom_css_and_refuses_unsafe_css() {
    let (pool, seed) = visibility_pool().await;
    let app = router(pool);
    let mut body = page_body(
        Some(seed.public_page),
        "vis-pub-public",
        "Vis public page",
        json!([{"o:layout": "pageTitle", "o:data": {}}]),
    );
    body["o:theme_settings"] = json!({"custom_css": "body { color: #cc0000 }"});
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
        answer
            .body
            .contains("<style id=\"custom-css\">\nbody { color: #cc0000 }\n</style>"),
        "{}",
        answer.body
    );

    body["o:theme_settings"] =
        json!({"custom_css": "body {\n  background: url(https://evil.example/x)\n}"});
    let refused = send(
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
        refused.status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "{}",
        refused.body
    );
    let message = refused.json()["errors"]["o:theme_settings"][0]
        .as_str()
        .unwrap_or_default()
        .to_string();
    assert!(
        message.starts_with("Custom CSS: Line 2, column 15: url("),
        "{message}"
    );
}

/// A new page before the curator types anything, as the editor sends it: no
/// title, no slug, no blocks.
fn blank_new_page() -> Value {
    page_body(None, "", "", json!([]))
}

async fn preview_as_owner(app: axum::Router, body: &Value) -> Answer {
    send(
        app,
        keyed_preview(
            "/s/vis-pub/preview",
            OWNER_KEY_IDENTITY,
            OWNER_KEY_CREDENTIAL,
            body,
        ),
    )
    .await
}

/// A draft with no title or slug yet previews under the site's own word for a
/// resource without a title, and nothing is written. Only the save needs them.
#[tokio::test]
async fn preview_of_a_blank_new_page_renders_a_placeholder_title() {
    let (pool, _seed) = visibility_pool().await;
    let before = counts(&pool).await;

    let answer = preview_as_owner(router(pool.clone()), &blank_new_page()).await;

    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    // The new page's page-title block shows it, as the saved page would.
    assert!(
        answer.body.contains("<h2>[Untitled]</h2>"),
        "the placeholder title is missing: {}",
        answer.body
    );
    assert_eq!(counts(&pool).await, before, "the preview wrote rows");
}

/// The placeholder is the site locale's, as every other word of the render is.
#[tokio::test]
async fn preview_placeholder_title_follows_the_site_locale() {
    let (pool, seed) = visibility_pool().await;
    omeka_db::repos::SiteSettingRepo::new(&pool)
        .set(
            omeka_db::ids::SiteId::from(seed.public_site),
            "locale",
            &json!("es"),
        )
        .await
        .unwrap();

    let answer = preview_as_owner(router(pool), &blank_new_page()).await;

    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    assert!(
        answer.body.contains("<h2>[Sin título]</h2>"),
        "the Spanish placeholder is missing: {}",
        answer.body
    );
}

/// Clearing the title and slug of a saved page still previews.
#[tokio::test]
async fn preview_of_an_edit_with_a_cleared_title_and_slug_renders() {
    let (pool, seed) = visibility_pool().await;
    let body = page_body(
        Some(seed.public_page),
        "",
        "",
        json!([{"o:layout": "html", "o:data": {"html": "<p>Still here</p>"}}]),
    );

    let answer = preview_as_owner(router(pool), &body).await;

    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    assert!(answer.body.contains("<p>Still here</p>"), "{}", answer.body);
    assert!(answer.body.contains("[Untitled]"), "{}", answer.body);
}

/// The save of the same blank body is refused as before: the preview's
/// placeholder never reaches a write.
#[tokio::test]
async fn saving_a_blank_new_page_still_needs_a_title_and_slug() {
    let (pool, seed) = visibility_pool().await;
    let mut body = blank_new_page();
    body["o:site"] = json!({"o:id": seed.public_site});

    let answer = send(
        router(pool),
        keyed_preview(
            "/api/site_pages",
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
    let errors = errors(&answer);
    assert_eq!(errors["o:title"], json!(["A page must have a title."]));
    assert!(errors.get("o:slug").is_some(), "{errors}");
}

/// The body that previews one resource page under a draft section order.
fn resource_body(kind: &str, id: u64, sections: &[&str]) -> Value {
    json!({
        "resource_page": {"kind": kind, "id": id},
        "o:theme_settings": {"resource_page_blocks": {
            "items": {"main": sections},
            "item_sets": {"main": ["values"]},
            "media": {"main": ["mediaRender", "values"]}
        }}
    })
}

/// A resource page preview renders the item page in the draft section order,
/// leaves out the sections the draft hides, and stores nothing: the public
/// page keeps the theme's default sections.
#[tokio::test]
async fn resource_page_preview_renders_the_item_in_the_draft_order() {
    let (pool, seed) = site_pool().await;
    let app = router(pool);
    let item = seed.item.0;
    let preview = |sections: &'static [&'static str]| {
        keyed_preview(
            "/s/museum/preview",
            KEY_IDENTITY,
            KEY_CREDENTIAL,
            &resource_body("items", item, sections),
        )
    };

    let values_first = send(app.clone(), preview(&["values", "itemSets"])).await;
    assert_eq!(values_first.status, StatusCode::OK, "{}", values_first.body);
    assert_eq!(header(&values_first, "cache-control"), Some("no-store"));
    let body = &values_first.body;
    let values = body
        .find("<dl class=\"values\">")
        .unwrap_or_else(|| panic!("{body}"));
    let sets = body
        .find("<dl class=\"item-sets\">")
        .unwrap_or_else(|| panic!("{body}"));
    assert!(values < sets, "values should come first: {body}");
    assert!(!body.contains("media-embeds"), "a hidden section rendered");
    assert!(!body.contains("media-list"), "a hidden section rendered");
    assert!(
        !body.contains(KEY_CREDENTIAL),
        "the credentials leaked into the page"
    );

    let sets_first = send(app.clone(), preview(&["itemSets", "values"])).await;
    let body = &sets_first.body;
    assert!(
        body.find("<dl class=\"item-sets\">") < body.find("<dl class=\"values\">"),
        "the item sets should come first: {body}"
    );

    let public = send(app, get(&format!("/s/museum/item/{item}"), false)).await;
    assert!(
        public.body.contains("<div class=\"media-embeds\">"),
        "the stored theme changed: {}",
        public.body
    );
}

/// A site viewer may not see a draft theme, and a body naming no known kind is
/// a 400.
#[tokio::test]
async fn resource_page_preview_needs_the_site_update_right_and_a_known_kind() {
    let (pool, seed) = visibility_pool().await;
    let app = router(pool);
    let viewer = send(
        app.clone(),
        keyed_preview(
            "/s/vis-pub/preview",
            VIEWER_KEY_IDENTITY,
            VIEWER_KEY_CREDENTIAL,
            &resource_body("items", seed.item.0, &["values"]),
        ),
    )
    .await;
    assert_eq!(viewer.status, StatusCode::FORBIDDEN, "{}", viewer.body);

    let unknown = send(
        app,
        keyed_preview(
            "/s/vis-pub/preview",
            OWNER_KEY_IDENTITY,
            OWNER_KEY_CREDENTIAL,
            &resource_body("pages", seed.item.0, &["values"]),
        ),
    )
    .await;
    assert_eq!(unknown.status, StatusCode::BAD_REQUEST, "{}", unknown.body);
}
