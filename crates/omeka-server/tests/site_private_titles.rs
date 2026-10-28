//! A title drawn from a private value, read on the public site by a visitor.
//!
//! `ResourceTitleHydrator` stores a resource's first title value as its `title` whatever that
//! value's visibility, and upstream's site prints that column. The port's API shows each caller
//! the first title value they may see (`tests/private_values.rs`). The public site shows only
//! public values, so every surface that names a resource shows its first public title value, or
//! the untitled text when it has none, and never the private one.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::http::StatusCode;
use common::{Answer, Seed, get, header, router, router_with_store, send, site_pool};
use omeka_db::ids::{ItemId, ItemSetId, MediaId, PropertyId, ResourceId, SiteId};
use omeka_db::repos::{
    FulltextRepo, ItemRepo, ItemSetRepo, MediaRepo, PropertyRepo, SiteBlockAttachmentRepo,
    SiteRepo, ValueRepo,
};
use omeka_db::rows::{
    FulltextSearchRow, NewItem, NewItemSet, NewMedia, NewProperty, NewResource, NewSiteBlock,
    NewSiteBlockAttachment, NewSitePage, NewValue,
};
use omeka_media::store::{FileStore, Store};
use sqlx::SqlitePool;

/// Every private title in the world. No public value contains one of them.
const SECRETS: [&str; 4] = ["Hiddentitle", "Lonelysecret", "Setsecret", "Mediasecret"];

struct World {
    pool: SqlitePool,
    titled: ItemId,
    untitled: ItemId,
    linker: ItemId,
    item_set: ItemSetId,
    media: MediaId,
}

