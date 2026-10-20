//! An import's writes inside the caller's transaction (`docs/import-v2.md`, "Actions" and "Per
//! record").
//!
//! A run opens one transaction per record and hands it here, then records the ledger rows and the
//! before-image on the same transaction, so the record commits whole or not at all. Every write
//! keeps the resource's title and its `fulltext_search` row in step, as the api write path does
//! after its own writes; the before-image and the restore live in `import_state`, the index row in
//! `import_index`.
//!
//! An apply or a restore writes `modified` = now, so undo's touched test can tell a later change
//! from the import's own. A create writes the same instant into `created` and `modified`.

use std::collections::HashSet;

use sqlx::{ColumnIndex, Decode, Encode, Executor, IntoArguments, Sqlite, Transaction, Type};

use super::resources::update_resource_columns;
use super::values::insert_value;
use super::{ItemRepo, ItemSetRepo, MediaRepo, Statement, bind_id, bind_opt_id};
use crate::DbError;
use crate::backend::Backend;
use crate::ids::{ItemId, ItemSetId, MediaId, PropertyId, ResourceId, SiteId};
use crate::rows::{
    ImportAction, ImportFields, ImportResource, ItemSlots, NewImportFields, NewImportItem,
    NewImportItemSet, NewImportMedia, NewItem, NewItemSet, NewMedia, NewResource, NewWholeItem,
    ResourceRow, Slot, ValueContent, ValueRow,
};

/// The writes of one import record, on the record's transaction.
pub struct ImportWrites<'t, DB: Backend = Sqlite> {
    pub(super) conn: &'t mut DB::Connection,
}

