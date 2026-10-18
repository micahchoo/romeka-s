//! Branded ids for every upstream table that the port addresses by primary key.
//!
//! One newtype per table, over `u64`, matching upstream auto-increment columns
//! (`application/data/install/schema.sql`). Two ids of different kinds never mix:
//! passing an `ItemId` where a `ResourceId` is required is a type error, not a
//! runtime assertion (DESIGN.md "Data shapes (core)", fidelity rule "no
//! cross-kind id mixups").
//!
//! `ResourceId` names the `resource` table, the shared primary key of `item`,
//! `item_set`, `media` and `value_annotation` (upstream `JOINED` inheritance,
//! `application/src/Entity/Resource.php`). The conversions between those ids and
//! `ResourceId` are explicit and named, so widening is deliberate.

use std::fmt;

use serde::{Deserialize, Serialize};

macro_rules! id_newtype {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(u64);

        impl $name {
            /// Wrap a raw primary key.
            #[must_use]
            pub const fn new(id: u64) -> Self {
                Self(id)
            }

            /// The raw primary key, for SQL and JSON-LD output.
            #[must_use]
            pub const fn get(self) -> u64 {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Display::fmt(&self.0, f)
            }
        }
    };
}

id_newtype!(
    /// `user.id`
    UserId
);
id_newtype!(
    /// `item.id`, which is also `resource.id` for an item
    ItemId
);
id_newtype!(
    /// `item_set.id`, which is also `resource.id` for an item set
    ItemSetId
);
id_newtype!(
    /// `media.id`, which is also `resource.id` for a media row
    MediaId
);
id_newtype!(
    /// `resource.id`, the shared primary key of item/item_set/media/value_annotation
    ResourceId
);
id_newtype!(
    /// `vocabulary.id`
    VocabularyId
);
id_newtype!(
    /// `resource_class.id`
    ClassId
);
id_newtype!(
    /// `property.id`
    PropertyId
);
id_newtype!(
    /// `resource_template.id`
    TemplateId
);
id_newtype!(
    /// `site.id`
    SiteId
);
id_newtype!(
    /// `site_page.id`
    PageId
);
id_newtype!(
    /// `site_page_block.id`
    BlockId
);
id_newtype!(
    /// `job.id`
    JobId
);
id_newtype!(
    /// `asset.id`
    AssetId
);
id_newtype!(
    /// `value.id`
    ValueId
);
id_newtype!(
    /// `value_annotation.id`, which is also `resource.id` for an annotation
    AnnotationId
);
id_newtype!(
    /// `api_key.id`, a 32 character hex string upstream, kept as a numeric handle here
    KeyId
);

impl ResourceId {
    /// An item's id is its resource id (`item.id -> resource.id`).
    #[must_use]
    pub const fn from_item(id: ItemId) -> Self {
        Self(id.0)
    }

    /// An item set's id is its resource id (`item_set.id -> resource.id`).
    #[must_use]
    pub const fn from_item_set(id: ItemSetId) -> Self {
        Self(id.0)
    }

    /// A media row's id is its resource id (`media.id -> resource.id`).
    #[must_use]
    pub const fn from_media(id: MediaId) -> Self {
        Self(id.0)
    }

    /// A value annotation's id is its resource id (`value_annotation.id -> resource.id`).
    #[must_use]
    pub const fn from_annotation(id: AnnotationId) -> Self {
        Self(id.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Upstream ids round-trip through serde as bare numbers: every id in the
    /// REST envelope is `o:id` from the same auto-increment column
    /// (application/src/Api/Representation/AbstractRepresentation.php).
    #[test]
    fn id_serde_matches_upstream_numeric_o_id() {
        let item = ItemId::new(42);
        let json = serde_json::to_string(&item).unwrap();
        assert_eq!(json, "42");
        let back: ItemId = serde_json::from_str(&json).unwrap();
        assert_eq!(back, item);
    }

    #[test]
    fn id_display_is_the_raw_key() {
        assert_eq!(ItemId::new(7).to_string(), "7");
        assert_eq!(format!("{}", SiteId::new(1000)), "1000");
    }

    #[test]
    fn ids_order_numerically() {
        assert!(ItemId::new(2) < ItemId::new(10));
        assert_eq!(ItemId::new(2).max(ItemId::new(3)), ItemId::new(3));
    }

    /// The item/item-set/media tables share `resource.id` with the resource row
    /// (application/data/install/schema.sql:44,60,108). The conversion is named.
    #[test]
    fn resource_id_from_kind_specific_ids_keeps_the_key() {
        assert_eq!(ResourceId::from_item(ItemId::new(5)), ResourceId::new(5));
        assert_eq!(
            ResourceId::from_item_set(ItemSetId::new(6)),
            ResourceId::new(6)
        );
        assert_eq!(ResourceId::from_media(MediaId::new(7)), ResourceId::new(7));
        assert_eq!(
            ResourceId::from_annotation(AnnotationId::new(8)),
            ResourceId::new(8)
        );
    }

    /// Cross-kind ids do not mix: `ItemId` is not assignable to `UserId`. The
    /// compile-fail proof lives in the crate docs (`lib.rs`).
    #[test]
    fn cross_kind_id_assignment_is_a_type_error() {}
}
