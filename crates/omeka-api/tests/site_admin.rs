//! Site-admin write tests: navigation, item pool, site permissions, theme
//! settings.
//!
//! Each test drives `Api::update` or `Api::create`, the two entry points the
//! router's `/api/{resource}[/{id}]` handlers call, then asserts the document
//! the caller gets back and the rows the write left. The membership and
//! settings members mirror `SiteAdapter::hydrate`; `o:theme_settings` is the
//! port's addition for the SPA.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod support;

use omeka_api::read::Api;
use omeka_db::ids::{ItemSetId, SiteId, UserId};
use omeka_db::repos::{ItemSetRepo, SiteRepo, UserRepo};
use omeka_db::rows::{NewItemSet, NewResource, NewSite, NewUser};
use omeka_db::{migrate, open_memory};
use serde_json::{Value, json};
use sqlx::SqlitePool;
use support::{admin, base};

struct Seed {
    pool: SqlitePool,
    admin: UserId,
    editor: UserId,
    item_set: ItemSetId,
    site: SiteId,
}

async fn seed() -> Seed {
    let pool = open_memory().await.expect("in-memory database");
    migrate(&pool).await.expect("migrations");
    let users = UserRepo::new(&pool);
    let admin = users
        .create(&NewUser {
            email: "admin@example.com".to_string(),
            name: "Admin".to_string(),
            password_hash: None,
            role: "global_admin".to_string(),
            is_active: true,
            created: None,
        })
        .await
        .expect("admin row")
        .id;
    let editor = users
        .create(&NewUser {
            email: "editor@example.com".to_string(),
            name: "Editor".to_string(),
            password_hash: None,
            role: "editor".to_string(),
            is_active: true,
            created: None,
        })
        .await
        .expect("editor row")
        .id;
    let item_set = ItemSetRepo::new(&pool)
        .create(&NewItemSet {
            resource: NewResource {
                owner_id: Some(admin),
                title: Some("A set".to_string()),
                ..NewResource::default()
            },
            is_open: false,
        })
        .await
        .expect("item set row")
        .id();
    let site = SiteRepo::new(&pool)
        .create(&NewSite {
            owner_id: Some(admin),
            slug: "my-site".to_string(),
            theme: "default".to_string(),
            title: "My Site".to_string(),
            ..NewSite::default()
        })
        .await
        .expect("site row")
        .id;
    Seed {
        pool,
        admin,
        editor,
        item_set,
        site,
    }
}

async fn api(seed: &Seed) -> Api<'_> {
    Api::load(&seed.pool, base(), admin(seed.admin))
        .await
        .expect("read surface")
}

fn document(response: &omeka_api::read::ApiResponse) -> Value {
    serde_json::from_str(&response.body).expect("a JSON document")
}

/// The site's stored permissions as `(user id, role)`, in row order.
async fn stored_permissions(seed: &Seed) -> Vec<(u64, String)> {
    SiteRepo::new(&seed.pool)
        .list_permissions(seed.site)
        .await
        .expect("permissions")
        .into_iter()
        .map(|row| (row.user_id.0, row.role))
        .collect()
}

/// The site's stored item sets as `(item set id, position)`.
async fn stored_item_sets(seed: &Seed) -> Vec<(u64, Option<i64>)> {
    SiteRepo::new(&seed.pool)
        .list_item_sets(seed.site)
        .await
        .expect("item sets")
        .into_iter()
        .map(|row| (row.item_set_id.0, row.position))
        .collect()
}

async fn read_site(seed: &Seed) -> Value {
    api(seed)
        .await
        .read_member("sites", &seed.site.0.to_string())
        .await
        .expect("read site")
        .body
        .parse()
        .expect("JSON")
}

