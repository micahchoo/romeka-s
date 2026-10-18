//! The column mapping: one pure function per table, from a source row to the port's insert input.
//!
//! Every function is pure and returns `None` when a foreign key it needs is not in the source, which
//! is the one reason a row is skipped. `IdMap` is the source id to target id lookup the functions
//! read; a dry run fills it with the identity map, so the same functions decide the same skips
//! without a target.
//!
//! `omeka-db` assigns the target its own ids, so a source id never travels to the target as a row
//! key. Every reference is looked up and rewritten, including the page ids inside a site's
//! navigation tree.

use std::collections::HashMap;

use omeka_db::ids::{
    AssetId, BlockId, ClassId, ItemId, ItemSetId, MediaId, PageId, PropertyId, ResourceId, SiteId,
    TemplateId, UserId, VocabularyId,
};
use omeka_db::rows::{
    ApiKeyRow, AssetRow, ClassRow, ItemRow, ItemSetRow, MediaRow, NewApiKey, NewAsset, NewClass,
    NewItem, NewItemSet, NewMedia, NewPasswordCreation, NewProperty, NewResource, NewSite,
    NewSiteBlock, NewSiteBlockAttachment, NewSitePage, NewTemplate, NewTemplateProperty, NewUser,
    NewValue, NewVocabulary, PasswordCreationRow, PropertyRow, ResourceRow, SiteBlockAttachmentRow,
    SiteBlockRow, SitePageRow, SiteRow, TemplatePropertyRow, TemplateRow, UserRow, ValueRow,
    VocabularyRow,
};
use serde_json::Value;

/// Source id to target id, one map per table whose id another table references.
#[derive(Debug, Default)]
pub(crate) struct IdMap {
    user: HashMap<u64, UserId>,
    asset: HashMap<u64, AssetId>,
    vocabulary: HashMap<u64, VocabularyId>,
    class: HashMap<u64, ClassId>,
    property: HashMap<u64, PropertyId>,
    template: HashMap<u64, TemplateId>,
    resource: HashMap<u64, ResourceId>,
    item: HashMap<u64, ItemId>,
    item_set: HashMap<u64, ItemSetId>,
    media: HashMap<u64, MediaId>,
    site: HashMap<u64, SiteId>,
    page: HashMap<u64, PageId>,
    block: HashMap<u64, BlockId>,
}

/// One map accessor per table, so a reference reads as `ids.user(src)`.
macro_rules! accessor {
    ($name:ident, $field:ident, $type:ty) => {
        pub(crate) fn $name(&self, source: u64) -> Option<$type> {
            self.$field.get(&source).copied()
        }
    };
}

impl IdMap {
    accessor!(user, user, UserId);
    accessor!(asset, asset, AssetId);
    accessor!(vocabulary, vocabulary, VocabularyId);
    accessor!(class, class, ClassId);
    accessor!(property, property, PropertyId);
    accessor!(template, template, TemplateId);
    accessor!(resource, resource, ResourceId);
    accessor!(item, item, ItemId);
    accessor!(item_set, item_set, ItemSetId);
    accessor!(media, media, MediaId);
    accessor!(site, site, SiteId);
    accessor!(page, page, PageId);
    accessor!(block, block, BlockId);

    pub(crate) fn insert_user(&mut self, source: u64, target: UserId) {
        self.user.insert(source, target);
    }

    pub(crate) fn insert_asset(&mut self, source: u64, target: AssetId) {
        self.asset.insert(source, target);
    }

    pub(crate) fn insert_vocabulary(&mut self, source: u64, target: VocabularyId) {
        self.vocabulary.insert(source, target);
    }

    pub(crate) fn insert_class(&mut self, source: u64, target: ClassId) {
        self.class.insert(source, target);
    }

    pub(crate) fn insert_property(&mut self, source: u64, target: PropertyId) {
        self.property.insert(source, target);
    }

    pub(crate) fn insert_template(&mut self, source: u64, target: TemplateId) {
        self.template.insert(source, target);
    }

    pub(crate) fn insert_resource(&mut self, source: u64, target: ResourceId) {
        self.resource.insert(source, target);
    }

    pub(crate) fn insert_item(&mut self, source: u64, target: ItemId) {
        self.item.insert(source, target);
    }

