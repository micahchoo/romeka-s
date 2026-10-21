//! The MySQL backend: the behaviour the SQLite suites freeze, against a real server.
//!
//! Gated on the `mysql` feature and on a reachable server. `ROMEKA_MYSQL_URL` names a server, not a
//! database (`mysql://root:secret@127.0.0.1:3306`); each test creates and drops its own database.
//! With no server the helper prints why and every test returns, so the suite skips loudly instead
//! of failing. Run it with
//!
//! ```text
//! ROMEKA_MYSQL_URL=mysql://root:secret@127.0.0.1:3306 \
//!   cargo test -p omeka-db --features mysql --test mysql
//! ```

#![cfg(feature = "mysql")]
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use omeka_db::ids::{ItemId, PageId, PropertyId, ResourceId, UserId};
use omeka_db::repos::{
    ClassFilter, ClassRepo, CustomVocabRepo, FulltextRepo, ItemFilter, ItemRepo, ItemSetRepo,
    JobRepo, MatchKind, MediaDerivativeRepo, MediaRepo, ModuleRepo, NewPublicationVersion, Paging,
    PasswordCreationRepo, PropertyFilter, PropertyRepo, PublicationRepo, QueryFilter, SessionRepo,
    SettingRepo, SiteFilter, SiteRepo, SiteSettingRepo, UserRepo, UserSettingRepo, ValueQuery,
    ValueRepo, VocabularyRepo,
};
use omeka_db::rows::{
    DerivativeKind, DerivativeState, FulltextSearchRow, NewClass, NewCustomVocab, NewDerivative,
    NewFirstPage, NewFirstPageBlock, NewItem, NewItemSet, NewJob, NewMedia, NewPasswordCreation,
    NewProperty, NewResource, NewSite, NewSiteBlock, NewSitePage, NewUser, NewValue, NewVocabulary,
    NewWholeItem, NewWholeSite, PublicationTarget, ValueContent,
};
use serde_json::json;
use sqlx::MySql;

/// A vocabulary with one property, ready for a value.
async fn title_property(pool: &sqlx::MySqlPool) -> PropertyId {
    let vocabulary = VocabularyRepo::<MySql>::new(pool)
        .create(&NewVocabulary {
            owner_id: None,
            namespace_uri: "http://purl.org/dc/terms/".to_string(),
            prefix: "dcterms".to_string(),
            label: "Dublin Core".to_string(),
            comment: None,
        })
        .await
        .expect("create vocabulary");
    PropertyRepo::<MySql>::new(pool)
        .create(&NewProperty {
            owner_id: None,
            vocabulary_id: vocabulary.id,
            local_name: "title".to_string(),
            label: "Title".to_string(),
            comment: None,
        })
        .await
        .expect("create property")
        .id
}

/// An item carrying one literal value for `property`.
async fn item_titled(pool: &sqlx::MySqlPool, title: &str, property: PropertyId) -> ItemId {
    let item = ItemRepo::<MySql>::new(pool)
        .create(&NewItem::default())
        .await
        .expect("create item");
    ValueRepo::<MySql>::new(pool)
        .create(&NewValue {
            resource_id: item.resource.id,
            property_id: property,
            value_resource_id: None,
            value_annotation_id: None,
            r#type: "literal".to_string(),
            lang: None,
            value: Some(title.to_string()),
            uri: None,
            is_public: true,
        })
        .await
        .expect("create value");
    item.id()
}

/// A filter that asks for one property whose value contains `text`.
fn contains(property: PropertyId, text: &str) -> QueryFilter {
    QueryFilter {
        value: vec![ValueQuery {
            property: Some(property),
            kind: MatchKind::In,
            negated: false,
            text: text.to_string(),
            joiner: omeka_db::repos::Joiner::And,
        }],
        ..QueryFilter::default()
    }
}

#[test]
fn the_test_url_keeps_the_authority_and_replaces_the_database() {
    assert_eq!(
        common::with_database("mysql://root:secret@127.0.0.1:3306", "romeka_test_1"),
        "mysql://root:secret@127.0.0.1:3306/romeka_test_1"
    );
    assert_eq!(
        common::with_database("mysql://root@db:3306/omeka", "romeka_test_2"),
        "mysql://root@db:3306/romeka_test_2"
    );
    assert_eq!(
        common::with_database("mysql://root@db:3306?ssl-mode=disabled", "romeka_test_3"),
        "mysql://root@db:3306/romeka_test_3?ssl-mode=disabled"
    );
}