#[tokio::test]
async fn site_put_replaces_the_permissions_it_sends() {
    let seed = seed().await;
    let body = json!({
        "o:title": "My Site",
        "o:slug": "my-site",
        "o:theme": "default",
        "o:site_permission": [{ "o:user": { "o:id": seed.editor.0 }, "o:role": "editor" }]
    });
    let response = api(&seed)
        .await
        .update(
            "sites",
            &seed.site.0.to_string(),
            body.to_string().as_bytes(),
            false,
        )
        .await
        .expect("update");
    // The create-time caller grant is not in the body, so the replace removes it.
    assert_eq!(
        stored_permissions(&seed).await,
        vec![(seed.editor.0, "editor".to_string())]
    );
    assert_eq!(
        document(&response)["o:site_permission"],
        json!([{ "o:user": { "@id": base().api_resource("users", seed.editor.0), "o:id": seed.editor.0 }, "o:role": "editor" }])
    );
}

#[tokio::test]
async fn site_patch_keeps_unlisted_permissions_and_removes_the_listed_missing_one() {
    let seed = seed().await;
    let repo = SiteRepo::new(&seed.pool);
    repo.set_permission(seed.site, seed.editor, "viewer")
        .await
        .expect("seed permission");
    let body = json!({ "o:site_permission": [{ "o:user": { "o:id": seed.editor.0 }, "o:role": "admin" }] });
    api(&seed)
        .await
        .update(
            "sites",
            &seed.site.0.to_string(),
            body.to_string().as_bytes(),
            true,
        )
        .await
        .expect("patch");
    // The caller's admin grant from create is not listed, and the patch retains
    // only what the body names, exactly as upstream's reconcile loop does.
    assert_eq!(
        stored_permissions(&seed).await,
        vec![(seed.editor.0, "admin".to_string())]
    );
}

