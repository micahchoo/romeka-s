//! `site` and its children: pages, blocks, attachments, item sets and permissions, including the
//! Doctrine-shaped delete the repository performs.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use omeka_db::DbError;
use omeka_db::ids::{PageId, UserId};
use omeka_db::repos::SiteSettingRepo;
use omeka_db::repos::{
    ItemRepo, ItemSetRepo, ItemSiteRepo, PageVersionRepo, SiteFilter, SiteRepo, UserRepo,
};
use omeka_db::rows::{
    ItemSiteRow, NewFirstPage, NewFirstPageBlock, NewItem, NewItemSet, NewSite, NewSiteBlock,
    NewSitePage, NewUser, NewWholeSite, PageImage, PageVersionOrigin,
};
use sqlx::SqlitePool;

async fn new_site(pool: &SqlitePool, slug: &str) -> omeka_db::rows::SiteRow {
    SiteRepo::new(pool)
        .create(&NewSite {
            slug: slug.to_string(),
            theme: "default".to_string(),
            title: "My site".to_string(),
            summary: Some("Port of a heritage catalogue".to_string()),
            navigation: serde_json::json!([{"type": "page", "data": {"id": 1}}]),
            item_pool: serde_json::json!([{"field": "resource_class_id", "type": "in", "value": [1]}]),
            ..Default::default()
        })
        .await
        .unwrap()
}

#[tokio::test]
async fn site_roundtrip_with_pages_and_blocks() {
    let pool = common::memory().await;
    let sites = SiteRepo::new(&pool);
    let site = new_site(&pool, "heritage").await;
    assert_eq!(sites.get(site.id).await.unwrap(), Some(site.clone()));
    assert_eq!(
        sites.get_by_slug("HERITAGE").await.unwrap(),
        Some(site.clone())
    );

    let page = sites
        .add_page(&NewSitePage {
            site_id: site.id,
            slug: "about".to_string(),
            title: "About".to_string(),
            is_public: true,
            layout: None,
            layout_data: None,
            ..Default::default()
        })
        .await
        .unwrap();
    let block = sites
        .add_block(&NewSiteBlock {
            page_id: page.id,
            layout: "html".to_string(),
            data: serde_json::json!({"html": "<p>Hello</p>"}),
            layout_data: Some(serde_json::json!({"grid": "full"})),
            position: 1,
        })
        .await
        .unwrap();
    assert_eq!(sites.list_pages(site.id).await.unwrap(), vec![page.clone()]);
    assert_eq!(sites.get_page(page.id).await.unwrap(), Some(page.clone()));
    assert_eq!(
        sites.list_blocks(page.id).await.unwrap(),
        vec![block.clone()]
    );
    assert_eq!(
        sites.get_block(block.id).await.unwrap(),
        Some(block.clone())
    );

    let mut site = site;
    site.homepage_id = Some(page.id);
    site.title = "My site, restored".to_string();
    assert!(sites.update(&site).await.unwrap());
    let after = sites.get(site.id).await.unwrap().unwrap();
    assert_eq!(after.homepage_id, Some(page.id));
    assert_eq!(after.title, "My site, restored");
    assert!(
        after.modified.unwrap() >= site.created,
        "update moves modified"
    );

    let mut page = page;
    page.title = "About us".to_string();
    page.layout_data = Some(serde_json::json!({"class": "page"}));
    assert!(sites.update_page(&page).await.unwrap());
    assert_eq!(
        sites.get_page(page.id).await.unwrap().unwrap().title,
        "About us"
    );

    let mut block = block;
    block.position = 2;
    assert!(sites.update_block(&block).await.unwrap());
    assert_eq!(
        sites.get_block(block.id).await.unwrap().unwrap().position,
        2
    );
}

