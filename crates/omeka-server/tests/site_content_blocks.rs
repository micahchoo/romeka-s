//! The content blocks, from the API write to the public page: the write stores
//! each block's plain rendering in `o:data.html` for upstream, and the public
//! page renders the rich form.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::http::StatusCode;
use common::{Seed, append_key, get, json_request, router, send, site_pool};
use omeka_db::repos::{PropertyRepo, ValueRepo};
use omeka_db::rows::{NewProperty, NewValue};
use serde_json::{Value, json};

/// A page body with one carrier of the Romeka block `block`.
fn page_with(seed: &Seed, slug: &str, block: &str, settings: &Value) -> String {
    json!({
        "o:site": { "o:id": seed.site },
        "o:title": slug,
        "o:slug": slug,
        "o:is_public": true,
        "o:block": [{
            "o:layout": "html",
            "o:data": {},
            "o:layout_data": { "romeka": { "block": block, "v": 2, "settings": settings } }
        }],
    })
    .to_string()
}

/// Store the page and return the plain rendering the write stored.
async fn stored_plain(app: &axum::Router, body: &str) -> String {
    let created = send(
        app.clone(),
        json_request("POST", &append_key("/api/site_pages"), body),
    )
    .await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.body);
    created.json()["o:block"][0]["o:data"]["html"]
        .as_str()
        .unwrap()
        .to_string()
}

/// The public page at `slug` of the seeded site.
async fn public_page(app: &axum::Router, slug: &str) -> String {
    let page = send(app.clone(), get(&format!("/s/museum/page/{slug}"), false)).await;
    assert_eq!(page.status, StatusCode::OK, "{}", page.body);
    page.body
}

/// The seeded item's main media, as the large file the gallery links.
const CAT_LARGE: &str = "/files/large/4b1e5b9e0f9f4f5a8f6b2c3d4e5f6a7b8c9d0e1f.jpg";

/// A gallery record with no media chosen shows the item's main media: the
/// write stores it as an image link with the item's title as the caption,
/// and the public page renders the gallery and loads the lightbox for it.
#[tokio::test]
async fn a_gallery_stores_image_links_and_renders_the_gallery() {
    let (pool, seed) = site_pool().await;
    let app = router(pool);
    let settings = json!({
        "media": [{ "item": seed.item.0, "media": null, "caption": "" }],
        "layout": "carousel",
    });
    let plain = stored_plain(&app, &page_with(&seed, "pictures", "gallery", &settings)).await;
    assert_eq!(
        plain,
        format!(
            "<ul><li><a href=\"{CAT_LARGE}\" rel=\"noreferrer noopener\"><img src=\"{CAT_LARGE}\" \
             alt=\"A picture of a cat\"></a><div>A picture of a cat</div></li></ul>"
        )
    );

    let page = public_page(&app, "pictures").await;
    assert!(
        page.contains("romeka-gallery romeka-gallery-carousel"),
        "{page}"
    );
    assert!(
        page.contains(&format!("{CAT_LARGE}\" alt=\"A picture of a cat\"")),
        "{page}"
    );
    assert!(page.contains("/assets/js/lightbox.js?v="), "{page}");
    let welcome = public_page(&app, "welcome").await;
    assert!(!welcome.contains("lightbox.js"), "{welcome}");
}

/// A chosen media that is private is left out of the stored plain rendering
/// and of the public page, and the save still succeeds.
#[tokio::test]
async fn a_gallery_leaves_out_a_private_media() {
    let (pool, seed) = site_pool().await;
    sqlx::query("UPDATE resource SET is_public = 0 WHERE id = ?")
        .bind(seed.media.0 as i64)
        .execute(&pool)
        .await
        .unwrap();
    let app = router(pool);
    let settings = json!({
        "media": [{ "item": seed.item.0, "media": seed.media.0, "caption": "<p>Hidden</p>" }],
    });
    let plain = stored_plain(&app, &page_with(&seed, "hidden", "gallery", &settings)).await;
    assert_eq!(plain, "");
    let page = public_page(&app, "hidden").await;
    assert!(!page.contains("romeka-gallery"), "{page}");
    assert!(!page.contains("Hidden"), "{page}");
}

/// A featured item stores a linked heading, the item's main image, the chosen
/// fields and a link in the site's language; the public page renders the card.
#[tokio::test]
async fn a_featured_item_stores_the_card_and_renders_it() {
    let (pool, seed) = site_pool().await;
    let app = router(pool);
    let settings = json!({ "item": seed.item.0, "fields": [{ "property": "dcterms:title" }] });
    let plain = stored_plain(
        &app,
        &page_with(&seed, "featured", "featuredItem", &settings),
    )
    .await;
    let item = format!("/s/museum/item/{}", seed.item.0);
    assert_eq!(
        plain,
        format!(
            "<h3><a href=\"{item}\" rel=\"noreferrer noopener\">A picture of a cat</a></h3>\
             <img src=\"{CAT_LARGE}\" alt=\"\"><dl><dt>Title</dt><dd>A picture of a cat</dd></dl>\
             <p><a href=\"{item}\" rel=\"noreferrer noopener\">View item</a></p>"
        )
    );

    let page = public_page(&app, "featured").await;
    assert!(
        page.contains("romeka-featured romeka-featured-image-left"),
        "{page}"
    );
    assert!(
        page.contains(&format!("{CAT_LARGE}\" alt=\"\" loading=\"lazy\"")),
        "{page}"
    );
    assert!(
        page.contains("<dt>Title</dt><dd>A picture of a cat</dd>"),
        "{page}"
    );
}

