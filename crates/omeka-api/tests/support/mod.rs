//! Seeding for the list-batching tests.
//!
//! One item set and `count` items, with every relation kind on every item: a
//! template, four values, one resource value, one annotation, and an upload
//! media on the first two. The perf dataset is narrower (no media, no
//! annotations, no template), so this seed exercises more relations than the
//! harness does.

#![allow(clippy::unwrap_used, clippy::expect_used, dead_code)]

use omeka_api::auth::Actor;
use omeka_api::jsonld::BaseUrl;
use omeka_db::ids::{
    AnnotationId, ItemId, ItemSetId, MediaId, PropertyId, ResourceId, TemplateId, UserId,
};
use omeka_db::repos::{
    ItemRepo, ItemSetRepo, MediaRepo, PropertyRepo, ResourceRepo, TemplateRepo, UserRepo,
    ValueRepo, VocabularyRepo,
};
use omeka_db::rows::{
    NewItem, NewItemSet, NewMedia, NewProperty, NewResource, NewTemplate, NewTemplateProperty,
    NewUser, NewValue, NewVocabulary, ValueAnnotationRow,
};
use omeka_db::{migrate, open_memory};
use sqlx::SqlitePool;

/// Rows the seed creates, for the assertions.
pub struct Seed {
    pub user: UserId,
    pub item_set: ItemSetId,
    pub items: Vec<ItemId>,
    pub media: Vec<MediaId>,
    pub annotation: ResourceId,
}

pub fn base() -> BaseUrl {
    BaseUrl::parse("http://localhost").unwrap()
}

pub fn admin(user: UserId) -> Actor {
    Actor::new(user, "global_admin")
}

/// One value on a resource.
#[allow(clippy::too_many_arguments)]
async fn add_value(
    pool: &SqlitePool,
    resource: ResourceId,
    property: PropertyId,
    kind: &str,
    text: Option<&str>,
    uri: Option<&str>,
    target: Option<ResourceId>,
    annotation: Option<AnnotationId>,
) {
    ValueRepo::new(pool)
        .create(&NewValue {
            resource_id: resource,
            property_id: property,
            value_resource_id: target,
            value_annotation_id: annotation,
            r#type: kind.to_string(),
            lang: None,
            value: text.map(str::to_string),
            uri: uri.map(str::to_string),
            is_public: true,
        })
        .await
        .unwrap();
}

/// The five `dcterms` properties the seed writes.
#[derive(Clone, Copy)]
struct Props {
    title: PropertyId,
    creator: PropertyId,
    description: PropertyId,
    subject: PropertyId,
    source: PropertyId,
}

/// One seeded item and the rows that hang off it.
struct SeededItem {
    id: ItemId,
    media: Option<MediaId>,
    annotation: ResourceId,
}

/// One item set, `count` items attached to it, and every relation kind on every
/// item, so every page carries the same relation kinds.
pub async fn seed(pool: &SqlitePool, count: usize) -> Seed {
    let user = seed_user(pool).await;
    let props = seed_vocabulary(pool).await;
    let template = seed_template(pool, user, props).await;
    let item_set = seed_item_set(pool, user).await;
    let (items, media, annotation) = seed_items(pool, user, item_set, template, props, count).await;
    Seed {
        user,
        item_set,
        items,
        media,
        annotation,
    }
}

async fn seed_user(pool: &SqlitePool) -> UserId {
    UserRepo::new(pool)
        .create(&NewUser {
            email: "admin@example.com".to_string(),
            name: "Admin".to_string(),
            password_hash: None,
            role: "global_admin".to_string(),
            is_active: true,
            created: None,
        })
        .await
        .unwrap()
        .id
}

/// A `dcterms` vocabulary with the five properties the seed writes.
async fn seed_vocabulary(pool: &SqlitePool) -> Props {
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
    let mut properties = Vec::new();
    for (local, label) in [
        ("title", "Title"),
        ("creator", "Creator"),
        ("description", "Description"),
        ("subject", "Subject"),
        ("source", "Source"),
    ] {
        properties.push(
            PropertyRepo::new(pool)
                .create(&NewProperty {
                    owner_id: None,
                    vocabulary_id: vocabulary.id,
                    local_name: local.to_string(),
                    label: label.to_string(),
                    comment: None,
                })
                .await
                .unwrap()
                .id,
        );
    }
    Props {
        title: properties[0],
        creator: properties[1],
        description: properties[2],
        subject: properties[3],
        source: properties[4],
    }
}

/// A template that orders two of the properties, so the page's documents read
/// the template relation.
async fn seed_template(pool: &SqlitePool, user: UserId, props: Props) -> TemplateId {
    let template = TemplateRepo::new(pool)
        .create(&NewTemplate {
            owner_id: Some(user),
            resource_class_id: None,
            title_property_id: Some(props.title),
            description_property_id: Some(props.description),
            label: "Perf template".to_string(),
        })
        .await
        .unwrap();
    for property in [props.creator, props.description] {
        TemplateRepo::new(pool)
            .add_property(&NewTemplateProperty {
                resource_template_id: template.id,
                property_id: property,
                alternate_label: None,
                alternate_comment: None,
                position: None,
                data_type: None,
                is_required: false,
                is_private: false,
                default_lang: None,
            })
            .await
            .unwrap();
    }
    template.id
}

async fn seed_item_set(pool: &SqlitePool, user: UserId) -> ItemSetId {
    ItemSetRepo::new(pool)
        .create(&NewItemSet {
            resource: NewResource {
                owner_id: Some(user),
                title: Some("Perf set".to_string()),
                ..NewResource::default()
            },
            is_open: false,
        })
        .await
        .unwrap()
        .id()
}

