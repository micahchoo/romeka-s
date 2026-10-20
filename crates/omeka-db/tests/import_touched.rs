//! The reads of import undo's touched test (`ImportTouchRepo`), and `ResourceRepo::touch`, on
//! SQLite and, with the `mysql` feature, on MySQL and MariaDB.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;
mod import_db;

use import_db::{draft, on_both};
use omeka_db::repos::{
    ImportResourceRepo, ImportTouchRepo, ItemRepo, ItemSetRepo, MediaRepo, Presence, PropertyRepo,
    ResourceRepo, TemplateRepo, ValueRepo, VocabularyRepo,
};
use omeka_db::rows::{
    ImportChange, ImportTarget, NewImportResource, NewItem, NewItemSet, NewMedia, NewProperty,
    NewResource, NewTemplate, NewVocabulary, ValueContent,
};

on_both!(the_touch_reads_answer_one_fact_each, |pool| {
    let vocabulary = VocabularyRepo::<Db>::new(pool)
        .create(&NewVocabulary {
            owner_id: None,
            namespace_uri: "http://purl.org/dc/terms/".into(),
            prefix: "dcterms".into(),
            label: "Dublin Core".into(),
            comment: None,
        })
        .await
        .unwrap();
    let relation = PropertyRepo::<Db>::new(pool)
        .create(&NewProperty {
            owner_id: None,
            vocabulary_id: vocabulary.id,
            local_name: "relation".into(),
            label: "Relation".into(),
            comment: None,
        })
        .await
        .unwrap();
    let unused = PropertyRepo::<Db>::new(pool)
        .create(&NewProperty {
            owner_id: None,
            vocabulary_id: vocabulary.id,
            local_name: "unused".into(),
            label: "Unused".into(),
            comment: None,
        })
        .await
        .unwrap();
    let template = TemplateRepo::<Db>::new(pool)
        .create(&NewTemplate {
            owner_id: None,
            resource_class_id: None,
            title_property_id: None,
            description_property_id: None,
            label: "Letters".into(),
        })
        .await
        .unwrap();
    let items = ItemRepo::<Db>::new(pool);
    let target = items.create(&NewItem::default()).await.unwrap();
    let linker = items
        .create(&NewItem {
            resource: NewResource {
                resource_template_id: Some(template.id),
                ..NewResource::default()
            },
            primary_media_id: None,
        })
        .await
        .unwrap();
    ValueRepo::<Db>::new(pool)
        .create(
            &ValueContent {
                r#type: "resource:item".into(),
                value: None,
                value_resource_id: Some(target.resource.id),
                ..ValueContent::literal(relation.id, "")
            }
            .on(linker.resource.id),
        )
        .await
        .unwrap();
    let set = ItemSetRepo::<Db>::new(pool)
        .create(&NewItemSet::default())
        .await
        .unwrap();
    items.add_to_item_set(linker.id(), set.id()).await.unwrap();
    let media = MediaRepo::<Db>::new(pool)
        .create(&NewMedia {
            item_id: target.id(),
            ingester: "html".into(),
            renderer: "html".into(),
            ..NewMedia::default()
        })
        .await
        .unwrap();
    let reads = ImportTouchRepo::<Db>::new(pool);

    assert!(matches!(
        reads
            .presence(ImportTarget::Item(target.id()))
            .await
            .unwrap(),
        Presence::Here { modified: Some(_) }
    ));
    assert_eq!(
        reads
            .presence(ImportTarget::ItemSet(omeka_db::ids::ItemSetId(
                target.id().0
            )))
            .await
            .unwrap(),
        Presence::Gone,
        "an item is not an item set"
    );
    assert_eq!(
        reads
            .presence(ImportTarget::ResourceTemplate(template.id))
            .await
            .unwrap(),
        Presence::Here { modified: None }
    );
    let linkers = reads.linkers(target.resource.id).await.unwrap();
    assert_eq!(linkers.len(), 1);
    assert_eq!(linkers[0].resource, linker.resource.id);
    assert_eq!(linkers[0].modified, linker.resource.modified);
    let members = reads.set_members(set.id()).await.unwrap();
    assert_eq!(members.len(), 1);
    assert_eq!(members[0].resource, linker.resource.id);
    assert_eq!(reads.media_of(target.id()).await.unwrap(), [media.id()]);
    assert!(reads.template_in_use(template.id).await.unwrap());
    assert!(reads.property_in_use(relation.id).await.unwrap());
    assert!(!reads.property_in_use(unused.id).await.unwrap());

    let first = draft!(pool, "a");
    let later = draft!(pool, "b");
    let row = ImportResourceRepo::<Db>::record(
        pool,
        first,
        &NewImportResource {
            target: ImportTarget::Item(target.id()),
            record: "group:A".into(),
            change: ImportChange::Created,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        reads
            .later_imports(first, ImportTarget::Item(target.id()), row.written)
            .await
            .unwrap(),
        [],
        "an import is never later than itself"
    );
    let earlier = row.written - chrono::TimeDelta::seconds(5);
    ImportResourceRepo::<Db>::record(
        pool,
        later,
        &NewImportResource {
            target: ImportTarget::Item(target.id()),
            record: "group:A".into(),
            change: ImportChange::Changed,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        reads
            .later_imports(first, ImportTarget::Item(target.id()), earlier)
            .await
            .unwrap(),
        [later]
    );

    let before = linker.resource.modified.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
    ResourceRepo::<Db>::new(pool)
        .touch(&[linker.resource.id])
        .await
        .unwrap();
    let Presence::Here { modified } = reads
        .presence(ImportTarget::Item(linker.id()))
        .await
        .unwrap()
    else {
        panic!("the linker is there");
    };
    assert!(modified.unwrap() > before, "touch moves `modified`");
});
