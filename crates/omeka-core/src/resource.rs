//! Resources: the subject of an RDF triple.
//!
//! Mirrors `application/src/Entity/Resource.php`, `Item.php`, `ItemSet.php`,
//! `Media.php` and the `resource`/`item`/`item_set`/`media` tables in
//! `application/data/install/schema.sql`.

use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

use crate::ids::{
    AnnotationId, AssetId, ClassId, ItemId, ItemSetId, MediaId, ResourceId, TemplateId, UserId,
};
use crate::value::LangTag;

/// A `DATETIME` column value, as MySQL stores it upstream.
///
/// Upstream has no timezone on these columns (`resource.created`, `site.created`,
/// `media.modified` and the rest are Doctrine `datetime`), so the port keeps a
/// naive timestamp and leaves any zone to the API layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp(NaiveDateTime);

const DATETIME_FORMAT: &str = "%Y-%m-%d %H:%M:%S";

impl Timestamp {
    /// Wrap a naive date and time.
    #[must_use]
    pub const fn from_naive(value: NaiveDateTime) -> Self {
        Self(value)
    }

    /// The naive date and time.
    #[must_use]
    pub const fn as_naive(self) -> NaiveDateTime {
        self.0
    }
}

impl std::fmt::Display for Timestamp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.format(DATETIME_FORMAT))
    }
}

impl Serialize for Timestamp {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0.format(DATETIME_FORMAT).to_string())
    }
}

impl<'de> Deserialize<'de> for Timestamp {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        NaiveDateTime::parse_from_str(&raw, DATETIME_FORMAT)
            .map(Self)
            .map_err(serde::de::Error::custom)
    }
}

/// The three concrete resource types upstream stores in `resource.resource_type`
/// (`application/config/module.config.php:76` `resource_discriminator_map`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ResourceKind {
    /// `Omeka\Entity\Item`
    Item,
    /// `Omeka\Entity\ItemSet`
    ItemSet,
    /// `Omeka\Entity\Media`
    Media,
}

impl ResourceKind {
    /// The value of the `resource_type` discriminator column.
    #[must_use]
    pub const fn discriminator(self) -> &'static str {
        match self {
            Self::Item => "Omeka\\Entity\\Item",
            Self::ItemSet => "Omeka\\Entity\\ItemSet",
            Self::Media => "Omeka\\Entity\\Media",
        }
    }

    /// The resource name the API uses (`Resource::getResourceName`).
    #[must_use]
    pub const fn api_name(self) -> &'static str {
        match self {
            Self::Item => "items",
            Self::ItemSet => "item_sets",
            Self::Media => "media",
        }
    }
}

/// The shared `resource` table columns, parsed into one struct
/// (`application/src/Entity/Resource.php`, `schema.sql:143`).
///
/// A resource title cannot be an empty string or whitespace; `Resource::setTitle`
/// trims and nulls it, and [`ResourceCore::with_title`] does the same.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceCore {
    /// `resource.id`.
    pub id: ResourceId,
    /// `resource.resource_type`, the discriminator.
    pub kind: ResourceKind,
    /// `resource.owner_id`. Nullable upstream: the column is `ON DELETE SET NULL`
    /// and the API can set the owner to null (`AbstractEntityAdapter::hydrateOwner`).
    pub owner: Option<UserId>,
    /// `resource.resource_class_id`.
    pub class: Option<ClassId>,
    /// `resource.resource_template_id`.
    pub template: Option<TemplateId>,
    /// `resource.title`, trimmed, never empty.
    pub title: Option<String>,
    /// `resource.is_public`.
    pub is_public: bool,
    /// `resource.created`.
    pub created: Timestamp,
    /// `resource.modified`, nullable until the first update.
    pub modified: Option<Timestamp>,
    /// `resource.thumbnail_id`.
    pub thumbnail: Option<AssetId>,
}

impl ResourceCore {
    /// A public resource with no class, template, title, thumbnail or modification.
    #[must_use]
    pub const fn new(id: ResourceId, kind: ResourceKind, created: Timestamp) -> Self {
        Self {
            id,
            kind,
            owner: None,
            class: None,
            template: None,
            title: None,
            is_public: true,
            created,
            modified: None,
            thumbnail: None,
        }
    }

    /// Set the owner.
    #[must_use]
    pub const fn with_owner(mut self, owner: UserId) -> Self {
        self.owner = Some(owner);
        self
    }