    pub(crate) fn insert_item_set(&mut self, source: u64, target: ItemSetId) {
        self.item_set.insert(source, target);
    }

    pub(crate) fn insert_media(&mut self, source: u64, target: MediaId) {
        self.media.insert(source, target);
    }

    pub(crate) fn insert_site(&mut self, source: u64, target: SiteId) {
        self.site.insert(source, target);
    }

    pub(crate) fn insert_page(&mut self, source: u64, target: PageId) {
        self.page.insert(source, target);
    }

    pub(crate) fn insert_block(&mut self, source: u64, target: BlockId) {
        self.block.insert(source, target);
    }
}

/// Map a nullable foreign key. `None` stays `None`; a `Some` id with no mapping makes the row
/// skip, so the caller's `?` returns `None` from the whole mapping function.
fn map_opt<T: Copy>(
    lookup: impl Fn(u64) -> Option<T>,
    id: Option<impl Into<u64>>,
) -> Option<Option<T>> {
    match id {
        None => Some(None),
        Some(value) => lookup(value.into()).map(Some),
    }
}

/// The `resource` half of any resource kind.
///
/// A `thumbnail_id` whose asset was skipped reads as absent, the way the source's
/// `ON DELETE SET NULL` would leave it, rather than skipping the resource.
fn new_resource(row: &ResourceRow, ids: &IdMap) -> Option<NewResource> {
    Some(NewResource {
        owner_id: map_opt(|id| ids.user(id), row.owner_id)?,
        resource_class_id: map_opt(|id| ids.class(id), row.resource_class_id)?,
        resource_template_id: map_opt(|id| ids.template(id), row.resource_template_id)?,
        thumbnail_id: row.thumbnail_id.and_then(|id| ids.asset(id.0)),
        title: row.title.clone(),
        is_public: row.is_public,
        created: Some(row.created),
    })
}

pub(crate) fn new_user(row: &UserRow) -> NewUser {
    NewUser {
        email: row.email.clone(),
        name: row.name.clone(),
        password_hash: row.password_hash.clone(),
        role: row.role.clone(),
        is_active: row.is_active,
        created: Some(row.created),
    }
}

pub(crate) fn new_asset(row: &AssetRow, ids: &IdMap) -> Option<NewAsset> {
    Some(NewAsset {
        owner_id: map_opt(|id| ids.user(id), row.owner_id)?,
        name: row.name.clone(),
        media_type: row.media_type.clone(),
        storage_id: row.storage_id.clone(),
        extension: row.extension.clone(),
        alt_text: row.alt_text.clone(),
    })
}

pub(crate) fn new_vocabulary(row: &VocabularyRow, ids: &IdMap) -> Option<NewVocabulary> {
    Some(NewVocabulary {
        owner_id: map_opt(|id| ids.user(id), row.owner_id)?,
        namespace_uri: row.namespace_uri.clone(),
        prefix: row.prefix.clone(),
        label: row.label.clone(),
        comment: row.comment.clone(),
    })
}

pub(crate) fn new_class(row: &ClassRow, ids: &IdMap) -> Option<NewClass> {
    Some(NewClass {
        owner_id: map_opt(|id| ids.user(id), row.owner_id)?,
        vocabulary_id: ids.vocabulary(row.vocabulary_id.0)?,
        local_name: row.local_name.clone(),
        label: row.label.clone(),
        comment: row.comment.clone(),
    })
}

pub(crate) fn new_property(row: &PropertyRow, ids: &IdMap) -> Option<NewProperty> {
    Some(NewProperty {
        owner_id: map_opt(|id| ids.user(id), row.owner_id)?,
        vocabulary_id: ids.vocabulary(row.vocabulary_id.0)?,
        local_name: row.local_name.clone(),
        label: row.label.clone(),
        comment: row.comment.clone(),
    })
}

pub(crate) fn new_template(row: &TemplateRow, ids: &IdMap) -> Option<NewTemplate> {
    Some(NewTemplate {
        owner_id: map_opt(|id| ids.user(id), row.owner_id)?,
        resource_class_id: map_opt(|id| ids.class(id), row.resource_class_id)?,
        title_property_id: map_opt(|id| ids.property(id), row.title_property_id)?,
        description_property_id: map_opt(|id| ids.property(id), row.description_property_id)?,
        label: row.label.clone(),
    })
}

