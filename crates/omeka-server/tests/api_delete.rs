//! Delete-endpoint integration tests.
//!
//! Every test drives the router, so the answer is what a caller sees: the
//! status, the document, and, for a media, the file store on disk. The cascade
//! assertions read the database directly, because a removed value or a removed
//! page has no endpoint left to read it through.
//!
//! The upstream rule each test freezes is named in its doc comment.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::http::StatusCode;
use common::{append_key, get, request, seeded_pool, seeded_router, send, site_pool};
use omeka_media::file_render::ThumbnailType;
use omeka_media::store::{FileStore, Store, StorePrefix, storage_path};
use omeka_server::FILES_URI;
use sqlx::SqlitePool;

/// The storage id the seeded media carries (`common::seed`).
const MEDIA_STORAGE_ID: &str = "4b1e5b9e0f9f4f5a8f6b2c3d4e5f6a7b8c9d0e1f";

/// A file store rooted in a tempdir, with the tempdir kept alive by the caller.
fn temp_store() -> (tempfile::TempDir, FileStore) {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = FileStore::new(dir.path(), FILES_URI);
    (dir, store)
}

fn keyed(uri: &str) -> String {
    append_key(uri)
}

async fn count(pool: &SqlitePool, sql: &str, id: u64) -> i64 {
    sqlx::query_scalar(sql)
        .bind(id as i64)
        .fetch_one(pool)
        .await
        .expect("count query")
}

async fn count_all(pool: &SqlitePool, sql: &str) -> i64 {
    sqlx::query_scalar(sql)
        .fetch_one(pool)
        .await
        .expect("count query")
}

/// Create one user holding the given role and an active API key for it.
///
/// The identity is 32 alphanumerics, the shape `ApiKey::STRING_CHARLIST`
/// requires; a shorter one would never reach the key lookup.
async fn keyed_user(pool: &SqlitePool, role: &str) -> (u64, String, String) {
    let user = omeka_db::repos::UserRepo::new(pool)
        .create(&omeka_db::rows::NewUser {
            email: format!("{role}@example.com"),
            name: role.to_string(),
            password_hash: None,
            role: role.to_string(),
            is_active: true,
            created: None,
        })
        .await
        .unwrap();
    // The identity must be 32 alphanumerics (`ApiKey::STRING_CHARLIST`); a
    // shorter or longer one would never reach the key lookup.
    let mut identity: String = role.chars().filter(char::is_ascii_alphanumeric).collect();
    identity.truncate(32);
    while identity.len() < 32 {
        identity.push('x');
    }
    let credential = format!("{role}-credential");
    omeka_db::repos::ApiKeyRepo::new(pool)
        .create(&omeka_db::rows::NewApiKey {
            id: identity.clone(),
            owner_id: user.id,
            label: format!("{role} key"),
            credential_hash: bcrypt::hash(&credential, 4).unwrap(),
            last_accessed: None,
            last_ip: None,
            created: None,
        })
        .await
        .unwrap();
    (user.id.0, identity, credential)
}

/// `ApiController::delete` answers with the detached entity
/// (`Api\Manager::finalize` renders it after the flush), so the body keeps the
/// read's members but loses the id and points `@id` at the collection, and the
/// read afterwards is a 404.
#[tokio::test]
async fn delete_an_item_answers_the_detached_entity_then_reads_404() {
    let (app, seed) = seeded_router().await;
    let before = send(
        app.clone(),
        get(&format!("/api/items/{}", seed.item.0), false),
    )
    .await;
    assert_eq!(before.status, StatusCode::OK, "{}", before.body);
    assert_eq!(before.json()["o:id"], seed.item.0);

    let deleted = send(
        app.clone(),
        request("DELETE", &keyed(&format!("/api/items/{}", seed.item.0))),
    )
    .await;
    assert_eq!(deleted.status, StatusCode::OK, "{}", deleted.body);
    let item = deleted.json();
    assert!(
        item["o:id"].is_null(),
        "delete kept the id: {}",
        deleted.body
    );
    assert_eq!(item["@id"], "http://localhost/api/items");
    assert_eq!(item["o:title"], before.json()["o:title"]);

    let read = send(app, get(&format!("/api/items/{}", seed.item.0), false)).await;
    assert_eq!(read.status, StatusCode::NOT_FOUND);
    assert_eq!(
        read.body,
        format!(
            r#"{{"errors":{{"error":"Omeka\\Entity\\Item entity with criteria {{\"id\":\"{}\"}} not found"}}}}"#,
            seed.item.0
        )
    );
}

