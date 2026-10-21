//! The notes a public page shows for one media: numbered markers on the
//! picture, the list under it, and the one script that links them.
//!
//! Everything a reader needs is decided before the page is sent. The server
//! orders the list, computes each marker's position as a percentage of the
//! canvas, and writes the address every entry links to. The SVG carries shapes
//! and no links, and the list is the keyboard and screen-reader path, so a
//! reader who cannot see the picture still reads every note in order. The
//! script only syncs hover, seeks a recording and marks the cited note; with no
//! script the anchors and `:target` do the same work.
//!
//! One note's place is one of four shapes, and the shape decides its words, its
//! marker and its link. A note on a recording is a time range and links to
//! `?t=`, which cues the player with no script (`omeka_site::slug::media_start_url`).

use crate::html::escape_html;
use crate::i18n::{Messages, sprintf, sprintf1};
use crate::slug::media_start_url;
use crate::theme::library_script;

/// The script the annotated figure loads: hover and focus sync, a seek for a
/// time note, and the line for a cited note that is gone.
pub const SCRIPT: &str = include_str!("annotations.js");

/// What a media's notes can point at. A picture takes regions, a recording
/// takes time ranges, and every other kind takes the whole file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Surface {
    Image,
    Time,
    Whole,
}

impl Surface {
    fn name(self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::Time => "time",
            Self::Whole => "whole",
        }
    }
}

/// Where one note points.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Place {
    /// The whole file: an image, a recording, or a document.
    Whole,
    /// Integer pixels in the upright canvas.
    Rect { x: u32, y: u32, w: u32, h: u32 },
    /// A polygon's points, in canvas pixels; its first two points bound it.
    Polygon(Vec<(u32, u32)>),
    /// A range of a recording, in milliseconds.
    Time { start_ms: u64, end_ms: u64 },
}

impl Place {
    /// The box a marker sits on, `None` for a whole-file note. A polygon's box
    /// is the smallest box around its points.
    fn bounds(&self) -> Option<(u32, u32, u32, u32)> {
        match self {
            Self::Whole | Self::Time { .. } => None,
            Self::Rect { x, y, w, h } => Some((*x, *y, *w, *h)),
            Self::Polygon(points) => {
                let (first_x, first_y) = *points.first()?;
                let (mut left, mut top, mut right, mut bottom) =
                    (first_x, first_y, first_x, first_y);
                for (x, y) in points {
                    left = left.min(*x);
                    top = top.min(*y);
                    right = right.max(*x);
                    bottom = bottom.max(*y);
                }
                Some((left, top, right - left, bottom - top))
            }
        }
    }

    /// Whole file first, then regions top to bottom and left to right, then
    /// moments by start and end. The list's numbers are this order's positions.
    fn sort_key(&self) -> (u8, u64, u64) {
        match self {
            Self::Whole => (0, 0, 0),
            Self::Rect { .. } | Self::Polygon(_) => {
                let (x, y, _, _) = self.bounds().unwrap_or((0, 0, 0, 0));
                (1, u64::from(y), u64::from(x))
            }
            Self::Time { start_ms, end_ms } => (2, *start_ms, *end_ms),
        }
    }
}

/// One note as a list entry shows it. The text is plain text from the note's
/// first text body, escaped when it is rendered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteLine {
    pub id: i64,
    pub place: Place,
    pub text: String,
}

/// One media's notes as the page shows them.
///
/// `annotated` renders the entries ordered by place, so the caller need not
/// order them. `total` counts every note of the media, which may be more than
/// `notes` holds: the item page shows at most eight, and the media page pages
/// by a hundred.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotesView {
    pub media_id: u64,
    /// The media's own page, absolute or root-relative. Every entry's "Link"
    /// and the item page's "All N notes" start here.
    pub page_url: String,
    pub surface: Surface,
    pub total: u32,
    pub notes: Vec<NoteLine>,
    /// The canvas in original pixels; `None` draws no overlay over the file.
    pub canvas: Option<(u32, u32)>,
    /// The recording's run time, from the extent table, for the note rail and
    /// the script's "playing now" mark. `None` draws no rail.
    pub duration_ms: Option<u64>,
    /// The address of the next page of notes, when there is one.
    pub next_url: Option<String>,
    /// The media page, when this list is a capped preview (the item page).
    pub more_url: Option<String>,
    /// The note `?note=` cited, so the script can say when it is no longer here.
    pub cited: Option<i64>,
}

