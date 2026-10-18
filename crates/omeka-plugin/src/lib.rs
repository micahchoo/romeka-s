//! omeka-plugin: the port's extension seam.
//!
//! DESIGN decision 4 rejects dynamic PHP module loading and names Rust
//! registries as the seam for block layouts, media adapters, jobs, data types
//! and API adapters. This crate is that seam: the traits a third party
//! implements, the [`Registry`] that holds the implementations, and the
//! vocabulary an API adapter speaks.
//!
//! A plugin is one type implementing [`Plugin`]. It contributes to the
//! extension points it extends, and [`Registry::install`] either takes all of
//! its contributions or none.
//!
//! ```
//! use std::sync::Arc;
//! use omeka_plugin::{ApiAdapter, AdapterReply, AdapterRequest, Plugin, RegistryBuilder};
//!
//! // An adapter a plugin serves, with the spec its resource is routed by.
//! # static SPEC: omeka_plugin::ApiAdapterSpec = omeka_plugin::ApiAdapterSpec {
//! #     name: "notes",
//! #     representation: "App\\Api\\Adapter\\NoteAdapter",
//! #     operations: &[omeka_plugin::Operation::Search],
//! # };
//! # struct Notes;
//! # impl ApiAdapter for Notes {
//! #     fn spec(&self) -> &'static omeka_plugin::ApiAdapterSpec { &SPEC }
//! #     fn serve(&self, _request: &AdapterRequest) -> Option<AdapterReply> { None }
//! # }
//! # struct NotesPlugin;
//! # impl Plugin for NotesPlugin {
//! #     fn id(&self) -> &'static str { "notes" }
//! #     fn label(&self) -> &'static str { "Notes" }
//! #     fn api_adapters(&self) -> Vec<Arc<dyn ApiAdapter>> { vec![Arc::new(Notes)] }
//! # }
//! let registry = RegistryBuilder::new().install(Arc::new(NotesPlugin)).build()?;
//! assert_eq!(registry.adapter("notes").map(|entry| entry.spec().name), Some("notes"));
//! # Ok::<(), omeka_plugin::PluginError>(())
//! ```
//!
//! Contracts are documented in `README.md`. Today `omeka-api` consumes
//! [`Plugin::api_adapters`]; the other four contribution methods are contracted
//! and nothing reads them yet.

pub mod api;
pub mod plugin;
pub mod registry;

pub use api::{AdapterReply, AdapterRequest, ApiAdapter, ApiAdapterSpec, Operation};
pub use plugin::Plugin;
pub use registry::{AdapterEntry, PluginError, Registry, RegistryBuilder};