#[tokio::test]
async fn migrating_twice_leaves_the_schema_and_the_rows_unchanged() {
    let Some(db) = common::mysql().await else {
        return;
    };
    let user = UserRepo::<MySql>::new(&db.pool)
        .create(&NewUser {
            email: "ada@example.org".to_string(),
            name: "Ada".to_string(),
            password_hash: None,
            role: "global_admin".to_string(),
            is_active: true,
            created: None,
        })
        .await
        .expect("create user");

    omeka_db::migrate(&db.pool)
        .await
        .expect("apply migrations a second time");

    assert_eq!(
        UserRepo::<MySql>::new(&db.pool).get(user.id).await.unwrap(),
        Some(user),
        "the second migrate kept the row"
    );
    let applied: Vec<i64> =
        sqlx::query_scalar("SELECT version FROM _sqlx_migrations ORDER BY version")
            .fetch_all(&db.pool)
            .await
            .expect("list migrations");
    assert_eq!(
        applied,
        [1, 2, 3, 5, 6, 7, 8, 9, 10],
        "each MySQL migration file applied once; 0004 is SQLite only"
    );

    db.cleanup().await;
}

#[tokio::test]
async fn a_derivative_row_is_queued_claimed_settled_and_removed_with_its_media() {
    let Some(db) = common::mysql().await else {
        return;
    };
    let item = ItemRepo::<MySql>::new(&db.pool)
        .create(&NewItem::default())
        .await
        .expect("create item");
    let media = MediaRepo::<MySql>::new(&db.pool)
        .create(&NewMedia {
            resource: NewResource::default(),
            item_id: item.id(),
            ingester: "upload".to_string(),
            renderer: "file".to_string(),
            storage_id: Some("mysql-queue".to_string()),
            extension: Some("mp4".to_string()),
            media_type: Some("video/mp4".to_string()),
            source: Some("reel.mp4".to_string()),
            has_original: true,
            ..NewMedia::default()
        })
        .await
        .expect("create media");
    let repo = MediaDerivativeRepo::<MySql>::new(&db.pool);
    let poster = NewDerivative {
        media_id: media.id(),
        kind: DerivativeKind::Poster,
    };

    assert!(repo.enqueue(&poster).await.expect("queue"));
    assert!(
        !repo.enqueue(&poster).await.expect("second queue"),
        "one row per pair"
    );
    assert_eq!(repo.pending().await.expect("count"), 1);

    let claimed = repo.claim_next().await.expect("claim").expect("a row");
    assert_eq!(claimed.state, DerivativeState::Running);
    assert_eq!(claimed.attempts, 1);
    assert!(
        repo.finish(
            media.id(),
            DerivativeKind::Poster,
            DerivativeState::Ready,
            None,
            Some("ffmpeg".to_string()),
        )
        .await
        .expect("settle")
    );
    let settled = repo
        .get(media.id(), DerivativeKind::Poster)
        .await
        .expect("read")
        .expect("the row");
    assert_eq!(settled.state, DerivativeState::Ready);
    assert_eq!(settled.progress, Some(100));

    MediaRepo::<MySql>::new(&db.pool)
        .delete(media.id())
        .await
        .expect("delete media");
    assert!(
        repo.for_media(media.id()).await.expect("rows").is_empty(),
        "the row went with the media"
    );
    db.cleanup().await;
}

#[tokio::test]
async fn a_user_email_matches_ignoring_case_as_the_unique_key_does() {
    let Some(db) = common::mysql().await else {
        return;
    };
    let created = UserRepo::<MySql>::new(&db.pool)
        .create(&NewUser {
            email: "Ada@Example.org".to_string(),
            name: "Ada Lovelace".to_string(),
            password_hash: Some("$2y$hash".to_string()),
            role: "global_admin".to_string(),
            is_active: true,
            created: None,
        })
        .await
        .expect("create user");

    assert_eq!(
        UserRepo::<MySql>::new(&db.pool)
            .get_by_email("ada@example.org")
            .await
            .expect("read by lower-case email"),
        Some(created)
    );

    db.cleanup().await;
}