    /// Set the title with `Resource::setTitle` semantics: trim, and treat an
    /// empty result as no title.
    #[must_use]
    pub fn with_title(mut self, title: impl AsRef<str>) -> Self {
        let trimmed = title.as_ref().trim();
        self.title = (!trimmed.is_empty()).then(|| trimmed.to_string());
        self
    }

    /// Set the class and template.
    #[must_use]
    pub const fn with_template(mut self, class: Option<ClassId>, template: TemplateId) -> Self {
        self.class = class;
        self.template = Some(template);
        self
    }
}

/// An item (`application/src/Entity/Item.php`).
///
/// `primary_media_id` lives on the item row; item set and site membership live in
/// the `item_item_set` and `item_site` join tables (`schema.sql:47,62`) and
/// belong to the repository layer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Item {
    /// The shared resource columns.
    pub core: ResourceCore,
    /// `item.primary_media_id`.
    pub primary_media: Option<MediaId>,
}

impl Item {
    /// An item with no primary media.
    #[must_use]
    pub const fn new(id: ItemId, created: Timestamp) -> Self {
        Self {
            core: ResourceCore::new(ResourceId::from_item(id), ResourceKind::Item, created),
            primary_media: None,
        }
    }
}

/// An item set (`application/src/Entity/ItemSet.php`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemSet {
    /// The shared resource columns.
    pub core: ResourceCore,
    /// `item_set.is_open`, default false.
    pub is_open: bool,
}

impl ItemSet {
    /// A closed item set.
    #[must_use]
    pub const fn new(id: ItemSetId, created: Timestamp) -> Self {
        Self {
            core: ResourceCore::new(
                ResourceId::from_item_set(id),
                ResourceKind::ItemSet,
                created,
            ),
            is_open: false,
        }
    }
}

/// A media row (`application/src/Entity/Media.php`, `schema.sql:85`).
///
/// `media.data` is JSON whose shape depends on the ingester and renderer. The
/// typed `MediaData` enum belongs to `omeka-media` (DESIGN.md "Media"), so core
/// carries the identity and file facts that every media row has, not the payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Media {
    /// The shared resource columns.
    pub core: ResourceCore,
    /// `media.item_id`, not null (`schema.sql:87`).
    pub item: ItemId,
    /// `media.ingester`.
    pub ingester: String,
    /// `media.renderer`.
    pub renderer: String,
    /// `media.source`.
    pub source: Option<String>,
    /// `media.media_type`.
    pub media_type: Option<String>,
    /// `media.storage_id`, the file name on disk without extension.
    pub storage_id: Option<String>,
    /// `media.extension`.
    pub extension: Option<String>,
    /// `media.sha256`, a 64 character hex digest.
    pub sha256: Option<String>,
    /// `media.size` in bytes.
    pub size: Option<i64>,
    /// `media.has_original`.
    pub has_original: bool,
    /// `media.has_thumbnails`.
    pub has_thumbnails: bool,
    /// `media.position` within the item.
    pub position: Option<i64>,
    /// `media.alt_text`.
    pub alt_text: Option<String>,
    /// `media.lang`, validated like a value's language.
    pub lang: Option<LangTag>,
}

impl Media {
    /// A media row with the upstream column defaults that are not nullable.
    #[must_use]
    pub const fn new(
        id: MediaId,
        item: ItemId,
        ingester: String,
        renderer: String,
        created: Timestamp,
    ) -> Self {
        Self {
            core: ResourceCore::new(ResourceId::from_media(id), ResourceKind::Media, created),
            item,
            ingester,
            renderer,
            source: None,
            media_type: None,
            storage_id: None,
            extension: None,
            sha256: None,
            size: None,
            has_original: false,
            has_thumbnails: false,
            position: None,
            alt_text: None,
            lang: None,
        }
    }

    /// `Media::getFilename`: storage id plus `.` plus extension, either part missing
    /// yields nothing.
    #[must_use]
    pub fn filename(&self) -> Option<String> {
        let storage_id = self.storage_id.as_deref()?;
        Some(match self.extension.as_deref() {
            Some(extension) => format!("{storage_id}.{extension}"),
            None => storage_id.to_string(),
        })
    }
}

