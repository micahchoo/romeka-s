//! Table ids, one newtype per table, over the upstream auto-increment integer.
//!
//! Branding keeps `site.get(page_id)` from type checking. The values are the upstream ids, so a
//! row read by this crate converts to a core id with `core::SiteId::from(row.id.0)` in wave 2.
//! `api_key`, `module`, `session`, `setting` and `migration` use upstream string keys and are
//! therefore plain `String` in their row structs, not newtypes here.

macro_rules! define_id {
    ($name:ident, $table:literal) => {
        #[doc = concat!("`", $table, "` row id (`", $table, ".id`).")]
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(pub u64);

        impl From<u64> for $name {
            fn from(value: u64) -> Self {
                Self(value)
            }
        }

        impl From<$name> for u64 {
            fn from(value: $name) -> Self {
                value.0
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.0.fmt(f)
            }
        }
    };
}

define_id!(UserId, "user");
define_id!(VocabularyId, "vocabulary");
define_id!(ClassId, "resource_class");
define_id!(PropertyId, "property");
define_id!(TemplateId, "resource_template");
define_id!(TemplatePropertyId, "resource_template_property");
define_id!(AssetId, "asset");
define_id!(ResourceId, "resource");
define_id!(ItemId, "item");
define_id!(ItemSetId, "item_set");
define_id!(MediaId, "media");
define_id!(ValueId, "value");
define_id!(AnnotationId, "value_annotation");
define_id!(SiteId, "site");
define_id!(PageId, "site_page");
define_id!(BlockId, "site_page_block");
define_id!(AttachmentId, "site_block_attachment");
define_id!(SiteItemSetId, "site_item_set");
define_id!(SitePermissionId, "site_permission");
define_id!(JobId, "job");
define_id!(CustomVocabId, "custom_vocab");
define_id!(PublicationId, "romeka_publication");
define_id!(ImportId, "romeka_import");
define_id!(BulkId, "romeka_bulk");

impl ItemId {
    /// The `resource` row this item shares its id with (`item.id` references `resource.id`).
    pub fn as_resource(self) -> ResourceId {
        ResourceId(self.0)
    }
}

impl ItemSetId {
    /// The `resource` row this item set shares its id with.
    pub fn as_resource(self) -> ResourceId {
        ResourceId(self.0)
    }
}

impl MediaId {
    /// The `resource` row this media shares its id with.
    pub fn as_resource(self) -> ResourceId {
        ResourceId(self.0)
    }
}

impl AnnotationId {
    /// The `resource` row this value annotation shares its id with.
    pub fn as_resource(self) -> ResourceId {
        ResourceId(self.0)
    }
}
