//! The API adapter extension point: the vocabulary a plugin speaks and the
//! trait it implements.
//!
//! Upstream resolves `/api/{resource}` through `Api\Manager` and one adapter
//! per resource, registered under `api_adapters` in
//! `application/config/module.config.php`. The port compiles those seventeen
//! adapters in. This module is where a third party adds an eighteenth without
//! editing `omeka-api`: an [`ApiAdapter`] supplies an [`ApiAdapterSpec`] and
//! serves the reads the spec declares.
//!
//! `omeka-api` re-exports [`Operation`] as `routes::Operation` and
//! [`ApiAdapterSpec`] as `adapters::AdapterSpec`, so the port's own adapters
//! and a plugin's adapter are described by one type.

use serde_json::Value;

/// A routed HTTP method, named as `AdapterInterface` names the operation.
///
/// Defined here rather than in `omeka-api` so a plugin can declare its
/// operations without depending on the API crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operation {
    Search,
    Read,
    Create,
    Update,
    /// `ApiController::patch` issues `ApiManager::update` with `isPartial`;
    /// the port keeps it as its own verb.
    Patch,
    Delete,
}

impl Operation {
    /// The name upstream `OperationNotImplementedException` messages use.
    pub fn adapter_name(self) -> &'static str {
        match self {
            Self::Search => "search",
            Self::Read => "read",
            Self::Create => "create",
            Self::Update | Self::Patch => "update",
            Self::Delete => "delete",
        }
    }
}

/// Registration record for one API adapter (`AdapterInterface`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ApiAdapterSpec {
    /// `AdapterInterface::getResourceName`, the `:resource` path segment.
    pub name: &'static str,
    /// The upstream representation class this adapter returns.
    pub representation: &'static str,
    /// Operations the adapter implements. Anything outside this list is an
    /// `OperationNotImplementedException` upstream, which the port answers as a
    /// 500.
    pub operations: &'static [Operation],
}

/// One request a plugin adapter is asked to serve.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdapterRequest {
    /// The `:resource` segment.
    pub resource: String,
    /// The operation being served.
    pub operation: Operation,
    /// The `{id}` segment, absent on a collection request.
    pub id: Option<String>,
    /// The decoded query parameters, in arrival order.
    pub query: Vec<(String, String)>,
}

/// What a plugin adapter answers with: the JSON-LD document for the request.
///
/// The document is the plugin's own wire shape. `omeka-api` serializes it and
/// sets the JSON-LD headers, so a plugin never touches `http`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdapterReply {
    pub document: Value,
}

impl AdapterReply {
    /// A reply carrying one JSON-LD document.
    #[must_use]
    pub fn document(document: Value) -> Self {
        Self { document }
    }
}

/// An API adapter a plugin supplies.
///
/// `omeka-api` serves a request from this adapter when the resource name
/// matches, the request's operation is in [`ApiAdapterSpec::operations`], and
/// [`ApiAdapter::serve`] returns a reply. Otherwise the port answers the
/// request the way it answers any adapter it has no handler for.
///
/// A plugin adapter cannot take over a resource the port already registers:
/// [`crate::Registry::install`] refuses a spec whose name is taken.
pub trait ApiAdapter: Send + Sync {
    /// The registration record (`AdapterInterface` metadata).
    fn spec(&self) -> &'static ApiAdapterSpec;

    /// Serve one request, or return `None` to leave it to the port.
    fn serve(&self, request: &AdapterRequest) -> Option<AdapterReply>;
}
