//! `romeka_item_history` behaviour at the repository seam: a save and its entry commit together
//! or not at all, a save that changes nothing records nothing, and a run's second pass over an
//! item folds into the entry its first pass made. `measure_history_cost` (ignored) prints the
//! table size per 1000 edits and the cost the entry adds to one save.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::time::{Duration, Instant};

use omeka_db::ids::{ImportId, ItemSetId, PropertyId};
use omeka_db::repos::{
    ImportWrites, ItemHistoryRepo, ItemRepo, ItemSave, ItemSetRepo, ItemWrite, Membership,
    PropertyRepo, VocabularyRepo,
};
use omeka_db::rows::{
    FieldChange, HistorySource, HistoryStamp, ImportResource, ItemRow, NewItem, NewItemSet,
    NewProperty, NewResource, NewVocabulary, ValueContent, ValueTree,
};
use sqlx::SqlitePool;

const EDIT: HistoryStamp = HistoryStamp {
    user: None,
    source: HistorySource::Edit,
};

async fn properties(pool: &SqlitePool, names: &[&str]) -> Vec<PropertyId> {
    let vocabulary = VocabularyRepo::new(pool)
        .create(&NewVocabulary {
            owner_id: None,
            namespace_uri: "http://purl.org/dc/terms/".to_string(),
            prefix: "dcterms".to_string(),
            label: "Dublin Core".to_string(),
            comment: None,
        })
        .await
        .unwrap();
    let mut ids = Vec::new();
    for name in names {
        ids.push(
            PropertyRepo::new(pool)
                .create(&NewProperty {
                    owner_id: None,
                    vocabulary_id: vocabulary.id,
                    local_name: (*name).to_string(),
                    label: (*name).to_string(),
                    comment: None,
                })
                .await
                .unwrap()
                .id,
        );
    }
    ids
}

fn tree(property: PropertyId, text: &str) -> ValueTree {
    ValueTree {
        content: ValueContent::literal(property, text),
        annotation: None,
    }
}

async fn create(pool: &SqlitePool, values: &[ValueTree]) -> ItemRow {
    let new = NewItem {
        resource: NewResource {
            is_public: true,
            ..NewResource::default()
        },
        primary_media_id: None,
    };
    ItemRepo::new(pool)
        .save(&ItemSave {
            write: ItemWrite::Create(&new),
            values: Some(values),
            item_sets: Membership::default(),
            sites: Membership::default(),
            stamp: EDIT,
        })
        .await
        .unwrap()
        .unwrap()
}

async fn update(
    pool: &SqlitePool,
    row: &ItemRow,
    values: &[ValueTree],
) -> Result<(), omeka_db::DbError> {
    ItemRepo::new(pool)
        .save(&ItemSave {
            write: ItemWrite::Update(row),
            values: Some(values),
            item_sets: Membership::default(),
            sites: Membership::default(),
            stamp: EDIT,
        })
        .await
        .map(|_| ())
}

async fn texts(pool: &SqlitePool, row: &ItemRow) -> Vec<String> {
    omeka_db::repos::ValueRepo::new(pool)
        .list_for_resource(row.resource.id)
        .await
        .unwrap()
        .into_iter()
        .filter_map(|value| value.value)
        .collect()
}

#[tokio::test]
async fn a_failing_save_keeps_the_item_and_records_nothing() {
    let pool = common::memory().await;
    let [title] = properties(&pool, &["title"]).await[..] else {
        unreachable!()
    };
    let row = create(&pool, &[tree(title, "Letter")]).await;

    let refused = update(
        &pool,
        &row,
        &[
            tree(title, "Letter to Anna"),
            tree(PropertyId(999_999), "x"),
        ],
    )
    .await;

    assert!(refused.is_err());
    assert_eq!(texts(&pool, &row).await, ["Letter"]);
    let entries = ItemHistoryRepo::new(&pool).list(row.id()).await.unwrap();
    assert_eq!(entries.len(), 1, "only the create");
}

