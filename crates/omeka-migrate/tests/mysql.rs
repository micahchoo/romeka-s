//! The MySQL end to end suite: a real upstream-shaped source, migrated into SQLite.
//!
//! Gated on a reachable server, the same way `omeka-db`'s MySQL suite is. `ROMEKA_MYSQL_URL` names
//! a server, not a database (`mysql://root:secret@127.0.0.1:3306`); each test creates its own
//! database from upstream's own schema, seeds a small fixture through the port's repositories, and
//! drops the database again. With no variable set, or a server that does not answer, the helper
//! prints why and the test returns: the suite skips loudly.
//!
//! ```text
//! ROMEKA_MYSQL_URL=mysql://root:secret@127.0.0.1:3306 \
//!   cargo test -p omeka-migrate --test mysql
//! ```

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use omeka_db::repos::{
    ApiKeyRepo, AssetFilter, AssetRepo, ItemFilter, ItemRepo, ItemSetRepo, ItemSiteRepo,
    MediaFilter, MediaRepo, Paging, PasswordCreationRepo, PropertyRepo, SettingRepo,
    SiteBlockAttachmentRepo, SiteFilter, SiteRepo, SiteSettingRepo, UserFilter, UserRepo,
    UserSettingRepo, ValueRepo, VocabularyRepo,
};
use omeka_db::rows::{
    ItemSiteRow, NewApiKey, NewAsset, NewItem, NewItemSet, NewMedia, NewPasswordCreation,
    NewProperty, NewResource, NewSite, NewSiteBlock, NewSiteBlockAttachment, NewSitePage, NewUser,
    NewValue, NewVocabulary,
};
use omeka_migrate::{MigrateError, MigrateOptions, migrate_from_upstream};
use sqlx::{MySql, MySqlPool};

