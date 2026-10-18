//! The seam's contract, exercised through the registry a consumer uses.
//!
//! One plugin contributes to all five extension points; the tests assert what
//! `Registry` reports, what `install` refuses, and that a refused install
//! changes nothing.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use omeka_core::datatypes::{DataType, HydratedValue};
use omeka_core::site::SiteBlock;
use omeka_core::value::{Value, ValueData, ValueError, ValueObject};
use omeka_media::ingest::{ErrorStore, IngestEnv, IngestRequest, Ingester, MediaDraft};
use omeka_media::render::{MediaView, RenderOptions, Renderer, View};
use omeka_plugin::{
    AdapterReply, AdapterRequest, ApiAdapter, ApiAdapterSpec, Operation, Plugin, PluginError,
    Registry, RegistryBuilder,
};
use omeka_site::blocks::BlockLayout;
use omeka_site::view::RenderContext;
use serde_json::{Value as JsonValue, json};

static NOTE_SPEC: ApiAdapterSpec = ApiAdapterSpec {
    name: "notes",
    representation: "App\\Api\\Adapter\\NoteAdapter",
    operations: &[Operation::Search, Operation::Read],
};

static TAKEOVER_SPEC: ApiAdapterSpec = ApiAdapterSpec {
    name: "items",
    representation: "App\\Api\\Adapter\\TakesOverItemsAdapter",
    operations: &[Operation::Search],
};

struct Notes;

impl ApiAdapter for Notes {
    fn spec(&self) -> &'static ApiAdapterSpec {
        &NOTE_SPEC
    }

    fn serve(&self, request: &AdapterRequest) -> Option<AdapterReply> {
        match request.operation {
            Operation::Search => Some(AdapterReply::document(json!({"notes": []}))),
            Operation::Read => request
                .id
                .as_deref()
                .map(|id| AdapterReply::document(json!({ "note": id }))),
            _ => None,
        }
    }
}

struct TakesOverItems;

impl ApiAdapter for TakesOverItems {
    fn spec(&self) -> &'static ApiAdapterSpec {
        &TAKEOVER_SPEC
    }

    fn serve(&self, _request: &AdapterRequest) -> Option<AdapterReply> {
        None
    }
}

struct PluginType;

impl DataType for PluginType {
    fn name(&self) -> &'static str {
        "plugin_type"
    }

    fn label(&self) -> &'static str {
        "Plugin type"
    }

    fn is_valid(&self, _object: &ValueObject) -> bool {
        true
    }

    fn hydrate(&self, _object: &ValueObject) -> Result<HydratedValue, ValueError> {
        Ok(HydratedValue {
            data: ValueData::literal("plugin").unwrap(),
            lang: None,
        })
    }

    fn json_ld(&self, _value: &Value) -> Option<JsonValue> {
        None
    }

    fn to_string(&self, _value: &Value) -> String {
        "plugin".to_string()
    }

    fn fulltext(&self, _value: &Value) -> Option<String> {
        None
    }
}

static PLUGIN_TYPE: PluginType = PluginType;

struct PluginLayout;

impl BlockLayout for PluginLayout {
    fn name(&self) -> &'static str {
        "pluginLayout"
    }

    fn label(&self) -> &'static str {
        "Plugin layout"
    }

    fn render(&self, _block: &SiteBlock, _ctx: &RenderContext<'_>) -> String {
        "<div class=\"plugin-layout\"></div>".to_string()
    }
}

static PLUGIN_LAYOUT: PluginLayout = PluginLayout;

struct PluginIngester;

impl Ingester for PluginIngester {
    fn name(&self) -> &str {
        "plugin_ingester"
    }

    fn label(&self) -> String {
        "Plugin ingester".to_string()
    }

    fn renderer(&self) -> &'static str {
        "file"
    }

    fn ingest(
        &self,
        _draft: &mut MediaDraft,
        _request: &IngestRequest,
        _env: &IngestEnv<'_>,
        errors: &mut ErrorStore,
    ) {
        errors.add("plugin_ingester", "not built");
    }
}

struct PluginRenderer;

impl Renderer for PluginRenderer {
    fn name(&self) -> &str {
        "plugin_renderer"
    }

    fn render(&self, _view: &View<'_>, _media: &MediaView, _options: &RenderOptions) -> String {
        "<span class=\"plugin-renderer\"></span>".to_string()
    }
}

struct AllPoints {
    adapters: Vec<Arc<dyn ApiAdapter>>,
}

impl Plugin for AllPoints {
    fn id(&self) -> &'static str {
        "all_points"
    }

    fn label(&self) -> &'static str {
        "All points"
    }

    fn api_adapters(&self) -> Vec<Arc<dyn ApiAdapter>> {
        self.adapters.clone()
    }

    fn data_types(&self) -> Vec<&'static dyn DataType> {
        vec![&PLUGIN_TYPE]
    }

    fn block_layouts(&self) -> Vec<&'static dyn BlockLayout> {
        vec![&PLUGIN_LAYOUT]
    }

    fn media_ingesters(&self) -> Vec<Box<dyn Ingester + Send + Sync>> {
        vec![Box::new(PluginIngester)]
    }

    fn media_renderers(&self) -> Vec<Box<dyn Renderer + Send + Sync>> {
        vec![Box::new(PluginRenderer)]
    }
}

