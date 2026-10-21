//! The "Use elsewhere" block on an item or a media page: the IIIF manifest, the
//! drag-and-drop link a viewer accepts, a copy button, the captions file, the
//! notes as a WAP container and a citable link.
//!
//! Upstream's item page links the manifest and stops there. This block is a
//! Romeka addition (`docs/compat.md`), and it is pure: the loader resolves the
//! URLs, [`head`] and [`block`] place them. The one script it loads,
//! [`SCRIPT`], copies the manifest URL; with no script the same URL sits in a
//! read-only input a reader can select.

use crate::html::escape_html;
use crate::i18n::{Messages, sprintf1};

/// The IIIF logo, after `iiif.io`'s mark: three bars and a red last stroke.
/// Decorative, because the link text names the manifest.
pub const IIIF_LOGO: &str = "<svg class=\"iiif-logo\" viewBox=\"0 0 24 21\" width=\"18\" height=\"16\" \
     aria-hidden=\"true\" focusable=\"false\"><path fill=\"#2873AB\" d=\"M0 4h3v17H0zM5 4h3v17H5zM10 4h3v17h-3z\"/>\
     <path fill=\"#ED1D33\" d=\"M15 4h9v3h-6v4h5v3h-5v7h-3z\"/>\
     <path fill=\"#2873AB\" d=\"M0 0h3v3H0zM5 0h3v3H5zM10 0h3v3h-3z\"/></svg>";

/// The one script the block loads: copies a manifest URL.
pub const SCRIPT: &str = include_str!("share.js");

/// What the page sharing is: its citation names an item or a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShareTarget {
    /// An item page, which cites the item.
    Item,
    /// A media page, which cites the file.
    Media,
}

/// What an item or a media page shares. The loader builds it, so the theme
/// never builds a URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShareView {
    /// Whether the page is an item or a file.
    pub target: ShareTarget,
    /// The manifest the page describes: the item manifest on an item page, the
    /// media manifest on a media page. `None` when the kind has no canvas and
    /// the server has no manifest to offer.
    pub manifest: Option<String>,
    /// The IIIF viewer URL for `manifest`, present only when the kind has a
    /// canvas.
    pub viewer: Option<String>,
    /// The page's own title, for `og:title`.
    pub title: String,
    /// The page's picture, for `og:image`, when it has one.
    pub image: Option<String>,
    /// The canonical page URL a citation names.
    pub cite: String,
    /// The captions files a recording offers, one per language.
    pub captions: Vec<ShareLink>,
    /// The WAP container the media's notes are read from, present when the
    /// media has notes a reader may see.
    pub notes: Option<String>,
}

impl ShareView {
    /// A share view with nothing but the page's own facts.
    #[must_use]
    pub fn page(
        target: ShareTarget,
        title: impl Into<String>,
        cite: impl Into<String>,
        image: Option<String>,
        manifest: Option<String>,
        viewer: Option<String>,
    ) -> Self {
        Self {
            target,
            manifest,
            viewer,
            title: title.into(),
            image,
            cite: cite.into(),
            captions: Vec::new(),
            notes: None,
        }
    }
}

/// One file the page shares under its own name, such as a language's captions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShareLink {
    /// The link's text, already in the site's locale.
    pub label: String,
    /// The absolute URL.
    pub url: String,
}

/// The `rel=alternate` link to the manifest and the two `og:` members a link
/// preview reads. Empty members are omitted rather than written empty.
#[must_use]
pub fn head(share: &ShareView) -> String {
    let mut out = format!(
        "<meta property=\"og:title\" content=\"{}\">\n",
        escape_html(&share.title)
    );
    if let Some(image) = &share.image {
        out.push_str(&format!(
            "<meta property=\"og:image\" content=\"{}\">\n",
            escape_html(image)
        ));
    }
    if let Some(manifest) = &share.manifest {
        out.push_str(&format!(
            "<link rel=\"alternate\" type=\"application/ld+json;profile=\
             &quot;http://iiif.io/api/presentation/3/context.json&quot;\" href=\"{}\">\n",
            escape_html(manifest)
        ));
    }
    out
}

/// The "Use elsewhere" section, and the script that makes its Copy button
/// work. Every URL and label is placed here; the script only reads data
/// attributes, so no text crosses into JavaScript.
#[must_use]
pub fn block(share: &ShareView, messages: &Messages, asset_base: &str) -> String {
    let mut out = format!(
        "<section class=\"use-elsewhere\" aria-labelledby=\"use-elsewhere-title\">\
         <h2 id=\"use-elsewhere-title\">{}</h2>",
        escape_html(messages.use_elsewhere)
    );
    if share.manifest.is_some() {
        out.push_str(&format!(
            "<p class=\"use-elsewhere-hint\">{}</p>",
            escape_html(messages.use_elsewhere_hint)
        ));
    }
    out.push_str("<ul class=\"use-elsewhere-list\">");
    if let (Some(viewer), Some(manifest)) = (&share.viewer, &share.manifest) {
        out.push_str(&format!(
            "<li class=\"use-elsewhere-viewer\"><a href=\"{}\">{}</a></li>",
            escape_html(viewer),
            escape_html(messages.iiif_view_in_viewer)
        ));
        out.push_str(&format!(
            "<li class=\"use-elsewhere-manifest\">\
             <a class=\"iiif-manifest\" href=\"{}?manifest={}\" title=\"{}\">{IIIF_LOGO}{}</a> \
             <button type=\"button\" class=\"copy-manifest\" data-copy-url=\"{}\" \
             data-label=\"{}\" data-copied=\"{}\" data-copy-failed=\"{}\">{}</button> \
             <input class=\"manifest-url\" type=\"text\" readonly value=\"{}\" aria-label=\"{}\"> \
             <span class=\"copy-status\" role=\"status\" aria-live=\"polite\" data-copied=\"{}\"></span></li>",
            escape_html(manifest),
            escape_html(manifest),
            escape_html(messages.drag_manifest),
            escape_html(messages.iiif_manifest),
            escape_html(manifest),
            escape_html(messages.copy),
            escape_html(messages.copied),
            escape_html(messages.copy_manual),
            escape_html(messages.copy),
            escape_html(manifest),
            escape_html(messages.manifest_url),
            escape_html(messages.manifest_copied)
        ));
    }
    for caption in &share.captions {
        out.push_str(&format!(
            "<li class=\"use-elsewhere-captions\"><a href=\"{}\">{}</a></li>",
            escape_html(&caption.url),
            escape_html(&caption.label)
        ));
    }
    if let Some(notes) = &share.notes {
        out.push_str(&format!(
            "<li class=\"use-elsewhere-notes\"><a href=\"{}\">{}</a></li>",
            escape_html(notes),
            escape_html(messages.notes_as_data)
        ));
    }
    let cite_label = match share.target {
        ShareTarget::Item => messages.cite_this_item,
        ShareTarget::Media => messages.cite_this_file,
    };
    out.push_str(&format!(
        "<li class=\"use-elsewhere-cite\">{} <a href=\"{}\">{}</a></li>",
        escape_html(cite_label),
        escape_html(&share.cite),
        escape_html(&share.cite)
    ));
    out.push_str("</ul></section>");
    out.push_str(&crate::theme::library_script(asset_base, "js/share.js"));
    out
}

/// A language's captions link, labelled for the language.
#[must_use]
pub fn captions_link(messages: &Messages, language: &str, url: impl Into<String>) -> ShareLink {
    ShareLink {
        label: sprintf1(messages.captions_file, language),
        url: url.into(),
    }
}
