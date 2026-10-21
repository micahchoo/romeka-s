//! The `gallery` Romeka block: images from items' media or from assets, laid
//! out as a grid, a masonry of columns or a carousel, each opening in a
//! lightbox.
//!
//! The block is stored as an upstream `html` block whose plain rendering is a
//! list of image links, each to the image's large file, with its caption, so
//! upstream shows the same images. The rich rendering links each image to the
//! same file, and `lightbox.js` turns those links into a lightbox: without the
//! script, a link opens the file. [`with_assets`] adds the script to a page
//! only when a gallery on it asks for the lightbox.

mod settings;

use omeka_core::ids::{AssetId, ItemId, MediaId};
use omeka_core::resource::ResourceKind;
use serde_json::Value as JsonValue;

use super::romeka::{ItemSelection, PlainImage, PlainRefs, RomkaBlock};
use super::schema::{
    Accepts, ChoiceOption, LayoutSchema, When, asset_ref, boolean, choice, html, integer_range,
    item_media, option, records,
};
use crate::html::{Attr, escape_html, open_tag};
use crate::theme::library_script;
use crate::view::{RenderContext, Thumbnail};
use settings::{COLUMNS, GallerySettings, Layout, Pictures};

/// The `gallery` block.
#[derive(Debug, Clone, Copy, Default)]
pub struct Gallery;

/// The attribute that marks a gallery for `lightbox.js`. The purifier drops
/// every `data-` attribute from stored html, so no stored block can ask for
/// the script.
pub const MARKER: &str = "data-romeka-lightbox";

/// The script that opens a gallery's images in a lightbox, served as
/// `js/lightbox.js`.
pub const LIGHTBOX_SCRIPT: &str = include_str!("lightbox.js");

const SOURCE: &[ChoiceOption] = &[
    option("media", "romekaChoice.gallery.media"),
    option("assets", "romekaChoice.gallery.assets"),
];

const LAYOUT: &[ChoiceOption] = &[
    option("grid", "romekaChoice.gallery.grid"),
    option("masonry", "romekaChoice.gallery.masonry"),
    option("carousel", "romekaChoice.gallery.carousel"),
];

const ASSET_FIELDS: &[super::schema::Field] = &[
    asset_ref(
        "asset",
        "romekaField.gallery.asset",
        "fieldHelp.gallery.asset",
        Accepts::Image,
    ),
    html(
        "caption",
        "romekaField.gallery.caption",
        "fieldHelp.gallery.caption",
    ),
];

const SCHEMA: LayoutSchema = LayoutSchema {
    fields: &[
        choice(
            "source",
            "romekaField.gallery.source",
            "fieldHelp.gallery.source",
            SOURCE,
        )
        .default_text("media"),
        item_media(
            "media",
            "romekaField.gallery.media",
            "fieldHelp.gallery.media",
        )
        .when(When::equals("source", "media")),
        records(
            "assets",
            "romekaField.gallery.assets",
            "fieldHelp.gallery.assets",
            ASSET_FIELDS,
        )
        .when(When::equals("source", "assets")),
        choice(
            "layout",
            "romekaField.gallery.layout",
            "fieldHelp.gallery.layout",
            LAYOUT,
        )
        .default_text("grid"),
        integer_range(
            "columns",
            "romekaField.gallery.columns",
            "fieldHelp.gallery.columns",
            COLUMNS.0 as i64,
            COLUMNS.1 as i64,
        )
        .default_int(COLUMNS.2 as i64),
        boolean(
            "captions",
            "romekaField.gallery.captions",
            "fieldHelp.gallery.captions",
        )
        .default_bool(true),
        boolean(
            "lightbox",
            "romekaField.gallery.lightbox",
            "fieldHelp.gallery.lightbox",
        )
        .default_bool(true),
    ],
};

/// One image as both renderings show it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Picture {
    /// The large file, shown in the gallery and opened by its link.
    src: String,
    alt: String,
    caption: Caption,
}

/// What a picture's caption says: the curator's html, else the item's title.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Caption {
    None,
    Html(String),
    Title(String),
}