pub(crate) fn new_template_property(
    row: &TemplatePropertyRow,
    ids: &IdMap,
) -> Option<NewTemplateProperty> {
    Some(NewTemplateProperty {
        resource_template_id: ids.template(row.resource_template_id.0)?,
        property_id: ids.property(row.property_id.0)?,
        alternate_label: row.alternate_label.clone(),
        alternate_comment: row.alternate_comment.clone(),
        position: row.position,
        data_type: row.data_type.clone(),
        is_required: row.is_required,
        is_private: row.is_private,
        default_lang: row.default_lang.clone(),
    })
}

pub(crate) fn new_item(row: &ItemRow, ids: &IdMap) -> Option<NewItem> {
    Some(NewItem {
        resource: new_resource(&row.resource, ids)?,
        // `item.primary_media_id` points forward at a media row the run has not written yet, so it
        // is set after the media pass.
        primary_media_id: None,
    })
}

pub(crate) fn new_item_set(row: &ItemSetRow, ids: &IdMap) -> Option<NewItemSet> {
    Some(NewItemSet {
        resource: new_resource(&row.resource, ids)?,
        is_open: row.is_open,
    })
}

pub(crate) fn new_media(row: &MediaRow, ids: &IdMap) -> Option<NewMedia> {
    Some(NewMedia {
        resource: new_resource(&row.resource, ids)?,
        item_id: ids.item(row.item_id.0)?,
        ingester: row.ingester.clone(),
        renderer: row.renderer.clone(),
        data: row.data.clone(),
        source: row.source.clone(),
        media_type: row.media_type.clone(),
        storage_id: row.storage_id.clone(),
        extension: row.extension.clone(),
        sha256: row.sha256.clone(),
        size: row.size,
        has_original: row.has_original,
        has_thumbnails: row.has_thumbnails,
        position: row.position,
        lang: row.lang.clone(),
        alt_text: row.alt_text.clone(),
    })
}

pub(crate) fn new_value(row: &ValueRow, ids: &IdMap) -> Option<NewValue> {
    Some(NewValue {
        resource_id: ids.resource(row.resource_id.0)?,
        property_id: ids.property(row.property_id.0)?,
        value_resource_id: map_opt(|id| ids.resource(id), row.value_resource_id)?,
        // Value annotations have no repository read, so no annotation is in the source data and
        // this reference is dropped. See the README's known losses.
        value_annotation_id: None,
        r#type: row.r#type.clone(),
        lang: row.lang.clone(),
        value: row.value.clone(),
        uri: row.uri.clone(),
        is_public: row.is_public,
    })
}

pub(crate) fn new_site(row: &SiteRow, ids: &IdMap) -> Option<NewSite> {
    Some(NewSite {
        thumbnail_id: row.thumbnail_id.and_then(|id| ids.asset(id.0)),
        // `site.homepage_id` points forward at a page the run has not written yet.
        homepage_id: None,
        owner_id: map_opt(|id| ids.user(id), row.owner_id)?,
        slug: row.slug.clone(),
        theme: row.theme.clone(),
        title: row.title.clone(),
        summary: row.summary.clone(),
        // Navigation carries page ids, so it is rewritten after the page pass.
        navigation: row.navigation.clone(),
        item_pool: row.item_pool.clone(),
        is_public: row.is_public,
        assign_new_items: row.assign_new_items,
        created: Some(row.created),
    })
}

pub(crate) fn new_site_page(row: &SitePageRow, ids: &IdMap) -> Option<NewSitePage> {
    Some(NewSitePage {
        site_id: ids.site(row.site_id.0)?,
        slug: row.slug.clone(),
        title: row.title.clone(),
        is_public: row.is_public,
        layout: row.layout.clone(),
        layout_data: row.layout_data.clone(),
        created: Some(row.created),
    })
}

pub(crate) fn new_site_block(row: &SiteBlockRow, ids: &IdMap) -> Option<NewSiteBlock> {
    Some(NewSiteBlock {
        page_id: ids.page(row.page_id.0)?,
        layout: row.layout.clone(),
        data: remap_block_data(&row.layout, &row.data, ids),
        layout_data: remap_block_layout_data(row.layout_data.as_ref(), ids),
        position: row.position,
    })
}

