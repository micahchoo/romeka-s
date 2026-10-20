//! The install an import writes into, for the import-writes suites: properties, a template whose
//! title is `dcterms:alternative`, a class, two users, two sites, two item sets and an item to link
//! to. Built through the public repositories on either engine.
#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use omeka_db::ids::{ClassId, ItemId, ItemSetId, PropertyId, SiteId, TemplateId, UserId};
use omeka_db::repos::{
    ClassRepo, ItemRepo, ItemSetRepo, PropertyRepo, SiteRepo, TemplateRepo, UserRepo,
    VocabularyRepo,
};
use omeka_db::rows::{
    NewClass, NewImportFields, NewItem, NewItemSet, NewProperty, NewResource, NewSite, NewTemplate,
    NewUser, NewVocabulary, ValueContent,
};

pub struct World {
    pub title: PropertyId,
    pub alternative: PropertyId,
    pub description: PropertyId,
    pub subject: PropertyId,
    pub relation: PropertyId,
    pub template: TemplateId,
    pub class: ClassId,
    pub users: [UserId; 2],
    pub sites: [SiteId; 2],
    pub sets: [ItemSetId; 2],
    pub other: ItemId,
}

macro_rules! world {
    ($pool:expr) => {
        import_writes_db::World::build::<Db>($pool).await
    };
}

pub(crate) use world;

impl World {
    pub async fn build<DB>(pool: &sqlx::Pool<DB>) -> World
    where
        DB: omeka_db::Backend,
        for<'c> &'c mut DB::Connection: sqlx::Executor<'c, Database = DB>,
        for<'q> DB::Arguments<'q>: sqlx::IntoArguments<'q, DB>,
        usize: sqlx::ColumnIndex<DB::Row>,
        for<'x> i64: sqlx::Decode<'x, DB> + sqlx::Encode<'x, DB> + sqlx::Type<DB>,
        for<'x> Option<i64>: sqlx::Decode<'x, DB> + sqlx::Encode<'x, DB> + sqlx::Type<DB>,
        for<'x> bool: sqlx::Decode<'x, DB> + sqlx::Encode<'x, DB> + sqlx::Type<DB>,
        for<'x> String: sqlx::Decode<'x, DB> + sqlx::Encode<'x, DB> + sqlx::Type<DB>,
        for<'x> Option<String>: sqlx::Decode<'x, DB> + sqlx::Encode<'x, DB> + sqlx::Type<DB>,
        for<'x> Vec<u8>: sqlx::Decode<'x, DB> + sqlx::Type<DB>,
        for<'x> Option<Vec<u8>>: sqlx::Decode<'x, DB> + sqlx::Encode<'x, DB> + sqlx::Type<DB>,
        for<'x> &'x str: sqlx::Encode<'x, DB> + sqlx::Type<DB> + sqlx::ColumnIndex<DB::Row>,
        for<'x> f64: sqlx::Decode<'x, DB> + sqlx::Type<DB>,
    {
        let vocabulary = VocabularyRepo::<DB>::new(pool)
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
        for name in ["title", "alternative", "description", "subject", "relation"] {
            let row = PropertyRepo::<DB>::new(pool)
                .create(&NewProperty {
                    owner_id: None,
                    vocabulary_id: vocabulary.id,
                    local_name: name.to_string(),
                    label: name.to_string(),
                    comment: None,
                })
                .await
                .unwrap();
            properties.push(row.id);
        }
        let class = ClassRepo::<DB>::new(pool)
            .create(&NewClass {
                owner_id: None,
                vocabulary_id: vocabulary.id,
                local_name: "Text".to_string(),
                label: "Text".to_string(),
                comment: None,
            })
            .await
            .unwrap()
            .id;
        let template = TemplateRepo::<DB>::new(pool)
            .create(&NewTemplate {
                owner_id: None,
                resource_class_id: None,
                title_property_id: Some(properties[1]),
                description_property_id: None,
                label: "Letter".to_string(),
            })
            .await
            .unwrap()
            .id;
        let mut users = Vec::new();
        for email in ["a@example.org", "b@example.org"] {
            let row = UserRepo::<DB>::new(pool)
                .create(&NewUser {
                    email: email.to_string(),
                    name: email.to_string(),
                    password_hash: None,
                    role: "editor".to_string(),
                    is_active: true,
                    created: None,
                })
                .await
                .unwrap();
            users.push(row.id);
        }
        let mut sites = Vec::new();
        for slug in ["one", "two"] {
            let row = SiteRepo::<DB>::new(pool)
                .create(&NewSite {
                    slug: slug.to_string(),
                    title: slug.to_string(),
                    theme: "default".to_string(),
                    ..NewSite::default()
                })
                .await
                .unwrap();
            sites.push(row.id);
        }
        let mut sets = Vec::new();
        for _ in 0..2 {
            let row = ItemSetRepo::<DB>::new(pool)
                .create(&NewItemSet::default())
                .await
                .unwrap();
            sets.push(row.id());
        }
        let other = ItemRepo::<DB>::new(pool)
            .create(&NewItem {
                resource: NewResource {
                    title: Some("Linked letter".to_string()),
                    ..NewResource::default()
                },
                primary_media_id: None,
            })
            .await
            .unwrap()
            .id();
        World {
            title: properties[0],
            alternative: properties[1],
            description: properties[2],
            subject: properties[3],
            relation: properties[4],
            template,
            class,
            users: [users[0], users[1]],
            sites: [sites[0], sites[1]],
            sets: [sets[0], sets[1]],
            other,
        }
    }

    pub fn fields(&self, values: Vec<ValueContent>) -> NewImportFields {
        NewImportFields {
            values,
            template: None,
            class: None,
            owner: Some(self.users[0]),
            is_public: false,
        }
    }
}

pub fn literal(property: PropertyId, text: &str, lang: Option<&str>) -> ValueContent {
    ValueContent {
        lang: lang.map(str::to_string),
        ..ValueContent::literal(property, text)
    }
}

pub fn uri(property: PropertyId, uri: &str, label: &str) -> ValueContent {
    ValueContent {
        r#type: "uri".to_string(),
        uri: Some(uri.to_string()),
        value: Some(label.to_string()),
        ..ValueContent::literal(property, "")
    }
}

pub fn link(property: PropertyId, item: ItemId) -> ValueContent {
    ValueContent {
        r#type: "resource:item".to_string(),
        value: None,
        value_resource_id: Some(item.as_resource()),
        ..ValueContent::literal(property, "")
    }
}
