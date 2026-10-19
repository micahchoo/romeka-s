//! The API adapter extension point end to end.
//!
//! A plugin registers an adapter for a resource the port does not have. The
//! route table built from the registry resolves the plugin's paths, and a read
//! request reaches the plugin's handler and comes back as the plugin's
//! document. The two calls the tests make are the two `omeka-server` makes for
//! a `/api/{resource}[/{id}]` request: resolve the path through `Routes`, then
//! open an `Api` and call `search` or `read_member`.
//!
//! The last two tests pin the other half of the contract: with no plugin
//! installed the resource is unknown, exactly as it was before this seam.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use omeka_api::adapters::ADAPTERS;
use omeka_api::auth::Actor;
use omeka_api::errors::ApiError;
use omeka_api::jsonld::BaseUrl;
use omeka_api::read::{Api, Params};
use omeka_api::routes::{self, ApiPath, Method, Resolution, Routes, Target};
use omeka_db::{migrate, open_memory};
use omeka_plugin::{
    AdapterReply, AdapterRequest, ApiAdapter, ApiAdapterSpec, Operation, Plugin, Registry,
    RegistryBuilder,
};
use serde_json::{Value, json};
use sqlx::SqlitePool;

/// The resource the plugin adds, with the operations it serves.
static WIDGET_SPEC: ApiAdapterSpec = ApiAdapterSpec {
    name: "widgets",
    representation: "App\\Api\\Adapter\\WidgetAdapter",
    operations: &[Operation::Search, Operation::Read],
};

/// A plugin adapter. It answers a list and one member, declines id `0` so the
/// port's own answer is observable, and echoes the `search` parameter so the
/// test can see the query reached it.
struct Widgets;

impl ApiAdapter for Widgets {
    fn spec(&self) -> &'static ApiAdapterSpec {
        &WIDGET_SPEC
    }

    fn serve(&self, request: &AdapterRequest) -> Option<AdapterReply> {
        match request.operation {
            Operation::Search => {
                let search = request
                    .query
                    .iter()
                    .find(|(name, _)| name == "search")
                    .map(|(_, value)| value.clone());
                Some(AdapterReply::document(json!({
                    "@id": "http://localhost/api/widgets",
                    "@type": "o:Widget",
                    "o:total": 2,
                    "o:search": search,
                    "widgets": [{"o:id": 1}, {"o:id": 2}],
                })))
            }
            Operation::Read => match request.id.as_deref() {
                Some("0") | None => None,
                Some(id) => Some(AdapterReply::document(json!({
                    "@id": format!("http://localhost/api/widgets/{id}"),
                    "o:id": id,
                    "o:label": "a widget",
                }))),
            },
            _ => None,
        }
    }
}

struct WidgetsPlugin;

impl Plugin for WidgetsPlugin {
    fn id(&self) -> &'static str {
        "widgets"
    }

    fn label(&self) -> &'static str {
        "Widgets"
    }

    fn api_adapters(&self) -> Vec<Arc<dyn ApiAdapter>> {
        vec![Arc::new(Widgets)]
    }
}

/// The registry a process with the widget plugin installed builds.
fn registry() -> Arc<Registry> {
    let registry = RegistryBuilder::with_adapters(ADAPTERS)
        .install(Arc::new(WidgetsPlugin))
        .build()
        .unwrap();
    Arc::new(registry)
}

async fn pool() -> SqlitePool {
    let pool = open_memory().await.unwrap();
    migrate(&pool).await.unwrap();
    pool
}

fn base() -> BaseUrl {
    BaseUrl::parse("http://localhost").unwrap()
}

fn adapter_target(resolution: &Resolution) -> (&'static str, Operation) {
    match resolution {
        Resolution::NotImplemented {
            target: Target::Adapter { adapter, operation },
        } => (adapter.name, *operation),
        other => panic!("expected an adapter target, got {other:?}"),
    }
}

#[test]
fn the_route_table_gains_the_plugin_resource() {
    let routes = Routes::new(registry());

    assert_eq!(
        routes.table().len(),
        2 + 19 * 7,
        "eighteen core adapters plus the plugin's, each with seven member rows"
    );
    assert_eq!(routes.registry().adapter_names().len(), 19);

    assert_eq!(
        adapter_target(&routes.resolve(ApiPath::Collection("widgets"), Method::Get)),
        ("widgets", Operation::Search)
    );
    assert_eq!(
        adapter_target(&routes.resolve(ApiPath::Member("widgets", "7"), Method::Get)),
        ("widgets", Operation::Read)
    );
    assert_eq!(
        adapter_target(&routes.resolve(ApiPath::Member("widgets", "7"), Method::Delete)),
        ("widgets", Operation::Delete)
    );
    assert_eq!(
        routes.resolve(ApiPath::Collection("widgets"), Method::Put),
        Resolution::MethodNotAllowed {
            shape: omeka_api::routes::PathShape::Collection,
            allow: routes::COLLECTION_ALLOW,
        }
    );
}

#[test]
fn the_core_routes_are_unchanged_when_no_plugin_is_registered() {
    assert_eq!(routes::ROUTES.len(), 2 + 18 * 7);

    let unknown = routes::resolve(ApiPath::Collection("widgets"), Method::Get);
    match unknown {
        Resolution::NotFound { message } => assert_eq!(
            message,
            "The API does not support the \"widgets\" resource."
        ),
        other => panic!("expected a 404, got {other:?}"),
    }

    assert_eq!(
        adapter_target(&routes::resolve(ApiPath::Collection("items"), Method::Get)),
        ("items", Operation::Search)
    );
}

#[tokio::test]
async fn a_request_for_the_plugin_resource_is_served_by_the_plugin() {
    let pool = pool().await;
    let api = Api::load_with_plugins(&pool, base(), Actor::anonymous(), registry())
        .await
        .unwrap();

    let answer = api
        .search("widgets", &Params::parse("search=blue"))
        .await
        .unwrap();
    assert_eq!(answer.status, http::StatusCode::OK);
    assert_eq!(
        answer
            .headers
            .get(http::header::CONTENT_TYPE)
            .map(|value| value.to_str().unwrap()),
        Some("application/ld+json")
    );
    let document: Value = serde_json::from_str(&answer.body).unwrap();
    assert_eq!(document["o:total"], 2);
    assert_eq!(document["widgets"].as_array().unwrap().len(), 2);
    assert_eq!(
        document["o:search"], "blue",
        "the query reached the plugin handler"
    );

    let member = api.read_member("widgets", "7").await.unwrap();
    let document: Value = serde_json::from_str(&member.body).unwrap();
    assert_eq!(document["o:id"], "7");
    assert_eq!(document["o:label"], "a widget");
}

#[tokio::test]
async fn a_declined_request_falls_through_to_the_port() {
    let pool = pool().await;
    let api = Api::load_with_plugins(&pool, base(), Actor::anonymous(), registry())
        .await
        .unwrap();

    let error = api.read_member("widgets", "0").await.unwrap_err();
    assert!(
        matches!(error, ApiError::NotImplemented(_)),
        "a handler that returns None leaves the request to the port, got {error:?}"
    );
}

#[tokio::test]
async fn without_a_plugin_the_resource_stays_unknown() {
    let pool = pool().await;
    let api = Api::load(&pool, base(), Actor::anonymous()).await.unwrap();

    let error = api.search("widgets", &Params::parse("")).await.unwrap_err();
    assert!(matches!(error, ApiError::NotImplemented(_)));
}
