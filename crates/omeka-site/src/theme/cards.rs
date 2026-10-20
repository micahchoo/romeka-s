//! A resource as a card in a list, and the name a resource shows, built for
//! poor data: many collections hold items with no title, no class and no
//! image, which differ only by identifier.
//!
//! A card always has a thumbnail slot. With no image the slot is a tinted
//! surface with the kind's icon and the resource's type, so a wall of
//! imageless items reads as a catalogue, not as one placeholder repeated. A
//! resource with no title shows its identifier, without a file extension, in
//! place of upstream's "[Untitled]" (a Romeka divergence, `docs/compat.md`).
//! The card carries details under its name: by default the type and date,
//! and the creator too in a row, which has room for it; a browse preview
//! block may choose its own ([`Detail`]).
//!
//! A list carries its card style as a class of its own ([`list_class`]), so a
//! block can draw its cards in another style than the theme's token.

use omeka_core::resource::ResourceKind;

use crate::html::escape_html;
use crate::slug::controller_for;
use crate::theme::choices::CardStyle;
use crate::view::{RenderContext, ResourceView, Thumbnail, ValueRef};

/// The class a resource list carries for the card style it draws. It is not
/// the root token's class (`cards-*`), which no element may repeat.
#[must_use]
pub const fn list_class(style: CardStyle) -> &'static str {
    match style {
        CardStyle::Caption => "resource-cards-caption",
        CardStyle::Overlay => "resource-cards-overlay",
        CardStyle::Row => "resource-cards-row",
    }
}

/// The opening tag of a resource list in the theme's card style.
#[must_use]
pub fn theme_list_open(ctx: &RenderContext<'_>) -> String {
    format!(
        "<ul class=\"resource-list {}\">",
        list_class(ctx.theme_settings.tokens.structure.cards)
    )
}

/// One detail a card shows under its name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Detail<'a> {
    /// The class, else the first `dcterms:type`.
    Type,
    /// The first `dcterms:creator`.
    Creator,
    /// The first `dcterms:date`.
    Date,
    /// The first item set the resource belongs to.
    ItemSet,
    /// The first value of a property, by term.
    Value(&'a str),
}

impl<'a> Detail<'a> {
    /// The details a card in `style` shows unless a block chooses its own: a
    /// row has room for the creator, a tile does not.
    #[must_use]
    pub const fn theme(style: CardStyle) -> &'static [Detail<'static>] {
        match style {
            CardStyle::Row => &[Detail::Type, Detail::Creator, Detail::Date],
            CardStyle::Caption | CardStyle::Overlay => &[Detail::Type, Detail::Date],
        }
    }

    /// The stored name of a detail a block may choose, `None` for any other.
    #[must_use]
    pub fn parse(name: &str) -> Option<Detail<'static>> {
        match name {
            "type" => Some(Detail::Type),
            "creator" => Some(Detail::Creator),
            "date" => Some(Detail::Date),
            "item_set" => Some(Detail::ItemSet),
            _ => None,
        }
    }

    /// The `resource-meta-*` suffix the sheet styles the detail by.
    const fn class(self) -> &'static str {
        match self {
            Self::Type => "type",
            Self::Creator => "creator",
            Self::Date => "date",
            Self::ItemSet => "item-set",
            Self::Value(_) => "value",
        }
    }

    /// The detail's text on `resource`, `None` when it has none.
    fn text(self, resource: &ResourceView) -> Option<&str> {
        match self {
            Self::Type => type_label(resource),
            Self::Creator => first_value(&resource.values, "dcterms:creator"),
            Self::Date => first_value(&resource.values, "dcterms:date"),
            Self::ItemSet => resource
                .item_sets
                .first()
                .map(|set| set.title.trim())
                .filter(|title| !title.is_empty()),
            Self::Value(term) => first_value(&resource.values, term),
        }
    }
}

