//! Site settings through the public seams: the `/api` site document and the
//! `/s` pages.
//!
//! One test per setting the audit named: page size, sort, browse headings, the
//! page body class, the favicon, embedded JSON-LD, the `noindex` meta and the
//! user bar. The site settings are written as `site_setting` rows, the shape
//! upstream stores, and read back through the rendered page.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::http::StatusCode;
use common::{
    Seed, append_key, get, get_cookie, json_request, login_cookie, router, send, site_pool,
};
use omeka_db::ids::{ItemId, PropertyId, SiteId, UserId};
use omeka_db::repos::{
    ItemRepo, Paging, PropertyFilter, PropertyRepo, SiteRepo, SiteSettingRepo, UserRepo, ValueRepo,
};
use omeka_db::rows::{NewItem, NewProperty, NewResource, NewUser, NewValue};
use omeka_site::settings::{SITE_SETTINGS, site_settings_registry};
use serde_json::json;
use sqlx::SqlitePool;

const PASSWORD: &str = "settings-pass-1";

/// The site pool with the seeded item attached, plus a `dcterms:description`
/// property for the browse body setting.
async fn settings_pool() -> (SqlitePool, Seed) {
    let (pool, seed) = site_pool().await;
    PropertyRepo::new(&pool)
        .create(&NewProperty {
            owner_id: None,
            vocabulary_id: seed.vocabulary,
            local_name: "description".to_string(),
            label: "Description".to_string(),
            comment: None,
        })
        .await
        .unwrap();
    (pool, seed)
}

fn db_site(seed: &Seed) -> SiteId {
    SiteId::from(seed.site)
}

async fn set(pool: &SqlitePool, site: SiteId, key: &str, value: serde_json::Value) {
    SiteSettingRepo::new(pool)
        .set(site, key, &value)
        .await
        .unwrap();
}

/// The `dcterms:<local>` property's id.
async fn property(pool: &SqlitePool, local: &str) -> PropertyId {
    PropertyRepo::new(pool)
        .list(&PropertyFilter::default(), &Paging::default())
        .await
        .unwrap()
        .into_iter()
        .find(|row| row.local_name == local)
        .unwrap_or_else(|| panic!("the {local} property exists"))
        .id
}

/// Create a public item owned by `owner` with a `dcterms:title` value.
async fn add_item(pool: &SqlitePool, owner: UserId, title: &str) -> ItemId {
    let title_property = property(pool, "title").await;
    let item = ItemRepo::new(pool)
        .create(&NewItem {
            resource: NewResource {
                owner_id: Some(owner),
                title: Some(title.to_string()),
                is_public: true,
                ..NewResource::default()
            },
            primary_media_id: None,
        })
        .await
        .unwrap();
    ValueRepo::new(pool)
        .create(&NewValue {
            resource_id: item.id().as_resource(),
            property_id: title_property,
            value_resource_id: None,
            value_annotation_id: None,
            r#type: "literal".to_string(),
            lang: None,
            value: Some(title.to_string()),
            uri: None,
            is_public: true,
        })
        .await
        .unwrap();
    item.id()
}

async fn attach(pool: &SqlitePool, item: ItemId, site: SiteId) {
    sqlx::query("INSERT INTO item_site (item_id, site_id) VALUES (?, ?)")
        .bind(i64::try_from(item.0).unwrap())
        .bind(i64::try_from(site.0).unwrap())
        .execute(pool)
        .await
        .unwrap();
}

/// The count of `li.resource` entries a browse page renders.
fn resource_count(body: &str) -> usize {
    body.matches("class=\"item resource\"").count()
}

/// `GET /api/sites/{id}` carries the stored rows as `o:site_settings`, and a
/// `PATCH` writes them back as rows.
#[tokio::test]
async fn api_site_settings_read_and_write_rows() {
    let (pool, seed) = settings_pool().await;
    let app = router(pool.clone());
    let site = db_site(&seed);
    set(&pool, site, "pagination_per_page", json!(7)).await;

    let answer = send(app.clone(), get(&format!("/api/sites/{}", seed.site), true)).await;
    assert_eq!(answer.status, StatusCode::OK);
    assert_eq!(
        answer.json()["o:site_settings"]["pagination_per_page"],
        json!(7)
    );

    let body = json!({
        "o:site_settings": {"installation_title": "Archive", "pagination_per_page": null}
    })
    .to_string();
    let answer = send(
        app,
        json_request(
            "PATCH",
            &append_key(&format!("/api/sites/{}", seed.site)),
            &body,
        ),
    )
    .await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);

    let repo = SiteSettingRepo::new(&pool);
    assert_eq!(
        repo.get(site, "installation_title").await.unwrap(),
        Some(json!("Archive"))
    );
    assert_eq!(repo.get(site, "pagination_per_page").await.unwrap(), None);
}

