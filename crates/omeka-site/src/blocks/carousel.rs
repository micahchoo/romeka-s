//! A carousel of cards: a list that scrolls sideways and snaps to each card,
//! in a labelled region, with previous and next links. The links work as
//! plain anchors, so the page needs no script; `carousel.js`, which
//! [`with_assets`] adds to a page that holds a carousel, makes them step one
//! screen at a time.

use crate::html::escape_html;
use crate::theme::library_script;
use crate::view::RenderContext;

/// The class that marks a carousel for `carousel.js`.
const MARKER: &str = "class=\"preview-carousel\"";

/// The script that steps every carousel on a page, served as `js/carousel.js`.
pub const SCRIPT: &str = include_str!("../carousel.js");

/// One carousel to draw.
#[derive(Debug, Clone, Copy)]
pub struct Carousel<'a> {
    /// The list's id; each card's id is `<id>-<position>`.
    pub id: &'a str,
    /// The list's classes.
    pub classes: &'a str,
    /// The cards, each an `<li …>` element.
    pub cards: &'a [String],
    /// How many cards one screen shows, which the next link skips.
    pub step: usize,
    /// The region's accessible name.
    pub label: &'a str,
    /// The previous link's text.
    pub prev: &'a str,
    /// The next link's text.
    pub next: &'a str,
}

/// The carousel's markup. Without a script the previous link jumps to the
/// first card and the next link to the first card past one screen.
#[must_use]
pub fn render(carousel: &Carousel<'_>) -> String {
    let Carousel { id, cards, .. } = *carousel;
    let slide = |position: usize| format!("{id}-{}", position.clamp(1, cards.len().max(1)));
    let mut list = format!(
        "<ul class=\"{}\" id=\"{id}\" tabindex=\"0\">",
        carousel.classes
    );
    for (index, card) in cards.iter().enumerate() {
        list.push_str(&card.replacen("<li ", &format!("<li id=\"{}\" ", slide(index + 1)), 1));
    }
    list.push_str("</ul>");
    format!(
        "<div class=\"preview-carousel\" role=\"region\" aria-roledescription=\"carousel\" \
         aria-label=\"{}\">{list}<p class=\"preview-carousel-nav\">\
         <a class=\"preview-carousel-prev\" href=\"#{}\" aria-controls=\"{id}\">{}</a>\
         <a class=\"preview-carousel-next\" href=\"#{}\" aria-controls=\"{id}\">{}</a></p></div>",
        escape_html(carousel.label),
        slide(1),
        escape_html(carousel.prev),
        slide(carousel.step + 1),
        escape_html(carousel.next),
    )
}

/// The page content with `carousel.js` appended when it holds a carousel, and
/// unchanged otherwise.
#[must_use]
pub fn with_assets(content: String, ctx: &RenderContext<'_>) -> String {
    if !content.contains(MARKER) {
        return content;
    }
    let mut out = content;
    out.push_str(&library_script(&ctx.asset_base, "js/carousel.js"));
    out
}
