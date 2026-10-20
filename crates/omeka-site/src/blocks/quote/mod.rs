//! The `quote` Romeka block: a quotation with its attribution and, when the
//! curator names one, the item it comes from.
//!
//! The block is stored as an upstream `html` block whose plain rendering is a
//! `blockquote` and a line with the attribution and a `cite` linking the
//! source item, so upstream shows the same quotation.

use omeka_core::ids::ItemId;
use omeka_core::resource::ResourceKind;
use serde_json::Value as JsonValue;

use super::romeka::{ItemSelection, PlainRefs, RomkaBlock};
use super::schema::{ChoiceOption, LayoutSchema, choice, html, item_ref, option, text};
use super::setting::{item_at, text as text_of};
use crate::html::escape_html;
use crate::view::RenderContext;

/// The `quote` block.
#[derive(Debug, Clone, Copy, Default)]
pub struct Quote;

const STYLE: &[ChoiceOption] = &[
    option("plain", "romekaChoice.quote.plain"),
    option("large", "romekaChoice.quote.large"),
    option("band", "romekaChoice.quote.band"),
];

const SCHEMA: LayoutSchema = LayoutSchema {
    fields: &[
        html("text", "romekaField.quote.text", "fieldHelp.quote.text").required(),
        text(
            "attribution",
            "romekaField.quote.attribution",
            "fieldHelp.quote.attribution",
        )
        .placeholder("fieldExample.quote.attribution"),
        item_ref("item", "romekaField.quote.item", "fieldHelp.quote.item"),
        choice(
            "style",
            "romekaField.quote.style",
            "fieldHelp.quote.style",
            STYLE,
        )
        .default_text("large"),
    ],
};

/// The block's typed settings.
#[derive(Debug, Clone, PartialEq, Eq)]
struct QuoteSettings {
    /// The quotation, purified.
    text: String,
    attribution: String,
    item: Option<ItemId>,
    style: &'static str,
}

impl QuoteSettings {
    fn parse(settings: &JsonValue) -> Self {
        let style = text_of(settings, "style");
        Self {
            text: omeka_core::purify::purify(text_of(settings, "text").trim()),
            attribution: text_of(settings, "attribution").trim().to_string(),
            item: item_at(settings, "item"),
            style: STYLE
                .iter()
                .find(|option| option.value == style)
                .map_or("large", |option| option.value),
        }
    }

    /// The quotation as block content: as stored when it holds a block
    /// element, else wrapped in a paragraph.
    fn body(&self) -> String {
        if self.text.starts_with('<') {
            self.text.clone()
        } else {
            format!("<p>{}</p>", self.text)
        }
    }
}

/// The attribution line's content: the name, then the linked source title,
/// or `None` when the quote names neither.
fn attribution(name: &str, source: Option<(&str, &str)>) -> Option<String> {
    let cite = source.map(|(title, url)| {
        format!(
            "<cite><a href=\"{}\">{}</a></cite>",
            escape_html(url),
            escape_html(title)
        )
    });
    match (name.is_empty(), cite) {
        (true, None) => None,
        (true, Some(cite)) => Some(cite),
        (false, None) => Some(escape_html(name)),
        (false, Some(cite)) => Some(format!("{}, {cite}", escape_html(name))),
    }
}

impl RomkaBlock for Quote {
    fn name(&self) -> &'static str {
        "quote"
    }

    fn label(&self) -> &'static str {
        "romeka.quote"
    }

    fn schema(&self) -> &'static LayoutSchema {
        &SCHEMA
    }

    fn render(&self, settings: &JsonValue, ctx: &RenderContext<'_>) -> String {
        let settings = QuoteSettings::parse(settings);
        if settings.text.is_empty() {
            return if ctx.is_preview() {
                format!(
                    "<p class=\"romeka-quote-empty\">{}</p>",
                    escape_html(ctx.translator().messages().quote_empty_preview)
                )
            } else {
                String::new()
            };
        }
        let source = settings
            .item
            .and_then(|id| ctx.resources.read(ResourceKind::Item, id.get()));
        let mut out = format!(
            "<figure class=\"romeka-quote romeka-quote-{}\"><blockquote class=\"romeka-quote-text\">{}</blockquote>",
            settings.style,
            settings.body()
        );
        if let Some(line) = attribution(
            &settings.attribution,
            source
                .as_ref()
                .map(|item| (item.title.as_str(), item.url.as_str())),
        ) {
            out.push_str(&format!(
                "<figcaption class=\"romeka-quote-attribution\">{line}</figcaption>"
            ));
        }
        out.push_str("</figure>");
        out
    }

    fn plain(&self, settings: &JsonValue, refs: &PlainRefs) -> String {
        let settings = QuoteSettings::parse(settings);
        if settings.text.is_empty() {
            return String::new();
        }
        let source = settings.item.and_then(|id| refs.item_of(id));
        let mut out = format!("<blockquote>{}</blockquote>", settings.body());
        if let Some(line) = attribution(
            &settings.attribution,
            source.map(|item| (item.title.as_str(), item.url.as_str())),
        ) {
            out.push_str(&format!("<p>{line}</p>"));
        }
        out
    }

    fn items(&self, settings: &JsonValue) -> Option<ItemSelection> {
        QuoteSettings::parse(settings)
            .item
            .map(|id| ItemSelection::Picked(vec![id]))
    }
}

#[cfg(test)]
mod tests;
