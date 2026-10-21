//! The `<track>` elements a media embed plays, and the languages they name.
//!
//! The loader fills one [`TrackLink`] per caption language and inserts the
//! markup after the embed's `<video>` or `<audio>` open tag. The renderer's own
//! string is untouched, so a media with no captions renders byte for byte as
//! before. The language's label is the autonym from [`languages`]; the default
//! track is the site locale's language, else the one with the most cues.

use crate::html::escape_html;
use crate::i18n::languages;

/// One `<track>` of a media embed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackLink {
    pub src: String,
    pub lang: Option<String>,
    /// The language's own name, or the site's word for "Captions" when the
    /// track carries no language.
    pub label: String,
    pub default: bool,
    /// How many cues the language holds, for the page's language links and its
    /// facts line.
    pub cues: u32,
}

/// The tracks of one media, default first, then by cue count, then by tag.
///
/// `counts` is `(language, cues)`; a language with no cues is left out.
/// `captions_base` is `…/captions.vtt` with no query; a language's own tag is
/// appended when it has one. `no_language` is the site's word for captions,
/// used when a row carries no tag.
#[must_use]
pub fn tracks(
    counts: &[(Option<String>, u32)],
    locale: &str,
    captions_base: &str,
    no_language: &str,
) -> Vec<TrackLink> {
    let mut entries: Vec<(Option<String>, u32)> = counts
        .iter()
        .filter(|(_, cues)| *cues > 0)
        .map(|(lang, cues)| (lang.clone().filter(|tag| !tag.is_empty()), *cues))
        .collect();
    if entries.is_empty() {
        return Vec::new();
    }
    entries.sort_by(|left, right| key(&left.0).cmp(&key(&right.0)));
    let chosen = best(&entries, Some(locale))
        .or_else(|| best(&entries, None))
        .unwrap_or(0);
    let mut links: Vec<(TrackLink, u32)> = entries
        .into_iter()
        .enumerate()
        .map(|(index, (lang, cues))| {
            (
                TrackLink {
                    src: match &lang {
                        Some(tag) => format!("{captions_base}?lang={tag}"),
                        None => captions_base.to_string(),
                    },
                    label: match &lang {
                        Some(tag) => languages::autonym(tag).to_string(),
                        None => no_language.to_string(),
                    },
                    default: index == chosen,
                    cues,
                    lang,
                },
                cues,
            )
        })
        .collect();
    links.sort_by(|left, right| {
        right
            .0
            .default
            .cmp(&left.0.default)
            .then_with(|| right.1.cmp(&left.1))
            .then_with(|| key(&left.0.lang).cmp(&key(&right.0.lang)))
    });
    links.into_iter().map(|(link, _)| link).collect()
}

/// The best entry, among those in `locale`'s language when one is named: the
/// most cues, then the named tag before an untagged row, then the smaller tag.
fn best(entries: &[(Option<String>, u32)], locale: Option<&str>) -> Option<usize> {
    let mut best: Option<usize> = None;
    for (index, (lang, cues)) in entries.iter().enumerate() {
        if let Some(locale) = locale
            && !lang
                .as_deref()
                .is_some_and(|tag| same_language(tag, locale))
        {
            continue;
        }
        let wins = match best {
            None => true,
            Some(current) => {
                let (current_lang, current_cues) = &entries[current];
                cues > current_cues || (cues == current_cues && key(lang) < key(current_lang))
            }
        };
        if wins {
            best = Some(index);
        }
    }
    best
}

/// Insert the tracks after the embed's `<video>` or `<audio>` open tag. An
/// embed with no such tag, or no tracks, is returned unchanged.
#[must_use]
pub fn with_tracks(embed: &str, tracks: &[TrackLink]) -> String {
    if tracks.is_empty() {
        return embed.to_string();
    }
    let Some(name_end) = player_tag(embed) else {
        return embed.to_string();
    };
    let Some(close) = embed[name_end..].find('>').map(|at| name_end + at) else {
        return embed.to_string();
    };
    let mut out = String::with_capacity(embed.len() + tracks.len() * 96);
    out.push_str(&embed[..=close]);
    for track in tracks {
        out.push_str(&markup(track));
    }
    out.push_str(&embed[close + 1..]);
    out
}

/// The end of `<video` or `<audio`, when the next character makes it a tag.
fn player_tag(embed: &str) -> Option<usize> {
    let lower = embed.to_ascii_lowercase();
    for name in ["<video", "<audio"] {
        let Some(at) = lower.find(name) else {
            continue;
        };
        let end = at + name.len();
        let boundary = lower[end..]
            .chars()
            .next()
            .is_none_or(|character| character == '>' || character.is_whitespace());
        if boundary {
            return Some(end);
        }
    }
    None
}

