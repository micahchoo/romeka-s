//! Undo against the run fixture: what it removes, what it keeps, and how a
//! stopped run or undo converges on the next undo.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use omeka_db::repos::{ImportResourceRepo, ItemRepo, ValueRepo};
use omeka_db::rows::{ImportKind, NewItem, NewValue};

use super::run::{find_property, run};
use super::run_tests::{fixture, plan};
use super::undo::{KeptKind, undo};
use crate::jsonld::BaseUrl;

#[tokio::test]
async fn undo_removes_items_media_files_and_the_new_set_type_and_field() {
    let fixture = fixture().await;
    let (import, _, _) = fixture.run().await;
    let report = fixture.undo(import).await;

    assert_eq!(
        (
            report.items,
            report.item_sets,
            report.templates,
            report.fields
        ),
        (3, 1, 1, 1)
    );
    assert!(report.kept.is_empty());
    assert_eq!(fixture.count("item").await, 0);
    assert_eq!(fixture.count("media").await, 0);
    assert_eq!(
        fixture.count("item_set").await,
        1,
        "the existing Photos set stays"
    );
    assert_eq!(fixture.count("resource_template").await, 0);
    assert!(fixture.stored().is_empty(), "{:?}", fixture.stored());
    assert!(
        find_property(&fixture.pool, "local:shelfMark")
            .await
            .expect("lookup")
            .is_none()
    );
    assert_eq!(fixture.count("romeka_import_resource").await, 0);
}

#[tokio::test]
async fn undo_counts_the_media_it_removed_and_a_second_undo_counts_none() {
    let fixture = fixture().await;
    let (import, _, _) = fixture.run().await;
    let media = fixture.count("media").await;
    assert!(media > 0, "the fixture imports media");

    let first = fixture.undo(import).await;
    assert_eq!(i64::try_from(first.media).expect("fits"), media);
    assert_eq!(fixture.recorded(import, ImportKind::Media).await, 0);

    let second = fixture.undo(import).await;
    assert_eq!(second.media, 0);
}

#[tokio::test]
async fn undo_keeps_a_new_field_a_hand_made_item_uses_and_a_second_undo_changes_nothing() {
    let fixture = fixture().await;
    let (import, _, _) = fixture.run().await;
    let field = fixture.property("local:shelfMark").await;
    let hand = ItemRepo::new(&fixture.pool)
        .create(&NewItem::default())
        .await
        .expect("hand-made item");
    ValueRepo::new(&fixture.pool)
        .create(&NewValue {
            resource_id: hand.resource.id,
            property_id: field,
            value_resource_id: None,
            value_annotation_id: None,
            r#type: "literal".to_string(),
            lang: None,
            value: Some("C-9".to_string()),
            uri: None,
            is_public: true,
        })
        .await
        .expect("value");

    let first = fixture.undo(import).await;
    assert_eq!(first.fields, 0);
    assert_eq!(first.kept.len(), 1);
    assert_eq!(first.kept[0].kind, KeptKind::Field);
    assert_eq!(first.kept[0].id, field.0);
    assert_eq!(fixture.count("item").await, 1, "only the hand-made item");
    assert_eq!(fixture.values(hand.resource.id.0).await["shelfMark"], "C-9");

    let before = (
        fixture.count("resource").await,
        fixture.count("value").await,
    );
    let second = fixture.undo(import).await;
    assert_eq!(
        (
            second.items,
            second.media,
            second.item_sets,
            second.templates
        ),
        (0, 0, 0, 0)
    );
    assert_eq!(second.kept, first.kept, "the kept field is reported again");
    assert_eq!(
        (
            fixture.count("resource").await,
            fixture.count("value").await
        ),
        before
    );
}

/// The progress callback panics after `steps` steps, the way a killed job stops.
async fn crash_after<Fut>(steps: usize, work: impl FnOnce(Box<dyn FnMut(usize) + Send>) -> Fut)
where
    Fut: std::future::Future<Output = ()> + Send + 'static,
{
    let mut seen = 0;
    let stop: Box<dyn FnMut(usize) + Send> = Box::new(move |_| {
        seen += 1;
        assert!(seen < steps, "simulated crash");
    });
    let outcome = tokio::spawn(work(stop)).await;
    assert!(outcome.is_err(), "the task stopped part way");
}

#[tokio::test(flavor = "multi_thread")]
async fn undo_after_a_run_that_stopped_part_way_converges() {
    let fixture = fixture().await;
    let import = fixture.import(&plan()).await;
    let (pool, store, input) = (
        fixture.pool.clone(),
        fixture.store.clone(),
        fixture.input(import, plan()),
    );
    // Three setup steps (set, type, field), then the first item.
    crash_after(4, |mut stop| async move {
        let _ = run(&pool, &store, input, move |step| stop(step.done)).await;
    })
    .await;
    assert_eq!(
        fixture.count("item").await,
        1,
        "one item committed before the stop"
    );

    let report = fixture.undo(import).await;
    assert_eq!(
        (
            report.items,
            report.item_sets,
            report.templates,
            report.fields
        ),
        (1, 1, 1, 1)
    );
    assert_eq!(fixture.count("item").await, 0);
    assert!(fixture.stored().is_empty());
    assert_eq!(fixture.count("romeka_import_resource").await, 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn an_undo_that_stopped_part_way_converges_on_the_next_undo() {
    let fixture = fixture().await;
    let (import, _, _) = fixture.run().await;
    let (pool, store) = (fixture.pool.clone(), fixture.store.clone());
    crash_after(2, |mut stop| async move {
        let _ = undo(&pool, &store, import, move |step| stop(step.done)).await;
    })
    .await;
    assert!(
        fixture.count("item").await < 3,
        "the stopped undo removed something"
    );
    // A stop between deleting a target and forgetting its row: the item is
    // gone and its row is still there.
    let left = ImportResourceRepo::new(&fixture.pool)
        .list(import, Some(ImportKind::Item))
        .await
        .expect("records");
    let api = crate::read::Api::load_with_store(
        &fixture.pool,
        BaseUrl::parse("http://localhost").expect("base"),
        super::system_actor(),
        Some(fixture.store.clone()),
    )
    .await
    .expect("api");
    let omeka_db::rows::ImportTarget::Item(item) = left[0].target else {
        panic!("an item row");
    };
    crate::write::items::delete(&api, &item.0.to_string())
        .await
        .expect("delete by hand");

    fixture.undo(import).await;
    assert_eq!(fixture.count("item").await, 0);
    assert_eq!(fixture.count("media").await, 0);
    assert_eq!(fixture.count("resource_template").await, 0);
    assert!(fixture.stored().is_empty());
    assert_eq!(fixture.count("romeka_import_resource").await, 0);
}
