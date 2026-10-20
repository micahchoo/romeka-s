//! What an import records about the install (migration 0006): the ledger
//! (`romeka_import_resource`), the before-images (`romeka_import_before`), and the item lookup by
//! identifier the match reads. Every test runs on SQLite and, with the `mysql` feature, on MySQL.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;
mod import_db;

use import_db::{draft, on_both};
use omeka_db::ids::{ItemId, ItemSetId, MediaId, PropertyId};
use omeka_db::repos::{
    BeforeImageRepo, IdentifierRepo, ImportRepo, ImportResourceRepo, ItemRepo, ItemSetRepo,
    PropertyRepo, VocabularyRepo,
};
use omeka_db::rows::{
    ImportChange, ImportKind, ImportTarget, NewImportResource, NewItem, NewItemSet, NewProperty,
    NewVocabulary, NewWholeItem, ValueContent,
};
use serde_json::json;

fn entry(target: ImportTarget, record: &str, change: ImportChange) -> NewImportResource {
    NewImportResource {
        target,
        record: record.to_string(),
        change,
    }
}

on_both!(a_ledger_row_commits_with_the_caller_s_transaction, |pool| {
    let id = draft!(pool, "a");
    let ledger = ImportResourceRepo::<Db>::new(pool);
    let item = ImportTarget::Item(ItemId(7));

    let mut tx = pool.begin().await.unwrap();
    ImportResourceRepo::<Db>::record(
        &mut *tx,
        id,
        &entry(item, "file:a/L_1", ImportChange::Created),
    )
    .await
    .unwrap();
    tx.rollback().await.unwrap();
    assert_eq!(ledger.list(id, None).await.unwrap(), []);

    let mut tx = pool.begin().await.unwrap();
    let written = ImportResourceRepo::<Db>::record(
        &mut *tx,
        id,
        &entry(item, "file:a/L_1", ImportChange::Created),
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(
        ledger.list(id, None).await.unwrap(),
        std::slice::from_ref(&written)
    );
    assert_eq!(written.import_id, id);
    assert_eq!(written.record, "file:a/L_1");
    assert_eq!(written.change, ImportChange::Created);
});

on_both!(
    the_ledger_lists_by_kind_finds_by_record_and_forgets_one,
    |pool| {
        let id = draft!(pool, "a");
        let other = draft!(pool, "b");
        let ledger = ImportResourceRepo::<Db>::new(pool);
        let item = ImportTarget::Item(ItemId(3));
        let media = ImportTarget::Media(MediaId(4));
        let set = ImportTarget::ItemSet(ItemSetId(5));
        for (target, record, change) in [
            (media, "file:a/L_1.jpg", ImportChange::Created),
            (item, "group:L_1", ImportChange::Changed),
            (set, "set:Letters", ImportChange::Created),
        ] {
            ImportResourceRepo::<Db>::record(pool, id, &entry(target, record, change))
                .await
                .unwrap();
        }
        ImportResourceRepo::<Db>::record(
            pool,
            other,
            &entry(item, "group:L_1", ImportChange::Created),
        )
        .await
        .unwrap();

        let targets = |rows: Vec<omeka_db::rows::ImportResourceRow>| -> Vec<ImportTarget> {
            rows.into_iter().map(|row| row.target).collect()
        };
        assert_eq!(
            targets(ledger.list(id, None).await.unwrap()),
            [item, set, media]
        );
        assert_eq!(
            targets(ledger.list(id, Some(ImportKind::Media)).await.unwrap()),
            [media]
        );
        let found = ledger.find_by_record(id, "group:L_1").await.unwrap();
        assert_eq!(targets(found.clone()), [item]);
        assert_eq!(found[0].change, ImportChange::Changed);
        assert_eq!(ledger.find_by_record(id, "group:l_1").await.unwrap(), []);

        assert!(ledger.delete(id, item).await.unwrap());
        assert!(!ledger.delete(id, item).await.unwrap());
        assert_eq!(targets(ledger.list(id, None).await.unwrap()), [set, media]);
        assert_eq!(targets(ledger.list(other, None).await.unwrap()), [item]);
    }
);

on_both!(a_before_image_is_kept_per_import_and_target, |pool| {
    let id = draft!(pool, "a");
    let images = BeforeImageRepo::<Db>::new(pool);
    let item = ImportTarget::Item(ItemId(3));
    let set = ImportTarget::ItemSet(ItemSetId(5));
    let image =
        json!({ "values": [{ "term": "dcterms:title", "value": "Old" }], "template": null });

    let mut tx = pool.begin().await.unwrap();
    BeforeImageRepo::<Db>::put(&mut *tx, id, item, &image)
        .await
        .unwrap();
    tx.rollback().await.unwrap();
    assert_eq!(images.get(id, item).await.unwrap(), None);

    BeforeImageRepo::<Db>::put(pool, id, item, &image)
        .await
        .unwrap();
    BeforeImageRepo::<Db>::put(pool, id, set, &json!({ "values": [] }))
        .await
        .unwrap();
    assert_eq!(images.get(id, item).await.unwrap(), Some(image.clone()));
    let listed: Vec<ImportTarget> = images
        .list(id)
        .await
        .unwrap()
        .into_iter()
        .map(|row| row.target)
        .collect();
    assert_eq!(listed, [item, set]);
    assert!(
        BeforeImageRepo::<Db>::put(pool, id, item, &json!({}))
            .await
            .is_err(),
        "the first image is the one before the import"
    );
    assert!(images.delete(id, item).await.unwrap());
    assert!(!images.delete(id, item).await.unwrap());
    assert_eq!(images.get(id, item).await.unwrap(), None);
});

on_both!(deleting_a_draft_removes_everything_it_recorded, |pool| {
    let id = draft!(pool, "a");
    let item = ImportTarget::Item(ItemId(3));
    ImportResourceRepo::<Db>::record(pool, id, &entry(item, "r", ImportChange::Created))
        .await
        .unwrap();
    BeforeImageRepo::<Db>::put(pool, id, item, &json!({}))
        .await
        .unwrap();
    assert!(ImportRepo::<Db>::new(pool).delete_draft(id).await.unwrap());
    assert_eq!(
        ImportResourceRepo::<Db>::new(pool)
            .list(id, None)
            .await
            .unwrap(),
        []
    );
    assert_eq!(BeforeImageRepo::<Db>::new(pool).list(id).await.unwrap(), []);
});

on_both!(
    items_are_found_by_their_trimmed_identifier_lowest_id_first,
    |pool| {
        let vocabulary = VocabularyRepo::<Db>::new(pool)
            .create(&NewVocabulary {
                owner_id: None,
                namespace_uri: "http://purl.org/dc/terms/".to_string(),
                prefix: "dcterms".to_string(),
                label: "Dublin Core".to_string(),
                comment: None,
            })
            .await
            .unwrap();
        let property = |name: &str| NewProperty {
            owner_id: None,
            vocabulary_id: vocabulary.id,
            local_name: name.to_string(),
            label: name.to_string(),
            comment: None,
        };
        let properties = PropertyRepo::<Db>::new(pool);
        let identifier: PropertyId = properties.create(&property("identifier")).await.unwrap().id;
        let title: PropertyId = properties.create(&property("title")).await.unwrap().id;
        let letters = ItemSetRepo::<Db>::new(pool)
            .create(&NewItemSet::default())
            .await
            .unwrap()
            .id();
        let photos = ItemSetRepo::<Db>::new(pool)
            .create(&NewItemSet::default())
            .await
            .unwrap()
            .id();
        let items = ItemRepo::<Db>::new(pool);
        let item = |sets: Vec<ItemSetId>, values: Vec<ValueContent>| {
            let items = &items;
            async move {
                items
                    .create_whole(&NewWholeItem {
                        item: NewItem::default(),
                        item_sets: sets,
                        values,
                    })
                    .await
                    .unwrap()
                    .id()
            }
        };
        let first = item(
            vec![letters],
            vec![ValueContent::literal(identifier, " L_1 ")],
        )
        .await;
        let second = item(vec![photos], vec![ValueContent::literal(identifier, "L_1")]).await;
        let photo = item(vec![photos], vec![ValueContent::literal(identifier, "P_1")]).await;
        item(vec![letters], vec![ValueContent::literal(title, "T_1")]).await;
        item(
            vec![letters],
            vec![ValueContent::literal(identifier, "   ")],
        )
        .await;
        let mut linked = ValueContent::literal(identifier, "U_1");
        linked.r#type = "uri".to_string();
        item(vec![letters], vec![linked]).await;

        let lookup = IdentifierRepo::<Db>::new(pool);
        let everywhere = lookup.items(None).await.unwrap();
        assert_eq!(
            everywhere.into_iter().collect::<Vec<_>>(),
            [("L_1".to_string(), first), ("P_1".to_string(), photo)]
        );
        let in_photos = lookup.items(Some(&[photos])).await.unwrap();
        assert_eq!(
            in_photos.into_iter().collect::<Vec<_>>(),
            [("L_1".to_string(), second), ("P_1".to_string(), photo)]
        );
        assert!(lookup.items(Some(&[])).await.unwrap().is_empty());
    }
);
