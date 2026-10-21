//! WebVTT and SRT cue blocks, parsed and rendered, and the dry-run diff a
//! caption replace shows before it commits.
//!
//! Pure: no I/O, no database, no clock. The size and cue limits are enforced
//! here as well as at the route, so a caller that comes straight to [`parse`]
//! still gets a bounded answer on a hostile file.

use std::collections::HashMap;

/// The largest caption file this module parses, in bytes.
pub const MAX_BYTES: usize = 1024 * 1024;

/// The most cues one caption file may hold.
pub const MAX_CUES: usize = 5000;

/// The cue block grammar to read: WebVTT needs its header and skips `NOTE`,
/// `STYLE` and `REGION` blocks; SRT starts with an optional cue number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackFormat {
    Vtt,
    Srt,
}

/// One cue as the import stores it: milliseconds and plain text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedCue {
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: String,
}

/// Why a caption file is a 400: one plain sentence the caller can show.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TrackError {
    #[error("The caption file is larger than 1 MiB.")]
    TooLarge,
    #[error("A caption file may hold at most 5,000 cues.")]
    TooManyCues,
    #[error("The file does not begin with WEBVTT.")]
    NoHeader,
    #[error("Line {line} is not a cue time.")]
    NotACueTime { line: usize },
    #[error("Line {line} ends before it begins.")]
    EmptyRange { line: usize },
}

/// Read every cue of a WebVTT or SRT file, in file order.
///
/// A cue without text is skipped, as the design's grammar says. A time range
/// that is empty or reversed is refused with its line, because the schema
/// stores only `end > start`.
pub fn parse(body: &str, format: TrackFormat) -> Result<Vec<ParsedCue>, TrackError> {
    if body.len() > MAX_BYTES {
        return Err(TrackError::TooLarge);
    }
    let body = body.strip_prefix('\u{feff}').unwrap_or(body);
    let text = body.replace("\r\n", "\n").replace('\r', "\n");
    let lines: Vec<&str> = text.split('\n').collect();
    let mut from = match format {
        TrackFormat::Vtt => {
            let header = lines
                .iter()
                .position(|line| !line.trim().is_empty())
                .ok_or(TrackError::NoHeader)?;
            if !is_vtt_header(lines[header]) {
                return Err(TrackError::NoHeader);
            }
            after_vtt_header(&lines, header + 1)
        }
        TrackFormat::Srt => 0,
    };
    let mut cues = Vec::new();
    while from < lines.len() {
        if lines[from].trim().is_empty() {
            from += 1;
            continue;
        }
        let mut to = from;
        while to < lines.len() && !lines[to].trim().is_empty() {
            to += 1;
        }
        let block = &lines[from..to];
        if let Some(cue) = cue_of(block, from, format)? {
            cues.push(cue);
            if cues.len() > MAX_CUES {
                return Err(TrackError::TooManyCues);
            }
        }
        from = to;
    }
    Ok(cues)
}

/// `WEBVTT` alone, or followed by a space and a description.
fn is_vtt_header(line: &str) -> bool {
    let line = line.trim_end();
    line == "WEBVTT"
        || line
            .strip_prefix("WEBVTT")
            .is_some_and(|rest| rest.starts_with(' ') || rest.starts_with('\t'))
}

/// The first line of the first cue block: skip the header's metadata up to a
/// blank line, but stop early when a file omits the blank line it should have.
fn after_vtt_header(lines: &[&str], mut from: usize) -> usize {
    while from < lines.len() && !lines[from].trim().is_empty() {
        if lines[from].contains("-->") {
            return from;
        }
        from += 1;
    }
    if from < lines.len() { from + 1 } else { from }
}

/// One block of non-blank lines, from its first line number (1-based).
fn cue_of(
    block: &[&str],
    first_line: usize,
    format: TrackFormat,
) -> Result<Option<ParsedCue>, TrackError> {
    let Some(time_at) = block.iter().position(|line| line.contains("-->")) else {
        if format == TrackFormat::Vtt && block.first().is_some_and(|line| is_meta_block(line)) {
            return Ok(None);
        }
        return Err(TrackError::NotACueTime {
            line: first_line + 1,
        });
    };
    let line = first_line + time_at + 1;
    let (start_ms, end_ms) = cue_times(block[time_at]).ok_or(TrackError::NotACueTime { line })?;
    if end_ms <= start_ms {
        return Err(TrackError::EmptyRange { line });
    }
    let text = cue_text(&block[time_at + 1..]);
    Ok((!text.is_empty()).then_some(ParsedCue {
        start_ms,
        end_ms,
        text,
    }))
}