impl Caption {
    fn of(html: &str, title: Option<&str>) -> Self {
        if !html.trim().is_empty() {
            return Self::Html(omeka_core::purify::purify(html));
        }
        title.map_or(Self::None, |title| Self::Title(title.to_string()))
    }

    fn markup(&self) -> Option<String> {
        match self {
            Self::None => None,
            Self::Html(html) => Some(html.clone()),
            Self::Title(title) => Some(escape_html(title)),
        }
    }
}

/// The alt text a file carries, else `fallback`, so an image link always has
/// a name.
fn alt_or(alt: &str, fallback: &str) -> String {
    if alt.is_empty() {
        fallback.to_string()
    } else {
        alt.to_string()
    }
}

impl RomkaBlock for Gallery {
    fn name(&self) -> &'static str {
        "gallery"
    }

    fn label(&self) -> &'static str {
        "romeka.gallery"
    }

    fn schema(&self) -> &'static LayoutSchema {
        &SCHEMA
    }

    fn render(&self, settings: &JsonValue, ctx: &RenderContext<'_>) -> String {
        let settings = GallerySettings::parse(settings);
        let pictures = rich_pictures(&settings.pictures, ctx);
        let messages = ctx.translator().messages();
        if pictures.is_empty() {
            return if ctx.is_preview() {
                format!(
                    "<p class=\"romeka-gallery-empty\">{}</p>",
                    escape_html(messages.gallery_empty_preview)
                )
            } else {
                String::new()
            };
        }
        let classes = format!(
            "romeka-gallery romeka-gallery-{} romeka-gallery-columns-{}",
            settings.layout.as_str(),
            settings.columns
        );
        let mut attrs = vec![Attr::text("class", &classes)];
        if settings.lightbox {
            attrs.extend([
                Attr::text(MARKER, ""),
                Attr::text("data-close", messages.gallery_close),
                Attr::text("data-previous", messages.gallery_previous),
                Attr::text("data-next", messages.gallery_next),
                Attr::text("data-position", messages.gallery_position),
            ]);
        }
        let mut out = open_tag("div", &attrs);
        let mut list = vec![Attr::text("class", "romeka-gallery-list")];
        if settings.layout == Layout::Carousel {
            list.extend([
                Attr::text("tabindex", "0"),
                Attr::text("aria-label", messages.gallery),
            ]);
        }
        out.push_str(&open_tag("ul", &list));
        for picture in &pictures {
            out.push_str(
                "<li class=\"romeka-gallery-item\"><figure class=\"romeka-gallery-figure\">",
            );
            out.push_str(&open_tag(
                "a",
                &[
                    Attr::text("class", "romeka-gallery-link"),
                    Attr::text("href", &picture.src),
                ],
            ));
            out.push_str(&open_tag(
                "img",
                &[
                    Attr::text("class", "romeka-gallery-image"),
                    Attr::text("src", &picture.src),
                    Attr::text("alt", &picture.alt),
                    Attr::text("loading", "lazy"),
                ],
            ));
            out.push_str("</a>");
            if settings.captions
                && let Some(caption) = picture.caption.markup()
            {
                out.push_str(&format!(
                    "<figcaption class=\"romeka-gallery-caption\">{caption}</figcaption>"
                ));
            }
            out.push_str("</figure></li>");
        }
        out.push_str("</ul></div>");
        out
    }

    fn plain(&self, settings: &JsonValue, refs: &PlainRefs) -> String {
        let settings = GallerySettings::parse(settings);
        let rows: String = plain_pictures(&settings.pictures, refs)
            .iter()
            .map(|picture| {
                let image = open_tag(
                    "img",
                    &[
                        Attr::text("src", &picture.src),
                        Attr::text("alt", &picture.alt),
                    ],
                );
                let caption = settings
                    .captions
                    .then(|| picture.caption.markup())
                    .flatten()
                    .map(|caption| format!("<div>{caption}</div>"))
                    .unwrap_or_default();
                format!(
                    "<li><a href=\"{}\">{image}</a>{caption}</li>",
                    escape_html(&picture.src)
                )
            })
            .collect();
        if rows.is_empty() {
            return String::new();
        }
        format!("<ul>{rows}</ul>")
    }

    fn asset_refs(&self, settings: &JsonValue) -> Vec<AssetId> {
        match GallerySettings::parse(settings).pictures {
            Pictures::Assets(records) => records.iter().map(|record| record.asset).collect(),
            Pictures::Media(_) => Vec::new(),
        }
    }

    fn media_refs(&self, settings: &JsonValue) -> Vec<MediaId> {
        match GallerySettings::parse(settings).pictures {
            Pictures::Media(records) => records.iter().filter_map(|record| record.media).collect(),
            Pictures::Assets(_) => Vec::new(),
        }
    }

    fn items(&self, settings: &JsonValue) -> Option<ItemSelection> {
        let Pictures::Media(records) = GallerySettings::parse(settings).pictures else {
            return None;
        };
        let mut items: Vec<ItemId> = Vec::new();
        for record in &records {
            if !items.contains(&record.item) {
                items.push(record.item);
            }
        }
        (!items.is_empty()).then_some(ItemSelection::Picked(items))
    }
}