/// `Entity/Item.php:24`: the media cascade remove, so deleting an item takes
/// its media and the values of both.
#[tokio::test]
async fn deleting_an_item_removes_its_values_and_its_media() {
    let (pool, seed) = seeded_pool().await;
    let app = common::router(pool.clone());

    let deleted = send(
        app.clone(),
        request("DELETE", &keyed(&format!("/api/items/{}", seed.item.0))),
    )
    .await;
    assert_eq!(deleted.status, StatusCode::OK, "{}", deleted.body);

    assert_eq!(
        count(
            &pool,
            "SELECT COUNT(*) FROM value WHERE resource_id = ?",
            seed.item.0
        )
        .await,
        0,
        "the item kept a value"
    );
    assert_eq!(
        count(
            &pool,
            "SELECT COUNT(*) FROM value WHERE resource_id = ?",
            seed.media.0
        )
        .await,
        0,
        "the media kept a value"
    );
    assert_eq!(
        count(
            &pool,
            "SELECT COUNT(*) FROM media WHERE id = ?",
            seed.media.0
        )
        .await,
        0,
        "the media row survived"
    );
    assert_eq!(
        count(&pool, "SELECT COUNT(*) FROM item WHERE id = ?", seed.item.0).await,
        0,
        "the item row survived"
    );

    let read = send(app, get(&format!("/api/media/{}", seed.media.0), false)).await;
    assert_eq!(read.status, StatusCode::NOT_FOUND);
}

/// `Module::deleteMediaFiles`: the original and every thumbnail type leave the
/// store when the media row is removed, and the answer describes that media.
#[tokio::test]
async fn deleting_a_media_removes_the_stored_file() {
    let (pool, seed) = seeded_pool().await;
    let (_dir, store) = temp_store();
    let original = storage_path(
        StorePrefix::Original,
        &format!("{MEDIA_STORAGE_ID}.jpg"),
        None,
    );
    let thumbnails: Vec<String> = ThumbnailType::ALL
        .iter()
        .map(|kind| storage_path(StorePrefix::Thumbnail(*kind), MEDIA_STORAGE_ID, Some("jpg")))
        .collect();
    store.put(b"original", &original).expect("put original");
    for path in &thumbnails {
        store.put(b"thumbnail", path).expect("put thumbnail");
    }

    let app = common::router_with_store(pool, store.clone());
    let deleted = send(
        app,
        request("DELETE", &keyed(&format!("/api/media/{}", seed.media.0))),
    )
    .await;
    assert_eq!(deleted.status, StatusCode::OK, "{}", deleted.body);
    assert_eq!(
        deleted.json()["o:filename"],
        format!("{MEDIA_STORAGE_ID}.jpg")
    );
    assert!(
        !store.exists(&original).unwrap(),
        "the original survived at {original}"
    );
    for path in &thumbnails {
        assert!(
            !store.exists(path).unwrap(),
            "the thumbnail survived at {path}"
        );
    }
}

