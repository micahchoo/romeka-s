//! The `/api` route table as data.
//!
//! Upstream has one route, `api` -> `ApiController` -> an adapter looked up by
//! the `:resource` segment, plus `api-context` outside the mount point. The
//! table here expands that lookup into rows so a reviewer can count routes and
//! wave 2 can attach real handlers per row.

use std::sync::{Arc, LazyLock};

use omeka_plugin::Registry;

use crate::adapters::{self, AdapterSpec};
use crate::errors::NotImplemented;

/// Path template for the collection of one resource.
pub const COLLECTION_PATH: &str = "/api/{resource}";
/// Path template for one member of one resource.
pub const MEMBER_PATH: &str = "/api/{resource}/{id}";
/// Path of the JSON-LD context document.
pub const CONTEXT_PATH: &str = "/api-context";

/// Routed HTTP methods. `Other` covers extension methods.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Method {
    Get,
    Post,
    Put,
    Patch,
    Delete,
    Head,
    Options,
    Other,
}

impl Method {
    pub fn from_http(method: &http::Method) -> Self {
        match *method {
            http::Method::GET => Self::Get,
            http::Method::POST => Self::Post,
            http::Method::PUT => Self::Put,
            http::Method::PATCH => Self::Patch,
            http::Method::DELETE => Self::Delete,
            http::Method::HEAD => Self::Head,
            http::Method::OPTIONS => Self::Options,
            _ => Self::Other,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Patch => "PATCH",
            Self::Delete => "DELETE",
            Self::Head => "HEAD",
            Self::Options => "OPTIONS",
            Self::Other => "OTHER",
        }
    }
}

/// An adapter operation, named as `AdapterInterface` names it.
///
/// Defined in `omeka-plugin`, so a plugin adapter declares its operations with
/// the same type the route table matches on. `Patch` is a route verb rather
/// than an adapter operation: `ApiController::patch` issues `ApiManager::update`
/// with `isPartial`.
pub use omeka_plugin::Operation;

/// What a route row reaches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// The `GET /api` resource listing (`ApiController::getList`, no resource).
    ApiRoot,
    /// `ApiController::contextAction` on `/api-context`.
    ApiContext,
    Adapter {
        adapter: &'static AdapterSpec,
        operation: Operation,
    },
}

impl Target {
    /// The typed 501 body for this target.
    pub fn not_implemented(self) -> NotImplemented {
        match self {
            Self::ApiRoot => NotImplemented::endpoint("api root"),
            Self::ApiContext => NotImplemented::endpoint("api context"),
            Self::Adapter { adapter, operation } => {
                NotImplemented::adapter(adapter.name, operation)
            }
        }
    }
}

/// One row of the route table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Route {
    pub path: &'static str,
    pub method: Method,
    pub target: Target,
}

/// The complete route table for the compiled-in adapters.
pub static ROUTES: LazyLock<Vec<Route>> = LazyLock::new(|| CORE.table().to_vec());

/// The route table and the registry it was built from.
///
/// [`Routes::core`] holds the compiled-in adapters. [`Routes::new`] adds the
/// adapters a plugin installed, so one value answers routing for the whole
/// process.
pub struct Routes {
    registry: Arc<Registry>,
    table: Vec<Route>,
}

impl Routes {
    /// Routes for the compiled-in adapters and no plugins.
    #[must_use]
    pub fn core() -> Self {
        Self::new(adapters::core_registry())
    }

    /// Routes for a registry: the compiled-in adapters plus any plugin adapter
    /// it holds.
    #[must_use]
    pub fn new(registry: Arc<Registry>) -> Self {
        let table = build_table(&registry);
        Self { registry, table }
    }

    /// The registry this table was built from.
    #[must_use]
    pub fn registry(&self) -> &Registry {
        &self.registry
    }

    /// The rows, in resource-name order.
    #[must_use]
    pub fn table(&self) -> &[Route] {
        &self.table
    }

