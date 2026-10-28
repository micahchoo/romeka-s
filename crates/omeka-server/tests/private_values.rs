//! A private value on a public resource, read through HTTP by five callers: anonymous, a second
//! non-admin, the resource's owner (an author), a reviewer and an admin.
//!
//! Upstream `ValueVisibilityFilter` (`application/src/Db/Filter/ValueVisibilityFilter.php`) adds
//! one condition to every SQL statement that reads `value`: a caller with `view-all` sees every
//! value, a signed-in caller sees the values of the resources they own, everyone else sees public
//! values only. So the private value is in the documents, the CSV, the property search and the
//! counts for the owner, the reviewer and the admin, and absent from all of them for anonymous and
//! the second non-admin.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::Router;
use axum::http::StatusCode;
use common::{
    Answer, KEY_CREDENTIAL, KEY_IDENTITY, caller, json_request, request, router, seeded_pool, send,
};
use omeka_db::ids::{ItemId, ItemSetId, MediaId, PropertyId, ResourceId, SiteId, TemplateId};
use omeka_db::repos::{
    ItemRepo, ItemSetRepo, MediaRepo, PropertyRepo, SettingRepo, TemplateRepo, ValueRepo,
};
use omeka_db::rows::{
    NewItem, NewItemSet, NewMedia, NewProperty, NewResource, NewTemplate, NewValue,
};
use serde_json::json;
use sqlx::SqlitePool;

/// The text of every private value in the world. No public value contains it.
const SECRET: &str = "Zqxsecret";

struct Who {
    name: &'static str,
    key: String,
    /// Whether upstream's filter lets this caller see the owner's private values.
    sees_private: bool,
}

impl Who {
    fn uri(&self, path: &str) -> String {
        if self.key.is_empty() {
            return path.to_string();
        }
        let separator = if path.contains('?') { '&' } else { '?' };
        format!("{path}{separator}{}", self.key)
    }
}

struct World {
    pool: SqlitePool,
    app: Router,
    callers: Vec<Who>,
    owner_key: String,
    template: TemplateId,
    /// Carries a public and a private value on the owner's public item.
    note: PropertyId,
    /// Carries only a private value on the owner's public item.
    secret: PropertyId,
    /// The owner's public item points at `target` through a private value.
    creator: PropertyId,
    item: ItemId,
    item_set: ItemSetId,
    media: MediaId,
    target: ItemId,
}

async fn property(
    pool: &SqlitePool,
    vocabulary: omeka_db::ids::VocabularyId,
    name: &str,
) -> PropertyId {
    PropertyRepo::new(pool)
        .create(&NewProperty {
            owner_id: None,
            vocabulary_id: vocabulary,
            local_name: name.to_string(),
            label: name.to_string(),
            comment: None,
        })
        .await
        .unwrap()
        .id
}

async fn value(
    pool: &SqlitePool,
    resource: ResourceId,
    property: PropertyId,
    text: &str,
    to: Option<ItemId>,
    is_public: bool,
) {
    ValueRepo::new(pool)
        .create(&NewValue {
            resource_id: resource,
            property_id: property,
            value_resource_id: to.map(ItemId::as_resource),
            value_annotation_id: None,
            r#type: if to.is_some() {
                "resource:item"
            } else {
                "literal"
            }
            .to_string(),
            lang: None,
            value: to.is_none().then(|| text.to_string()),
            uri: None,
            is_public,
        })
        .await
        .unwrap();
}

fn public_resource(
    owner: omeka_db::ids::UserId,
    template: Option<TemplateId>,
    title: &str,
) -> NewResource {
    NewResource {
        owner_id: Some(owner),
        resource_template_id: template,
        title: Some(title.to_string()),
        is_public: true,
        ..NewResource::default()
    }
}