/// The `resource.id` of a value annotation, which is itself a resource
/// (`application/src/Entity/ValueAnnotation.php`, `schema.sql:372`).
#[must_use]
pub const fn annotation_resource_id(id: AnnotationId) -> ResourceId {
    ResourceId::from_annotation(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::ResourceId;

    fn at(seconds: i64) -> Timestamp {
        Timestamp::from_naive(
            chrono::DateTime::from_timestamp(1_700_000_000 + seconds, 0)
                .unwrap()
                .naive_utc(),
        )
    }

    /// `Resource::setTitle` in application/src/Entity/Resource.php: an empty or
    /// whitespace-only title becomes null, any other title is trimmed.
    #[test]
    fn resource_title_trim_matches_upstream_resource_entity() {
        let core = ResourceCore::new(ResourceId::new(1), ResourceKind::Item, at(0));
        assert_eq!(
            core.clone().with_title("  A title  ").title,
            Some("A title".into())
        );
        assert_eq!(core.clone().with_title("").title, None);
        assert_eq!(core.clone().with_title("   \n\t ").title, None);
    }

    /// The discriminator values are the keys of `resource_discriminator_map`
    /// (application/config/module.config.php:76).
    #[test]
    fn resource_kind_discriminators_match_upstream_map() {
        assert_eq!(ResourceKind::Item.discriminator(), "Omeka\\Entity\\Item");
        assert_eq!(
            ResourceKind::ItemSet.discriminator(),
            "Omeka\\Entity\\ItemSet"
        );
        assert_eq!(ResourceKind::Media.discriminator(), "Omeka\\Entity\\Media");
    }

    /// `getResourceName` on the three entities: 'items', 'item_sets', 'media'.
    #[test]
    fn resource_kind_api_names_match_upstream_resource_names() {
        assert_eq!(ResourceKind::Item.api_name(), "items");
        assert_eq!(ResourceKind::ItemSet.api_name(), "item_sets");
        assert_eq!(ResourceKind::Media.api_name(), "media");
    }

    /// Constructors pin the kind, so an item can never carry the media kind.
    #[test]
    fn item_item_set_media_constructors_pin_the_kind() {
        assert_eq!(
            Item::new(ItemId::new(3), at(0)).core.kind,
            ResourceKind::Item
        );
        assert_eq!(
            ItemSet::new(ItemSetId::new(4), at(0)).core.kind,
            ResourceKind::ItemSet
        );
        let media = Media::new(
            MediaId::new(5),
            ItemId::new(3),
            "upload".into(),
            "file".into(),
            at(0),
        );
        assert_eq!(media.core.kind, ResourceKind::Media);
        assert_eq!(media.core.id, ResourceId::new(5));
    }

    /// `ItemSet::$isOpen = false` in application/src/Entity/ItemSet.php.
    #[test]
    fn item_set_defaults_match_upstream_entity() {
        assert!(!ItemSet::new(ItemSetId::new(1), at(0)).is_open);
    }

    /// `Media::getFilename`.
    #[test]
    fn media_filename_matches_upstream_media_entity() {
        let mut media = Media::new(
            MediaId::new(1),
            ItemId::new(1),
            "upload".into(),
            "file".into(),
            at(0),
        );
        assert_eq!(media.filename(), None);
        media.storage_id = Some("abc123".into());
        assert_eq!(media.filename(), Some("abc123".into()));
        media.extension = Some("jpg".into());
        assert_eq!(media.filename(), Some("abc123.jpg".into()));
    }

    /// `has_original` and `has_thumbnails` are NOT NULL with no default upstream
    /// (`schema.sql:97`); the entity initialises both false.
    #[test]
    fn media_defaults_match_upstream_media_entity() {
        let media = Media::new(
            MediaId::new(1),
            ItemId::new(1),
            "upload".into(),
            "file".into(),
            at(0),
        );
        assert!(!media.has_original);
        assert!(!media.has_thumbnails);
        assert_eq!(media.position, None);
        assert_eq!(media.lang, None);
    }

    /// A timestamp round-trips as the MySQL `DATETIME` string the columns hold.
    #[test]
    fn timestamp_uses_upstream_datetime_format() {
        let stamp = at(0);
        let json = serde_json::to_string(&stamp).unwrap();
        assert_eq!(json, "\"2023-11-14 22:13:20\"");
        assert_eq!(serde_json::from_str::<Timestamp>(&json).unwrap(), stamp);
        assert_eq!(stamp.to_string(), "2023-11-14 22:13:20");
        assert!(serde_json::from_str::<Timestamp>("\"2023-11-14T22:13:20\"").is_err());
    }
}
