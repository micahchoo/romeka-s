//! The Omeka S domain core: typed entities, value rules, the data-type
//! registry and the permission matrix, ported from upstream PHP.
//!
//! Upstream reference: `source/omeka-s/application/src` (read only). Each
//! module mirrors a family of upstream files and cites them in doc comments:
//!
//! - [`ids`]: branded primary keys, one newtype per upstream table.
//! - [`value`]: `value` rows, the three data shapes, language tags and URI
//!   validation (`Entity/Value.php`, `Api/Adapter/ValueHydrator.php`).
//! - [`resource`]: `resource`, `item`, `item_set` and `media`
//!   (`Entity/Resource.php`, `Item.php`, `ItemSet.php`, `Media.php`).
//! - [`vocabulary`]: `vocabulary`, `resource_class`, `property`
//!   (`Entity/Vocabulary.php`, `ResourceClass.php`, `Property.php`).
//! - [`template`]: `resource_template`, `resource_template_property`
//!   (`Entity/ResourceTemplate.php`, `ResourceTemplateProperty.php`).
//! - [`site`]: sites, pages, blocks, slugs and block layouts
//!   (`Entity/Site.php`, `SitePage.php`, `SitePageBlock.php`,
//!   `Site/BlockLayout/*`, `Api/Adapter/SiteSlugTrait.php`).
//! - [`datatypes`]: the data-type registry and conversion targets
//!   (`DataType/*`, `Service/DataType/Database.php`).
//! - [`permissions`]: the ACL rules (`Service/AclFactory.php`,
//!   `Permissions/Acl.php`, `Permissions/Assertion/*`).
//!
//! The crate is pure: no I/O, no async, no database. Repository and API
//! crates in wave 2 converge onto these types; validation lives at their
//! boundaries (HTTP parse, DB row parse, block JSON parse), so the types here
//! assume well-formed input.
//!
//! # Illegal states
//!
//! Where upstream guarantees a column is non-null for a kind, the port uses a
//! plain field, not an `Option`. `ValueData` makes "exactly one of
//! `value`/`uri`/`value_resource_id`" a type rather than a comment. Ids are
//! branded per table: mixing `ItemId` and `UserId` is a compile error, which
//! the module docs on [`ids`] demonstrate by construction.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod datatypes;
pub mod ids;
pub mod permissions;
pub mod resource;
pub mod site;
pub mod template;
pub mod value;
pub mod vocabulary;