impl<'t, DB: Backend> ImportWrites<'t, DB>
where
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
    usize: ColumnIndex<DB::Row>,
    for<'x> i64: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Option<i64>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> bool: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> String: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Option<String>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> Vec<u8>: Decode<'x, DB> + Type<DB>,
    for<'x> Option<Vec<u8>>: Decode<'x, DB> + Encode<'x, DB> + Type<DB>,
    for<'x> &'x str: Encode<'x, DB> + Type<DB> + ColumnIndex<DB::Row>,
    for<'x> f64: Decode<'x, DB> + Type<DB>,
{
    pub fn new(tx: &'t mut Transaction<'_, DB>) -> Self {
        Self { conn: &mut **tx }
    }

    /// An item with its values, sets and sites. Its media follow with [`Self::create_media`].
    pub async fn create_item(&mut self, new: &NewImportItem) -> Result<ItemId, DbError> {
        let whole = NewWholeItem {
            item: NewItem {
                resource: new_resource(&new.fields),
                primary_media_id: None,
            },
            item_sets: new.item_sets.clone(),
            values: new.fields.values.clone(),
        };
        let id = ItemRepo::<DB>::insert_whole(self.conn, &whole).await?.id();
        self.add_item_sites(id, &new.sites).await?;
        self.settle(ImportResource::Item(id)).await?;
        Ok(id)
    }

    /// A media row for a file already in the store, or for what a URL ingester made, with its
    /// values.
    pub async fn create_media(&mut self, new: &NewImportMedia) -> Result<MediaId, DbError> {
        let file = new.file.as_ref();
        let row = MediaRepo::<DB>::insert(
            self.conn,
            &NewMedia {
                resource: new_resource(&new.fields),
                item_id: new.item,
                ingester: new.ingester.clone(),
                renderer: new.renderer.clone(),
                data: new.data.clone(),
                source: new.source.clone(),
                media_type: file.and_then(|file| file.media_type.clone()),
                storage_id: file.map(|file| file.storage_id.clone()),
                extension: file.and_then(|file| file.extension.clone()),
                sha256: file.and_then(|file| file.sha256.clone()),
                size: file.and_then(|file| file.size),
                has_original: file.is_some(),
                has_thumbnails: file.is_some_and(|file| file.has_thumbnails),
                position: Some(new.position),
                lang: None,
                alt_text: None,
            },
        )
        .await?;
        self.insert_values(row.resource.id, &new.fields.values)
            .await?;
        self.settle(ImportResource::Media(row.id())).await?;
        Ok(row.id())
    }

    /// An item set with its values, appended to each site's item-set list.
    pub async fn create_item_set(&mut self, new: &NewImportItemSet) -> Result<ItemSetId, DbError> {
        let row = ItemSetRepo::<DB>::insert(
            self.conn,
            &NewItemSet {
                resource: new_resource(&new.fields),
                is_open: new.is_open,
            },
        )
        .await?;
        self.insert_values(row.resource.id, &new.fields.values)
            .await?;
        for site in &new.sites {
            self.append_set_to_site(row.id(), *site).await?;
        }
        self.settle(ImportResource::ItemSet(row.id())).await?;
        Ok(row.id())
    }

    /// Name the item's primary media, or clear it.
    pub async fn set_primary_media(
        &mut self,
        item: ItemId,
        media: Option<MediaId>,
    ) -> Result<(), DbError> {
        sqlx::query("UPDATE item SET primary_media_id = ? WHERE id = ?")
            .bind(bind_opt_id(media))
            .bind(bind_id(item))
            .execute(&mut *self.conn)
            .await?;
        Ok(())
    }

    /// Apply `action` to a matched item. False when the item is gone.
    pub async fn apply_item(
        &mut self,
        id: ItemId,
        action: ImportAction,
        fields: &ImportFields,
        slots: &ItemSlots,
    ) -> Result<bool, DbError> {
        if !self.apply_fields(id.as_resource(), action, fields).await? {
            return Ok(false);
        }
        if let Some(sets) = memberships(action, &slots.item_sets) {
            if action != ImportAction::Append {
                self.retain_item_sets(id, &sets).await?;
            }
            for set in &sets {
                sqlx::query(DB::insert_item_item_set())
                    .bind(bind_id(id))
                    .bind(bind_id(*set))
                    .execute(&mut *self.conn)
                    .await?;
            }
        }
        if let Some(sites) = memberships(action, &slots.sites) {
            if action != ImportAction::Append {
                self.retain_item_sites(id, &sites).await?;
            }
            self.add_item_sites(id, &sites).await?;
        }
        self.settle(ImportResource::Item(id)).await?;
        Ok(true)
    }

    /// Apply `action` to a matched media. A media has no sets or sites. False when it is gone.
    pub async fn apply_media(
        &mut self,
        id: MediaId,
        action: ImportAction,
        fields: &ImportFields,
    ) -> Result<bool, DbError> {
        if !self.apply_fields(id.as_resource(), action, fields).await? {
            return Ok(false);
        }
        self.settle(ImportResource::Media(id)).await?;
        Ok(true)
    }

    /// Apply `action` to a matched item set. A site it joins gets it at the end of its list; a
    /// site it stays in keeps its position. False when it is gone.
    pub async fn apply_item_set(
        &mut self,
        id: ItemSetId,
        action: ImportAction,
        fields: &ImportFields,
        sites: &Slot<Vec<SiteId>>,
    ) -> Result<bool, DbError> {
        if !self.apply_fields(id.as_resource(), action, fields).await? {
            return Ok(false);
        }
        if let Some(sites) = memberships(action, sites) {
            if action != ImportAction::Append {
                self.retain_set_sites(id, &sites).await?;
            }
            let held = self.set_sites(id).await?;
            for site in sites.iter().filter(|site| !held.contains(site)) {
                self.append_set_to_site(id, *site).await?;
            }
        }
        self.settle(ImportResource::ItemSet(id)).await?;
        Ok(true)
    }

    /// The values and the single-valued slots, which every kind shares. Writes `modified`.
    async fn apply_fields(
        &mut self,
        id: ResourceId,
        action: ImportAction,
        fields: &ImportFields,
    ) -> Result<bool, DbError> {
        let Some(mut row) = self.resource_row(id).await? else {
            return Ok(false);
        };
        match action {
            ImportAction::Append => {
                let stored = self.values_of(id).await?;
                let mut seen: HashSet<ValueKey> = stored.iter().map(ValueKey::of_row).collect();
                let added: Vec<ValueContent> = fields
                    .values
                    .iter()
                    .filter(|value| seen.insert(ValueKey::of(value)))
                    .cloned()
                    .collect();
                self.insert_values(id, &added).await?;
            }
            ImportAction::Revise => {
                let terms: Vec<PropertyId> = terms_of(&fields.values, &[]);
                self.delete_values(id, Some(&terms)).await?;
                self.insert_values(id, &fields.values).await?;
            }
            ImportAction::Update => {
                let terms = terms_of(&fields.values, &fields.mapped_terms);
                self.delete_values(id, Some(&terms)).await?;
                self.insert_values(id, &fields.values).await?;
            }
            ImportAction::Replace => {
                self.delete_values(id, None).await?;
                self.insert_values(id, &fields.values).await?;
            }
        }
        if let Some(template) = fields.template.written(action) {
            row.resource_template_id = template;
        }
        if let Some(class) = fields.class.written(action) {
            row.resource_class_id = class;
        }
        if let Some(owner) = fields.owner.written(action) {
            row.owner_id = owner;
        }
        if let Some(is_public) = fields.is_public.written(action) {
            row.is_public = is_public;
        }
        update_resource_columns(self.conn, &row).await?;
        Ok(true)
    }

    pub(super) async fn resource_row(
        &mut self,
        id: ResourceId,
    ) -> Result<Option<ResourceRow>, DbError> {
        let row = sqlx::query("SELECT * FROM resource WHERE id = ?")
            .bind(bind_id(id))
            .fetch_optional(&mut *self.conn)
            .await?;
        row.as_ref().map(ResourceRow::from_db_row::<DB>).transpose()
    }

    /// A resource's values in upstream's order.
    pub(super) async fn values_of(&mut self, id: ResourceId) -> Result<Vec<ValueRow>, DbError> {
        let rows = sqlx::query("SELECT * FROM value WHERE resource_id = ? ORDER BY id")
            .bind(bind_id(id))
            .fetch_all(&mut *self.conn)
            .await?;
        rows.iter().map(ValueRow::from_db_row::<DB>).collect()
    }

    pub(super) async fn insert_values(
        &mut self,
        id: ResourceId,
        values: &[ValueContent],
    ) -> Result<(), DbError> {
        for value in values {
            insert_value(self.conn, &value.on(id)).await?;
        }
        Ok(())
    }

    /// Delete a resource's values, all of them or those of some terms, and the annotations they
    /// carry: upstream maps an annotation to its value with `orphanRemoval`.
    pub(super) async fn delete_values(
        &mut self,
        id: ResourceId,
        terms: Option<&[PropertyId]>,
    ) -> Result<(), DbError> {
        if terms.is_some_and(<[PropertyId]>::is_empty) {
            return Ok(());
        }
        let annotations: Vec<ResourceId> = self
            .values_of(id)
            .await?
            .into_iter()
            .filter(|value| terms.is_none_or(|terms| terms.contains(&value.property_id)))
            .filter_map(|value| value.value_annotation_id.map(|id| id.as_resource()))
            .collect();
        let mut statement = Statement::<DB>::new("DELETE FROM value WHERE resource_id = ");
        statement.bind(bind_id(id))?;
        if let Some(terms) = terms {
            statement.push(" AND property_id IN (");
            for (index, term) in terms.iter().enumerate() {
                if index > 0 {
                    statement.push(", ");
                }
                statement.bind(bind_id(*term))?;
            }
            statement.push(")");
        }
        let (sql, arguments) = statement.into_parts();
        sqlx::query_with::<DB, _>(&sql, arguments)
            .execute(&mut *self.conn)
            .await?;
        for annotation in annotations {
            Box::pin(self.delete_values(annotation, None)).await?;
            sqlx::query("DELETE FROM resource WHERE id = ?")
                .bind(bind_id(annotation))
                .execute(&mut *self.conn)
                .await?;
        }
        Ok(())
    }

    pub(super) async fn add_item_sites(
        &mut self,
        item: ItemId,
        sites: &[SiteId],
    ) -> Result<(), DbError> {
        let sql = format!("{}(?, ?)", DB::insert_item_site());
        for site in sites {
            sqlx::query(&sql)
                .bind(bind_id(item))
                .bind(bind_id(*site))
                .execute(&mut *self.conn)
                .await?;
        }
        Ok(())
    }

    /// Detach the item from every set not in `keep`.
    pub(super) async fn retain_item_sets(
        &mut self,
        item: ItemId,
        keep: &[ItemSetId],
    ) -> Result<(), DbError> {
        self.retain(
            "item_item_set",
            "item_id",
            "item_set_id",
            bind_id(item),
            keep,
        )
        .await
    }

    pub(super) async fn retain_item_sites(
        &mut self,
        item: ItemId,
        keep: &[SiteId],
    ) -> Result<(), DbError> {
        self.retain("item_site", "item_id", "site_id", bind_id(item), keep)
            .await
    }

    pub(super) async fn retain_set_sites(
        &mut self,
        set: ItemSetId,
        keep: &[SiteId],
    ) -> Result<(), DbError> {
        self.retain(
            "site_item_set",
            "item_set_id",
            "site_id",
            bind_id(set),
            keep,
        )
        .await
    }

    /// `DELETE FROM table WHERE owner = ? AND member NOT IN (keep)`, over fixed table names.
    async fn retain<T: Copy + Into<u64>>(
        &mut self,
        table: &'static str,
        owner: &'static str,
        member: &'static str,
        id: i64,
        keep: &[T],
    ) -> Result<(), DbError> {
        let mut statement = Statement::<DB>::new(&format!("DELETE FROM {table} WHERE {owner} = "));
        statement.bind(id)?;
        if !keep.is_empty() {
            statement.push(&format!(" AND {member} NOT IN ("));
            for (index, kept) in keep.iter().enumerate() {
                if index > 0 {
                    statement.push(", ");
                }
                statement.bind(bind_id(*kept))?;
            }
            statement.push(")");
        }
        let (sql, arguments) = statement.into_parts();
        sqlx::query_with::<DB, _>(&sql, arguments)
            .execute(&mut *self.conn)
            .await?;
        Ok(())
    }

    /// The sites whose item-set list holds the set.
    pub(super) async fn set_sites(&mut self, set: ItemSetId) -> Result<Vec<SiteId>, DbError> {
        let ids: Vec<i64> = sqlx::query_scalar::<DB, i64>(
            "SELECT site_id FROM site_item_set WHERE item_set_id = ? ORDER BY site_id",
        )
        .bind(bind_id(set))
        .fetch_all(&mut *self.conn)
        .await?;
        ids.into_iter()
            .map(|raw| Ok(SiteId::from(super::new_id(raw)?)))
            .collect()
    }

    /// Put the set at the end of the site's item-set list, positions being 1-based as the api's
    /// site write numbers them.
    async fn append_set_to_site(&mut self, set: ItemSetId, site: SiteId) -> Result<(), DbError> {
        let last: Option<i64> = sqlx::query_scalar::<DB, Option<i64>>(
            "SELECT MAX(position) FROM site_item_set WHERE site_id = ?",
        )
        .bind(bind_id(site))
        .fetch_one(&mut *self.conn)
        .await?;
        self.put_set_in_site(set, site, Some(last.unwrap_or(0) + 1))
            .await
    }

    pub(super) async fn put_set_in_site(
        &mut self,
        set: ItemSetId,
        site: SiteId,
        position: Option<i64>,
    ) -> Result<(), DbError> {
        sqlx::query(DB::upsert_site_item_set())
            .bind(bind_id(site))
            .bind(bind_id(set))
            .bind(position)
            .execute(&mut *self.conn)
            .await?;
        Ok(())
    }
}

