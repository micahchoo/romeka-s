//! The MySQL backend: the behaviour the SQLite suites freeze, against a real server.
//!
//! Gated on the `mysql` feature and on a reachable server. `OMEKA_MYSQL_URL` names a server, not a
//! database (`mysql://root:secret@127.0.0.1:3306`); each test creates and drops its own database.
//! With no server the helper prints why and every test returns, so the suite skips loudly instead
//! of failing. Run it with
//!
//! ```text
//! OMEKA_MYSQL_URL=mysql://root:secret@127.0.0.1:3306 \
//!   cargo test -p omeka-db --features mysql --test mysql
//! ```

#![cfg(feature = "mysql")]
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use omeka_db::ids::{ItemId, PropertyId, ResourceId};
use omeka_db::repos::{
    FulltextRepo, ItemFilter, ItemRepo, ItemSetRepo, JobRepo, MatchKind, ModuleRepo, Paging,
    PropertyRepo, QueryFilter, SessionRepo, SettingRepo, SiteFilter, SiteRepo, SiteSettingRepo,
    UserRepo, UserSettingRepo, ValueQuery, ValueRepo, VocabularyRepo,
};
use omeka_db::rows::{
    FulltextSearchRow, NewItem, NewItemSet, NewJob, NewProperty, NewSite, NewSiteBlock,
    NewSitePage, NewUser, NewValue, NewVocabulary,
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
        common::with_database("mysql://root:secret@127.0.0.1:3306", "omeka_test_1"),
        "mysql://root:secret@127.0.0.1:3306/omeka_test_1"
    );
    assert_eq!(
        common::with_database("mysql://root@db:3306/omeka", "omeka_test_2"),
        "mysql://root@db:3306/omeka_test_2"
    );
    assert_eq!(
        common::with_database("mysql://root@db:3306?ssl-mode=disabled", "omeka_test_3"),
        "mysql://root@db:3306/omeka_test_3?ssl-mode=disabled"
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

    omeka_db::migrate_mysql(&db.pool)
        .await
        .expect("apply migrations a second time");

    assert_eq!(
        UserRepo::<MySql>::new(&db.pool).get(user.id).await.unwrap(),
        Some(user),
        "the second migrate kept the row"
    );
    let applied: i64 = sqlx::query_scalar("SELECT count(*) FROM _sqlx_migrations")
        .fetch_one(&db.pool)
        .await
        .expect("count migrations");
    assert_eq!(applied, 1, "one migration file");

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
