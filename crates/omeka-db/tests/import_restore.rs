//! Undo's promise (`docs/import-v2.md`, "Actions"): every action but `delete` is undone by the
//! before-image plus deleting the media the import added, and no import write removes a media
//! row. Every test runs on SQLite and, with the `mysql` feature, on MySQL.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;
mod import_db;
mod import_writes_db;

use import_db::on_both;
use import_writes_db::{World, link, literal, uri, world};
use omeka_db::ids::{ItemId, MediaId};
use omeka_db::repos::{
    FulltextRepo, ImportWrites, ItemRepo, MediaFilter, MediaRepo, Paging, ResourceRepo, ValueRepo,
};
use omeka_db::rows::{
    BeforeImage, ImportAction, ImportFields, ImportResource, ItemSlots, NewImportItem,
    NewImportMedia, NewResource, NewValue, Slot, StoredFile, ValueContent,
};

const ACTIONS: [ImportAction; 4] = [
    ImportAction::Append,
    ImportAction::Revise,
    ImportAction::Update,
    ImportAction::Replace,
];

fn stored(name: &str) -> StoredFile {
    StoredFile {
        storage_id: name.to_string(),
        extension: Some("tif".to_string()),
        media_type: Some("image/tiff".to_string()),
        sha256: None,
        size: Some(10),
        has_thumbnails: false,
    }
}

fn media(item: ItemId, w: &World, name: &str, position: i64) -> NewImportMedia {
    NewImportMedia {
        item,
        fields: w.fields(vec![literal(w.title, name, None)]),
        file: stored(name),
        ingester: "upload".to_string(),
        source: Some(format!("{name}.tif")),
        position,
    }
}

/// A small deterministic generator, so a failing case names its seed and replays.
struct Cases(u64);

impl Cases {
    fn next(&mut self, below: usize) -> usize {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((self.0 >> 33) % below as u64) as usize
    }

    fn pick<T: Clone>(&mut self, from: &[T]) -> T {
        from[self.next(from.len())].clone()
    }

    fn fields(&mut self, w: &World) -> (ImportFields, ItemSlots) {
        let pool = [
            literal(w.title, "Letter", Some("en")),
            literal(w.title, "Neu", Some("de")),
            literal(w.alternative, "Lettre nouvelle", Some("fr")),
            literal(w.description, "ink", None),
            ValueContent {
                is_public: false,
                ..literal(w.description, "hidden", Some("en"))
            },
            uri(w.subject, "http://example.org/ink", "Ink"),
            link(w.relation, w.other),
        ];
        let values = (0..self.next(4)).map(|_| self.pick(&pool)).collect();
        let terms = [w.title, w.alternative, w.description, w.subject, w.relation];
        let mapped_terms = terms.into_iter().filter(|_| self.next(2) == 0).collect();
        let fields = ImportFields {
            values,
            mapped_terms,
            template: self.pick(&[
                Slot::Unmapped,
                Slot::Mapped(None),
                Slot::Mapped(Some(w.template)),
            ]),
            class: self.pick(&[
                Slot::Unmapped,
                Slot::Mapped(None),
                Slot::Mapped(Some(w.class)),
            ]),
            owner: self.pick(&[
                Slot::Unmapped,
                Slot::Mapped(None),
                Slot::Mapped(Some(w.users[1])),
            ]),
            is_public: self.pick(&[Slot::Unmapped, Slot::Mapped(true), Slot::Mapped(false)]),
        };
        let slots = ItemSlots {
            item_sets: self.pick(&[
                Slot::Unmapped,
                Slot::Mapped(Vec::new()),
                Slot::Mapped(vec![w.sets[1]]),
                Slot::Mapped(w.sets.to_vec()),
            ]),
            sites: self.pick(&[
                Slot::Unmapped,
                Slot::Mapped(Vec::new()),
                Slot::Mapped(vec![w.sites[1]]),
            ]),
        };
        (fields, slots)
    }
}