/// The asset file the source store holds and the migrated site renders.
const ASSET_STORAGE_ID: &str = "a1b2c3d4e5f60718293a4b5c6d7e8f9012345678";
/// The media file name the media block resolves to a thumbnail URL.
const MEDIA_STORAGE_ID: &str = "helmet123";

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
    match std::env::var("ROMEKA_MYSQL_URL") {
        Ok(url) if !url.trim().is_empty() => Some(url.trim().to_string()),
        _ => {
            eprintln!("SKIP: ROMEKA_MYSQL_URL is not set, so the MySQL suite did not run");
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
    if let Err(error) = omeka_db::migrate(&pool).await {
        eprintln!("SKIP: cannot migrate {name}: {error}");
        return None;
    }
    Some(Source { pool, base, name })
}

/// Seed an upstream-shaped fixture: an asset with a file, a vocabulary, an item with a title value,
/// media and a site membership, an item set, a site with a thumbnail, pages whose blocks reference
/// the asset and the page, a media block with an attachment, and the three setting tables.
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
    let grace = users
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

    let asset = AssetRepo::<MySql>::new(pool)
        .create(&NewAsset {
            owner_id: Some(ada.id),
            name: "omeka-logo.png".to_string(),
            media_type: "image/png".to_string(),
            storage_id: ASSET_STORAGE_ID.to_string(),
            extension: Some("png".to_string()),
            alt_text: None,
        })
        .await
        .expect("create asset");

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
        .create(&NewItem {
            resource: NewResource {
                thumbnail_id: Some(asset.id),
                title: Some("Bronze helmet".to_string()),
                ..NewResource::default()
            },
            ..NewItem::default()
        })
        .await
        .expect("create item");
    let media = MediaRepo::<MySql>::new(pool)
        .create(&NewMedia {
            item_id: item.id(),
            ingester: "upload".to_string(),
            renderer: "file".to_string(),
            media_type: Some("image/jpeg".to_string()),
            storage_id: Some(MEDIA_STORAGE_ID.to_string()),
            extension: Some("jpg".to_string()),
            has_original: true,
            has_thumbnails: true,
            alt_text: Some("A helmet".to_string()),
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
            thumbnail_id: Some(asset.id),
            owner_id: Some(ada.id),
            slug: "museum".to_string(),
            theme: "default".to_string(),
            title: "The Museum".to_string(),
            ..NewSite::default()
        })
        .await
        .expect("create site");
    let about = sites
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
            page_id: about.id,
            layout: "html".to_string(),
            data: serde_json::json!({"html": "<p>Hello</p>"}),
            layout_data: None,
            position: 0,
        })
        .await
        .expect("add block");

    let brand = sites
        .add_page(&NewSitePage {
            site_id: site.id,
            slug: "brand".to_string(),
            title: "Brand".to_string(),
            is_public: true,
            ..NewSitePage::default()
        })
        .await
        .expect("add the brand page");
    sites
        .add_block(&NewSiteBlock {
            page_id: brand.id,
            layout: "asset".to_string(),
            data: serde_json::json!([{
                "id": asset.id.0,
                "page": about.id.0,
                "alt_link_title": "",
                "caption": "Our mark"
            }]),
            layout_data: None,
            position: 0,
        })
        .await
        .expect("add the asset block");

    let gallery = sites
        .add_page(&NewSitePage {
            site_id: site.id,
            slug: "gallery".to_string(),
            title: "Gallery".to_string(),
            is_public: true,
            ..NewSitePage::default()
        })
        .await
        .expect("add the gallery page");
    let media_block = sites
        .add_block(&NewSiteBlock {
            page_id: gallery.id,
            layout: "media".to_string(),
            data: serde_json::json!({"media_display": "thumbnail"}),
            layout_data: None,
            position: 0,
        })
        .await
        .expect("add the media block");
    SiteBlockAttachmentRepo::<MySql>::new(pool)
        .create(&NewSiteBlockAttachment {
            block_id: media_block.id,
            item_id: Some(item.id()),
            media_id: Some(media.id()),
            caption: "A helmet".to_string(),
            position: 1,
        })
        .await
        .expect("attach the media to the block");

    let banner = sites
        .add_page(&NewSitePage {
            site_id: site.id,
            slug: "banner".to_string(),
            title: "Banner".to_string(),
            is_public: true,
            ..NewSitePage::default()
        })
        .await
        .expect("add the banner page");
    sites
        .add_block(&NewSiteBlock {
            page_id: banner.id,
            layout: "html".to_string(),
            data: serde_json::json!({"html": "<p>Behind</p>"}),
            layout_data: Some(serde_json::json!({"background_image_asset": asset.id.0})),
            position: 0,
        })
        .await
        .expect("add the background block");

    let contents = sites
        .add_page(&NewSitePage {
            site_id: site.id,
            slug: "contents".to_string(),
            title: "Contents".to_string(),
            is_public: true,
            ..NewSitePage::default()
        })
        .await
        .expect("add the contents page");
    sites
        .add_block(&NewSiteBlock {
            page_id: contents.id,
            layout: "listOfPages".to_string(),
            data: serde_json::json!({
                "pagelist": serde_json::json!([
                    {"text": "About", "data": {"id": about.id.0}, "children": []}
                ]).to_string()
            }),
            layout_data: None,
            position: 0,
        })
        .await
        .expect("add the list of pages block");

    let mut site_row = sites.get(site.id).await.expect("read site").expect("site");
    site_row.navigation = serde_json::json!([
        {"type": "page", "data": {"id": about.id.0, "label": null}}
    ]);
    sites.update(&site_row).await.expect("link the page");

    ItemSiteRepo::<MySql>::new(pool)
        .create(&ItemSiteRow {
            item_id: item.id(),
            site_id: site.id,
        })
        .await
        .expect("attach the item to the site");

    SettingRepo::<MySql>::new(pool)
        .set("administrator_email", &serde_json::json!("ada@example.org"))
        .await
        .expect("write the global setting");
    let site_settings = SiteSettingRepo::<MySql>::new(pool);
    site_settings
        .set(
            site.id,
            "attachment_link_type",
            &serde_json::json!("original"),
        )
        .await
        .expect("write a site setting");
    site_settings
        .set(
            site.id,
            "installation_title",
            &serde_json::json!("Migrated Museum"),
        )
        .await
        .expect("write a second site setting");
    UserSettingRepo::<MySql>::new(pool)
        .set(ada.id, "default_locale", &serde_json::json!("fr"))
        .await
        .expect("write the user setting");
    PasswordCreationRepo::<MySql>::new(pool)
        .create(&NewPasswordCreation {
            id: "reset-key".to_string(),
            user_id: grace.id,
            activate: true,
            created: None,
        })
        .await
        .expect("create the password key");

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
        "romeka-migrate-{}-{seq}-{nanos}-{name}.sqlite",
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
    assert_eq!(pages.len(), 5, "every page moved");
    let about = pages
        .iter()
        .find(|page| page.slug == "about")
        .expect("the about page moved");
    let blocks = SiteRepo::new(&target)
        .list_blocks(about.id)
        .await
        .expect("list blocks");
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].data["html"], serde_json::json!("<p>Hello</p>"));

    let assets = AssetRepo::new(&target)
        .list(&AssetFilter::default(), &Paging::default())
        .await
        .expect("list assets");
    assert_eq!(assets.len(), 1);
    assert_eq!(assets[0].storage_id, ASSET_STORAGE_ID);
    assert_eq!(
        items[0].resource.thumbnail_id,
        Some(assets[0].id),
        "the item's thumbnail names the migrated asset"
    );
    assert_eq!(
        sites[0].thumbnail_id,
        Some(assets[0].id),
        "the site's thumbnail names the migrated asset"
    );

    let gallery = pages
        .iter()
        .find(|page| page.slug == "gallery")
        .expect("the gallery page moved");
    let media_block = SiteRepo::new(&target)
        .list_blocks(gallery.id)
        .await
        .expect("list the gallery blocks")
        .into_iter()
        .find(|block| block.layout == "media")
        .expect("the media block moved");
    let attachments = SiteBlockAttachmentRepo::new(&target)
        .list_for_block(media_block.id)
        .await
        .expect("list attachments");
    assert_eq!(attachments.len(), 1);
    assert_eq!(attachments[0].item_id, Some(item));
    assert_eq!(attachments[0].media_id, Some(media[0].id()));
    assert_eq!(attachments[0].caption, "A helmet");

    let site_items = ItemSiteRepo::new(&target)
        .list()
        .await
        .expect("read item_site");
    assert_eq!(
        site_items,
        vec![ItemSiteRow {
            item_id: item,
            site_id: sites[0].id,
        }],
        "the item is in the site pool"
    );

    assert_eq!(
        SettingRepo::new(&target)
            .get("administrator_email")
            .await
            .expect("read the global setting"),
        Some(serde_json::json!("ada@example.org"))
    );
    assert_eq!(
        SiteSettingRepo::new(&target)
            .get(sites[0].id, "attachment_link_type")
            .await
            .expect("read a site setting"),
        Some(serde_json::json!("original"))
    );
    assert_eq!(
        UserSettingRepo::new(&target)
            .get(users[0].id, "default_locale")
            .await
            .expect("read the user setting"),
        Some(serde_json::json!("fr"))
    );
    assert!(
        PasswordCreationRepo::new(&target)
            .get_for_user(users[1].id)
            .await
            .expect("read the password key")
            .is_some(),
        "the password key moved"
    );

    let keys = ApiKeyRepo::new(&target)
        .list_for_owner(users[0].id)
        .await
        .expect("list keys");
    assert_eq!(keys.len(), 1);
    assert_eq!(keys[0].id, "key-abc");

    source.cleanup().await;
    remove_db(&path);
}