/// The admin form writes only the keys the registry offers. A stored key the
/// registry leaves out, because no renderer reads it, keeps its row through
/// that write.
#[tokio::test]
async fn a_form_save_keeps_the_rows_the_form_does_not_offer() {
    let (pool, seed) = settings_pool().await;
    let app = router(pool.clone());
    let site = db_site(&seed);
    let offered: Vec<&str> = site_settings_registry()
        .settings
        .iter()
        .map(|entry| entry.key)
        .collect();
    let unoffered: Vec<&str> = SITE_SETTINGS
        .iter()
        .map(|spec| spec.key)
        .filter(|key| !offered.contains(key))
        .collect();
    assert!(!unoffered.is_empty(), "the registry offers every key");
    for key in &unoffered {
        set(&pool, site, key, json!(format!("kept {key}"))).await;
    }

    let form: serde_json::Map<String, serde_json::Value> = offered
        .iter()
        .map(|key| ((*key).to_string(), serde_json::Value::Null))
        .chain([("installation_title".to_string(), json!("Archive"))])
        .collect();
    let body = json!({ "o:site_settings": form }).to_string();
    let answer = send(
        app,
        json_request(
            "PATCH",
            &append_key(&format!("/api/sites/{}", seed.site)),
            &body,
        ),
    )
    .await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);

    let repo = SiteSettingRepo::new(&pool);
    assert_eq!(
        repo.get(site, "installation_title").await.unwrap(),
        Some(json!("Archive"))
    );
    for key in &unoffered {
        assert_eq!(
            repo.get(site, key).await.unwrap(),
            Some(json!(format!("kept {key}"))),
            "{key}"
        );
    }
}

/// The page size is the site `pagination_per_page`, else the global setting,
/// else 25.
#[tokio::test]
async fn page_size_comes_from_the_site_setting() {
    let (pool, seed) = settings_pool().await;
    let app = router(pool.clone());
    let site = db_site(&seed);
    let alpha = add_item(&pool, seed.user, "Alpha").await;
    let beta = add_item(&pool, seed.user, "Beta").await;
    attach(&pool, alpha, site).await;
    attach(&pool, beta, site).await;

    // Default: the global setting is unset, so 25, and all three rows show.
    let answer = send(app.clone(), get("/s/museum/item", false)).await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    assert_eq!(resource_count(&answer.body), 3, "{}", answer.body);

    set(&pool, site, "pagination_per_page", json!(1)).await;
    let answer = send(app, get("/s/museum/item", false)).await;
    assert_eq!(resource_count(&answer.body), 1, "{}", answer.body);
}

/// The browse sort is `browse_defaults_public_items`; the default is `id` desc.
#[tokio::test]
async fn sort_comes_from_browse_defaults_public_items() {
    let (pool, seed) = settings_pool().await;
    let app = router(pool.clone());
    let site = db_site(&seed);
    let alpha = add_item(&pool, seed.user, "Alpha").await;
    let beta = add_item(&pool, seed.user, "Beta").await;
    attach(&pool, alpha, site).await;
    attach(&pool, beta, site).await;

    // Default `id` desc: the later item leads.
    let answer = send(app.clone(), get("/s/museum/item", false)).await;
    let beta_at = answer.body.find("Beta").expect("Beta is listed");
    let alpha_at = answer.body.find("Alpha").expect("Alpha is listed");
    assert!(
        beta_at < alpha_at,
        "default sort was not id desc: {}",
        answer.body
    );

    set(
        &pool,
        site,
        "browse_defaults_public_items",
        json!({"sort_by": "title", "sort_order": "asc"}),
    )
    .await;
    let answer = send(app, get("/s/museum/item", false)).await;
    let alpha_at = answer.body.find("Alpha").expect("Alpha is listed");
    let beta_at = answer.body.find("Beta").expect("Beta is listed");
    assert!(
        alpha_at < beta_at,
        "title asc was not applied: {}",
        answer.body
    );
}

/// `browse_body_property_term` picks the value the browse body prints.
#[tokio::test]
async fn browse_body_property_term_renders_the_value() {
    let (pool, seed) = settings_pool().await;
    let app = router(pool.clone());
    let site = db_site(&seed);
    let item = add_item(&pool, seed.user, "Alpha").await;
    attach(&pool, item, site).await;
    ValueRepo::new(&pool)
        .create(&NewValue {
            resource_id: item.as_resource(),
            property_id: property(&pool, "description").await,
            value_resource_id: None,
            value_annotation_id: None,
            r#type: "literal".to_string(),
            lang: None,
            value: Some("A long description".to_string()),
            uri: None,
            is_public: true,
        })
        .await
        .unwrap();
    set(
        &pool,
        site,
        "browse_body_property_term",
        json!("dcterms:description"),
    )
    .await;

    let answer = send(app, get("/s/museum/item", false)).await;
    assert!(
        answer.body.contains("A long description"),
        "{}",
        answer.body
    );
}

