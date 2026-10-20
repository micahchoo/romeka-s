//! The `timeline` Romeka block: chosen items placed in date order by a date
//! property, each with its thumbnail and title.
//!
//! The items come from the item picker or a browse query, as the map's do.
//! Each item's place is the first value of the chosen property that reads as
//! a date ([`date::Date`]); an item with no such value is listed after the
//! timeline, with the value it has. The block is stored as an upstream `html`
//! block whose plain rendering is the same list in date order.

mod date;

use serde_json::Value as JsonValue;

use super::romeka::{ItemSelection, PlainRefs, RomkaBlock};
use super::schema::{
    ChoiceOption, LayoutSchema, When, choice, item_list, item_query, option, property,
};
use super::setting::{selected, selection, text as text_of};
use crate::html::{Attr, escape_html, open_tag};
use crate::view::RenderContext;
use date::Date;

/// The `timeline` block.
#[derive(Debug, Clone, Copy, Default)]
pub struct Timeline;

const SOURCE: &[ChoiceOption] = &[
    option("items", "romekaChoice.map.items"),
    option("query", "romekaChoice.map.query"),
];

/// The property a timeline reads when none is chosen.
const DEFAULT_PROPERTY: &str = "dcterms:date";

const SCHEMA: LayoutSchema = LayoutSchema {
    fields: &[
        choice(
            "source",
            "romekaField.timeline.source",
            "fieldHelp.timeline.source",
            SOURCE,
        )
        .default_text("items"),
        item_list(
            "items",
            "romekaField.timeline.items",
            "fieldHelp.timeline.items",
        )
        .when(When::equals("source", "items")),
        item_query(
            "query",
            "romekaField.timeline.query",
            "fieldHelp.timeline.query",
        )
        .when(When::equals("source", "query")),
        property(
            "property",
            "romekaField.timeline.property",
            "fieldHelp.timeline.property",
        )
        .default_text(DEFAULT_PROPERTY),
    ],
};

/// The block's typed settings.
#[derive(Debug, Clone, PartialEq, Eq)]
struct TimelineSettings {
    selection: Option<ItemSelection>,
    property: String,
}

impl TimelineSettings {
    fn parse(settings: &JsonValue) -> Self {
        let property = text_of(settings, "property");
        Self {
            selection: selection(settings),
            property: if property.is_empty() {
                DEFAULT_PROPERTY.to_string()
            } else {
                property
            },
        }
    }
}

/// One item as the timeline places it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Entry {
    title: String,
    url: String,
    /// The thumbnail's URL and alt text.
    thumbnail: Option<(String, String)>,
    /// The value shown beside the title: the date as written, or the first
    /// value that is not a date.
    when: Option<String>,
    date: Option<Date>,
}

impl Entry {
    /// An entry placed by the first of `values` that is a date.
    fn new<'a>(
        title: &str,
        url: &str,
        thumbnail: Option<(String, String)>,
        values: impl Iterator<Item = &'a str>,
    ) -> Self {
        let values: Vec<&str> = values.collect();
        let dated = values
            .iter()
            .find_map(|text| Date::parse(text).map(|date| (*text, date)));
        Self {
            title: title.to_string(),
            url: url.to_string(),
            thumbnail,
            when: dated
                .map(|(text, _)| text)
                .or_else(|| values.first().copied())
                .map(|text| text.trim().to_string()),
            date: dated.map(|(_, date)| date),
        }
    }
}

/// The dated entries from the earliest, items of one date in selection
/// order, then the undated ones in selection order.
fn arrange(entries: Vec<Entry>) -> (Vec<Entry>, Vec<Entry>) {
    let (mut dated, undated): (Vec<Entry>, Vec<Entry>) =
        entries.into_iter().partition(|entry| entry.date.is_some());
    dated.sort_by_key(|entry| entry.date);
    (dated, undated)
}