/// A unique temporary directory the caller removes.
fn temp_dir(tag: &str) -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let seq = NEXT.fetch_add(1, Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.subsec_nanos());
    let path = std::env::temp_dir().join(format!(
        "romeka-migrate-{}-{seq}-{nanos}-{tag}",
        std::process::id()
    ));
    std::fs::create_dir_all(&path).expect("create the temp directory");
    path
}

/// Fetch one public site page through the router and return its body.
async fn page_body(router: axum::Router, uri: &str) -> String {
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    let request = axum::http::Request::builder()
        .uri(uri)
        .body(axum::body::Body::empty())
        .expect("a request");
    let response = router.oneshot(request).await.expect("the router answers");
    assert_eq!(
        response.status(),
        axum::http::StatusCode::OK,
        "{uri} did not render"
    );
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("read the body")
        .to_bytes();
    String::from_utf8(bytes.to_vec()).expect("the body is UTF-8")
}

/// A migrated site renders its thumbnail, asset block, block background, media attachment and page
/// tree on `/s`, and the item pool lists the item the source held.
#[tokio::test]
async fn a_migrated_site_renders_its_assets_and_attachments() {
    let Some(source) = source().await else {
        return;
    };
    seed(&source.pool).await;

    let source_store = temp_dir("asset-source");
    std::fs::create_dir_all(source_store.join("asset")).expect("create the source store");
    std::fs::write(
        source_store.join(format!("asset/{ASSET_STORAGE_ID}.png")),
        b"logo bytes",
    )
    .expect("write the source asset file");
    let target_store = temp_dir("asset-target");

    let path = target_path("rendered-site");
    remove_db(&path);
    let options = MigrateOptions {
        source_files: Some(source_store.clone()),
        target_files: Some(target_store.clone()),
        ..MigrateOptions::default()
    };
    migrate_from_upstream(&with_database(&source.base, &source.name), &path, &options)
        .await
        .expect("the migration succeeds");

    assert_eq!(
        std::fs::read(target_store.join(format!("asset/{ASSET_STORAGE_ID}.png")))
            .expect("the asset file was copied"),
        b"logo bytes"
    );

    let target = omeka_db::open_file(&path).await.expect("open the target");
    let item = ItemRepo::new(&target)
        .list(&ItemFilter::default(), &Paging::default())
        .await
        .expect("list items")
        .remove(0);
    let base = omeka_api::jsonld::BaseUrl::parse("http://localhost").expect("a base URL");

    let brand = page_body(
        omeka_server::build_router(target.clone(), base.clone()),
        "/s/museum/page/brand",
    )
    .await;
    assert!(
        brand.contains(&format!(
            "<img src=\"http://localhost/files/asset/{ASSET_STORAGE_ID}.png\""
        )),
        "the asset block did not render the migrated asset: {brand}"
    );
    assert!(
        brand.contains("<a href=\"/s/museum/page/about\">"),
        "the asset block's page link was not remapped: {brand}"
    );
    assert!(brand.contains("Our mark"), "{brand}");

    let gallery = page_body(
        omeka_server::build_router(target.clone(), base.clone()),
        "/s/museum/page/gallery",
    )
    .await;
    assert!(
        gallery.contains(&format!("/files/square/{MEDIA_STORAGE_ID}.jpg")),
        "the media block did not render its attachment's thumbnail: {gallery}"
    );
    assert!(gallery.contains("A helmet"), "{gallery}");
    assert!(gallery.contains("Bronze helmet"), "{gallery}");

    let banner = page_body(
        omeka_server::build_router(target.clone(), base.clone()),
        "/s/museum/page/banner",
    )
    .await;
    assert!(
        banner.contains("has-background"),
        "the block lost its background: {banner}"
    );
    assert!(
        banner.contains(&format!("files\\2F asset\\2F {ASSET_STORAGE_ID}")),
        "the block background asset was not remapped: {banner}"
    );

    let contents = page_body(
        omeka_server::build_router(target.clone(), base.clone()),
        "/s/museum/page/contents",
    )
    .await;
    assert!(
        contents.contains("/s/museum/page/about"),
        "the list of pages page id was not remapped: {contents}"
    );

    let browse = page_body(omeka_server::build_router(target, base), "/s/museum/item").await;
    assert!(
        browse.contains(&format!("/s/museum/item/{}", item.id().0)),
        "the item pool did not list the migrated item: {browse}"
    );
    assert!(browse.contains("Bronze helmet"), "{browse}");
    assert!(
        browse.contains(&format!(
            "<img src=\"http://localhost/files/asset/{ASSET_STORAGE_ID}.png\""
        )),
        "the item's thumbnail did not name the migrated asset: {browse}"
    );

    source.cleanup().await;
    remove_db(&path);
    std::fs::remove_dir_all(source_store).expect("remove the source store");
    std::fs::remove_dir_all(target_store).expect("remove the target store");
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
        &MigrateOptions {
            dry_run: true,
            ..MigrateOptions::default()
        },
    )
    .await
    .expect("the dry run succeeds");
    assert_eq!(
        report.table("asset").map(|t| t.written),
        Some(1),
        "the dry run counts the asset"
    );

    assert!(report.dry_run);
    assert_eq!(report.table("item").map(|t| t.written), Some(1));
    assert!(!path.exists(), "the dry run never opened the target");

    source.cleanup().await;
    remove_db(&path);
}
