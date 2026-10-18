//! Shared test fixtures: a migrated in-memory database seeded through
//! `omeka-db`, and the `oneshot` request helper.
//!
//! Seed data mirrors the golden fixtures under `fixtures/api/`
//! (an admin user, one item with a title value, one item set, one upload media
//! with thumbnails) so a response can be read against the frozen shape.

#![allow(clippy::unwrap_used, clippy::expect_used, dead_code)]

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode};
use http_body_util::BodyExt;
use omeka_api::jsonld::BaseUrl;
use omeka_db::ids::{ClassId, ItemId, ItemSetId, MediaId, TemplateId, UserId, VocabularyId};
use omeka_db::repos::{
    ApiKeyRepo, ClassRepo, ItemRepo, ItemSetRepo, MediaRepo, PropertyRepo, ResourceRepo,
    SiteBlockAttachmentRepo, SiteRepo, TemplateRepo, UserRepo, ValueRepo, VocabularyRepo,
};
use omeka_db::rows::{
    NewApiKey, NewClass, NewItem, NewItemSet, NewMedia, NewProperty, NewResource, NewSite,
    NewSiteBlock, NewSiteBlockAttachment, NewSitePage, NewTemplate, NewTemplateProperty, NewUser,
    NewValue, NewVocabulary,
};
use omeka_db::{migrate, open_memory};
use omeka_media::store::FileStore;
use omeka_server::build_router;
use sqlx::SqlitePool;
use tower::ServiceExt;

/// A 32 character key identity from the `ApiKey::STRING_CHARLIST` alphabet.
pub const KEY_IDENTITY: &str = "0123456789abcdef0123456789abcdef";
/// The credential that verifies against the seeded bcrypt hash.
pub const KEY_CREDENTIAL: &str = "fedcba9876543210fedcba9876543210";

/// Ids the seeded rows carry, for test assertions.
pub struct Seed {
    pub user: UserId,
    pub vocabulary: VocabularyId,
    pub class: ClassId,
    pub template: TemplateId,
    pub item: ItemId,
    pub item_set: ItemSetId,
    pub media: MediaId,
    pub site: u64,
    pub annotation: u64,
}

/// One HTTP answer split into its parts.
pub struct Answer {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: String,
}

impl Answer {
    pub fn json(&self) -> serde_json::Value {
        serde_json::from_str(&self.body)
            .unwrap_or_else(|error| panic!("body is not JSON: {error}\n{}", self.body))
    }
}

/// Build a router over a migrated in-memory database with no rows.
pub async fn empty_router() -> Router {
    let pool = open_memory().await.unwrap();
    migrate(&pool).await.unwrap();
    router(pool)
}

/// Build a router over a migrated in-memory database seeded with the fixtures.
pub async fn seeded_router() -> (Router, Seed) {
    let (pool, seed) = seeded_pool().await;
    (router(pool), seed)
}

/// The seeded pool, for tests that add more rows after seeding.
pub async fn seeded_pool() -> (SqlitePool, Seed) {
    let pool = open_memory().await.unwrap();
    migrate(&pool).await.unwrap();
    let seed = seed(&pool).await;
    (pool, seed)
}

pub fn router(pool: SqlitePool) -> Router {
    build_router(pool, BaseUrl::parse("http://localhost").unwrap())
}

/// Build a router whose media deletes reach `store`.
pub fn router_with_store(pool: SqlitePool, store: FileStore) -> Router {
    omeka_server::build_router_with_store(pool, BaseUrl::parse("http://localhost").unwrap(), store)
}

