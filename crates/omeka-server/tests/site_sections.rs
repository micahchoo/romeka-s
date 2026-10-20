//! A block group sent through the API renders on the public page, whatever
//! JSON kind its span arrives as: upstream reads `(int)
//! $block->dataValue('span')`, so a number and a numeric string both count.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::http::StatusCode;
use common::{append_key, get, json_request, router, seeded_pool, send};
use serde_json::{Value, json};

/// A page holding a split section of two html blocks, its span sent as `span`.
fn section_page(site: u64, slug: &str, span: &Value) -> String {
    let html =
        |text: &str| json!({ "o:layout": "html", "o:data": { "html": format!("<p>{text}</p>") } });
    json!({
        "o:site": { "o:id": site },
        "o:title": "Sections",
        "o:slug": slug,
        "o:block": [
            {
                "o:layout": "blockGroup",
                "o:data": { "span": span },
                "o:layout_data": { "romeka": {
                    "block": "blockGroup", "v": 2,
                    "settings": { "section": { "layout": "split-50-50" } },
                }},
            },
            html("Left"),
            html("Right"),
            html("After"),
        ],
    })
    .to_string()
}

#[tokio::test]
async fn a_group_span_sent_as_a_number_or_text_renders_the_group() {
    let (pool, seed) = seeded_pool().await;
    let app = router(pool.clone());
    for (slug, span) in [("number", json!(2)), ("text", json!("2"))] {
        let created = send(
            app.clone(),
            json_request(
                "POST",
                &append_key("/api/site_pages"),
                &section_page(seed.site, slug, &span),
            ),
        )
        .await;
        assert_eq!(created.status, StatusCode::OK, "{}", created.body);

        let page = send(app.clone(), get(&format!("/s/museum/page/{slug}"), false)).await;
        assert_eq!(page.status, StatusCode::OK, "{}", page.body);
        let group = "<div class=\"block block-blockGroup romeka-section romeka-section-split-50-50\" \
                     style=\"\">\
                     <div class=\"block block-html\" style=\"\"><p>Left</p></div>\
                     <div class=\"block block-html\" style=\"\"><p>Right</p></div>\
                     </div>\
                     <div class=\"block block-html\" style=\"\"><p>After</p></div>";
        assert!(
            page.body.contains(group),
            "span {span}: the group did not hold its two blocks: {}",
            page.body
        );
    }
}
