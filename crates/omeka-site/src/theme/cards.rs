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
use crate::i18n::sprintf1;
use crate::slug::controller_for;
use crate::theme::choices::CardStyle;
use crate::view::{MediaIcon, MediaKindCue, RenderContext, ResourceView, Thumbnail, ValueRef};

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

/// The glyph the theme draws for a media kind, in a tinted slot or a badge.
/// Decorative; the kind's word is always beside it.
#[must_use]
pub fn media_icon(icon: MediaIcon) -> &'static str {
    match icon {
        MediaIcon::Image => kind_icon(ResourceKind::Media),
        MediaIcon::Audio => {
            "<svg class=\"kind-icon\" viewBox=\"0 0 24 24\" aria-hidden=\"true\" focusable=\"false\">\
             <path d=\"M4 10v4 M8 7v10 M12 4v16 M16 7v10 M20 10v4\" fill=\"none\" \
             stroke=\"currentColor\" stroke-width=\"1.5\" stroke-linecap=\"round\"/></svg>"
        }
        MediaIcon::Video => {
            "<svg class=\"kind-icon\" viewBox=\"0 0 24 24\" aria-hidden=\"true\" focusable=\"false\">\
             <path d=\"M3.5 5h17v14h-17z\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"1.5\" \
             stroke-linejoin=\"round\"/><path d=\"M10 9.5v5l4.5-2.5z\" fill=\"currentColor\"/></svg>"
        }
        MediaIcon::Pdf => {
            "<svg class=\"kind-icon\" viewBox=\"0 0 24 24\" aria-hidden=\"true\" focusable=\"false\">\
             <path d=\"M6 3.5h8l4 4v13H6z M14 3.5v4h4 M9 12h6 M9 15h6\" fill=\"none\" \
             stroke=\"currentColor\" stroke-width=\"1.5\" stroke-linejoin=\"round\"/></svg>"
        }
        MediaIcon::Model => {
            "<svg class=\"kind-icon\" viewBox=\"0 0 24 24\" aria-hidden=\"true\" focusable=\"false\">\
             <path d=\"M3.5 7.5 12 3l8.5 4.5v9L12 21l-8.5-4.5z M3.5 7.5 12 12l8.5-4.5M12 12v9\" \
             fill=\"none\" stroke=\"currentColor\" stroke-width=\"1.5\" stroke-linejoin=\"round\"/></svg>"
        }
        MediaIcon::Captions => {
            "<svg class=\"kind-icon\" viewBox=\"0 0 24 24\" aria-hidden=\"true\" focusable=\"false\">\
             <path d=\"M3.5 5.5h17v11h-17z M7 9.5h4 M7 13h7 M14 9.5h3\" fill=\"none\" \
             stroke=\"currentColor\" stroke-width=\"1.5\" stroke-linejoin=\"round\"/></svg>"
        }
        MediaIcon::Embed => {
            "<svg class=\"kind-icon\" viewBox=\"0 0 24 24\" aria-hidden=\"true\" focusable=\"false\">\
             <path d=\"M3.5 4.5h17v15h-17z M3.5 12h17 M12 4.5c-3 3-3 12 0 15 M12 4.5c3 3 3 12 0 15\" \
             fill=\"none\" stroke=\"currentColor\" stroke-width=\"1.5\" stroke-linejoin=\"round\"/></svg>"
        }
        MediaIcon::File => {
            "<svg class=\"kind-icon\" viewBox=\"0 0 24 24\" aria-hidden=\"true\" focusable=\"false\">\
             <path d=\"M6 3.5h8l4 4v13H6z M14 3.5v4h4 M9 12h6 M9 15h6\" fill=\"none\" \
             stroke=\"currentColor\" stroke-width=\"1.5\" stroke-linejoin=\"round\"/></svg>"
        }
    }
}

/// The inside of a kind placeholder: the icon and the kind's word, the one
/// function the cards and the item page's file strip both draw.
#[must_use]
pub fn media_placeholder(cue: &MediaKindCue) -> String {
    format!(
        "{}<span class=\"resource-kind\">{}</span>",
        media_icon(cue.icon),
        escape_html(cue.label)
    )
}