/// `embed` unchanged when there are no notes to show. Otherwise the figure, the
/// list and the script, as one block the page prints where the file is.
///
/// The overlay is drawn only when the file is a picture whose canvas size is
/// known and at least one note names a region. A cropped tile never reaches
/// this function: the item page shows its first file whole and lists the rest
/// as chips.
#[must_use]
pub fn annotated(
    embed: &str,
    notes: Option<&NotesView>,
    messages: &Messages,
    asset_base: &str,
) -> String {
    let Some(notes) = notes.filter(|notes| !notes.notes.is_empty()) else {
        return embed.to_string();
    };
    let mut ordered: Vec<&NoteLine> = notes.notes.iter().collect();
    ordered.sort_by_key(|line| line.place.sort_key());
    let overlay = overlay(notes, &ordered);
    let media = notes.media_id;
    let picture = if overlay.is_some() {
        " annotated-image"
    } else {
        ""
    };
    let mut out = format!(
        "<figure class=\"annotated-media{picture}\" data-notes=\"{media}\" data-surface=\"{}\"",
        notes.surface.name()
    );
    if let Some(duration) = notes.duration_ms {
        out.push_str(&format!(" data-duration-ms=\"{duration}\""));
    }
    if let Some(cited) = notes.cited {
        out.push_str(&format!(" data-cited=\"{cited}\""));
    }
    out.push('>');
    if overlay.is_some() {
        out.push_str(&format!(
            "<div class=\"annotated-toggle\"><input type=\"checkbox\" class=\"notes-toggle\" \
             id=\"notes-toggle-{media}\" checked><label for=\"notes-toggle-{media}\">{}</label></div>",
            escape_html(messages.notes_show)
        ));
    }
    out.push_str("<div class=\"annotated-frame\">");
    out.push_str(&player_embed(embed));
    if let Some((shapes, markers)) = overlay {
        out.push_str(&shapes);
        out.push_str(&markers);
    }
    out.push_str("</div>");
    out.push_str(&list(notes, &ordered, messages));
    out.push_str("</figure>");
    out.push_str(&library_script(asset_base, "js/annotations.js"));
    out
}

/// The `<svg>` of shapes and the HTML markers, `None` when nothing can be
/// drawn. Shapes are larger-first so a small region inside a large one stays
/// hittable, but the markers keep the list's numbers.
fn overlay(notes: &NotesView, ordered: &[&NoteLine]) -> Option<(String, String)> {
    let (width, height) = notes.canvas?;
    if notes.surface != Surface::Image || width == 0 || height == 0 {
        return None;
    }
    let mut regions: Vec<Region<'_>> = Vec::new();
    for (index, line) in ordered.iter().enumerate() {
        if let Some(bounds) = line.place.bounds() {
            regions.push(Region {
                number: index + 1,
                line,
                bounds,
            });
        }
    }
    if regions.is_empty() {
        return None;
    }
    let mut drawing = regions.clone();
    drawing.sort_by_key(|region| {
        let (_, _, width, height) = region.bounds;
        u64::from(width) * u64::from(height)
    });
    drawing.reverse();
    let mut shapes = format!(
        "<svg class=\"annotation-overlay\" viewBox=\"0 0 {width} {height}\" \
         preserveAspectRatio=\"xMidYMid meet\" aria-hidden=\"true\" focusable=\"false\">"
    );
    for region in &drawing {
        shapes.push_str(&shape(region.number, region.line));
    }
    shapes.push_str("</svg>");
    let mut markers = String::new();
    for region in &regions {
        let (x, y, _, _) = region.bounds;
        markers.push_str(&marker(region.number, region.line.id, x, y, width, height));
    }
    Some((shapes, markers))
}

/// One region on the canvas: its list number, its note, and its box.
#[derive(Clone, Copy)]
struct Region<'a> {
    number: usize,
    line: &'a NoteLine,
    bounds: (u32, u32, u32, u32),
}