/// The kind icon: an outline of a box for an item, a stack for an item set,
/// a frame for a media file. Decorative; the kind label beside it is text.
#[must_use]
pub fn kind_icon(kind: ResourceKind) -> &'static str {
    match kind {
        ResourceKind::Item => {
            "<svg class=\"kind-icon\" viewBox=\"0 0 24 24\" aria-hidden=\"true\" focusable=\"false\">\
             <path d=\"M3.5 7.5 12 3l8.5 4.5v9L12 21l-8.5-4.5z M3.5 7.5 12 12l8.5-4.5M12 12v9\" \
             fill=\"none\" stroke=\"currentColor\" stroke-width=\"1.5\" stroke-linejoin=\"round\"/></svg>"
        }
        ResourceKind::ItemSet => {
            "<svg class=\"kind-icon\" viewBox=\"0 0 24 24\" aria-hidden=\"true\" focusable=\"false\">\
             <path d=\"M4 9h16v11H4z M6 6h12 M8 3h8\" fill=\"none\" stroke=\"currentColor\" \
             stroke-width=\"1.5\" stroke-linejoin=\"round\" stroke-linecap=\"round\"/></svg>"
        }
        ResourceKind::Media => {
            "<svg class=\"kind-icon\" viewBox=\"0 0 24 24\" aria-hidden=\"true\" focusable=\"false\">\
             <path d=\"M3.5 4.5h17v15h-17z M3.5 16l5-5 4 4 3-3 5 5\" fill=\"none\" \
             stroke=\"currentColor\" stroke-width=\"1.5\" stroke-linejoin=\"round\"/></svg>"
        }
    }
}

/// The first value of a property, by term.
fn first_value<'a>(values: &'a [ValueRef], term: &str) -> Option<&'a str> {
    values
        .iter()
        .find(|value| value.property_term == term)
        .map(|value| value.value.trim())
        .filter(|value| !value.is_empty())
}

/// The name a resource shows and whether it is a fallback. A resource whose
/// title is the untitled placeholder shows its first `dcterms:identifier`
/// without a file extension; with no identifier it keeps the placeholder.
#[must_use]
pub fn display_name(resource: &ResourceView, ctx: &RenderContext<'_>) -> (String, bool) {
    if resource.title != ctx.translator().messages().untitled {
        return (resource.title.clone(), false);
    }
    let name = first_value(&resource.values, "dcterms:identifier").map_or_else(
        || resource.title.clone(),
        |id| strip_extension(id).to_string(),
    );
    (name, true)
}

/// An identifier without a trailing file extension: `photo_12.jpg` is
/// `photo_12`. A dot that starts the name or ends it, or an "extension" longer
/// than five characters, with no letter, or with a non-alphanumeric, is part
/// of the name, so `v1.2` stays whole.
fn strip_extension(identifier: &str) -> &str {
    match identifier.rsplit_once('.') {
        Some((stem, ext))
            if !stem.is_empty()
                && (1..=5).contains(&ext.len())
                && ext.chars().all(|ch| ch.is_ascii_alphanumeric())
                && ext.chars().any(|ch| ch.is_ascii_alphabetic()) =>
        {
            stem
        }
        _ => identifier,
    }
}

/// The name as markup. An identifier is one long word; a `<wbr>` after each
/// underscore or hyphen lets it wrap at its own separators rather than
/// mid-code in a narrow card.
fn name_markup(name: &str, fallback: bool) -> String {
    let escaped = escape_html(name);
    if fallback {
        escaped.replace('_', "_<wbr>").replace('-', "-<wbr>")
    } else {
        escaped
    }
}

/// The resource's type: its class, else its first `dcterms:type`.
#[must_use]
pub fn type_label(resource: &ResourceView) -> Option<&str> {
    resource
        .class_label
        .as_deref()
        .filter(|label| !label.is_empty())
        .or_else(|| first_value(&resource.values, "dcterms:type"))
}

/// The kind's own name: Item, Item set or Media.
#[must_use]
pub fn kind_label(kind: ResourceKind, ctx: &RenderContext<'_>) -> &'static str {
    let messages = ctx.translator().messages();
    match kind {
        ResourceKind::Item => messages.item,
        ResourceKind::ItemSet => messages.item_set,
        ResourceKind::Media => messages.media,
    }
}

/// The empty thumbnail: a tinted surface with the kind's icon and a label.
#[must_use]
pub fn empty_thumb(kind: ResourceKind, label: &str) -> String {
    format!(
        "<span class=\"resource-thumb resource-thumb-empty\" data-kind=\"{}\">{}\
         <span class=\"resource-kind\">{}</span></span>",
        controller_for(kind),
        kind_icon(kind),
        escape_html(label)
    )
}

/// Which parts of a card show: the browse preview block lets an editor turn
/// each off (its `components`) and choose the details; a browse page shows
/// them all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CardParts<'a> {
    /// The thumbnail slot.
    pub thumbnail: bool,
    /// The name and the details.
    pub heading: bool,
    /// The description.
    pub body: bool,
    /// The details under the name, in order.
    pub details: &'a [Detail<'a>],
}