/// A media kind as a card's thumbnail slot when the file has no picture: a
/// tinted surface with the kind's icon and word, plus the run time when an
/// extent row records one.
#[must_use]
pub fn media_thumb(cue: &MediaKindCue, duration_ms: Option<u64>) -> String {
    let duration = match duration_ms.filter(|_| is_recording(cue.icon)) {
        Some(ms) => format!(
            "<span class=\"resource-duration\"><span aria-hidden=\"true\">▶</span> {}</span>",
            duration_label(ms)
        ),
        None => String::new(),
    };
    format!(
        "<span class=\"resource-thumb resource-thumb-empty\" data-kind=\"media\" \
         data-media-kind=\"{}\">{}{duration}</span>",
        escape_html(cue.kind),
        media_placeholder(cue)
    )
}

/// The corner badge a card with a picture shows for a file that is not an
/// image: the kind in words, the play glyph and the run time for a recording.
/// An image card gets no badge, because its picture is the cue.
fn media_badge(cue: &MediaKindCue, duration_ms: Option<u64>) -> String {
    if cue.icon == MediaIcon::Image {
        return String::new();
    }
    let mut text = String::new();
    if is_recording(cue.icon) {
        text.push_str("<span aria-hidden=\"true\">▶</span> ");
    }
    text.push_str(&escape_html(cue.label));
    if let Some(ms) = duration_ms.filter(|_| is_recording(cue.icon)) {
        text.push_str(&format!(" · {}", duration_label(ms)));
    }
    format!("<span class=\"resource-badge\">{text}</span>")
}

const fn is_recording(icon: MediaIcon) -> bool {
    matches!(icon, MediaIcon::Audio | MediaIcon::Video)
}