#[tokio::test]
async fn a_created_datetime_round_trips_through_mysql_datetime() {
    let Some(db) = common::mysql().await else {
        return;
    };
    let user = UserRepo::<MySql>::new(&db.pool)
        .create(&NewUser {
            email: "grace@example.org".to_string(),
            name: "Grace".to_string(),
            password_hash: None,
            role: "editor".to_string(),
            is_active: true,
            created: None,
        })
        .await
        .expect("create user");

    let stored = UserRepo::<MySql>::new(&db.pool)
        .get(user.id)
        .await
        .expect("read user")
        .expect("user is there");
    assert_eq!(
        stored.created, user.created,
        "the returned instant is the stored one"
    );
    assert_eq!(stored.modified, user.modified);

    db.cleanup().await;
}

/// `property.local_name` is `utf8mb4_bin` upstream, so FOAF's `givenName` and `givenname` are two
/// properties, and a lookup matches case exactly. The SQLite schema agrees from migration 0004.
#[tokio::test]
async fn local_names_differing_only_in_case_are_two_properties() {
    let Some(db) = common::mysql().await else {
        return;
    };
    let foaf = VocabularyRepo::<MySql>::new(&db.pool)
        .create(&NewVocabulary {
            owner_id: None,
            namespace_uri: "http://xmlns.com/foaf/0.1/".to_string(),
            prefix: "foaf".to_string(),
            label: "Friend of a Friend".to_string(),
            comment: None,
        })
        .await
        .expect("create vocabulary");
    let properties = PropertyRepo::<MySql>::new(&db.pool);
    for local_name in ["givenName", "givenname"] {
        properties
            .create(&NewProperty {
                owner_id: None,
                vocabulary_id: foaf.id,
                local_name: local_name.to_string(),
                label: "Given name".to_string(),
                comment: None,
            })
            .await
            .unwrap_or_else(|error| panic!("foaf:{local_name} is stored: {error}"));
    }
    let lower = properties
        .find(foaf.id, "givenname")
        .await
        .expect("find")
        .expect("foaf:givenname");
    assert_eq!(lower.local_name, "givenname");
    assert_eq!(
        properties.find(foaf.id, "GIVENNAME").await.expect("find"),
        None
    );

    db.cleanup().await;
}

/// The three `utf8mb4_bin` columns (`property.local_name`, `resource_class.local_name` and
/// `password_creation.id`) reach sqlx as `VARBINARY`. Each read goes through `Backend::text`, which
/// must decode the column's bytes as the UTF-8 text they hold.
#[tokio::test]
async fn binary_collated_text_columns_decode_to_their_text() {
    let Some(db) = common::mysql().await else {
        return;
    };
    let vocabulary = VocabularyRepo::<MySql>::new(&db.pool)
        .create(&NewVocabulary {
            owner_id: None,
            namespace_uri: "http://purl.org/dc/terms/".to_string(),
            prefix: "dcterms".to_string(),
            label: "Dublin Core".to_string(),
            comment: None,
        })
        .await
        .expect("create vocabulary");

    ClassRepo::<MySql>::new(&db.pool)
        .create(&NewClass {
            owner_id: None,
            vocabulary_id: vocabulary.id,
            local_name: "Text".to_string(),
            label: "Text".to_string(),
            comment: None,
        })
        .await
        .expect("create class");
    let classes = ClassRepo::<MySql>::new(&db.pool)
        .list(&ClassFilter::default(), &Paging::default())
        .await
        .expect("list classes");
    assert_eq!(classes[0].local_name, "Text");

    PropertyRepo::<MySql>::new(&db.pool)
        .create(&NewProperty {
            owner_id: None,
            vocabulary_id: vocabulary.id,
            local_name: "title".to_string(),
            label: "Title".to_string(),
            comment: None,
        })
        .await
        .expect("create property");
    let properties = PropertyRepo::<MySql>::new(&db.pool)
        .list(&PropertyFilter::default(), &Paging::default())
        .await
        .expect("list properties");
    assert_eq!(properties[0].local_name, "title");

    let user = UserRepo::<MySql>::new(&db.pool)
        .create(&NewUser {
            email: "ada@example.org".to_string(),
            name: "Ada".to_string(),
            password_hash: None,
            role: "editor".to_string(),
            is_active: true,
            created: None,
        })
        .await
        .expect("create user");
    PasswordCreationRepo::<MySql>::new(&db.pool)
        .create(&NewPasswordCreation {
            id: "ResetKey".to_string(),
            user_id: user.id,
            activate: true,
            created: None,
        })
        .await
        .expect("create password key");
    let stored = PasswordCreationRepo::<MySql>::new(&db.pool)
        .get("ResetKey")
        .await
        .expect("read the password key")
        .expect("the key is there");
    assert_eq!(stored.id, "ResetKey");

    db.cleanup().await;
}

