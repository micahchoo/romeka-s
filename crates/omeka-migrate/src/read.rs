//! Reading the source: every table the port serves, through `omeka-db`'s MySQL repositories.
//!
//! The reads reuse the port's own row structs, which are the storage boundary for both engines, so a
//! MySQL datetime, JSON column or boolean decodes through the same code a SQLite read uses. Each
//! repository's `list` with a default filter returns the whole table in id order; a child table is
//! read once per parent, matching the repository's per-parent read methods.
//!
//! `module`, `job`, `session` and `fulltext_search` are not read; the README's known losses says
//! why. `value_annotation` is the one served table with no read path at all: `omeka-db` creates an
//! annotation but never lists one, so the source data cannot hold any.

use sqlx::MySql;
use sqlx::MySqlPool;

use omeka_db::ids::ResourceId;
use omeka_db::repos::{
    ApiKeyRepo, AssetFilter, AssetRepo, ClassFilter, ClassRepo, ItemFilter, ItemRepo,
    ItemSetFilter, ItemSetRepo, ItemSiteRepo, MediaFilter, MediaRepo, Paging, PasswordCreationRepo,
    PropertyFilter, PropertyRepo, SettingRepo, SiteBlockAttachmentRepo, SiteFilter, SiteRepo,
    SiteSettingRepo, TemplateRepo, UserFilter, UserRepo, UserSettingRepo, ValueRepo,
    VocabularyFilter, VocabularyRepo,
};
use omeka_db::rows::{
    ApiKeyRow, AssetRow, ClassRow, ItemItemSetRow, ItemRow, ItemSetRow, ItemSiteRow, MediaRow,
    PasswordCreationRow, PropertyRow, SettingRow, SiteBlockAttachmentRow, SiteBlockRow,
    SiteItemSetRow, SitePageRow, SitePermissionRow, SiteRow, SiteSettingRow, TemplatePropertyRow,
    TemplateRow, UserRow, UserSettingRow, ValueRow, VocabularyRow,
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
    pub(crate) assets: Vec<AssetRow>,
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
    pub(crate) block_attachments: Vec<SiteBlockAttachmentRow>,
    pub(crate) site_item_sets: Vec<SiteItemSetRow>,
    pub(crate) site_permissions: Vec<SitePermissionRow>,
    pub(crate) item_sites: Vec<ItemSiteRow>,
    pub(crate) settings: Vec<SettingRow>,
    pub(crate) site_settings: Vec<SiteSettingRow>,
    pub(crate) user_settings: Vec<UserSettingRow>,
    pub(crate) password_creations: Vec<PasswordCreationRow>,
    pub(crate) api_keys: Vec<ApiKeyRow>,
}

/// Read every table the port serves.
pub(crate) async fn read_source(source: &MySqlPool) -> Result<SourceData, MigrateError> {
    let (users, vocabularies, classes, properties, templates, template_properties) =
        read_catalog(source).await?;
    let assets = AssetRepo::<MySql>::new(source)
        .list(&AssetFilter::default(), &Paging::default())
        .await?;
    let (items, item_sets, media, values, item_item_sets) = read_resources(source).await?;
    let (sites, pages, blocks, block_attachments, site_item_sets, site_permissions) =
        read_sites(source).await?;
    let item_sites = ItemSiteRepo::<MySql>::new(source).list().await?;
    let settings = SettingRepo::<MySql>::new(source).list().await?;
    let mut site_settings = Vec::new();
    let mut user_settings = Vec::new();
    let mut password_creations = Vec::new();
    for site in &sites {
        site_settings.extend(SiteSettingRepo::<MySql>::new(source).list(site.id).await?);
    }
    for user in &users {
        user_settings.extend(UserSettingRepo::<MySql>::new(source).list(user.id).await?);
        if let Some(row) = PasswordCreationRepo::<MySql>::new(source)
            .get_for_user(user.id)
            .await?
        {
            password_creations.push(row);
        }
    }
    let api_keys = read_api_keys(source, &users).await?;

    Ok(SourceData {
        users,
        assets,
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
        block_attachments,
        site_item_sets,
        site_permissions,
        item_sites,
        settings,
        site_settings,
        user_settings,
        password_creations,
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
    let classes = ClassRepo::<MySql>::new(source)
        .list(&ClassFilter::default(), all)
        .await?;
    let properties = PropertyRepo::<MySql>::new(source)
        .list(&PropertyFilter::default(), all)
        .await?;

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
    Vec<SiteBlockAttachmentRow>,
    Vec<SiteItemSetRow>,
    Vec<SitePermissionRow>,
);

/// Sites, their pages, blocks and block attachments, their item set pool and their permissions.
async fn read_sites(source: &MySqlPool) -> Result<Sites, MigrateError> {
    let sites = SiteRepo::<MySql>::new(source)
        .list(&SiteFilter::default(), &Paging::default())
        .await?;

    let mut pages = Vec::new();
    for site in &sites {
        pages.extend(SiteRepo::<MySql>::new(source).list_pages(site.id).await?);
    }

    let mut blocks = Vec::new();
    let mut block_attachments = Vec::new();
    for page in &pages {
        let page_blocks = SiteRepo::<MySql>::new(source).list_blocks(page.id).await?;
        for block in &page_blocks {
            block_attachments.extend(
                SiteBlockAttachmentRepo::<MySql>::new(source)
                    .list_for_block(block.id)
                    .await?,
            );
        }
        blocks.extend(page_blocks);
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

    Ok((
        sites,
        pages,
        blocks,
        block_attachments,
        site_item_sets,
        site_permissions,
    ))
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