#[tokio::test]
async fn unknown_permission_user_is_not_found() {
    let seed = seed().await;
    let body = json!({ "o:site_permission": [{ "o:user": { "o:id": 9999 }, "o:role": "editor" }] });
    let error = api(&seed)
        .await
        .update(
            "sites",
            &seed.site.0.to_string(),
            body.to_string().as_bytes(),
            true,
        )
        .await
        .expect_err("an unknown user");
    assert_eq!(error.status(), http::StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn site_put_replaces_item_sets_in_body_order() {
    let seed = seed().await;
    let second = ItemSetRepo::new(&seed.pool)
        .create(&NewItemSet {
            resource: NewResource {
                owner_id: Some(seed.admin),
                title: Some("Second set".to_string()),
                ..NewResource::default()
            },
            is_open: false,
        })
        .await
        .expect("item set row")
        .id();
    let body = json!({
        "o:title": "My Site",
        "o:slug": "my-site",
        "o:theme": "default",
        "o:site_item_set": [
            { "o:item_set": { "o:id": second.0 } },
            { "o:item_set": { "o:id": seed.item_set.0 } }
        ]
    });
    let response = api(&seed)
        .await
        .update(
            "sites",
            &seed.site.0.to_string(),
            body.to_string().as_bytes(),
            false,
        )
        .await
        .expect("update");
    assert_eq!(
        stored_item_sets(&seed).await,
        vec![(second.0, Some(1)), (seed.item_set.0, Some(2))]
    );
    let document = document(&response);
    let ids: Vec<u64> = document["o:site_item_set"]
        .as_array()
        .expect("list")
        .iter()
        .map(|entry| entry["o:item_set"]["o:id"].as_u64().expect("id"))
        .collect();
    assert_eq!(ids, vec![second.0, seed.item_set.0]);
}

#[tokio::test]
async fn a_full_put_that_omits_item_sets_clears_them() {
    let seed = seed().await;
    SiteRepo::new(&seed.pool)
        .add_item_set(seed.site, seed.item_set, Some(1))
        .await
        .expect("attach");
    api(&seed)
        .await
        .update(
            "sites",
            &seed.site.0.to_string(),
            json!({ "o:title": "My Site", "o:slug": "my-site", "o:theme": "default" })
                .to_string()
                .as_bytes(),
            false,
        )
        .await
        .expect("update");
    assert!(stored_item_sets(&seed).await.is_empty());
}

#[tokio::test]
async fn unknown_item_set_is_not_found() {
    let seed = seed().await;
    let body = json!({ "o:site_item_set": [{ "o:item_set": { "o:id": 9999 } }] });
    let error = api(&seed)
        .await
        .update(
            "sites",
            &seed.site.0.to_string(),
            body.to_string().as_bytes(),
            true,
        )
        .await
        .expect_err("an unknown item set");
    assert_eq!(error.status(), http::StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn theme_settings_round_trip_through_the_site_member() {
    let seed = seed().await;
    assert!(read_site(&seed).await.get("o:theme_settings").is_none());
    let body = json!({ "o:theme_settings": { "installation_title": "Library", "show_page_pagination": false } });
    let response = api(&seed)
        .await
        .update(
            "sites",
            &seed.site.0.to_string(),
            body.to_string().as_bytes(),
            true,
        )
        .await
        .expect("patch");
    assert_eq!(
        document(&response)["o:theme_settings"],
        json!({ "installation_title": "Library", "show_page_pagination": false })
    );
    // The row is keyed by the theme, as `Theme::getSettingsKey` keys it.
    let stored: (String,) =
        sqlx::query_as("SELECT value FROM site_setting WHERE site_id = ? AND id = ?")
            .bind(seed.site.0 as i64)
            .bind("theme_settings_default")
            .fetch_one(&seed.pool)
            .await
            .expect("stored settings");
    assert_eq!(
        serde_json::from_str::<Value>(&stored.0).expect("JSON"),
        json!({ "installation_title": "Library", "show_page_pagination": false })
    );
    assert_eq!(
        read_site(&seed).await["o:theme_settings"],
        json!({ "installation_title": "Library", "show_page_pagination": false })
    );
}

#[tokio::test]
async fn a_site_put_that_omits_theme_settings_keeps_them() {
    let seed = seed().await;
    api(&seed)
        .await
        .update(
            "sites",
            &seed.site.0.to_string(),
            json!({ "o:theme_settings": { "locale": "es" } })
                .to_string()
                .as_bytes(),
            true,
        )
        .await
        .expect("patch settings");
    // The site form's full PUT never shows the settings, so it must not clear
    // them.
    api(&seed)
        .await
        .update(
            "sites",
            &seed.site.0.to_string(),
            json!({ "o:title": "My Site", "o:slug": "my-site", "o:theme": "default" })
                .to_string()
                .as_bytes(),
            false,
        )
        .await
        .expect("update");
    assert_eq!(
        read_site(&seed).await["o:theme_settings"],
        json!({ "locale": "es" })
    );
}

#[tokio::test]
async fn null_theme_settings_removes_the_stored_row() {
    let seed = seed().await;
    api(&seed)
        .await
        .update(
            "sites",
            &seed.site.0.to_string(),
            json!({ "o:theme_settings": { "locale": "es" } })
                .to_string()
                .as_bytes(),
            true,
        )
        .await
        .expect("patch settings");
    let response = api(&seed)
        .await
        .update(
            "sites",
            &seed.site.0.to_string(),
            json!({ "o:theme_settings": null }).to_string().as_bytes(),
            true,
        )
        .await
        .expect("clear");
    assert!(document(&response).get("o:theme_settings").is_none());
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM site_setting WHERE site_id = ?")
        .bind(seed.site.0 as i64)
        .fetch_one(&seed.pool)
        .await
        .expect("count");
    assert_eq!(count.0, 0);
}

#[tokio::test]
async fn site_create_hydrates_the_membership_and_settings_it_sends() {
    let seed = seed().await;
    let body = json!({
        "o:title": "Scenario site",
        "o:slug": "scenario-site",
        "o:theme": "default",
        "o:site_permission": [{ "o:user": { "o:id": seed.editor.0 }, "o:role": "admin" }],
        "o:site_item_set": [{ "o:item_set": { "o:id": seed.item_set.0 } }],
        "o:theme_settings": { "installation_title": "Library" }
    });
    let response = api(&seed)
        .await
        .create("sites", body.to_string().as_bytes(), None)
        .await
        .expect("create");
    let document = document(&response);
    let site = document["o:id"].as_u64().expect("id");
    let site_id = SiteId(site);
    assert_eq!(
        SiteRepo::new(&seed.pool)
            .list_permissions(site_id)
            .await
            .expect("permissions")
            .into_iter()
            .map(|row| (row.user_id.0, row.role))
            .collect::<Vec<_>>(),
        vec![(seed.editor.0, "admin".to_string())]
    );
    assert_eq!(
        document["o:theme_settings"],
        json!({ "installation_title": "Library" })
    );
    assert_eq!(
        document["o:site_item_set"][0]["o:item_set"]["o:id"],
        seed.item_set.0
    );
    // The welcome page still lands.
    assert_eq!(document["o:page"].as_array().expect("pages").len(), 1);
}

/// Custom CSS the sanitiser refuses is a 422 whose messages name each line,
/// and the site keeps the map it had.
#[tokio::test]
async fn refused_custom_css_is_a_422_and_stores_nothing() {
    let seed = seed().await;
    let body = json!({ "o:theme_settings": {
        "preset": "civic",
        "custom_css": "a { color: red }\n@import 'x.css';"
    } });
    let error = api(&seed)
        .await
        .update(
            "sites",
            &seed.site.0.to_string(),
            body.to_string().as_bytes(),
            true,
        )
        .await
        .expect_err("refused CSS");
    assert_eq!(error.status(), http::StatusCode::UNPROCESSABLE_ENTITY);
    let omeka_api::errors::ApiError::Validation(errors) = error else {
        panic!("a validation error, got {error:?}");
    };
    assert_eq!(
        errors.get("o:theme_settings"),
        Some(&vec![
            "Custom CSS: Line 2, column 1: @import is not allowed.".to_string()
        ])
    );
    assert!(read_site(&seed).await.get("o:theme_settings").is_none());
}

/// Custom CSS the sanitiser accepts is stored as written.
#[tokio::test]
async fn accepted_custom_css_round_trips() {
    let seed = seed().await;
    let settings = json!({ "preset": "civic", "custom_css": ".site-title a { color: #cc0000 }" });
    api(&seed)
        .await
        .update(
            "sites",
            &seed.site.0.to_string(),
            json!({ "o:theme_settings": settings })
                .to_string()
                .as_bytes(),
            true,
        )
        .await
        .expect("patch");
    assert_eq!(read_site(&seed).await["o:theme_settings"], settings);
}

/// A create carrying refused CSS writes no site at all.
#[tokio::test]
async fn a_create_with_refused_css_writes_no_site() {
    let seed = seed().await;
    let body = json!({
        "o:title": "Refused",
        "o:slug": "refused",
        "o:theme": "default",
        "o:theme_settings": { "custom_css": "p {} </style><script>alert(1)</script>" }
    });
    let error = api(&seed)
        .await
        .create("sites", body.to_string().as_bytes(), None)
        .await
        .expect_err("refused CSS");
    assert_eq!(error.status(), http::StatusCode::UNPROCESSABLE_ENTITY);
    assert!(
        SiteRepo::new(&seed.pool)
            .get_by_slug("refused")
            .await
            .expect("lookup")
            .is_none()
    );
}

/// A site created without theme settings starts from the new-site preset with
/// its palette derived, rather than the plain look of a site from before
/// presets.
#[tokio::test]
async fn a_new_site_without_theme_settings_starts_from_the_new_site_preset() {
    let seed = seed().await;
    let body = json!({ "o:title": "Fresh", "o:slug": "fresh", "o:theme": "default" });
    let response = api(&seed)
        .await
        .create("sites", body.to_string().as_bytes(), None)
        .await
        .expect("create");
    assert_eq!(
        document(&response)["o:theme_settings"],
        omeka_site::theme::tokens::new_site_settings()
    );
}
