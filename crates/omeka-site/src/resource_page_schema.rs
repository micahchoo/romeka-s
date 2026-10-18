//! The resource page block schema the admin edits: the registered layouts, the
//! resource each accepts, and the bundled theme's default set.
//!
//! [`crate::resource_page`] owns the registration and the render. This module
//! is the data the site Theme page turns into an ordered-list editor, and
//! `web/admin/src/generated/resourcePageBlocks.json` is its serialisation. A
//! test here keeps the file byte-identical, the way the block, token and site
//! settings registries do.
//!
//! Upstream reference (`source/omeka-s/application`):
//! `src/Site/ResourcePageBlockLayout/Manager.php` (`getAllForResource`, the
//! `getLabel` each layout answers) and `view/omeka/admin/site/theme.phtml`,
//! whose form posts `resource_page_blocks[<resource>][<region>][]`.

use omeka_core::resource::ResourceKind;
use serde::Serialize;

use crate::resource_page::{RESOURCE_PAGE_BLOCKS, default_blocks};
use crate::theme::tokens_schema::camel;

/// One resource page the manager configures, in the order
/// `Manager::standardizeResourcePageBlocks` walks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourcePageResourceSpec {
    /// The resource name the stored map uses (`items`, `item_sets`, `media`).
    pub key: &'static str,
    /// The resource kind it names.
    pub kind: ResourceKind,
}

/// The three resources, in the storage order and the theme's order.
pub const RESOURCES: [ResourcePageResourceSpec; 3] = [
    ResourcePageResourceSpec {
        key: "items",
        kind: ResourceKind::Item,
    },
    ResourcePageResourceSpec {
        key: "item_sets",
        kind: ResourceKind::ItemSet,
    },
    ResourcePageResourceSpec {
        key: "media",
        kind: ResourceKind::Media,
    },
];

/// The whole registry, as `resourcePageBlocks.json` holds it.
#[derive(Debug, Serialize)]
pub struct ResourcePageBlocksRegistry {
    /// Every resource, in storage order.
    pub resources: Vec<ResourcePageResourceEntry>,
}

/// One resource, its theme default and the layouts it accepts.
#[derive(Debug, Serialize)]
pub struct ResourcePageResourceEntry {
    /// The resource name the stored map uses.
    pub key: &'static str,
    /// The catalogue key for the resource's label.
    #[serde(rename = "labelKey")]
    pub label_key: String,
    /// The theme's default block names, in render order.
    pub default: Vec<&'static str>,
    /// The registered layouts compatible with this resource, in registry order.
    pub blocks: Vec<ResourcePageBlockEntry>,
}

/// One registered layout, compatible with the resource it is listed under.
#[derive(Debug, Serialize)]
pub struct ResourcePageBlockEntry {
    /// The registered name the stored map carries.
    pub name: &'static str,
    /// The catalogue key for the layout's label.
    #[serde(rename = "labelKey")]
    pub label_key: String,
}

/// The registry the admin reads.
#[must_use]
pub fn registry() -> ResourcePageBlocksRegistry {
    ResourcePageBlocksRegistry {
        resources: RESOURCES.iter().map(|resource| entry(*resource)).collect(),
    }
}

/// One resource as the admin reads it.
fn entry(resource: ResourcePageResourceSpec) -> ResourcePageResourceEntry {
    ResourcePageResourceEntry {
        key: resource.key,
        label_key: format!("resourcePage.{}", camel(resource.key)),
        default: default_blocks(resource.kind).to_vec(),
        blocks: RESOURCE_PAGE_BLOCKS
            .iter()
            .filter(|spec| spec.resources.contains(&resource.kind))
            .map(|spec| ResourcePageBlockEntry {
                name: spec.name,
                label_key: format!("resourcePageBlock.{}", camel(spec.name)),
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Where the admin reads the generated resource page block schema.
    const ADMIN_FILE: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../web/admin/src/generated/resourcePageBlocks.json"
    );

    /// The admin's file is the registry's serialisation, byte for byte. A
    /// difference fails with the command that rewrites it.
    #[test]
    fn generated_registry_matches_the_admin_file() {
        let rendered = format!(
            "{}\n",
            serde_json::to_string_pretty(&registry()).expect("the registry serialises")
        );
        if std::env::var_os("UPDATE_GENERATED").is_some() {
            let path = std::path::Path::new(ADMIN_FILE);
            std::fs::create_dir_all(path.parent().expect("the file has a parent"))
                .expect("the generated directory exists");
            std::fs::write(path, &rendered).expect("the registry file is writable");
            return;
        }
        let current = std::fs::read_to_string(ADMIN_FILE).unwrap_or_default();
        assert_eq!(
            current, rendered,
            "run UPDATE_GENERATED=1 cargo test -p omeka-site to rewrite {ADMIN_FILE}"
        );
    }

    /// Each resource lists exactly the layouts that accept it, in
    /// `module.config.php` order, and its default names are among them.
    #[test]
    fn each_resource_lists_its_compatible_layouts() {
        for resource in RESOURCES {
            let expected: Vec<&str> = RESOURCE_PAGE_BLOCKS
                .iter()
                .filter(|spec| spec.resources.contains(&resource.kind))
                .map(|spec| spec.name)
                .collect();
            let entry = entry(resource);
            let listed: Vec<&str> = entry.blocks.iter().map(|block| block.name).collect();
            assert_eq!(listed, expected, "{}", resource.key);
            for name in &entry.default {
                assert!(
                    listed.contains(name),
                    "{} default {name} is not a compatible layout",
                    resource.key
                );
            }
            assert!(
                entry.default.iter().all(|name| *name != "mediaLinks"),
                "{} default names the unregistered mediaLinks",
                resource.key
            );
        }
    }

    /// The catalogue keys are camel case of the stored names, and the three
    /// resources stand in storage order.
    #[test]
    fn the_registry_names_camel_case_labels() {
        let registry = registry();
        assert_eq!(registry.resources.len(), 3);
        assert_eq!(registry.resources[0].key, "items");
        assert_eq!(registry.resources[0].label_key, "resourcePage.items");
        assert_eq!(
            registry.resources[1].label_key,
            "resourcePage.itemSets".to_string()
        );
        assert_eq!(registry.resources[2].label_key, "resourcePage.media");
        let first = registry.resources[0]
            .blocks
            .first()
            .expect("items accepts a layout");
        assert_eq!(first.name, "itemSets");
        assert_eq!(first.label_key, "resourcePageBlock.itemSets");
    }
}
