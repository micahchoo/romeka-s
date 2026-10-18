//! Wire types for the `/api` JSON-LD documents.
//!
//! Wave 1 defines these here instead of reusing `omeka-core`: this unit must
//! not depend on `omeka-core` (brief), so every struct is a mirror of the
//! upstream Representation class named on it. Wave 2 replaces them with core
//! types through `From` conversions.
//!
//! Field order in every struct is the insertion order the PHP class produces,
//! not alphabetical. `serde_json` therefore emits upstream key order.

pub mod api_resource;
pub mod asset;
pub mod common;
pub mod data_type;
pub mod item;
pub mod item_set;
pub mod job;
pub mod media;
pub mod module;
pub mod property;
pub mod resource_class;
pub mod resource_template;
pub mod site;
pub mod site_page;
pub mod user;
pub mod value_annotation;
pub mod vocabulary;

pub use common::{
    ApiUrl, JsonLdDate, JsonLdType, LiteralType, NavLink, ResourceRef, ResourceValueType,
    ReverseRef, ThumbnailDisplayUrls, ThumbnailUrls, UriType, Value, ValueLiteral, ValueMap,
    ValueResource, ValueUri, XSD_DATETIME, XsdDateTime,
};