async fn world() -> World {
    let (pool, seed) = seeded_pool().await;
    let owner = caller(&pool, "author").await;
    let other = caller(&pool, "researcher").await;
    let reviewer = caller(&pool, "reviewer").await;
    let admin = seed.user;
    let note = property(&pool, seed.vocabulary, "note").await;
    let secret = property(&pool, seed.vocabulary, "secret").await;
    let creator = property(&pool, seed.vocabulary, "creator").await;
    let template = TemplateRepo::new(&pool)
        .create(&NewTemplate {
            owner_id: Some(admin),
            resource_class_id: None,
            title_property_id: None,
            description_property_id: None,
            label: "Letter".to_string(),
        })
        .await
        .unwrap()
        .id;

    let target = ItemRepo::new(&pool)
        .create(&NewItem {
            resource: public_resource(admin, None, "A person"),
            primary_media_id: None,
        })
        .await
        .unwrap()
        .id();
    let item = ItemRepo::new(&pool)
        .create(&NewItem {
            resource: public_resource(owner.id, Some(template), "Open letter"),
            primary_media_id: None,
        })
        .await
        .unwrap()
        .id();
    value(&pool, item.as_resource(), note, "Open note", None, true).await;
    value(&pool, item.as_resource(), note, SECRET, None, false).await;
    value(&pool, item.as_resource(), secret, SECRET, None, false).await;
    value(&pool, item.as_resource(), creator, "", Some(target), false).await;
    ItemRepo::new(&pool)
        .add_to_site(item, SiteId(seed.site))
        .await
        .unwrap();

    let item_set = ItemSetRepo::new(&pool)
        .create(&NewItemSet {
            resource: public_resource(owner.id, None, "Open set"),
            is_open: true,
        })
        .await
        .unwrap()
        .id();
    value(&pool, item_set.as_resource(), note, SECRET, None, false).await;

    let media = MediaRepo::new(&pool)
        .create(&NewMedia {
            resource: public_resource(owner.id, None, "Open photo"),
            item_id: item,
            ingester: "upload".to_string(),
            renderer: "file".to_string(),
            data: None,
            source: Some("photo.jpg".to_string()),
            media_type: Some("image/jpeg".to_string()),
            storage_id: Some("5c2f6c0f1a0a5a6b9a7c3d4e5f6a7b8c9d0e1f2a".to_string()),
            extension: Some("jpg".to_string()),
            sha256: Some("def456".to_string()),
            size: Some(1234),
            has_original: true,
            has_thumbnails: true,
            position: Some(1),
            lang: None,
            alt_text: None,
        })
        .await
        .unwrap()
        .id();
    value(&pool, media.as_resource(), note, SECRET, None, false).await;

    let admin_key = format!("key_identity={KEY_IDENTITY}&key_credential={KEY_CREDENTIAL}");
    World {
        app: router(pool.clone()),
        pool,
        owner_key: owner.key.clone(),
        callers: vec![
            Who {
                name: "anonymous",
                key: String::new(),
                sees_private: false,
            },
            Who {
                name: "other",
                key: other.key,
                sees_private: false,
            },
            Who {
                name: "owner",
                key: owner.key,
                sees_private: true,
            },
            Who {
                name: "reviewer",
                key: reviewer.key,
                sees_private: true,
            },
            Who {
                name: "admin",
                key: admin_key,
                sees_private: true,
            },
        ],
        template,
        note,
        secret,
        creator,
        item,
        item_set,
        media,
        target,
    }
}

async fn read(app: &Router, uri: &str) -> Answer {
    let answer = send(app.clone(), request("GET", uri)).await;
    assert_eq!(answer.status, StatusCode::OK, "{uri}: {}", answer.body);
    answer
}

async fn total(app: &Router, uri: &str) -> u64 {
    common::header(&read(app, uri).await, "omeka-s-total-results")
        .expect("a total header")
        .parse()
        .unwrap()
}

#[tokio::test]
async fn documents_and_the_csv_carry_a_private_value_only_for_callers_who_may_see_it() {
    let world = world().await;
    let paths = [
        format!("/api/items/{}", world.item.0),
        format!("/api/items?id[]={}", world.item.0),
        format!("/api/item_sets/{}", world.item_set.0),
        format!("/api/item_sets?id[]={}", world.item_set.0),
        format!("/api/media/{}", world.media.0),
        format!("/api/media?item_id={}", world.item.0),
        format!("/api/resources/{}", world.item.0),
        format!("/api/romeka/items.csv?id[]={}", world.item.0),
    ];
    for who in &world.callers {
        for path in &paths {
            let answer = read(&world.app, &who.uri(path)).await;
            assert_eq!(
                answer.body.contains(SECRET),
                who.sees_private,
                "{}: {path}: {}",
                who.name,
                answer.body
            );
        }
        let item = read(
            &world.app,
            &who.uri(&format!("/api/items/{}", world.item.0)),
        )
        .await
        .json();
        assert_eq!(
            item["dcterms:note"].as_array().unwrap().len(),
            if who.sees_private { 2 } else { 1 },
            "{}",
            who.name
        );
        assert_eq!(
            item.get("dcterms:secret").is_some(),
            who.sees_private,
            "{}: a property whose every value is hidden is left out",
            who.name
        );
        assert_eq!(
            item.get("dcterms:creator").is_some(),
            who.sees_private,
            "{}",
            who.name
        );
    }
}

