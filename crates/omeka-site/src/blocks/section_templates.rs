//! The built-in section templates: a `blockGroup` with a section layout and the
//! blocks that fill its slots, for the shapes curators build most.
//!
//! The block picker offers them beside the blocks, and inserting one lays down
//! the group and its members in one edit. Each is a starting point: its text
//! blocks are empty and show their hints, and its media and asset blocks are
//! empty until the curator picks an item or an asset. A section none of whose
//! blocks is written reaches no visitor. [`super::templates::template_registry`] carries
//! them to the admin in `templates.json`.

use serde_json::{Value as JsonValue, json};

use super::featured_item::FeaturedItem;
use super::quote::Quote;
use super::romeka::CARRIER_VERSION;
use super::templates::{TemplateBlock, layout_block, romka_block};

/// One section template.
pub struct SectionTemplate {
    /// The template's id, unique among the section templates.
    pub id: &'static str,
    /// The catalogue key for the template's name.
    pub label: &'static str,
    /// The catalogue key for the template's description.
    pub description: &'static str,
    /// The group's Romeka settings, the section layout among them.
    pub settings: JsonValue,
    /// The blocks in the section's slots, in order.
    pub members: Vec<TemplateBlock>,
}

impl SectionTemplate {
    /// The group block, then its members, as a page stores them.
    #[must_use]
    pub fn blocks(self) -> Vec<TemplateBlock> {
        let span = self.members.len().to_string();
        let group = layout_block("blockGroup", json!({ "span": span })).with_layout_data(
            json!({ "romeka": {
                "block": "blockGroup",
                "v": CARRIER_VERSION,
                "settings": self.settings,
            } }),
        );
        std::iter::once(group).chain(self.members).collect()
    }
}

/// An empty media block: one item's media, once the curator picks the item.
fn media() -> TemplateBlock {
    layout_block(
        "media",
        json!({
            "layout": "",
            "media_display": "",
            "thumbnail_type": "large",
            "show_title_option": "no_title",
        }),
    )
}

/// An empty HTML block, which shows its hint until the curator writes it.
fn html() -> TemplateBlock {
    layout_block("html", json!({ "html": "" }))
}

/// The built-in section templates, in the order the block picker lists them.
#[must_use]
pub fn section_templates() -> Vec<SectionTemplate> {
    vec![
        SectionTemplate {
            id: "media-text",
            label: "sectionTemplate.mediaText.label",
            description: "sectionTemplate.mediaText.description",
            settings: json!({ "section": { "layout": "split-40-60", "align": "center" } }),
            members: vec![media(), html()],
        },
        SectionTemplate {
            id: "text-media",
            label: "sectionTemplate.textMedia.label",
            description: "sectionTemplate.textMedia.description",
            settings: json!({ "section": { "layout": "split-60-40", "align": "center" } }),
            members: vec![html(), media()],
        },
        SectionTemplate {
            id: "three-cards",
            label: "sectionTemplate.threeCards.label",
            description: "sectionTemplate.threeCards.description",
            settings: json!({ "section": { "layout": "cards-3" } }),
            members: vec![html(), html(), html()],
        },
        SectionTemplate {
            id: "featured-items",
            label: "sectionTemplate.featuredItems.label",
            description: "sectionTemplate.featuredItems.description",
            settings: json!({ "section": { "layout": "cards-3" } }),
            members: vec![
                romka_block(&FeaturedItem, json!({ "image": "top" })),
                romka_block(&FeaturedItem, json!({ "image": "top" })),
                romka_block(&FeaturedItem, json!({ "image": "top" })),
            ],
        },
        SectionTemplate {
            id: "quote-band",
            label: "sectionTemplate.quoteBand.label",
            description: "sectionTemplate.quoteBand.description",
            settings: json!({ "section": { "layout": "band" }, "background_role": "surface" }),
            members: vec![romka_block(
                &Quote,
                json!({
                    "text": "",
                    "attribution": "",
                    "style": "large",
                }),
            )],
        },
        SectionTemplate {
            id: "image-band",
            label: "sectionTemplate.imageBand.label",
            description: "sectionTemplate.imageBand.description",
            settings: json!({ "section": { "layout": "band", "gap": "none" } }),
            members: vec![layout_block(
                "asset",
                json!({
                    "0": { "id": "", "page": "", "alt_link_title": "", "caption": "" },
                }),
            )],
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks::style::{RomekaStyle, Section};
    use omeka_core::site::BlockLayoutData;

    /// The five shapes the sections brief named and the featured items row,
    /// each id unique.
    #[test]
    fn the_picker_offers_the_named_sections() {
        let ids: Vec<&str> = section_templates()
            .iter()
            .map(|section| section.id)
            .collect();
        assert_eq!(
            ids,
            [
                "media-text",
                "text-media",
                "three-cards",
                "featured-items",
                "quote-band",
                "image-band"
            ]
        );
    }

    /// Each group's span counts its members, and its section and styling are
    /// values the renderer reads, so no template stores a layout that renders
    /// as no section.
    #[test]
    fn every_section_template_renders_as_a_section() {
        for section in section_templates() {
            let id = section.id;
            let count = section.members.len();
            let blocks = super::super::templates::block_entry(
                section.blocks().first().expect("the group block"),
            );
            assert_eq!(blocks.layout, "blockGroup", "{id}");
            assert_eq!(blocks.data, json!({ "span": count.to_string() }), "{id}");
            let parsed = BlockLayoutData::parse(&blocks.layout_data);
            assert!(Section::parse(&parsed).is_some(), "{id}");
            let carrier = parsed.romeka.as_ref().expect("the carrier");
            let settings = carrier.settings.as_object().expect("settings");
            for key in settings.keys().filter(|key| *key != "section") {
                let alone = BlockLayoutData::parse(&json!({ "romeka": {
                    "block": carrier.block, "v": carrier.v,
                    "settings": { key.as_str(): settings[key] },
                }}));
                assert_ne!(
                    RomekaStyle::parse(&alone),
                    RomekaStyle::default(),
                    "{id}: `{key}` is not styling the renderer reads"
                );
            }
        }
    }
}