/// A page's body class is `page {layout_data.class} site-page-{slug}`.
#[tokio::test]
async fn page_body_class_follows_the_layout_data() {
    let (pool, _seed) = settings_pool().await;
    let app = router(pool);
    let answer = send(app, get("/s/museum/page/welcome", false)).await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    assert!(
        answer.body.contains("class=\"page site-page-welcome\""),
        "{}",
        answer.body
    );
}

/// The `favicon` setting's asset id becomes a `rel="icon"` link.
#[tokio::test]
async fn favicon_links_the_asset() {
    let (pool, seed) = settings_pool().await;
    let app = router(pool.clone());
    let site = db_site(&seed);
    let asset: i64 = sqlx::query_scalar(
        "INSERT INTO asset (owner_id, name, media_type, storage_id, extension, alt_text) \
         VALUES (?, ?, ?, ?, ?, ?) RETURNING id",
    )
    .bind(seed.user.0 as i64)
    .bind("favicon.png")
    .bind("image/png")
    .bind("a1b2c3d4e5f60718293a4b5c6d7e8f9012345678")
    .bind("png")
    .bind(Option::<String>::None)
    .fetch_one(&pool)
    .await
    .unwrap();
    set(&pool, site, "favicon", json!(asset.to_string())).await;

    let answer = send(app, get("/s/museum/page/welcome", false)).await;
    assert!(answer.body.contains("rel=\"icon\""), "{}", answer.body);
    assert!(
        answer
            .body
            .contains("/files/asset/a1b2c3d4e5f60718293a4b5c6d7e8f9012345678.png"),
        "{}",
        answer.body
    );
}

/// Embedded JSON-LD appears on a show page, unless `disable_jsonld_embed` is
/// set.
#[tokio::test]
async fn jsonld_embed_follows_disable_jsonld_embed() {
    let (pool, seed) = settings_pool().await;
    let app = router(pool.clone());
    let site = db_site(&seed);
    let path = format!("/s/museum/item/{}", seed.item.0);

    let answer = send(app.clone(), get(&path, false)).await;
    assert!(
        answer.body.contains("application/ld+json"),
        "{}",
        answer.body
    );

    set(&pool, site, "disable_jsonld_embed", json!(true)).await;
    let answer = send(app, get(&path, false)).await;
    assert!(
        !answer.body.contains("application/ld+json"),
        "{}",
        answer.body
    );
}

/// An item not attached to the site is `noindex`; one that is attached is not.
#[tokio::test]
async fn noindex_marks_an_item_outside_the_site() {
    let (pool, seed) = settings_pool().await;
    let app = router(pool.clone());
    let outside = add_item(&pool, seed.user, "Outside").await;

    let attached = send(
        app.clone(),
        get(&format!("/s/museum/item/{}", seed.item.0), false),
    )
    .await;
    assert!(
        !attached
            .body
            .contains("name=\"robots\" content=\"noindex\""),
        "{}",
        attached.body
    );

    let outside_answer = send(app, get(&format!("/s/museum/item/{}", outside.0), false)).await;
    assert!(
        outside_answer
            .body
            .contains("name=\"robots\" content=\"noindex\""),
        "{}",
        outside_answer.body
    );
}

/// The user bar follows `show_user_bar`: never at `-1`, only signed in at `0`,
/// always at `1`.
#[tokio::test]
async fn user_bar_follows_show_user_bar_and_the_session() {
    let (pool, seed) = settings_pool().await;
    let app = router(pool.clone());
    let site = db_site(&seed);
    let user = UserRepo::new(&pool)
        .create(&NewUser {
            email: "bar@example.com".to_string(),
            name: "Bar Person".to_string(),
            password_hash: Some(omeka_api::password::hash(PASSWORD).unwrap()),
            role: "researcher".to_string(),
            is_active: true,
            created: None,
        })
        .await
        .unwrap();
    SiteRepo::new(&pool)
        .set_permission(site, user.id, "viewer")
        .await
        .unwrap();

    // `show_user_bar` unset is `0`: anonymous hides, a session shows.
    let anonymous = send(app.clone(), get("/s/museum/page/welcome", false)).await;
    assert!(
        !anonymous.body.contains("id=\"user-bar\""),
        "{}",
        anonymous.body
    );

    let cookie = login_cookie(app.clone(), "bar@example.com", PASSWORD).await;
    let signed_in = send(app.clone(), get_cookie("/s/museum/page/welcome", &cookie)).await;
    assert!(
        signed_in.body.contains("id=\"user-bar\""),
        "{}",
        signed_in.body
    );
    assert!(signed_in.body.contains("Bar Person"), "{}", signed_in.body);

    set(&pool, site, "show_user_bar", json!(-1)).await;
    let hidden = send(app.clone(), get_cookie("/s/museum/page/welcome", &cookie)).await;
    assert!(!hidden.body.contains("id=\"user-bar\""), "{}", hidden.body);

    set(&pool, site, "show_user_bar", json!(1)).await;
    let always = send(app, get("/s/museum/page/welcome", false)).await;
    assert!(always.body.contains("id=\"user-bar\""), "{}", always.body);
}
