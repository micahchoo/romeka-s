//! What an import writes inside a record's transaction (`docs/import-v2.md`, "Actions" and
//! "Lifecycle and the ledger"): the creates, the four actions on a matched resource, the
//! before-image and the restore. Every test runs on SQLite and, with the `mysql` feature, on MySQL.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;
mod import_db;
mod import_writes_db;

use import_db::on_both;
use import_writes_db::{link, literal, uri, world};
use omeka_db::repos::{
    FulltextRepo, ImportWrites, ItemRepo, ItemSetRepo, MediaRepo, ResourceRepo, SiteRepo, ValueRepo,
};
use omeka_db::rows::{
    BeforeImage, ImageMemberships, ImportAction, ImportFields, ImportResource, ItemSlots,
    NewImportItem, NewImportItemSet, NewImportMedia, Slot, StoredFile,
};

fn stored(name: &str) -> StoredFile {
    StoredFile {
        storage_id: format!("{name}-storage"),
        extension: Some("jpg".to_string()),
        media_type: Some("image/jpeg".to_string()),
        sha256: Some("ab".repeat(32)),
        size: Some(1234),
        has_thumbnails: true,
    }
}

on_both!(a_created_item_carries_every_slot_and_is_found, |pool| {
    let w = world!(pool);
    let mut fields = w.fields(vec![
        literal(w.title, "Letter to Ada", Some("en")),
        literal(w.alternative, "Lettre à Ada", Some("fr")),
        uri(w.subject, "http://example.org/post", "Post"),
        link(w.relation, w.other),
    ]);
    fields.template = Some(w.template);
    fields.class = Some(w.class);

    let mut tx = pool.begin().await.unwrap();
    let id = ImportWrites::<Db>::new(&mut tx)
        .create_item(&NewImportItem {
            fields,
            item_sets: vec![w.sets[1]],
            sites: vec![w.sites[0]],
        })
        .await
        .unwrap();
    tx.commit().await.unwrap();

    let items = ItemRepo::<Db>::new(pool);
    let row = items.get(id).await.unwrap().unwrap();
    // The template names `dcterms:alternative` as its title property.
    assert_eq!(row.resource.title.as_deref(), Some("Lettre à Ada"));
    assert_eq!(row.resource.resource_template_id, Some(w.template));
    assert_eq!(row.resource.resource_class_id, Some(w.class));
    assert_eq!(row.resource.owner_id, Some(w.users[0]));
    assert!(!row.resource.is_public);
    assert_eq!(row.resource.modified, Some(row.resource.created));
    assert_eq!(items.list_item_sets(id).await.unwrap(), [w.sets[1]]);
    assert_eq!(items.list_sites(id).await.unwrap(), [w.sites[0]]);
    let values = ValueRepo::<Db>::new(pool)
        .list_for_resource(id.as_resource())
        .await
        .unwrap();
    assert_eq!(values.len(), 4);
    assert_eq!(values[2].uri.as_deref(), Some("http://example.org/post"));
    assert_eq!(values[3].value_resource_id, Some(w.other.as_resource()));

    let found = FulltextRepo::<Db>::new(pool)
        .search("items", "Ada")
        .await
        .unwrap();
    assert_eq!(
        found.iter().map(|hit| hit.id).collect::<Vec<_>>(),
        [id.as_resource()]
    );
    let by_link = FulltextRepo::<Db>::new(pool)
        .search("items", "Linked")
        .await
        .unwrap();
    assert!(by_link.iter().any(|hit| hit.id == id.as_resource()));
});

on_both!(a_rolled_back_record_leaves_nothing, |pool| {
    let w = world!(pool);
    let mut tx = pool.begin().await.unwrap();
    let mut writes = ImportWrites::<Db>::new(&mut tx);
    let item = writes
        .create_item(&NewImportItem {
            fields: w.fields(vec![literal(w.title, "Ghost", None)]),
            item_sets: vec![w.sets[0]],
            sites: vec![w.sites[0]],
        })
        .await
        .unwrap();
    writes
        .create_media(&NewImportMedia {
            item,
            fields: w.fields(Vec::new()),
            file: Some(stored("ghost")),
            ingester: "sideload".to_string(),
            renderer: "file".to_string(),
            data: None,
            source: None,
            position: 1,
        })
        .await
        .unwrap();
    tx.rollback().await.unwrap();

    assert_eq!(ItemRepo::<Db>::new(pool).get(item).await.unwrap(), None);
    assert_eq!(
        MediaRepo::<Db>::new(pool)
            .list(&Default::default(), &Default::default())
            .await
            .unwrap(),
        []
    );
    assert_eq!(
        FulltextRepo::<Db>::new(pool)
            .search("items", "Ghost")
            .await
            .unwrap(),
        []
    );
});

