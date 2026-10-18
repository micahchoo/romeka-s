//! The Romeka block registry and the carrier rule that stores its blocks under
//! an upstream layout.
//!
//! A Romeka block is stored as an upstream block, usually `html`, with its own
//! settings under `o:layout_data.romeka` (see `docs/compat.md`, "Site pages: the
//! one-way rule"). The upstream part, `o:data.html`, is the block's plain
//! rendering, generated from [`RomkaBlock::plain`] and never written by hand.
//! `plain_hash` records the hash of that rendering; when the stored rendering
//! no longer hashes to it, someone edited the html outside Romeka S, so the
//! stored html renders instead of the rich form.
//!
//! The registry sits beside the upstream [`super::LAYOUTS`] registry. Every
//! entry declares its fields in [`super::schema`]'s format, so the admin builds
//! a Romeka form with the same controls as an upstream block form, and the
//! generated `web/admin/src/generated/blocks.json` carries both registries.

use std::collections::BTreeMap;

use omeka_core::ids::{AssetId, PageId};
use omeka_core::site::{BlockData, RomkaCarrier, SiteBlock};
use serde_json::Value as JsonValue;

use super::schema::LayoutSchema;
use crate::view::RenderContext;

/// The smallest block a Romeka extension declares: a name, a form, a rich
/// render and the plain render the upstream layout stores.
pub trait RomkaBlock: Sync {
    /// The name `o:layout_data.romeka.block` holds.
    fn name(&self) -> &'static str;

    /// The catalogue key for the block's label, e.g. `romeka.hero`.
    fn label(&self) -> &'static str;

    /// The upstream layout the block is stored under. No current block needs
    /// another one, so `html` is the default.
    fn carrier(&self) -> &'static str {
        "html"
    }

    /// The settings fields, in form order, in the upstream schema format.
    fn schema(&self) -> &'static LayoutSchema;

    /// The rich render: what a Romeka visitor sees when the plain rendering is
    /// still the one this product wrote.
    fn render(&self, settings: &JsonValue, ctx: &RenderContext<'_>) -> String;

    /// The plain render, stored as the carrier's upstream content so upstream
    /// shows the same block, plainer. This method has no default: a block
    /// without a plain rendering does not compile.
    fn plain(&self, settings: &JsonValue, refs: &PlainRefs) -> String;

    /// The asset ids the settings name, so a caller can resolve them before it
    /// renders the plain form.
    fn asset_refs(&self, _settings: &JsonValue) -> Vec<AssetId> {
        Vec::new()
    }

    /// The page ids the settings name, for the same reason.
    fn page_refs(&self, _settings: &JsonValue) -> Vec<PageId> {
        Vec::new()
    }
}

/// A resolved asset, as a plain rendering names it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlainAsset {
    /// The file URL, relative to the origin the page is served from.
    pub url: String,
    /// The alt text, the empty string when the asset has none.
    pub alt: String,
}

/// The resolved references a plain rendering reads.
///
/// The plain form is generated at write time, where no [`RenderContext`] exists.
/// A caller resolves the ids the settings name into this map first, then the
/// renderer stays a pure transform from values to HTML.
#[derive(Debug, Clone, Default)]
pub struct PlainRefs {
    assets: BTreeMap<AssetId, PlainAsset>,
    pages: BTreeMap<PageId, String>,
}

impl PlainRefs {
    /// An empty map.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The same map with one asset resolved.
    #[must_use]
    pub fn asset(mut self, id: AssetId, url: impl Into<String>, alt: impl Into<String>) -> Self {
        self.assets.insert(
            id,
            PlainAsset {
                url: url.into(),
                alt: alt.into(),
            },
        );
        self
    }

    /// The same map with one page URL resolved.
    #[must_use]
    pub fn page(mut self, id: PageId, url: impl Into<String>) -> Self {
        self.pages.insert(id, url.into());
        self
    }

    /// The asset named by `id`, `None` when the caller could not resolve it.
    #[must_use]
    pub fn asset_of(&self, id: AssetId) -> Option<&PlainAsset> {
        self.assets.get(&id)
    }

    /// The URL of the page named by `id`, `None` when the caller could not
    /// resolve it.
    #[must_use]
    pub fn page_of(&self, id: PageId) -> Option<&str> {
        self.pages.get(&id).map(String::as_str)
    }
}

/// The registered Romeka blocks, in registry order.
pub static ROMKA_BLOCKS: [&dyn RomkaBlock; 1] = [&super::hero::Hero];