#[tokio::test]
async fn site_filters_narrow_by_owner_slug_exclusion_and_flag() {
    let pool = common::memory().await;
    let owner = UserRepo::new(&pool)
        .create(&NewUser {
            email: "owner@example.org".to_string(),
            name: "Owner".to_string(),
            password_hash: None,
            role: "editor".to_string(),
            is_active: true,
            created: None,
        })
        .await
        .unwrap();
    let sites = SiteRepo::new(&pool);
    let mut mine = new_site(&pool, "mine").await;
    mine.owner_id = Some(owner.id);
    mine.assign_new_items = true;
    assert!(sites.update(&mine).await.unwrap());
    let not_mine = new_site(&pool, "not-mine").await;

    let owned = sites
        .list(
            &SiteFilter {
                owner_id: Some(owner.id),
                ..Default::default()
            },
            &Default::default(),
        )
        .await
        .unwrap();
    assert_eq!(
        owned.iter().map(|row| row.id).collect::<Vec<_>>(),
        vec![mine.id]
    );

    let excluding = sites
        .list(
            &SiteFilter {
                owner_id: Some(owner.id),
                exclude_id: Some(mine.id),
                ..Default::default()
            },
            &Default::default(),
        )
        .await
        .unwrap();
    assert!(excluding.is_empty(), "exclude_id skips the site itself");

    let assigning = sites
        .list(
            &SiteFilter {
                assign_new_items: Some(true),
                ..Default::default()
            },
            &Default::default(),
        )
        .await
        .unwrap();
    assert_eq!(
        assigning.iter().map(|row| row.id).collect::<Vec<_>>(),
        vec![mine.id]
    );
    assert_ne!(not_mine.id, mine.id);
}

#[tokio::test]
async fn site_slug_is_unique_case_insensitively() {
    let pool = common::memory().await;
    new_site(&pool, "heritage").await;
    let error = SiteRepo::new(&pool)
        .create(&NewSite {
            slug: "HERITAGE".to_string(),
            theme: "default".to_string(),
            title: "Another".to_string(),
            ..Default::default()
        })
        .await
        .unwrap_err();
    assert!(error.is_unique_violation(), "{error}");
}

#[tokio::test]
async fn site_item_sets_upsert_position_and_order_as_mysql_sorts_them() {
    let pool = common::memory().await;
    let site = new_site(&pool, "heritage").await;
    let sites = SiteRepo::new(&pool);
    let item_sets = ItemSetRepo::new(&pool);
    let last = item_sets.create(&NewItemSet::default()).await.unwrap();
    let first = item_sets.create(&NewItemSet::default()).await.unwrap();
    let unpositioned = item_sets.create(&NewItemSet::default()).await.unwrap();

    sites
        .add_item_set(site.id, last.id(), Some(3))
        .await
        .unwrap();
    sites
        .add_item_set(site.id, first.id(), Some(1))
        .await
        .unwrap();
    sites
        .add_item_set(site.id, unpositioned.id(), None)
        .await
        .unwrap();
    sites
        .add_item_set(site.id, last.id(), Some(2))
        .await
        .unwrap();

    let listed = sites.list_item_sets(site.id).await.unwrap();
    assert_eq!(
        listed.iter().map(|row| row.item_set_id).collect::<Vec<_>>(),
        vec![unpositioned.id(), first.id(), last.id()],
        "position ascending with a positional update; MySQL sorts the NULL first"
    );
    assert_eq!(listed[0].position, None);
    assert_eq!(listed[1].position, Some(1));
    assert_eq!(listed[2].position, Some(2));

    assert!(sites.remove_item_set(site.id, first.id()).await.unwrap());
    assert!(!sites.remove_item_set(site.id, first.id()).await.unwrap());
    assert_eq!(sites.list_item_sets(site.id).await.unwrap().len(), 2);
}

#[tokio::test]
async fn site_permissions_upsert_the_role() {
    let pool = common::memory().await;
    let site = new_site(&pool, "heritage").await;
    let user = UserRepo::new(&pool)
        .create(&NewUser {
            email: "editor@example.org".to_string(),
            name: "Editor".to_string(),
            password_hash: None,
            role: "editor".to_string(),
            is_active: true,
            created: None,
        })
        .await
        .unwrap();
    let sites = SiteRepo::new(&pool);
    sites
        .set_permission(site.id, user.id, "viewer")
        .await
        .unwrap();
    sites
        .set_permission(site.id, user.id, "site_admin")
        .await
        .unwrap();

    let permissions = sites.list_permissions(site.id).await.unwrap();
    assert_eq!(permissions.len(), 1);
    assert_eq!(permissions[0].user_id, user.id);
    assert_eq!(permissions[0].role, "site_admin");

    assert!(sites.remove_permission(site.id, user.id).await.unwrap());
    assert!(sites.list_permissions(site.id).await.unwrap().is_empty());
}

