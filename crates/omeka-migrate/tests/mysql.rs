//! The MySQL end to end suite: a real upstream-shaped source, migrated into SQLite.
//!
//! Gated on a reachable server, the same way `omeka-db`'s MySQL suite is. `OMEKA_MYSQL_URL` names
//! a server, not a database (`mysql://root:secret@127.0.0.1:3306`); each test creates its own
//! database from upstream's own schema, seeds a small fixture through the port's repositories, and
//! drops the database again. With no variable set, or a server that does not answer, the helper
//! prints why and the test returns: the suite skips loudly.
//!
//! ```text
//! OMEKA_MYSQL_URL=mysql://root:secret@127.0.0.1:3306 \
//!   cargo test -p omeka-migrate --test mysql
//! ```

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use omeka_db::repos::{
    ApiKeyRepo, ItemFilter, ItemRepo, ItemSetRepo, MediaFilter, MediaRepo, Paging, PropertyRepo,
    SiteFilter, SiteRepo, UserFilter, UserRepo, ValueRepo, VocabularyRepo,
};
use omeka_db::rows::{
    NewApiKey, NewItem, NewItemSet, NewMedia, NewProperty, NewSite, NewSiteBlock, NewSitePage,
    NewUser, NewValue, NewVocabulary,
};
use omeka_migrate::{MigrateError, MigrateOptions, migrate_from_upstream};
use sqlx::{MySql, MySqlPool};

/// A source database for one test, dropped on cleanup.
struct Source {
    pool: MySqlPool,
    base: String,
    name: String,
}

impl Source {
    async fn cleanup(&self) {
        if let Ok(admin) = omeka_db::open_mysql(&self.base).await {
            let _ = sqlx::query(&format!("DROP DATABASE IF EXISTS `{}`", self.name))
                .execute(&admin)
                .await;
        }
    }
}

fn server_url() -> Option<String> {
    match std::env::var("OMEKA_MYSQL_URL") {
        Ok(url) if !url.trim().is_empty() => Some(url.trim().to_string()),
        _ => {
            eprintln!("SKIP: OMEKA_MYSQL_URL is not set, so the MySQL suite did not run");
            None
        }
    }
}

/// The server URL with the database segment replaced, keeping any query string.
fn with_database(base: &str, name: &str) -> String {
    let (head, query) = match base.split_once('?') {
        Some((head, query)) => (head, Some(query)),
        None => (base, None),
    };
    let head = head.trim_end_matches('/');
    match query {
        Some(query) => format!("{head}/{name}?{query}"),
        None => format!("{head}/{name}"),
    }
}

/// Create a database holding upstream's own schema, or `None` when no server answers.
async fn source() -> Option<Source> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let base = server_url()?;
    let seq = NEXT.fetch_add(1, Ordering::Relaxed);
    let name = format!("omeka_migrate_{}_{seq}", std::process::id());

    let admin = match omeka_db::open_mysql(&base).await {
        Ok(admin) => admin,
        Err(error) => {
            eprintln!("SKIP: cannot reach {base}: {error}");
            return None;
        }
    };
    if let Err(error) = sqlx::query(&format!("CREATE DATABASE `{name}`"))
        .execute(&admin)
        .await
    {
        eprintln!("SKIP: cannot create database {name}: {error}");
        return None;
    }

    let url = with_database(&base, &name);
    let pool = match omeka_db::open_mysql(&url).await {
        Ok(pool) => pool,
        Err(error) => {
            eprintln!("SKIP: cannot open {url}: {error}");
            return None;
        }
    };
    if let Err(error) = omeka_db::migrate_mysql(&pool).await {
        eprintln!("SKIP: cannot migrate {name}: {error}");
        return None;
    }
    Some(Source { pool, base, name })
}