#[tokio::test]
async fn a_property_filter_matches_a_literal_and_treats_a_percent_as_a_literal() {
    let Some(db) = common::mysql().await else {
        return;
    };
    let title = title_property(&db.pool).await;
    let hit = item_titled(&db.pool, "Bronze helmet", title).await;
    item_titled(&db.pool, "Silver cup", title).await;
    item_titled(&db.pool, "50% off", title).await;

    let items = ItemRepo::<MySql>::new(&db.pool);
    let rows = items
        .list_filtered(
            &ItemFilter::default(),
            &contains(title, "helmet"),
            &Paging::default(),
        )
        .await
        .expect("list items");
    assert_eq!(rows.len(), 1, "one item carries the word");
    assert_eq!(rows[0].id(), hit);

    let rows = items
        .list_filtered(
            &ItemFilter::default(),
            &contains(title, "50%"),
            &Paging::default(),
        )
        .await
        .expect("list items with a percent in the needle");
    assert_eq!(rows.len(), 1, "the percent is a literal, not a wildcard");

    db.cleanup().await;
}

async fn value_texts(values: &ValueRepo<'_, MySql>, resource: ResourceId) -> Vec<String> {
    values
        .list_for_resource(resource)
        .await
        .expect("list values")
        .into_iter()
        .filter_map(|value| value.value)
        .collect()
}

#[tokio::test]
async fn a_whole_item_create_and_a_value_rewrite_commit_or_roll_back_as_one() {
    let Some(db) = common::mysql().await else {
        return;
    };
    let property = title_property(&db.pool).await;
    let no_such_property = PropertyId::from(999_999);
    let set = ItemSetRepo::<MySql>::new(&db.pool)
        .create(&NewItemSet::default())
        .await
        .expect("create item set");
    let items = ItemRepo::<MySql>::new(&db.pool);
    let values = ValueRepo::<MySql>::new(&db.pool);

    let refused = items
        .create_whole(&NewWholeItem {
            item: NewItem::default(),
            item_sets: vec![set.id()],
            values: vec![
                ValueContent::literal(property, "first"),
                ValueContent::literal(no_such_property, "second"),
            ],
        })
        .await;
    assert!(refused.is_err(), "the foreign key refuses the second value");
    assert!(
        items
            .list(&ItemFilter::default(), &Paging::default())
            .await
            .expect("list items")
            .is_empty(),
        "a refused create leaves no item"
    );

    let item = items
        .create_whole(&NewWholeItem {
            item: NewItem::default(),
            item_sets: vec![set.id()],
            values: vec![ValueContent::literal(property, "old")],
        })
        .await
        .expect("create whole");
    assert_eq!(
        items.list_item_sets(item.id()).await.expect("list sets"),
        vec![set.id()]
    );
    let old = values
        .list_for_resource(item.resource.id)
        .await
        .expect("list values");
    let refused = values
        .rewrite(
            item.resource.id,
            &[old[0].id],
            &[ValueContent::literal(no_such_property, "new")],
        )
        .await;
    assert!(refused.is_err());
    assert_eq!(value_texts(&values, item.resource.id).await, ["old"]);

    values
        .rewrite(
            item.resource.id,
            &[old[0].id],
            &[ValueContent::literal(property, "new")],
        )
        .await
        .expect("rewrite");
    assert_eq!(value_texts(&values, item.resource.id).await, ["new"]);

    db.cleanup().await;
}

#[tokio::test]
async fn attaching_an_item_to_an_item_set_twice_keeps_one_row() {
    let Some(db) = common::mysql().await else {
        return;
    };
    let items = ItemRepo::<MySql>::new(&db.pool);
    let item = items
        .create(&NewItem::default())
        .await
        .expect("create item");
    let set = ItemSetRepo::<MySql>::new(&db.pool)
        .create(&NewItemSet::default())
        .await
        .expect("create item set");

    items
        .add_to_item_set(item.id(), set.id())
        .await
        .expect("attach");
    items
        .add_to_item_set(item.id(), set.id())
        .await
        .expect("attach again");

    assert_eq!(
        items.list_item_sets(item.id()).await.expect("list sets"),
        vec![set.id()]
    );

    db.cleanup().await;
}