/// One `site_block_attachment`: the block, item and media references are remapped. A reference whose
/// row was skipped drops the attachment.
pub(crate) fn new_site_block_attachment(
    row: &SiteBlockAttachmentRow,
    ids: &IdMap,
) -> Option<NewSiteBlockAttachment> {
    Some(NewSiteBlockAttachment {
        block_id: ids.block(row.block_id.0)?,
        item_id: map_opt(|id| ids.item(id), row.item_id)?,
        media_id: map_opt(|id| ids.media(id), row.media_id)?,
        caption: row.caption.clone(),
        position: row.position,
    })
}

/// A `password_creation` row, with its user remapped.
pub(crate) fn new_password_creation(
    row: &PasswordCreationRow,
    ids: &IdMap,
) -> Option<NewPasswordCreation> {
    Some(NewPasswordCreation {
        id: row.id.clone(),
        user_id: ids.user(row.user_id.0)?,
        activate: row.activate,
        created: Some(row.created),
    })
}

pub(crate) fn new_api_key(row: &ApiKeyRow, ids: &IdMap) -> Option<NewApiKey> {
    Some(NewApiKey {
        id: row.id.clone(),
        owner_id: ids.user(row.owner_id.0)?,
        label: row.label.clone(),
        credential_hash: row.credential_hash.clone(),
        last_ip: row.last_ip.clone(),
        last_accessed: row.last_accessed,
        created: Some(row.created),
    })
}

/// Rewrite the page ids inside a site's navigation tree.
///
/// Upstream stores navigation as a JSON array of links; a `page` link carries its page id under
/// `data.id` (`application/src/Site/Navigation/Link/Page.php`, `isValid`). Other link types carry
/// no page id, so only `page` links are rewritten. A page id with no mapping is left as the source
/// id, which is what upstream shows for a missing page.
pub(crate) fn remap_navigation(navigation: &Value, ids: &IdMap) -> Value {
    match navigation {
        Value::Array(links) => {
            Value::Array(links.iter().map(|link| remap_link(link, ids)).collect())
        }
        other => other.clone(),
    }
}

fn remap_link(link: &Value, ids: &IdMap) -> Value {
    let Value::Object(fields) = link else {
        return link.clone();
    };
    let mut rewritten = fields.clone();

    if let Some(Value::Array(children)) = fields.get("links") {
        let children = children
            .iter()
            .map(|child| remap_link(child, ids))
            .collect();
        rewritten.insert("links".to_string(), Value::Array(children));
    }

    let is_page = matches!(fields.get("type"), Some(Value::String(kind)) if kind == "page");
    if is_page && let Some(Value::Object(data)) = fields.get("data") {
        let mut data = data.clone();
        if let Some(source) = data.get("id").and_then(json_id)
            && let Some(page) = ids.page(source)
        {
            data.insert("id".to_string(), Value::from(page.0));
        }
        rewritten.insert("data".to_string(), Value::Object(data));
    }

    Value::Object(rewritten)
}

/// A navigation id is a JSON number in upstream's stored tree and a string in some hand-edited
/// trees. Both name the same page.
fn json_id(value: &Value) -> Option<u64> {
    match value {
        Value::Number(number) => number.as_u64(),
        Value::String(text) => text.parse().ok(),
        _ => None,
    }
}

/// Rewrite the ids a block stores inside its `data`, for the two layouts that carry one.
///
/// Upstream's `asset` block stores an array of slots with an asset id under `id` and a page id
/// under `page` (`BlockLayout/Asset.php::prepareAssetAttachments`), and `listOfPages` stores a page
/// tree in the `pagelist` JSON string (`BlockLayout/ListOfPages.php`). Every other layout's `data`
/// is copied unchanged. An id whose row was skipped is left as the source wrote it, which is what
/// `site.navigation` does with a missing page.
pub(crate) fn remap_block_data(layout: &str, data: &Value, ids: &IdMap) -> Value {
    match layout {
        "asset" => remap_asset_slots(data, ids),
        "listOfPages" => remap_page_list(data, ids),
        _ => data.clone(),
    }
}