/// `subjectValues` reads `value` through the same filter, so the target does not name the owner's
/// item as a linker for a caller who may not see the linking value.
#[tokio::test]
async fn a_private_link_is_not_a_reverse_reference_for_callers_who_may_not_see_it() {
    let world = world().await;
    for who in &world.callers {
        let target = read(
            &world.app,
            &who.uri(&format!("/api/items/{}", world.target.0)),
        )
        .await
        .json();
        let linker = format!("/api/resources/{}", world.item.0);
        let names_linker = target
            .get("@reverse")
            .map(|reverse| reverse.to_string().contains(&linker))
            .unwrap_or(false);
        assert_eq!(names_linker, who.sees_private, "{}: {target}", who.name);
    }
}

#[tokio::test]
async fn a_property_search_matches_a_private_value_only_for_callers_who_may_see_it() {
    let world = world().await;
    let one = |property: PropertyId, kind: &str, text: &str| {
        format!(
            "/api/items?property[0][property]={}&property[0][type]={kind}&property[0][text]={text}",
            property.0
        )
    };
    let only_item = |search: String| format!("{search}&id[]={}", world.item.0);
    let target = world.target.0.to_string();
    for who in &world.callers {
        let seen = u64::from(who.sees_private);
        let searches = [
            (one(world.note, "eq", SECRET), seen),
            (one(world.note, "in", "zqxsec"), seen),
            (one(world.secret, "ex", ""), seen),
            (only_item(one(world.secret, "nex", "")), 1 - seen),
            (one(world.creator, "res", &target), seen),
            (only_item(one(world.creator, "nres", &target)), 1 - seen),
            (format!("/api/items?search={SECRET}"), seen),
            (format!("/api/item_sets?search={SECRET}"), seen),
            (format!("/api/media?search={SECRET}"), seen),
        ];
        for (search, want) in searches {
            assert_eq!(
                total(&world.app, &who.uri(&search)).await,
                want,
                "{}: {search}",
                who.name
            );
        }
    }
}

#[tokio::test]
async fn field_and_link_counts_count_a_private_value_only_for_callers_who_may_see_it() {
    let world = world().await;
    for who in &world.callers {
        let counts = read(
            &world.app,
            &who.uri(&format!(
                "/api/romeka/field-counts?resource_template_id={}",
                world.template.0
            )),
        )
        .await
        .json();
        let filled = |property: PropertyId| {
            counts["fields"]
                .as_array()
                .unwrap()
                .iter()
                .find(|row| row["property_id"] == property.0)
                .map_or(0, |row| row["filled"].as_u64().unwrap())
        };
        assert_eq!(counts["total"], 1, "{}", who.name);
        assert_eq!(
            filled(world.note),
            1,
            "{}: a public note fills the field",
            who.name
        );
        assert_eq!(
            filled(world.secret),
            u64::from(who.sees_private),
            "{}",
            who.name
        );
        assert_eq!(
            filled(world.creator),
            u64::from(who.sees_private),
            "{}",
            who.name
        );

        let links = read(
            &world.app,
            &who.uri(&format!("/api/romeka/links/{}", world.target.0)),
        )
        .await
        .json();
        let rows = links["links"].as_array().unwrap();
        assert_eq!(
            rows.len(),
            usize::from(who.sees_private),
            "{}: {links}",
            who.name
        );
    }
}

