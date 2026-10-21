//! The `/api` REST layer of Romeka S: wire types, routing rules, the
//! read endpoints that turn `omeka-db` rows into JSON-LD documents, and the
//! write endpoints (`POST`, `PUT`, `PATCH`, `DELETE`) that hydrate, persist or
//! remove them.
//!
//! `omeka-server` binds the HTTP surface and turns these decisions into
//! responses; [`read::Api`] answers one request against a `DbPool`. Write
//! operations the port does not build stay typed 501s.
//!
//! Upstream references are to `source/omeka-s` at the frozen SHA; module
//! docs name the file each rule comes from.

pub mod adapters;
pub mod annotation;
pub mod auth;
pub mod auth_cache;
pub mod derivatives;
pub mod errors;
pub mod import;
pub mod jsonld;
pub mod limits;
pub mod login;
pub mod media_add;
pub mod messages;
pub mod origin;
pub mod page_history;
pub mod password;
pub mod publish;
pub mod read;
pub mod routes;
pub mod session;
pub mod settings;
pub mod site_items;
pub mod site_settings;
pub mod tagging;
pub mod theme_settings;
pub mod token;
pub mod uploads;
pub mod vocab_import;
pub mod wire;
pub mod write;

#[cfg(test)]
pub(crate) mod testing;
