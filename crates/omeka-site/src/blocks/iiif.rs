//! What the two IIIF blocks share: the viewer height a curator picks, and the
//! poster the script-free preview shows in place of a viewer.
//!
//! The height is a Romeka setting of an upstream block, so it lives in
//! `o:layout_data.romeka.settings.height`, which upstream ignores and its admin
//! keeps. Unset, each block keeps its upstream height.

use omeka_core::site::SiteBlock;

use crate::blocks::schema::{ChoiceOption, Field, choice, option};
use crate::html::escape_html;
use crate::i18n::sprintf1;
use crate::view::RenderContext;

/// The heights a curator may pick, with the CSS length each renders as.
const HEIGHTS: [(&str, &str); 4] = [
    ("small", "400px"),
    ("medium", "600px"),
    ("large", "800px"),
    ("full", "100vh"),
];

const HEIGHT_OPTIONS: &[ChoiceOption] = &[
    option("", "blockChoice.height_standard"),
    option("small", "blockChoice.height_small"),
    option("medium", "blockChoice.height_medium"),
    option("large", "blockChoice.height_large"),
    option("full", "blockChoice.height_full"),
];

/// The height control both blocks declare; `help` says what "Standard" is for
/// that block.
pub(crate) const fn height_field(help: &'static str) -> Field {
    choice("height", "blockField.iiif.height", help, HEIGHT_OPTIONS)
        .default_text("")
        .romeka()
}

/// The viewer height a block renders at: the curator's choice, or the
/// block's upstream height.
pub(crate) fn height(block: &SiteBlock, upstream: &'static str) -> &'static str {
    let chosen = block
        .layout_data
        .romeka
        .as_ref()
        .and_then(|carrier| carrier.settings.get("height"))
        .and_then(serde_json::Value::as_str);
    HEIGHTS
        .iter()
        .find(|(name, _)| Some(*name) == chosen)
        .map_or(upstream, |(_, css)| css)
}

/// The poster the editor's preview shows in place of a viewer, which would
/// need a script: a card at the viewer's height that names what opens on the
/// public page. `None` outside the preview.
pub(crate) fn preview_poster(name: &str, height: &str, ctx: &RenderContext<'_>) -> Option<String> {
    if !ctx.is_preview() {
        return None;
    }
    let text = sprintf1(ctx.translator().messages().iiif_poster, name);
    Some(format!(
        "<div class=\"iiif-poster\" style=\"height: {height};\"><p>{}</p></div>",
        escape_html(&text)
    ))
}