/// Seed the default-install-like rows every read test asserts against.
async fn seed(pool: &SqlitePool) -> Seed {
    let user = UserRepo::new(pool)
        .create(&NewUser {
            email: "admin@example.com".to_string(),
            name: "Admin".to_string(),
            password_hash: None,
            role: "global_admin".to_string(),
            is_active: true,
            created: None,
        })
        .await
        .unwrap();
    ApiKeyRepo::new(pool)
        .create(&NewApiKey {
            id: KEY_IDENTITY.to_string(),
            owner_id: user.id,
            label: "Test key".to_string(),
            credential_hash: bcrypt::hash(KEY_CREDENTIAL, 4).unwrap(),
            last_ip: None,
            last_accessed: None,
            created: None,
        })
        .await
        .unwrap();

    let dcterms = VocabularyRepo::new(pool)
        .create(&NewVocabulary {
            owner_id: None,
            namespace_uri: "http://purl.org/dc/terms/".to_string(),
            prefix: "dcterms".to_string(),
            label: "Dublin Core".to_string(),
            comment: Some("DCMI Metadata Terms".to_string()),
        })
        .await
        .unwrap();
    let dctype = VocabularyRepo::new(pool)
        .create(&NewVocabulary {
            owner_id: None,
            namespace_uri: "http://purl.org/dc/dcmitype/".to_string(),
            prefix: "dctype".to_string(),
            label: "Dublin Core Type".to_string(),
            comment: None,
        })
        .await
        .unwrap();
    let title = PropertyRepo::new(pool)
        .create(&NewProperty {
            owner_id: None,
            vocabulary_id: dcterms.id,
            local_name: "title".to_string(),
            label: "Title".to_string(),
            comment: Some("A name given to the resource.".to_string()),
        })
        .await
        .unwrap();
    let image = ClassRepo::new(pool)
        .create(&NewClass {
            owner_id: None,
            vocabulary_id: dctype.id,
            local_name: "Image".to_string(),
            label: "Image".to_string(),
            comment: Some("An image.".to_string()),
        })
        .await
        .unwrap();
    let template = TemplateRepo::new(pool)
        .create(&NewTemplate {
            owner_id: Some(user.id),
            resource_class_id: Some(image.id),
            title_property_id: Some(title.id),
            description_property_id: None,
            label: "Picture".to_string(),
        })
        .await
        .unwrap();
    TemplateRepo::new(pool)
        .add_property(&NewTemplateProperty {
            resource_template_id: template.id,
            property_id: title.id,
            alternate_label: None,
            alternate_comment: None,
            position: Some(1),
            data_type: Some(vec!["literal".to_string()]),
            is_required: false,
            is_private: false,
            default_lang: None,
        })
        .await
        .unwrap();

    let item_set = ItemSetRepo::new(pool)
        .create(&NewItemSet {
            resource: NewResource {
                owner_id: Some(user.id),
                resource_class_id: None,
                resource_template_id: None,
                thumbnail_id: None,
                title: Some("Featured photographs".to_string()),
                is_public: true,
                created: None,
            },
            is_open: true,
        })
        .await
        .unwrap();

    let item = ItemRepo::new(pool)
        .create(&NewItem {
            resource: NewResource {
                owner_id: Some(user.id),
                resource_class_id: Some(image.id),
                resource_template_id: Some(template.id),
                thumbnail_id: None,
                title: Some("A picture of a cat".to_string()),
                is_public: true,
                created: None,
            },
            primary_media_id: None,
        })
        .await
        .unwrap();
    ValueRepo::new(pool)
        .create(&NewValue {
            resource_id: item.id().as_resource(),
            property_id: title.id,
            value_resource_id: None,
            value_annotation_id: None,
            r#type: "literal".to_string(),
            lang: None,
            value: Some("A picture of a cat".to_string()),
            uri: None,
            is_public: true,
        })
        .await
        .unwrap();
    ItemRepo::new(pool)
        .add_to_item_set(item.id(), item_set.id())
        .await
        .unwrap();
    ValueRepo::new(pool)
        .create(&NewValue {
            resource_id: item_set.id().as_resource(),
            property_id: title.id,
            value_resource_id: None,
            value_annotation_id: None,
            r#type: "literal".to_string(),
            lang: None,
            value: Some("Featured photographs".to_string()),
            uri: None,
            is_public: true,
        })
        .await
        .unwrap();

    let media = MediaRepo::new(pool)
        .create(&NewMedia {
            resource: NewResource {
                owner_id: Some(user.id),
                resource_class_id: None,
                resource_template_id: None,
                thumbnail_id: None,
                title: Some("A photo".to_string()),
                is_public: true,
                created: None,
            },
            item_id: item.id(),
            ingester: "upload".to_string(),
            renderer: "file".to_string(),
            data: None,
            source: Some("source.jpg".to_string()),
            media_type: Some("image/jpeg".to_string()),
            storage_id: Some("4b1e5b9e0f9f4f5a8f6b2c3d4e5f6a7b8c9d0e1f".to_string()),
            extension: Some("jpg".to_string()),
            sha256: Some("abc123".to_string()),
            size: Some(1234),
            has_original: true,
            has_thumbnails: true,
            position: Some(1),
            lang: None,
            alt_text: None,
        })
        .await
        .unwrap();
    let mut item_row = ItemRepo::new(pool).get(item.id()).await.unwrap().unwrap();
    item_row.primary_media_id = Some(media.id());
    ItemRepo::new(pool).update(&item_row).await.unwrap();
    ValueRepo::new(pool)
        .create(&NewValue {
            resource_id: media.id().as_resource(),
            property_id: title.id,
            value_resource_id: None,
            value_annotation_id: None,
            r#type: "literal".to_string(),
            lang: None,
            value: Some("A photo".to_string()),
            uri: None,
            is_public: true,
        })
        .await
        .unwrap();

    let site = SiteRepo::new(pool)
        .create(&NewSite {
            thumbnail_id: None,
            homepage_id: None,
            owner_id: Some(user.id),
            slug: "museum".to_string(),
            theme: "default".to_string(),
            title: "Museum".to_string(),
            summary: Some("A museum site.".to_string()),
            navigation: serde_json::json!([]),
            item_pool: serde_json::json!([]),
            is_public: true,
            assign_new_items: false,
            created: None,
        })
        .await
        .unwrap();

    let annotation = ResourceRepo::new(pool)
        .create_annotation(&NewResource {
            owner_id: Some(user.id),
            resource_class_id: None,
            resource_template_id: None,
            thumbnail_id: None,
            title: Some("Editorial note".to_string()),
            is_public: true,
            created: None,
        })
        .await
        .unwrap();
    ValueRepo::new(pool)
        .create(&NewValue {
            resource_id: annotation.id().as_resource(),
            property_id: title.id,
            value_resource_id: None,
            value_annotation_id: None,
            r#type: "literal".to_string(),
            lang: None,
            value: Some("Editorial note".to_string()),
            uri: None,
            is_public: true,
        })
        .await
        .unwrap();

    Seed {
        user: user.id,
        vocabulary: dcterms.id,
        class: image.id,
        template: template.id,
        item: item.id(),
        item_set: item_set.id(),
        media: media.id(),
        site: site.id.0,
        annotation: annotation.id().0,
    }
}