async fn dcterms_title(pool: &SqlitePool) -> PropertyId {
    let id: i64 = sqlx::query_scalar(
        "SELECT p.id FROM property p JOIN vocabulary v ON v.id = p.vocabulary_id \
         WHERE v.prefix = 'dcterms' AND p.local_name = 'title'",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    PropertyId(u64::try_from(id).unwrap())
}

async fn literal(
    pool: &SqlitePool,
    resource: ResourceId,
    property: PropertyId,
    text: &str,
    is_public: bool,
) {
    ValueRepo::new(pool)
        .create(&NewValue {
            resource_id: resource,
            property_id: property,
            value_resource_id: None,
            value_annotation_id: None,
            r#type: "literal".to_string(),
            lang: None,
            value: Some(text.to_string()),
            uri: None,
            is_public,
        })
        .await
        .unwrap();
}

async fn link(pool: &SqlitePool, resource: ResourceId, property: PropertyId, to: ResourceId) {
    ValueRepo::new(pool)
        .create(&NewValue {
            resource_id: resource,
            property_id: property,
            value_resource_id: Some(to),
            value_annotation_id: None,
            r#type: "resource:item".to_string(),
            lang: None,
            value: None,
            uri: None,
            is_public: true,
        })
        .await
        .unwrap();
}

/// A public resource whose stored title is its first title value, as the hydrator stores it.
fn stored(titles: &[(&str, bool)]) -> NewResource {
    NewResource {
        title: titles.first().map(|(text, _)| (*text).to_string()),
        is_public: true,
        ..NewResource::default()
    }
}

/// A public item on the site carrying these title values, in order.
async fn item(pool: &SqlitePool, seed: &Seed, titles: &[(&str, bool)]) -> ItemId {
    let id = ItemRepo::new(pool)
        .create(&NewItem {
            resource: stored(titles),
            primary_media_id: None,
        })
        .await
        .unwrap()
        .id();
    let title = dcterms_title(pool).await;
    for (text, is_public) in titles {
        literal(pool, id.as_resource(), title, text, *is_public).await;
    }
    ItemRepo::new(pool)
        .add_to_site(id, SiteId(seed.site))
        .await
        .unwrap();
    id
}

async fn world() -> World {
    let (pool, seed) = site_pool().await;
    let title = dcterms_title(&pool).await;
    let titled = item(
        &pool,
        &seed,
        &[("Hiddentitle", false), ("Shown title", true)],
    )
    .await;
    let untitled = item(&pool, &seed, &[("Lonelysecret", false)]).await;
    let linker = item(&pool, &seed, &[("Linker", true)]).await;
    let relation = PropertyRepo::new(&pool)
        .create(&NewProperty {
            owner_id: None,
            vocabulary_id: seed.vocabulary,
            local_name: "relation".to_string(),
            label: "Relation".to_string(),
            comment: None,
        })
        .await
        .unwrap()
        .id;
    link(&pool, linker.as_resource(), relation, titled.as_resource()).await;
    link(
        &pool,
        linker.as_resource(),
        relation,
        untitled.as_resource(),
    )
    .await;

    let set_titles = [("Setsecret", false), ("Shown set", true)];
    let item_set = ItemSetRepo::new(&pool)
        .create(&NewItemSet {
            resource: stored(&set_titles),
            is_open: true,
        })
        .await
        .unwrap()
        .id();
    for (text, is_public) in set_titles {
        literal(&pool, item_set.as_resource(), title, text, is_public).await;
    }
    ItemRepo::new(&pool)
        .add_to_item_set(titled, item_set)
        .await
        .unwrap();
    SiteRepo::new(&pool)
        .add_item_set(SiteId(seed.site), item_set, Some(2))
        .await
        .unwrap();

    let media = MediaRepo::new(&pool)
        .create(&NewMedia {
            resource: stored(&[("Mediasecret", false)]),
            item_id: titled,
            ingester: "upload".to_string(),
            renderer: "file".to_string(),
            data: None,
            source: Some("letter-scan.jpg".to_string()),
            media_type: Some("image/jpeg".to_string()),
            storage_id: Some("7d3f6c0f1a0a5a6b9a7c3d4e5f6a7b8c9d0e1f2a".to_string()),
            extension: Some("jpg".to_string()),
            sha256: Some("abc123".to_string()),
            size: Some(1234),
            has_original: true,
            has_thumbnails: true,
            position: Some(1),
            lang: None,
            alt_text: None,
        })
        .await
        .unwrap()
        .id();
    literal(&pool, media.as_resource(), title, "Mediasecret", false).await;

    // The fulltext index answers everyone, so it holds the public title.
    FulltextRepo::new(&pool)
        .save(&FulltextSearchRow {
            id: titled.as_resource(),
            resource: "items".to_string(),
            owner_id: None,
            is_public: true,
            title: Some("Shown title".to_string()),
            text: Some("Shown title".to_string()),
        })
        .await
        .unwrap();

    let site = SiteRepo::new(&pool);
    let page = site
        .add_page(&NewSitePage {
            site_id: SiteId(seed.site),
            slug: "gallery".to_string(),
            title: "Gallery".to_string(),
            is_public: true,
            ..NewSitePage::default()
        })
        .await
        .unwrap();
    let block = site
        .add_block(&NewSiteBlock {
            page_id: page.id,
            layout: "media".to_string(),
            data: serde_json::json!({"media_display": "thumbnail", "show_title_option": "item_title"}),
            layout_data: None,
            position: 1,
        })
        .await
        .unwrap();
    for (position, attached) in [titled, untitled].into_iter().enumerate() {
        SiteBlockAttachmentRepo::new(&pool)
            .create(&NewSiteBlockAttachment {
                block_id: block.id,
                item_id: Some(attached),
                media_id: None,
                caption: String::new(),
                position: position as i64 + 1,
            })
            .await
            .unwrap();
    }

    World {
        pool,
        titled,
        untitled,
        linker,
        item_set,
        media,
    }
}

async fn visit(world: &World, uri: &str) -> Answer {
    let answer = send(router(world.pool.clone()), get(uri, false)).await;
    assert_eq!(answer.status, StatusCode::OK, "{uri}: {}", answer.body);
    for secret in SECRETS {
        assert!(
            !answer.body.contains(secret),
            "{uri} prints the private title {secret}: {}",
            answer.body
        );
    }
    answer
}

fn assert_has(answer: &Answer, uri: &str, text: &str) {
    assert!(
        answer.body.contains(text),
        "{uri} lacks {text}: {}",
        answer.body
    );
}

#[tokio::test]
async fn every_public_site_surface_names_a_resource_by_its_public_title() {
    let world = world().await;
    let titled = world.titled.0;
    let untitled = world.untitled.0;

    let uri = format!("/s/museum/item/{titled}");
    let page = visit(&world, &uri).await;
    assert_has(&page, &uri, "<title>Shown title");
    assert_has(&page, &uri, "<h2 class=\"resource-title\">Shown title</h2>");
    assert_has(&page, &uri, "\"o:title\":\"Shown title\"");
    assert_has(&page, &uri, "Shown set");
    // A media whose only title is private reads as its file name.
    assert_has(&page, &uri, "letter-scan.jpg");

    let uri = format!("/s/museum/item/{untitled}");
    let page = visit(&world, &uri).await;
    assert_has(&page, &uri, "<title>[Untitled]");
    assert_has(&page, &uri, "\"o:title\":null");

    let uri = format!("/s/museum/item/{}", world.linker.0);
    let page = visit(&world, &uri).await;
    assert_has(
        &page,
        &uri,
        "<span class=\"resource-name\">Shown title</span></a>",
    );
    assert_has(
        &page,
        &uri,
        "<span class=\"resource-name\">[Untitled]</span></a>",
    );

    for uri in [
        "/s/museum/item".to_string(),
        "/s/museum/page/gallery".to_string(),
        "/s/museum/search?fulltext_search=Shown".to_string(),
        "/s/museum/item?fulltext_search=Shown".to_string(),
    ] {
        let page = visit(&world, &uri).await;
        assert_has(&page, &uri, "Shown title");
    }

    for uri in [
        "/s/museum/item-set".to_string(),
        format!("/s/museum/item-set/{}", world.item_set.0),
        "/s/museum/item/search".to_string(),
    ] {
        let page = visit(&world, &uri).await;
        assert_has(&page, &uri, "Shown set");
    }

    let uri = format!("/s/museum/media/{}", world.media.0);
    let page = visit(&world, &uri).await;
    assert_has(&page, &uri, "letter-scan.jpg");
}

/// A title sort shelves each item by the title the visitor is shown: "Shown title" after
/// "Linker", where the stored "Hiddentitle" would shelve it before. A sort by the title
/// property reads public values only, so it agrees.
#[tokio::test]
async fn the_site_title_sort_orders_by_the_public_title() {
    let world = world().await;
    for uri in [
        "/s/museum/item?sort_by=title&sort_order=asc",
        "/s/museum/item?sort_by=dcterms:title&sort_order=asc",
    ] {
        let page = visit(&world, uri).await;
        let at = |id: ItemId| {
            page.body
                .find(&format!("href=\"/s/museum/item/{}\"", id.0))
                .unwrap_or_else(|| panic!("{uri} lists item {}: {}", id.0, page.body))
        };
        assert!(at(world.linker) < at(world.titled), "{uri}: {}", page.body);
    }
}

/// A file whose source names no file downloads under the media's display name, which must not
/// be a private title.
#[tokio::test]
async fn a_download_name_never_carries_a_private_title() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = FileStore::new(dir.path(), "/files");
    let (pool, seed) = site_pool().await;
    store
        .put(b"scan", "original/unnamed.pdf", Some("application/pdf"))
        .unwrap();
    let media = MediaRepo::new(&pool)
        .create(&NewMedia {
            resource: stored(&[("Mediasecret", false)]),
            item_id: seed.item,
            ingester: "upload".to_string(),
            renderer: "file".to_string(),
            data: None,
            source: Some("   ".to_string()),
            media_type: Some("application/pdf".to_string()),
            storage_id: Some("unnamed".to_string()),
            extension: Some("pdf".to_string()),
            sha256: Some("unnamed".to_string()),
            size: Some(4),
            has_original: true,
            has_thumbnails: false,
            position: None,
            lang: None,
            alt_text: None,
        })
        .await
        .unwrap()
        .id();
    let title = dcterms_title(&pool).await;
    literal(&pool, media.as_resource(), title, "Mediasecret", false).await;

    let uri = format!("/s/museum/media/{}/download", media.0);
    let answer = send(router_with_store(pool, store), get(&uri, false)).await;
    assert_eq!(answer.status, StatusCode::OK, "{uri}: {}", answer.body);
    assert_eq!(
        header(&answer, "content-disposition"),
        Some("attachment; filename=\"[Untitled].pdf\"; filename*=UTF-8''%5BUntitled%5D.pdf")
    );
}