on_both!(a_created_media_names_its_stored_file, |pool| {
    let w = world!(pool);
    let mut tx = pool.begin().await.unwrap();
    let mut writes = ImportWrites::<Db>::new(&mut tx);
    let item = writes
        .create_item(&NewImportItem {
            fields: w.fields(vec![literal(w.title, "Album", None)]),
            item_sets: Vec::new(),
            sites: Vec::new(),
        })
        .await
        .unwrap();
    let mut media_fields = w.fields(vec![literal(w.title, "Page one", None)]);
    media_fields.is_public = true;
    let media = writes
        .create_media(&NewImportMedia {
            item,
            fields: media_fields,
            file: Some(stored("page-1")),
            ingester: "sideload".to_string(),
            renderer: "file".to_string(),
            data: None,
            source: Some("album/page-1.jpg".to_string()),
            position: 1,
        })
        .await
        .unwrap();
    writes.set_primary_media(item, Some(media)).await.unwrap();
    tx.commit().await.unwrap();

    let row = MediaRepo::<Db>::new(pool)
        .get(media)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.item_id, item);
    assert_eq!(row.storage_id.as_deref(), Some("page-1-storage"));
    assert_eq!(row.extension.as_deref(), Some("jpg"));
    assert_eq!(row.media_type.as_deref(), Some("image/jpeg"));
    assert_eq!(row.sha256, Some("ab".repeat(32)));
    assert_eq!(row.size, Some(1234));
    assert_eq!(row.source.as_deref(), Some("album/page-1.jpg"));
    assert_eq!(row.ingester, "sideload");
    assert_eq!(row.renderer, "file");
    assert!(row.has_original && row.has_thumbnails);
    assert_eq!(row.position, Some(1));
    assert!(row.resource.is_public);
    assert_eq!(row.resource.title.as_deref(), Some("Page one"));
    let item_row = ItemRepo::<Db>::new(pool).get(item).await.unwrap().unwrap();
    assert_eq!(item_row.primary_media_id, Some(media));

    let fulltext = FulltextRepo::<Db>::new(pool);
    let media_hits = fulltext.search("media", "Page").await.unwrap();
    assert_eq!(media_hits.len(), 1);
    // `ItemAdapter::getFulltextText` includes the item's media.
    let item_hits = fulltext.search("items", "Page").await.unwrap();
    assert_eq!(
        item_hits.iter().map(|hit| hit.id).collect::<Vec<_>>(),
        [item.as_resource()]
    );
});

on_both!(a_created_item_set_joins_its_sites, |pool| {
    let w = world!(pool);
    let mut tx = pool.begin().await.unwrap();
    let id = ImportWrites::<Db>::new(&mut tx)
        .create_item_set(&NewImportItemSet {
            fields: w.fields(vec![literal(w.title, "Correspondence", None)]),
            is_open: false,
            sites: vec![w.sites[1]],
        })
        .await
        .unwrap();
    tx.commit().await.unwrap();

    let row = ItemSetRepo::<Db>::new(pool).get(id).await.unwrap().unwrap();
    assert_eq!(row.resource.title.as_deref(), Some("Correspondence"));
    assert!(!row.is_open);
    let listed = SiteRepo::<Db>::new(pool)
        .list_item_sets(w.sites[1])
        .await
        .unwrap();
    assert_eq!(
        listed.iter().map(|row| row.item_set_id).collect::<Vec<_>>(),
        [id]
    );
    assert_eq!(listed[0].position, Some(1));
    let hits = FulltextRepo::<Db>::new(pool)
        .search("item_sets", "Correspondence")
        .await
        .unwrap();
    assert_eq!(
        hits.iter().map(|hit| hit.id).collect::<Vec<_>>(),
        [id.as_resource()]
    );
});

/// The values of one item as `(property, text, lang)`, in stored order.
macro_rules! texts {
    ($pool:expr, $item:expr) => {
        ValueRepo::<Db>::new($pool)
            .list_for_resource($item.as_resource())
            .await
            .unwrap()
            .into_iter()
            .map(|row| {
                (
                    row.property_id,
                    row.value.or(row.uri).unwrap_or_default(),
                    row.lang,
                )
            })
            .collect::<Vec<_>>()
    };
}

