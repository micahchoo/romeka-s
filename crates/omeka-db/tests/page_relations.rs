//! `RelationRepo`: the rows a page of resource documents reads, one statement per relation, on
//! SQLite and, with the `mysql` feature, on MySQL and MariaDB.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;
mod import_db;

use import_db::on_both;
use omeka_db::ids::{ItemId, ItemSetId, ResourceId, TemplateId};
use omeka_db::repos::{
    AssetRepo, ItemRepo, ItemSetRepo, MediaRepo, MediaVisibility, PropertyRepo, RelationRepo,
    SiteRepo, TemplateRepo, ValueRepo, VocabularyRepo,
};
use omeka_db::rows::{
    NewAsset, NewItem, NewItemSet, NewMedia, NewProperty, NewResource, NewSite, NewTemplate,
    NewTemplateProperty, NewVocabulary, ValueContent,
};

on_both!(a_page_reads_each_relation_over_its_ids, |pool| {
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
    let properties = PropertyRepo::<Db>::new(pool);
    let title = properties
        .create(&NewProperty {
            owner_id: None,
            vocabulary_id: vocabulary.id,
            local_name: "title".into(),
            label: "Title".into(),
            comment: None,
        })
        .await
        .unwrap();
    let relation = properties
        .create(&NewProperty {
            owner_id: None,
            vocabulary_id: vocabulary.id,
            local_name: "relation".into(),
            label: "Relation".into(),
            comment: None,
        })
        .await
        .unwrap();
    let templates = TemplateRepo::<Db>::new(pool);
    let template = templates
        .create(&NewTemplate {
            owner_id: None,
            resource_class_id: None,
            title_property_id: None,
            description_property_id: None,
            label: "Letters".into(),
        })
        .await
        .unwrap();
    for (property, position) in [(relation.id, 1), (title.id, 2)] {
        templates
            .add_property(&NewTemplateProperty {
                resource_template_id: template.id,
                property_id: property,
                alternate_label: None,
                alternate_comment: None,
                position: Some(position),
                data_type: None,
                is_required: false,
                is_private: false,
                default_lang: None,
            })
            .await
            .unwrap();
    }
    let asset = AssetRepo::<Db>::new(pool)
        .create(&NewAsset {
            owner_id: None,
            name: "cover".into(),
            media_type: "image/png".into(),
            storage_id: "cover0001".into(),
            extension: Some("png".into()),
            alt_text: None,
        })
        .await
        .unwrap();

    let items = ItemRepo::<Db>::new(pool);
    let first = items
        .create(&NewItem {
            resource: NewResource {
                resource_template_id: Some(template.id),
                thumbnail_id: Some(asset.id),
                title: Some("First".into()),
                ..NewResource::default()
            },
            primary_media_id: None,
        })
        .await
        .unwrap();
    let second = items.create(&NewItem::default()).await.unwrap();
    let lonely = items.create(&NewItem::default()).await.unwrap();

    let media = MediaRepo::<Db>::new(pool);
    let later = media
        .create(&NewMedia {
            item_id: first.id(),
            ingester: "html".into(),
            renderer: "html".into(),
            position: Some(2),
            ..NewMedia::default()
        })
        .await
        .unwrap();
    let earlier = media
        .create(&NewMedia {
            item_id: first.id(),
            ingester: "html".into(),
            renderer: "html".into(),
            position: Some(1),
            ..NewMedia::default()
        })
        .await
        .unwrap();

    let sets = ItemSetRepo::<Db>::new(pool);
    let set = sets.create(&NewItemSet::default()).await.unwrap();
    let empty_set = sets.create(&NewItemSet::default()).await.unwrap();
    items.add_to_item_set(second.id(), set.id()).await.unwrap();
    items.add_to_item_set(first.id(), set.id()).await.unwrap();

    let site = SiteRepo::<Db>::new(pool)
        .create(&NewSite {
            slug: "s".into(),
            theme: "default".into(),
            title: "S".into(),
            ..NewSite::default()
        })
        .await
        .unwrap();
    items.add_to_site(first.id(), site.id).await.unwrap();

    let values = ValueRepo::<Db>::new(pool);
    let named = values
        .create(&ValueContent::literal(title.id, "First").on(first.resource.id))
        .await
        .unwrap();
    let link = values
        .create(
            &ValueContent {
                r#type: "resource:item".into(),
                value: None,
                value_resource_id: Some(first.resource.id),
                ..ValueContent::literal(relation.id, "")
            }
            .on(second.resource.id),
        )
        .await
        .unwrap();

    let reads = RelationRepo::<Db>::new(pool);
    let page = [first.id(), second.id(), lonely.id()];
    let resources: Vec<ResourceId> = page.iter().map(|id| ResourceId(id.0)).collect();

    let found: Vec<_> = reads
        .media_of_items(&page)
        .await
        .unwrap()
        .iter()
        .map(|row| row.id())
        .collect();
    assert_eq!(found, [earlier.id(), later.id()], "media order by position");

    assert_eq!(
        reads.item_sets_of_items(&page).await.unwrap(),
        [(first.id(), set.id()), (second.id(), set.id())]
    );
    assert_eq!(
        reads.sites_of_items(&page).await.unwrap(),
        [(first.id(), site.id)]
    );
    assert_eq!(
        reads
            .first_items(&[set.id(), empty_set.id()])
            .await
            .unwrap(),
        [(set.id(), first.id(), None)],
        "the smallest member id, and no row for an empty set"
    );

    let thumbnails = reads.thumbnail_assets(&resources).await.unwrap();
    assert_eq!(thumbnails.len(), 1);
    assert_eq!(thumbnails[0].id, asset.id);

    let on_page: Vec<_> = reads
        .values_of(&resources, MediaVisibility::All)
        .await
        .unwrap()
        .iter()
        .map(|row| row.id)
        .collect();
    assert_eq!(on_page, [named.id, link.id], "values by resource, then id");
    let pointing: Vec<_> = reads
        .values_pointing_at(&resources, MediaVisibility::All)
        .await
        .unwrap()
        .iter()
        .map(|row| row.id)
        .collect();
    assert_eq!(pointing, [link.id]);

    let mut titles: Vec<_> = reads
        .resources(&[first.resource.id, second.resource.id])
        .await
        .unwrap()
        .into_iter()
        .map(|row| (row.id, row.title))
        .collect();
    titles.sort();
    assert_eq!(
        titles,
        [
            (first.resource.id, Some("First".to_string())),
            (second.resource.id, None)
        ]
    );

    assert_eq!(
        reads.template_properties(&[template.id]).await.unwrap(),
        [(template.id, relation.id), (template.id, title.id)],
        "template properties in position order"
    );
});

on_both!(an_empty_id_list_reads_nothing, |pool| {
    let reads = RelationRepo::<Db>::new(pool);
    let no_items: [ItemId; 0] = [];
    let no_resources: [ResourceId; 0] = [];
    let no_sets: [ItemSetId; 0] = [];
    let no_templates: [TemplateId; 0] = [];
    assert!(reads.media_of_items(&no_items).await.unwrap().is_empty());
    assert!(
        reads
            .item_sets_of_items(&no_items)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(reads.sites_of_items(&no_items).await.unwrap().is_empty());
    assert!(reads.first_items(&no_sets).await.unwrap().is_empty());
    assert!(
        reads
            .thumbnail_assets(&no_resources)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        reads
            .values_of(&no_resources, MediaVisibility::All)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        reads
            .values_pointing_at(&no_resources, MediaVisibility::All)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(reads.resources(&no_resources).await.unwrap().is_empty());
    assert!(
        reads
            .template_properties(&no_templates)
            .await
            .unwrap()
            .is_empty()
    );
});