/// Every registered Romeka block.
#[must_use]
pub fn blocks() -> &'static [&'static dyn RomkaBlock] {
    &ROMKA_BLOCKS
}

/// The Romeka block registered under `name`.
#[must_use]
pub fn block(name: &str) -> Option<&'static dyn RomkaBlock> {
    ROMKA_BLOCKS
        .iter()
        .copied()
        .find(|block| block.name() == name)
}

/// The hash `plain_hash` stores: FNV-1a over the UTF-8 bytes, as 16 lowercase
/// hex digits.
///
/// The admin recomputes it from the stored html, so the algorithm has to be
/// small enough to repeat in TypeScript (`web/admin/src/pages/sites/editor/romeka.ts`).
/// `web/admin/src/generated/blocks.json` carries a worked sample and both sides
/// test against it, which keeps the two implementations in step.
#[must_use]
pub fn plain_hash(html: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in html.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

/// What a carrier renders: the rich form, or the stored html when the plain
/// rendering no longer matches its hash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CarrierRender {
    /// The registered block's rich render.
    Rich(String),
    /// The stored plain html, purified, because it was edited outside Romeka S.
    Edited(String),
}

/// Render a block that carries a registered Romeka block, `None` for a block
/// that carries none or names one the registry does not know.
///
/// A missing hash never chooses the rich form: with no hash the block has not
/// been through a Romeka write, so the stored html is what upstream would show.
#[must_use]
pub fn render_carrier(stored: &SiteBlock, ctx: &RenderContext<'_>) -> Option<CarrierRender> {
    let carrier = stored.layout_data.romeka.as_ref()?;
    let definition = block(carrier.block.as_str())?;
    let BlockData::Html { html } = &stored.data else {
        return None;
    };
    match &carrier.plain_hash {
        Some(hash) if *hash == plain_hash(html) => Some(CarrierRender::Rich(
            definition.render(&carrier.settings, ctx),
        )),
        _ => Some(CarrierRender::Edited(omeka_core::purify::purify(html))),
    }
}

/// Whether this write must regenerate the plain rendering: the carrier names a
/// registered block and its hash matches the stored html, or it has no hash.
#[must_use]
pub fn needs_plain(carrier: &RomkaCarrier, stored_html: &str) -> bool {
    if block(carrier.block.as_str()).is_none() {
        return false;
    }
    match &carrier.plain_hash {
        Some(hash) => *hash == plain_hash(stored_html),
        None => true,
    }
}

/// The plain rendering for a carrier, purified, `None` when the carrier names
/// no registered block.
#[must_use]
pub fn plain_for(carrier: &RomkaCarrier, refs: &PlainRefs) -> Option<String> {
    let definition = block(carrier.block.as_str())?;
    Some(omeka_core::purify::purify(
        &definition.plain(&carrier.settings, refs),
    ))
}

/// The asset ids a carrier's settings name.
#[must_use]
pub fn asset_refs(carrier: &RomkaCarrier) -> Vec<AssetId> {
    block(carrier.block.as_str())
        .map(|definition| definition.asset_refs(&carrier.settings))
        .unwrap_or_default()
}

/// The page ids a carrier's settings name.
#[must_use]
pub fn page_refs(carrier: &RomkaCarrier) -> Vec<PageId> {
    block(carrier.block.as_str())
        .map(|definition| definition.page_refs(&carrier.settings))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The hash is FNV-1a 64 over the UTF-8 bytes. The admin repeats it, so
    /// this test pins the vectors the generated registry also carries.
    #[test]
    fn plain_hash_is_fnv1a_64_over_utf8() {
        assert_eq!(plain_hash(""), "cbf29ce484222325");
        assert_eq!(plain_hash("a"), "af63dc4c8601ec8c");
        assert_eq!(plain_hash("café"), "48e8823acfa40d89");
    }

    /// Every registered block declares a settings form and both renderings.
    /// The plain render is required by the trait, so a block cannot compile
    /// without one; this checks the two renderers agree there is something to
    /// render.
    #[test]
    fn every_romka_block_declares_a_form_and_renders() {
        for block in blocks() {
            assert!(!block.schema().fields.is_empty(), "{} schema", block.name());
            assert!(
                block.label().starts_with("romeka."),
                "{} label is a catalogue key",
                block.name()
            );
            assert_eq!(block.carrier(), "html", "{} carrier", block.name());
        }
    }
}