#[tokio::test]
async fn a_save_that_changes_nothing_records_nothing() {
    let pool = common::memory().await;
    let [title] = properties(&pool, &["title"]).await[..] else {
        unreachable!()
    };
    let row = create(&pool, &[tree(title, "Letter")]).await;

    update(&pool, &row, &[tree(title, "Letter")]).await.unwrap();

    assert_eq!(
        ItemHistoryRepo::new(&pool)
            .list(row.id())
            .await
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn a_runs_second_pass_folds_into_its_entry() {
    let pool = common::memory().await;
    let [title, subject] = properties(&pool, &["title", "subject"]).await[..] else {
        unreachable!()
    };
    let set = ItemSetRepo::new(&pool)
        .create(&NewItemSet::default())
        .await
        .unwrap()
        .id();
    let row = create(&pool, &[tree(title, "Letter")]).await;
    let import = HistoryStamp {
        user: None,
        source: HistorySource::Import(ImportId(7)),
    };
    let pass = |values: Vec<ValueTree>, sets: Vec<ItemSetId>| {
        let pool = pool.clone();
        let row = row.clone();
        async move {
            let mut tx = pool.begin().await.unwrap();
            let before = ImportWrites::new(&mut tx)
                .before_image(ImportResource::Item(row.id()))
                .await
                .unwrap();
            sqlx::query("DELETE FROM value WHERE resource_id = ?")
                .bind(row.id().0 as i64)
                .execute(&mut *tx)
                .await
                .unwrap();
            for value in values {
                sqlx::query("INSERT INTO value (resource_id, property_id, type, value, is_public) VALUES (?, ?, 'literal', ?, 1)")
                    .bind(row.id().0 as i64)
                    .bind(value.content.property_id.0 as i64)
                    .bind(value.content.value)
                    .execute(&mut *tx)
                    .await
                    .unwrap();
            }
            sqlx::query("DELETE FROM item_item_set WHERE item_id = ?")
                .bind(row.id().0 as i64)
                .execute(&mut *tx)
                .await
                .unwrap();
            for set in sets {
                sqlx::query("INSERT INTO item_item_set (item_id, item_set_id) VALUES (?, ?)")
                    .bind(row.id().0 as i64)
                    .bind(set.0 as i64)
                    .execute(&mut *tx)
                    .await
                    .unwrap();
            }
            ImportWrites::new(&mut tx)
                .record_history(row.id(), before.as_ref(), &import)
                .await
                .unwrap();
            tx.commit().await.unwrap();
        }
    };

    pass(vec![tree(title, "Letter to Anna")], vec![set]).await;
    pass(
        vec![tree(title, "Letter to Anna"), tree(subject, "letters")],
        vec![],
    )
    .await;

    let entries = ItemHistoryRepo::new(&pool).list(row.id()).await.unwrap();
    assert_eq!(entries.len(), 2, "the create and one entry for the import");
    let changes = &entries[0].changes;
    assert_eq!(changes.len(), 2, "the set came and went: {changes:?}");
    assert!(matches!(&changes[0], FieldChange::Values { property, .. } if *property == title));
    assert!(
        matches!(&changes[1], FieldChange::Values { property, before, .. } if *property == subject && before.is_empty())
    );

    pass(vec![tree(title, "Letter")], vec![]).await;

    let entries = ItemHistoryRepo::new(&pool).list(row.id()).await.unwrap();
    assert_eq!(
        entries.len(),
        1,
        "the import put everything back, so its entry goes"
    );
}

/// The table's size per 1000 edits, and what the entry adds to one save: the image read before
/// the write and `record_history` after it, on a file database like a real install. Run with
/// `cargo test -p omeka-db --test item_history -- --ignored --nocapture`.
#[tokio::test]
#[ignore = "a measurement, not a check"]
async fn measure_history_cost() {
    const EDITS: usize = 1000;
    let (pool, path) = common::file("history-cost").await;
    let names = [
        "title",
        "creator",
        "date",
        "description",
        "subject",
        "identifier",
        "type",
        "format",
        "rights",
        "language",
    ];
    let props = properties(&pool, &names).await;
    let full = |n: usize| -> Vec<ValueTree> {
        props
            .iter()
            .zip(names)
            .map(|(property, name)| {
                let text = if name == "date" {
                    format!("19{:02}", n % 100)
                } else {
                    format!("A typical {name} of a catalogue record, about forty letters")
                };
                tree(*property, &text)
            })
            .collect()
    };
    let row = create(&pool, &full(0)).await;
    let mut saves = Duration::ZERO;
    for n in 1..=EDITS {
        let started = Instant::now();
        update(&pool, &row, &full(n)).await.unwrap();
        saves += started.elapsed();
    }
    let mut history = Duration::ZERO;
    for n in 1..=EDITS {
        let mut tx = pool.begin().await.unwrap();
        let started = Instant::now();
        let before = ImportWrites::new(&mut tx)
            .before_image(ImportResource::Item(row.id()))
            .await
            .unwrap();
        history += started.elapsed();
        sqlx::query("UPDATE value SET value = ? WHERE resource_id = ? AND property_id = ?")
            .bind(format!("18{:02}", n % 100))
            .bind(row.id().0 as i64)
            .bind(props[2].0 as i64)
            .execute(&mut *tx)
            .await
            .unwrap();
        let started = Instant::now();
        ImportWrites::new(&mut tx)
            .record_history(row.id(), before.as_ref(), &EDIT)
            .await
            .unwrap();
        history += started.elapsed();
        tx.rollback().await.unwrap();
    }
    let rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM romeka_item_history")
        .fetch_one(&pool)
        .await
        .unwrap();
    let json: i64 = sqlx::query_scalar("SELECT SUM(LENGTH(changes)) FROM romeka_item_history")
        .fetch_one(&pool)
        .await
        .unwrap();
    let settle = |pool: SqlitePool| async move {
        sqlx::query("VACUUM").execute(&pool).await.unwrap();
        sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
            .execute(&pool)
            .await
            .unwrap();
    };
    settle(pool.clone()).await;
    let with = std::fs::metadata(&path).unwrap().len();
    sqlx::query("DELETE FROM romeka_item_history")
        .execute(&pool)
        .await
        .unwrap();
    settle(pool.clone()).await;
    let without = std::fs::metadata(&path).unwrap().len();
    println!(
        "entries {rows}; changes JSON {json} bytes; file {with} with, {without} without, {} bytes per 1000 edits",
        (with - without) * 1000 / u64::try_from(rows).unwrap()
    );
    println!(
        "save with entry {:?} per save; entry part {:?} per save",
        saves / EDITS as u32,
        history / EDITS as u32
    );
    pool.close().await;
    common::remove_db(&path);
}