#[tokio::test]
async fn a_site_round_trips_and_its_upserts_replace() {
    let Some(db) = common::mysql().await else {
        return;
    };
    let sites = SiteRepo::<MySql>::new(&db.pool);
    let site = sites
        .create(&NewSite {
            slug: "museum".to_string(),
            theme: "default".to_string(),
            title: "The Museum".to_string(),
            ..NewSite::default()
        })
        .await
        .expect("create site");
    let page = sites
        .add_page(&NewSitePage {
            site_id: site.id,
            slug: "about".to_string(),
            title: "About".to_string(),
            is_public: true,
            ..NewSitePage::default()
        })
        .await
        .expect("add page");
    let block = sites
        .add_block(&NewSiteBlock {
            page_id: page.id,
            layout: "html".to_string(),
            data: serde_json::json!({"html": "<p>Hello</p>"}),
            layout_data: None,
            position: 0,
        })
        .await
        .expect("add block");

    assert_eq!(
        sites.get_by_slug("MUSEUM").await.expect("read by slug"),
        Some(site.clone()),
        "the slug unique key is case-insensitive"
    );
    assert_eq!(
        sites.list_pages(site.id).await.expect("list pages"),
        vec![page.clone()]
    );
    assert_eq!(
        sites.list_blocks(page.id).await.expect("list blocks"),
        vec![block]
    );

    let set = ItemSetRepo::<MySql>::new(&db.pool)
        .create(&NewItemSet::default())
        .await
        .expect("create item set");
    sites
        .add_item_set(site.id, set.id(), Some(1))
        .await
        .expect("attach set");
    sites
        .add_item_set(site.id, set.id(), Some(7))
        .await
        .expect("attach set again");
    let membership = sites.list_item_sets(site.id).await.expect("list sets");
    assert_eq!(membership.len(), 1, "one membership row");
    assert_eq!(membership[0].position, Some(7), "the repeat replaced it");

    let user = UserRepo::<MySql>::new(&db.pool)
        .create(&NewUser {
            email: "editor@example.org".to_string(),
            name: "Editor".to_string(),
            password_hash: None,
            role: "editor".to_string(),
            is_active: true,
            created: None,
        })
        .await
        .expect("create user");
    sites
        .set_permission(site.id, user.id, "site_admin")
        .await
        .expect("grant");
    sites
        .set_permission(site.id, user.id, "editor")
        .await
        .expect("change");
    let permissions = sites
        .list_permissions(site.id)
        .await
        .expect("list permissions");
    assert_eq!(permissions.len(), 1, "one permission row");
    assert_eq!(permissions[0].role, "editor", "the repeat replaced it");

    assert_eq!(
        sites
            .list(&SiteFilter::default(), &Paging::default())
            .await
            .expect("list sites"),
        vec![site]
    );

    db.cleanup().await;
}

#[tokio::test]
async fn a_fulltext_index_row_is_saved_searched_and_deleted() {
    let Some(db) = common::mysql().await else {
        return;
    };
    let index = FulltextRepo::<MySql>::new(&db.pool);
    index
        .save(&FulltextSearchRow {
            id: ResourceId(1),
            resource: "items".to_string(),
            owner_id: None,
            is_public: true,
            title: Some("Bronze helmet".to_string()),
            text: Some("Excavated at Vindolanda".to_string()),
        })
        .await
        .expect("save index row");
    index
        .save(&FulltextSearchRow {
            id: ResourceId(2),
            resource: "item_sets".to_string(),
            owner_id: None,
            is_public: true,
            title: Some("Bronze age".to_string()),
            text: None,
        })
        .await
        .expect("save a second resource name");

    let matches = index.search("items", "helmet").await.expect("search");
    assert_eq!(matches.len(), 1, "one matching item");
    assert_eq!(matches[0].id, ResourceId(1));
    assert_eq!(
        index
            .search("items", "vindolanda")
            .await
            .expect("search the text column")
            .len(),
        1,
        "the text column is indexed too"
    );
    assert!(
        index
            .search("items", "absent")
            .await
            .expect("search with no match")
            .is_empty(),
        "a word no row carries matches nothing"
    );
    let other = index
        .search("item_sets", "bronze")
        .await
        .expect("search the other resource name");
    assert_eq!(other.len(), 1, "the other resource name holds the word");
    assert_eq!(other[0].id, ResourceId(2));
    assert!(
        index
            .search("item_sets", "helmet")
            .await
            .expect("search across the resource name")
            .is_empty(),
        "the resource name narrows the search"
    );

    assert!(index.delete(ResourceId(1), "items").await.expect("delete"));
    assert!(
        index
            .search("items", "helmet")
            .await
            .expect("search after delete")
            .is_empty()
    );

    db.cleanup().await;
}

