//! The block markers the editor's live preview reads: `POST /s/:site-slug/preview`
//! stamps `data-romeka-block="n"` on every rendered block and group wrapper,
//! where `n` is the block's one-based position in the body's `o:block`, and the
//! public page render of the same blocks carries none.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::http::StatusCode;
use common::{
    OWNER_KEY_CREDENTIAL, OWNER_KEY_IDENTITY, json_request, router, send, visibility_pool,
};
use omeka_db::ids::PageId;
use omeka_db::repos::SiteRepo;
use omeka_db::rows::NewSiteBlock;
use serde_json::{Value, json};

const MARKER: &str = "data-romeka-block=\"";

/// The blocks both renders show: a block, a group of one member, a block.
fn blocks() -> Vec<(&'static str, Value)> {
    vec![
        ("html", json!({"html": "<p>Opening copy</p>"})),
        ("blockGroup", json!({"span": "1"})),
        ("html", json!({"html": "<p>Grouped copy</p>"})),
        ("lineBreak", json!({"break_type": "opaque"})),
    ]
}

/// Each marker's value, in document order, with the tag it sits on.
fn markers(document: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for (at, _) in document.match_indices(MARKER) {
        let value_start = at + MARKER.len();
        let value_end = value_start + document[value_start..].find('"').unwrap();
        let tag_start = document[..at].rfind('<').unwrap();
        let tag_end = at + document[at..].find('>').unwrap();
        out.push((
            document[value_start..value_end].to_string(),
            document[tag_start..=tag_end].to_string(),
        ));
    }
    out
}

#[tokio::test]
async fn the_preview_marks_every_block_and_group_and_the_public_page_marks_none() {
    let (pool, seed) = visibility_pool().await;
    for (position, (layout, data)) in (1_i64..).zip(blocks()) {
        SiteRepo::new(&pool)
            .add_block(&NewSiteBlock {
                page_id: PageId::from(seed.public_page),
                layout: layout.to_string(),
                data,
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
    assert!(public.body.contains("Grouped copy"), "{}", public.body);
    assert!(
        !public.body.contains("data-romeka-block"),
        "the public page carries a marker: {}",
        public.body
    );

    let body = json!({
        "o:id": seed.public_page,
        "o:title": "Vis public page",
        "o:slug": "vis-pub-public",
        "o:is_public": true,
        "o:layout": "",
        "o:layout_data": {},
        "o:block": blocks()
            .into_iter()
            .map(|(layout, data)| json!({"o:layout": layout, "o:data": data}))
            .collect::<Vec<_>>(),
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

    let found = markers(&preview.body);
    let values: Vec<&str> = found.iter().map(|(value, _)| value.as_str()).collect();
    assert_eq!(values, ["1", "2", "3", "4"], "{}", preview.body);
    for (value, tag) in &found {
        assert!(
            tag.starts_with("<div class=\"block "),
            "marker {value} is not on a block wrapper: {tag}"
        );
    }
    assert!(
        found[1].1.contains("block-blockGroup"),
        "marker 2 is not the group: {}",
        found[1].1
    );
}