/// `AclFactory`: no role below the two admin roles holds `delete` on
/// `ItemAdapter`, so an anonymous delete never reaches the row.
#[tokio::test]
async fn an_anonymous_delete_is_403() {
    let (app, seed) = seeded_router().await;
    let answer = send(
        app,
        request("DELETE", &format!("/api/items/{}", seed.item.0)),
    )
    .await;
    assert_eq!(answer.status, StatusCode::FORBIDDEN);
    assert_eq!(
        answer.body,
        r#"{"errors":{"error":"Permission denied for the current user to delete the Omeka\\Api\\Adapter\\ItemAdapter resource."}}"#
    );
}

/// `AclFactory::addRulesForAuthor`: `['update', 'delete']` on the resource
/// entities carries `OwnsEntityAssertion`, so an author may not delete another
/// user's item and the refusal names the entity class.
#[tokio::test]
async fn an_author_cannot_delete_another_users_item() {
    let (pool, seed) = seeded_pool().await;
    let (_id, identity, credential) = keyed_user(&pool, "author").await;
    let app = common::router(pool);
    let uri = format!(
        "/api/items/{}?key_identity={identity}&key_credential={credential}",
        seed.item.0
    );
    let answer = send(app, request("DELETE", &uri)).await;
    assert_eq!(answer.status, StatusCode::FORBIDDEN);
    assert_eq!(
        answer.body,
        r#"{"errors":{"error":"Permission denied for the current user to delete the Omeka\\Entity\\Item resource."}}"#
    );
}

/// The same assertion passes on ownership, so an author may delete their own
/// item.
#[tokio::test]
async fn an_author_may_delete_their_own_item() {
    let (pool, seed) = seeded_pool().await;
    let (author, identity, credential) = keyed_user(&pool, "author").await;
    let item = omeka_db::repos::ItemRepo::new(&pool)
        .create(&omeka_db::rows::NewItem {
            resource: omeka_db::rows::NewResource {
                owner_id: Some(omeka_db::ids::UserId(author)),
                resource_class_id: None,
                resource_template_id: None,
                thumbnail_id: None,
                title: Some("Owned by the author".to_string()),
                is_public: true,
                created: None,
            },
            primary_media_id: None,
        })
        .await
        .unwrap();
    let app = common::router(pool);
    let uri = format!(
        "/api/items/{}?key_identity={identity}&key_credential={credential}",
        item.id().0
    );
    let answer = send(app.clone(), request("DELETE", &uri)).await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    assert!(
        answer.json()["o:id"].is_null(),
        "the detached item kept its id: {}",
        answer.body
    );

    // The admin's item is untouched.
    let other = send(app, get(&format!("/api/items/{}", seed.item.0), false)).await;
    assert_eq!(other.status, StatusCode::OK);
}

/// `Entity/Site.php:88`: `$pages` cascades remove, and each page takes its
/// blocks, so deleting a site leaves no page or block rows.
#[tokio::test]
async fn deleting_a_site_removes_its_pages_and_blocks() {
    let (pool, seed) = site_pool().await;
    let app = common::router(pool.clone());
    let pages_before = count(
        &pool,
        "SELECT COUNT(*) FROM site_page WHERE site_id = ?",
        seed.site,
    )
    .await;
    assert!(pages_before > 0, "the fixture site has no pages");

    let deleted = send(
        app.clone(),
        request("DELETE", &keyed(&format!("/api/sites/{}", seed.site))),
    )
    .await;
    assert_eq!(deleted.status, StatusCode::OK, "{}", deleted.body);
    assert_eq!(
        deleted.json()["o:page"].as_array().map(Vec::len),
        Some(pages_before as usize),
        "the answer should carry the pages it removed"
    );

    assert_eq!(
        count(
            &pool,
            "SELECT COUNT(*) FROM site_page WHERE site_id = ?",
            seed.site
        )
        .await,
        0,
        "a page survived"
    );
    assert_eq!(
        count_all(&pool, "SELECT COUNT(*) FROM site_page_block").await,
        0,
        "a block survived"
    );
    let read = send(app, get(&format!("/api/sites/{}", seed.site), false)).await;
    assert_eq!(read.status, StatusCode::NOT_FOUND);
}

