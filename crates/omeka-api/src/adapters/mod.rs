//! One module per registered API adapter.
//!
//! `application/config/module.config.php` registers 17 adapters under
//! `api_adapters`. The other files in `application/src/Api/Adapter/` are
//! abstract bases, interfaces, the adapter manager, a trait and two hydrators;
//! they carry no routes and have no module here.
//!
//! Every adapter answers 501 with a [`NotImplemented`] body until wave 2 wires
//! repositories behind it. Upstream has no 501: an adapter that lacks an
//! operation throws `OperationNotImplementedException`, which the JSON
//! strategy maps to 500. The port uses 501 so a caller can tell "not built
//! yet" apart from "failed".

pub mod api_resources;
pub mod assets;
pub mod data_types;
pub mod item_sets;
pub mod items;
pub mod jobs;
pub mod media;
pub mod modules;
pub mod properties;
pub mod resource_classes;
pub mod resource_templates;
pub mod resources;
pub mod site_pages;
pub mod sites;
pub mod users;
pub mod value_annotations;
pub mod vocabularies;

use std::sync::{Arc, LazyLock};

use omeka_plugin::Registry;

use crate::errors::NotImplemented;
use crate::routes::Operation;

/// Registration record for one upstream API adapter (`AdapterInterface`).
///
/// The type and the [`Operation`] enum it holds live in `omeka-plugin`, so a
/// plugin adapter and the port's own adapters are described by one type.
pub use omeka_plugin::ApiAdapterSpec as AdapterSpec;

/// The typed 501 body for one routed operation.
#[must_use]
pub fn not_implemented(spec: &AdapterSpec, operation: Operation) -> NotImplemented {
    NotImplemented::adapter(spec.name, operation)
}

/// All registered adapters, sorted by name.
pub static ADAPTERS: &[AdapterSpec] = &[
    api_resources::SPEC,
    assets::SPEC,
    data_types::SPEC,
    item_sets::SPEC,
    items::SPEC,
    jobs::SPEC,
    media::SPEC,
    modules::SPEC,
    properties::SPEC,
    resource_classes::SPEC,
    resource_templates::SPEC,
    resources::SPEC,
    site_pages::SPEC,
    sites::SPEC,
    users::SPEC,
    value_annotations::SPEC,
    vocabularies::SPEC,
];

/// The registry the port ships: the seventeen compiled-in adapters and no
/// plugins.
#[must_use]
pub fn core_registry() -> Arc<Registry> {
    CORE_REGISTRY.clone()
}

static CORE_REGISTRY: LazyLock<Arc<Registry>> =
    LazyLock::new(|| Arc::new(Registry::with_builtin_adapters(ADAPTERS)));

/// Look up an adapter by resource name.
pub fn find(name: &str) -> Option<&'static AdapterSpec> {
    ADAPTERS.iter().find(|spec| spec.name == name)
}

/// The upstream adapter class name for one resource, as
/// `AdapterInterface::getResourceId` reports it in permission messages.
pub fn php_class(name: &str) -> &'static str {
    match name {
        "api_resources" => "Omeka\\Api\\Adapter\\ApiResourceAdapter",
        "assets" => "Omeka\\Api\\Adapter\\AssetAdapter",
        "data_types" => "Omeka\\Api\\Adapter\\DataTypeAdapter",
        "item_sets" => "Omeka\\Api\\Adapter\\ItemSetAdapter",
        "items" => "Omeka\\Api\\Adapter\\ItemAdapter",
        "jobs" => "Omeka\\Api\\Adapter\\JobAdapter",
        "media" => "Omeka\\Api\\Adapter\\MediaAdapter",
        "modules" => "Omeka\\Api\\Adapter\\ModuleAdapter",
        "properties" => "Omeka\\Api\\Adapter\\PropertyAdapter",
        "resource_classes" => "Omeka\\Api\\Adapter\\ResourceClassAdapter",
        "resource_templates" => "Omeka\\Api\\Adapter\\ResourceTemplateAdapter",
        "resources" => "Omeka\\Api\\Adapter\\ResourceAdapter",
        "site_pages" => "Omeka\\Api\\Adapter\\SitePageAdapter",
        "sites" => "Omeka\\Api\\Adapter\\SiteAdapter",
        "users" => "Omeka\\Api\\Adapter\\UserAdapter",
        "value_annotations" => "Omeka\\Api\\Adapter\\ValueAnnotationAdapter",
        "vocabularies" => "Omeka\\Api\\Adapter\\VocabularyAdapter",
        _ => "Omeka\\Api\\Adapter\\AdapterInterface",
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn registry_names_match_module_config_api_adapters() {
        let names: Vec<&str> = ADAPTERS.iter().map(|spec| spec.name).collect();
        assert_eq!(
            names,
            [
                "api_resources",
                "assets",
                "data_types",
                "item_sets",
                "items",
                "jobs",
                "media",
                "modules",
                "properties",
                "resource_classes",
                "resource_templates",
                "resources",
                "site_pages",
                "sites",
                "users",
                "value_annotations",
                "vocabularies",
            ]
        );
    }

    #[test]
    fn adapters_that_lack_an_operation_say_so() {
        assert_eq!(
            modules::SPEC.operations,
            [Operation::Search, Operation::Read]
        );
        assert_eq!(
            resources::SPEC.operations,
            [Operation::Search, Operation::Read]
        );
        assert_eq!(
            items::SPEC.operations,
            [
                Operation::Search,
                Operation::Read,
                Operation::Create,
                Operation::Update,
                Operation::Delete
            ]
        );
    }

    #[test]
    fn not_implemented_names_the_adapter_and_operation() {
        let body = not_implemented(&items::SPEC, Operation::Search);
        assert_eq!(
            body.errors.error,
            "The items adapter does not implement the search operation."
        );
    }
}
