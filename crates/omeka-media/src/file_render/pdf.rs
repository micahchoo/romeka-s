//! The PDF reader over a stored file. Upstream has none: a PDF falls through to
//! the thumbnail renderer, so the page shows a picture of the first page linked
//! to the file and never the document itself.
//!
//! This renderer draws the browser's own PDF reader on the file's page, with a
//! card as its fallback and a second card for the phone rule. The card is a
//! link to the original with the first-page derivative as its picture, the
//! display title, and the kind and size. The theme's sheet swaps the reader for
//! the sibling card on a phone, because a phone browser draws no scrollable
//! inline PDF.
//!
//! An office document has no PDF of its own. Once its access copy is ready
//! (`crate::access`) the reader shows that copy, and the original stays the
//! card's link and the download: the `.docx` is never what the reader opens.
//! Before the copy is ready the page keeps today's card, which is the thumbnail
//! renderer's.

use crate::file_render::thumbnail::Thumbnail;
use crate::file_render::{FileRenderer, ThumbnailType};
use crate::render::{MediaView, RenderOptions, human_size};
use crate::text::escape_html;

/// The PDF reader renderer.
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
        if !is_pdf(media) && media.access_url.is_none() {
            // An office document with no PDF copy yet: the page it had before
            // this renderer answered for it, a picture linked to the original.
            return Thumbnail.render(media, options);
        }
        let card = card(media);
        let Some(source) = reader_source(media) else {
            return format!("<div class=\"pdf-reader\">{card}</div>");
        };
        if !options.reader {
            return format!("<div class=\"pdf-reader\">{card}</div>");
        }
        let title = escape_html(&media.display_title);
        let source = escape_html(source);
        format!(
            "<div class=\"pdf-reader pdf-reader-page\">\
             <object class=\"pdf-object\" data=\"{source}\" type=\"application/pdf\" title=\"{title}\">\
             {card}\
             </object>{card}</div>"
        )
    }
}

/// Whether the stored original is itself a PDF.
fn is_pdf(media: &MediaView) -> bool {
    media.media.media_type.as_deref() == Some("application/pdf")
        || media
            .media
            .extension
            .as_deref()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("pdf"))
}

/// The PDF a reader opens: the original of a PDF, or the ready access copy of an
/// office document.
fn reader_source(media: &MediaView) -> Option<&str> {
    if is_pdf(media) {
        media.original_url.as_deref()
    } else {
        media.access_url.as_deref()
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
/// `PDF · 2.3 MB`, `DOCX · 12 KB`, or the kind alone when the row recorded no
/// size.
fn facts(media: &MediaView) -> String {
    let kind = if is_pdf(media) {
        "PDF".to_string()
    } else {
        media
            .media
            .extension
            .as_deref()
            .filter(|extension| !extension.is_empty())
            .map(str::to_ascii_uppercase)
            .unwrap_or_else(|| "File".to_string())
    };
    match media.media.size.filter(|size| *size > 0) {
        Some(size) => format!("{kind} · {}", human_size(size.unsigned_abs())),
        None => kind,
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

    fn document_media() -> MediaView {
        let mut media = media_row(8, "upload", "file");
        media.storage_id = Some("def456".into());
        media.extension = Some("docx".into());
        media.media_type =
            Some("application/vnd.openxmlformats-officedocument.wordprocessingml.document".into());
        media.has_original = true;
        media.has_thumbnails = true;
        media.size = Some(12_288);
        media.core = media.core.clone().with_title("Field notes");
        media.source = Some("field-notes.docx".into());
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

    /// A document whose PDF copy is ready opens the reader at the copy, and the
    /// card, its facts and the download stay the original document.
    #[test]
    fn a_document_reader_opens_the_pdf_copy_and_links_the_original() {
        let view = document_media().with_access(Some("/files/access/def456.pdf".to_string()));
        let html = Pdf.render(
            &view,
            &RenderOptions {
                reader: true,
                ..RenderOptions::default()
            },
        );
        assert!(
            html.contains(
                "<object class=\"pdf-object\" data=\"/files/access/def456.pdf\" \
                 type=\"application/pdf\" title=\"Field notes\">"
            ),
            "{html}"
        );
        assert!(!html.contains("data=\"/files/original/"), "{html}");
        assert!(
            html.matches("href=\"/files/original/def456.docx\"").count() == 2,
            "both cards link the original: {html}"
        );
        assert!(html.contains("DOCX · 12 KB"), "{html}");
    }

    /// Before the copy is ready a document keeps the card it had: a picture
    /// linked to the original, never a reader pointed at a `.docx`.
    #[test]
    fn a_document_without_a_copy_keeps_its_thumbnail_card() {
        let html = Pdf.render(&document_media(), &RenderOptions::default());
        assert!(!html.contains("<object"), "{html}");
        assert!(!html.contains("application/pdf"), "{html}");
        assert_eq!(
            html,
            "<a title=\"Field notes\" href=\"/files/original/def456.docx\">\
             <img src=\"/files/large/def456.jpg\"></a>"
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