    /// Resolve a path under the `/api` mount point.
    #[must_use]
    pub fn resolve(&self, path: ApiPath<'_>, method: Method) -> Resolution {
        match path {
            ApiPath::Root => resolve_simple(method, ROOT_ALLOW, PathShape::Root, Target::ApiRoot),
            ApiPath::Collection(name) => {
                let Some(adapter) = self.adapter(name) else {
                    return Resolution::NotFound {
                        message: unknown_resource_message(name),
                    };
                };
                match method {
                    Method::Get => not_implemented_target(adapter, Operation::Search),
                    Method::Post => not_implemented_target(adapter, Operation::Create),
                    _ => Resolution::MethodNotAllowed {
                        shape: PathShape::Collection,
                        allow: COLLECTION_ALLOW,
                    },
                }
            }
            ApiPath::Member(name, _id) => {
                let Some(adapter) = self.adapter(name) else {
                    return Resolution::NotFound {
                        message: unknown_resource_message(name),
                    };
                };
                match method {
                    Method::Get => not_implemented_target(adapter, Operation::Read),
                    Method::Post => not_implemented_target(adapter, Operation::Create),
                    Method::Put => not_implemented_target(adapter, Operation::Update),
                    Method::Patch => not_implemented_target(adapter, Operation::Patch),
                    Method::Delete => not_implemented_target(adapter, Operation::Delete),
                    _ => Resolution::MethodNotAllowed {
                        shape: PathShape::Member,
                        allow: MEMBER_ALLOW,
                    },
                }
            }
        }
    }

    /// Resolve the `/api-context` route, which lives outside the `/api` mount.
    #[must_use]
    pub fn resolve_context(&self, method: Method) -> Resolution {
        resolve_simple(
            method,
            CONTEXT_ALLOW,
            PathShape::Context,
            Target::ApiContext,
        )
    }

    fn adapter(&self, name: &str) -> Option<&'static AdapterSpec> {
        self.registry.adapter(name).map(|entry| entry.spec())
    }
}

static CORE: LazyLock<Routes> = LazyLock::new(Routes::core);

fn build_table(registry: &Registry) -> Vec<Route> {
    let mut rows = vec![
        Route {
            path: "/api",
            method: Method::Get,
            target: Target::ApiRoot,
        },
        Route {
            path: CONTEXT_PATH,
            method: Method::Get,
            target: Target::ApiContext,
        },
    ];
    for entry in registry.adapters() {
        let adapter = entry.spec();
        let adapter_target = |operation| Target::Adapter { adapter, operation };
        rows.push(Route {
            path: COLLECTION_PATH,
            method: Method::Get,
            target: adapter_target(Operation::Search),
        });
        rows.push(Route {
            path: COLLECTION_PATH,
            method: Method::Post,
            target: adapter_target(Operation::Create),
        });
        rows.push(Route {
            path: MEMBER_PATH,
            method: Method::Get,
            target: adapter_target(Operation::Read),
        });
        // Laminas AbstractRestfulController maps POST to create with or
        // without an identifier.
        rows.push(Route {
            path: MEMBER_PATH,
            method: Method::Post,
            target: adapter_target(Operation::Create),
        });
        rows.push(Route {
            path: MEMBER_PATH,
            method: Method::Put,
            target: adapter_target(Operation::Update),
        });
        rows.push(Route {
            path: MEMBER_PATH,
            method: Method::Patch,
            target: adapter_target(Operation::Patch),
        });
        rows.push(Route {
            path: MEMBER_PATH,
            method: Method::Delete,
            target: adapter_target(Operation::Delete),
        });
    }
    rows
}

/// A parsed path under the API mount point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiPath<'a> {
    /// `/api`
    Root,
    /// `/api/{resource}`
    Collection(&'a str),
    /// `/api/{resource}/{id}`
    Member(&'a str, &'a str),
}

/// The path template a resolution came from, for error reporting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathShape {
    Root,
    Context,
    Collection,
    Member,
}

impl PathShape {
    pub fn template(self) -> &'static str {
        match self {
            Self::Root => "/api",
            Self::Context => CONTEXT_PATH,
            Self::Collection => COLLECTION_PATH,
            Self::Member => MEMBER_PATH,
        }
    }
}

/// What to do with one request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolution {
    NotImplemented {
        target: Target,
    },
    MethodNotAllowed {
        shape: PathShape,
        allow: &'static [Method],
    },
    NotFound {
        message: String,
    },
}

/// Methods the collection path accepts (`ApiController` + Laminas defaults).
pub static COLLECTION_ALLOW: &[Method] = &[Method::Get, Method::Post];
/// Methods the member path accepts.
pub static MEMBER_ALLOW: &[Method] = &[
    Method::Get,
    Method::Post,
    Method::Put,
    Method::Patch,
    Method::Delete,
];
/// Methods `/api` accepts.
pub static ROOT_ALLOW: &[Method] = &[Method::Get];
/// Methods `/api-context` accepts.
pub static CONTEXT_ALLOW: &[Method] = &[Method::Get];

/// Resolve a path under the `/api` mount point against the compiled-in
/// adapters. Callers that hold a plugin registry use [`Routes::resolve`].
#[must_use]
pub fn resolve(path: ApiPath<'_>, method: Method) -> Resolution {
    CORE.resolve(path, method)
}