fn all_points() -> Arc<dyn Plugin> {
    Arc::new(AllPoints {
        adapters: vec![Arc::new(Notes)],
    })
}

fn builtin_adapters() -> &'static [ApiAdapterSpec] {
    static BUILTIN: [ApiAdapterSpec; 2] = [
        ApiAdapterSpec {
            name: "items",
            representation: "Omeka\\Api\\Adapter\\ItemAdapter",
            operations: &[Operation::Search, Operation::Read],
        },
        ApiAdapterSpec {
            name: "media",
            representation: "Omeka\\Api\\Adapter\\MediaAdapter",
            operations: &[Operation::Search],
        },
    ];
    &BUILTIN
}

#[test]
fn a_plugin_contributes_to_every_extension_point() {
    let registry = RegistryBuilder::with_adapters(builtin_adapters())
        .install(all_points())
        .build()
        .unwrap();

    assert_eq!(registry.plugins().len(), 1);
    assert_eq!(registry.plugins()[0].label(), "All points");
    assert_eq!(
        registry.adapter_names(),
        ["items", "media", "notes"],
        "a plugin adapter joins the built-ins in resource-name order"
    );
    assert_eq!(
        registry
            .data_types()
            .iter()
            .map(|data_type| data_type.name())
            .collect::<Vec<_>>(),
        ["plugin_type"]
    );
    assert_eq!(
        registry
            .block_layouts()
            .iter()
            .map(|layout| layout.name())
            .collect::<Vec<_>>(),
        ["pluginLayout"]
    );
    assert_eq!(
        registry
            .media_ingesters()
            .iter()
            .map(|ingester| ingester.name())
            .collect::<Vec<_>>(),
        ["plugin_ingester"]
    );
    assert_eq!(
        registry
            .media_renderers()
            .iter()
            .map(|renderer| renderer.name())
            .collect::<Vec<_>>(),
        ["plugin_renderer"]
    );
}

#[test]
fn a_plugin_adapter_serves_a_request_and_a_builtin_has_no_handler() {
    let registry = RegistryBuilder::with_adapters(builtin_adapters())
        .install(all_points())
        .build()
        .unwrap();

    let entry = registry.adapter("notes").unwrap();
    assert_eq!(entry.spec().name, "notes");
    let served = entry
        .handler()
        .unwrap()
        .serve(&AdapterRequest {
            resource: "notes".to_string(),
            operation: Operation::Read,
            id: Some("7".to_string()),
            query: Vec::new(),
        })
        .unwrap();
    assert_eq!(served.document, json!({"note": "7"}));

    let builtin = registry.adapter("items").unwrap();
    assert_eq!(builtin.spec().name, "items");
    assert!(
        builtin.handler().is_none(),
        "the port's own adapter carries no plugin handler"
    );
}

#[test]
fn an_empty_builder_holds_no_plugin() {
    let registry = RegistryBuilder::new().build().unwrap();
    assert!(registry.is_empty());
    assert_eq!(registry.adapter_names(), Vec::<&'static str>::new());
    assert!(registry.adapters().next().is_none());
}

#[test]
fn the_same_plugin_id_twice_is_refused() {
    let error = RegistryBuilder::new()
        .install(all_points())
        .install(all_points())
        .build()
        .unwrap_err();
    assert_eq!(
        error,
        PluginError::DuplicatePlugin {
            id: "all_points".to_string()
        }
    );
}

#[test]
fn a_plugin_cannot_take_over_a_builtin_adapter_name() {
    let plugin: Arc<dyn Plugin> = Arc::new(AllPoints {
        adapters: vec![Arc::new(TakesOverItems)],
    });
    let error = RegistryBuilder::with_adapters(builtin_adapters())
        .install(plugin)
        .build()
        .unwrap_err();
    assert_eq!(
        error,
        PluginError::DuplicateEntry {
            point: "api adapter",
            name: "items".to_string()
        }
    );
}

#[test]
fn a_refused_install_leaves_the_registry_unchanged() {
    let mut registry = Registry::with_builtin_adapters(builtin_adapters());
    let conflict: Arc<dyn Plugin> = Arc::new(AllPoints {
        adapters: vec![Arc::new(TakesOverItems)],
    });
    let error = registry.install(conflict).unwrap_err();
    assert!(matches!(error, PluginError::DuplicateEntry { .. }));
    assert!(registry.is_empty(), "the plugin was not added");
    assert_eq!(registry.adapter_names(), ["items", "media"]);
    assert!(
        registry.data_types().is_empty(),
        "no other contribution landed either"
    );
}

#[test]
fn one_plugin_cannot_contribute_one_name_twice() {
    struct TwoOfTheSame;
    impl Plugin for TwoOfTheSame {
        fn id(&self) -> &'static str {
            "two_of_the_same"
        }

        fn label(&self) -> &'static str {
            "Two of the same"
        }

        fn api_adapters(&self) -> Vec<Arc<dyn ApiAdapter>> {
            vec![Arc::new(Notes), Arc::new(Notes)]
        }
    }

    let error = RegistryBuilder::new()
        .install(Arc::new(TwoOfTheSame))
        .build()
        .unwrap_err();
    assert_eq!(
        error,
        PluginError::DuplicateEntry {
            point: "api adapter",
            name: "notes".to_string()
        }
    );
}