/// Upstream indexes only public values (`AbstractResourceEntityAdapter::getFulltextText`), so no
/// caller finds a private value by its words, while a public word finds the item.
#[tokio::test]
async fn fulltext_search_never_finds_a_private_value() {
    let world = world().await;
    let created = send(
        world.app.clone(),
        json_request(
            "POST",
            &format!("/api/items?{}", world.owner_key),
            &json!({
                "o:is_public": true,
                "dcterms:title": [{"type": "literal", "property_id": "auto", "@value": "Indexed letter"}],
                "dcterms:note": [
                    {"type": "literal", "property_id": "auto", "@value": "Plainword"},
                    {"type": "literal", "property_id": "auto", "@value": SECRET, "is_public": false},
                ],
            })
            .to_string(),
        ),
    )
    .await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.body);
    for who in &world.callers {
        assert_eq!(
            total(&world.app, &who.uri("/api/items?fulltext_search=Plainword")).await,
            1,
            "{}",
            who.name
        );
        assert_eq!(
            total(
                &world.app,
                &who.uri(&format!("/api/items?fulltext_search={SECRET}"))
            )
            .await,
            0,
            "{}",
            who.name
        );
    }
}

/// The public site prints public values only, as `resource-values.phtml` does for a visitor.
#[tokio::test]
async fn the_public_site_item_page_hides_a_private_value_from_a_visitor() {
    let world = world().await;
    let page = read(&world.app, &format!("/s/museum/item/{}", world.item.0)).await;
    assert!(page.body.contains("Open note"), "{}", page.body);
    assert!(!page.body.contains(SECRET), "{}", page.body);
    let target = read(&world.app, &format!("/s/museum/item/{}", world.target.0)).await;
    assert!(!target.body.contains("Open letter"), "{}", target.body);
}

/// `MediaRepresentation::altTextResolved` reads the alt text property through `value()`, which
/// sees only the values the filter keeps. The public site answers any visitor.
#[tokio::test]
async fn an_alt_text_drawn_from_a_private_value_is_not_shown_to_a_visitor() {
    let world = world().await;
    SettingRepo::new(&world.pool)
        .set("media_alt_text_property", &json!("dcterms:note"))
        .await
        .unwrap();
    let page = read(&world.app, &format!("/s/museum/media/{}", world.media.0)).await;
    assert!(page.body.contains("Open photo"), "{}", page.body);
    assert!(!page.body.contains(SECRET), "{}", page.body);
}

/// Upstream stores the first title value as `o:title` whatever its visibility, so its own reads
/// print a private title. The port shows a caller the first title value they may see, in the
/// document, in a reverse reference, in a linked value's `display_title`, in the CSV and in the
/// fulltext index (which answers everyone, so it holds the public title).
#[tokio::test]
async fn a_title_drawn_from_a_private_value_shows_only_to_callers_who_may_see_it() {
    let world = world().await;
    let created = send(
        world.app.clone(),
        json_request(
            "POST",
            &format!("/api/items?{}", world.owner_key),
            &json!({
                "o:is_public": true,
                "dcterms:title": [
                    {"type": "literal", "property_id": "auto", "@value": "Hiddentitle", "is_public": false},
                    {"type": "literal", "property_id": "auto", "@value": "Shown title"},
                ],
                "dcterms:creator": [
                    {"type": "resource:item", "property_id": "auto", "value_resource_id": world.target.0},
                ],
            })
            .to_string(),
        ),
    )
    .await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.body);
    let titled = created.json()["o:id"].as_u64().unwrap();
    let linker = send(
        world.app.clone(),
        json_request(
            "POST",
            &format!("/api/items?{}", world.owner_key),
            &json!({
                "o:is_public": true,
                "dcterms:title": [{"type": "literal", "property_id": "auto", "@value": "Linker"}],
                "dcterms:note": [
                    {"type": "resource:item", "property_id": "auto", "value_resource_id": titled},
                ],
            })
            .to_string(),
        ),
    )
    .await;
    assert_eq!(linker.status, StatusCode::OK, "{}", linker.body);
    let linker = linker.json()["o:id"].as_u64().unwrap();

    for who in &world.callers {
        let want = if who.sees_private {
            "Hiddentitle"
        } else {
            "Shown title"
        };
        let item = read(&world.app, &who.uri(&format!("/api/items/{titled}"))).await;
        assert_eq!(item.json()["o:title"], want, "{}", who.name);
        let list = read(&world.app, &who.uri(&format!("/api/items?id[]={titled}"))).await;
        assert_eq!(list.json()[0]["o:title"], want, "{}", who.name);
        let target = read(
            &world.app,
            &who.uri(&format!("/api/items/{}", world.target.0)),
        )
        .await;
        assert!(
            target.json()["@reverse"].to_string().contains(want),
            "{}: {}",
            who.name,
            target.body
        );
        let linked = read(&world.app, &who.uri(&format!("/api/items/{linker}"))).await;
        assert_eq!(
            linked.json()["dcterms:note"][0]["display_title"],
            want,
            "{}",
            who.name
        );
        let csv = read(
            &world.app,
            &who.uri(&format!(
                "/api/romeka/items.csv?id[]={titled}&id[]={linker}"
            )),
        )
        .await;
        assert_eq!(
            csv.body.contains("Hiddentitle"),
            who.sees_private,
            "{}: {}",
            who.name,
            csv.body
        );
        assert_eq!(
            total(
                &world.app,
                &who.uri("/api/items?fulltext_search=Hiddentitle")
            )
            .await,
            0,
            "{}",
            who.name
        );
        let seen = u64::from(who.sees_private);
        for search in [
            format!(
                "/api/items?property[0][property]={}&property[0][type]=eq&property[0][text]=Hiddentitle",
                world.note.0
            ),
            format!(
                "/api/items?property[0][property]={}&property[0][type]=in&property[0][text]=iddentit",
                world.note.0
            ),
            "/api/resources?search=Hiddentitle".to_string(),
        ] {
            assert_eq!(
                total(&world.app, &who.uri(&search)).await,
                seen,
                "{}: {search}",
                who.name
            );
        }
    }
}