fn markup(track: &TrackLink) -> String {
    let mut out = String::from("<track kind=\"captions\"");
    if let Some(lang) = &track.lang {
        out.push_str(&format!(" srclang=\"{}\"", escape_html(lang)));
    }
    out.push_str(&format!(" label=\"{}\"", escape_html(&track.label)));
    out.push_str(&format!(" src=\"{}\"", escape_html(&track.src)));
    if track.default {
        out.push_str(" default");
    }
    out.push('>');
    out
}

/// The ordering key: a named language before an untagged row, then the tag.
fn key(lang: &Option<String>) -> (bool, &str) {
    (lang.is_none(), lang.as_deref().unwrap_or(""))
}

/// A tag's primary subtag, case-insensitively: `pt-BR` is Portuguese.
fn same_language(tag: &str, locale: &str) -> bool {
    subtag(tag).eq_ignore_ascii_case(subtag(locale))
}

fn subtag(tag: &str) -> &str {
    tag.split(['-', '_']).next().unwrap_or(tag)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn link(lang: Option<&str>, label: &str, default: bool, cues: u32) -> TrackLink {
        TrackLink {
            src: match lang {
                Some(tag) => format!("/annotations/media/7/captions.vtt?lang={tag}"),
                None => "/annotations/media/7/captions.vtt".to_string(),
            },
            lang: lang.map(str::to_string),
            label: label.to_string(),
            default,
            cues,
        }
    }

    #[test]
    fn the_site_locale_language_is_the_default_track() {
        let counts = [(Some("en".to_string()), 9), (Some("fr".to_string()), 214)];
        let got = tracks(
            &counts,
            "fr",
            "/annotations/media/7/captions.vtt",
            "Captions",
        );
        assert_eq!(got[0], link(Some("fr"), "Français", true, 214));
        assert_eq!(got[1], link(Some("en"), "English", false, 9));
    }

    #[test]
    fn without_the_locale_the_language_with_the_most_cues_is_the_default() {
        let counts = [(Some("en".to_string()), 9), (Some("fr".to_string()), 214)];
        let got = tracks(
            &counts,
            "de",
            "/annotations/media/7/captions.vtt",
            "Captions",
        );
        assert_eq!(got[0], link(Some("fr"), "Français", true, 214));
        assert_eq!(got[1], link(Some("en"), "English", false, 9));
    }

    #[test]
    fn an_unknown_tag_is_its_own_label() {
        let got = tracks(
            &[(Some("gom".to_string()), 3)],
            "en",
            "/annotations/media/7/captions.vtt",
            "Captions",
        );
        assert_eq!(got[0].label, "gom");
        assert_eq!(got[0].src, "/annotations/media/7/captions.vtt?lang=gom");
    }

    #[test]
    fn a_row_with_no_language_has_a_label_and_no_srclang() {
        let counts = [(None, 3), (Some("en".to_string()), 2)];
        let got = tracks(
            &counts,
            "en",
            "/annotations/media/7/captions.vtt",
            "Captions",
        );
        let none = got
            .iter()
            .find(|track| track.lang.is_none())
            .expect("the untagged track");
        assert_eq!(none.label, "Captions");
        assert_eq!(none.src, "/annotations/media/7/captions.vtt");
        let markup = with_tracks("<video src=\"/f.mp4\" controls></video>", &got);
        assert!(
            markup.contains(
                "<track kind=\"captions\" label=\"Captions\" src=\"/annotations/media/7/captions.vtt\">"
            ),
            "{markup}"
        );
        assert!(!markup.contains("srclang=\"\""));
    }

    #[test]
    fn the_tracks_land_inside_the_video_element() {
        let tracks = [link(Some("en"), "English", true, 1)];
        let html = with_tracks(
            "<video src=\"/v.mp4\" controls><a href=\"/v.mp4\">v</a></video>",
            &tracks,
        );
        assert_eq!(
            html,
            "<video src=\"/v.mp4\" controls><track kind=\"captions\" srclang=\"en\" label=\"English\" src=\"/annotations/media/7/captions.vtt?lang=en\" default><a href=\"/v.mp4\">v</a></video>"
        );
    }

    #[test]
    fn an_audio_element_takes_tracks_and_an_embed_without_a_player_is_unchanged() {
        let tracks = [link(Some("en"), "English", true, 1)];
        assert!(with_tracks("<audio src=\"/a.mp3\" controls></audio>", &tracks).contains("<track"));
        let image = "<img src=\"/i.jpg\">";
        assert_eq!(with_tracks(image, &tracks), image);
        assert_eq!(
            with_tracks("<video src=\"/v.mp4\"></video>", &[]),
            "<video src=\"/v.mp4\"></video>"
        );
    }
}
