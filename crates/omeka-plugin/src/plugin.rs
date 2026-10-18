//! The plugin trait: one third-party unit, and the registries it extends.

use std::sync::Arc;

use omeka_core::datatypes::DataType;
use omeka_media::ingest::Ingester;
use omeka_media::render::Renderer;
use omeka_site::blocks::BlockLayout;

use crate::api::ApiAdapter;

/// One installed plugin.
///
/// DESIGN decision 4 names five Rust registries as the module seam. This trait
/// is the unit a third party adds: `install` pulls whatever it implements into
/// the [`crate::Registry`]. Every contribution method has a default empty list,
/// so a plugin implements only the points it extends.
///
/// Wired today: [`Plugin::api_adapters`], which `omeka-api` routes and serves.
/// Contracted but not wired: the other four, whose registries live in
/// `omeka-core`, `omeka-media` and `omeka-site`. The registry holds their
/// entries and `install` validates their names, but no consumer reads them yet,
/// so a contribution there changes no behaviour. The README names what each one
/// needs before it is wired.
pub trait Plugin: Send + Sync {
    /// The plugin's stable identifier, unique across the installed set.
    fn id(&self) -> &'static str;

    /// The human-readable name.
    fn label(&self) -> &'static str;

    /// API adapters this plugin adds, one per resource it serves.
    fn api_adapters(&self) -> Vec<Arc<dyn ApiAdapter>> {
        Vec::new()
    }

    /// Literal and value data types this plugin adds, registered under
    /// [`DataType::name`] (`application/src/DataType/*`). Contracted.
    fn data_types(&self) -> Vec<&'static dyn DataType> {
        Vec::new()
    }

    /// Site block layouts this plugin adds, registered under
    /// [`BlockLayout::name`] (`application/src/Site/BlockLayout/*`). Contracted.
    fn block_layouts(&self) -> Vec<&'static dyn BlockLayout> {
        Vec::new()
    }

    /// Media ingesters this plugin adds, registered under [`Ingester::name`]
    /// (`application/src/Media/Ingester/*`). Contracted.
    ///
    /// The registry holds `dyn Ingester + Send + Sync` because a plugin is
    /// shared across request threads; the media registry's own bound is the
    /// bare trait, which this coerces to.
    fn media_ingesters(&self) -> Vec<Box<dyn Ingester + Send + Sync>> {
        Vec::new()
    }

    /// Media renderers this plugin adds, registered under [`Renderer::name`]
    /// (`application/src/Media/Renderer/*`). Contracted.
    fn media_renderers(&self) -> Vec<Box<dyn Renderer + Send + Sync>> {
        Vec::new()
    }
}