/// The term list table from the Custom Vocab module: the JSON columns round-trip through
/// `LONGTEXT`, and a delete sets the list's values back to `literal`, which writes the `value`
/// table MariaDB reserves.
#[tokio::test]
async fn a_term_list_round_trips_and_its_delete_resets_its_values() {
    let Some(db) = common::mysql().await else {
        return;
    };
    let title = title_property(&db.pool).await;
    let item = item_titled(&db.pool, "Bronze helmet", title).await;
    let repo = CustomVocabRepo::<MySql>::new(&db.pool);
    let list = repo
        .create(&NewCustomVocab {
            item_set_id: None,
            owner_id: None,
            label: "Places".to_string(),
            lang: Some("en".to_string()),
            terms: Some(json!(["Cork", "Dublin"])),
            uris: Some(json!({"http://www.wikidata.org/entity/Q36405": "Cork"})),
        })
        .await
        .expect("create list");
    assert_eq!(
        repo.get(list.id).await.expect("read list"),
        Some(list.clone())
    );
    assert!(
        repo.find_by_label("PLACES").await.expect("find").is_some(),
        "the label collates without case"
    );

    let name = format!("customvocab:{}", list.id);
    ValueRepo::<MySql>::new(&db.pool)
        .create(&NewValue {
            resource_id: item.as_resource(),
            property_id: title,
            value_resource_id: None,
            value_annotation_id: None,
            r#type: name.clone(),
            lang: None,
            value: Some("Cork".to_string()),
            uri: None,
            is_public: true,
        })
        .await
        .expect("create value");

    assert!(repo.delete(list.id).await.expect("delete list"));

    let types: Vec<String> =
        sqlx::query_scalar("SELECT type FROM value WHERE resource_id = ? ORDER BY id")
            .bind(item.0 as i64)
            .fetch_all(&db.pool)
            .await
            .expect("read types");
    assert_eq!(types, ["literal", "literal"]);
    assert_eq!(repo.get(list.id).await.expect("read list"), None);

    db.cleanup().await;
}

/// `0003_publication.sql`: a publish appends numbered versions, the slug is unique, term state
/// saves without a version, and the versions outlive the deleted list.
#[tokio::test]
async fn a_publication_appends_versions_and_outlives_its_list() {
    let Some(db) = common::mysql().await else {
        return;
    };
    let list = CustomVocabRepo::<MySql>::new(&db.pool)
        .create(&NewCustomVocab {
            item_set_id: None,
            owner_id: None,
            label: "Places".to_string(),
            lang: Some("en".to_string()),
            terms: Some(json!(["Cork", "Dublin"])),
            uris: None,
        })
        .await
        .expect("create list");
    let target = PublicationTarget::TermList(list.id);
    let repo = PublicationRepo::<MySql>::new(&db.pool);
    let drafted = repo
        .save_terms(
            target,
            &json!({"Cork": {"exact_match": "https://sws.geonames.org/2965140/"}}),
        )
        .await
        .expect("save term state");
    assert_eq!(repo.latest(drafted.id).await.expect("latest"), None);

    let terms = json!({"Cork": {"local": "1"}, "Dublin": {"local": "2"}});
    let created = chrono::NaiveDate::from_ymd_opt(2026, 10, 18)
        .and_then(|day| day.and_hms_opt(9, 30, 0))
        .expect("a date");
    for snapshot in [json!({"n": 1}), json!({"n": 2})] {
        repo.publish(
            target,
            &NewPublicationVersion {
                slug: "places",
                namespace_uri: "http://localhost/vocab/places/",
                terms: &terms,
                snapshot: &snapshot,
            },
            created,
        )
        .await
        .expect("publish");
    }
    let versions = repo.versions(drafted.id).await.expect("versions");
    assert_eq!(
        versions
            .iter()
            .map(|row| (row.version, row.created, row.snapshot.clone()))
            .collect::<Vec<_>>(),
        [(1, created, json!({"n": 1})), (2, created, json!({"n": 2}))]
    );

    let other = VocabularyRepo::<MySql>::new(&db.pool)
        .create(&NewVocabulary {
            owner_id: None,
            namespace_uri: "http://localhost/vocab/places#".to_string(),
            prefix: "places".to_string(),
            label: "Places".to_string(),
            comment: None,
        })
        .await
        .expect("create vocabulary");
    let taken = repo
        .publish(
            PublicationTarget::Vocabulary(other.id),
            &NewPublicationVersion {
                slug: "places",
                namespace_uri: "http://localhost/vocab/places#",
                terms: &json!({}),
                snapshot: &json!({}),
            },
            created,
        )
        .await;
    assert!(taken.is_err(), "the slug is unique");

    assert!(
        CustomVocabRepo::<MySql>::new(&db.pool)
            .delete(list.id)
            .await
            .expect("delete list")
    );
    let record = repo
        .by_slug("places")
        .await
        .expect("by slug")
        .expect("the record stays");
    assert_eq!(record.target, None);
    assert_eq!(repo.versions(record.id).await.expect("versions").len(), 2);

    db.cleanup().await;
}