/// Seed an upstream-shaped fixture: a vocabulary, an item with a value and media, an item set, a
/// site with a page and block, and an API key.
async fn seed(pool: &MySqlPool) {
    let users = UserRepo::<MySql>::new(pool);
    let ada = users
        .create(&NewUser {
            email: "ada@example.org".to_string(),
            name: "Ada".to_string(),
            password_hash: Some("$2y$hash".to_string()),
            role: "global_admin".to_string(),
            is_active: true,
            created: None,
        })
        .await
        .expect("create ada");
    users
        .create(&NewUser {
            email: "grace@example.org".to_string(),
            name: "Grace".to_string(),
            password_hash: None,
            role: "editor".to_string(),
            is_active: true,
            created: None,
        })
        .await
        .expect("create grace");

    let vocabulary = VocabularyRepo::<MySql>::new(pool)
        .create(&NewVocabulary {
            owner_id: Some(ada.id),
            namespace_uri: "http://purl.org/dc/terms/".to_string(),
            prefix: "dcterms".to_string(),
            label: "Dublin Core".to_string(),
            comment: None,
        })
        .await
        .expect("create vocabulary");
    let title = PropertyRepo::<MySql>::new(pool)
        .create(&NewProperty {
            owner_id: Some(ada.id),
            vocabulary_id: vocabulary.id,
            local_name: "title".to_string(),
            label: "Title".to_string(),
            comment: None,
        })
        .await
        .expect("create property");

    let item = ItemRepo::<MySql>::new(pool)
        .create(&NewItem::default())
        .await
        .expect("create item");
    MediaRepo::<MySql>::new(pool)
        .create(&NewMedia {
            item_id: item.id(),
            ingester: "upload".to_string(),
            renderer: "file".to_string(),
            media_type: Some("image/jpeg".to_string()),
            storage_id: Some("store/helmet.jpg".to_string()),
            has_original: true,
            ..NewMedia::default()
        })
        .await
        .expect("create media");
    ValueRepo::<MySql>::new(pool)
        .create(&NewValue {
            resource_id: item.resource.id,
            property_id: title.id,
            value_resource_id: None,
            value_annotation_id: None,
            r#type: "literal".to_string(),
            lang: Some("en".to_string()),
            value: Some("Bronze helmet".to_string()),
            uri: None,
            is_public: true,
        })
        .await
        .expect("create value");

    let item_set = ItemSetRepo::<MySql>::new(pool)
        .create(&NewItemSet::default())
        .await
        .expect("create item set");
    ItemRepo::<MySql>::new(pool)
        .add_to_item_set(item.id(), item_set.id())
        .await
        .expect("attach item");

    let sites = SiteRepo::<MySql>::new(pool);
    let site = sites
        .create(&NewSite {
            owner_id: Some(ada.id),
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
    sites
        .add_block(&NewSiteBlock {
            page_id: page.id,
            layout: "html".to_string(),
            data: serde_json::json!({"html": "<p>Hello</p>"}),
            layout_data: None,
            position: 0,
        })
        .await
        .expect("add block");

    ApiKeyRepo::<MySql>::new(pool)
        .create(&NewApiKey {
            id: "key-abc".to_string(),
            owner_id: ada.id,
            label: "dev".to_string(),
            credential_hash: "hash".to_string(),
            last_ip: None,
            last_accessed: None,
            created: None,
        })
        .await
        .expect("create api key");
}

/// A unique SQLite target path for one test.
fn target_path(name: &str) -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let seq = NEXT.fetch_add(1, Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.subsec_nanos());
    std::env::temp_dir().join(format!(
        "omeka-migrate-{}-{seq}-{nanos}-{name}.sqlite",
        std::process::id()
    ))
}

fn remove_db(path: &Path) {
    for suffix in ["", "-wal", "-shm"] {
        let mut name = std::ffi::OsString::from(path.as_os_str());
        name.push(suffix);
        let candidate = PathBuf::from(name);
        if candidate.exists() {
            let _ = std::fs::remove_file(&candidate);
        }
    }
}

#[tokio::test]
async fn a_small_upstream_source_migrates_into_a_sqlite_target() {
    let Some(source) = source().await else {
        return;
    };
    seed(&source.pool).await;

    let path = target_path("end-to-end");
    remove_db(&path);
    let report = migrate_from_upstream(
        &with_database(&source.base, &source.name),
        &path,
        &MigrateOptions::default(),
    )
    .await
    .expect("the migration succeeds");

    assert_eq!(report.table("user").map(|t| t.read), Some(2));
    assert_eq!(report.table("item").map(|t| t.written), Some(1));
    assert_eq!(report.table("value").map(|t| t.written), Some(1));
    assert_eq!(report.table("media").map(|t| t.written), Some(1));
    assert_eq!(report.totals().2, 0, "nothing is skipped");

    let target = omeka_db::open_file(&path).await.expect("open the target");

    let users = UserRepo::new(&target)
        .list(&UserFilter::default(), &Paging::default())
        .await
        .expect("list users");
    assert_eq!(users.len(), 2, "both users moved");

    let items = ItemRepo::new(&target)
        .list(&ItemFilter::default(), &Paging::default())
        .await
        .expect("list items");
    assert_eq!(items.len(), 1);
    let item = items[0].id();

    let values = ValueRepo::new(&target)
        .list_for_resource(item.as_resource())
        .await
        .expect("list values");
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].value.as_deref(), Some("Bronze helmet"));
    assert_eq!(values[0].lang.as_deref(), Some("en"));

    let media = MediaRepo::new(&target)
        .list(&MediaFilter::default(), &Paging::default())
        .await
        .expect("list media");
    assert_eq!(media.len(), 1);
    assert_eq!(media[0].item_id, item, "media points at the item");

    let sets = ItemRepo::new(&target)
        .list_item_sets(item)
        .await
        .expect("list an item's sets");
    assert_eq!(sets.len(), 1, "the item set membership moved");

    let sites = SiteRepo::new(&target)
        .list(&SiteFilter::default(), &Paging::default())
        .await
        .expect("list sites");
    assert_eq!(sites.len(), 1);
    let pages = SiteRepo::new(&target)
        .list_pages(sites[0].id)
        .await
        .expect("list pages");
    assert_eq!(pages.len(), 1);
    let blocks = SiteRepo::new(&target)
        .list_blocks(pages[0].id)
        .await
        .expect("list blocks");
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].data["html"], serde_json::json!("<p>Hello</p>"));

    let keys = ApiKeyRepo::new(&target)
        .list_for_owner(users[0].id)
        .await
        .expect("list keys");
    assert_eq!(keys.len(), 1);
    assert_eq!(keys[0].id, "key-abc");

    source.cleanup().await;
    remove_db(&path);
}