/// A run time as a reader says it: `3:42`, and `1:02:03` past an hour.
#[must_use]
pub fn duration_label(ms: u64) -> String {
    let seconds = ms / 1_000;
    let (hours, minutes, seconds) = (seconds / 3_600, seconds / 60 % 60, seconds % 60);
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
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
    let cue = resource.media_kind.as_ref();
    let (thumb, state) = match &resource.thumbnail {
        _ if !parts.thumbnail => (String::new(), "no-thumb"),
        Some(Thumbnail { url, alt }) => (
            format!(
                "<span class=\"resource-thumb\"><img src=\"{}\" alt=\"{}\" loading=\"lazy\">{}</span>",
                escape_html(url),
                escape_html(alt),
                cue.map_or_else(String::new, |cue| {
                    media_badge(cue, resource.duration_ms)
                })
            ),
            "has-thumb",
        ),
        None => match cue {
            Some(cue) => (media_thumb(cue, resource.duration_ms), "no-thumb"),
            None => {
                let label = match kind {
                    ResourceKind::Item => ctx.translator().messages().no_file,
                    ResourceKind::ItemSet | ResourceKind::Media => {
                        type_label.unwrap_or_else(|| kind_label(kind, ctx))
                    }
                };
                (empty_thumb(kind, label), "no-thumb")
            }
        },
    };
    let mut meta: String = parts
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
    if resource.file_count > 1 {
        meta.push_str(&format!(
            "<span class=\"resource-meta-files\">{}</span>",
            escape_html(&sprintf1(
                ctx.translator().messages().files_count,
                &resource.file_count.to_string()
            ))
        ));
    }
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
    let kind_attribute = cue.map_or_else(String::new, |cue| {
        format!(" data-media-kind=\"{}\"", escape_html(cue.kind))
    });
    let mut out = format!(
        "<li class=\"{} resource {state}\"{kind_attribute}>",
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
    use super::{CardParts, duration_label, resource_card, strip_extension};
    use crate::blocks::test_support::{TestResources, context, item, site};
    use crate::theme::choices::CardStyle;
    use crate::view::{MediaIcon, MediaKindCue, Thumbnail};
    use omeka_core::resource::ResourceKind;

    fn cue(kind: &'static str, label: &'static str, icon: MediaIcon) -> MediaKindCue {
        MediaKindCue { kind, label, icon }
    }

    /// A card names the kind of its first file in words and in
    /// `data-media-kind`, counts the item's files, and draws the kind's icon
    /// instead of a generic placeholder.
    #[test]
    fn a_card_names_the_kind_of_its_first_file() {
        let site = site();
        let resources = TestResources::default();
        let ctx = context(&site, &resources);
        let mut audio = item(7, "Field recording", "/s/main/item/7");
        audio.media_kind = Some(cue("audio", "Audio", MediaIcon::Audio));
        audio.file_count = 3;
        let card = resource_card(
            ResourceKind::Item,
            &audio,
            &ctx,
            CardParts::all(CardStyle::Caption),
        );
        assert!(
            card.starts_with("<li class=\"item resource no-thumb\" data-media-kind=\"audio\">"),
            "{card}"
        );
        assert!(
            card.contains("<span class=\"resource-kind\">Audio</span>"),
            "{card}"
        );
        assert!(
            card.contains("<span class=\"resource-meta-files\">3 files</span>"),
            "{card}"
        );
        assert!(
            !card.contains("resource-badge"),
            "an empty slot has no badge: {card}"
        );
    }

    /// A picture of a non-image file carries the kind in a corner badge, with
    /// the run time only when an extent row recorded one.
    #[test]
    fn a_card_with_a_picture_badges_its_kind_and_run_time() {
        let site = site();
        let resources = TestResources::default();
        let ctx = context(&site, &resources);
        let mut video = item(8, "Interview", "/s/main/item/8");
        video.thumbnail = Some(Thumbnail::new("/files/square/8.jpg", ""));
        video.media_kind = Some(cue("video", "Video", MediaIcon::Video));
        video.duration_ms = Some(222_000);
        let card = resource_card(
            ResourceKind::Item,
            &video,
            &ctx,
            CardParts::all(CardStyle::Caption),
        );
        assert!(card.contains("data-media-kind=\"video\""), "{card}");
        assert!(
            card.contains(
                "<span class=\"resource-badge\"><span aria-hidden=\"true\">▶</span> \
                 Video · 3:42</span>"
            ),
            "{card}"
        );
    }

    /// An image card's picture is its cue: no badge, but the server's kind is
    /// still on the card for the matrix and for a style hook.
    #[test]
    fn an_image_card_has_no_badge() {
        let site = site();
        let resources = TestResources::default();
        let ctx = context(&site, &resources);
        let mut image = item(9, "Portrait", "/s/main/item/9");
        image.thumbnail = Some(Thumbnail::new("/files/square/9.jpg", ""));
        image.media_kind = Some(cue("local-image", "Image", MediaIcon::Image));
        let card = resource_card(
            ResourceKind::Item,
            &image,
            &ctx,
            CardParts::all(CardStyle::Caption),
        );
        assert!(card.contains("data-media-kind=\"local-image\""), "{card}");
        assert!(!card.contains("resource-badge"), "{card}");
    }

    /// An item with no file says so where the kind would stand, so it cannot
    /// be mistaken for a file of some other kind.
    #[test]
    fn an_item_with_no_file_says_no_file() {
        let site = site();
        let resources = TestResources::default();
        let ctx = context(&site, &resources);
        let bare = item(10, "Empty", "/s/main/item/10");
        let card = resource_card(
            ResourceKind::Item,
            &bare,
            &ctx,
            CardParts::all(CardStyle::Caption),
        );
        assert!(
            card.contains("<span class=\"resource-kind\">No file</span>"),
            "{card}"
        );
        assert!(!card.contains("data-media-kind"), "{card}");
    }

    /// A run time reads as minutes and seconds, and grows an hours field only
    /// past an hour.
    #[test]
    fn a_run_time_reads_as_minutes_and_seconds() {
        assert_eq!(duration_label(222_000), "3:42");
        assert_eq!(duration_label(0), "0:00");
        assert_eq!(duration_label(59_999), "0:59");
        assert_eq!(duration_label(3_723_000), "1:02:03");
    }

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