/// One region, drawn twice: a light halo under a dark line, both strokes one
/// pixel wide however the picture scales (`vector-effect`).
fn shape(number: usize, line: &NoteLine) -> String {
    let title = format!("<title>{} {}</title>", number, escape_html(&line.text));
    let (body, halo) = match &line.place {
        Place::Rect { x, y, w, h } => (
            format!(
                "<rect class=\"region-line\" x=\"{x}\" y=\"{y}\" width=\"{w}\" height=\"{h}\"/>"
            ),
            format!(
                "<rect class=\"region-halo\" x=\"{x}\" y=\"{y}\" width=\"{w}\" height=\"{h}\"/>"
            ),
        ),
        Place::Polygon(points) => {
            let points = points
                .iter()
                .map(|(x, y)| format!("{x},{y}"))
                .collect::<Vec<_>>()
                .join(" ");
            (
                format!("<polygon class=\"region-line\" points=\"{points}\"/>"),
                format!("<polygon class=\"region-halo\" points=\"{points}\"/>"),
            )
        }
        Place::Whole | Place::Time { .. } => return String::new(),
    };
    format!(
        "<g id=\"region-{}\" class=\"annotation-region\" data-note=\"{}\">{halo}{body}{title}</g>",
        line.id, line.id
    )
}

/// One numbered marker at the region's top-left corner, as a percentage of the
/// canvas. The marker is hidden from assistive technology and out of the tab
/// order: the list is the path both use. It stays a link so a tap with no
/// script still reaches its entry.
fn marker(number: usize, id: i64, x: u32, y: u32, width: u32, height: u32) -> String {
    format!(
        "<a class=\"note-marker\" href=\"#note-{id}\" data-note=\"{id}\" tabindex=\"-1\" \
         aria-hidden=\"true\" style=\"left:{};top:{}\">{number}</a>",
        percent(x, width),
        percent(y, height)
    )
}

fn percent(value: u32, of: u32) -> String {
    format!("{:.2}%", f64::from(value) / f64::from(of) * 100.0)
}

/// The heading, the facts, the entries and the links that page them.
fn list(notes: &NotesView, ordered: &[&NoteLine], messages: &Messages) -> String {
    let total = u64::from(notes.total);
    let heading = match total {
        0 | 1 => messages.notes_one.to_string(),
        _ => sprintf1(messages.notes_count, &total.to_string()),
    };
    let mut out = format!(
        "<section class=\"annotation-list\" aria-labelledby=\"notes-heading-{}\">\
         <h3 class=\"annotation-heading\" id=\"notes-heading-{}\">{}</h3>",
        notes.media_id,
        notes.media_id,
        escape_html(&heading)
    );
    out.push_str(&facts(notes, ordered, messages));
    out.push_str(&rail(notes, ordered));
    if notes.cited.is_some() {
        out.push_str(&format!(
            "<p class=\"annotation-missing\" hidden>{}</p>",
            escape_html(messages.notes_missing)
        ));
    }
    out.push_str("<ol class=\"annotation-items\">");
    for (index, line) in ordered.iter().enumerate() {
        out.push_str(&entry(index + 1, line, notes, messages));
    }
    out.push_str("</ol>");
    if let Some(next) = &notes.next_url {
        out.push_str(&format!(
            "<a class=\"notes-next\" rel=\"next\" href=\"{}\">{}</a>",
            escape_html(next),
            escape_html(&sprintf1(
                messages.notes_next,
                &notes.notes.len().to_string()
            ))
        ));
    }
    if let Some(more) = &notes.more_url {
        out.push_str(&format!(
            "<a class=\"notes-more\" href=\"{}\">{}</a>",
            escape_html(more),
            escape_html(&sprintf1(messages.notes_all, &notes.total.to_string()))
        ));
    }
    out.push_str("</section>");
    out
}

/// The line under the heading: how many notes and where they point. Only a
/// page that holds every note states the split; a paged list would count its
/// page and call it the whole.
fn facts(notes: &NotesView, ordered: &[&NoteLine], messages: &Messages) -> String {
    if u64::from(notes.total) != ordered.len() as u64 {
        return String::new();
    }
    let regions = ordered
        .iter()
        .filter(|line| matches!(line.place, Place::Rect { .. } | Place::Polygon(_)))
        .count();
    let moments = ordered
        .iter()
        .filter(|line| matches!(line.place, Place::Time { .. }))
        .count();
    let whole = ordered
        .iter()
        .filter(|line| matches!(line.place, Place::Whole))
        .count();
    let pattern = match notes.surface {
        Surface::Image => messages.notes_facts,
        Surface::Time => messages.notes_facts_time,
        Surface::Whole => return String::new(),
    };
    let counted = ordered.len();
    // A kind takes regions or moments, never both; the second count is the one
    // this surface has.
    let placed = if matches!(notes.surface, Surface::Time) {
        moments
    } else {
        regions
    };
    format!(
        "<p class=\"annotation-facts\">{}</p>",
        escape_html(&sprintf(
            pattern,
            &[
                &counted.to_string(),
                &placed.to_string(),
                &whole.to_string(),
            ]
        ))
    )
}

