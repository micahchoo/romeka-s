//! Whether a block is written: the words a visitor reads are in it.
//!
//! A block's schema marks at most one field primary ([`Field::primary`]): the
//! hero's heading, an HTML block's text, a quote, a list's pages. While that
//! field is empty the block is not written. The page render leaves such a
//! block out for visitors, so a template's empty block never reaches them, and
//! the preview shows the field's hint where the words will go. The plain
//! rendering of an unwritten Romeka block is empty, so an upstream reader sees
//! nothing false either.
//!
//! The admin counts unwritten blocks by the same rule
//! (`web/admin/src/pages/sites/editor/written.ts`). The generated registry
//! carries [`BLANK_HTML_SAMPLES`], and both sides test their blank-HTML check
//! against it, so the two cannot disagree on what "empty" means.

use omeka_core::site::{BlockData, SiteBlock};
use serde::Serialize;
use serde_json::Value as JsonValue;

use super::romeka::{self, RomkaBlock};
use super::schema::{Field, FieldKind, LayoutSchema};
use crate::html::escape_html;
use crate::view::RenderContext;

/// A block whose primary field is empty.
pub struct Unwritten {
    /// The empty field.
    pub field: &'static Field,
    /// Where the field is stored.
    place: Place,
}

/// Where an unwritten block keeps its primary field.
enum Place {
    /// In the settings of the Romeka block the carrier names.
    Settings(&'static dyn RomkaBlock),
    /// In the block's own `o:data`.
    Data,
}

/// The schema's primary field, if it marks one.
#[must_use]
pub fn primary_field(schema: &'static LayoutSchema) -> Option<&'static Field> {
    schema.fields.iter().find(|field| field.primary)
}

/// The block's empty primary field, `None` when the block is written or its
/// layout marks no primary field.
///
/// A carrier whose stored HTML the hash vouches for renders from its settings,
/// so its settings decide. A carrier edited outside Romeka S renders its
/// stored HTML, so that HTML decides, by the `html` layout's own rule.
#[must_use]
pub fn unwritten(block: &SiteBlock) -> Option<Unwritten> {
    if let (Some(carrier), BlockData::Html { html }) = (&block.layout_data.romeka, &block.data)
        && let Some(definition) = romeka::block(carrier.block.as_str())
        && romeka::stored_matches(carrier, html)
    {
        let field = primary_field(definition.schema())?;
        return is_blank(field, carrier.settings.get(field.key)).then_some(Unwritten {
            field,
            place: Place::Settings(definition),
        });
    }
    let field = primary_field(super::layout_for_block(block).schema())?;
    let data = block.data.to_json();
    is_blank(field, data.get(field.key)).then_some(Unwritten {
        field,
        place: Place::Data,
    })
}

/// Whether a Romeka block's settings leave its primary field empty, so its
/// plain rendering is empty too.
#[must_use]
pub fn settings_unwritten(definition: &dyn RomkaBlock, settings: &JsonValue) -> bool {
    primary_field(definition.schema()).is_some_and(|field| is_blank(field, settings.get(field.key)))
}

/// Whether a stored value leaves `field` empty.
fn is_blank(field: &Field, value: Option<&JsonValue>) -> bool {
    let text = match value {
        None | Some(JsonValue::Null) => return true,
        Some(JsonValue::String(text)) => text.as_str(),
        Some(_) => return false,
    };
    match field.kind {
        FieldKind::Html { .. } => is_blank_html(text),
        FieldKind::PageList { .. } => {
            text.trim().is_empty()
                || serde_json::from_str::<Vec<JsonValue>>(text).is_ok_and(|nodes| nodes.is_empty())
        }
        _ => text.trim().is_empty(),
    }
}

/// The tags a rich editor leaves behind in an emptied field. Any other tag (an
/// image, an embed, a rule) is content.
const EMPTY_TAGS: &[&str] = &["p", "br", "div", "span"];

/// Whether HTML holds no words and no element a visitor would see: only
/// whitespace, non-breaking spaces and the empty wrappers a rich editor leaves
/// (`<p><br></p>`).
#[must_use]
pub fn is_blank_html(html: &str) -> bool {
    let mut rest = html;
    while let Some(start) = rest.find('<') {
        if !is_blank_text(&rest[..start]) {
            return false;
        }
        let Some(end) = rest[start..].find('>') else {
            return false;
        };
        let tag = &rest[start + 1..start + end];
        let name: String = tag
            .trim_start_matches('/')
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric())
            .collect::<String>()
            .to_ascii_lowercase();
        if !EMPTY_TAGS.contains(&name.as_str()) {
            return false;
        }
        rest = &rest[start + end + 1..];
    }
    is_blank_text(rest)
}

/// Whether text between tags is only whitespace and non-breaking spaces.
fn is_blank_text(text: &str) -> bool {
    text.replace("&nbsp;", " ")
        .replace("&#160;", " ")
        .trim()
        .is_empty()
}

/// One blank-HTML case the admin's check is tested against.
#[derive(Debug, Serialize)]
pub struct BlankSample {
    /// The stored HTML.
    pub html: &'static str,
    /// Whether [`is_blank_html`] calls it empty.
    pub blank: bool,
}

/// The cases both blank-HTML checks agree on.
pub const BLANK_HTML_SAMPLES: &[BlankSample] = &[
    BlankSample {
        html: "",
        blank: true,
    },
    BlankSample {
        html: "  \n ",
        blank: true,
    },
    BlankSample {
        html: "<p></p>",
        blank: true,
    },
    BlankSample {
        html: "<p><br></p>",
        blank: true,
    },
    BlankSample {
        html: "<p>&nbsp;</p><div class=\"x\"> <br/> </div>",
        blank: true,
    },
    BlankSample {
        html: "<p>Words</p>",
        blank: false,
    },
    BlankSample {
        html: "Plain words",
        blank: false,
    },
    BlankSample {
        html: "<p><img src=\"/files/asset/a.png\" alt=\"\"></p>",
        blank: false,
    },
    BlankSample {
        html: "<hr>",
        blank: false,
    },
    BlankSample {
        html: "<p>&amp;</p>",
        blank: false,
    },
];

/// The token the preview stands in for an empty field while the block renders,
/// then swaps for the hint. It is plain ASCII, so escaping and purifying keep
/// it as it is.
const HINT_SLOT: &str = "romeka-hint-slot-3f9c2a";

/// The preview of an unwritten block: the block as it will look, with the
/// field's hint, greyed, where its words go.
///
/// The block renders with a token in the empty field, and the token becomes
/// the hint. A field the renderer does not print once as text (a page tree)
/// shows the hint as the block's whole body. With no hint text in the request
/// the block renders as it is.
#[must_use]
pub(crate) fn render_hinted(
    block: &SiteBlock,
    unwritten: &Unwritten,
    ctx: &RenderContext<'_>,
) -> String {
    let hint = unwritten.field.hint().and_then(|key| ctx.hint(key));
    let Some(hint) = hint else {
        return super::render_block(block, ctx);
    };
    let hint = escape_html(hint);
    let slotted = match unwritten.field.kind {
        FieldKind::Text { .. } | FieldKind::Html { .. } => render_with_slot(block, unwritten, ctx),
        _ => None,
    };
    match slotted {
        Some(rendered) if rendered.matches(HINT_SLOT).count() == 1 => rendered.replace(
            HINT_SLOT,
            &format!("<span class=\"romeka-hint\">{hint}</span>"),
        ),
        _ => format!("<p class=\"romeka-hint\">{hint}</p>"),
    }
}

/// The block rendered with [`HINT_SLOT`] in its empty primary field.
fn render_with_slot(
    block: &SiteBlock,
    unwritten: &Unwritten,
    ctx: &RenderContext<'_>,
) -> Option<String> {
    let key = unwritten.field.key;
    match unwritten.place {
        Place::Settings(definition) => {
            let mut settings = block.layout_data.romeka.as_ref()?.settings.clone();
            settings
                .as_object_mut()?
                .insert(key.to_string(), JsonValue::String(HINT_SLOT.to_string()));
            Some(definition.render(&settings, ctx))
        }
        Place::Data => {
            let mut data = block.data.to_json();
            data.as_object_mut()?
                .insert(key.to_string(), JsonValue::String(HINT_SLOT.to_string()));
            let slotted = SiteBlock {
                data: BlockData::parse(&block.layout, &data).ok()?,
                ..block.clone()
            };
            Some(super::render_block(&slotted, ctx))
        }
    }
}

#[cfg(test)]
#[path = "written_tests.rs"]
mod tests;
