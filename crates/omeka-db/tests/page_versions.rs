//! `romeka_site_page_version` behaviour: a create records the page it makes as version 1, a save
//! records the state it makes, the first save of a page with no version also keeps the state it
//! replaced, a stale save writes nothing, a failing row rolls the whole create or save back, a
//! page keeps its newest 100 versions, and deleting a page deletes its history.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use omeka_db::DbError;
use omeka_db::ids::{ItemId, PageId, UserId};
use omeka_db::repos::{PageVersionRepo, SiteRepo, UserRepo};
use omeka_db::rows::{
    BlockChange, NewPageAttachment, NewPageBlock, NewSite, NewSiteBlock, NewSitePage, NewUser,
    PageCreate, PageImage, PageSave, PageSaveOutcome, PageVersionOrigin, SitePageRow,
    VersionAuthor,
};
use serde_json::{Value, json};
use sqlx::SqlitePool;

/// A snapshot that names what a reader of the history needs: the title and each block's text.
fn summary(image: &PageImage) -> Result<Value, DbError> {
    Ok(json!({
        "title": image.page.title,
        "blocks": image
            .blocks
            .iter()
            .map(|entry| entry.block.data.get("html").cloned().unwrap_or(Value::Null))
            .collect::<Vec<_>>(),
        "attachments": image.blocks.iter().map(|entry| entry.attachments.len()).sum::<usize>(),
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

async fn user(pool: &SqlitePool, name: &str) -> UserId {
    UserRepo::new(pool)
        .create(&NewUser {
            email: format!("{name}@example.com"),
            name: name.to_string(),
            password_hash: None,
            role: "editor".to_string(),
            is_active: true,
            created: None,
        })
        .await
        .unwrap()
        .id
}

/// A page titled "Intro" with one html block reading "first".
async fn page(pool: &SqlitePool) -> SitePageRow {
    let repo = SiteRepo::new(pool);
    let site = repo
        .create(&NewSite {
            slug: "history".to_string(),
            theme: "default".to_string(),
            title: "History".to_string(),
            ..NewSite::default()
        })
        .await
        .unwrap();
    let page = repo
        .add_page(&NewSitePage {
            site_id: site.id,
            slug: "intro".to_string(),
            title: "Intro".to_string(),
            is_public: true,
            ..NewSitePage::default()
        })
        .await
        .unwrap();
    repo.add_block(&NewSiteBlock {
        page_id: page.id,
        layout: "html".to_string(),
        data: json!({ "html": "first" }),
        layout_data: None,
        position: 1,
    })
    .await
    .unwrap();
    page
}

fn titled(page: &SitePageRow, title: &str) -> SitePageRow {
    SitePageRow {
        title: title.to_string(),
        ..page.clone()
    }
}

async fn save(
    pool: &SqlitePool,
    page: &SitePageRow,
    blocks: BlockChange<'_>,
    user: Option<UserId>,
    expected_modified: Option<chrono::NaiveDateTime>,
) -> PageSaveOutcome {
    PageVersionRepo::new(pool)
        .save(
            &PageSave {
                page,
                blocks,
                user,
                expected_modified,
            },
            &summary,
        )
        .await
        .unwrap()
}

async fn block_texts(pool: &SqlitePool, page: PageId) -> Vec<Value> {
    SiteRepo::new(pool)
        .list_blocks(page)
        .await
        .unwrap()
        .into_iter()
        .map(|block| block.data["html"].clone())
        .collect()
}

#[tokio::test]
async fn the_first_save_keeps_the_replaced_state_and_records_the_new_one() {
    let pool = common::memory().await;
    let ana = user(&pool, "Ana").await;
    let page = page(&pool).await;

    let outcome = save(
        &pool,
        &titled(&page, "Welcome"),
        BlockChange::Replace(&[html("second"), html("third")]),
        Some(ana),
        None,
    )
    .await;

    let PageSaveOutcome::Saved {
        page: saved,
        version,
    } = outcome
    else {
        panic!("the save was refused: {outcome:?}");
    };
    assert_eq!(version, 2);
    assert_eq!(saved.title, "Welcome");
    assert_eq!(
        block_texts(&pool, page.id).await,
        [json!("second"), json!("third")]
    );

    let versions = PageVersionRepo::new(&pool).list(page.id).await.unwrap();
    let kept: Vec<_> = versions
        .iter()
        .map(|row| (row.version, row.origin, row.author(), row.snapshot.clone()))
        .collect();
    assert_eq!(
        kept,
        [
            (
                2,
                PageVersionOrigin::Save,
                VersionAuthor::User(ana),
                json!({"title": "Welcome", "blocks": ["second", "third"], "attachments": 0})
            ),
            (
                1,
                PageVersionOrigin::BeforeHistory,
                VersionAuthor::NotRecorded,
                json!({"title": "Intro", "blocks": ["first"], "attachments": 0})
            ),
        ]
    );
    assert_eq!(versions[0].created, saved.modified.unwrap());
}

#[tokio::test]
async fn a_save_that_does_not_change_blocks_still_records_a_version() {
    let pool = common::memory().await;
    let page = page(&pool).await;

    save(
        &pool,
        &titled(&page, "Renamed"),
        BlockChange::Keep,
        None,
        None,
    )
    .await;
    save(
        &pool,
        &titled(&page, "Appended"),
        BlockChange::Append(&[html("more")]),
        None,
        None,
    )
    .await;

    let snapshots: Vec<Value> = PageVersionRepo::new(&pool)
        .list(page.id)
        .await
        .unwrap()
        .into_iter()
        .map(|row| row.snapshot)
        .collect();
    assert_eq!(
        snapshots,
        [
            json!({"title": "Appended", "blocks": ["first", "more"], "attachments": 0}),
            json!({"title": "Renamed", "blocks": ["first"], "attachments": 0}),
            json!({"title": "Intro", "blocks": ["first"], "attachments": 0}),
        ]
    );
}

#[tokio::test]
async fn a_stale_save_writes_nothing_and_names_who_saved_since() {
    let pool = common::memory().await;
    let ana = user(&pool, "Ana").await;
    let ben = user(&pool, "Ben").await;
    let page = page(&pool).await;
    let loaded = page.modified;

    let PageSaveOutcome::Saved { page: anas, .. } = save(
        &pool,
        &titled(&page, "Ana's"),
        BlockChange::Replace(&[html("ana")]),
        Some(ana),
        loaded,
    )
    .await
    else {
        panic!("Ana's save was refused");
    };
    // Ben loaded the page before Ana saved; make Ana's save a later second than his load.
    let earlier = anas.modified.unwrap() - chrono::Duration::seconds(5);

    let outcome = save(
        &pool,
        &titled(&page, "Ben's"),
        BlockChange::Replace(&[html("ben")]),
        Some(ben),
        Some(earlier),
    )
    .await;

    assert_eq!(
        outcome,
        PageSaveOutcome::Stale {
            modified: anas.modified,
            saved_by: Some(ana),
        }
    );
    let stored = SiteRepo::new(&pool)
        .get_page(page.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored.title, "Ana's");
    assert_eq!(block_texts(&pool, page.id).await, [json!("ana")]);
    assert_eq!(
        PageVersionRepo::new(&pool)
            .list(page.id)
            .await
            .unwrap()
            .len(),
        2
    );

    let resent = save(
        &pool,
        &titled(&page, "Ben's"),
        BlockChange::Replace(&[html("ben")]),
        Some(ben),
        anas.modified,
    )
    .await;
    assert!(matches!(resent, PageSaveOutcome::Saved { version: 3, .. }));
}

#[tokio::test]
async fn a_failing_block_row_leaves_the_old_page_and_history_intact() {
    let pool = common::memory().await;
    let page = page(&pool).await;
    save(&pool, &titled(&page, "Kept"), BlockChange::Keep, None, None).await;

    let broken = NewPageBlock {
        attachments: vec![NewPageAttachment {
            item_id: Some(ItemId(999_999)),
            media_id: None,
            caption: String::new(),
        }],
        ..html("broken")
    };
    let failed = PageVersionRepo::new(&pool)
        .save(
            &PageSave {
                page: &titled(&page, "Lost"),
                blocks: BlockChange::Replace(&[html("new"), broken]),
                user: None,
                expected_modified: None,
            },
            &summary,
        )
        .await;

    assert!(failed.unwrap_err().is_foreign_key_violation());
    let stored = SiteRepo::new(&pool)
        .get_page(page.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored.title, "Kept");
    assert_eq!(block_texts(&pool, page.id).await, [json!("first")]);
    let versions: Vec<u32> = PageVersionRepo::new(&pool)
        .list(page.id)
        .await
        .unwrap()
        .into_iter()
        .map(|row| row.version)
        .collect();
    assert_eq!(versions, [2, 1]);
}

#[tokio::test]
async fn a_page_keeps_its_newest_hundred_versions() {
    let pool = common::memory().await;
    let page = page(&pool).await;

    for n in 1..=105 {
        save(
            &pool,
            &titled(&page, &format!("Save {n}")),
            BlockChange::Keep,
            None,
            None,
        )
        .await;
    }

    let versions = PageVersionRepo::new(&pool).list(page.id).await.unwrap();
    assert_eq!(versions.len(), 100);
    assert_eq!(versions[0].version, 106);
    assert_eq!(versions[0].snapshot["title"], "Save 105");
    assert_eq!(versions[99].version, 7);
    assert_eq!(versions[99].snapshot["title"], "Save 6");
    let repo = PageVersionRepo::new(&pool);
    assert!(repo.get(page.id, 6).await.unwrap().is_none());
    assert_eq!(
        repo.get(page.id, 7).await.unwrap().unwrap().snapshot["title"],
        "Save 6"
    );
}

#[tokio::test]
async fn deleting_a_page_deletes_its_history_and_a_save_of_it_finds_nothing() {
    let pool = common::memory().await;
    let page = page(&pool).await;
    save(&pool, &titled(&page, "Once"), BlockChange::Keep, None, None).await;

    SiteRepo::new(&pool).delete_page(page.id).await.unwrap();

    assert!(
        PageVersionRepo::new(&pool)
            .list(page.id)
            .await
            .unwrap()
            .is_empty()
    );
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM romeka_site_page_version")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    assert_eq!(
        save(&pool, &titled(&page, "Gone"), BlockChange::Keep, None, None).await,
        PageSaveOutcome::Missing
    );
}

#[tokio::test]
async fn deleting_the_user_keeps_their_versions_without_a_name() {
    let pool = common::memory().await;
    let ana = user(&pool, "Ana").await;
    let page = page(&pool).await;
    save(
        &pool,
        &titled(&page, "Ana's"),
        BlockChange::Keep,
        Some(ana),
        None,
    )
    .await;

    sqlx::query("DELETE FROM user WHERE id = ?")
        .bind(i64::try_from(ana.0).unwrap())
        .execute(&pool)
        .await
        .unwrap();

    let newest = PageVersionRepo::new(&pool)
        .get(page.id, 2)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(newest.user_id, None);
    assert_eq!(newest.snapshot["title"], "Ana's");
}

async fn site(pool: &SqlitePool) -> omeka_db::ids::SiteId {
    SiteRepo::new(pool)
        .create(&NewSite {
            slug: "created".to_string(),
            theme: "default".to_string(),
            title: "Created".to_string(),
            ..NewSite::default()
        })
        .await
        .unwrap()
        .id
}

fn new_page(site: omeka_db::ids::SiteId) -> NewSitePage {
    NewSitePage {
        site_id: site,
        slug: "made".to_string(),
        title: "Made".to_string(),
        is_public: true,
        ..NewSitePage::default()
    }
}

async fn create(
    pool: &SqlitePool,
    page: &NewSitePage,
    blocks: &[NewPageBlock],
    user: Option<UserId>,
) -> Result<SitePageRow, DbError> {
    PageVersionRepo::new(pool)
        .create(&PageCreate { page, blocks, user }, &summary)
        .await
}

#[tokio::test]
async fn a_create_records_the_page_it_makes_as_version_one_by_its_creator() {
    let pool = common::memory().await;
    let ana = user(&pool, "Ana").await;
    let site = site(&pool).await;

    let page = create(&pool, &new_page(site), &[html("a"), html("b")], Some(ana))
        .await
        .unwrap();

    assert_eq!(block_texts(&pool, page.id).await, [json!("a"), json!("b")]);
    let versions = PageVersionRepo::new(&pool).list(page.id).await.unwrap();
    let kept: Vec<_> = versions
        .iter()
        .map(|row| (row.version, row.origin, row.author(), row.snapshot.clone()))
        .collect();
    assert_eq!(
        kept,
        [(
            1,
            PageVersionOrigin::Create,
            VersionAuthor::User(ana),
            json!({"title": "Made", "blocks": ["a", "b"], "attachments": 0})
        )]
    );
    assert_eq!(versions[0].created, page.created);
    assert_eq!(page.modified, Some(page.created));
}

#[tokio::test]
async fn the_first_save_of_a_created_page_adds_one_version() {
    let pool = common::memory().await;
    let ana = user(&pool, "Ana").await;
    let site = site(&pool).await;
    let page = create(&pool, &new_page(site), &[html("a")], Some(ana))
        .await
        .unwrap();

    save(
        &pool,
        &titled(&page, "Again"),
        BlockChange::Keep,
        None,
        None,
    )
    .await;

    let origins: Vec<_> = PageVersionRepo::new(&pool)
        .list(page.id)
        .await
        .unwrap()
        .into_iter()
        .map(|row| (row.version, row.origin, row.author()))
        .collect();
    assert_eq!(
        origins,
        [
            (2, PageVersionOrigin::Save, VersionAuthor::Deleted),
            (1, PageVersionOrigin::Create, VersionAuthor::User(ana)),
        ]
    );
}

#[tokio::test]
async fn a_create_whose_attachment_fails_leaves_no_page_block_or_version() {
    let pool = common::memory().await;
    let site = site(&pool).await;
    let broken = NewPageBlock {
        attachments: vec![NewPageAttachment {
            item_id: Some(ItemId(999_999)),
            media_id: None,
            caption: String::new(),
        }],
        ..html("broken")
    };

    let failed = create(&pool, &new_page(site), &[html("kept?"), broken], None).await;

    assert!(failed.unwrap_err().is_foreign_key_violation());
    for table in [
        "site_page",
        "site_page_block",
        "site_block_attachment",
        "romeka_site_page_version",
    ] {
        let count: i64 = sqlx::query_scalar(&format!("SELECT count(*) FROM {table}"))
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 0, "{table} keeps no row");
    }
}

#[tokio::test]
async fn deleting_the_creator_keeps_the_created_version_as_a_deleted_users() {
    let pool = common::memory().await;
    let ana = user(&pool, "Ana").await;
    let site = site(&pool).await;
    let page = create(&pool, &new_page(site), &[], Some(ana))
        .await
        .unwrap();

    UserRepo::new(&pool).delete(ana).await.unwrap();

    let first = PageVersionRepo::new(&pool)
        .get(page.id, 1)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        (first.origin, first.author()),
        (PageVersionOrigin::Create, VersionAuthor::Deleted)
    );
}

#[tokio::test]
async fn the_trim_takes_a_created_pages_create_version_like_any_other() {
    let pool = common::memory().await;
    let site = site(&pool).await;
    let page = create(&pool, &new_page(site), &[], None).await.unwrap();

    for n in 1..=100 {
        save(
            &pool,
            &titled(&page, &format!("Save {n}")),
            BlockChange::Keep,
            None,
            None,
        )
        .await;
    }

    let versions = PageVersionRepo::new(&pool).list(page.id).await.unwrap();
    assert_eq!(versions.len(), 100);
    assert_eq!(versions[0].version, 101);
    assert_eq!(
        (versions[99].version, versions[99].origin),
        (2, PageVersionOrigin::Save)
    );
}