/// A site page delete answers with the page and leaves the site; a non-public
/// page is deletable by its owner even though a read hides it.
#[tokio::test]
async fn deleting_a_private_site_page_leaves_its_site() {
    let (pool, seed) = site_pool().await;
    let app = common::router(pool.clone());
    let page: i64 = sqlx::query_scalar(
        "SELECT id FROM site_page WHERE site_id = ? AND is_public = 0 ORDER BY id",
    )
    .bind(seed.site as i64)
    .fetch_one(&pool)
    .await
    .unwrap();

    let deleted = send(
        app.clone(),
        request("DELETE", &keyed(&format!("/api/site_pages/{page}"))),
    )
    .await;
    assert_eq!(deleted.status, StatusCode::OK, "{}", deleted.body);
    assert!(
        deleted.json()["o:id"].is_null(),
        "the detached page kept its id: {}",
        deleted.body
    );
    assert_eq!(deleted.json()["o:is_public"], false);

    let read = send(app.clone(), get(&format!("/api/site_pages/{page}"), false)).await;
    assert_eq!(read.status, StatusCode::NOT_FOUND);
    assert_eq!(
        count(
            &pool,
            "SELECT COUNT(*) FROM site_page_block WHERE page_id = ?",
            page as u64
        )
        .await,
        0,
        "a block survived"
    );
    let site = send(app, get(&format!("/api/sites/{}", seed.site), false)).await;
    assert_eq!(site.status, StatusCode::OK);
}

/// An item set delete takes its values and its memberships.
#[tokio::test]
async fn deleting_an_item_set_removes_its_values_and_memberships() {
    let (pool, seed) = seeded_pool().await;
    let app = common::router(pool.clone());
    let deleted = send(
        app.clone(),
        request(
            "DELETE",
            &keyed(&format!("/api/item_sets/{}", seed.item_set.0)),
        ),
    )
    .await;
    assert_eq!(deleted.status, StatusCode::OK, "{}", deleted.body);
    assert_eq!(
        count(
            &pool,
            "SELECT COUNT(*) FROM value WHERE resource_id = ?",
            seed.item_set.0
        )
        .await,
        0,
        "the item set kept a value"
    );
    assert_eq!(
        count(
            &pool,
            "SELECT COUNT(*) FROM item_item_set WHERE item_set_id = ?",
            seed.item_set.0
        )
        .await,
        0,
        "a membership survived"
    );
    let read = send(
        app,
        get(&format!("/api/item_sets/{}", seed.item_set.0), false),
    )
    .await;
    assert_eq!(read.status, StatusCode::NOT_FOUND);
}

/// `AclFactory::addRulesForGlobalAdmin` denies `Entity\User` `delete` to
/// `IsSelfAssertion`, so an admin cannot remove their own account.
#[tokio::test]
async fn a_global_admin_cannot_delete_themselves() {
    let (app, seed) = seeded_router().await;
    let answer = send(
        app.clone(),
        request("DELETE", &keyed(&format!("/api/users/{}", seed.user.0))),
    )
    .await;
    assert_eq!(answer.status, StatusCode::FORBIDDEN);
    assert_eq!(
        answer.body,
        r#"{"errors":{"error":"Permission denied for the current user to delete the Omeka\\Entity\\User resource."}}"#
    );
    // The account is still there.
    let read = send(app, get(&format!("/api/users/{}", seed.user.0), true)).await;
    assert_eq!(read.status, StatusCode::OK);
}

