//! The transcript section of a media page: one caption language at a time,
//! every cue a `?t=` link, and a caption band the page draws for a recording
//! whose player draws none.
//!
//! Upstream has no transcript; a media page shows the `<track>` elements and
//! stops (`view/omeka/site/media/show.phtml`). This is a Romeka addition, so it
//! lives behind the same `mediaRender` block as the file it transcribes.
//!
//! Every cue of the language is in the page. Find-in-page therefore searches
//! the words a visitor heard, and a reader with no script follows a cue by
//! reloading its `?t=` link. The theme's `annotations.js` wires the section, so
//! the page has one seek and one mark of the playing line for its notes and its
//! transcript alike.

use crate::html::escape_html;
use crate::i18n::{Messages, sprintf, sprintf1};

/// One caption track of one language.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Language {
    /// The track's BCP 47 tag, `None` for a row the importer could not tag.
    pub lang: Option<String>,
    /// The language's own name, or the site's word for captions.
    pub label: String,
    /// How many cues the language holds.
    pub cues: u32,
}

/// One line of the transcript, in the row's own terms.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cue {
    /// The row id, so the line has a stable `#cue-` anchor.
    pub id: i64,
    /// Where the phrase starts, in milliseconds.
    pub start_ms: u64,
    /// Where the phrase ends, in milliseconds.
    pub end_ms: u64,
    /// The phrase, as the row stores it. Plain text.
    pub text: String,
}

/// A recording's transcript: every language it offers and the cues of the one
/// on the page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transcript {
    /// The languages with cues, the default first.
    pub languages: Vec<Language>,
    /// The language the page shows, `None` for an untagged track.
    pub current: Option<String>,
    /// Whether the player draws no captions, so the page draws them instead.
    /// An `<audio>` element has no display area, so its captions are read in
    /// the transcript and in the band.
    pub audio: bool,
    /// Every cue of [`Self::current`], in time order.
    pub cues: Vec<Cue>,
}

impl Transcript {
    /// Whether `language` is the one the page shows.
    #[must_use]
    pub fn is_current(&self, language: &Language) -> bool {
        language.lang == self.current
    }

    /// The shown language's own name, or the first language's when no listed
    /// one matches.
    #[must_use]
    fn current_label(&self) -> &str {
        self.languages
            .iter()
            .find(|language| self.is_current(language))
            .or_else(|| self.languages.first())
            .map_or("", |language| language.label.as_str())
    }
}

/// The tag a `?transcript=` value names, when it names one the recording
/// offers. A tag matches case-insensitively, as BCP 47 does.
#[must_use]
pub fn requested_language<'a>(
    languages: &'a [Language],
    requested: Option<&str>,
) -> Option<&'a str> {
    let requested = requested?;
    if !plausible_tag(requested) {
        return None;
    }
    languages.iter().find_map(|language| {
        let tag = language.lang.as_deref()?;
        tag.eq_ignore_ascii_case(requested).then_some(tag)
    })
}

/// A tag a track could carry: short, letters, digits and hyphens. A query
/// string is a boundary, so a value that is not a tag asks nothing.
fn plausible_tag(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 35
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-')
}

