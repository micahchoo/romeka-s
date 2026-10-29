//! The hints the editor's live preview shows: `POST /s/:site-slug/preview`
//! with `romeka_hints` renders an unwritten block's hint, greyed, where its
//! words go, and the public page of the same blocks leaves that block out.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::http::StatusCode;
use common::{
    OWNER_KEY_CREDENTIAL, OWNER_KEY_IDENTITY, json_request, router, send, visibility_pool,
};
use omeka_db::ids::PageId;
use omeka_db::repos::SiteRepo;
use omeka_db::rows::NewSiteBlock;
use serde_json::json;

#[tokio::test]
async fn the_preview_shows_a_hint_and_the_public_page_leaves_the_block_out() {
    let (pool, seed) = visibility_pool().await;
    for (position, html) in [(1_i64, ""), (2, "<p>Written copy</p>")] {
        SiteRepo::new(&pool)
            .add_block(&NewSiteBlock {
                page_id: PageId::from(seed.public_page),
                layout: "html".to_string(),
                data: json!({ "html": html }),
                layout_data: None,
                position,
            })
            .await
            .unwrap();
    }
    let app = router(pool);

    let public = send(
        app.clone(),
        common::get("/s/vis-pub/page/vis-pub-public", false),
    )
    .await;
    assert_eq!(public.status, StatusCode::OK, "{}", public.body);
    assert!(public.body.contains("Written copy"), "{}", public.body);
    assert_eq!(
        public.body.matches("class=\"block block-html\"").count(),
        1,
        "the empty block left a wrapper: {}",
        public.body
    );

    let body = json!({
        "o:id": seed.public_page,
        "o:title": "Vis public page",
        "o:slug": "vis-pub-public",
        "o:is_public": true,
        "o:layout": "",
        "o:layout_data": {},
        "o:block": [
            {"o:layout": "html", "o:data": {"html": ""}},
            {"o:layout": "html", "o:data": {"html": "<p>Written copy</p>"}},
        ],
        "romeka_hints": {"fieldExample.html.html": "Escriba aquí <lo que leen>"},
    });
    let preview = send(
        app,
        json_request(
            "POST",
            &format!(
                "/s/vis-pub/preview?key_identity={OWNER_KEY_IDENTITY}&key_credential={OWNER_KEY_CREDENTIAL}"
            ),
            &body.to_string(),
        ),
    )
    .await;
    assert_eq!(preview.status, StatusCode::OK, "{}", preview.body);
    assert!(
        preview.body.contains(
            "data-romeka-block=\"1\"><span class=\"romeka-hint\">Escriba aquí &lt;lo que leen&gt;</span></div>"
        ),
        "{}",
        preview.body
    );
    assert!(preview.body.contains("Written copy"), "{}", preview.body);
}
