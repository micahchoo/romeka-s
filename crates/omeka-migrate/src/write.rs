//! Writing the target: the port's repositories, in dependency order, with source ids remapped.
//!
//! `omeka-db` assigns the target its own ids, so the writer carries an [`IdMap`] from source id to
//! target id and rewrites every reference. A run without a target (a dry run) fills the map with
//! the source id itself, so the same code decides the same skips and counts the same rows without
//! writing.
//!
//! Two references point forward and are set after their table is written: `item.primary_media_id`
//! after the media pass, and `site.homepage_id` plus the page ids inside `site.navigation` after the
//! page pass.

use std::path::Path;

use sqlx::SqlitePool;

use omeka_db::ids::MediaId;
use omeka_db::repos::{
    ApiKeyRepo, AssetRepo, ClassRepo, ItemFilter, ItemRepo, ItemSetRepo, MediaRepo, Paging,
    PasswordCreationRepo, PropertyRepo, SettingRepo, SiteBlockAttachmentRepo, SiteRepo,
    SiteSettingRepo, TemplateRepo, UserFilter, UserRepo, UserSettingRepo, ValueRepo,
    VocabularyRepo,
};
use omeka_db::rows::{
    ApiKeyRow, AssetRow, ClassRow, ItemItemSetRow, ItemRow, ItemSetRow, ItemSiteRow, MediaRow,
    PasswordCreationRow, PropertyRow, SettingRow, SiteBlockAttachmentRow, SiteBlockRow,
    SiteItemSetRow, SitePageRow, SitePermissionRow, SiteRow, SiteSettingRow, TemplatePropertyRow,
    TemplateRow, UserRow, UserSettingRow, ValueRow, VocabularyRow,
};

use crate::MigrationReport;
use crate::error::MigrateError;
use crate::map::{
    IdMap, new_api_key, new_asset, new_class, new_item, new_item_set, new_media,
    new_password_creation, new_property, new_site, new_site_block, new_site_block_attachment,
    new_site_page, new_template, new_template_property, new_user, new_value, new_vocabulary,
    remap_navigation,
};
use crate::read::SourceData;
use crate::report::Ledger;

/// Write a whole source into a target, or count what a run would write.
pub(crate) struct Writer<'a> {
    target: Option<&'a SqlitePool>,
    ids: IdMap,
    ledger: Ledger,
    /// Items whose source `primary_media_id` is set: the target row and the source media id.
    items_with_primary_media: Vec<(ItemRow, MediaId)>,
    /// Sites whose homepage or navigation must be rewritten: the target row and the source row.
    sites_to_finish: Vec<(SiteRow, SiteRow)>,
}

impl<'a> Writer<'a> {
    /// A writer that only counts: no target, so the id map stays the identity.
    pub(crate) fn dry() -> Self {
        Self::with_target(None)
    }

    /// A writer over a migrated, empty target.
    pub(crate) fn live(target: &'a SqlitePool) -> Self {
        Self::with_target(Some(target))
    }

    fn with_target(target: Option<&'a SqlitePool>) -> Self {
        Self {
            target,
            ids: IdMap::default(),
            ledger: Ledger::default(),
            items_with_primary_media: Vec::new(),
            sites_to_finish: Vec::new(),
        }
    }

    /// Write every table in dependency order and return the report.
    pub(crate) async fn run(mut self, data: &SourceData) -> Result<MigrationReport, MigrateError> {
        self.users(&data.users).await?;
        self.assets(&data.assets).await?;
        self.password_creations(&data.password_creations).await?;
        self.vocabularies(&data.vocabularies).await?;
        self.classes(&data.classes).await?;
        self.properties(&data.properties).await?;
        self.templates(&data.templates).await?;
        self.template_properties(&data.template_properties).await?;
        self.items(&data.items).await?;
        self.item_sets(&data.item_sets).await?;
        self.media(&data.media).await?;
        self.values(&data.values).await?;
        self.item_item_sets(&data.item_item_sets).await?;
        self.sites(&data.sites).await?;
        self.pages(&data.pages).await?;
        self.blocks(&data.blocks).await?;
        self.site_block_attachments(&data.block_attachments).await?;
        self.site_item_sets(&data.site_item_sets).await?;
        self.site_permissions(&data.site_permissions).await?;
        self.item_sites(&data.item_sites).await?;
        self.settings(&data.settings).await?;
        self.site_settings(&data.site_settings).await?;
        self.user_settings(&data.user_settings).await?;
        self.api_keys(&data.api_keys).await?;
        self.finish_items().await?;
        self.finish_sites().await?;
        let dry_run = self.target.is_none();
        Ok(self.ledger.into_report(dry_run))
    }

    async fn users(&mut self, rows: &[UserRow]) -> Result<(), MigrateError> {
        self.ledger.read("user", rows.len() as u64);
        for row in rows {
            let new = new_user(row);
            let target_id = match self.target {
                Some(target) => UserRepo::new(target).create(&new).await?.id,
                None => row.id,
            };
            self.ids.insert_user(row.id.0, target_id);
            self.ledger.written("user");
        }
        Ok(())
    }