/// The pictures on the rendered page, skipping a record whose image the
/// visitor may not see.
fn rich_pictures(pictures: &Pictures, ctx: &RenderContext<'_>) -> Vec<Picture> {
    match pictures {
        Pictures::Media(records) => records
            .iter()
            .filter_map(|record| {
                let item = ctx.resources.read(ResourceKind::Item, record.item.get());
                let title = item.as_ref().map(|item| item.title.as_str());
                let image: Thumbnail = match record.media {
                    Some(id) => ctx.resources.read_image(id)?,
                    None => item.as_ref()?.primary_media.as_ref()?.thumbnail.clone()?,
                };
                Some(Picture {
                    alt: alt_or(&image.alt, title.unwrap_or_default()),
                    src: image.url,
                    caption: Caption::of(&record.caption, title),
                })
            })
            .collect(),
        Pictures::Assets(records) => records
            .iter()
            .filter_map(|record| {
                let asset = ctx.resources.read_asset(record.asset)?;
                let alt = asset
                    .thumbnail
                    .as_ref()
                    .map(|thumbnail| thumbnail.alt.clone())
                    .unwrap_or_default();
                Some(Picture {
                    src: asset.url?,
                    alt: alt_or(&alt, &asset.title),
                    caption: Caption::of(&record.caption, None),
                })
            })
            .collect(),
    }
}

/// The same pictures at write time, from the resolved references.
fn plain_pictures(pictures: &Pictures, refs: &PlainRefs) -> Vec<Picture> {
    let picture = |image: &PlainImage, fallback: &str, caption: Caption| Picture {
        src: image.url.clone(),
        alt: alt_or(&image.alt, fallback),
        caption,
    };
    match pictures {
        Pictures::Media(records) => records
            .iter()
            .filter_map(|record| {
                let item = refs.item_of(record.item);
                let title = item.map(|item| item.title.as_str());
                let image = match record.media {
                    Some(id) => refs.media_of(id)?,
                    None => item?.image.as_ref()?,
                };
                Some(picture(
                    image,
                    title.unwrap_or_default(),
                    Caption::of(&record.caption, title),
                ))
            })
            .collect(),
        Pictures::Assets(records) => records
            .iter()
            .filter_map(|record| {
                Some(picture(
                    refs.asset_of(record.asset)?,
                    "",
                    Caption::of(&record.caption, None),
                ))
            })
            .collect(),
    }
}

/// `content` with the lightbox script added when a gallery in it asks for the
/// lightbox, so a page without one loads no gallery code.
#[must_use]
pub fn with_assets(content: String, ctx: &RenderContext<'_>) -> String {
    if !content.contains(MARKER) {
        return content;
    }
    let mut out = content;
    out.push_str(&library_script(&ctx.asset_base, "js/lightbox.js"));
    out
}

#[cfg(test)]
mod tests;