#[tokio::test]
async fn deleting_a_site_removes_every_child_row() {
    let pool = common::memory().await;
    let sites = SiteRepo::new(&pool);
    let site = new_site(&pool, "heritage").await;
    let page = sites
        .add_page(&NewSitePage {
            site_id: site.id,
            slug: "about".to_string(),
            title: "About".to_string(),
            ..Default::default()
        })
        .await
        .unwrap();
    let block = sites
        .add_block(&NewSiteBlock {
            page_id: page.id,
            layout: "html".to_string(),
            data: serde_json::json!({}),
            layout_data: None,
            position: 1,
        })
        .await
        .unwrap();
    // The attachment and settings tables have no repository yet: seed them directly so the site
    // delete has something to clear.
    sqlx::query("INSERT INTO site_block_attachment (block_id, caption, position) VALUES (?, ?, ?)")
        .bind(block.id.0 as i64)
        .bind("caption")
        .bind(1)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO site_setting (id, site_id, value) VALUES (?, ?, ?)")
        .bind("default_locale")
        .bind(site.id.0 as i64)
        .bind("\"en\"")
        .execute(&pool)
        .await
        .unwrap();
    let item_set = ItemSetRepo::new(&pool)
        .create(&NewItemSet::default())
        .await
        .unwrap();
    sites
        .add_item_set(site.id, item_set.id(), Some(1))
        .await
        .unwrap();
    let user = UserRepo::new(&pool)
        .create(&NewUser {
            email: "editor@example.org".to_string(),
            name: "Editor".to_string(),
            password_hash: None,
            role: "editor".to_string(),
            is_active: true,
            created: None,
        })
        .await
        .unwrap();
    sites
        .set_permission(site.id, user.id, "editor")
        .await
        .unwrap();
    let item = ItemRepo::new(&pool)
        .create(&NewItem::default())
        .await
        .unwrap();

    assert!(sites.delete(site.id).await.unwrap());

    assert!(sites.get(site.id).await.unwrap().is_none());
    for (table, column, expected) in [
        ("site_page", "site_id", 0i64),
        ("site_item_set", "site_id", 0),
        ("site_permission", "site_id", 0),
        ("site_setting", "site_id", 0),
    ] {
        let count: i64 =
            sqlx::query_scalar(&format!("SELECT count(*) FROM {table} WHERE {column} = ?"))
                .bind(site.id.0 as i64)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(count, expected, "{table} left rows behind");
    }
    let attachments: i64 = sqlx::query_scalar("SELECT count(*) FROM site_block_attachment")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(attachments, 0);
    let blocks: i64 = sqlx::query_scalar("SELECT count(*) FROM site_page_block")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(blocks, 0);
    // No row outside the site is touched.
    assert!(
        ItemSetRepo::new(&pool)
            .get(item_set.id())
            .await
            .unwrap()
            .is_some()
    );
    assert!(UserRepo::new(&pool).get(user.id).await.unwrap().is_some());
    assert!(ItemRepo::new(&pool).get(item.id()).await.unwrap().is_some());
}

#[tokio::test]
async fn item_site_memberships_attach_once_and_list_in_order() {
    let pool = common::memory().await;
    let site = new_site(&pool, "heritage").await;
    let item = ItemRepo::new(&pool)
        .create(&NewItem::default())
        .await
        .unwrap();
    let memberships = ItemSiteRepo::new(&pool);
    let row = ItemSiteRow {
        item_id: item.id(),
        site_id: site.id,
    };

    memberships.create(&row).await.unwrap();
    memberships.create(&row).await.unwrap();

    assert_eq!(
        memberships.list().await.unwrap(),
        vec![row],
        "attaching twice keeps one membership"
    );
}

fn link_first_page(page: PageId) -> serde_json::Value {
    serde_json::json!([{ "type": "page", "data": { "id": page.0 }, "links": [] }])
}

fn layouts(image: &PageImage) -> Result<serde_json::Value, DbError> {
    Ok(serde_json::json!(
        image
            .blocks
            .iter()
            .map(|entry| entry.block.layout.clone())
            .collect::<Vec<_>>()
    ))
}