fn remap_asset_slots(data: &Value, ids: &IdMap) -> Value {
    let Value::Array(slots) = data else {
        return data.clone();
    };
    Value::Array(
        slots
            .iter()
            .map(|slot| {
                let Value::Object(fields) = slot else {
                    return slot.clone();
                };
                let mut rewritten = fields.clone();
                if let Some(asset) = fields
                    .get("id")
                    .and_then(json_id)
                    .and_then(|id| ids.asset(id))
                {
                    rewritten.insert("id".to_string(), Value::from(asset.0));
                }
                if let Some(page) = fields
                    .get("page")
                    .and_then(json_id)
                    .and_then(|id| ids.page(id))
                {
                    rewritten.insert("page".to_string(), Value::from(page.0));
                }
                Value::Object(rewritten)
            })
            .collect(),
    )
}

fn remap_page_list(data: &Value, ids: &IdMap) -> Value {
    let Value::Object(fields) = data else {
        return data.clone();
    };
    let Some(Value::String(pagelist)) = fields.get("pagelist") else {
        return data.clone();
    };
    let Ok(nodes) = serde_json::from_str::<Value>(pagelist) else {
        return data.clone();
    };
    let mut rewritten = fields.clone();
    rewritten.insert(
        "pagelist".to_string(),
        Value::String(remap_page_nodes(&nodes, ids).to_string()),
    );
    Value::Object(rewritten)
}

fn remap_page_nodes(nodes: &Value, ids: &IdMap) -> Value {
    let Value::Array(items) = nodes else {
        return nodes.clone();
    };
    Value::Array(
        items
            .iter()
            .map(|node| remap_page_node(node, ids))
            .collect(),
    )
}

fn remap_page_node(node: &Value, ids: &IdMap) -> Value {
    let Value::Object(fields) = node else {
        return node.clone();
    };
    let mut rewritten = fields.clone();
    if let Some(Value::Array(children)) = fields.get("children") {
        rewritten.insert(
            "children".to_string(),
            Value::Array(
                children
                    .iter()
                    .map(|child| remap_page_node(child, ids))
                    .collect(),
            ),
        );
    }
    if let Some(Value::Object(data)) = fields.get("data") {
        let mut data = data.clone();
        if let Some(page) = data.get("id").and_then(json_id).and_then(|id| ids.page(id)) {
            data.insert("id".to_string(), Value::from(page.0));
        }
        rewritten.insert("data".to_string(), Value::Object(data));
    }
    Value::Object(rewritten)
}

