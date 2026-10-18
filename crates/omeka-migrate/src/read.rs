//! Reading the source: every table the port serves, through `omeka-db`'s MySQL repositories.
//!
//! The reads reuse the port's own row structs, which are the storage boundary for both engines, so a
//! MySQL datetime, JSON column or boolean decodes through the same code a SQLite read uses. Each
//! repository's `list` with a default filter returns the whole table in id order; a child table is
//! read once per parent, matching the repository's per-parent read methods.
//!
//! Tables the port does not serve are not read: `setting`, `module`, `job`, `session`, `asset`,
//! `site_block_attachment`, `password_creation`, `user_setting`, `site_setting`, `item_site` and
//! `fulltext_search` have no repository, and `fulltext_search` is an index the port rebuilds.
//! `value_annotation` is the one served table with no read path: `omeka-db` creates an annotation
//! but never lists one, so the source data cannot hold any.

use sqlx::MySql;
use sqlx::MySqlPool;

use omeka_db::ids::ResourceId;
use omeka_db::repos::{
    ApiKeyRepo, ItemFilter, ItemRepo, ItemSetFilter, ItemSetRepo, MediaFilter, MediaRepo, Paging,
    SiteFilter, SiteRepo, TemplateRepo, UserFilter, UserRepo, ValueRepo, VocabularyFilter,
    VocabularyRepo,
};
use omeka_db::rows::{
    ApiKeyRow, ClassRow, ItemItemSetRow, ItemRow, ItemSetRow, MediaRow, PropertyRow, SiteBlockRow,
    SiteItemSetRow, SitePageRow, SitePermissionRow, SiteRow, TemplatePropertyRow, TemplateRow,
    UserRow, ValueRow, VocabularyRow,
};

use crate::error::MigrateError;

/// Everything a run reads from the source, before any write.
///
/// The whole source is held in memory. The read pass is separate from the write pass so a dry run
/// can report what a run would do without a target, and so a skip in one table never depends on a
/// write in another.
#[derive(Debug, Default)]
pub(crate) struct SourceData {
    pub(crate) users: Vec<UserRow>,
    pub(crate) vocabularies: Vec<VocabularyRow>,
    pub(crate) classes: Vec<ClassRow>,
    pub(crate) properties: Vec<PropertyRow>,
    pub(crate) templates: Vec<TemplateRow>,
    pub(crate) template_properties: Vec<TemplatePropertyRow>,
    pub(crate) items: Vec<ItemRow>,
    pub(crate) item_sets: Vec<ItemSetRow>,
    pub(crate) media: Vec<MediaRow>,
    pub(crate) values: Vec<ValueRow>,
    pub(crate) item_item_sets: Vec<ItemItemSetRow>,
    pub(crate) sites: Vec<SiteRow>,
    pub(crate) pages: Vec<SitePageRow>,
    pub(crate) blocks: Vec<SiteBlockRow>,
    pub(crate) site_item_sets: Vec<SiteItemSetRow>,
    pub(crate) site_permissions: Vec<SitePermissionRow>,
    pub(crate) api_keys: Vec<ApiKeyRow>,
}

/// Read every table the port serves.
pub(crate) async fn read_source(source: &MySqlPool) -> Result<SourceData, MigrateError> {
    let (users, vocabularies, classes, properties, templates, template_properties) =
        read_catalog(source).await?;
    let (items, item_sets, media, values, item_item_sets) = read_resources(source).await?;
    let (sites, pages, blocks, site_item_sets, site_permissions) = read_sites(source).await?;
    let api_keys = read_api_keys(source, &users).await?;

    Ok(SourceData {
        users,
        vocabularies,
        classes,
        properties,
        templates,
        template_properties,
        items,
        item_sets,
        media,
        values,
        item_item_sets,
        sites,
        pages,
        blocks,
        site_item_sets,
        site_permissions,
        api_keys,
    })
}

type Catalog = (
    Vec<UserRow>,
    Vec<VocabularyRow>,
    Vec<ClassRow>,
    Vec<PropertyRow>,
    Vec<TemplateRow>,
    Vec<TemplatePropertyRow>,
);

/// Users, vocabularies, classes, properties and templates, in dependency order.
async fn read_catalog(source: &MySqlPool) -> Result<Catalog, MigrateError> {
    let all = &Paging::default();
    let users = UserRepo::<MySql>::new(source)
        .list(&UserFilter::default(), all)
        .await?;
    let vocabularies = VocabularyRepo::<MySql>::new(source)
        .list(&VocabularyFilter::default(), all)
        .await?;
    let classes = read_classes(source).await?;
    let properties = read_properties(source).await?;

    let templates = TemplateRepo::<MySql>::new(source).list().await?;
    let mut template_properties = Vec::new();
    for template in &templates {
        template_properties.extend(
            TemplateRepo::<MySql>::new(source)
                .list_properties(template.id)
                .await?,
        );
    }

    Ok((
        users,
        vocabularies,
        classes,
        properties,
        templates,
        template_properties,
    ))
}