/// The text of one stored annotation body: its `TextualBody` values, joined by
/// a newline. A body the site cannot read as text contributes nothing, so a
/// malformed row costs its line and not the page.
#[must_use]
pub fn cue_text(body_json: &str) -> String {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(body_json) else {
        return String::new();
    };
    let bodies: Vec<&serde_json::Value> = match &value {
        serde_json::Value::Array(items) => items.iter().collect(),
        serde_json::Value::Object(_) => vec![&value],
        _ => return String::new(),
    };
    bodies
        .iter()
        .filter(|body| body.get("type").and_then(serde_json::Value::as_str) == Some("TextualBody"))
        .filter_map(|body| body.get("value").and_then(serde_json::Value::as_str))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The transcript section: the facts line, the language links, the "Follow
/// along" toggle the script reveals, and every cue as a `?t=` link.
#[must_use]
pub fn section(transcript: &Transcript, path: &str, messages: &Messages) -> String {
    let mut out = String::from("<section class=\"transcript\" id=\"transcript\" data-transcript>");
    out.push_str(&facts(transcript, messages));
    out.push_str("<details class=\"transcript-details\" open>");
    out.push_str(&format!(
        "<summary class=\"transcript-heading\">{}</summary>",
        escape_html(&heading(transcript, messages))
    ));
    out.push_str(&languages(transcript, path, messages));
    out.push_str(&format!(
        "<label class=\"transcript-follow\" data-transcript-follow hidden>\
         <input type=\"checkbox\" checked> {}</label>",
        escape_html(messages.transcript_follow)
    ));
    out.push_str(&format!(
        "<ol class=\"transcript-lines\" tabindex=\"0\" aria-label=\"{}\">",
        escape_html(messages.transcript)
    ));
    for cue in &transcript.cues {
        out.push_str(&line(cue, &transcript.current, path));
    }
    out.push_str("</ol></details>");
    out.push_str("</section>");
    out
}

/// The caption band, which goes under the player rather than in the transcript
/// section: an `<audio>` element has no display area, so the page draws the
/// playing line where the frame would be. Empty for a video, whose element
/// draws its own captions.
#[must_use]
pub fn band(transcript: &Transcript) -> &'static str {
    if transcript.audio {
        "<p class=\"transcript-band\" data-transcript-band hidden aria-hidden=\"true\"></p>"
    } else {
        ""
    }
}

/// "Captions: English, Français. Transcript below." Under the player, before
/// the list, so a visitor learns that captions exist and where to read them.
fn facts(transcript: &Transcript, messages: &Messages) -> String {
    if transcript.audio {
        return format!(
            "<p class=\"transcript-facts\">{}</p>",
            escape_html(messages.transcript_in_transcript)
        );
    }
    let names = transcript
        .languages
        .iter()
        .map(|language| language.label.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "<p class=\"transcript-facts\">{} {}</p>",
        escape_html(&sprintf1(messages.transcript_captions, &names)),
        escape_html(messages.transcript_below)
    )
}

/// `Transcript · English · 214 lines`.
fn heading(transcript: &Transcript, messages: &Messages) -> String {
    let count = transcript.cues.len().to_string();
    let lines = if transcript.cues.len() == 1 {
        sprintf1(messages.transcript_line, &count)
    } else {
        sprintf1(messages.transcript_lines, &count)
    };
    sprintf(
        messages.transcript_heading,
        &[transcript.current_label(), &lines],
    )
}

/// The languages with cues, each a link, the shown one marked `aria-current`.
/// One language is no choice, so it renders no switcher.
fn languages(transcript: &Transcript, path: &str, messages: &Messages) -> String {
    if transcript.languages.len() < 2 {
        return String::new();
    }
    let mut out = format!(
        "<nav class=\"transcript-languages\" aria-label=\"{}\">",
        escape_html(messages.transcript_languages)
    );
    for (index, language) in transcript.languages.iter().enumerate() {
        if index > 0 {
            out.push_str("<span class=\"transcript-separator\" aria-hidden=\"true\"> · </span>");
        }
        let href = match &language.lang {
            Some(lang) => format!("{path}?transcript={lang}#transcript"),
            None => format!("{path}#transcript"),
        };
        out.push_str(&format!(
            "<a class=\"transcript-language\" href=\"{}\"",
            escape_html(&href)
        ));
        // The label is the language's own name, so it is announced in that
        // language and names the tag a client asks for.
        if let Some(lang) = &language.lang {
            out.push_str(&format!(
                " hreflang=\"{}\" lang=\"{}\"",
                escape_html(lang),
                escape_html(lang)
            ));
        }
        if transcript.is_current(language) {
            out.push_str(" aria-current=\"true\"");
        }
        out.push_str(&format!(
            ">{} ({})</a>",
            escape_html(&language.label),
            language.cues
        ));
    }
    out.push_str("</nav>");
    out
}

/// One cue: its time and its words, the whole line a link to that moment.
fn line(cue: &Cue, language: &Option<String>, path: &str) -> String {
    let mut query = format!("t={}", seconds(cue.start_ms));
    if let Some(lang) = language {
        query.push_str(&format!("&transcript={lang}"));
    }
    format!(
        "<li class=\"transcript-line\" id=\"cue-{id}\" data-start-ms=\"{start}\" \
         data-end-ms=\"{end}\"><a class=\"transcript-cue\" href=\"{href}\">\
         <span class=\"transcript-time\">{time}</span>\
         <span class=\"transcript-text\">{text}</span></a></li>",
        id = cue.id,
        start = cue.start_ms,
        end = cue.end_ms,
        href = escape_html(&format!("{path}?{query}#player")),
        time = escape_html(&crate::annotation::clock(cue.start_ms)),
        text = escape_html(&cue.text),
    )
}

