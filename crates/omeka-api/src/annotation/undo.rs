//! The caption edition a replace kept, so one import can be undone (G6).
//!
//! An import replaces a language's track rows. The rows it deletes are kept as
//! one document per media and language, beside the media files in the store and
//! under a prefix no media or asset owns, so the container, the IIIF page, the
//! text search, the public page and `captions.vtt` cannot reach it: they read
//! `romeka_media_annotation`, and the kept edition is not in that table.
//!
//! The document names the edition that replaced it by that edition's newest row
//! id. An undo reads the live rows and puts the kept ones back only while that
//! id is still the newest, so it never overwrites an import it did not see.
//!
//! The encoding is a boundary: a document that does not parse, or that belongs
//! to another media or language, is no kept edition rather than an error.

use std::future::Future;
use std::pin::Pin;

use chrono::NaiveDateTime;
use omeka_db::ids::MediaId;
use omeka_db::repos::MediaAnnotationRow;

use super::AnnotationError;

/// The storage path prefix of every kept caption edition. No `StorePrefix`, so
/// the store's file route refuses a path under it.
pub const PREFIX: &str = "caption-undo";

/// The file name a language's kept edition is stored under. `_none` cannot be a
/// language tag, so an untagged edition never shares a name with a tagged one.
const NO_LANGUAGE: &str = "_none";

/// The clock format the database stores and this document carries.
const CLOCK: &str = "%Y-%m-%d %H:%M:%S";

/// One replaced flavour of a language's captions, as it was before the import.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeptEdition {
    pub media: MediaId,
    pub lang: Option<String>,
    /// The newest row id of the edition that replaced this one.
    pub replaced_by: i64,
    /// When this edition was imported: the newest `created` among its cues.
    pub imported: Option<NaiveDateTime>,
    pub cues: Vec<KeptCue>,
}

/// One cue of a kept edition, with the row fields an undo writes back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeptCue {
    pub id: i64,
    pub revision: u32,
    pub created: NaiveDateTime,
    pub modified: NaiveDateTime,
    pub start_ms: i64,
    pub end_ms: i64,
    pub body_json: String,
}

impl KeptEdition {
    /// The edition `rows` held when `replaced_by` replaced it. `None` when a row
    /// carries no time range or no id, which no track row does.
    #[must_use]
    pub fn of(
        media: MediaId,
        lang: Option<&str>,
        replaced_by: i64,
        rows: &[MediaAnnotationRow],
    ) -> Option<Self> {
        let mut cues = Vec::with_capacity(rows.len());
        for row in rows {
            cues.push(KeptCue {
                id: row.id,
                revision: row.revision,
                created: row.created,
                modified: row.modified,
                start_ms: row.t_start_ms?,
                end_ms: row.t_end_ms?,
                body_json: row.body_json.clone(),
            });
        }
        let imported = cues.iter().map(|cue| cue.created).max();
        Some(Self {
            media,
            lang: lang.map(str::to_string),
            replaced_by,
            imported,
            cues,
        })
    }

    /// The language tag a client names, `None` for the untagged edition.
    #[must_use]
    pub fn tag(&self) -> Option<&str> {
        self.lang.as_deref()
    }
}

/// The size and import time of a kept edition, for the language list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeptSummary {
    pub cues: u32,
    pub imported: Option<NaiveDateTime>,
}

/// The answer to an undo: the edition now live again.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackUndo {
    pub lang: Option<String>,
    pub cues: u32,
    pub imported: Option<NaiveDateTime>,
}

/// Where a kept edition lives. One implementation reads the media store; a
/// server without one keeps nothing, and every undo then finds nothing.
pub trait KeptEditions: Send + Sync {
    /// The edition kept for `media` in `lang`, or `None` when there is none, it
    /// does not parse, or it belongs to another media or language.
    fn kept<'a>(
        &'a self,
        media: MediaId,
        lang: Option<&'a str>,
    ) -> Pin<Box<dyn Future<Output = Option<KeptEdition>> + Send + 'a>>;

    /// Keep `edition` as the one an undo brings back, replacing the previous one.
    fn keep<'a>(
        &'a self,
        media: MediaId,
        lang: Option<&'a str>,
        edition: &'a KeptEdition,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>>;

    /// Drop the kept edition, so the next undo finds nothing.
    fn forget<'a>(
        &'a self,
        media: MediaId,
        lang: Option<&'a str>,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>>;
}

/// A service with nowhere to keep an edition: nothing to undo, ever.
#[derive(Debug, Clone, Copy)]
pub struct NoKeptEditions;

static NO_KEPT: NoKeptEditions = NoKeptEditions;

impl KeptEditions for NoKeptEditions {
    fn kept<'a>(
        &'a self,
        _media: MediaId,
        _lang: Option<&'a str>,
    ) -> Pin<Box<dyn Future<Output = Option<KeptEdition>> + Send + 'a>> {
        Box::pin(std::future::ready(None))
    }

    fn keep<'a>(
        &'a self,
        _media: MediaId,
        _lang: Option<&'a str>,
        _edition: &'a KeptEdition,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(std::future::ready(()))
    }

    fn forget<'a>(
        &'a self,
        _media: MediaId,
        _lang: Option<&'a str>,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(std::future::ready(()))
    }
}

/// The kept editions a service built by [`super::Annotations::new`] sees.
#[must_use]
pub fn none() -> &'static dyn KeptEditions {
    &NO_KEPT
}

/// The store path of one media's kept edition in `lang`.
#[must_use]
pub fn storage_path(media: MediaId, lang: Option<&str>) -> String {
    let name = lang.filter(|tag| !tag.is_empty()).unwrap_or(NO_LANGUAGE);
    format!("{PREFIX}/{}/{name}.json", media.0)
}