/// An item with values in three languages and four types, a private value, an annotated value,
/// a template, a class, one set, one site and two media, the second primary.
macro_rules! rich_item {
    ($pool:expr, $w:expr) => {{
        let w: &World = &$w;
        let mut fields = w.fields(vec![
            literal(w.title, "Letter", Some("en")),
            literal(w.title, "Lettre", Some("fr")),
            literal(w.alternative, "Brief", Some("de")),
            ValueContent {
                is_public: false,
                ..literal(w.description, "secret", None)
            },
            uri(w.subject, "http://example.org/post", "Post"),
            link(w.relation, w.other),
        ]);
        fields.template = Some(w.template);
        fields.class = Some(w.class);
        let mut tx = $pool.begin().await.unwrap();
        let mut writes = ImportWrites::<Db>::new(&mut tx);
        let id = writes
            .create_item(&NewImportItem {
                fields,
                item_sets: vec![w.sets[0]],
                sites: vec![w.sites[0]],
            })
            .await
            .unwrap();
        writes
            .create_media(&media(id, w, "front", 1))
            .await
            .unwrap();
        let back = writes.create_media(&media(id, w, "back", 2)).await.unwrap();
        writes.set_primary_media(id, Some(back)).await.unwrap();
        tx.commit().await.unwrap();

        let annotation = ResourceRepo::<Db>::new($pool)
            .create_annotation(&NewResource::default())
            .await
            .unwrap();
        let values = ValueRepo::<Db>::new($pool);
        values
            .create(&NewValue {
                value_annotation_id: Some(annotation.resource.id.0.into()),
                ..literal(w.description, "annotated", Some("en")).on(id.as_resource())
            })
            .await
            .unwrap();
        values
            .create(&literal(w.description, "a note on it", None).on(annotation.resource.id))
            .await
            .unwrap();
        id
    }};
}

macro_rules! image_of {
    ($pool:expr, $resource:expr) => {{
        let mut tx = $pool.begin().await.unwrap();
        let image = ImportWrites::<Db>::new(&mut tx)
            .before_image($resource)
            .await
            .unwrap()
            .unwrap();
        tx.commit().await.unwrap();
        image
    }};
}

macro_rules! media_count {
    ($pool:expr) => {
        MediaRepo::<Db>::new($pool)
            .list(&MediaFilter::default(), &Paging::default())
            .await
            .unwrap()
            .len()
    };
}

on_both!(
    restore_after_any_action_gives_back_the_prior_state,
    |pool| {
        let w = world!(pool);
        let id = rich_item!(pool, w);
        let target = ImportResource::Item(id);
        let items = ItemRepo::<Db>::new(pool);
        let original: BeforeImage = image_of!(pool, target);
        let original_title = items.get(id).await.unwrap().unwrap().resource.title;
        assert_eq!(original.values.len(), 7);
        assert_eq!(original.values[6].annotation.len(), 1);
        assert_eq!(
            BeforeImage::from_json(&original.to_json()).unwrap(),
            original
        );

        let mut cases = Cases(0x5eed);
        let mut changed = 0;
        for action in ACTIONS {
            for case in 0..6 {
                let (fields, slots) = cases.fields(&w);
                let context = format!("{action:?} case {case}: {fields:?} {slots:?}");

                let mut tx = pool.begin().await.unwrap();
                let mut writes = ImportWrites::<Db>::new(&mut tx);
                let image = writes.before_image(target).await.unwrap().unwrap();
                assert!(
                    writes
                        .apply_item(id, action, &fields, &slots)
                        .await
                        .unwrap()
                );
                if writes.before_image(target).await.unwrap().unwrap() != original {
                    changed += 1;
                }
                let added = writes
                    .create_media(&media(id, &w, "added", 3))
                    .await
                    .unwrap();
                writes.set_primary_media(id, Some(added)).await.unwrap();
                tx.commit().await.unwrap();
                assert_eq!(image, original, "{context}");

                let mut tx = pool.begin().await.unwrap();
                assert!(
                    ImportWrites::<Db>::new(&mut tx)
                        .restore(target, &image)
                        .await
                        .unwrap()
                );
                tx.commit().await.unwrap();
                // Undo deletes the media the import added after it restores.
                assert!(MediaRepo::<Db>::new(pool).delete(added).await.unwrap());

                assert_eq!(image_of!(pool, target), original, "{context}");
                let row = items.get(id).await.unwrap().unwrap();
                assert_eq!(row.resource.title, original_title, "{context}");
            }
        }
        assert!(changed >= 18, "only {changed} of 24 cases changed the item");
        let hits = FulltextRepo::<Db>::new(pool)
            .search("items", "Brief")
            .await
            .unwrap();
        assert_eq!(
            hits.iter().map(|hit| hit.id).collect::<Vec<_>>(),
            [id.as_resource()]
        );
        let gone = FulltextRepo::<Db>::new(pool)
            .search("items", "Neu nouvelle")
            .await
            .unwrap();
        assert_eq!(gone, []);
    }
);