/// A button block stores its links as a paragraph, with a page of the site
/// resolved to its URL, and the public page draws them as buttons.
#[tokio::test]
async fn a_button_block_stores_its_links_and_renders_buttons() {
    let (pool, seed) = site_pool().await;
    let app = router(pool);
    let welcome = send(
        app.clone(),
        get(&append_key("/api/site_pages?slug=welcome"), true),
    )
    .await;
    let page_id = welcome.json()[0]["o:id"].as_u64().unwrap();
    let settings = json!({
        "button_label": "Start here",
        "button_page": page_id,
        "button2_label": "Elsewhere",
        "button2_url": "https://example.org/",
        "style": "outline",
    });
    let plain = stored_plain(&app, &page_with(&seed, "buttons", "button", &settings)).await;
    assert_eq!(
        plain,
        "<p><a href=\"/s/museum/page/welcome\" rel=\"noreferrer noopener\">Start here</a> \
         <a href=\"https://example.org/\" rel=\"noreferrer noopener\">Elsewhere</a></p>"
    );
    let page = public_page(&app, "buttons").await;
    assert!(
        page.contains("<a href=\"/s/museum/page/welcome\" class=\"romeka-button romeka-button-outline romeka-button-medium\">Start here</a>"),
        "{page}"
    );
}

/// A quote stores a blockquote and a line citing its source item, and the
/// public page renders the styled figure.
#[tokio::test]
async fn a_quote_stores_blockquote_and_cite_and_renders_the_figure() {
    let (pool, seed) = site_pool().await;
    let app = router(pool);
    let settings = json!({
        "text": "<p>Curiosity is the cat's.</p>",
        "attribution": "A keeper",
        "item": seed.item.0,
        "style": "band",
    });
    let plain = stored_plain(&app, &page_with(&seed, "quoted", "quote", &settings)).await;
    let item = format!("/s/museum/item/{}", seed.item.0);
    assert_eq!(
        plain,
        format!(
            "<blockquote><p>Curiosity is the cat's.</p></blockquote><p>A keeper, <cite>\
             <a href=\"{item}\" rel=\"noreferrer noopener\">A picture of a cat</a></cite></p>"
        )
    );
    let page = public_page(&app, "quoted").await;
    assert!(
        page.contains("<figure class=\"romeka-quote romeka-quote-band\">"),
        "{page}"
    );
    assert!(
        page.contains(&format!(
            "<cite><a href=\"{item}\">A picture of a cat</a></cite>"
        )),
        "{page}"
    );
}

/// An accordion stores each heading as an `h3` with its answer, which the
/// purifier keeps, and the public page renders `details` the purifier would
/// drop.
#[tokio::test]
async fn an_accordion_stores_headings_and_renders_details() {
    let (pool, seed) = site_pool().await;
    let app = router(pool);
    let settings = json!({ "entries": [
        { "heading": "Is it free?", "answer": "<p>Yes, <em>always</em>.</p>" },
    ]});
    let plain = stored_plain(&app, &page_with(&seed, "faq", "accordion", &settings)).await;
    assert_eq!(plain, "<h3>Is it free?</h3><p>Yes, <em>always</em>.</p>");
    let page = public_page(&app, "faq").await;
    assert!(
        page.contains("<summary class=\"romeka-accordion-heading\">Is it free?</summary>"),
        "{page}"
    );
}

/// A timeline stores the items in date order by the chosen property, the
/// undated ones after, and the public page renders the timeline.
#[tokio::test]
async fn a_timeline_stores_the_items_in_date_order() {
    let (pool, seed) = site_pool().await;
    let date = PropertyRepo::new(&pool)
        .create(&NewProperty {
            owner_id: None,
            vocabulary_id: seed.vocabulary,
            local_name: "date".to_string(),
            label: "Date".to_string(),
            comment: None,
        })
        .await
        .unwrap();
    ValueRepo::new(&pool)
        .create(&NewValue {
            resource_id: seed.item.as_resource(),
            property_id: date.id,
            value_resource_id: None,
            value_annotation_id: None,
            r#type: "literal".to_string(),
            lang: None,
            value: Some("1894-06".to_string()),
            uri: None,
            is_public: true,
        })
        .await
        .unwrap();
    let app = router(pool);
    let settings = json!({ "source": "items", "items": [seed.item.0] });
    let plain = stored_plain(&app, &page_with(&seed, "dates", "timeline", &settings)).await;
    assert_eq!(
        plain,
        format!(
            "<ol><li>1894-06: <a href=\"/s/museum/item/{}\" rel=\"noreferrer noopener\">A picture of a cat</a></li></ol>",
            seed.item.0
        )
    );
    let page = public_page(&app, "dates").await;
    assert!(
        page.contains("<p class=\"romeka-timeline-date\">1894-06</p>"),
        "{page}"
    );
}

/// A search box stores a link to the search page, because the purifier
/// allows no form, and the public page renders the form.
#[tokio::test]
async fn a_search_box_stores_a_link_and_renders_the_form() {
    let (pool, seed) = site_pool().await;
    let app = router(pool);
    let settings = json!({ "placeholder": "Find a cat", "resource": "items" });
    let plain = stored_plain(&app, &page_with(&seed, "find", "searchBox", &settings)).await;
    assert_eq!(
        plain,
        "<p><a href=\"../item\" rel=\"noreferrer noopener\">Find a cat</a></p>"
    );
    let page = public_page(&app, "find").await;
    assert!(
        page.contains("<form class=\"romeka-search\" action=\"/s/museum/item\""),
        "{page}"
    );
}