/// Rewrite the `background_image_asset` in a block's `layout_data`
/// (`Form/BlockLayoutDataForm.php` and `View/Helper/AbstractLayout.php`). Every other member is
/// copied unchanged.
pub(crate) fn remap_block_layout_data(layout_data: Option<&Value>, ids: &IdMap) -> Option<Value> {
    let data = layout_data?;
    let Value::Object(fields) = data else {
        return Some(data.clone());
    };
    let mut rewritten = fields.clone();
    if let Some(asset) = fields
        .get("background_image_asset")
        .and_then(json_id)
        .and_then(|id| ids.asset(id))
    {
        rewritten.insert("background_image_asset".to_string(), Value::from(asset.0));
    }
    Some(Value::Object(rewritten))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page_map(pairs: &[(u64, u64)]) -> IdMap {
        let mut ids = IdMap::default();
        for (source, target) in pairs {
            ids.insert_page(*source, PageId(*target));
        }
        ids
    }

    #[test]
    fn navigation_page_ids_follow_the_page_map() {
        let ids = page_map(&[(10, 1), (11, 2)]);
        let navigation = serde_json::json!([
            {"type": "page", "links": [], "data": {"id": 10, "label": null}},
            {"type": "browse", "data": {"query": "x"}},
            {"type": "page", "links": [
                {"type": "page", "data": {"id": "11"}}
            ], "data": {"id": 99, "label": "gone"}}
        ]);

        let rewritten = remap_navigation(&navigation, &ids);

        assert_eq!(rewritten[0]["data"]["id"], serde_json::json!(1));
        assert_eq!(
            rewritten[1]["data"]["query"],
            serde_json::json!("x"),
            "a non-page link keeps its data"
        );
        assert_eq!(
            rewritten[2]["links"][0]["data"]["id"],
            serde_json::json!(2),
            "a nested page link is rewritten"
        );
        assert_eq!(
            rewritten[2]["data"]["id"],
            serde_json::json!(99),
            "an unmapped page id stays as the source wrote it"
        );
    }

    #[test]
    fn a_non_array_navigation_is_copied_unchanged() {
        let ids = page_map(&[]);
        let navigation = serde_json::json!({"type": "page"});
        assert_eq!(remap_navigation(&navigation, &ids), navigation);
    }

    #[test]
    fn a_resource_without_its_owner_maps_to_nothing() {
        // A dry run fills the map with the identity; an owner absent from the source is a skip.
        let ids = IdMap::default();
        let row = ResourceRow {
            id: ResourceId(7),
            owner_id: Some(UserId(3)),
            resource_class_id: None,
            resource_template_id: None,
            thumbnail_id: None,
            title: Some("T".to_string()),
            is_public: true,
            created: "2024-01-01T00:00:00".parse().expect("a datetime"),
            modified: None,
            resource_type: "Omeka\\Entity\\Item".to_string(),
        };
        assert!(new_resource(&row, &ids).is_none(), "owner 3 has no target");
    }

    #[test]
    fn a_value_keeps_its_literal_and_loses_only_the_annotation() {
        let mut ids = IdMap::default();
        ids.insert_resource(4, ResourceId(1));
        ids.insert_property(9, PropertyId(2));
        let row = ValueRow {
            id: omeka_db::ids::ValueId(5),
            resource_id: ResourceId(4),
            property_id: PropertyId(9),
            value_resource_id: None,
            value_annotation_id: Some(omeka_db::ids::AnnotationId(6)),
            r#type: "literal".to_string(),
            lang: Some("en".to_string()),
            value: Some("Bronze".to_string()),
            uri: None,
            is_public: true,
        };
        let mapped = new_value(&row, &ids).expect("the value maps");
        assert_eq!(mapped.resource_id, ResourceId(1));
        assert_eq!(mapped.property_id, PropertyId(2));
        assert_eq!(mapped.value.as_deref(), Some("Bronze"));
        assert_eq!(
            mapped.value_annotation_id, None,
            "the annotation is dropped"
        );
    }

    #[test]
    fn block_asset_and_page_ids_follow_their_maps() {
        let mut ids = IdMap::default();
        ids.insert_asset(77, AssetId(1));
        ids.insert_page(40, PageId(1));
        let data = serde_json::json!([
            {"id": 77, "page": 40, "caption": "Logo"},
            {"id": "", "page": ""}
        ]);

        let rewritten = remap_block_data("asset", &data, &ids);

        assert_eq!(rewritten[0]["id"], serde_json::json!(1));
        assert_eq!(rewritten[0]["page"], serde_json::json!(1));
        assert_eq!(rewritten[0]["caption"], serde_json::json!("Logo"));
        assert_eq!(
            rewritten[1]["id"],
            serde_json::json!(""),
            "an empty slot stays empty"
        );

        let layout_data = serde_json::json!({
            "background_image_asset": 77,
            "background_color": "#ffffff"
        });
        let rewritten =
            remap_block_layout_data(Some(&layout_data), &ids).expect("layout data maps");
        assert_eq!(rewritten["background_image_asset"], serde_json::json!(1));
        assert_eq!(rewritten["background_color"], serde_json::json!("#ffffff"));
    }

    #[test]
    fn list_of_pages_ids_follow_the_page_map() {
        let ids = page_map(&[(40, 1), (41, 2)]);
        let data = serde_json::json!({
            "pagelist": "[{\"text\":\"About\",\"data\":{\"id\":40},\
                \"children\":[{\"text\":\"Sub\",\"data\":{\"id\":\"41\"}}]}]"
        });

        let rewritten = remap_block_data("listOfPages", &data, &ids);
        let nodes: Value =
            serde_json::from_str(rewritten["pagelist"].as_str().expect("a pagelist string"))
                .expect("the pagelist is JSON");

        assert_eq!(nodes[0]["data"]["id"], serde_json::json!(1));
        assert_eq!(nodes[0]["children"][0]["data"]["id"], serde_json::json!(2));
        assert_eq!(nodes[0]["text"], serde_json::json!("About"));
    }

    #[test]
    fn another_layouts_data_is_copied_unchanged() {
        let ids = IdMap::default();
        let data = serde_json::json!({"html": "<p>x</p>"});
        assert_eq!(remap_block_data("html", &data, &ids), data);
    }
}