on_both!(no_import_write_deletes_a_media_row, |pool| {
    let w = world!(pool);
    let id = rich_item!(pool, w);
    let target = ImportResource::Item(id);
    let bare = image_of!(pool, target);
    assert_eq!(media_count!(pool), 2);

    for action in ACTIONS {
        let mut tx = pool.begin().await.unwrap();
        let mut writes = ImportWrites::<Db>::new(&mut tx);
        let fields = ImportFields {
            mapped_terms: vec![w.title, w.description],
            ..ImportFields::default()
        };
        let slots = ItemSlots {
            item_sets: Slot::Mapped(Vec::new()),
            sites: Slot::Mapped(Vec::new()),
        };
        writes
            .apply_item(id, action, &fields, &slots)
            .await
            .unwrap();
        writes.restore(target, &bare).await.unwrap();
        tx.commit().await.unwrap();
        assert_eq!(media_count!(pool), 2, "{action:?}");
    }

    let mut tx = pool.begin().await.unwrap();
    let extra = ImportWrites::<Db>::new(&mut tx)
        .create_media(&media(id, &w, "extra", 3))
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let mut tx = pool.begin().await.unwrap();
    ImportWrites::<Db>::new(&mut tx)
        .restore(target, &bare)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(
        media_count!(pool),
        3,
        "a media the image does not list stays"
    );
    let order: Vec<MediaId> = MediaRepo::<Db>::new(pool)
        .list(
            &MediaFilter {
                item_id: Some(id),
                ..MediaFilter::default()
            },
            &Paging::default(),
        )
        .await
        .unwrap()
        .into_iter()
        .map(|row| row.id())
        .collect();
    assert_eq!(
        order.last(),
        Some(&extra),
        "it keeps its place after the imaged media"
    );

    let first = order[0];
    MediaRepo::<Db>::new(pool).delete(first).await.unwrap();
    let mut tx = pool.begin().await.unwrap();
    ImportWrites::<Db>::new(&mut tx)
        .restore(target, &bare)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(media_count!(pool), 2, "a deleted media is not made again");
});

on_both!(restore_skips_what_was_deleted_since_the_image, |pool| {
    let w = world!(pool);
    let id = rich_item!(pool, w);
    let target = ImportResource::Item(id);
    let image = image_of!(pool, target);
    ItemRepo::<Db>::new(pool).delete(w.other).await.unwrap();
    omeka_db::repos::ItemSetRepo::<Db>::new(pool)
        .delete(w.sets[0])
        .await
        .unwrap();

    let mut tx = pool.begin().await.unwrap();
    assert!(
        ImportWrites::<Db>::new(&mut tx)
            .restore(target, &image)
            .await
            .unwrap()
    );
    tx.commit().await.unwrap();
    let after = image_of!(pool, target);
    assert_eq!(
        after.values.len(),
        image.values.len() - 1,
        "the link to a gone item is skipped"
    );
    assert_eq!(after.template, image.template);
    assert_eq!(after.memberships, {
        let mut kept = image.memberships.clone();
        if let omeka_db::rows::ImageMemberships::Item { item_sets, .. } = &mut kept {
            item_sets.clear();
        }
        kept
    });
});

on_both!(a_media_and_an_item_set_restore_too, |pool| {
    let w = world!(pool);
    let id = rich_item!(pool, w);
    let front = MediaRepo::<Db>::new(pool)
        .list(
            &MediaFilter {
                item_id: Some(id),
                ..MediaFilter::default()
            },
            &Paging::default(),
        )
        .await
        .unwrap()[0]
        .id();
    let mut tx = pool.begin().await.unwrap();
    let set = ImportWrites::<Db>::new(&mut tx)
        .create_item_set(&omeka_db::rows::NewImportItemSet {
            fields: w.fields(vec![literal(w.title, "Letters", Some("en"))]),
            sites: w.sites.to_vec(),
        })
        .await
        .unwrap();
    tx.commit().await.unwrap();

    let mut cases = Cases(7);
    for target in [ImportResource::Media(front), ImportResource::ItemSet(set)] {
        let original = image_of!(pool, target);
        for action in ACTIONS {
            let (fields, slots) = cases.fields(&w);
            let mut tx = pool.begin().await.unwrap();
            let mut writes = ImportWrites::<Db>::new(&mut tx);
            match target {
                ImportResource::Media(media) => {
                    assert!(writes.apply_media(media, action, &fields).await.unwrap());
                }
                ImportResource::ItemSet(set) => {
                    assert!(
                        writes
                            .apply_item_set(set, action, &fields, &slots.sites)
                            .await
                            .unwrap()
                    );
                }
                ImportResource::Item(_) => unreachable!(),
            }
            writes.restore(target, &original).await.unwrap();
            tx.commit().await.unwrap();
            assert_eq!(image_of!(pool, target), original, "{target:?} {action:?}");
        }
    }
});