/// Resolve the `/api-context` route against the compiled-in adapters.
#[must_use]
pub fn resolve_context(method: Method) -> Resolution {
    CORE.resolve_context(method)
}

fn resolve_simple(
    method: Method,
    allow: &'static [Method],
    shape: PathShape,
    target: Target,
) -> Resolution {
    if allow.contains(&method) {
        Resolution::NotImplemented { target }
    } else {
        Resolution::MethodNotAllowed { shape, allow }
    }
}

fn not_implemented_target(adapter: &'static AdapterSpec, operation: Operation) -> Resolution {
    Resolution::NotImplemented {
        target: Target::Adapter { adapter, operation },
    }
}

/// The upstream sentence from `Api\Manager::execute`.
fn unknown_resource_message(name: &str) -> String {
    format!("The API does not support the \"{name}\" resource.")
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use crate::errors::ApiError;

    #[test]
    fn table_has_two_rows_per_collection_and_five_per_member() {
        let rows = ROUTES.len();
        let adapters = adapters::ADAPTERS.len();
        assert_eq!(rows, 2 + adapters * 7);
        // 2 + 18 * 7
        assert_eq!(rows, 128);
    }

    #[test]
    fn every_table_row_resolves_to_its_target() {
        for route in ROUTES.iter() {
            let resource = match route.target {
                Target::Adapter { adapter, .. } => adapter.name,
                _ => "items",
            };
            let resolution = match route.path {
                "/api" => resolve(ApiPath::Root, route.method),
                CONTEXT_PATH => resolve_context(route.method),
                COLLECTION_PATH => resolve(ApiPath::Collection(resource), route.method),
                MEMBER_PATH => resolve(ApiPath::Member(resource, "1"), route.method),
                other => panic!("unexpected path template {other}"),
            };
            match (route.target, resolution) {
                (
                    Target::ApiRoot,
                    Resolution::NotImplemented {
                        target: Target::ApiRoot,
                    },
                ) => {}
                (
                    Target::ApiContext,
                    Resolution::NotImplemented {
                        target: Target::ApiContext,
                    },
                ) => {}
                (
                    Target::Adapter {
                        adapter: want,
                        operation: want_op,
                    },
                    Resolution::NotImplemented {
                        target:
                            Target::Adapter {
                                adapter: got,
                                operation: got_op,
                            },
                    },
                ) => {
                    assert_eq!(want.name, got.name);
                    assert_eq!(want_op, got_op);
                }
                (_, resolution) => panic!("row {route:?} resolved to {resolution:?}"),
            }
        }
    }

    #[test]
    fn unknown_resource_is_a_404_with_the_manager_message() {
        let resolution = resolve(ApiPath::Collection("nonsense"), Method::Post);
        let error = ApiError::from(resolution);
        assert_eq!(error.status(), http::StatusCode::NOT_FOUND);
        assert_eq!(
            error.document().errors,
            crate::errors::ErrorPayload::General(crate::errors::GeneralError {
                error: "The API does not support the \"nonsense\" resource.".to_string(),
            })
        );
    }

    #[test]
    fn put_on_a_collection_is_405_with_the_collection_allow_list() {
        let resolution = resolve(ApiPath::Collection("items"), Method::Put);
        assert_eq!(
            resolution,
            Resolution::MethodNotAllowed {
                shape: PathShape::Collection,
                allow: COLLECTION_ALLOW
            }
        );
    }

    #[test]
    fn head_on_a_member_is_405() {
        let resolution = resolve(ApiPath::Member("items", "1"), Method::Head);
        assert_eq!(
            resolution,
            Resolution::MethodNotAllowed {
                shape: PathShape::Member,
                allow: MEMBER_ALLOW
            }
        );
    }

    #[test]
    fn post_on_a_member_creates_like_laminas() {
        let resolution = resolve(ApiPath::Member("items", "1"), Method::Post);
        match resolution {
            Resolution::NotImplemented {
                target: Target::Adapter { adapter, operation },
            } => {
                assert_eq!(adapter.name, "items");
                assert_eq!(operation, Operation::Create);
            }
            other => panic!("expected create, got {other:?}"),
        }
    }

    #[test]
    fn operation_adapter_names_follow_upstream_messages() {
        assert_eq!(Operation::Search.adapter_name(), "search");
        assert_eq!(Operation::Patch.adapter_name(), "update");
        assert_eq!(Operation::Delete.adapter_name(), "delete");
    }
}