/// A committed item with a title in English, a description in French and one set and site.
macro_rules! seeded {
    ($pool:expr, $w:expr) => {{
        let mut tx = $pool.begin().await.unwrap();
        let id = ImportWrites::<Db>::new(&mut tx)
            .create_item(&NewImportItem {
                fields: $w.fields(vec![
                    literal($w.title, "Old", Some("en")),
                    literal($w.description, "Ancienne", Some("fr")),
                ]),
                item_sets: vec![$w.sets[0]],
                sites: vec![$w.sites[0]],
            })
            .await
            .unwrap();
        tx.commit().await.unwrap();
        id
    }};
}

macro_rules! apply {
    ($pool:expr, $id:expr, $action:expr, $fields:expr, $slots:expr) => {{
        let mut tx = $pool.begin().await.unwrap();
        let found = ImportWrites::<Db>::new(&mut tx)
            .apply_item($id, $action, &$fields, &$slots)
            .await
            .unwrap();
        tx.commit().await.unwrap();
        assert!(found);
    }};
}

on_both!(append_adds_distinct_values_and_memberships, |pool| {
    let w = world!(pool);
    let id = seeded!(pool, w);
    let fields = ImportFields {
        values: vec![
            literal(w.title, "Old", Some("en")),
            literal(w.title, "Old", Some("de")),
            literal(w.title, "Old", Some("de")),
        ],
        mapped_terms: vec![w.title],
        template: Slot::Mapped(Some(w.template)),
        owner: Slot::Mapped(Some(w.users[1])),
        is_public: Slot::Mapped(true),
        ..ImportFields::default()
    };
    let slots = ItemSlots {
        item_sets: Slot::Mapped(vec![w.sets[1]]),
        sites: Slot::Mapped(vec![w.sites[1]]),
    };
    apply!(pool, id, ImportAction::Append, fields, slots);

    assert_eq!(
        texts!(pool, id),
        [
            (w.title, "Old".to_string(), Some("en".to_string())),
            (
                w.description,
                "Ancienne".to_string(),
                Some("fr".to_string())
            ),
            (w.title, "Old".to_string(), Some("de".to_string())),
        ]
    );
    let items = ItemRepo::<Db>::new(pool);
    let row = items.get(id).await.unwrap().unwrap();
    assert_eq!(row.resource.resource_template_id, None);
    assert_eq!(row.resource.owner_id, Some(w.users[0]));
    assert!(!row.resource.is_public);
    assert_eq!(items.list_item_sets(id).await.unwrap(), w.sets);
    assert_eq!(items.list_sites(id).await.unwrap(), w.sites);
});

on_both!(revise_replaces_only_slots_with_values, |pool| {
    let w = world!(pool);
    let id = seeded!(pool, w);
    let fields = ImportFields {
        values: vec![literal(w.title, "New", None)],
        mapped_terms: vec![w.title, w.description],
        template: Slot::Mapped(None),
        class: Slot::Mapped(Some(w.class)),
        ..ImportFields::default()
    };
    let slots = ItemSlots {
        item_sets: Slot::Mapped(Vec::new()),
        sites: Slot::Mapped(vec![w.sites[1]]),
    };
    apply!(pool, id, ImportAction::Revise, fields, slots);

    assert_eq!(
        texts!(pool, id),
        [
            (
                w.description,
                "Ancienne".to_string(),
                Some("fr".to_string())
            ),
            (w.title, "New".to_string(), None),
        ]
    );
    let items = ItemRepo::<Db>::new(pool);
    let row = items.get(id).await.unwrap().unwrap();
    assert_eq!(row.resource.title.as_deref(), Some("New"));
    assert_eq!(row.resource.resource_class_id, Some(w.class));
    assert_eq!(items.list_item_sets(id).await.unwrap(), [w.sets[0]]);
    assert_eq!(items.list_sites(id).await.unwrap(), [w.sites[1]]);
    let hits = FulltextRepo::<Db>::new(pool)
        .search("items", "Old")
        .await
        .unwrap();
    assert!(
        hits.iter().all(|hit| hit.id != id.as_resource()),
        "old text left the index"
    );
});

on_both!(
    update_clears_mapped_empty_slots_and_keeps_unmapped,
    |pool| {
        let w = world!(pool);
        let id = seeded!(pool, w);
        let fields = ImportFields {
            values: vec![literal(w.subject, "Post", None)],
            mapped_terms: vec![w.description, w.subject],
            owner: Slot::Mapped(None),
            ..ImportFields::default()
        };
        let slots = ItemSlots {
            item_sets: Slot::Mapped(Vec::new()),
            sites: Slot::Unmapped,
        };
        apply!(pool, id, ImportAction::Update, fields, slots);

        assert_eq!(
            texts!(pool, id),
            [
                (w.title, "Old".to_string(), Some("en".to_string())),
                (w.subject, "Post".to_string(), None),
            ]
        );
        let items = ItemRepo::<Db>::new(pool);
        let row = items.get(id).await.unwrap().unwrap();
        assert_eq!(row.resource.owner_id, None);
        assert!(!row.resource.is_public);
        assert_eq!(items.list_item_sets(id).await.unwrap(), []);
        assert_eq!(items.list_sites(id).await.unwrap(), [w.sites[0]]);
    }
);