#[tokio::test]
async fn deleting_an_item_removes_its_values_and_set_membership() {
    let Some(db) = common::mysql().await else {
        return;
    };
    let title = title_property(&db.pool).await;
    let item = item_titled(&db.pool, "Bronze helmet", title).await;
    let set = ItemSetRepo::<MySql>::new(&db.pool)
        .create(&NewItemSet::default())
        .await
        .expect("create item set");
    let items = ItemRepo::<MySql>::new(&db.pool);
    items.add_to_item_set(item, set.id()).await.expect("attach");

    assert!(items.delete(item).await.expect("delete item"));

    let values: i64 = sqlx::query_scalar("SELECT count(*) FROM value WHERE resource_id = ?")
        .bind(item.0 as i64)
        .fetch_one(&db.pool)
        .await
        .expect("count values");
    assert_eq!(values, 0, "the values went with the item");
    let memberships: i64 = sqlx::query_scalar("SELECT count(*) FROM item_item_set")
        .fetch_one(&db.pool)
        .await
        .expect("count memberships");
    assert_eq!(memberships, 0, "the membership cascaded");

    db.cleanup().await;
}

#[tokio::test]
async fn the_setting_tables_upsert_on_a_mysql_duplicate_key() {
    let Some(db) = common::mysql().await else {
        return;
    };
    let settings = SettingRepo::<MySql>::new(&db.pool);
    settings
        .set("administrator_email", &json!("ada@example.org"))
        .await
        .expect("insert a setting");
    settings
        .set("administrator_email", &json!("grace@example.org"))
        .await
        .expect("replace a setting");
    assert_eq!(
        settings
            .get("administrator_email")
            .await
            .expect("read setting"),
        Some(json!("grace@example.org")),
        "the repeat replaced the value"
    );

    let site = SiteRepo::<MySql>::new(&db.pool)
        .create(&NewSite {
            slug: "museum".to_string(),
            theme: "default".to_string(),
            title: "The Museum".to_string(),
            ..NewSite::default()
        })
        .await
        .expect("create site");
    let site_settings = SiteSettingRepo::<MySql>::new(&db.pool);
    site_settings
        .set(site.id, "default_locale", &json!("en"))
        .await
        .expect("insert a site setting");
    site_settings
        .set(site.id, "default_locale", &json!("fr"))
        .await
        .expect("replace a site setting");
    assert_eq!(
        site_settings
            .get(site.id, "default_locale")
            .await
            .expect("read site setting"),
        Some(json!("fr")),
        "the repeat replaced the value"
    );

    let user = UserRepo::<MySql>::new(&db.pool)
        .create(&NewUser {
            email: "ada@example.org".to_string(),
            name: "Ada".to_string(),
            password_hash: None,
            role: "editor".to_string(),
            is_active: true,
            created: None,
        })
        .await
        .expect("create user");
    let user_settings = UserSettingRepo::<MySql>::new(&db.pool);
    user_settings
        .set(user.id, "default_locale", &json!("en"))
        .await
        .expect("insert a user setting");
    user_settings
        .set(user.id, "default_locale", &json!("de"))
        .await
        .expect("replace a user setting");
    assert_eq!(
        user_settings
            .get(user.id, "default_locale")
            .await
            .expect("read user setting"),
        Some(json!("de")),
        "the repeat replaced the value"
    );

    db.cleanup().await;
}

