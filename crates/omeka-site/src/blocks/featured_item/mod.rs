//! The `featuredItem` Romeka block: one item as a card, with its image beside
//! or above the text, the values of the fields the curator chose, and a link
//! to the item.
//!
//! The block is stored as an upstream `html` block whose plain rendering is a
//! heading, the image, a definition list of the values and the link, so
//! upstream shows the same card, plainer.

use omeka_core::ids::ItemId;
use omeka_core::resource::ResourceKind;
use serde_json::Value as JsonValue;

use super::romeka::{ItemSelection, PlainRefs, RomkaBlock};
use super::schema::{
    ChoiceOption, Field, LayoutSchema, choice, item_ref, option, property, records, text,
};
use super::setting::{item_at, records as records_at, text as text_of};
use crate::html::{Attr, escape_html, open_tag};
use crate::view::{RenderContext, ResourceView};

/// The `featuredItem` block.
#[derive(Debug, Clone, Copy, Default)]
pub struct FeaturedItem;

const IMAGE: &[ChoiceOption] = &[
    option("left", "blockChoice.left"),
    option("right", "blockChoice.right"),
    option("top", "romekaChoice.featuredItem.top"),
];

const FIELD: &[Field] = &[property(
    "property",
    "romekaField.featuredItem.property",
    "fieldHelp.featuredItem.property",
)];

const SCHEMA: LayoutSchema = LayoutSchema {
    fields: &[
        item_ref(
            "item",
            "romekaField.featuredItem.item",
            "fieldHelp.featuredItem.item",
        )
        .required(),
        choice(
            "image",
            "romekaField.featuredItem.image",
            "fieldHelp.featuredItem.image",
            IMAGE,
        )
        .default_text("left"),
        records(
            "fields",
            "romekaField.featuredItem.fields",
            "fieldHelp.featuredItem.fields",
            FIELD,
        ),
        text(
            "link_label",
            "romekaField.featuredItem.link_label",
            "fieldHelp.featuredItem.link_label",
        )
        .placeholder("fieldExample.featuredItem.link_label"),
    ],
};

/// Where the image sits beside the text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ImageAt {
    Left,
    Right,
    Top,
}

impl ImageAt {
    fn as_str(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Right => "right",
            Self::Top => "top",
        }
    }
}

/// The block's typed settings.
#[derive(Debug, Clone, PartialEq, Eq)]
struct FeaturedSettings {
    item: Option<ItemId>,
    image: ImageAt,
    /// The terms of the fields to show, in order; none shows the description.
    fields: Vec<String>,
    link_label: String,
}

impl FeaturedSettings {
    fn parse(settings: &JsonValue) -> Self {
        Self {
            item: item_at(settings, "item"),
            image: match text_of(settings, "image").as_str() {
                "right" => ImageAt::Right,
                "top" => ImageAt::Top,
                _ => ImageAt::Left,
            },
            fields: records_at(settings, "fields")
                .map(|record| text_of(record, "property"))
                .filter(|term| !term.is_empty())
                .collect(),
            link_label: text_of(settings, "link_label"),
        }
    }

    /// The link's text: the curator's, else the site's "View item".
    fn link_text<'a>(&'a self, view_item: &'a str) -> &'a str {
        if self.link_label.trim().is_empty() {
            view_item
        } else {
            &self.link_label
        }
    }
}

/// The description a card with no chosen fields shows.
const DESCRIPTION: &str = "dcterms:description";