/// The bar over the list with one mark per moment note, at its start as a
/// percentage of the run time, so the density reads at a glance. It is a
/// picture of the list, so it is hidden from assistive technology and out of
/// the tab order like the markers; a mark stays a link for a pointer.
fn rail(notes: &NotesView, ordered: &[&NoteLine]) -> String {
    let Some(duration) = notes.duration_ms.filter(|duration| *duration > 0) else {
        return String::new();
    };
    if notes.surface != Surface::Time {
        return String::new();
    }
    let moments: Vec<&NoteLine> = ordered
        .iter()
        .copied()
        .filter(|line| matches!(line.place, Place::Time { .. }))
        .collect();
    if moments.is_empty() {
        return String::new();
    }
    let mut out = String::from("<div class=\"note-rail\" aria-hidden=\"true\">");
    for line in moments {
        let Place::Time { start_ms, .. } = line.place else {
            continue;
        };
        let percent = start_ms as f64 / duration as f64 * 100.0;
        out.push_str(&format!(
            "<a class=\"note-rail-mark\" href=\"#note-{}\" data-note=\"{}\" tabindex=\"-1\" \
             aria-hidden=\"true\" style=\"left:{percent:.2}%\"></a>",
            line.id, line.id
        ));
    }
    out.push_str("</div>");
    out
}

/// One entry: its number, its place, its text and its citable link.
fn entry(number: usize, line: &NoteLine, notes: &NotesView, messages: &Messages) -> String {
    let mut attrs = format!(
        "id=\"note-{}\" class=\"annotation-item\" data-note=\"{}\"",
        line.id, line.id
    );
    if let Place::Time { start_ms, end_ms } = line.place {
        attrs.push_str(&format!(
            " data-start-ms=\"{start_ms}\" data-end-ms=\"{end_ms}\""
        ));
    }
    format!(
        "<li {attrs}><span class=\"note-number\">{number}</span>{} \
         <span class=\"note-text\">{}</span>{}</li>",
        place(line, notes, messages),
        escape_html(&line.text),
        cite(line, notes, messages)
    )
}

/// The place, as a link where it can be one: "Region" jumps to the shape and a
/// time range cues the player with no script. A whole-file note is plain text.
fn place(line: &NoteLine, notes: &NotesView, messages: &Messages) -> String {
    match &line.place {
        Place::Whole => format!(
            "<span class=\"note-place\">{}</span>",
            escape_html(whole_words(notes.surface, messages))
        ),
        Place::Rect { .. } | Place::Polygon(_) => format!(
            "<a class=\"note-place\" href=\"#region-{}\">{}</a>",
            line.id,
            escape_html(messages.notes_region)
        ),
        Place::Time { start_ms, end_ms } => {
            let range = sprintf(messages.notes_range, &[&clock(*start_ms), &clock(*end_ms)]);
            let label = sprintf(messages.notes_play, &[&clock(*start_ms), &clock(*end_ms)]);
            format!(
                "<a class=\"note-place note-time\" href=\"{}#player\" aria-label=\"{}\">{}</a>",
                escape_html(&media_start_url(&notes.page_url, start_ms / 1000)),
                escape_html(&label),
                escape_html(&range)
            )
        }
    }
}

/// The entry's "Link": the media page, the page of notes that holds it, and
/// either way the entry itself. A time note carries the start moment too.
fn cite(line: &NoteLine, notes: &NotesView, messages: &Messages) -> String {
    let base = format!("{}?note={}#note-{}", notes.page_url, line.id, line.id);
    let url = match line.place {
        Place::Time { start_ms, .. } => media_start_url(&base, start_ms / 1000),
        Place::Whole | Place::Rect { .. } | Place::Polygon(_) => base,
    };
    format!(
        " <a class=\"note-link\" href=\"{}\">{}</a>",
        escape_html(&url),
        escape_html(messages.notes_link)
    )
}

