//! `site_block_attachment`: the round trip, the position order, the null-on-delete references and
//! the parent key.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use omeka_db::ids::BlockId;
use omeka_db::repos::{ItemRepo, SiteBlockAttachmentRepo, SiteRepo};
use omeka_db::rows::{NewItem, NewSite, NewSiteBlock, NewSiteBlockAttachment, NewSitePage};
use sqlx::SqlitePool;

/// A block to attach to, with its site and page behind it.
async fn a_block(pool: &SqlitePool) -> BlockId {
    let sites = SiteRepo::new(pool);
    let site = sites
        .create(&NewSite {
            slug: "museum".to_string(),
            theme: "default".to_string(),
            title: "Museum".to_string(),
            ..Default::default()
        })
        .await
        .unwrap();
    let page = sites
        .add_page(&NewSitePage {
            site_id: site.id,
            slug: "visit".to_string(),
            title: "Visit".to_string(),
            is_public: true,
            ..Default::default()
        })
        .await
        .unwrap();
    sites
        .add_block(&NewSiteBlock {
            page_id: page.id,
            layout: "media".to_string(),
            data: serde_json::json!({}),
            layout_data: None,
            position: 1,
        })
        .await
        .unwrap()
        .id
}

#[tokio::test]
async fn attachments_round_trip_and_order_by_position() {
    let pool = common::memory().await;
    let block = a_block(&pool).await;
    let attachments = SiteBlockAttachmentRepo::new(&pool);

    let last = attachments
        .create(&NewSiteBlockAttachment {
            block_id: block,
            item_id: None,
            media_id: None,
            caption: "Third".to_string(),
            position: 3,
        })
        .await
        .unwrap();
    let first = attachments
        .create(&NewSiteBlockAttachment {
            block_id: block,
            item_id: None,
            media_id: None,
            caption: "First".to_string(),
            position: 1,
        })
        .await
        .unwrap();
    let tied = attachments
        .create(&NewSiteBlockAttachment {
            block_id: block,
            item_id: None,
            media_id: None,
            caption: "First too".to_string(),
            position: 1,
        })
        .await
        .unwrap();

    assert_eq!(
        attachments.get(first.id).await.unwrap(),
        Some(first.clone())
    );
    assert_eq!(
        attachments
            .list_for_block(block)
            .await
            .unwrap()
            .iter()
            .map(|row| row.id)
            .collect::<Vec<_>>(),
        vec![first.id, tied.id, last.id],
        "position ascending, id breaking a tie"
    );

    let mut moved = last.clone();
    moved.position = 0;
    moved.caption = "Now first".to_string();
    assert!(attachments.update(&moved).await.unwrap());
    let after = attachments.list_for_block(block).await.unwrap();
    assert_eq!(after[0].id, last.id);
    assert_eq!(after[0].caption, "Now first");

    assert!(attachments.delete(first.id).await.unwrap());
    assert_eq!(attachments.get(first.id).await.unwrap(), None);
    assert!(!attachments.delete(first.id).await.unwrap());
}

#[tokio::test]
async fn deleting_an_item_clears_the_attachment_reference() {
    let pool = common::memory().await;
    let block = a_block(&pool).await;
    let item = ItemRepo::new(&pool)
        .create(&NewItem::default())
        .await
        .unwrap();
    let attachment = SiteBlockAttachmentRepo::new(&pool)
        .create(&NewSiteBlockAttachment {
            block_id: block,
            item_id: Some(item.id()),
            media_id: None,
            caption: "A helmet".to_string(),
            position: 1,
        })
        .await
        .unwrap();

    assert!(ItemRepo::new(&pool).delete(item.id()).await.unwrap());

    let kept = SiteBlockAttachmentRepo::new(&pool)
        .get(attachment.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        kept.item_id, None,
        "the attachment stays and loses the item, as the schema declares"
    );
}

#[tokio::test]
async fn an_attachment_needs_a_block() {
    let pool = common::memory().await;
    let error = SiteBlockAttachmentRepo::new(&pool)
        .create(&NewSiteBlockAttachment {
            block_id: BlockId(404),
            item_id: None,
            media_id: None,
            caption: "Orphan".to_string(),
            position: 1,
        })
        .await
        .unwrap_err();
    assert!(error.is_foreign_key_violation(), "{error}");
}
