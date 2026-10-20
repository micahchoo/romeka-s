//! The `button` Romeka block: one or two links drawn as buttons, each to a
//! page of the site or a URL.
//!
//! The block is stored as an upstream `html` block whose plain rendering is
//! the links in one paragraph, so upstream shows the same links as text.

use omeka_core::ids::PageId;
use serde_json::Value as JsonValue;

use super::romeka::{PlainRefs, RomkaBlock};
use super::schema::{ChoiceOption, LayoutSchema, choice, link, option, text};
use super::setting::{Button, text as text_of};
use crate::html::{Attr, escape_html, hyperlink};
use crate::view::RenderContext;

/// The `button` block.
#[derive(Debug, Clone, Copy, Default)]
pub struct ButtonBlock;

const STYLE: &[ChoiceOption] = &[
    option("solid", "romekaChoice.button.solid"),
    option("outline", "romekaChoice.button.outline"),
];

const ALIGNMENT: &[ChoiceOption] = &[
    option("left", "blockChoice.left"),
    option("center", "blockChoice.center"),
    option("right", "blockChoice.right"),
];

const SIZE: &[ChoiceOption] = &[
    option("small", "romekaChoice.button.small"),
    option("medium", "romekaChoice.medium"),
    option("large", "romekaChoice.button.large"),
];

const SCHEMA: LayoutSchema = LayoutSchema {
    fields: &[
        text(
            "button_label",
            "romekaField.button.button_label",
            "fieldHelp.button.button_label",
        )
        .placeholder("fieldExample.button.button_label")
        .required(),
        link(
            "button_page",
            "button_url",
            "romekaField.button.button_page",
            "fieldHelp.button.button_page",
        ),
        text(
            "button2_label",
            "romekaField.button.button2_label",
            "fieldHelp.button.button2_label",
        ),
        link(
            "button2_page",
            "button2_url",
            "romekaField.button.button2_page",
            "fieldHelp.button.button2_page",
        ),
        choice(
            "style",
            "romekaField.button.style",
            "fieldHelp.button.style",
            STYLE,
        )
        .default_text("solid"),
        choice(
            "alignment",
            "romekaField.button.alignment",
            "fieldHelp.button.alignment",
            ALIGNMENT,
        )
        .default_text("left"),
        choice(
            "size",
            "romekaField.button.size",
            "fieldHelp.button.size",
            SIZE,
        )
        .default_text("medium"),
    ],
};

/// The block's typed settings.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ButtonSettings {
    buttons: [Button; 2],
    style: &'static str,
    alignment: &'static str,
    size: &'static str,
}

impl ButtonSettings {
    fn parse(settings: &JsonValue) -> Self {
        let pick = |key: &str, allowed: &'static [ChoiceOption]| -> &'static str {
            let stored = text_of(settings, key);
            allowed
                .iter()
                .find(|option| option.value == stored)
                .map_or(allowed[0].value, |option| option.value)
        };
        Self {
            buttons: [
                Button::parse(settings, "button"),
                Button::parse(settings, "button2"),
            ],
            style: pick("style", STYLE),
            alignment: pick("alignment", ALIGNMENT),
            size: match text_of(settings, "size").as_str() {
                "small" => "small",
                "large" => "large",
                _ => "medium",
            },
        }
    }
}

impl RomkaBlock for ButtonBlock {
    fn name(&self) -> &'static str {
        "button"
    }

    fn label(&self) -> &'static str {
        "romeka.button"
    }

    fn schema(&self) -> &'static LayoutSchema {
        &SCHEMA
    }

    fn render(&self, settings: &JsonValue, ctx: &RenderContext<'_>) -> String {
        let settings = ButtonSettings::parse(settings);
        let class = format!(
            "romeka-button romeka-button-{} romeka-button-{}",
            settings.style, settings.size
        );
        let links: Vec<String> = settings
            .buttons
            .iter()
            .filter_map(|button| {
                let href = button.href(ctx)?;
                Some(hyperlink(
                    &button.label,
                    Some(&href),
                    &[Attr::text("class", &class)],
                ))
            })
            .collect();
        if links.is_empty() {
            return if ctx.is_preview() {
                format!(
                    "<p class=\"romeka-buttons-empty\">{}</p>",
                    escape_html(ctx.translator().messages().button_empty_preview)
                )
            } else {
                String::new()
            };
        }
        format!(
            "<div class=\"romeka-buttons romeka-buttons-align-{}\">{}</div>",
            settings.alignment,
            links.concat()
        )
    }

    fn plain(&self, settings: &JsonValue, refs: &PlainRefs) -> String {
        let settings = ButtonSettings::parse(settings);
        let links: Vec<String> = settings
            .buttons
            .iter()
            .filter_map(|button| {
                let href = button.plain_href(refs)?;
                Some(hyperlink(&button.label, Some(&href), &[]))
            })
            .collect();
        if links.is_empty() {
            return String::new();
        }
        format!("<p>{}</p>", links.join(" "))
    }

    fn page_refs(&self, settings: &JsonValue) -> Vec<PageId> {
        ButtonSettings::parse(settings)
            .buttons
            .iter()
            .filter_map(|button| button.page)
            .collect()
    }
}

#[cfg(test)]
mod tests;