/// `AclFactory::addRulesForSiteAdmin` denies `Entity\User` `delete` to
/// `UserIsAdminAssertion` combined with "not self", so a supervisor cannot
/// remove another admin.
#[tokio::test]
async fn a_site_admin_cannot_delete_another_admin() {
    let (pool, seed) = seeded_pool().await;
    let (_id, identity, credential) = keyed_user(&pool, "site_admin").await;
    let app = common::router(pool);
    let uri = format!(
        "/api/users/{}?key_identity={identity}&key_credential={credential}",
        seed.user.0
    );
    let answer = send(app, request("DELETE", &uri)).await;
    assert_eq!(answer.status, StatusCode::FORBIDDEN);
    assert_eq!(
        answer.body,
        r#"{"errors":{"error":"Permission denied for the current user to delete the Omeka\\Entity\\User resource."}}"#
    );
}

/// `Entity/User.php:59`: the keys cascade remove, and every row the user owned
/// keeps its data and loses the reference (`resource.owner_id` is
/// `ON DELETE SET NULL`).
///
/// Upstream has no refusal for a user who still owns resources; the brief's
/// claim that one exists did not survive the PHP. The caller is a second
/// global admin, because an admin may not delete their own account.
#[tokio::test]
async fn deleting_a_user_removes_its_keys_and_clears_the_owner() {
    let (pool, seed) = seeded_pool().await;
    let (_admin, identity, credential) = keyed_user(&pool, "global_admin").await;
    let app = common::router(pool.clone());
    let uri = format!(
        "/api/users/{}?key_identity={identity}&key_credential={credential}",
        seed.user.0
    );
    let deleted = send(app.clone(), request("DELETE", &uri)).await;
    assert_eq!(deleted.status, StatusCode::OK, "{}", deleted.body);
    assert!(
        deleted.json()["o:id"].is_null(),
        "the detached user kept its id: {}",
        deleted.body
    );
    assert_eq!(
        count(
            &pool,
            "SELECT COUNT(*) FROM api_key WHERE owner_id = ?",
            seed.user.0
        )
        .await,
        0,
        "an API key survived"
    );
    let owner: Option<i64> = sqlx::query_scalar("SELECT owner_id FROM resource WHERE id = ?")
        .bind(seed.item.0 as i64)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(owner, None, "the item kept its owner");
}

/// Only the admin roles hold `delete` on `UserAdapter`, so an editor is refused
/// at the adapter level.
#[tokio::test]
async fn an_editor_cannot_delete_a_user() {
    let (pool, seed) = seeded_pool().await;
    let (_id, identity, credential) = keyed_user(&pool, "editor").await;
    let app = common::router(pool);
    let uri = format!(
        "/api/users/{}?key_identity={identity}&key_credential={credential}",
        seed.user.0
    );
    let answer = send(app, request("DELETE", &uri)).await;
    assert_eq!(answer.status, StatusCode::FORBIDDEN);
    assert_eq!(
        answer.body,
        r#"{"errors":{"error":"Permission denied for the current user to delete the Omeka\\Api\\Adapter\\UserAdapter resource."}}"#
    );
}

/// A missing row is the same 404 a read gives, whatever the resource.
#[tokio::test]
async fn deleting_a_missing_row_is_404() {
    let (app, _seed) = seeded_router().await;
    for resource in [
        "items",
        "item_sets",
        "media",
        "sites",
        "site_pages",
        "users",
    ] {
        let answer = send(
            app.clone(),
            request("DELETE", &keyed(&format!("/api/{resource}/999999"))),
        )
        .await;
        assert_eq!(
            answer.status,
            StatusCode::NOT_FOUND,
            "{resource}: {}",
            answer.body
        );
    }
}

/// An adapter with no delete handler in this unit keeps its typed 501.
#[tokio::test]
async fn an_adapter_without_a_delete_handler_stays_501() {
    let (app, _seed) = seeded_router().await;
    let answer = send(app, request("DELETE", &keyed("/api/modules/Common"))).await;
    assert_eq!(answer.status, StatusCode::NOT_IMPLEMENTED);
    assert_eq!(
        answer.body,
        r#"{"errors":{"error":"The modules adapter does not implement the delete operation."}}"#
    );
}