    async fn assets(&mut self, rows: &[AssetRow]) -> Result<(), MigrateError> {
        self.ledger.read("asset", rows.len() as u64);
        for row in rows {
            let Some(new) = new_asset(row, &self.ids) else {
                self.ledger.skipped("asset");
                continue;
            };
            let target_id = match self.target {
                Some(target) => AssetRepo::new(target).create(&new).await?.id,
                None => row.id,
            };
            self.ids.insert_asset(row.id.0, target_id);
            self.ledger.written("asset");
        }
        Ok(())
    }

    async fn password_creations(
        &mut self,
        rows: &[PasswordCreationRow],
    ) -> Result<(), MigrateError> {
        self.ledger.read("password_creation", rows.len() as u64);
        for row in rows {
            let Some(new) = new_password_creation(row, &self.ids) else {
                self.ledger.skipped("password_creation");
                continue;
            };
            if let Some(target) = self.target {
                PasswordCreationRepo::new(target).create(&new).await?;
            }
            self.ledger.written("password_creation");
        }
        Ok(())
    }

    async fn vocabularies(&mut self, rows: &[VocabularyRow]) -> Result<(), MigrateError> {
        self.ledger.read("vocabulary", rows.len() as u64);
        for row in rows {
            let Some(new) = new_vocabulary(row, &self.ids) else {
                self.ledger.skipped("vocabulary");
                continue;
            };
            let target_id = match self.target {
                Some(target) => VocabularyRepo::new(target).create(&new).await?.id,
                None => row.id,
            };
            self.ids.insert_vocabulary(row.id.0, target_id);
            self.ledger.written("vocabulary");
        }
        Ok(())
    }

    async fn classes(&mut self, rows: &[ClassRow]) -> Result<(), MigrateError> {
        self.ledger.read("resource_class", rows.len() as u64);
        for row in rows {
            let Some(new) = new_class(row, &self.ids) else {
                self.ledger.skipped("resource_class");
                continue;
            };
            let target_id = match self.target {
                Some(target) => ClassRepo::new(target).create(&new).await?.id,
                None => row.id,
            };
            self.ids.insert_class(row.id.0, target_id);
            self.ledger.written("resource_class");
        }
        Ok(())
    }

    async fn properties(&mut self, rows: &[PropertyRow]) -> Result<(), MigrateError> {
        self.ledger.read("property", rows.len() as u64);
        for row in rows {
            let Some(new) = new_property(row, &self.ids) else {
                self.ledger.skipped("property");
                continue;
            };
            let target_id = match self.target {
                Some(target) => PropertyRepo::new(target).create(&new).await?.id,
                None => row.id,
            };
            self.ids.insert_property(row.id.0, target_id);
            self.ledger.written("property");
        }
        Ok(())
    }

    async fn templates(&mut self, rows: &[TemplateRow]) -> Result<(), MigrateError> {
        self.ledger.read("resource_template", rows.len() as u64);
        for row in rows {
            let Some(new) = new_template(row, &self.ids) else {
                self.ledger.skipped("resource_template");
                continue;
            };
            let target_id = match self.target {
                Some(target) => TemplateRepo::new(target).create(&new).await?.id,
                None => row.id,
            };
            self.ids.insert_template(row.id.0, target_id);
            self.ledger.written("resource_template");
        }
        Ok(())
    }

    async fn template_properties(
        &mut self,
        rows: &[TemplatePropertyRow],
    ) -> Result<(), MigrateError> {
        self.ledger
            .read("resource_template_property", rows.len() as u64);
        for row in rows {
            let Some(new) = new_template_property(row, &self.ids) else {
                self.ledger.skipped("resource_template_property");
                continue;
            };
            if let Some(target) = self.target {
                TemplateRepo::new(target).add_property(&new).await?;
            }
            self.ledger.written("resource_template_property");
        }
        Ok(())
    }

    async fn items(&mut self, rows: &[ItemRow]) -> Result<(), MigrateError> {
        self.ledger.read("item", rows.len() as u64);
        for row in rows {
            let Some(new) = new_item(row, &self.ids) else {
                self.ledger.skipped("item");
                continue;
            };
            match self.target {
                Some(target) => {
                    let created = ItemRepo::new(target).create(&new).await?;
                    self.ids
                        .insert_resource(row.resource.id.0, created.resource.id);
                    self.ids.insert_item(row.id().0, created.id());
                    if let Some(source_media) = row.primary_media_id {
                        self.items_with_primary_media.push((created, source_media));
                    }
                }
                None => {
                    self.ids.insert_resource(row.resource.id.0, row.resource.id);
                    self.ids.insert_item(row.id().0, row.id());
                }
            }
            self.ledger.written("item");
        }
        Ok(())
    }

