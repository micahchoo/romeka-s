//! The Romeka S command line and job runner.
//!
//! Commands mirror the upstream admin workflows that have a CLI meaning:
//! `install` (the install tasks), `user` (accounts), `key` (API keys),
//! `import` (templates and CSV items), `backup`/`restore` (the database and the
//! media store) and `serve` (the HTTP server). Jobs mirror
//! `application/src/Job/*`; see [`jobs`].
//!
//! Persistence goes through `omeka-db` repositories. The four tables those
//! repositories do not cover (`setting`, `job`, `fulltext_search`, `item_site`)
//! are reached through [`store`], which documents why and what would retire it.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod archive;
pub mod commands;
pub mod csv;
pub mod error;
pub mod jobs;
pub mod store;
pub mod tar;
pub mod terms;
pub mod vocabularies;

#[cfg(test)]
mod testing;

pub use error::CliError;