impl RomkaBlock for Timeline {
    fn name(&self) -> &'static str {
        "timeline"
    }

    fn label(&self) -> &'static str {
        "romeka.timeline"
    }

    fn schema(&self) -> &'static LayoutSchema {
        &SCHEMA
    }

    fn render(&self, settings: &JsonValue, ctx: &RenderContext<'_>) -> String {
        let settings = TimelineSettings::parse(settings);
        let messages = ctx.translator().messages();
        let entries: Vec<Entry> = selected(settings.selection.as_ref(), ctx)
            .iter()
            .map(|item| {
                Entry::new(
                    &item.title,
                    &item.url,
                    item.thumbnail
                        .as_ref()
                        .map(|thumbnail| (thumbnail.url.clone(), thumbnail.alt.clone())),
                    item.values
                        .iter()
                        .filter(|value| value.property_term == settings.property)
                        .map(|value| value.value.as_str()),
                )
            })
            .collect();
        if entries.is_empty() {
            return if ctx.is_preview() {
                format!(
                    "<p class=\"romeka-timeline-empty\">{}</p>",
                    escape_html(messages.timeline_empty_preview)
                )
            } else {
                String::new()
            };
        }
        let (dated, undated) = arrange(entries);
        let mut out = String::from("<div class=\"romeka-timeline\">");
        if !dated.is_empty() {
            out.push_str("<ol class=\"romeka-timeline-list\">");
            for entry in &dated {
                out.push_str("<li class=\"romeka-timeline-entry\">");
                out.push_str(&format!(
                    "<p class=\"romeka-timeline-date\">{}</p>",
                    escape_html(entry.when.as_deref().unwrap_or_default())
                ));
                out.push_str(&open_tag(
                    "a",
                    &[
                        Attr::text("class", "romeka-timeline-link"),
                        Attr::text("href", &entry.url),
                    ],
                ));
                if let Some((url, alt)) = &entry.thumbnail {
                    out.push_str(&open_tag(
                        "img",
                        &[
                            Attr::text("class", "romeka-timeline-thumbnail"),
                            Attr::text("src", url),
                            Attr::text("alt", alt),
                            Attr::text("loading", "lazy"),
                        ],
                    ));
                }
                out.push_str(&format!(
                    "<span class=\"romeka-timeline-title\">{}</span></a></li>",
                    escape_html(&entry.title)
                ));
            }
            out.push_str("</ol>");
        }
        if !undated.is_empty() {
            out.push_str(&format!(
                "<h3 class=\"romeka-timeline-undated-heading\">{}</h3><ul class=\"romeka-timeline-undated\">",
                escape_html(messages.timeline_undated)
            ));
            for entry in &undated {
                out.push_str(&format!(
                    "<li><a href=\"{}\">{}</a>{}</li>",
                    escape_html(&entry.url),
                    escape_html(&entry.title),
                    entry
                        .when
                        .as_ref()
                        .map(|when| format!(
                            " <span class=\"romeka-timeline-value\">{}</span>",
                            escape_html(when)
                        ))
                        .unwrap_or_default()
                ));
            }
            out.push_str("</ul>");
        }
        out.push_str("</div>");
        out
    }

    fn plain(&self, settings: &JsonValue, refs: &PlainRefs) -> String {
        let settings = TimelineSettings::parse(settings);
        let entries: Vec<Entry> = refs
            .items()
            .iter()
            .map(|item| Entry::new(&item.title, &item.url, None, item.texts(&settings.property)))
            .collect();
        let (dated, undated) = arrange(entries);
        let row = |entry: &Entry, date_first: bool| {
            let link = format!(
                "<a href=\"{}\">{}</a>",
                escape_html(&entry.url),
                escape_html(&entry.title)
            );
            match (&entry.when, date_first) {
                (Some(when), true) => format!("<li>{}: {link}</li>", escape_html(when)),
                (Some(when), false) => format!("<li>{link} {}</li>", escape_html(when)),
                (None, _) => format!("<li>{link}</li>"),
            }
        };
        let mut out = String::new();
        if !dated.is_empty() {
            out.push_str("<ol>");
            out.extend(dated.iter().map(|entry| row(entry, true)));
            out.push_str("</ol>");
        }
        if !undated.is_empty() {
            out.push_str(&format!(
                "<h3>{}</h3><ul>",
                escape_html(refs.messages().timeline_undated)
            ));
            out.extend(undated.iter().map(|entry| row(entry, false)));
            out.push_str("</ul>");
        }
        out
    }

    fn items(&self, settings: &JsonValue) -> Option<ItemSelection> {
        TimelineSettings::parse(settings).selection
    }
}

#[cfg(test)]
mod tests;