/// `NOTE`, `STYLE` and `REGION` blocks are skipped, with or without a word
/// after the name.
fn is_meta_block(line: &str) -> bool {
    let line = line.trim();
    ["NOTE", "STYLE", "REGION"].iter().any(|name| {
        line == *name
            || line
                .strip_prefix(name)
                .is_some_and(|rest| rest.starts_with(char::is_whitespace))
    })
}

/// The two sides of the arrow, with any cue settings after the end dropped.
fn cue_times(line: &str) -> Option<(u64, u64)> {
    let (start, rest) = line.split_once("-->")?;
    let end = rest.split_whitespace().next()?;
    Some((timestamp(start.trim())?, timestamp(end)?))
}

/// `[hh:]mm:ss.mmm` or `[hh:]mm:ss,mmm`, with 1 to 3 fraction digits.
fn timestamp(text: &str) -> Option<u64> {
    let at = text.find(['.', ','])?;
    let (clock, fraction) = (&text[..at], &text[at + 1..]);
    if !(1..=3).contains(&fraction.len()) || !fraction.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let mut millis = fraction.parse::<u64>().ok()?;
    for _ in fraction.len()..3 {
        millis *= 10;
    }
    let parts: Vec<&str> = clock.split(':').collect();
    let (hours, minutes, seconds) = match parts.as_slice() {
        [hours, minutes, seconds] => (number(hours)?, number(minutes)?, number(seconds)?),
        [minutes, seconds] => (0, number(minutes)?, number(seconds)?),
        _ => return None,
    };
    if minutes > 59 || seconds > 59 {
        return None;
    }
    hours
        .checked_mul(3_600_000)?
        .checked_add(minutes.checked_mul(60_000)?)?
        .checked_add(seconds.checked_mul(1_000)?)?
        .checked_add(millis)
}

/// A clock field of 1 to 6 ASCII digits. The bound keeps `hours` inside `u64`.
fn number(text: &str) -> Option<u64> {
    if text.is_empty() || text.len() > 6 || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

/// The cue's lines, cue tags removed, blank lines kept only where a caller put
/// one inside a line's own text.
fn cue_text(lines: &[&str]) -> String {
    let joined = lines
        .iter()
        .map(|line| strip_tags(line))
        .collect::<Vec<_>>()
        .join("\n");
    joined.trim().to_string()
}

/// The longest angle-bracket run this module reads as a tag. A longer one is
/// literal text, which keeps the scan linear on hostile input.
const MAX_TAG_BYTES: usize = 64;

/// Remove WebVTT's own inline tags (`<b>`, `</b>`, `<c.yellow>`, `<v Name>`,
/// `<lang>`, `<ruby>`, `<rt>`, `<00:00:01.000>`) and leave every other
/// angle-bracket run alone. A `<script>` in a caption is therefore plain text
/// that the page escapes; it is never markup.
fn strip_tags(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(open) = rest.find('<') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        let window = &after[..after.len().min(MAX_TAG_BYTES)];
        let Some(close) = window.find('>') else {
            out.push_str(&rest[open..]);
            return out;
        };
        if is_cue_tag(&after[..close]) {
            rest = &after[close + 1..];
        } else {
            out.push('<');
            rest = after;
        }
    }
    out.push_str(rest);
    out
}

fn is_cue_tag(inner: &str) -> bool {
    if inner.starts_with(|character: char| character.is_ascii_digit()) {
        return inner
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b':' | b'.' | b','));
    }
    let name = inner.strip_prefix('/').unwrap_or(inner);
    let end = name
        .bytes()
        .position(|byte| !byte.is_ascii_alphabetic())
        .unwrap_or(name.len());
    matches!(
        &name[..end],
        "b" | "i" | "u" | "c" | "v" | "lang" | "ruby" | "rt"
    )
}

