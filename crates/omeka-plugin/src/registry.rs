//! The registry that holds the installed plugins and their contributions.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use omeka_core::datatypes::DataType;
use omeka_media::ingest::Ingester;
use omeka_media::render::Renderer;
use omeka_site::blocks::BlockLayout;
use thiserror::Error;

use crate::api::{ApiAdapter, ApiAdapterSpec};
use crate::plugin::Plugin;

/// Why an install was refused.
///
/// Upstream's `ModuleManager` lets a later module overwrite an earlier one.
/// The port refuses instead: a silent overwrite is the failure a registry is
/// meant to prevent, because two adapters on one resource name is a routing
/// bug, not a preference.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PluginError {
    /// The same plugin id is installed twice.
    #[error("plugin `{id}` is already installed")]
    DuplicatePlugin { id: String },
    /// Two sources claim one extension point under one name.
    #[error("the {point} `{name}` is already registered")]
    DuplicateEntry { point: &'static str, name: String },
}

/// One registered API adapter: its spec, and the plugin that serves it.
#[derive(Clone)]
pub struct AdapterEntry {
    spec: &'static ApiAdapterSpec,
    handler: Option<Arc<dyn ApiAdapter>>,
}

impl std::fmt::Debug for AdapterEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AdapterEntry")
            .field("name", &self.spec.name)
            .field("representation", &self.spec.representation)
            .field("plugin", &self.handler.is_some())
            .finish()
    }
}

impl AdapterEntry {
    /// The registration record.
    #[must_use]
    pub fn spec(&self) -> &'static ApiAdapterSpec {
        self.spec
    }

    /// The plugin handler, `None` for an adapter the port itself serves.
    #[must_use]
    pub fn handler(&self) -> Option<&dyn ApiAdapter> {
        self.handler.as_deref()
    }
}

/// The installed plugins and everything they contributed.
///
/// One value describes the whole extension surface of a running process. The
/// compiled-in adapters are seeded with [`Registry::with_builtin_adapters`] so
/// adapter names live in one namespace and [`Registry::install`] can refuse a
/// plugin that would take one over.
#[derive(Default)]
pub struct Registry {
    plugins: Vec<Arc<dyn Plugin>>,
    adapters: BTreeMap<&'static str, AdapterEntry>,
    data_types: Vec<&'static dyn DataType>,
    block_layouts: Vec<&'static dyn BlockLayout>,
    media_ingesters: Vec<Box<dyn Ingester + Send + Sync>>,
    media_renderers: Vec<Box<dyn Renderer + Send + Sync>>,
}

impl std::fmt::Debug for Registry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Registry")
            .field(
                "plugins",
                &self
                    .plugins
                    .iter()
                    .map(|plugin| plugin.id())
                    .collect::<Vec<_>>(),
            )
            .field("adapters", &self.adapter_names())
            .field(
                "data_types",
                &self
                    .data_types
                    .iter()
                    .map(|data_type| data_type.name())
                    .collect::<Vec<_>>(),
            )
            .field(
                "block_layouts",
                &self
                    .block_layouts
                    .iter()
                    .map(|layout| layout.name())
                    .collect::<Vec<_>>(),
            )
            .field(
                "media_ingesters",
                &self
                    .media_ingesters
                    .iter()
                    .map(|ingester| ingester.name())
                    .collect::<Vec<_>>(),
            )
            .field(
                "media_renderers",
                &self
                    .media_renderers
                    .iter()
                    .map(|renderer| renderer.name())
                    .collect::<Vec<_>>(),
            )
            .finish()
    }
}

impl Registry {
    /// A registry holding the compiled-in API adapters and no plugins.
    ///
    /// A duplicate name in `adapters` keeps the first entry; the port's own
    /// list is checked for uniqueness by a test in `omeka-api`.
    #[must_use]
    pub fn with_builtin_adapters(adapters: &'static [ApiAdapterSpec]) -> Self {
        let mut registry = Self::default();
        for spec in adapters {
            registry.adapters.entry(spec.name).or_insert(AdapterEntry {
                spec,
                handler: None,
            });
        }
        registry
    }