fn new_resource(fields: &NewImportFields) -> NewResource {
    NewResource {
        owner_id: fields.owner,
        resource_class_id: fields.class,
        resource_template_id: fields.template,
        is_public: fields.is_public,
        ..NewResource::default()
    }
}

/// The sets or sites `action` writes: `append` adds what the slot names, the others follow the
/// slot's own rule.
fn memberships<T: Clone>(action: ImportAction, slot: &Slot<Vec<T>>) -> Option<Vec<T>> {
    match (action, slot) {
        (ImportAction::Append, Slot::Mapped(members)) => Some(members.clone()),
        (ImportAction::Append, Slot::Unmapped) => None,
        _ => slot.written(action),
    }
}

/// The terms a record's values name, plus `mapped`, each once.
fn terms_of(values: &[ValueContent], mapped: &[PropertyId]) -> Vec<PropertyId> {
    let mut terms: Vec<PropertyId> = mapped.to_vec();
    for value in values {
        if !terms.contains(&value.property_id) {
            terms.push(value.property_id);
        }
    }
    terms
}

/// What makes two values the same for `append`: the term, the type, the text and the language.
/// The text of a URI is the URI and of a link the linked id; a URI's label is not part of it.
#[derive(PartialEq, Eq, Hash)]
struct ValueKey {
    property: PropertyId,
    r#type: String,
    uri: Option<String>,
    resource: Option<ResourceId>,
    text: Option<String>,
    lang: Option<String>,
}

impl ValueKey {
    fn new(
        property: PropertyId,
        r#type: &str,
        uri: &Option<String>,
        resource: Option<ResourceId>,
        text: &Option<String>,
        lang: &Option<String>,
    ) -> Self {
        let keyed = uri.is_some() || resource.is_some();
        Self {
            property,
            r#type: r#type.to_string(),
            uri: uri.clone(),
            resource,
            text: if keyed { None } else { text.clone() },
            lang: lang.clone(),
        }
    }

    fn of(value: &ValueContent) -> Self {
        Self::new(
            value.property_id,
            &value.r#type,
            &value.uri,
            value.value_resource_id,
            &value.value,
            &value.lang,
        )
    }

    fn of_row(value: &ValueRow) -> Self {
        Self::new(
            value.property_id,
            &value.r#type,
            &value.uri,
            value.value_resource_id,
            &value.value,
            &value.lang,
        )
    }
}