    async fn item_sets(&mut self, rows: &[ItemSetRow]) -> Result<(), MigrateError> {
        self.ledger.read("item_set", rows.len() as u64);
        for row in rows {
            let Some(new) = new_item_set(row, &self.ids) else {
                self.ledger.skipped("item_set");
                continue;
            };
            match self.target {
                Some(target) => {
                    let created = ItemSetRepo::new(target).create(&new).await?;
                    self.ids
                        .insert_resource(row.resource.id.0, created.resource.id);
                    self.ids.insert_item_set(row.id().0, created.id());
                }
                None => {
                    self.ids.insert_resource(row.resource.id.0, row.resource.id);
                    self.ids.insert_item_set(row.id().0, row.id());
                }
            }
            self.ledger.written("item_set");
        }
        Ok(())
    }

    async fn media(&mut self, rows: &[MediaRow]) -> Result<(), MigrateError> {
        self.ledger.read("media", rows.len() as u64);
        for row in rows {
            let Some(new) = new_media(row, &self.ids) else {
                self.ledger.skipped("media");
                continue;
            };
            match self.target {
                Some(target) => {
                    let created = MediaRepo::new(target).create(&new).await?;
                    self.ids
                        .insert_resource(row.resource.id.0, created.resource.id);
                    self.ids.insert_media(row.id().0, created.id());
                }
                None => {
                    self.ids.insert_resource(row.resource.id.0, row.resource.id);
                    self.ids.insert_media(row.id().0, row.id());
                }
            }
            self.ledger.written("media");
        }
        Ok(())
    }

    async fn values(&mut self, rows: &[ValueRow]) -> Result<(), MigrateError> {
        self.ledger.read("value", rows.len() as u64);
        for row in rows {
            let Some(new) = new_value(row, &self.ids) else {
                self.ledger.skipped("value");
                continue;
            };
            if let Some(target) = self.target {
                ValueRepo::new(target).create(&new).await?;
            }
            self.ledger.written("value");
        }
        Ok(())
    }

    async fn item_item_sets(&mut self, rows: &[ItemItemSetRow]) -> Result<(), MigrateError> {
        self.ledger.read("item_item_set", rows.len() as u64);
        for row in rows {
            let (Some(item), Some(item_set)) = (
                self.ids.item(row.item_id.0),
                self.ids.item_set(row.item_set_id.0),
            ) else {
                self.ledger.skipped("item_item_set");
                continue;
            };
            if let Some(target) = self.target {
                ItemRepo::new(target)
                    .add_to_item_set(item, item_set)
                    .await?;
            }
            self.ledger.written("item_item_set");
        }
        Ok(())
    }

    async fn sites(&mut self, rows: &[SiteRow]) -> Result<(), MigrateError> {
        self.ledger.read("site", rows.len() as u64);
        for row in rows {
            let Some(new) = new_site(row, &self.ids) else {
                self.ledger.skipped("site");
                continue;
            };
            match self.target {
                Some(target) => {
                    let created = SiteRepo::new(target).create(&new).await?;
                    self.ids.insert_site(row.id.0, created.id);
                    self.sites_to_finish.push((created, row.clone()));
                }
                None => {
                    self.ids.insert_site(row.id.0, row.id);
                }
            }
            self.ledger.written("site");
        }
        Ok(())
    }

    async fn pages(&mut self, rows: &[SitePageRow]) -> Result<(), MigrateError> {
        self.ledger.read("site_page", rows.len() as u64);
        for row in rows {
            let Some(new) = new_site_page(row, &self.ids) else {
                self.ledger.skipped("site_page");
                continue;
            };
            let target_id = match self.target {
                Some(target) => SiteRepo::new(target).add_page(&new).await?.id,
                None => row.id,
            };
            self.ids.insert_page(row.id.0, target_id);
            self.ledger.written("site_page");
        }
        Ok(())
    }

    async fn blocks(&mut self, rows: &[SiteBlockRow]) -> Result<(), MigrateError> {
        self.ledger.read("site_page_block", rows.len() as u64);
        for row in rows {
            let Some(new) = new_site_block(row, &self.ids) else {
                self.ledger.skipped("site_page_block");
                continue;
            };
            let target_id = match self.target {
                Some(target) => SiteRepo::new(target).add_block(&new).await?.id,
                None => row.id,
            };
            self.ids.insert_block(row.id.0, target_id);
            self.ledger.written("site_page_block");
        }
        Ok(())
    }

    async fn site_block_attachments(
        &mut self,
        rows: &[SiteBlockAttachmentRow],
    ) -> Result<(), MigrateError> {
        self.ledger.read("site_block_attachment", rows.len() as u64);
        for row in rows {
            let Some(new) = new_site_block_attachment(row, &self.ids) else {
                self.ledger.skipped("site_block_attachment");
                continue;
            };
            if let Some(target) = self.target {
                SiteBlockAttachmentRepo::new(target).create(&new).await?;
            }
            self.ledger.written("site_block_attachment");
        }
        Ok(())
    }