    /// Install one plugin and everything it contributes.
    ///
    /// All or nothing: a conflict leaves the registry exactly as it was, so a
    /// caller that ignores the error cannot end up with half a plugin.
    ///
    /// # Errors
    /// [`PluginError::DuplicatePlugin`] when the id is installed already, or
    /// [`PluginError::DuplicateEntry`] when a contribution's name is taken by
    /// the registry or by another contribution in the same plugin.
    pub fn install(&mut self, plugin: Arc<dyn Plugin>) -> Result<(), PluginError> {
        if self
            .plugins
            .iter()
            .any(|installed| installed.id() == plugin.id())
        {
            return Err(PluginError::DuplicatePlugin {
                id: plugin.id().to_string(),
            });
        }

        let adapters = plugin.api_adapters();
        let data_types = plugin.data_types();
        let block_layouts = plugin.block_layouts();
        let media_ingesters = plugin.media_ingesters();
        let media_renderers = plugin.media_renderers();

        let mut names = Names::new("api adapter");
        names.reserve(self.adapters.keys().copied());
        for adapter in &adapters {
            names.check(adapter.spec().name)?;
        }

        let mut names = Names::new("data type");
        names.reserve(self.data_types.iter().map(|entry| entry.name()));
        for data_type in &data_types {
            names.check(data_type.name())?;
        }

        let mut names = Names::new("block layout");
        names.reserve(self.block_layouts.iter().map(|entry| entry.name()));
        for block_layout in &block_layouts {
            names.check(block_layout.name())?;
        }

        let mut names = Names::new("media ingester");
        names.reserve(self.media_ingesters.iter().map(|entry| entry.name()));
        for ingester in &media_ingesters {
            names.check(ingester.name())?;
        }

        let mut names = Names::new("media renderer");
        names.reserve(self.media_renderers.iter().map(|entry| entry.name()));
        for renderer in &media_renderers {
            names.check(renderer.name())?;
        }

        for adapter in adapters {
            let spec = adapter.spec();
            self.adapters.insert(
                spec.name,
                AdapterEntry {
                    spec,
                    handler: Some(adapter),
                },
            );
        }
        self.data_types.extend(data_types);
        self.block_layouts.extend(block_layouts);
        self.media_ingesters.extend(media_ingesters);
        self.media_renderers.extend(media_renderers);
        self.plugins.push(plugin);
        Ok(())
    }

    /// The installed plugins, in install order.
    #[must_use]
    pub fn plugins(&self) -> &[Arc<dyn Plugin>] {
        &self.plugins
    }

    /// Whether no plugin is installed.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.plugins.is_empty()
    }

    /// The registered adapters, in resource-name order.
    pub fn adapters(&self) -> impl Iterator<Item = &AdapterEntry> {
        self.adapters.values()
    }

    /// The entry for one resource name, `None` when nothing registers it.
    #[must_use]
    pub fn adapter(&self, name: &str) -> Option<&AdapterEntry> {
        self.adapters.get(name)
    }

    /// The registered resource names, in order.
    #[must_use]
    pub fn adapter_names(&self) -> Vec<&'static str> {
        self.adapters.keys().copied().collect()
    }

    /// The data types plugins contributed.
    #[must_use]
    pub fn data_types(&self) -> &[&'static dyn DataType] {
        &self.data_types
    }

    /// The block layouts plugins contributed.
    #[must_use]
    pub fn block_layouts(&self) -> &[&'static dyn BlockLayout] {
        &self.block_layouts
    }

    /// The media ingesters plugins contributed.
    #[must_use]
    pub fn media_ingesters(&self) -> &[Box<dyn Ingester + Send + Sync>] {
        &self.media_ingesters
    }

    /// The media renderers plugins contributed.
    #[must_use]
    pub fn media_renderers(&self) -> &[Box<dyn Renderer + Send + Sync>] {
        &self.media_renderers
    }
}

/// A [`Registry`] a caller builds by chaining installs, deferring the first
/// failure to [`RegistryBuilder::build`].
#[derive(Default)]
pub struct RegistryBuilder {
    registry: Registry,
    error: Option<PluginError>,
}

impl RegistryBuilder {
    /// An empty builder: no built-in adapters, no plugins.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A builder seeded with the compiled-in API adapters.
    #[must_use]
    pub fn with_adapters(adapters: &'static [ApiAdapterSpec]) -> Self {
        Self {
            registry: Registry::with_builtin_adapters(adapters),
            error: None,
        }
    }

    /// Install one plugin. The first failure is kept and later installs are
    /// skipped, so a report names the first conflict rather than a cascade.
    #[must_use]
    pub fn install(mut self, plugin: Arc<dyn Plugin>) -> Self {
        if self.error.is_none()
            && let Err(error) = self.registry.install(plugin)
        {
            self.error = Some(error);
        }
        self
    }

    /// The registry, or the first install failure.
    ///
    /// # Errors
    /// The [`PluginError`] of the first install that failed.
    pub fn build(self) -> Result<Registry, PluginError> {
        match self.error {
            Some(error) => Err(error),
            None => Ok(self.registry),
        }
    }
}

/// Collects the names one extension point already holds, and refuses a repeat.
struct Names {
    point: &'static str,
    seen: BTreeSet<String>,
}

impl Names {
    fn new(point: &'static str) -> Self {
        Self {
            point,
            seen: BTreeSet::new(),
        }
    }

    fn reserve<'a>(&mut self, names: impl IntoIterator<Item = &'a str>) {
        self.seen.extend(names.into_iter().map(str::to_string));
    }

    fn check(&mut self, name: &str) -> Result<(), PluginError> {
        if self.seen.insert(name.to_string()) {
            Ok(())
        } else {
            Err(PluginError::DuplicateEntry {
                point: self.point,
                name: name.to_string(),
            })
        }
    }
}