/// `resource_class` and `property` carry `local_name` in `utf8mb4_bin`, and sqlx reports a
/// binary-collated column to the row layer as `VARBINARY`. `omeka-db`'s MySQL text reader decodes a
/// `String`, so `ClassRepo::list` and `PropertyRepo::list` fail on those two tables against a real
/// server. Until `omeka-db` decodes a binary-collated text column, this crate reads the two tables
/// itself with `CAST(local_name AS CHAR)`, which reports the column as the text it holds, and parses
/// each row through the same `from_db_row` the repositories use. Reported to the coordinator as an
/// `omeka-db` fix.
const CLASS_COLUMNS: &str = "SELECT id, owner_id, vocabulary_id,
         CAST(local_name AS CHAR) AS local_name, label, comment
     FROM resource_class ORDER BY id";

const PROPERTY_COLUMNS: &str = "SELECT id, owner_id, vocabulary_id,
         CAST(local_name AS CHAR) AS local_name, label, comment
     FROM property ORDER BY id";

async fn read_classes(source: &MySqlPool) -> Result<Vec<ClassRow>, MigrateError> {
    let rows = sqlx::query(CLASS_COLUMNS).fetch_all(source).await?;
    let mut classes = Vec::with_capacity(rows.len());
    for row in &rows {
        classes.push(ClassRow::from_db_row::<MySql>(row)?);
    }
    Ok(classes)
}

async fn read_properties(source: &MySqlPool) -> Result<Vec<PropertyRow>, MigrateError> {
    let rows = sqlx::query(PROPERTY_COLUMNS).fetch_all(source).await?;
    let mut properties = Vec::with_capacity(rows.len());
    for row in &rows {
        properties.push(PropertyRow::from_db_row::<MySql>(row)?);
    }
    Ok(properties)
}

type Resources = (
    Vec<ItemRow>,
    Vec<ItemSetRow>,
    Vec<MediaRow>,
    Vec<ValueRow>,
    Vec<ItemItemSetRow>,
);

/// Items, item sets and media, their values, and their item set memberships.
async fn read_resources(source: &MySqlPool) -> Result<Resources, MigrateError> {
    let all = &Paging::default();
    let items = ItemRepo::<MySql>::new(source)
        .list(&ItemFilter::default(), all)
        .await?;
    let item_sets = ItemSetRepo::<MySql>::new(source)
        .list(&ItemSetFilter::default(), all)
        .await?;
    let media = MediaRepo::<MySql>::new(source)
        .list(&MediaFilter::default(), all)
        .await?;

    // A value hangs off a resource; the resources are the items, item sets and media just read.
    let resource_ids: Vec<ResourceId> = items
        .iter()
        .map(|row| row.resource.id)
        .chain(item_sets.iter().map(|row| row.resource.id))
        .chain(media.iter().map(|row| row.resource.id))
        .collect();
    let mut values = Vec::new();
    for resource_id in resource_ids {
        values.extend(
            ValueRepo::<MySql>::new(source)
                .list_for_resource(resource_id)
                .await?,
        );
    }

    let mut item_item_sets = Vec::new();
    for item in &items {
        for item_set_id in ItemRepo::<MySql>::new(source)
            .list_item_sets(item.id())
            .await?
        {
            item_item_sets.push(ItemItemSetRow {
                item_id: item.id(),
                item_set_id,
            });
        }
    }

    Ok((items, item_sets, media, values, item_item_sets))
}

type Sites = (
    Vec<SiteRow>,
    Vec<SitePageRow>,
    Vec<SiteBlockRow>,
    Vec<SiteItemSetRow>,
    Vec<SitePermissionRow>,
);

/// Sites, their pages and blocks, their item set pool and their permissions.
async fn read_sites(source: &MySqlPool) -> Result<Sites, MigrateError> {
    let sites = SiteRepo::<MySql>::new(source)
        .list(&SiteFilter::default(), &Paging::default())
        .await?;

    let mut pages = Vec::new();
    for site in &sites {
        pages.extend(SiteRepo::<MySql>::new(source).list_pages(site.id).await?);
    }

    let mut blocks = Vec::new();
    for page in &pages {
        blocks.extend(SiteRepo::<MySql>::new(source).list_blocks(page.id).await?);
    }

    let mut site_item_sets = Vec::new();
    let mut site_permissions = Vec::new();
    for site in &sites {
        site_item_sets.extend(
            SiteRepo::<MySql>::new(source)
                .list_item_sets(site.id)
                .await?,
        );
        site_permissions.extend(
            SiteRepo::<MySql>::new(source)
                .list_permissions(site.id)
                .await?,
        );
    }

    Ok((sites, pages, blocks, site_item_sets, site_permissions))
}

/// Every API key, read once per owner.
async fn read_api_keys(
    source: &MySqlPool,
    users: &[UserRow],
) -> Result<Vec<ApiKeyRow>, MigrateError> {
    let mut keys = Vec::new();
    for user in users {
        keys.extend(
            ApiKeyRepo::<MySql>::new(source)
                .list_for_owner(user.id)
                .await?,
        );
    }
    Ok(keys)
}