impl RomkaBlock for FeaturedItem {
    fn name(&self) -> &'static str {
        "featuredItem"
    }

    fn label(&self) -> &'static str {
        "romeka.featuredItem"
    }

    fn schema(&self) -> &'static LayoutSchema {
        &SCHEMA
    }

    fn render(&self, settings: &JsonValue, ctx: &RenderContext<'_>) -> String {
        let settings = FeaturedSettings::parse(settings);
        let messages = ctx.translator().messages();
        let Some(item) = settings
            .item
            .and_then(|id| ctx.resources.read(ResourceKind::Item, id.get()))
        else {
            return if ctx.is_preview() {
                format!(
                    "<p class=\"romeka-featured-empty\">{}</p>",
                    escape_html(messages.featured_empty_preview)
                )
            } else {
                String::new()
            };
        };
        let image = item
            .primary_media
            .as_ref()
            .and_then(|media| media.thumbnail.clone())
            .or_else(|| item.thumbnail.clone());
        let mut classes = format!(
            "romeka-featured romeka-featured-image-{}",
            settings.image.as_str()
        );
        if image.is_none() {
            classes.push_str(" romeka-featured-no-image");
        }
        let mut out = open_tag("article", &[Attr::text("class", &classes)]);
        out.push_str("<div class=\"romeka-featured-card\">");
        if let Some(image) = &image {
            out.push_str("<div class=\"romeka-featured-media\">");
            out.push_str(&open_tag(
                "img",
                &[
                    Attr::text("class", "romeka-featured-image"),
                    Attr::text("src", &image.url),
                    Attr::text("alt", &image.alt),
                    Attr::text("loading", "lazy"),
                ],
            ));
            out.push_str("</div>");
        }
        out.push_str("<div class=\"romeka-featured-body\">");
        out.push_str(&format!(
            "<h3 class=\"romeka-featured-title\"><a href=\"{}\">{}</a></h3>",
            escape_html(&item.url),
            escape_html(&item.title)
        ));
        out.push_str(&rich_values(&settings, &item));
        out.push_str(&format!(
            "<p class=\"romeka-featured-actions\"><a class=\"romeka-featured-link\" href=\"{}\">{}</a></p>",
            escape_html(&item.url),
            escape_html(settings.link_text(messages.featured_view_item))
        ));
        out.push_str("</div></div></article>");
        out
    }

    fn plain(&self, settings: &JsonValue, refs: &PlainRefs) -> String {
        let settings = FeaturedSettings::parse(settings);
        let Some(item) = settings.item.and_then(|id| refs.item_of(id)) else {
            return String::new();
        };
        let url = escape_html(&item.url);
        let mut out = format!(
            "<h3><a href=\"{url}\">{}</a></h3>",
            escape_html(&item.title)
        );
        if let Some(image) = &item.image {
            out.push_str(&open_tag(
                "img",
                &[Attr::text("src", &image.url), Attr::text("alt", &image.alt)],
            ));
        }
        if settings.fields.is_empty() {
            if let Some(description) = item.texts(DESCRIPTION).next() {
                out.push_str(&format!("<p>{}</p>", escape_html(description)));
            }
        } else {
            let mut rows = String::new();
            for term in &settings.fields {
                let mut values = item
                    .values
                    .iter()
                    .filter(|value| &value.term == term)
                    .peekable();
                let Some(first) = values.peek() else {
                    continue;
                };
                rows.push_str(&format!("<dt>{}</dt>", escape_html(&first.label)));
                for value in values {
                    rows.push_str(&format!("<dd>{}</dd>", escape_html(&value.text)));
                }
            }
            if !rows.is_empty() {
                out.push_str(&format!("<dl>{rows}</dl>"));
            }
        }
        out.push_str(&format!(
            "<p><a href=\"{url}\">{}</a></p>",
            escape_html(settings.link_text(refs.messages().featured_view_item))
        ));
        out
    }

    fn items(&self, settings: &JsonValue) -> Option<ItemSelection> {
        FeaturedSettings::parse(settings)
            .item
            .map(|id| ItemSelection::Picked(vec![id]))
    }
}

/// The chosen fields' values as a definition list, or the description when no
/// field is chosen.
fn rich_values(settings: &FeaturedSettings, item: &ResourceView) -> String {
    if settings.fields.is_empty() {
        return item
            .description
            .as_ref()
            .map(|description| {
                format!(
                    "<p class=\"romeka-featured-description\">{}</p>",
                    escape_html(description)
                )
            })
            .unwrap_or_default();
    }
    let mut rows = String::new();
    for term in &settings.fields {
        let values: Vec<_> = item
            .values
            .iter()
            .filter(|value| &value.property_term == term)
            .collect();
        let Some(first) = values.first() else {
            continue;
        };
        rows.push_str(&format!(
            "<div class=\"romeka-featured-value\"><dt>{}</dt>",
            escape_html(&first.property)
        ));
        for value in values {
            rows.push_str(&format!("<dd>{}</dd>", value.html));
        }
        rows.push_str("</div>");
    }
    if rows.is_empty() {
        return String::new();
    }
    format!("<dl class=\"romeka-featured-values\">{rows}</dl>")
}

#[cfg(test)]
mod tests;