/// The kept edition as the store holds it.
#[derive(serde::Serialize, serde::Deserialize)]
struct Document {
    media: i64,
    lang: Option<String>,
    replaced_by: i64,
    imported: Option<String>,
    cues: Vec<CueDocument>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct CueDocument {
    id: i64,
    revision: u32,
    created: String,
    modified: String,
    start: i64,
    end: i64,
    body: String,
}

/// The bytes to store for `edition`. Empty when it cannot be written, which
/// costs the undo and never the import that asked for it.
#[must_use]
pub fn encode(edition: &KeptEdition) -> Vec<u8> {
    let document = Document {
        media: i64::try_from(edition.media.0).unwrap_or(i64::MAX),
        lang: edition.lang.clone(),
        replaced_by: edition.replaced_by,
        imported: edition.imported.map(clock),
        cues: edition
            .cues
            .iter()
            .map(|cue| CueDocument {
                id: cue.id,
                revision: cue.revision,
                created: clock(cue.created),
                modified: clock(cue.modified),
                start: cue.start_ms,
                end: cue.end_ms,
                body: cue.body_json.clone(),
            })
            .collect(),
    };
    serde_json::to_vec(&document).unwrap_or_default()
}

/// The edition `bytes` hold for `media` in `lang`, or `None` when the document
/// is not one: a failed parse, another media, another language, or a cue the
/// table would refuse.
#[must_use]
pub fn decode(media: MediaId, lang: Option<&str>, bytes: &[u8]) -> Option<KeptEdition> {
    let document: Document = serde_json::from_slice(bytes).ok()?;
    if u64::try_from(document.media).ok() != Some(media.0) || document.lang.as_deref() != lang {
        return None;
    }
    if document.replaced_by < 0 {
        return None;
    }
    let mut cues = Vec::with_capacity(document.cues.len());
    for cue in document.cues {
        let created = NaiveDateTime::parse_from_str(&cue.created, CLOCK).ok()?;
        let modified = NaiveDateTime::parse_from_str(&cue.modified, CLOCK).ok()?;
        let shaped = cue.id > 0
            && cue.revision >= 1
            && cue.start >= 0
            && cue.end > cue.start
            && !cue.body.is_empty();
        if !shaped {
            return None;
        }
        cues.push(KeptCue {
            id: cue.id,
            revision: cue.revision,
            created,
            modified,
            start_ms: cue.start,
            end_ms: cue.end,
            body_json: cue.body,
        });
    }
    let imported = cues.iter().map(|cue| cue.created).max();
    Some(KeptEdition {
        media,
        lang: document.lang,
        replaced_by: document.replaced_by,
        imported,
        cues,
    })
}

fn clock(at: NaiveDateTime) -> String {
    at.format(CLOCK).to_string()
}

/// The text one stored cue carries, from its canonical body JSON.
pub(crate) fn cue_text(body_json: &str) -> Result<String, AnnotationError> {
    super::wadm::text_of_body(body_json)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    fn cue(id: i64, text: &str) -> KeptCue {
        KeptCue {
            id,
            revision: 2,
            created: NaiveDateTime::parse_from_str("2026-10-20 09:12:44", CLOCK).unwrap(),
            modified: NaiveDateTime::parse_from_str("2026-10-21 07:00:00", CLOCK).unwrap(),
            start_ms: 0,
            end_ms: 400,
            body_json: format!("[{{\"type\":\"TextualBody\",\"value\":\"{text}\"}}]"),
        }
    }

    fn edition(lang: Option<&str>) -> KeptEdition {
        KeptEdition {
            media: MediaId(4127),
            lang: lang.map(str::to_string),
            replaced_by: 90312,
            imported: Some(NaiveDateTime::parse_from_str("2026-10-20 09:12:44", CLOCK).unwrap()),
            cues: vec![cue(90311, "A red square")],
        }
    }

    #[test]
    fn a_stored_edition_reads_back_equal() {
        let kept = edition(Some("en"));
        let bytes = encode(&kept);
        assert_eq!(decode(MediaId(4127), Some("en"), &bytes), Some(kept));
    }

    #[test]
    fn another_media_or_language_is_not_this_edition() {
        let bytes = encode(&edition(Some("en")));
        assert_eq!(decode(MediaId(4128), Some("en"), &bytes), None);
        assert_eq!(decode(MediaId(4127), Some("fr"), &bytes), None);
        assert_eq!(decode(MediaId(4127), None, &bytes), None);
    }

    #[test]
    fn bytes_that_are_not_an_edition_read_as_none() {
        assert_eq!(decode(MediaId(4127), Some("en"), b""), None);
        assert_eq!(decode(MediaId(4127), Some("en"), b"not json"), None);
        let refused = "[{\"id\":1,\"revision\":0,\"created\":\"2026-10-20 09:12:44\",\
                        \"modified\":\"2026-10-20 09:12:44\",\"start\":0,\"end\":0,\"body\":\"[]\"}]";
        let document = format!(
            "{{\"media\":4127,\"lang\":\"en\",\"replaced_by\":9,\"imported\":null,\"cues\":{refused}}}"
        );
        assert_eq!(
            decode(MediaId(4127), Some("en"), document.as_bytes()),
            None,
            "a cue the table would refuse is not a kept edition"
        );
    }

    #[test]
    fn a_language_and_the_untagged_edition_get_different_files() {
        assert_eq!(
            storage_path(MediaId(4127), Some("pt-BR")),
            "caption-undo/4127/pt-BR.json"
        );
        assert_eq!(
            storage_path(MediaId(4127), None),
            "caption-undo/4127/_none.json"
        );
        assert_ne!(
            storage_path(MediaId(4127), None),
            storage_path(MediaId(4127), Some("und"))
        );
    }
}
