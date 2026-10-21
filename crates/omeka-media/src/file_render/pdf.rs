//! The PDF file renderer. Upstream has none: a PDF falls through to the
//! thumbnail renderer, so the page shows a picture of the first page linked to
//! the file and never the document itself.
//!
//! This renderer draws the browser's own PDF reader on the file's page, with a
//! card as its fallback and a second card for the phone rule. The card is a
//! link to the original with the first-page derivative as its picture, the
//! display title, and the kind and size. The theme's sheet swaps the reader for
//! the sibling card on a phone, because a phone browser draws no scrollable
//! inline PDF.

use crate::file_render::{FileRenderer, ThumbnailType};
use crate::render::{MediaView, RenderOptions, human_size};
use crate::text::escape_html;

/// The PDF renderer.
#[derive(Debug, Clone, Copy, Default)]
pub struct Pdf;

impl FileRenderer for Pdf {
    fn name(&self) -> &'static str {
        "pdf"
    }

    /// The browser's reader on the file's own page, the card everywhere else.
    /// `options.reader` is what tells the two apart: an item page or a block
    /// shows one card per file, never a reader per file.
    fn render(&self, media: &MediaView, options: &RenderOptions) -> String {
        let card = card(media);
        let Some(original) = media.original_url.as_deref() else {
            return format!("<div class=\"pdf-reader\">{card}</div>");
        };
        if !options.reader {
            return format!("<div class=\"pdf-reader\">{card}</div>");
        }
        let title = escape_html(&media.display_title);
        let original = escape_html(original);
        format!(
            "<div class=\"pdf-reader pdf-reader-page\">\
             <object class=\"pdf-object\" data=\"{original}\" type=\"application/pdf\" title=\"{title}\">\
             {card}\
             </object>{card}</div>"
        )
    }
}

/// The card: the first page when a derivative exists, the title, the kind and
/// the size, all one link to the original. A file with no original is not a
/// link, so a caller never gets a dead one.
fn card(media: &MediaView) -> String {
    let title = escape_html(&media.display_title);
    let mut body = format!(
        "<span class=\"pdf-card-title\">{title}</span>\
         <span class=\"pdf-card-facts\">{}</span>",
        facts(media)
    );
    if let Some(page) = media.stored_thumbnail_url(ThumbnailType::Large) {
        body.push_str(&format!(
            "<img class=\"pdf-card-page\" src=\"{}\" alt=\"\" loading=\"lazy\">",
            escape_html(page)
        ));
    }
    match media.original_url.as_deref() {
        Some(original) => format!(
            "<a class=\"pdf-card\" href=\"{}\">{body}</a>",
            escape_html(original)
        ),
        None => format!("<span class=\"pdf-card\">{body}</span>"),
    }
}

/// The kind and the size the card states before the reader follows it:
/// `PDF · 2.3 MB`, or `PDF` when the row recorded no size.
fn facts(media: &MediaView) -> String {
    match media.media.size.filter(|size| *size > 0) {
        Some(size) => format!("PDF · {}", human_size(size.unsigned_abs())),
        None => "PDF".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{media_row, media_view_from};

    fn pdf_media() -> MediaView {
        let mut media = media_row(7, "upload", "file");
        media.storage_id = Some("abc123".into());
        media.extension = Some("pdf".into());
        media.media_type = Some("application/pdf".into());
        media.has_original = true;
        media.has_thumbnails = true;
        media.size = Some(2_411_724);
        media.core = media.core.clone().with_title("Field notes");
        media.source = Some("field-notes.pdf".into());
        media_view_from(&media)
    }

    /// The file's own page draws the browser's reader at the original, with the
    /// card as its fallback and a second card for the phone rule. Both carry the
    /// first page, the title, and the kind and size.
    #[test]
    fn the_file_page_draws_the_reader_and_the_card() {
        let html = Pdf.render(
            &pdf_media(),
            &RenderOptions {
                reader: true,
                ..RenderOptions::default()
            },
        );
        assert_eq!(
            html,
            "<div class=\"pdf-reader pdf-reader-page\">\
             <object class=\"pdf-object\" data=\"/files/original/abc123.pdf\" \
             type=\"application/pdf\" title=\"Field notes\">\
             <a class=\"pdf-card\" href=\"/files/original/abc123.pdf\">\
             <span class=\"pdf-card-title\">Field notes</span>\
             <span class=\"pdf-card-facts\">PDF · 2.3 MB</span>\
             <img class=\"pdf-card-page\" src=\"/files/large/abc123.jpg\" alt=\"\" loading=\"lazy\"></a>\
             </object>\
             <a class=\"pdf-card\" href=\"/files/original/abc123.pdf\">\
             <span class=\"pdf-card-title\">Field notes</span>\
             <span class=\"pdf-card-facts\">PDF · 2.3 MB</span>\
             <img class=\"pdf-card-page\" src=\"/files/large/abc123.jpg\" alt=\"\" loading=\"lazy\"></a></div>"
        );
    }

    /// Every other surface shows the card only, so a page of files never loads
    /// one document reader per file.
    #[test]
    fn another_surface_draws_the_card_only() {
        let html = Pdf.render(&pdf_media(), &RenderOptions::default());
        assert!(!html.contains("<object"), "{html}");
        assert!(
            html.starts_with("<div class=\"pdf-reader\"><a class=\"pdf-card\""),
            "{html}"
        );
    }

    /// A PDF whose first page could not be made shows no picture at all rather
    /// than the theme's placeholder drawn as if it were the page.
    #[test]
    fn a_pdf_without_a_first_page_shows_no_picture() {
        let mut media = pdf_media();
        media.media.has_thumbnails = false;
        let html = Pdf.render(&media, &RenderOptions::default());
        assert!(!html.contains("<img"), "{html}");
        assert!(html.contains("pdf-card-title"), "{html}");
    }

    /// A row with no recorded size states the kind alone, and no original means
    /// the card is not a link.
    #[test]
    fn a_pdf_with_no_size_or_original_still_names_itself() {
        let mut media = pdf_media();
        media.media.size = None;
        media.media.has_original = false;
        media.original_url = None;
        let html = Pdf.render(&media, &RenderOptions::default());
        assert_eq!(
            html,
            "<div class=\"pdf-reader\"><span class=\"pdf-card\">\
             <span class=\"pdf-card-title\">Field notes</span>\
             <span class=\"pdf-card-facts\">PDF</span>\
             <img class=\"pdf-card-page\" src=\"/files/large/abc123.jpg\" alt=\"\" loading=\"lazy\"></span></div>"
        );
    }
}