#[tokio::test]
async fn a_source_missing_a_column_is_refused_by_name() {
    let Some(source) = source().await else {
        return;
    };
    sqlx::query("ALTER TABLE user DROP COLUMN is_active")
        .execute(&source.pool)
        .await
        .expect("drop the column");

    let path = target_path("missing-column");
    remove_db(&path);
    let error = migrate_from_upstream(
        &with_database(&source.base, &source.name),
        &path,
        &MigrateOptions::default(),
    )
    .await
    .expect_err("the stale source is refused");

    match error {
        MigrateError::MissingColumn { table, column } => {
            assert_eq!(table, "user");
            assert_eq!(column, "is_active");
        }
        other => panic!("expected a missing column error, got {other:?}"),
    }
    assert!(
        !path.exists(),
        "the refusal happened before the target was created"
    );

    source.cleanup().await;
    remove_db(&path);
}

#[tokio::test]
async fn a_dry_run_leaves_no_target_behind() {
    let Some(source) = source().await else {
        return;
    };
    seed(&source.pool).await;

    let path = target_path("dry-run");
    remove_db(&path);
    let report = migrate_from_upstream(
        &with_database(&source.base, &source.name),
        &path,
        &MigrateOptions { dry_run: true },
    )
    .await
    .expect("the dry run succeeds");

    assert!(report.dry_run);
    assert_eq!(report.table("item").map(|t| t.written), Some(1));
    assert!(!path.exists(), "the dry run never opened the target");

    source.cleanup().await;
    remove_db(&path);
}