/// `count` items, each with a template, four values, one resource value and one
/// annotation, and an upload media on the first two.
async fn seed_items(
    pool: &SqlitePool,
    user: UserId,
    item_set: ItemSetId,
    template: TemplateId,
    props: Props,
    count: usize,
) -> (Vec<ItemId>, Vec<MediaId>, ResourceId) {
    let mut seeded = Vec::with_capacity(count);
    for index in 1..=count {
        seeded.push(seed_one_item(pool, user, item_set, template, props, index).await);
    }
    // The resource values come second, so every target item exists: the foreign
    // key rejects a forward reference.
    for (index, item) in seeded.iter().enumerate() {
        let target = seeded[(index + 1) % seeded.len()].id;
        add_value(
            pool,
            item.id.as_resource(),
            props.subject,
            "resource",
            None,
            None,
            Some(target.as_resource()),
            None,
        )
        .await;
    }
    (
        seeded.iter().map(|item| item.id).collect(),
        seeded.iter().filter_map(|item| item.media).collect(),
        seeded
            .first()
            .map(|item| item.annotation)
            .unwrap_or(ResourceId(0)),
    )
}

async fn seed_one_item(
    pool: &SqlitePool,
    user: UserId,
    item_set: ItemSetId,
    template: TemplateId,
    props: Props,
    index: usize,
) -> SeededItem {
    let id = ItemRepo::new(pool)
        .create(&NewItem {
            resource: NewResource {
                owner_id: Some(user),
                resource_template_id: Some(template),
                title: Some(format!("Perf item {index:04}")),
                ..NewResource::default()
            },
            primary_media_id: None,
        })
        .await
        .unwrap()
        .id();
    ItemRepo::new(pool)
        .add_to_item_set(id, item_set)
        .await
        .unwrap();
    add_value(
        pool,
        id.as_resource(),
        props.title,
        "literal",
        Some("title"),
        None,
        None,
        None,
    )
    .await;
    add_value(
        pool,
        id.as_resource(),
        props.description,
        "literal",
        Some("description"),
        None,
        None,
        None,
    )
    .await;
    add_value(
        pool,
        id.as_resource(),
        props.source,
        "uri",
        None,
        Some(&format!("https://example.org/sources/{index}")),
        None,
        None,
    )
    .await;
    let note = ResourceRepo::new(pool)
        .create_annotation(&NewResource {
            owner_id: Some(user),
            title: Some(format!("Note {index}")),
            ..NewResource::default()
        })
        .await
        .unwrap();
    let ValueAnnotationRow { resource } = note;
    add_value(
        pool,
        resource.id,
        props.description,
        "literal",
        Some("annotated"),
        None,
        None,
        None,
    )
    .await;
    add_value(
        pool,
        id.as_resource(),
        props.creator,
        "literal",
        Some("annotated value"),
        None,
        None,
        Some(AnnotationId(resource.id.0)),
    )
    .await;
    let media = if index <= 2 {
        Some(
            MediaRepo::new(pool)
                .create(&NewMedia {
                    resource: NewResource {
                        owner_id: Some(user),
                        title: Some(format!("File {index}")),
                        ..NewResource::default()
                    },
                    item_id: id,
                    ingester: "upload".to_string(),
                    renderer: "file".to_string(),
                    storage_id: Some(format!("storage{index}")),
                    extension: Some("png".to_string()),
                    media_type: Some("image/png".to_string()),
                    ..NewMedia::default()
                })
                .await
                .unwrap()
                .id(),
        )
    } else {
        None
    };
    SeededItem {
        id,
        media,
        annotation: resource.id,
    }
}

/// The perf harness's item shape: one item set and `count` items with three
/// literal values and one URI value, and no media, annotation, template or
/// resource value (`verification/perf/bench.py` `item_body`).
pub async fn seed_plain(pool: &SqlitePool, count: usize) -> Seed {
    let user = seed_user(pool).await;
    let props = seed_vocabulary(pool).await;
    let item_set = seed_item_set(pool, user).await;
    let mut items = Vec::new();
    for index in 1..=count {
        let id = ItemRepo::new(pool)
            .create(&NewItem {
                resource: NewResource {
                    owner_id: Some(user),
                    title: Some(format!("Perf item {index:04}")),
                    ..NewResource::default()
                },
                primary_media_id: None,
            })
            .await
            .unwrap()
            .id();
        ItemRepo::new(pool)
            .add_to_item_set(id, item_set)
            .await
            .unwrap();
        for (property, text) in [
            (props.title, "title"),
            (props.creator, "creator"),
            (props.description, "description"),
        ] {
            add_value(
                pool,
                id.as_resource(),
                property,
                "literal",
                Some(text),
                None,
                None,
                None,
            )
            .await;
        }
        add_value(
            pool,
            id.as_resource(),
            props.source,
            "uri",
            None,
            Some(&format!("https://example.org/sources/{index}")),
            None,
            None,
        )
        .await;
        items.push(id);
    }
    Seed {
        user,
        item_set,
        items,
        media: Vec::new(),
        annotation: ResourceId(0),
    }
}

/// A migrated in-memory database seeded with `count` items.
pub async fn open_seeded(count: usize) -> (SqlitePool, Seed) {
    let pool = open_memory().await.unwrap();
    migrate(&pool).await.unwrap();
    let seed = seed(&pool, count).await;
    (pool, seed)
}