    async fn site_item_sets(&mut self, rows: &[SiteItemSetRow]) -> Result<(), MigrateError> {
        self.ledger.read("site_item_set", rows.len() as u64);
        for row in rows {
            let (Some(site), Some(item_set)) = (
                self.ids.site(row.site_id.0),
                self.ids.item_set(row.item_set_id.0),
            ) else {
                self.ledger.skipped("site_item_set");
                continue;
            };
            if let Some(target) = self.target {
                SiteRepo::new(target)
                    .add_item_set(site, item_set, row.position)
                    .await?;
            }
            self.ledger.written("site_item_set");
        }
        Ok(())
    }

    async fn site_permissions(&mut self, rows: &[SitePermissionRow]) -> Result<(), MigrateError> {
        self.ledger.read("site_permission", rows.len() as u64);
        for row in rows {
            let (Some(site), Some(user)) =
                (self.ids.site(row.site_id.0), self.ids.user(row.user_id.0))
            else {
                self.ledger.skipped("site_permission");
                continue;
            };
            if let Some(target) = self.target {
                SiteRepo::new(target)
                    .set_permission(site, user, &row.role)
                    .await?;
            }
            self.ledger.written("site_permission");
        }
        Ok(())
    }

    /// `item_site` has no `omeka-db` repository, so this is the one write the crate makes itself.
    async fn item_sites(&mut self, rows: &[ItemSiteRow]) -> Result<(), MigrateError> {
        self.ledger.read("item_site", rows.len() as u64);
        for row in rows {
            let (Some(item), Some(site)) =
                (self.ids.item(row.item_id.0), self.ids.site(row.site_id.0))
            else {
                self.ledger.skipped("item_site");
                continue;
            };
            if let Some(target) = self.target {
                sqlx::query("INSERT INTO item_site (item_id, site_id) VALUES (?, ?)")
                    .bind(item.0 as i64)
                    .bind(site.0 as i64)
                    .execute(target)
                    .await?;
            }
            self.ledger.written("item_site");
        }
        Ok(())
    }

    async fn settings(&mut self, rows: &[SettingRow]) -> Result<(), MigrateError> {
        self.ledger.read("setting", rows.len() as u64);
        for row in rows {
            if let Some(target) = self.target {
                SettingRepo::new(target).set(&row.id, &row.value).await?;
            }
            self.ledger.written("setting");
        }
        Ok(())
    }

    async fn site_settings(&mut self, rows: &[SiteSettingRow]) -> Result<(), MigrateError> {
        self.ledger.read("site_setting", rows.len() as u64);
        for row in rows {
            let Some(site) = self.ids.site(row.site_id.0) else {
                self.ledger.skipped("site_setting");
                continue;
            };
            if let Some(target) = self.target {
                SiteSettingRepo::new(target)
                    .set(site, &row.id, &row.value)
                    .await?;
            }
            self.ledger.written("site_setting");
        }
        Ok(())
    }

    async fn user_settings(&mut self, rows: &[UserSettingRow]) -> Result<(), MigrateError> {
        self.ledger.read("user_setting", rows.len() as u64);
        for row in rows {
            let Some(user) = self.ids.user(row.user_id.0) else {
                self.ledger.skipped("user_setting");
                continue;
            };
            if let Some(target) = self.target {
                UserSettingRepo::new(target)
                    .set(user, &row.id, &row.value)
                    .await?;
            }
            self.ledger.written("user_setting");
        }
        Ok(())
    }

    async fn api_keys(&mut self, rows: &[ApiKeyRow]) -> Result<(), MigrateError> {
        self.ledger.read("api_key", rows.len() as u64);
        for row in rows {
            let Some(new) = new_api_key(row, &self.ids) else {
                self.ledger.skipped("api_key");
                continue;
            };
            if let Some(target) = self.target {
                ApiKeyRepo::new(target).create(&new).await?;
            }
            self.ledger.written("api_key");
        }
        Ok(())
    }

    /// Set each item's primary media now that the media rows exist.
    async fn finish_items(&mut self) -> Result<(), MigrateError> {
        let Some(target) = self.target else {
            return Ok(());
        };
        for (row, source_media) in &self.items_with_primary_media {
            let Some(media) = self.ids.media(source_media.0) else {
                continue;
            };
            let mut updated = row.clone();
            updated.primary_media_id = Some(media);
            ItemRepo::new(target).update(&updated).await?;
        }
        Ok(())
    }

    /// Set each site's homepage and rewrite its navigation now that the page ids exist.
    async fn finish_sites(&mut self) -> Result<(), MigrateError> {
        let Some(target) = self.target else {
            return Ok(());
        };
        for (row, source) in &self.sites_to_finish {
            let mut updated = row.clone();
            updated.navigation = remap_navigation(&source.navigation, &self.ids);
            updated.homepage_id = source.homepage_id.and_then(|id| self.ids.page(id.0));
            SiteRepo::new(target).update(&updated).await?;
        }
        Ok(())
    }
}