fn whole_words(surface: Surface, messages: &Messages) -> &'static str {
    match surface {
        Surface::Image => messages.notes_whole_image,
        Surface::Time => messages.notes_whole_recording,
        Surface::Whole => messages.notes_whole_file,
    }
}

/// `1:02`, or `1:02:03` past an hour.
fn clock(ms: u64) -> String {
    let seconds = ms / 1000;
    let (hours, minutes, seconds) = (seconds / 3600, seconds / 60 % 60, seconds % 60);
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

/// The embed with `id="player"` on its media element, so `?t=…#player` lands on
/// the player. The page holds one annotated file, so one such id exists.
fn player_embed(embed: &str) -> String {
    for tag in ["<video", "<audio"] {
        if let Some(at) = embed.find(tag)
            && !embed[..at + tag.len()].contains(" id=")
        {
            let mut out = String::with_capacity(embed.len() + 12);
            out.push_str(&embed[..at + tag.len()]);
            out.push_str(" id=\"player\"");
            out.push_str(&embed[at + tag.len()..]);
            return out;
        }
    }
    embed.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::EN;

    fn view(surface: Surface, canvas: Option<(u32, u32)>, notes: Vec<NoteLine>) -> NotesView {
        NotesView {
            media_id: 4127,
            page_url: "/s/archive/media/4127".to_string(),
            surface,
            total: u32::try_from(notes.len()).unwrap_or(0),
            notes,
            canvas,
            duration_ms: None,
            next_url: None,
            more_url: None,
            cited: None,
        }
    }

    fn line(id: i64, place: Place, text: &str) -> NoteLine {
        NoteLine {
            id,
            place,
            text: text.to_string(),
        }
    }

    /// No notes, or no list at all: the renderer's markup comes back as it was,
    /// so a page with nothing to say is byte for byte the page without this
    /// unit.
    #[test]
    fn a_media_with_no_notes_renders_its_embed_unchanged() {
        let embed = "<div class=\"media-render file\"><img src=\"/files/large/a.jpg\"></div>";
        assert_eq!(annotated(embed, None, &EN, "/assets"), embed);
        let empty = view(Surface::Image, Some((4000, 3000)), Vec::new());
        assert_eq!(annotated(embed, Some(&empty), &EN, "/assets"), embed);
    }

    /// A marker's percentage is its canvas pixel over the canvas size, and the
    /// overlay is one SVG whose viewBox is that size.
    #[test]
    fn a_region_marker_sits_on_its_pixels() {
        let notes = view(
            Surface::Image,
            Some((4000, 3000)),
            vec![line(
                90311,
                Place::Rect {
                    x: 1200,
                    y: 340,
                    w: 410,
                    h: 388,
                },
                "Watermark: crowned shield",
            )],
        );
        let html = annotated(
            "<img src=\"/files/large/a.jpg\">",
            Some(&notes),
            &EN,
            "/assets",
        );
        assert!(
            html.contains("viewBox=\"0 0 4000 3000\""),
            "the canvas size is the viewBox: {html}"
        );
        assert!(
            html.contains("style=\"left:30.00%;top:11.33%\""),
            "1200/4000 and 340/3000: {html}"
        );
        assert!(html.contains("href=\"#note-90311\""), "{html}");
        assert!(html.contains("id=\"region-90311\""), "{html}");
        assert!(
            html.contains("class=\"annotation-region\" data-note=\"90311\""),
            "{html}"
        );
    }

    /// The SVG holds no links: the markers are HTML beside it.
    #[test]
    fn the_aria_hidden_overlay_holds_no_links() {
        let notes = view(
            Surface::Image,
            Some((4000, 3000)),
            vec![
                line(1, Place::Whole, "Whole"),
                line(
                    2,
                    Place::Rect {
                        x: 1,
                        y: 2,
                        w: 10,
                        h: 20,
                    },
                    "A region",
                ),
            ],
        );
        let html = annotated(
            "<img src=\"/files/large/a.jpg\">",
            Some(&notes),
            &EN,
            "/assets",
        );
        let svg = html
            .split("<svg")
            .nth(1)
            .and_then(|rest| rest.split("</svg>").next())
            .unwrap_or_default();
        assert!(!svg.contains("<a"), "no anchor inside the SVG: {svg}");
        assert!(svg.contains("aria-hidden=\"true\""), "{svg}");
        assert!(
            html.contains("<a class=\"note-marker\" href=\"#note-2\""),
            "the marker is HTML: {html}"
        );
        assert!(html.contains("tabindex=\"-1\""), "{html}");
    }

    /// A region note with no canvas size draws no overlay, and its place still
    /// reads "Region" in the list.
    #[test]
    fn an_unknown_canvas_draws_no_overlay() {
        let notes = view(
            Surface::Image,
            None,
            vec![line(
                7,
                Place::Rect {
                    x: 1,
                    y: 2,
                    w: 10,
                    h: 20,
                },
                "A region",
            )],
        );
        let html = annotated("<img src=\"/remote/a.jpg\">", Some(&notes), &EN, "/assets");
        assert!(!html.contains("annotation-overlay"), "{html}");
        assert!(!html.contains("note-marker"), "{html}");
        assert!(html.contains(">Region</a>"), "{html}");
    }

    /// Whole-file notes come first, then regions by height and then across, and
    /// the numbers are those positions.
    #[test]
    fn the_list_is_ordered_by_place() {
        let notes = view(
            Surface::Image,
            Some((1000, 1000)),
            vec![
                line(
                    4,
                    Place::Rect {
                        x: 10,
                        y: 500,
                        w: 10,
                        h: 10,
                    },
                    "Lower",
                ),
                line(3, Place::Whole, "Whole"),
                line(
                    2,
                    Place::Rect {
                        x: 700,
                        y: 100,
                        w: 10,
                        h: 10,
                    },
                    "Right",
                ),
                line(
                    1,
                    Place::Rect {
                        x: 100,
                        y: 100,
                        w: 10,
                        h: 10,
                    },
                    "Left",
                ),
            ],
        );
        let html = annotated(
            "<img src=\"/files/large/a.jpg\">",
            Some(&notes),
            &EN,
            "/assets",
        );
        let list = html
            .split("<ol class=\"annotation-items\">")
            .nth(1)
            .unwrap_or_default();
        let order: Vec<usize> = ["Whole", "Left", "Right", "Lower"]
            .iter()
            .map(|text| list.find(text).unwrap_or(usize::MAX))
            .collect();
        assert!(
            order.windows(2).all(|pair| pair[0] < pair[1]),
            "place order {order:?} in {html}"
        );
        assert!(
            html.contains(
                "<span class=\"note-number\">1</span><span class=\"note-place\">Whole image"
            ),
            "{html}"
        );
    }

    /// A time note links to the player at its start with no script, and carries
    /// its range for the script to seek to.
    #[test]
    fn a_time_note_cues_the_player_at_its_start() {
        let mut notes = view(
            Surface::Time,
            None,
            vec![line(
                90311,
                Place::Time {
                    start_ms: 62_000,
                    end_ms: 70_000,
                },
                "The shield",
            )],
        );
        notes.duration_ms = Some(90_000);
        let html = annotated(
            "<video src=\"/files/original/a.mp4\" controls></video>",
            Some(&notes),
            &EN,
            "/assets",
        );
        assert!(
            html.contains("href=\"/s/archive/media/4127?t=62#player\""),
            "{html}"
        );
        assert!(html.contains("data-start-ms=\"62000\""), "{html}");
        assert!(html.contains("data-end-ms=\"70000\""), "{html}");
        assert!(html.contains(">1:02 to 1:10</a>"), "{html}");
        assert!(html.contains("aria-label=\"Play 1:02 to 1:10\""), "{html}");
        assert!(html.contains("id=\"player\""), "{html}");
        assert!(
            html.contains("class=\"note-rail-mark\"") && html.contains("left:68.89%"),
            "a mark at 62 s of 90 s: {html}"
        );
        assert!(
            html.contains("href=\"/s/archive/media/4127?note=90311&amp;t=62#note-90311\""),
            "{html}"
        );
    }

    /// A note's text and the region's title are escaped, so a note that holds
    /// markup is read as text.
    #[test]
    fn note_text_is_escaped() {
        let notes = view(
            Surface::Image,
            Some((100, 100)),
            vec![line(
                1,
                Place::Rect {
                    x: 1,
                    y: 1,
                    w: 10,
                    h: 10,
                },
                "<script>alert(1)</script>",
            )],
        );
        let html = annotated(
            "<img src=\"/files/large/a.jpg\">",
            Some(&notes),
            &EN,
            "/assets",
        );
        assert!(!html.contains("<script>alert"), "{html}");
        assert!(
            html.contains("&lt;script&gt;alert(1)&lt;/script&gt;"),
            "{html}"
        );
    }

    /// The toggle is a real checkbox and only appears when there are shapes to
    /// hide.
    #[test]
    fn the_toggle_appears_only_over_a_picture_with_regions() {
        let picture = view(
            Surface::Image,
            Some((100, 100)),
            vec![line(
                1,
                Place::Rect {
                    x: 1,
                    y: 1,
                    w: 10,
                    h: 10,
                },
                "A region",
            )],
        );
        let html = annotated(
            "<img src=\"/files/large/a.jpg\">",
            Some(&picture),
            &EN,
            "/assets",
        );
        assert!(html.contains("class=\"notes-toggle\""), "{html}");
        assert!(html.contains("Show notes on the picture"), "{html}");
        let document = view(Surface::Whole, None, vec![line(1, Place::Whole, "A note")]);
        let html = annotated(
            "<a href=\"/files/a.pdf\">a.pdf</a>",
            Some(&document),
            &EN,
            "/assets",
        );
        assert!(!html.contains("notes-toggle"), "{html}");
    }

    /// The item page caps its list and links the rest; the media page pages by
    /// a hundred.
    #[test]
    fn a_capped_list_links_the_media_page() {
        let mut notes = view(Surface::Whole, None, vec![line(1, Place::Whole, "A note")]);
        notes.total = 37;
        notes.more_url = Some("/s/archive/media/4127".to_string());
        let html = annotated(
            "<a href=\"/files/a.pdf\">a.pdf</a>",
            Some(&notes),
            &EN,
            "/assets",
        );
        assert!(html.contains(">All 37 notes</a>"), "{html}");
        assert!(html.contains("37 notes"), "{html}");

        let mut paged = view(Surface::Whole, None, vec![line(1, Place::Whole, "A note")]);
        paged.total = 1204;
        paged.next_url = Some("/s/archive/media/4127?notes_after=9".to_string());
        let html = annotated(
            "<a href=\"/files/a.pdf\">a.pdf</a>",
            Some(&paged),
            &EN,
            "/assets",
        );
        assert!(
            html.contains("href=\"/s/archive/media/4127?notes_after=9\""),
            "{html}"
        );
        assert!(html.contains(">Next 1 notes</a>"), "{html}");
    }

    /// A cited note the page no longer holds gets one line above the list; the
    /// script reveals it, so a reader without a script sees the page without.
    #[test]
    fn a_cited_note_carries_the_missing_line() {
        let mut notes = view(Surface::Whole, None, vec![line(1, Place::Whole, "A note")]);
        notes.cited = Some(90311);
        let html = annotated(
            "<a href=\"/files/a.pdf\">a.pdf</a>",
            Some(&notes),
            &EN,
            "/assets",
        );
        assert!(html.contains("data-cited=\"90311\""), "{html}");
        assert!(html.contains("annotation-missing\" hidden"), "{html}");
        assert!(
            html.contains("The note this link points to is no longer here."),
            "{html}"
        );
    }

    /// The page loads the theme's own script, versioned like every other asset.
    #[test]
    fn the_page_loads_the_versioned_script() {
        let notes = view(Surface::Whole, None, vec![line(1, Place::Whole, "A note")]);
        let html = annotated(
            "<a href=\"/files/a.pdf\">a.pdf</a>",
            Some(&notes),
            &EN,
            "/assets",
        );
        assert!(
            html.contains("src=\"/assets/js/annotations.js?v="),
            "{html}"
        );
    }

    /// Minutes and seconds, and an hour only when there is one.
    #[test]
    fn a_clock_names_the_moment() {
        assert_eq!(clock(0), "0:00");
        assert_eq!(clock(62_000), "1:02");
        assert_eq!(clock(3_723_000), "1:02:03");
    }
}