on_both!(
    replace_removes_every_value_and_clears_unmapped_slots,
    |pool| {
        let w = world!(pool);
        let id = seeded!(pool, w);
        let fields = ImportFields {
            values: vec![literal(w.subject, "Post", None)],
            mapped_terms: vec![w.subject],
            is_public: Slot::Mapped(true),
            ..ImportFields::default()
        };
        apply!(
            pool,
            id,
            ImportAction::Replace,
            fields,
            ItemSlots::default()
        );

        assert_eq!(texts!(pool, id), [(w.subject, "Post".to_string(), None)]);
        let items = ItemRepo::<Db>::new(pool);
        let row = items.get(id).await.unwrap().unwrap();
        assert_eq!(row.resource.title, None);
        assert_eq!(row.resource.owner_id, None);
        assert!(row.resource.is_public);
        assert_eq!(items.list_item_sets(id).await.unwrap(), []);
        assert_eq!(items.list_sites(id).await.unwrap(), []);
    }
);

on_both!(apply_moves_modified_and_restore_puts_it_back, |pool| {
    let w = world!(pool);
    let id = seeded!(pool, w);
    import_db::exec_id!(
        pool,
        "UPDATE resource SET created = '2001-01-01 00:00:00', modified = '2001-01-01 00:00:00' WHERE id = ?",
        id.0
    );
    let items = ItemRepo::<Db>::new(pool);
    let created = items.get(id).await.unwrap().unwrap().resource.created;

    let mut tx = pool.begin().await.unwrap();
    let mut writes = ImportWrites::<Db>::new(&mut tx);
    let image = writes
        .before_image(ImportResource::Item(id))
        .await
        .unwrap()
        .unwrap();
    writes
        .apply_item(
            id,
            ImportAction::Append,
            &ImportFields::default(),
            &ItemSlots::default(),
        )
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let applied = items.get(id).await.unwrap().unwrap().resource;
    assert!(applied.modified > Some(created), "apply moved modified");
    assert_eq!(applied.created, created);

    let mut tx = pool.begin().await.unwrap();
    assert!(
        ImportWrites::<Db>::new(&mut tx)
            .restore(ImportResource::Item(id), &image)
            .await
            .unwrap()
    );
    tx.commit().await.unwrap();
    assert_eq!(
        items.get(id).await.unwrap().unwrap().resource.modified,
        Some(created),
        "the image's modified"
    );
});

on_both!(a_gone_resource_is_neither_imaged_nor_restored, |pool| {
    let w = world!(pool);
    let id = seeded!(pool, w);
    let mut tx = pool.begin().await.unwrap();
    let image = ImportWrites::<Db>::new(&mut tx)
        .before_image(ImportResource::Item(id))
        .await
        .unwrap()
        .unwrap();
    tx.commit().await.unwrap();
    ItemRepo::<Db>::new(pool).delete(id).await.unwrap();

    let mut tx = pool.begin().await.unwrap();
    let mut writes = ImportWrites::<Db>::new(&mut tx);
    assert_eq!(
        writes.before_image(ImportResource::Item(id)).await.unwrap(),
        None
    );
    assert!(
        !writes
            .restore(ImportResource::Item(id), &image)
            .await
            .unwrap()
    );
    assert!(
        !writes
            .apply_item(
                id,
                ImportAction::Update,
                &ImportFields::default(),
                &ItemSlots::default()
            )
            .await
            .unwrap()
    );
    tx.commit().await.unwrap();
    assert_eq!(
        ResourceRepo::<Db>::new(pool)
            .get(id.as_resource())
            .await
            .unwrap(),
        None
    );
});

on_both!(a_restore_refuses_an_image_of_another_kind, |pool| {
    let w = world!(pool);
    let id = seeded!(pool, w);
    let image = BeforeImage {
        values: Vec::new(),
        template: None,
        class: None,
        owner: None,
        is_public: true,
        modified: None,
        memberships: ImageMemberships::Media,
    };
    let mut tx = pool.begin().await.unwrap();
    let refused = ImportWrites::<Db>::new(&mut tx)
        .restore(ImportResource::Item(id), &image)
        .await;
    assert!(refused.is_err());
    tx.rollback().await.unwrap();
});