/// Refuse a target that already holds rows, so a run never merges into an existing database.
pub(crate) async fn ensure_empty(target: &SqlitePool, path: &Path) -> Result<(), MigrateError> {
    let users = UserRepo::new(target)
        .list(&UserFilter::default(), &Paging::new(1, 0))
        .await?;
    if !users.is_empty() {
        return Err(MigrateError::TargetNotEmpty {
            path: path.to_path_buf(),
            table: "user",
        });
    }
    let resources = ItemRepo::new(target)
        .list(&ItemFilter::default(), &Paging::new(1, 0))
        .await?;
    if !resources.is_empty() {
        return Err(MigrateError::TargetNotEmpty {
            path: path.to_path_buf(),
            table: "resource",
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use omeka_db::ids::{
        AnnotationId, AssetId, AttachmentId, BlockId, ItemId, ItemSetId, PageId, PropertyId,
        ResourceId, SiteId, SiteItemSetId, SitePermissionId, UserId, ValueId, VocabularyId,
    };
    use omeka_db::repos::{
        AssetFilter, MediaFilter, SettingRepo, SiteSettingRepo, UserSettingRepo,
    };
    use omeka_db::rows::{
        AssetRow, ItemItemSetRow, ItemSetRow, ItemSiteRow, PasswordCreationRow, SettingRow,
        SiteBlockAttachmentRow, SiteItemSetRow, SitePermissionRow, SiteSettingRow, UserSettingRow,
    };
    use serde_json::json;
    use sqlx::types::chrono::NaiveDateTime;

    fn at(text: &str) -> NaiveDateTime {
        // The literals read as upstream's stored form (`YYYY-MM-DD HH:MM:SS`); chrono's own parser
        // wants the ISO `T` between the date and the time.
        text.replacen(' ', "T", 1)
            .parse()
            .expect("a datetime literal")
    }

    fn resource(id: u64, owner: u64, title: &str) -> omeka_db::rows::ResourceRow {
        omeka_db::rows::ResourceRow {
            id: ResourceId(id),
            owner_id: Some(UserId(owner)),
            resource_class_id: None,
            resource_template_id: None,
            thumbnail_id: None,
            title: Some(title.to_string()),
            is_public: true,
            created: at("2024-01-02 03:04:05"),
            modified: None,
            resource_type: "Omeka\\Entity\\Item".to_string(),
        }
    }

    fn user(id: u64, name: &str) -> UserRow {
        UserRow {
            id: UserId(id),
            email: format!("{name}@example.org"),
            name: name.to_string(),
            created: at("2024-01-01 00:00:00"),
            modified: None,
            password_hash: None,
            role: "global_admin".to_string(),
            is_active: true,
        }
    }

    /// An asset row with a stored file, owned by the user in `sample`.
    fn asset(id: u64, storage_id: &str) -> AssetRow {
        AssetRow {
            id: AssetId(id),
            owner_id: Some(UserId(2)),
            name: "logo.png".to_string(),
            media_type: "image/png".to_string(),
            storage_id: storage_id.to_string(),
            extension: Some("png".to_string()),
            alt_text: None,
        }
    }

    /// A source with gaps in every id sequence, so a target that renumbers exposes every reference.
    fn sample() -> SourceData {
        let mut helmet_resource = resource(7, 2, "Bronze helmet");
        helmet_resource.thumbnail_id = Some(AssetId(77));
        let mut site_row = SiteRow {
            id: SiteId(30),
            thumbnail_id: Some(AssetId(77)),
            homepage_id: Some(PageId(40)),
            owner_id: Some(UserId(2)),
            slug: "museum".to_string(),
            theme: "default".to_string(),
            title: "The Museum".to_string(),
            summary: None,
            navigation: json!([
                {"type": "page", "links": [], "data": {"id": 40, "label": null}},
                {"type": "browse", "data": {"query": "helmets"}}
            ]),
            item_pool: json!([]),
            created: at("2024-01-03 00:00:00"),
            modified: None,
            is_public: true,
            assign_new_items: false,
        };
        site_row.thumbnail_id = Some(AssetId(77));

        SourceData {
            users: vec![user(2, "ada"), user(5, "grace")],
            assets: vec![asset(77, "a1b2c3d4e5f60718293a4b5c6d7e8f9012345678")],
            vocabularies: vec![VocabularyRow {
                id: VocabularyId(3),
                owner_id: Some(UserId(2)),
                namespace_uri: "http://purl.org/dc/terms/".to_string(),
                prefix: "dcterms".to_string(),
                label: "Dublin Core".to_string(),
                comment: None,
            }],
            properties: vec![PropertyRow {
                id: PropertyId(4),
                owner_id: Some(UserId(2)),
                vocabulary_id: VocabularyId(3),
                local_name: "title".to_string(),
                label: "Title".to_string(),
                comment: None,
            }],
            items: vec![ItemRow {
                resource: helmet_resource,
                primary_media_id: Some(MediaId(11)),
            }],
            item_sets: vec![ItemSetRow {
                resource: resource(8, 2, "Helmets"),
                is_open: true,
            }],
            media: vec![MediaRow {
                resource: resource(11, 2, "helmet.jpg"),
                item_id: ItemId(7),
                ingester: "upload".to_string(),
                renderer: "file".to_string(),
                data: None,
                source: None,
                media_type: Some("image/jpeg".to_string()),
                storage_id: Some("abc123".to_string()),
                extension: Some("jpg".to_string()),
                sha256: None,
                size: Some(1234),
                has_original: true,
                has_thumbnails: false,
                position: Some(0),
                lang: None,
                alt_text: Some("A helmet".to_string()),
            }],
            values: vec![
                ValueRow {
                    id: ValueId(20),
                    resource_id: ResourceId(7),
                    property_id: PropertyId(4),
                    value_resource_id: Some(ResourceId(8)),
                    value_annotation_id: Some(AnnotationId(99)),
                    r#type: "literal".to_string(),
                    lang: Some("en".to_string()),
                    value: Some("Bronze helmet".to_string()),
                    uri: None,
                    is_public: true,
                },
                ValueRow {
                    id: ValueId(21),
                    resource_id: ResourceId(7),
                    property_id: PropertyId(999),
                    value_resource_id: None,
                    value_annotation_id: None,
                    r#type: "literal".to_string(),
                    lang: None,
                    value: Some("orphan".to_string()),
                    uri: None,
                    is_public: true,
                },
            ],
            item_item_sets: vec![ItemItemSetRow {
                item_id: ItemId(7),
                item_set_id: ItemSetId(8),
            }],
            sites: vec![site_row],
            pages: vec![
                SitePageRow {
                    id: PageId(40),
                    site_id: SiteId(30),
                    slug: "about".to_string(),
                    title: "About".to_string(),
                    is_public: true,
                    layout: None,
                    layout_data: None,
                    created: at("2024-01-03 00:00:01"),
                    modified: None,
                },
                SitePageRow {
                    id: PageId(41),
                    site_id: SiteId(30),
                    slug: "contents".to_string(),
                    title: "Contents".to_string(),
                    is_public: true,
                    layout: None,
                    layout_data: None,
                    created: at("2024-01-03 00:00:02"),
                    modified: None,
                },
            ],
            blocks: vec![
                SiteBlockRow {
                    id: BlockId(50),
                    page_id: PageId(40),
                    layout: "html".to_string(),
                    data: json!({"html": "<p>Hello</p>"}),
                    layout_data: None,
                    position: 0,
                },
                SiteBlockRow {
                    id: BlockId(51),
                    page_id: PageId(40),
                    layout: "media".to_string(),
                    data: json!({"media_display": "thumbnail"}),
                    layout_data: None,
                    position: 1,
                },
                SiteBlockRow {
                    id: BlockId(52),
                    page_id: PageId(40),
                    layout: "asset".to_string(),
                    data: json!([{"id": 77, "page": 40, "alt_link_title": "", "caption": "Logo"}]),
                    layout_data: Some(json!({"background_image_asset": 77})),
                    position: 2,
                },
                SiteBlockRow {
                    id: BlockId(53),
                    page_id: PageId(41),
                    layout: "listOfPages".to_string(),
                    data: json!({"pagelist": "[{\"text\":\"About\",\"data\":{\"id\":40},\"children\":[]}]"}),
                    layout_data: None,
                    position: 0,
                },
            ],
            site_item_sets: vec![SiteItemSetRow {
                id: SiteItemSetId(60),
                site_id: SiteId(30),
                item_set_id: ItemSetId(8),
                position: Some(1),
            }],
            site_permissions: vec![SitePermissionRow {
                id: SitePermissionId(70),
                site_id: SiteId(30),
                user_id: UserId(5),
                role: "editor".to_string(),
            }],
            api_keys: vec![ApiKeyRow {
                id: "key-abc".to_string(),
                owner_id: UserId(2),
                label: "dev".to_string(),
                credential_hash: "hash".to_string(),
                last_ip: None,
                last_accessed: None,
                created: at("2024-01-04 00:00:00"),
            }],
            block_attachments: vec![SiteBlockAttachmentRow {
                id: AttachmentId(80),
                block_id: BlockId(51),
                item_id: Some(ItemId(7)),
                media_id: Some(MediaId(11)),
                caption: "A helmet".to_string(),
                position: 1,
            }],
            item_sites: vec![ItemSiteRow {
                item_id: ItemId(7),
                site_id: SiteId(30),
            }],
            settings: vec![SettingRow {
                id: "administrator_email".to_string(),
                value: json!("ada@example.org"),
            }],
            site_settings: vec![SiteSettingRow {
                id: "attachment_link_type".to_string(),
                site_id: SiteId(30),
                value: json!("original"),
            }],
            user_settings: vec![UserSettingRow {
                id: "default_locale".to_string(),
                user_id: UserId(5),
                value: json!("fr"),
            }],
            password_creations: vec![PasswordCreationRow {
                id: "reset-key".to_string(),
                user_id: UserId(5),
                created: at("2024-01-05 00:00:00"),
                activate: true,
            }],
            classes: vec![],
            templates: vec![],
            template_properties: vec![],
        }
    }

    async fn target() -> SqlitePool {
        let pool = omeka_db::open_memory().await.expect("in-memory target");
        omeka_db::migrate(&pool).await.expect("migrate");
        pool
    }

    #[tokio::test]
    async fn a_gapped_source_remaps_every_reference() {
        let pool = target().await;
        let report = Writer::live(&pool)
            .run(&sample())
            .await
            .expect("the run succeeds");

        let items = ItemRepo::new(&pool)
            .list(&ItemFilter::default(), &Paging::default())
            .await
            .expect("list items");
        assert_eq!(items.len(), 1);
        let item = &items[0];
        assert_eq!(
            item.resource.id,
            ResourceId(1),
            "the first resource gets id 1"
        );
        assert_eq!(item.resource.title.as_deref(), Some("Bronze helmet"));
        assert_eq!(item.resource.owner_id, Some(UserId(1)));
        assert_eq!(
            item.resource.thumbnail_id,
            Some(AssetId(1)),
            "the item's thumbnail points at the asset's new id"
        );
        assert_eq!(
            item.primary_media_id,
            Some(MediaId(3)),
            "the item points at the media's new id"
        );

        let assets = AssetRepo::new(&pool)
            .list(&AssetFilter::default(), &Paging::default())
            .await
            .expect("list assets");
        assert_eq!(assets.len(), 1);
        assert_eq!(assets[0].id, AssetId(1));
        assert_eq!(
            assets[0].storage_id,
            "a1b2c3d4e5f60718293a4b5c6d7e8f9012345678"
        );

        let media = MediaRepo::new(&pool)
            .list(&MediaFilter::default(), &Paging::default())
            .await
            .expect("list media");
        assert_eq!(media[0].id(), MediaId(3));
        assert_eq!(
            media[0].item_id,
            ItemId(1),
            "media points at the item's new id"
        );
        assert_eq!(media[0].alt_text.as_deref(), Some("A helmet"));

        let values = ValueRepo::new(&pool)
            .list_for_resource(ResourceId(1))
            .await
            .expect("list values");
        assert_eq!(
            values.len(),
            1,
            "the value with a missing property was skipped"
        );
        assert_eq!(values[0].property_id, PropertyId(1));
        assert_eq!(
            values[0].value_resource_id,
            Some(ResourceId(2)),
            "the item set reference is remapped"
        );
        assert_eq!(values[0].value_annotation_id, None);

        let sites = SiteRepo::new(&pool)
            .list(&omeka_db::repos::SiteFilter::default(), &Paging::default())
            .await
            .expect("list sites");
        assert_eq!(sites[0].id, SiteId(1));
        assert_eq!(
            sites[0].thumbnail_id,
            Some(AssetId(1)),
            "the site's thumbnail points at the asset's new id"
        );
        assert_eq!(sites[0].homepage_id, Some(PageId(1)));
        assert_eq!(
            sites[0].navigation[0]["data"]["id"],
            json!(1),
            "the navigation page id is remapped"
        );
        assert_eq!(
            sites[0].navigation[1]["data"]["query"],
            json!("helmets"),
            "a non-page link keeps its data"
        );

        let pages = SiteRepo::new(&pool)
            .list_pages(SiteId(1))
            .await
            .expect("list pages");
        assert_eq!(pages[0].site_id, SiteId(1));
        let blocks = SiteRepo::new(&pool)
            .list_blocks(PageId(1))
            .await
            .expect("list blocks");
        assert_eq!(blocks[0].page_id, PageId(1));
        assert_eq!(blocks[0].data["html"], json!("<p>Hello</p>"));

        let asset_block = blocks
            .iter()
            .find(|block| block.layout == "asset")
            .expect("the asset block moved");
        assert_eq!(
            asset_block.data[0]["id"],
            json!(1),
            "the asset block's asset id is remapped"
        );
        assert_eq!(
            asset_block.data[0]["page"],
            json!(1),
            "the asset block's page id is remapped"
        );
        assert_eq!(
            asset_block.layout_data.as_ref().expect("layout data")["background_image_asset"],
            json!(1),
            "the block background asset is remapped"
        );

        let contents = SiteRepo::new(&pool)
            .list_pages(SiteId(1))
            .await
            .expect("list pages")
            .into_iter()
            .find(|page| page.slug == "contents")
            .expect("the contents page moved");
        let list_block = SiteRepo::new(&pool)
            .list_blocks(contents.id)
            .await
            .expect("list the contents blocks")
            .into_iter()
            .find(|block| block.layout == "listOfPages")
            .expect("the list of pages block moved");
        let pagelist: serde_json::Value = serde_json::from_str(
            list_block.data["pagelist"]
                .as_str()
                .expect("a pagelist string"),
        )
        .expect("the pagelist is JSON");
        assert_eq!(
            pagelist[0]["data"]["id"],
            json!(1),
            "the list of pages page id is remapped"
        );

        let attachments = SiteBlockAttachmentRepo::new(&pool)
            .list_for_block(BlockId(2))
            .await
            .expect("list attachments");
        assert_eq!(attachments.len(), 1);
        assert_eq!(attachments[0].block_id, BlockId(2));
        assert_eq!(
            attachments[0].item_id,
            Some(ItemId(1)),
            "the attachment's item is remapped"
        );
        assert_eq!(
            attachments[0].media_id,
            Some(MediaId(3)),
            "the attachment's media is remapped"
        );
        assert_eq!(attachments[0].caption, "A helmet");

        let site_items: Vec<(i64, i64)> =
            sqlx::query_as("SELECT item_id, site_id FROM item_site ORDER BY item_id")
                .fetch_all(&pool)
                .await
                .expect("read item_site");
        assert_eq!(site_items, vec![(1, 1)], "the site membership is remapped");

        let setting = SettingRepo::new(&pool)
            .get("administrator_email")
            .await
            .expect("read a global setting");
        assert_eq!(setting, Some(json!("ada@example.org")));
        let site_setting = SiteSettingRepo::new(&pool)
            .get(SiteId(1), "attachment_link_type")
            .await
            .expect("read a site setting");
        assert_eq!(site_setting, Some(json!("original")));
        let user_setting = UserSettingRepo::new(&pool)
            .get(UserId(2), "default_locale")
            .await
            .expect("read a user setting");
        assert_eq!(user_setting, Some(json!("fr")));
        let reset = omeka_db::repos::PasswordCreationRepo::new(&pool)
            .get_for_user(UserId(2))
            .await
            .expect("read the password key")
            .expect("the password key moved");
        assert_eq!(reset.id, "reset-key");
        assert!(reset.activate);

        let memberships = SiteRepo::new(&pool)
            .list_item_sets(SiteId(1))
            .await
            .expect("list item set pool");
        assert_eq!(memberships[0].item_set_id, ItemSetId(2));
        assert_eq!(memberships[0].position, Some(1));

        let permissions = SiteRepo::new(&pool)
            .list_permissions(SiteId(1))
            .await
            .expect("list permissions");
        assert_eq!(
            permissions[0].user_id,
            UserId(2),
            "grace is the second user"
        );
        assert_eq!(permissions[0].role, "editor");

        let item_sets = ItemRepo::new(&pool)
            .list_item_sets(ItemId(1))
            .await
            .expect("list an item's sets");
        assert_eq!(item_sets, vec![ItemSetId(2)]);

        let key = ApiKeyRepo::new(&pool)
            .get("key-abc")
            .await
            .expect("read the key")
            .expect("the key is there");
        assert_eq!(key.owner_id, UserId(1));

        assert_eq!(report.table("value").map(|t| t.skipped), Some(1));
        assert_eq!(report.table("item").map(|t| t.written), Some(1));
        for table in [
            "asset",
            "site_block_attachment",
            "item_site",
            "setting",
            "site_setting",
            "user_setting",
            "password_creation",
        ] {
            assert_eq!(
                report.table(table).map(|t| t.written),
                Some(1),
                "{table} moved"
            );
        }
        assert_eq!(report.totals().2, 1, "exactly one row is skipped");
        assert!(!report.dry_run);
    }

    #[tokio::test]
    async fn a_dry_run_counts_the_same_rows_and_writes_nothing() {
        let pool = target().await;
        let report = Writer::dry().run(&sample()).await.expect("the dry run");

        assert!(report.dry_run);
        assert_eq!(report.table("item").map(|t| t.written), Some(1));
        assert_eq!(report.table("value").map(|t| t.skipped), Some(1));
        assert_eq!(report.totals().2, 1);

        let items = ItemRepo::new(&pool)
            .list(&ItemFilter::default(), &Paging::default())
            .await
            .expect("list items");
        assert!(items.is_empty(), "the dry run wrote nothing");
    }

    #[tokio::test]
    async fn a_non_empty_target_is_refused() {
        let pool = target().await;
        let data = sample();
        Writer::live(&pool).run(&data).await.expect("first run");

        let error = ensure_empty(&pool, Path::new("target.sqlite"))
            .await
            .expect_err("a second run is refused");
        assert!(matches!(
            error,
            MigrateError::TargetNotEmpty { table: "user", .. }
        ));
    }
}
