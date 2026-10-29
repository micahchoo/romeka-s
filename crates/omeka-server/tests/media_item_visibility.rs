//! `GET /api/media?romeka_item_is_public=<0|1>`: the media whose item carries that visibility.
//!
//! The public site lists a public media of a private item, but the file route refuses its files to
//! a visitor, who cannot open its item either. The site check asks this filter, with `is_public=1`,
//! to find a media list that shows a visitor such media.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::http::StatusCode;
use common::{KEY_CREDENTIAL, KEY_IDENTITY, request, router, seeded_pool, send};
use omeka_db::ids::{ItemId, UserId};
use omeka_db::repos::{ItemRepo, MediaRepo};
use omeka_db::rows::{NewItem, NewMedia, NewResource};
use sqlx::SqlitePool;

fn resource(owner: UserId, is_public: bool) -> NewResource {
    NewResource {
        owner_id: Some(owner),
        title: Some("a record".to_string()),
        is_public,
        ..NewResource::default()
    }
}

async fn item(pool: &SqlitePool, owner: UserId, is_public: bool) -> ItemId {
    ItemRepo::new(pool)
        .create(&NewItem {
            resource: resource(owner, is_public),
            primary_media_id: None,
        })
        .await
        .unwrap()
        .id()
}

async fn media(pool: &SqlitePool, item: ItemId, owner: UserId, is_public: bool) -> u64 {
    MediaRepo::new(pool)
        .create(&NewMedia {
            resource: resource(owner, is_public),
            item_id: item,
            ingester: "html".to_string(),
            renderer: "html".to_string(),
            ..NewMedia::default()
        })
        .await
        .unwrap()
        .id()
        .0
}

async fn ids(app: &axum::Router, query: &str) -> Vec<u64> {
    let uri = format!(
        "/api/media?{query}&key_identity={KEY_IDENTITY}&key_credential={KEY_CREDENTIAL}&sort_by=id"
    );
    let answer = send(app.clone(), request("GET", &uri)).await;
    assert_eq!(answer.status, StatusCode::OK, "{uri}: {}", answer.body);
    let mut ids: Vec<u64> = answer
        .json()
        .as_array()
        .expect("a list")
        .iter()
        .map(|media| media["o:id"].as_u64().expect("o:id"))
        .collect();
    ids.sort_unstable();
    ids
}

#[tokio::test]
async fn the_filter_reads_the_media_item_visibility_not_its_own() {
    let (pool, seed) = seeded_pool().await;
    let public_item = item(&pool, seed.user, true).await;
    let private_item = item(&pool, seed.user, false).await;
    let public_of_public = media(&pool, public_item, seed.user, true).await;
    let public_of_private = media(&pool, private_item, seed.user, true).await;
    let private_of_private = media(&pool, private_item, seed.user, false).await;
    let app = router(pool);
    let mine = |found: Vec<u64>| {
        found
            .into_iter()
            .filter(|id| [public_of_public, public_of_private, private_of_private].contains(id))
            .collect::<Vec<_>>()
    };

    assert_eq!(
        mine(ids(&app, "romeka_item_is_public=0").await),
        [public_of_private, private_of_private]
    );
    assert_eq!(
        mine(ids(&app, "romeka_item_is_public=1").await),
        [public_of_public]
    );
    assert_eq!(
        mine(ids(&app, "is_public=1&romeka_item_is_public=0").await),
        [public_of_private],
        "what the site check asks: a public media a visitor cannot open"
    );
    assert_eq!(
        mine(ids(&app, "romeka_item_is_public=maybe").await),
        [public_of_public, public_of_private, private_of_private],
        "a value that is not a flag filters nothing, as `is_public` does"
    );
}