impl CardParts<'static> {
    /// Every part, with the details of the theme's `style`.
    #[must_use]
    pub const fn all(style: CardStyle) -> Self {
        Self {
            thumbnail: true,
            heading: true,
            body: true,
            details: Detail::theme(style),
        }
    }
}

/// One `li` of a resource list (`view/omeka/site/item/browse.phtml` and the
/// browse preview block): the thumbnail slot, the name, the details, then the
/// description.
#[must_use]
pub fn resource_card(
    kind: ResourceKind,
    resource: &ResourceView,
    ctx: &RenderContext<'_>,
    parts: CardParts<'_>,
) -> String {
    let (name, fallback) = display_name(resource, ctx);
    let type_label = type_label(resource);
    let (thumb, state) = match &resource.thumbnail {
        _ if !parts.thumbnail => (String::new(), "no-thumb"),
        Some(Thumbnail { url, alt }) => (
            format!(
                "<span class=\"resource-thumb\"><img src=\"{}\" alt=\"{}\" loading=\"lazy\"></span>",
                escape_html(url),
                escape_html(alt)
            ),
            "has-thumb",
        ),
        None => (
            empty_thumb(kind, type_label.unwrap_or_else(|| kind_label(kind, ctx))),
            "no-thumb",
        ),
    };
    let meta: String = parts
        .details
        .iter()
        .filter_map(|detail| {
            detail.text(resource).map(|value| {
                format!(
                    "<span class=\"resource-meta-{}\">{}</span>",
                    detail.class(),
                    escape_html(value)
                )
            })
        })
        .collect();
    let mut text = String::new();
    if parts.heading {
        text = format!(
            "<span class=\"resource-text\"><span class=\"resource-name{}\">{}</span>",
            if fallback {
                " resource-name-fallback"
            } else {
                ""
            },
            name_markup(&name, fallback),
        );
        if !meta.is_empty() {
            text.push_str(&format!("<span class=\"resource-meta\">{meta}</span>"));
        }
        text.push_str("</span>");
    }
    let mut out = format!(
        "<li class=\"{} resource {state}\">",
        escape_html(controller_for(kind))
    );
    if !thumb.is_empty() || !text.is_empty() {
        out.push_str(&format!(
            "<a href=\"{}\" class=\"resource-link\">{thumb}{text}</a>",
            escape_html(&resource.url)
        ));
    }
    if let Some(body) = resource
        .description
        .as_deref()
        .filter(|body| parts.body && !body.is_empty())
    {
        out.push_str(&format!(
            "<div class=\"description\">{}</div>",
            escape_html(body)
        ));
    }
    out.push_str("</li>");
    out
}

/// The heading of an item, item-set or media page: the kind (and type) as a
/// small label, the display name, and a note when the name is a fallback.
#[must_use]
pub fn resource_header(resource: &ResourceView, ctx: &RenderContext<'_>) -> String {
    let (name, fallback) = display_name(resource, ctx);
    let kind = kind_label(resource.kind, ctx);
    let label = match type_label(resource) {
        Some(type_label) => format!("{} · {}", escape_html(kind), escape_html(type_label)),
        None => escape_html(kind),
    };
    let note = if fallback {
        format!(
            "<p class=\"resource-untitled-note\">{}</p>",
            escape_html(ctx.translator().messages().no_title_recorded)
        )
    } else {
        String::new()
    };
    format!(
        "<div class=\"resource-header\"><p class=\"resource-kind-label\">{label}</p>\
         <h2 class=\"resource-title{}\">{}</h2>{note}</div>",
        if fallback {
            " resource-name-fallback"
        } else {
            ""
        },
        name_markup(&name, fallback),
    )
}

#[cfg(test)]
mod tests {
    use super::strip_extension;

    /// A file extension goes; a dot that belongs to the name stays.
    #[test]
    fn strip_extension_keeps_names_with_dots() {
        assert_eq!(
            strip_extension("BHC006_GMT_PN_215.png"),
            "BHC006_GMT_PN_215"
        );
        assert_eq!(strip_extension("scan.tiff"), "scan");
        assert_eq!(strip_extension("v1.2"), "v1.2");
        assert_eq!(strip_extension("clip.mp4"), "clip");
        assert_eq!(strip_extension(".hidden"), ".hidden");
        assert_eq!(strip_extension("no-extension"), "no-extension");
        assert_eq!(strip_extension("doc.with-dash"), "doc.with-dash");
        assert_eq!(strip_extension("archive.longext"), "archive.longext");
    }
}
