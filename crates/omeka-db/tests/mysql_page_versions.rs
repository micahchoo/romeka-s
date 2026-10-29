//! `romeka_site_page_version` against a real MySQL server: migration 0007 applies after the gap at
//! 0004 and 0006, a save records its versions in one transaction, a stale save is refused, a
//! failing row rolls back, and a page delete cascades; a create records version 1 with origin
//! `create` in one transaction. Gated as `tests/mysql.rs` is.

#![cfg(feature = "mysql")]
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use omeka_db::DbError;
use omeka_db::ids::ItemId;
use omeka_db::repos::{PageVersionRepo, SiteRepo, UserRepo};
use omeka_db::rows::{
    BlockChange, NewPageAttachment, NewPageBlock, NewSite, NewSitePage, NewUser, PageCreate,
    PageImage, PageSave, PageSaveOutcome, PageVersionOrigin, SitePageRow,
};
use serde_json::{Value, json};
use sqlx::MySql;

fn summary(image: &PageImage) -> Result<Value, DbError> {
    Ok(json!({
        "title": image.page.title,
        "blocks": image.blocks.iter().map(|entry| entry.block.data["html"].clone()).collect::<Vec<_>>(),
    }))
}

fn html(text: &str) -> NewPageBlock {
    NewPageBlock {
        layout: "html".to_string(),
        data: json!({ "html": text }),
        layout_data: None,
        attachments: Vec::new(),
    }
}

#[tokio::test]
async fn page_versions_save_refuse_roll_back_and_cascade_on_mysql() {
    let Some(db) = common::mysql().await else {
        return;
    };
    let applied: Vec<i64> =
        sqlx::query_scalar("SELECT version FROM _sqlx_migrations ORDER BY version")
            .fetch_all(&db.pool)
            .await
            .expect("applied migrations");
    assert!(applied.contains(&7), "0007 applied: {applied:?}");

    let ana = UserRepo::<MySql>::new(&db.pool)
        .create(&NewUser {
            email: "ana@example.com".to_string(),
            name: "Ana".to_string(),
            password_hash: None,
            role: "editor".to_string(),
            is_active: true,
            created: None,
        })
        .await
        .expect("create user")
        .id;
    let sites = SiteRepo::<MySql>::new(&db.pool);
    let site = sites
        .create(&NewSite {
            slug: "history".to_string(),
            theme: "default".to_string(),
            title: "History".to_string(),
            ..NewSite::default()
        })
        .await
        .expect("create site");
    let page = sites
        .add_page(&NewSitePage {
            site_id: site.id,
            slug: "intro".to_string(),
            title: "Intro".to_string(),
            is_public: true,
            ..NewSitePage::default()
        })
        .await
        .expect("create page");
    let repo = PageVersionRepo::<MySql>::new(&db.pool);
    let retitled = |title: &str| SitePageRow {
        title: title.to_string(),
        ..page.clone()
    };

    let first = repo
        .save(
            &PageSave {
                page: &retitled("Welcome"),
                blocks: BlockChange::Replace(&[html("one")]),
                user: Some(ana),
                expected_modified: page.modified,
            },
            &summary,
        )
        .await
        .expect("save");
    let PageSaveOutcome::Saved {
        page: saved,
        version: 2,
    } = first
    else {
        panic!("the first save made {first:?}");
    };

    let stale = repo
        .save(
            &PageSave {
                page: &retitled("Late"),
                blocks: BlockChange::Keep,
                user: None,
                expected_modified: Some(saved.modified.unwrap() - chrono::Duration::seconds(5)),
            },
            &summary,
        )
        .await
        .expect("stale save");
    assert_eq!(
        stale,
        PageSaveOutcome::Stale {
            modified: saved.modified,
            saved_by: Some(ana),
        }
    );

    let broken = NewPageBlock {
        attachments: vec![NewPageAttachment {
            item_id: Some(ItemId(999_999)),
            media_id: None,
            caption: String::new(),
        }],
        ..html("broken")
    };
    let failed = repo
        .save(
            &PageSave {
                page: &retitled("Lost"),
                blocks: BlockChange::Replace(&[broken]),
                user: None,
                expected_modified: None,
            },
            &summary,
        )
        .await;
    assert!(failed.is_err(), "a missing item fails the attachment row");

    let snapshots: Vec<(Value, PageVersionOrigin)> = repo
        .list(page.id)
        .await
        .expect("list")
        .into_iter()
        .map(|row| (row.snapshot, row.origin))
        .collect();
    assert_eq!(
        snapshots,
        [
            (
                json!({"title": "Welcome", "blocks": ["one"]}),
                PageVersionOrigin::Save
            ),
            (
                json!({"title": "Intro", "blocks": []}),
                PageVersionOrigin::BeforeHistory
            ),
        ]
    );
    let stored = sites.get_page(page.id).await.expect("get").expect("page");
    assert_eq!(stored.title, "Welcome");

    sites.delete_page(page.id).await.expect("delete page");
    assert!(repo.list(page.id).await.expect("list").is_empty());

    db.cleanup().await;
}

#[tokio::test]
async fn a_page_create_is_one_transaction_with_its_version_on_mysql() {
    let Some(db) = common::mysql().await else {
        return;
    };
    let site = SiteRepo::<MySql>::new(&db.pool)
        .create(&NewSite {
            slug: "created".to_string(),
            theme: "default".to_string(),
            title: "Created".to_string(),
            ..NewSite::default()
        })
        .await
        .expect("create site");
    let new = |slug: &str| NewSitePage {
        site_id: site.id,
        slug: slug.to_string(),
        title: "Made".to_string(),
        is_public: true,
        ..NewSitePage::default()
    };
    let repo = PageVersionRepo::<MySql>::new(&db.pool);

    let made = repo
        .create(
            &PageCreate {
                page: &new("made"),
                blocks: &[html("one")],
                user: None,
            },
            &summary,
        )
        .await
        .expect("create");
    let broken = NewPageBlock {
        attachments: vec![NewPageAttachment {
            item_id: Some(ItemId(999_999)),
            media_id: None,
            caption: String::new(),
        }],
        ..html("broken")
    };
    let failed = repo
        .create(
            &PageCreate {
                page: &new("lost"),
                blocks: &[broken],
                user: None,
            },
            &summary,
        )
        .await;

    assert!(failed.is_err(), "a missing item fails the attachment row");
    let pages = SiteRepo::<MySql>::new(&db.pool)
        .list_pages(site.id)
        .await
        .expect("pages");
    assert_eq!(pages.len(), 1, "the failed create left no page");
    let versions: Vec<(u32, PageVersionOrigin)> = repo
        .list(made.id)
        .await
        .expect("list")
        .into_iter()
        .map(|row| (row.version, row.origin))
        .collect();
    assert_eq!(versions, [(1, PageVersionOrigin::Create)]);

    db.cleanup().await;
}
