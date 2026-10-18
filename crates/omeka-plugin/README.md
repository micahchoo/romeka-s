# omeka-plugin

Romeka S is a fork of Omeka S. This crate is the extension seam: DESIGN decision 4 rejects dynamic PHP module loading
and names Rust registries as the seam for block layouts, media adapters, jobs, data types and API adapters. It holds
the traits a third party implements and the registry that holds the implementations, and the trait methods carry the
same names as the upstream interfaces they replace.

## The contract

A plugin is one type implementing `Plugin`. It reports an id and a label, and contributes to the
extension points it extends. Every contribution method defaults to an empty list.

```rust
pub trait Plugin: Send + Sync {
    fn id(&self) -> &'static str;
    fn label(&self) -> &'static str;
    fn api_adapters(&self) -> Vec<Arc<dyn ApiAdapter>> { Vec::new() }
    fn data_types(&self) -> Vec<&'static dyn DataType> { Vec::new() }
    fn block_layouts(&self) -> Vec<&'static dyn BlockLayout> { Vec::new() }
    fn media_ingesters(&self) -> Vec<Box<dyn Ingester + Send + Sync>> { Vec::new() }
    fn media_renderers(&self) -> Vec<Box<dyn Renderer + Send + Sync>> { Vec::new() }
}
```

`Registry` holds the installed plugins and everything they contributed. `Registry::with_builtin_adapters`
seeds it with the compiled-in API adapters, so a plugin adapter shares one namespace with them.

`Registry::install` takes all of a plugin or none of it. A conflicting name or a repeated plugin id leaves
the registry exactly as it was, and reports `PluginError`. `RegistryBuilder` chains installs and defers the
first failure to `build`.

Install refuses a duplicate name instead of overwriting, which is what upstream `ModuleManager` does. Two
adapters on one resource name is a routing bug. The behaviour is a choice, not a port of the PHP module
manager, which this crate replaces.

## Extension points

`omeka-api` consumes `api_adapters`. The other four are contracted: the registry holds their entries and
refuses duplicate names, but nothing reads them yet, so a contribution there changes no behaviour.

| Point | Owner registry | State |
| --- | --- | --- |
| API adapters | `omeka-api::adapters::ADAPTERS` | Wired. A plugin adapter is routed and its reads are served. |
| Data types | `omeka-core::datatypes::Registry` | Contracted. `omeka-api` does not read the core registry yet, so a plugin data type reaches no request. |
| Block layouts | `omeka-site::blocks::LAYOUTS` | Contracted. The site registry is a `static` array with no install API, so a plugin layout renders nowhere. |
| Media ingesters | `omeka-media::IngesterRegistry` | Contracted. The media CLI path builds the registry from `with_defaults`, so a plugin ingester is never looked up. |
| Media renderers | `omeka-media::RendererRegistry` | Contracted. Same as ingesters. |

Jobs are not in this trait set. The `Job` trait lives in `omeka-cli`, which sits above this crate in the
import graph, so a job contribution here would invert the graph. `omeka-cli::jobs::job_for` resolves the
four compiled-in classes by name.

## Adding a plugin

1. Implement `ApiAdapter` for the type that serves your resource. `spec` returns a `&'static ApiAdapterSpec`
   with the resource name, the upstream representation class and the operations you implement. `serve`
   returns an `AdapterReply` holding the JSON-LD document, or `None` to leave the request to the port.
2. Implement `Plugin`, returning your adapters from `api_adapters`.
3. Install it into the registry the process builds at startup.

A request reaches a plugin adapter when the resource name matches, the request's operation is in the spec's
`operations`, and `serve` returns a reply. `ApiAdapterSpec::name` must not be a name the port already
registers, and `install` enforces that.

A plugin adapter's reads are public, like the core read adapters (`Api::allow` grants `search` and `read` to
everyone for an unknown resource name). A per-resource permission rule, a write operation, and an error
reply are open; `omeka-api` names each in its module docs.
