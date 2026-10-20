//! The `accordion` Romeka block: questions or headings, each opening its
//! answer.
//!
//! The rich rendering is `details` and `summary`, built here: the purifier
//! does not allow `details`, so it cannot come from stored html. The block is
//! stored as an upstream `html` block whose plain rendering is each heading as
//! an `h3` followed by its answer, so upstream shows every answer open.

use serde_json::Value as JsonValue;

use super::romeka::{PlainRefs, RomkaBlock};
use super::schema::{Field, LayoutSchema, html, records, text};
use super::setting::{records as records_at, text as text_of};
use crate::html::escape_html;
use crate::view::RenderContext;

/// The `accordion` block.
#[derive(Debug, Clone, Copy, Default)]
pub struct Accordion;

const ENTRY: &[Field] = &[
    text(
        "heading",
        "romekaField.accordion.heading",
        "fieldHelp.accordion.heading",
    ),
    html(
        "answer",
        "romekaField.accordion.answer",
        "fieldHelp.accordion.answer",
    ),
];

const SCHEMA: LayoutSchema = LayoutSchema {
    fields: &[records(
        "entries",
        "romekaField.accordion.entries",
        "fieldHelp.accordion.entries",
        ENTRY,
    )],
};

/// One heading and its answer, the answer purified.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Entry {
    heading: String,
    answer: String,
}

/// The entries with a heading, in order.
fn entries(settings: &JsonValue) -> Vec<Entry> {
    records_at(settings, "entries")
        .filter_map(|record| {
            let heading = text_of(record, "heading").trim().to_string();
            (!heading.is_empty()).then(|| Entry {
                heading,
                answer: omeka_core::purify::purify(&text_of(record, "answer")),
            })
        })
        .collect()
}

impl RomkaBlock for Accordion {
    fn name(&self) -> &'static str {
        "accordion"
    }

    fn label(&self) -> &'static str {
        "romeka.accordion"
    }

    fn schema(&self) -> &'static LayoutSchema {
        &SCHEMA
    }

    fn render(&self, settings: &JsonValue, ctx: &RenderContext<'_>) -> String {
        let entries = entries(settings);
        if entries.is_empty() {
            return if ctx.is_preview() {
                format!(
                    "<p class=\"romeka-accordion-empty\">{}</p>",
                    escape_html(ctx.translator().messages().accordion_empty_preview)
                )
            } else {
                String::new()
            };
        }
        let items: String = entries
            .iter()
            .map(|entry| {
                format!(
                    "<details class=\"romeka-accordion-item\"><summary class=\"romeka-accordion-heading\">{}</summary>\
                     <div class=\"romeka-accordion-answer\">{}</div></details>",
                    escape_html(&entry.heading),
                    entry.answer
                )
            })
            .collect();
        format!("<div class=\"romeka-accordion\">{items}</div>")
    }

    fn plain(&self, settings: &JsonValue, _refs: &PlainRefs) -> String {
        entries(settings)
            .iter()
            .map(|entry| format!("<h3>{}</h3>{}", escape_html(&entry.heading), entry.answer))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks::test_support::{context, site};
    use crate::view::{BlockMarkers, NoResources};
    use serde_json::json;

    fn settings() -> JsonValue {
        json!({"entries": [
            {"heading": "When are you open?", "answer": "<p>Every day <strong>but Monday</strong>.</p>"},
            {"heading": "  ", "answer": "<p>An answer with no question</p>"},
            {"heading": "Can I <bring> a dog?", "answer": "<p>Yes.</p><script>x</script>"},
        ]})
    }

    /// The plain rendering is each heading as an `h3` and its purified
    /// answer, open, because the purifier allows no `details`. An entry with
    /// no heading is left out.
    #[test]
    fn plain_renders_headings_and_answers() {
        assert_eq!(
            Accordion.plain(&settings(), &PlainRefs::new()),
            "<h3>When are you open?</h3><p>Every day <strong>but Monday</strong>.</p>\
             <h3>Can I &lt;bring&gt; a dog?</h3><p>Yes.</p>"
        );
        assert_eq!(Accordion.plain(&json!({}), &PlainRefs::new()), "");
    }

    /// The purifier drops `details` and `summary` and keeps their text, which
    /// is why the rich rendering is built here and the plain one uses `h3`.
    #[test]
    fn the_purifier_drops_details() {
        assert_eq!(
            omeka_core::purify::purify("<details><summary>Q</summary><p>A</p></details>"),
            "Q<p>A</p>"
        );
    }

    /// The rich rendering is one closed `details` per entry.
    #[test]
    fn rich_renders_details_and_summary() {
        let site = site();
        let ctx = context(&site, &NoResources);
        let rich = Accordion.render(&settings(), &ctx);
        assert!(rich.starts_with("<div class=\"romeka-accordion\"><details class=\"romeka-accordion-item\">\
             <summary class=\"romeka-accordion-heading\">When are you open?</summary>\
             <div class=\"romeka-accordion-answer\"><p>Every day <strong>but Monday</strong>.</p></div></details>"), "{rich}");
        assert_eq!(rich.matches("<details").count(), 2, "{rich}");
        assert!(!rich.contains("<script"), "{rich}");
    }

    /// No entry shows nothing publicly and a hint in the preview.
    #[test]
    fn an_empty_accordion_renders_nothing_publicly_and_a_hint_in_the_preview() {
        let site = site();
        let ctx = context(&site, &NoResources);
        assert_eq!(Accordion.render(&json!({}), &ctx), "");
        let mut preview = context(&site, &NoResources);
        preview.block_markers = BlockMarkers::Preview;
        assert!(
            Accordion
                .render(&json!({}), &preview)
                .contains("romeka-accordion-empty")
        );
    }
}