#[tokio::test]
async fn a_session_upsert_and_a_job_log_use_the_mysql_spelling() {
    let Some(db) = common::mysql().await else {
        return;
    };
    let sessions = SessionRepo::<MySql>::new(&db.pool);
    sessions
        .save("sess-1", b"user=1", Some(1000))
        .await
        .expect("insert a session");
    sessions
        .save("sess-1", b"user=2", Some(1001))
        .await
        .expect("replace a session");
    let stored = sessions
        .get("sess-1")
        .await
        .expect("read session")
        .expect("session is there");
    assert_eq!(stored.data, b"user=2", "the repeat replaced the data");
    assert_eq!(stored.modified, 1001, "and the time");

    let jobs = JobRepo::<MySql>::new(&db.pool);
    let job = jobs
        .create(&NewJob {
            owner_id: None,
            pid: None,
            status: Some("in_progress".to_string()),
            class: "Omeka\\Job\\BatchDelete".to_string(),
            args: None,
            log: None,
            started: None,
        })
        .await
        .expect("create job");
    jobs.append_log(job.id, "first").await.expect("log line");
    jobs.append_log(job.id, "second").await.expect("log line");
    assert_eq!(
        jobs.get(job.id)
            .await
            .expect("read job")
            .unwrap()
            .log
            .as_deref(),
        Some("first\nsecond\n"),
        "CONCAT appends each line and a newline"
    );

    let modules = ModuleRepo::<MySql>::new(&db.pool);
    modules
        .install("OmekaModuleFoo", "1.2.0")
        .await
        .expect("install a module");
    assert!(
        modules
            .set_active("OmekaModuleFoo", false)
            .await
            .expect("deactivate")
    );
    assert_eq!(modules.list().await.expect("list modules").len(), 1);

    db.cleanup().await;
}

fn link_first_page(page: PageId) -> serde_json::Value {
    json!([{ "type": "page", "data": { "id": page.0 }, "links": [] }])
}

/// `SiteRepo::create_whole` on InnoDB: a whole create stores every row with the navigation
/// linking the first page, and a permission whose user fails its foreign key after the site, page
/// and block are inserted rolls all of them back.
#[tokio::test]
async fn a_whole_site_create_commits_together_or_not_at_all() {
    let Some(db) = common::mysql().await else {
        return;
    };
    let new = |slug: &str, user: UserId| NewWholeSite {
        site: NewSite {
            slug: slug.to_string(),
            theme: "default".to_string(),
            title: "Whole".to_string(),
            ..NewSite::default()
        },
        first_page: NewFirstPage {
            slug: "welcome".to_string(),
            title: "Welcome".to_string(),
            is_public: true,
            blocks: vec![NewFirstPageBlock {
                layout: "html".to_string(),
                data: json!({ "html": "<p>One</p>" }),
                layout_data: None,
            }],
        },
        navigation_linking_first_page: Some(link_first_page),
        permissions: vec![(user, "admin".to_string())],
        item_sets: Vec::new(),
        settings: vec![("locale".to_string(), json!("es"))],
    };
    let user = UserRepo::<MySql>::new(&db.pool)
        .create(&NewUser {
            email: "whole@example.org".to_string(),
            name: "Whole".to_string(),
            password_hash: None,
            role: "editor".to_string(),
            is_active: true,
            created: None,
        })
        .await
        .expect("create user");
    let sites = SiteRepo::<MySql>::new(&db.pool);

    let site = sites
        .create_whole(&new("whole", user.id))
        .await
        .expect("create whole site");
    let pages = sites.list_pages(site.id).await.expect("list pages");
    assert_eq!(pages.len(), 1);
    let stored = sites.get(site.id).await.expect("read site").expect("site");
    assert_eq!(stored.navigation, link_first_page(pages[0].id));
    assert_eq!(stored.modified, Some(stored.created));

    assert!(
        sites
            .create_whole(&new("broken", UserId(999_999)))
            .await
            .is_err()
    );
    assert_eq!(sites.get_by_slug("broken").await.expect("read slug"), None);
    for table in [
        "site",
        "site_page",
        "site_page_block",
        "site_permission",
        "site_setting",
    ] {
        let rows: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table}"))
            .fetch_one(&db.pool)
            .await
            .expect("count rows");
        assert_eq!(rows, 1, "{table} holds only the whole site's row");
    }
}