/// Rows for the resources `omeka-db` keeps as tables but no repository writes:
/// one asset, one job and one module.
///
/// The insert statements are here because there is no repository to call.
/// `api_read_gaps.rs` reads them back through the `/api` handlers.
pub struct SystemSeed {
    pub asset: u64,
    pub job: u64,
    pub module: &'static str,
}

/// Insert one asset, one job and one module, mirroring the values in
/// `fixtures/api/{asset,job,module}.json`.
pub async fn seed_system(pool: &SqlitePool, seed: &Seed) -> SystemSeed {
    let asset: i64 = sqlx::query_scalar(
        "INSERT INTO asset (owner_id, name, media_type, storage_id, extension, alt_text) \
         VALUES (?, ?, ?, ?, ?, ?) RETURNING id",
    )
    .bind(seed.user.0 as i64)
    .bind("omeka-logo.png")
    .bind("image/png")
    .bind("a1b2c3d4e5f60718293a4b5c6d7e8f9012345678")
    .bind("png")
    .bind(Option::<String>::None)
    .fetch_one(pool)
    .await
    .unwrap();
    let job: i64 = sqlx::query_scalar(
        "INSERT INTO job (owner_id, status, class, args, started, ended) \
         VALUES (?, ?, ?, ?, ?, ?) RETURNING id",
    )
    .bind(seed.user.0 as i64)
    .bind("completed")
    .bind("Omeka\\Job\\UpdateSiteItems")
    .bind(r#"{"action":"add","sites":[]}"#)
    .bind("2024-01-03 11:00:00")
    .bind("2024-01-03 11:00:05")
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO module (id, is_active, version) VALUES (?, ?, ?)")
        .bind("Common")
        .bind(1)
        .bind("3.4.0")
        .execute(pool)
        .await
        .unwrap();
    SystemSeed {
        asset: asset as u64,
        job: job as u64,
        module: "Common",
    }
}

/// The seeded site plus its pages and blocks, for the public site tests.
///
/// The site keeps no homepage, so `GET /s/museum` redirects to the first page
/// its navigation links (`Site::linkedPages`, `SiteRepresentation.php:175-190`).
/// `seed` is left untouched so `api_read.rs` keeps its exact totals.
pub async fn site_pool() -> (SqlitePool, Seed) {
    let (pool, seed) = seeded_pool().await;
    let site = omeka_db::ids::SiteId::from(seed.site);
    let repo = SiteRepo::new(&pool);

    let welcome = repo
        .add_page(&NewSitePage {
            site_id: site,
            slug: "welcome".to_string(),
            title: "Welcome".to_string(),
            is_public: true,
            ..NewSitePage::default()
        })
        .await
        .unwrap();
    for (position, layout, data) in [
        (1_i64, "pageTitle", serde_json::json!({})),
        (2, "html", serde_json::json!({"html": "<p>Body copy</p>"})),
        (3, "lineBreak", serde_json::json!({"break_type": "opaque"})),
    ] {
        repo.add_block(&NewSiteBlock {
            page_id: welcome.id,
            layout: layout.to_string(),
            data,
            layout_data: None,
            position,
        })
        .await
        .unwrap();
    }

    repo.add_page(&NewSitePage {
        site_id: site,
        slug: "notes".to_string(),
        title: "Notes".to_string(),
        is_public: false,
        ..NewSitePage::default()
    })
    .await
    .unwrap();

    let preview = repo
        .add_page(&NewSitePage {
            site_id: site,
            slug: "preview".to_string(),
            title: "Preview".to_string(),
            is_public: true,
            ..NewSitePage::default()
        })
        .await
        .unwrap();
    repo.add_block(&NewSiteBlock {
        page_id: preview.id,
        layout: "browsePreview".to_string(),
        data: serde_json::json!({"resource_type": "items", "heading": "Latest"}),
        layout_data: None,
        position: 1,
    })
    .await
    .unwrap();

    let markup = repo
        .add_page(&NewSitePage {
            site_id: site,
            slug: "markup".to_string(),
            title: "<script>alert('x')</script>".to_string(),
            is_public: true,
            ..NewSitePage::default()
        })
        .await
        .unwrap();
    repo.add_block(&NewSiteBlock {
        page_id: markup.id,
        layout: "pageTitle".to_string(),
        data: serde_json::json!({}),
        layout_data: None,
        position: 1,
    })
    .await
    .unwrap();

    // Link the welcome page so the home route redirects to it, as upstream's
    // `linkedPages()` reads the navigation tree (`SiteRepresentation.php:175-190`).
    let mut site_row = repo.get(site).await.unwrap().unwrap();
    site_row.navigation = serde_json::json!([
        {"type": "page", "data": {"id": welcome.id.0, "label": null}}
    ]);
    repo.update(&site_row).await.unwrap();

    // Attach the seeded item and item set to the site. The site browse sets
    // `site_id` on the query (`Site\ItemController::browseAction:22`), and
    // `item_site` and `site_item_set` are what that filter reads, so the browse
    // and its search results list the rows the site holds.
    sqlx::query("INSERT INTO item_site (item_id, site_id) VALUES (?, ?)")
        .bind(i64::try_from(seed.item.0).unwrap())
        .bind(i64::try_from(seed.site).unwrap())
        .execute(&pool)
        .await
        .unwrap();
    repo.add_item_set(
        site,
        omeka_db::ids::ItemSetId::from(seed.item_set.0),
        Some(1),
    )
    .await
    .unwrap();

    (pool, seed)
}

/// A 32 character key identity for the visibility scenario's owner.
pub const OWNER_KEY_IDENTITY: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa0";
/// The owner key's credential.
pub const OWNER_KEY_CREDENTIAL: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa1";
/// A 32 character key identity for the visibility scenario's site viewer.
pub const VIEWER_KEY_IDENTITY: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb0";
/// The viewer key's credential.
pub const VIEWER_KEY_CREDENTIAL: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb1";
/// The password both visibility users log in with.
pub const VISIBILITY_PASSWORD: &str = "visibility-pass-1";

/// The site-visibility scenario: a non-admin author who owns a public site and
/// a private site, a second user holding a viewer permission on both, and the
/// pages, item and media the matrix reads.
pub struct VisibilitySeed {
    pub owner: UserId,
    pub viewer: UserId,
    pub owner_email: &'static str,
    pub viewer_email: &'static str,
    pub public_site: u64,
    pub private_site: u64,
    pub public_page: u64,
    pub private_page: u64,
    pub attach_page: u64,
    pub private_site_public_page: u64,
    pub item: ItemId,
    pub media_public: MediaId,
    pub media_private: MediaId,
    pub private_item: ItemId,
}

/// Build a migrated in-memory database carrying the visibility scenario.
///
/// The owner is an `author`, so the owner rule is exercised apart from an
/// admin's `view-all`. The viewer holds a `viewer` site permission on both
/// sites. The item's media is one public and one private row, and the attach
/// page renders the item's media list.
pub async fn visibility_pool() -> (SqlitePool, VisibilitySeed) {
    let (pool, seed) = seeded_pool().await;
    let _ = &seed;

    let owner = UserRepo::new(&pool)
        .create(&NewUser {
            email: "vis-owner@example.com".to_string(),
            name: "Vis Owner".to_string(),
            password_hash: Some(bcrypt::hash(VISIBILITY_PASSWORD, 4).unwrap()),
            role: "author".to_string(),
            is_active: true,
            created: None,
        })
        .await
        .unwrap();
    let viewer = UserRepo::new(&pool)
        .create(&NewUser {
            email: "vis-viewer@example.com".to_string(),
            name: "Vis Viewer".to_string(),
            password_hash: Some(bcrypt::hash(VISIBILITY_PASSWORD, 4).unwrap()),
            role: "researcher".to_string(),
            is_active: true,
            created: None,
        })
        .await
        .unwrap();
    for (identity, credential, label, user) in [
        (
            OWNER_KEY_IDENTITY,
            OWNER_KEY_CREDENTIAL,
            "Owner key",
            owner.id,
        ),
        (
            VIEWER_KEY_IDENTITY,
            VIEWER_KEY_CREDENTIAL,
            "Viewer key",
            viewer.id,
        ),
    ] {
        ApiKeyRepo::new(&pool)
            .create(&NewApiKey {
                id: identity.to_string(),
                owner_id: user,
                label: label.to_string(),
                credential_hash: bcrypt::hash(credential, 4).unwrap(),
                last_ip: None,
                last_accessed: None,
                created: None,
            })
            .await
            .unwrap();
    }

    let repo = SiteRepo::new(&pool);
    let public_site = repo
        .create(&NewSite {
            owner_id: Some(owner.id),
            slug: "vis-pub".to_string(),
            theme: "default".to_string(),
            title: "Vis public".to_string(),
            is_public: true,
            ..NewSite::default()
        })
        .await
        .unwrap();
    let private_site = repo
        .create(&NewSite {
            owner_id: Some(owner.id),
            slug: "vis-priv".to_string(),
            theme: "default".to_string(),
            title: "Vis private".to_string(),
            is_public: false,
            ..NewSite::default()
        })
        .await
        .unwrap();
    for site in [public_site.id, private_site.id] {
        repo.set_permission(site, viewer.id, "viewer")
            .await
            .unwrap();
    }

    // Page ids: public, attach, private, private-site. The attach page precedes
    // the private page in id order but follows it in navigation, so the
    // pagination test tells the two orders apart.
    let public_page = add_page(
        &pool,
        public_site.id,
        "vis-pub-public",
        "Vis public page",
        true,
    )
    .await;
    let attach_page = add_page(&pool, public_site.id, "vis-attach", "Vis attach", true).await;
    let private_page = add_page(
        &pool,
        public_site.id,
        "vis-pub-private",
        "Vis private page",
        false,
    )
    .await;
    let private_site_public_page = add_page(
        &pool,
        private_site.id,
        "vis-priv-public",
        "Vis private site page",
        true,
    )
    .await;

    // Navigation: the public page, its private child, then the attach page. The
    // order fixes the expected prev/next sequence.
    let mut nav_site = repo.get(public_site.id).await.unwrap().unwrap();
    nav_site.navigation = serde_json::json!([
        {"type": "page", "data": {"id": public_page, "label": null},
         "links": [{"type": "page", "data": {"id": private_page, "label": null}, "links": []}]},
        {"type": "page", "data": {"id": attach_page, "label": null}, "links": []}
    ]);
    repo.update(&nav_site).await.unwrap();

    let item = ItemRepo::new(&pool)
        .create(&NewItem {
            resource: NewResource {
                owner_id: Some(owner.id),
                title: Some("Vis item".to_string()),
                is_public: true,
                ..NewResource::default()
            },
            primary_media_id: None,
        })
        .await
        .unwrap();
    // The private media is created first, so the item's first media is the
    // hidden one and the primary-media fallback is exercised.
    let media_private = add_media(
        &pool,
        item.id(),
        owner.id,
        "Vis media private",
        false,
        "ffffffff",
    )
    .await;
    let media_public = add_media(
        &pool,
        item.id(),
        owner.id,
        "Vis media public",
        true,
        "eeeeeeee",
    )
    .await;
    let private_item = ItemRepo::new(&pool)
        .create(&NewItem {
            resource: NewResource {
                owner_id: Some(owner.id),
                title: Some("Vis private item".to_string()),
                is_public: false,
                ..NewResource::default()
            },
            primary_media_id: None,
        })
        .await
        .unwrap();

    // The attach page renders the item's media list and a browse preview of the
    // site's items, so a private media title or thumbnail the item view carries
    // reaches the page.
    let block = repo
        .add_block(&NewSiteBlock {
            page_id: omeka_db::ids::PageId::from(attach_page),
            layout: "itemWithMetadata".to_string(),
            data: serde_json::json!({}),
            layout_data: None,
            position: 1,
        })
        .await
        .unwrap();
    SiteBlockAttachmentRepo::new(&pool)
        .create(&NewSiteBlockAttachment {
            block_id: block.id,
            item_id: Some(item.id()),
            media_id: None,
            caption: String::new(),
            position: 1,
        })
        .await
        .unwrap();
    // Put the item in the site so the browse preview block lists it, and its
    // primary-media thumbnail is the one the media visibility test reads.
    sqlx::query("INSERT INTO item_site (item_id, site_id) VALUES (?, ?)")
        .bind(i64::try_from(item.id().0).unwrap())
        .bind(i64::try_from(public_site.id.0).unwrap())
        .execute(&pool)
        .await
        .unwrap();
    repo.add_block(&NewSiteBlock {
        page_id: omeka_db::ids::PageId::from(attach_page),
        layout: "browsePreview".to_string(),
        data: serde_json::json!({"resource_type": "items"}),
        layout_data: None,
        position: 2,
    })
    .await
    .unwrap();

    (
        pool,
        VisibilitySeed {
            owner: owner.id,
            viewer: viewer.id,
            owner_email: "vis-owner@example.com",
            viewer_email: "vis-viewer@example.com",
            public_site: public_site.id.0,
            private_site: private_site.id.0,
            public_page,
            private_page,
            attach_page,
            private_site_public_page,
            item: item.id(),
            media_public,
            media_private,
            private_item: private_item.id(),
        },
    )
}

/// Add one page and return its id.
async fn add_page(
    pool: &SqlitePool,
    site: omeka_db::ids::SiteId,
    slug: &str,
    title: &str,
    is_public: bool,
) -> u64 {
    SiteRepo::new(pool)
        .add_page(&NewSitePage {
            site_id: site,
            slug: slug.to_string(),
            title: title.to_string(),
            is_public,
            ..NewSitePage::default()
        })
        .await
        .unwrap()
        .id
        .0
}

/// Add one media row carrying `title` as its display title and a stored
/// thumbnail under `storage`.
async fn add_media(
    pool: &SqlitePool,
    item: ItemId,
    owner: UserId,
    title: &str,
    is_public: bool,
    storage: &str,
) -> MediaId {
    MediaRepo::new(pool)
        .create(&NewMedia {
            resource: NewResource {
                owner_id: Some(owner),
                title: Some(title.to_string()),
                is_public,
                ..NewResource::default()
            },
            item_id: item,
            ingester: "upload".to_string(),
            renderer: "file".to_string(),
            data: None,
            source: Some(format!("{storage}.jpg")),
            media_type: Some("image/jpeg".to_string()),
            storage_id: Some(storage.to_string()),
            extension: Some("jpg".to_string()),
            sha256: Some(storage.to_string()),
            size: Some(1),
            has_original: true,
            has_thumbnails: true,
            position: None,
            lang: None,
            alt_text: None,
        })
        .await
        .unwrap()
        .id()
}

/// Send one request through the router.
pub async fn send(app: Router, request: Request<Body>) -> Answer {
    let response = app.oneshot(request).await.unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    Answer {
        status,
        headers,
        body: String::from_utf8(bytes.to_vec()).unwrap(),
    }
}

/// Build a GET request, appending the API key when `keyed` is set.
pub fn get(uri: &str, keyed: bool) -> Request<Body> {
    let uri = if keyed {
        append_key(uri)
    } else {
        uri.to_string()
    };
    request("GET", &uri)
}

pub fn request(method: &str, uri: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .body(Body::empty())
        .unwrap()
}

/// Build a request with a JSON body and the JSON content type.
pub fn json_request(method: &str, uri: &str, body: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

/// Append the seeded API key as `key_identity` and `key_credential`.
pub fn append_key(uri: &str) -> String {
    let separator = if uri.contains('?') { '&' } else { '?' };
    format!("{uri}{separator}key_identity={KEY_IDENTITY}&key_credential={KEY_CREDENTIAL}")
}

/// A GET carrying one user's API key pair.
pub fn get_keyed(uri: &str, identity: &str, credential: &str) -> Request<Body> {
    let separator = if uri.contains('?') { '&' } else { '?' };
    request(
        "GET",
        &format!("{uri}{separator}key_identity={identity}&key_credential={credential}"),
    )
}

/// A GET carrying a session cookie.
pub fn get_cookie(uri: &str, cookie: &str) -> Request<Body> {
    Request::builder()
        .method("GET")
        .uri(uri)
        .header("cookie", cookie)
        .body(Body::empty())
        .unwrap()
}

/// Log in through `POST /api/login` and return the `Cookie` header value the
/// answer sets.
pub async fn login_cookie(app: Router, email: &str, password: &str) -> String {
    let body = format!("{{\"email\":{email:?},\"password\":{password:?}}}");
    let answer = send(app, json_request("POST", "/api/login", &body)).await;
    assert_eq!(
        answer.status,
        StatusCode::OK,
        "login for {email} failed: {}",
        answer.body
    );
    header(&answer, "set-cookie")
        .expect("login answer sets a cookie")
        .split(';')
        .next()
        .unwrap()
        .to_string()
}

/// The header value as a string, or `None` when absent.
pub fn header<'a>(answer: &'a Answer, name: &str) -> Option<&'a str> {
    answer
        .headers
        .get(name)
        .and_then(|value| value.to_str().ok())
}

/// Assert the common JSON-LD envelope of one resource document.
pub fn assert_envelope(document: &serde_json::Value, id: u64) {
    assert_eq!(
        document.get("@context").and_then(|value| value.as_str()),
        Some("http://localhost/api-context"),
        "document has no @context: {document}"
    );
    assert_eq!(
        document.get("o:id").and_then(|value| value.as_u64()),
        Some(id),
        "document has wrong o:id: {document}"
    );
}