/// WEBVTT for a set of cues, in time order, dot decimals, milliseconds.
///
/// A blank line inside a cue's text becomes one newline, because a blank line
/// would end the cue block.
#[must_use]
pub fn render_vtt(cues: &[(u64, u64, &str)]) -> String {
    let mut ordered: Vec<&(u64, u64, &str)> = cues.iter().collect();
    ordered.sort_by_key(|(start, end, _)| (*start, *end));
    let mut out = String::from("WEBVTT\n");
    for (start, end, text) in ordered {
        out.push('\n');
        out.push_str(&format_timestamp(*start));
        out.push_str(" --> ");
        out.push_str(&format_timestamp(*end));
        out.push('\n');
        for line in text
            .lines()
            .map(str::trim_end)
            .filter(|line| !line.is_empty())
        {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

fn format_timestamp(millis: u64) -> String {
    let hours = millis / 3_600_000;
    let minutes = (millis / 60_000) % 60;
    let seconds = (millis / 1_000) % 60;
    let fraction = millis % 1_000;
    format!("{hours:02}:{minutes:02}:{seconds:02}.{fraction:03}")
}

/// An existing caption cue as the dry run sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExistingCue<'a> {
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: &'a str,
    /// The row was edited after its import (`revision > 1`).
    pub hand_edited: bool,
}

/// What a caption file would change, for the replace dialog's preview.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CueDiff {
    /// The new file's cue count.
    pub cues: u32,
    /// The new file's first start and last end, 0 when it has no cue.
    pub start_ms: u64,
    pub end_ms: u64,
    /// The existing cues the import would replace.
    pub replaces: u32,
    /// New cues with no existing cue at the same time.
    pub added: u32,
    /// New cues whose time matches an existing cue with different words.
    pub changed: u32,
    /// Existing cues whose time the new file does not have.
    pub removed: u32,
    /// Existing cues the new file repeats word for word.
    pub unchanged: u32,
    /// Hand-edited existing cues the new file repeats, so their words survive.
    pub hand_edited_kept: u32,
    /// Hand-edited existing cues the new file changes or drops, so those
    /// words are lost.
    pub hand_edited_lost: u32,
}

/// Compare the track a media has with the cues a file would store.
///
/// A cue is matched by its exact time range, the one coordinate that outlives
/// a reimport; among cues that share a range, an equal text is paired first so
/// a hand edit that matches the new file counts as kept, not changed.
#[must_use]
pub fn diff(old: &[ExistingCue<'_>], new: &[ParsedCue]) -> CueDiff {
    let mut by_time: HashMap<(u64, u64), Vec<usize>> = HashMap::new();
    for (index, cue) in old.iter().enumerate() {
        by_time
            .entry((cue.start_ms, cue.end_ms))
            .or_default()
            .push(index);
    }
    let mut used = vec![false; old.len()];
    let mut out = CueDiff {
        cues: count(new.len()),
        start_ms: new.iter().map(|cue| cue.start_ms).min().unwrap_or(0),
        end_ms: new.iter().map(|cue| cue.end_ms).max().unwrap_or(0),
        replaces: count(old.len()),
        ..CueDiff::default()
    };
    for cue in new {
        let found = by_time
            .get(&(cue.start_ms, cue.end_ms))
            .and_then(|candidates| {
                candidates
                    .iter()
                    .copied()
                    .find(|index| !used[*index] && old[*index].text == cue.text)
                    .or_else(|| candidates.iter().copied().find(|index| !used[*index]))
            });
        match found {
            Some(index) => {
                used[index] = true;
                if old[index].text == cue.text {
                    out.unchanged += 1;
                    if old[index].hand_edited {
                        out.hand_edited_kept += 1;
                    }
                } else {
                    out.changed += 1;
                    if old[index].hand_edited {
                        out.hand_edited_lost += 1;
                    }
                }
            }
            None => out.added += 1,
        }
    }
    for (index, cue) in old.iter().enumerate() {
        if !used[index] {
            out.removed += 1;
            if cue.hand_edited {
                out.hand_edited_lost += 1;
            }
        }
    }
    out
}

fn count(items: usize) -> u32 {
    u32::try_from(items).unwrap_or(u32::MAX)
}