fn whole_site(slug: &str, permissions: Vec<(UserId, String)>) -> NewWholeSite {
    NewWholeSite {
        site: NewSite {
            slug: slug.to_string(),
            theme: "default".to_string(),
            title: "Whole".to_string(),
            ..Default::default()
        },
        first_page: NewFirstPage {
            slug: "welcome".to_string(),
            title: "Welcome".to_string(),
            is_public: true,
            blocks: vec![
                NewFirstPageBlock {
                    layout: "html".to_string(),
                    data: serde_json::json!({ "html": "<p>One</p>" }),
                    layout_data: None,
                },
                NewFirstPageBlock {
                    layout: "browsePreview".to_string(),
                    data: serde_json::json!({ "limit": 6 }),
                    layout_data: Some(serde_json::json!({ "class": "wide" })),
                },
            ],
        },
        creator: None,
        navigation_linking_first_page: Some(link_first_page),
        permissions,
        item_sets: Vec::new(),
        settings: vec![("locale".to_string(), serde_json::json!("es"))],
    }
}

async fn count(pool: &SqlitePool, table: &str) -> i64 {
    sqlx::query_scalar(&format!("SELECT count(*) FROM {table}"))
        .fetch_one(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn create_whole_stores_the_site_with_its_first_page_and_rows() {
    let pool = common::memory().await;
    let user = UserRepo::new(&pool)
        .create(&NewUser {
            email: "whole@example.org".to_string(),
            name: "Whole".to_string(),
            password_hash: None,
            role: "editor".to_string(),
            is_active: true,
            created: None,
        })
        .await
        .unwrap();
    let item_set = ItemSetRepo::new(&pool)
        .create(&NewItemSet::default())
        .await
        .unwrap();
    let mut new = whole_site("whole", vec![(user.id, "admin".to_string())]);
    new.item_sets = vec![item_set.id()];
    new.creator = Some(user.id);
    let sites = SiteRepo::new(&pool);

    let site = sites.create_whole(&new, &layouts).await.unwrap();

    let stored = sites.get(site.id).await.unwrap().unwrap();
    assert_eq!(stored, site);
    assert_eq!(stored.modified, Some(stored.created));
    let pages = sites.list_pages(site.id).await.unwrap();
    assert_eq!(pages.len(), 1);
    assert_eq!(pages[0].slug, "welcome");
    assert_eq!(stored.navigation, link_first_page(pages[0].id));
    let blocks = sites.list_blocks(pages[0].id).await.unwrap();
    let shape: Vec<(&str, i64)> = blocks
        .iter()
        .map(|block| (block.layout.as_str(), block.position))
        .collect();
    assert_eq!(shape, [("html", 1), ("browsePreview", 2)]);
    let versions: Vec<_> = PageVersionRepo::new(&pool)
        .list(pages[0].id)
        .await
        .unwrap()
        .into_iter()
        .map(|row| {
            (
                row.version,
                row.origin,
                row.user_id,
                row.created,
                row.snapshot,
            )
        })
        .collect();
    assert_eq!(
        versions,
        [(
            1,
            PageVersionOrigin::Create,
            Some(user.id),
            pages[0].created,
            serde_json::json!(["html", "browsePreview"]),
        )]
    );
    assert_eq!(
        blocks[1].layout_data,
        Some(serde_json::json!({ "class": "wide" }))
    );
    let permissions = sites.list_permissions(site.id).await.unwrap();
    assert_eq!(permissions.len(), 1);
    assert_eq!(
        (permissions[0].user_id, permissions[0].role.as_str()),
        (user.id, "admin")
    );
    let item_sets = sites.list_item_sets(site.id).await.unwrap();
    assert_eq!(item_sets.len(), 1);
    assert_eq!(
        (item_sets[0].item_set_id, item_sets[0].position),
        (item_set.id(), Some(1))
    );
    assert_eq!(
        SiteSettingRepo::new(&pool)
            .get(site.id, "locale")
            .await
            .unwrap(),
        Some(serde_json::json!("es"))
    );
}

/// A permission for a user that does not exist fails on its foreign key after the site, page
/// and blocks are inserted, and the whole create rolls back.
#[tokio::test]
async fn create_whole_leaves_nothing_when_a_later_row_fails() {
    let pool = common::memory().await;
    let new = whole_site("broken", vec![(UserId(999), "admin".to_string())]);

    assert!(
        SiteRepo::new(&pool)
            .create_whole(&new, &layouts)
            .await
            .is_err()
    );

    for table in [
        "site",
        "site_page",
        "site_page_block",
        "romeka_site_page_version",
        "site_permission",
        "site_setting",
    ] {
        assert_eq!(count(&pool, table).await, 0, "{table}");
    }
}
