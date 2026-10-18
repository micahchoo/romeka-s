//! HTML purification through the public seams: an API write read back through
//! `GET /api/site_pages/:id`, and the same value rendered by the public page.
//!
//! Upstream purifies at hydrate with `Omeka\Stdlib\HtmlPurifier`
//! (`BlockLayout/Html.php:34`, `Api/Adapter/SitePageAdapter.php:326`). The port
//! purifies on write for those paths and again when the site renders a stored
//! `html` block or an attachment caption, so a row written before the fix, or
//! copied by the migrator, still renders safely.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::http::StatusCode;
use common::{append_key, get, json_request, router, seeded_pool, send};
use omeka_db::ids::SiteId;
use omeka_db::repos::SiteRepo;
use omeka_db::rows::{NewSiteBlock, NewSitePage};

/// The measured payload from the brief, plus a caption that carries both an
/// allowed tag and a script.
const DANGEROUS: &str = r#"<p>ok</p><script>alert(1)</script><img src=x onerror=alert(2)>"#;
const DANGEROUS_CAPTION: &str = r#"<b>bold</b><script>alert(1)</script>"#;

fn page_body(site: u64, item: u64, media: u64) -> String {
    let attachment = serde_json::json!({
        "o:item": {"o:id": item},
        "o:media": {"o:id": media},
        "o:caption": DANGEROUS_CAPTION,
    });
    serde_json::json!({
        "o:site": {"o:id": site},
        "o:title": "Purified",
        "o:slug": "purified",
        "o:block": [
            {"o:layout": "html", "o:data": {"html": DANGEROUS}, "o:attachment": [attachment]},
            {
                "o:layout": "media",
                "o:data": {"media_display": "thumbnail", "show_title_option": "item_title"},
                "o:attachment": [attachment],
            },
        ],
    })
    .to_string()
}

/// `SitePageAdapter::hydrate` runs `BlockLayout\Html::onHydrate` and
/// `hydrateAttachments`, which purify the `html` block body and every
/// `o:caption`. The stored page a caller reads back carries none of the script.
#[tokio::test]
async fn a_write_stores_purified_html_and_captions() {
    let (pool, seed) = seeded_pool().await;
    let app = router(pool);
    let body = page_body(seed.site, seed.item.0, seed.media.0);

    let created = send(
        app.clone(),
        json_request("POST", &append_key("/api/site_pages"), &body),
    )
    .await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.body);
    let document = created.json();
    let block = &document["o:block"][0];
    let html = block["o:data"]["html"].as_str().unwrap();
    assert!(
        !html.contains("<script"),
        "stored body kept a script: {html}"
    );
    assert!(
        !html.contains("onerror"),
        "stored body kept an event handler: {html}"
    );
    assert!(
        html.contains("<p>ok</p>"),
        "stored body dropped the paragraph: {html}"
    );

    let caption = block["o:attachment"][0]["o:caption"].as_str().unwrap();
    assert!(
        !caption.contains("<script"),
        "stored caption kept a script: {caption}"
    );
    assert!(
        caption.contains("<b>bold</b>"),
        "stored caption dropped bold: {caption}"
    );
}

/// `view/common/block-layout/html.phtml` echoes the stored body and
/// `view/common/block-layout/file.phtml` echoes the caption; both are safely
/// purified when rendered.
#[tokio::test]
async fn the_public_page_renders_a_written_block_safely() {
    let (pool, seed) = seeded_pool().await;
    let app = router(pool);
    let body = page_body(seed.site, seed.item.0, seed.media.0);
    let created = send(
        app.clone(),
        json_request("POST", &append_key("/api/site_pages"), &body),
    )
    .await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.body);

    let answer = send(app, get("/s/museum/page/purified", false)).await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    // The bundled theme carries its own `<script src>` tags; only the injected
    // payload must be gone.
    assert!(!answer.body.contains("<script>alert"), "{}", answer.body);
    assert!(!answer.body.contains("onerror="), "{}", answer.body);
    assert!(answer.body.contains("<p>ok</p>"), "{}", answer.body);
    assert!(
        answer
            .body
            .contains("<div class=\"caption\"><b>bold</b></div>"),
        "{}",
        answer.body
    );
}

/// A row stored before the fix (or copied by the migrator) is not purified in
/// the database, so the render purifies it again. This is the defence in depth
/// the brief asks for.
#[tokio::test]
async fn the_public_page_purifies_a_pre_existing_row() {
    let (pool, seed) = seeded_pool().await;
    let repo = SiteRepo::new(&pool);
    let page = repo
        .add_page(&NewSitePage {
            site_id: SiteId::from(seed.site),
            slug: "legacy".to_string(),
            title: "Legacy".to_string(),
            is_public: true,
            ..NewSitePage::default()
        })
        .await
        .unwrap();
    repo.add_block(&NewSiteBlock {
        page_id: page.id,
        layout: "html".to_string(),
        data: serde_json::json!({"html": DANGEROUS}),
        layout_data: None,
        position: 1,
    })
    .await
    .unwrap();

    let answer = send(router(pool), get("/s/museum/page/legacy", false)).await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    assert!(!answer.body.contains("<script>alert"), "{}", answer.body);
    assert!(!answer.body.contains("onerror="), "{}", answer.body);
    assert!(answer.body.contains("<p>ok</p>"), "{}", answer.body);
}