/// A title sort orders each caller's list by the title that caller is shown, so a private title
/// never decides where a row sits for a caller who may not read it.
#[tokio::test]
async fn a_title_sort_orders_by_the_title_each_caller_is_shown() {
    let world = world().await;
    let item = |titles: serde_json::Value| {
        let app = world.app.clone();
        let uri = format!("/api/items?{}", world.owner_key);
        async move {
            let created = send(
                app,
                json_request(
                    "POST",
                    &uri,
                    &json!({"o:is_public": true, "dcterms:title": titles}).to_string(),
                ),
            )
            .await;
            assert_eq!(created.status, StatusCode::OK, "{}", created.body);
            created.json()["o:id"].as_u64().unwrap()
        }
    };
    let hidden = item(json!([
        {"type": "literal", "property_id": "auto", "@value": "Aaa private", "is_public": false},
        {"type": "literal", "property_id": "auto", "@value": "Zzz shown"},
    ]))
    .await;
    let plain = item(json!([{"type": "literal", "property_id": "auto", "@value": "Mmm"}])).await;

    for who in &world.callers {
        let want = if who.sees_private {
            [hidden, plain]
        } else {
            [plain, hidden]
        };
        let list = read(
            &world.app,
            &who.uri(&format!("/api/items?id[]={hidden}&id[]={plain}&sort_by=title")),
        )
        .await;
        let got: Vec<u64> = list
            .json()
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row["o:id"].as_u64().unwrap())
            .collect();
        assert_eq!(got, want, "{}", who.name);
    }
}

/// A media's annotation container labels the media by the title the caller is shown, so a title
/// drawn from a private value stays with those who may see it.
#[tokio::test]
async fn a_media_annotation_container_never_carries_a_hidden_title() {
    let world = world().await;
    let title: i64 = sqlx::query_scalar(
        "SELECT p.id FROM property p JOIN vocabulary v ON v.id = p.vocabulary_id \
         WHERE v.prefix = 'dcterms' AND p.local_name = 'title'",
    )
    .fetch_one(&world.pool)
    .await
    .unwrap();
    let title = PropertyId(u64::try_from(title).unwrap());
    value(&world.pool, world.media.as_resource(), title, "Mediasecret", None, false).await;
    value(&world.pool, world.media.as_resource(), title, "Media shown", None, true).await;
    sqlx::query("UPDATE resource SET title = 'Mediasecret' WHERE id = ?")
        .bind(i64::try_from(world.media.0).unwrap())
        .execute(&world.pool)
        .await
        .unwrap();

    for who in &world.callers {
        let path = format!("/annotations/media/{}/", world.media.0);
        let answer = read(&world.app, &who.uri(&path)).await;
        assert_eq!(
            answer.body.contains("Mediasecret"),
            who.sees_private,
            "{}: {}",
            who.name,
            answer.body
        );
    }
}