/// The seconds a `?t=` link states: whole seconds when the cue begins on one,
/// else up to milliseconds. The same spelling the player's `#t=` fragment
/// takes, so a link and the fragment it produces name one moment.
fn seconds(milliseconds: u64) -> String {
    let whole = milliseconds / 1000;
    let fraction = milliseconds % 1000;
    if fraction == 0 {
        whole.to_string()
    } else {
        format!("{whole}.{fraction:03}")
            .trim_end_matches('0')
            .to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::EN;

    fn language(lang: Option<&str>, label: &str, cues: u32) -> Language {
        Language {
            lang: lang.map(str::to_string),
            label: label.to_string(),
            cues,
        }
    }

    fn transcript() -> Transcript {
        Transcript {
            languages: vec![
                language(Some("en"), "English", 214),
                language(Some("fr"), "Français", 2),
            ],
            current: Some("en".to_string()),
            audio: false,
            cues: vec![
                Cue {
                    id: 11,
                    start_ms: 0,
                    end_ms: 400,
                    text: "A red square.".to_string(),
                },
                Cue {
                    id: 12,
                    start_ms: 62_000,
                    end_ms: 70_000,
                    text: "A blue bar <and> a bold claim.".to_string(),
                },
            ],
        }
    }

    #[test]
    fn every_cue_is_a_link_to_its_moment() {
        let html = section(&transcript(), "/s/museum/media/7", &EN);
        assert!(
            html.contains(
                "<li class=\"transcript-line\" id=\"cue-12\" data-start-ms=\"62000\" data-end-ms=\"70000\">"
            ),
            "{html}"
        );
        assert!(
            html.contains("href=\"/s/museum/media/7?t=62&amp;transcript=en#player\""),
            "{html}"
        );
        assert!(
            html.contains("<span class=\"transcript-time\">1:02</span>"),
            "{html}"
        );
        // A cue's words are text, never markup.
        assert!(
            html.contains("A blue bar &lt;and&gt; a bold claim."),
            "{html}"
        );
        assert!(!html.contains("<and>"), "{html}");
    }

    #[test]
    fn the_heading_names_the_language_and_the_line_count() {
        let html = section(&transcript(), "/s/museum/media/7", &EN);
        assert!(
            html.contains(
                "<summary class=\"transcript-heading\">Transcript · English · 2 lines</summary>"
            ),
            "{html}"
        );
    }

    #[test]
    fn one_cue_reads_as_one_line() {
        let mut one = transcript();
        one.cues.truncate(1);
        let html = section(&one, "/s/museum/media/7", &EN);
        assert!(
            html.contains(
                "<summary class=\"transcript-heading\">Transcript · English · 1 line</summary>"
            ),
            "{html}"
        );
    }

    #[test]
    fn the_facts_line_names_every_language_and_points_at_the_list() {
        let html = section(&transcript(), "/s/museum/media/7", &EN);
        assert!(
            html.contains(
                "<p class=\"transcript-facts\">Captions: English, Français. Transcript below.</p>"
            ),
            "{html}"
        );
    }

    #[test]
    fn the_language_links_mark_the_one_showing() {
        let html = section(&transcript(), "/s/museum/media/7", &EN);
        assert!(
            html.contains(
                "<a class=\"transcript-language\" href=\"/s/museum/media/7?transcript=en#transcript\" \
                 hreflang=\"en\" lang=\"en\" aria-current=\"true\">English (214)</a>"
            ),
            "{html}"
        );
        assert!(
            html.contains(
                "<a class=\"transcript-language\" href=\"/s/museum/media/7?transcript=fr#transcript\" \
                 hreflang=\"fr\" lang=\"fr\">Français (2)</a>"
            ),
            "{html}"
        );
    }

    #[test]
    fn one_language_renders_no_switcher() {
        let mut one = transcript();
        one.languages.truncate(1);
        let html = section(&one, "/s/museum/media/7", &EN);
        assert!(!html.contains("transcript-languages"), "{html}");
    }

    #[test]
    fn a_checked_follow_toggle_is_in_the_page_for_the_script_to_reveal() {
        let html = section(&transcript(), "/s/museum/media/7", &EN);
        assert!(
            html.contains(
                "<label class=\"transcript-follow\" data-transcript-follow hidden>\
                 <input type=\"checkbox\" checked> Follow along</label>"
            ),
            "{html}"
        );
    }

    #[test]
    fn audio_gets_a_band_and_says_its_captions_are_in_the_list() {
        let mut audio = transcript();
        audio.audio = true;
        assert_eq!(
            band(&audio),
            "<p class=\"transcript-band\" data-transcript-band hidden aria-hidden=\"true\"></p>"
        );
        let html = section(&audio, "/s/museum/media/7", &EN);
        assert!(
            html.contains(
                "<p class=\"transcript-facts\">Captions show in the transcript below.</p>"
            ),
            "{html}"
        );
        assert!(!html.contains("Transcript below.</p>"), "{html}");
        assert!(!html.contains("transcript-band\""), "{html}");
    }

    #[test]
    fn a_video_gets_no_band() {
        assert_eq!(band(&transcript()), "");
        let html = section(&transcript(), "/s/museum/media/7", &EN);
        assert!(!html.contains("transcript-band"), "{html}");
    }

    #[test]
    fn an_untagged_track_is_labelled_and_asked_for_by_the_bare_path() {
        let mut untagged = transcript();
        untagged.languages = vec![
            language(None, "Captions", 3),
            language(Some("en"), "English", 2),
        ];
        untagged.current = None;
        let html = section(&untagged, "/s/museum/media/7", &EN);
        assert!(
            html.contains(
                "<summary class=\"transcript-heading\">Transcript · Captions · 2 lines</summary>"
            ),
            "{html}"
        );
        assert!(
            html.contains(
                "<a class=\"transcript-language\" href=\"/s/museum/media/7#transcript\" \
                 aria-current=\"true\">Captions (3)</a>"
            ),
            "{html}"
        );
        assert!(html.contains("?t=62#player"), "{html}");
    }

    #[test]
    fn a_cue_time_is_a_position_and_never_rounds_up() {
        let mut early = transcript();
        early.cues[0].start_ms = 200;
        let html = section(&early, "/s/museum/media/7", &EN);
        assert!(
            html.contains("<span class=\"transcript-time\">0:00</span>"),
            "{html}"
        );
        assert!(
            !html.contains("<span class=\"transcript-time\">0:01</span>"),
            "{html}"
        );
        assert!(
            html.contains("<span class=\"transcript-time\">1:02</span>"),
            "{html}"
        );
    }

    #[test]
    fn the_link_states_the_same_seconds_a_media_fragment_takes() {
        assert_eq!(seconds(0), "0");
        assert_eq!(seconds(62_000), "62");
        assert_eq!(seconds(1_500), "1.5");
        assert_eq!(seconds(12_340), "12.34");
        assert_eq!(seconds(3_600_001), "3600.001");
    }

    #[test]
    fn a_requested_language_wins_only_when_the_recording_offers_it() {
        let languages = transcript().languages;
        assert_eq!(requested_language(&languages, Some("fr")), Some("fr"));
        assert_eq!(requested_language(&languages, Some("FR")), Some("fr"));
        assert_eq!(requested_language(&languages, Some("de")), None);
        assert_eq!(requested_language(&languages, Some("<script>")), None);
        assert_eq!(requested_language(&languages, None), None);
    }

    #[test]
    fn the_cue_text_comes_from_the_stored_textual_bodies() {
        let body = r#"[{"type":"TextualBody","value":"A red square.","format":"text/plain"}]"#;
        assert_eq!(cue_text(body), "A red square.");
        let two = r#"[{"type":"TextualBody","value":"One.","format":"text/plain"},{"type":"TextualBody","value":"Two.","format":"text/plain"}]"#;
        assert_eq!(cue_text(two), "One.\nTwo.");
        assert_eq!(cue_text("not json"), "");
        assert_eq!(
            cue_text(r#"{"type":"Link","id":"https://example.org/a"}"#),
            ""
        );
    }
}
