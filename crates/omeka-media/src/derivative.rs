//! The still one stored original makes, and the three thumbnails from it.
//!
//! This is the work a derivative worker does for one media, and the same work
//! `romeka media thumbnails` did inline. It is pure over the [`Store`]: an
//! original that a tool must open is streamed out of the store in
//! [`COPY_CHUNK`] pieces into a temporary file, so an object store and a 2 GB
//! video cost the same memory as a local JPEG.
//!
//! The outcome keeps the difference a caller shows: ready (with or without
//! files this run wrote), a typed skip (a tool this install does not have, a
//! type no decoder reads), or a failure with the tool's or decoder's reason.

use std::io::Write;
use std::path::PathBuf;

use crate::file_render::ThumbnailType;
use crate::store::{Store, StoreError, StorePrefix, storage_path};
use crate::thumbnailer::{
    SOURCE_MEDIA_TYPES, ThumbnailOutcome, ThumbnailSkip, generate_thumbnails,
    generate_thumbnails_from_path, thumbnail_path,
};
use crate::tool::{Still, ToolError};

/// How much of an original is copied out of the store at a time on its way to
/// a temporary file. A video's poster reads the size of this buffer, not the
/// size of the video.
pub const COPY_CHUNK: u64 = 8 * 1024 * 1024;

/// The facts of one stored original, which is all the work reads of a media.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewSource {
    pub storage_id: String,
    pub extension: Option<String>,
    pub media_type: Option<String>,
    pub has_original: bool,
}

/// What one media's derivative work did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DerivativeWork {
    /// The three files are in the store, now or from an earlier run.
    Ready {
        /// Whether this run wrote them.
        made: bool,
        tool: Option<&'static str>,
    },
    /// No derivative is made for this media, or the tool is not installed.
    Skipped {
        reason: String,
        tool: Option<&'static str>,
    },
    /// The store, the decoder or the tool refused.
    Failed {
        reason: String,
        tool: Option<&'static str>,
    },
}

impl DerivativeWork {
    /// The reason a caller shows, when there is one.
    #[must_use]
    pub fn reason(&self) -> Option<&str> {
        match self {
            Self::Ready { .. } => None,
            Self::Skipped { reason, .. } | Self::Failed { reason, .. } => Some(reason),
        }
    }

    /// The external command the work used or needed.
    #[must_use]
    pub const fn tool(&self) -> Option<&'static str> {
        match self {
            Self::Ready { tool, .. } | Self::Skipped { tool, .. } | Self::Failed { tool, .. } => {
                *tool
            }
        }
    }
}

/// Make the preview for one stored original.
///
/// `force` removes the three files first, which is the backfill's `--force`.
#[must_use]
pub fn make_preview(store: &dyn Store, source: &PreviewSource, force: bool) -> DerivativeWork {
    let media_type = source.media_type.clone().unwrap_or_default();
    let tool = tool_for(&media_type);
    let needs_tool = Still::of(&media_type).is_some();
    let reads_whole = SOURCE_MEDIA_TYPES.contains(&media_type.as_str());
    if !needs_tool && !reads_whole {
        // The type decides before the store is touched: a .docx has no
        // derivative, and reading it would only cost time.
        return DerivativeWork::Skipped {
            reason: format!("unsupportedType: {media_type}"),
            tool: None,
        };
    }
    if !source.has_original {
        return DerivativeWork::Failed {
            reason: "the row stored no original".to_string(),
            tool,
        };
    }
    if force {
        for thumbnail in ThumbnailType::ALL {
            if let Err(error) = store.delete(&thumbnail_path(&source.storage_id, thumbnail)) {
                return DerivativeWork::Failed {
                    reason: format!("cannot remove an existing thumbnail: {error}"),
                    tool,
                };
            }
        }
    }
    let original_path = storage_path(
        StorePrefix::Original,
        &source.storage_id,
        source.extension.as_deref(),
    );

    // A still's tool needs a file to open, and the original may be 2 GB and
    // live in an object store. Copy it out in chunks instead of holding it.
    let mut streamed = None;
    let original: Vec<u8> = if needs_tool {
        // An extension on the temporary file, because `ffmpeg`'s format probe
        // reads AVIF by it (see `Still::make_from_bytes`).
        let suffix = source
            .extension
            .clone()
            .or_else(|| {
                crate::media_type::extension_for_media_type(&media_type).map(str::to_string)
            })
            .map(|extension| format!(".{extension}"))
            .unwrap_or_default();
        let mut file = match tempfile::Builder::new()
            .prefix("romeka-derivative")
            .suffix(&suffix)
            .tempfile()
        {
            Ok(file) => file,
            Err(error) => {
                return DerivativeWork::Failed {
                    reason: format!("cannot open a temporary file: {error}"),
                    tool,
                };
            }
        };
        if let Err(error) = copy_to(store, &original_path, file.as_file_mut()) {
            return DerivativeWork::Failed {
                reason: format!("cannot read {original_path}: {error}"),
                tool,
            };
        }
        streamed = Some(file);
        Vec::new()
    } else {
        match store.read(&original_path) {
            Ok(bytes) => bytes,
            Err(error) => {
                return DerivativeWork::Failed {
                    reason: format!("cannot read {original_path}: {error}"),
                    tool,
                };
            }
        }
    };

    let outcome = match &streamed {
        Some(file) => {
            generate_thumbnails_from_path(store, &source.storage_id, file.path(), Some(&media_type))
        }
        None => generate_thumbnails(store, &source.storage_id, &original, Some(&media_type)),
    };
    match outcome {
        Ok(ThumbnailOutcome::Generated) => DerivativeWork::Ready { made: true, tool },
        Ok(ThumbnailOutcome::Existing) => DerivativeWork::Ready { made: false, tool },
        Ok(ThumbnailOutcome::Skipped(skip)) => skipped(skip, tool),
        Err(error) => DerivativeWork::Failed {
            reason: format!("cannot write a thumbnail: {error}"),
            tool,
        },
    }
}

/// A typed skip is a skip, not a failure: a tool this install does not have, or
/// a type no decoder reads, is not a broken install. A command that ran and
/// failed is a failure, with the command's reason.
fn skipped(skip: ThumbnailSkip, tool: Option<&'static str>) -> DerivativeWork {
    match skip {
        ThumbnailSkip::UnsupportedType(media_type) => DerivativeWork::Skipped {
            reason: format!("unsupportedType: {media_type}"),
            tool,
        },
        ThumbnailSkip::Still(error) => {
            let name = match &error {
                ToolError::ToolMissing(name)
                | ToolError::Timeout(name, _)
                | ToolError::Failed(name, _) => Some(*name),
            };
            match error {
                ToolError::ToolMissing(name) => DerivativeWork::Skipped {
                    reason: format!("toolMissing: {name}"),
                    tool: Some(name),
                },
                other => DerivativeWork::Failed {
                    reason: other.to_string(),
                    tool: name.or(tool),
                },
            }
        }
        other => DerivativeWork::Failed {
            reason: other.to_string(),
            tool,
        },
    }
}

/// The command a media type's still comes from.
#[must_use]
pub fn tool_for(media_type: &str) -> Option<&'static str> {
    match Still::of(media_type)? {
        Still::Page => Some("pdftoppm"),
        Still::Frame | Still::Poster | Still::RawPreview => Some("ffmpeg"),
    }
}

/// Copy a stored original into `file` in [`COPY_CHUNK`] pieces, so the caller
/// holds one piece and never the whole file. Returns the byte count.
fn copy_to(store: &dyn Store, path: &str, file: &mut std::fs::File) -> Result<u64, StoreError> {
    let mut at = 0u64;
    loop {
        let chunk = store.read_range(path, at..at.saturating_add(COPY_CHUNK))?;
        if chunk.is_empty() {
            return Ok(at);
        }
        file.write_all(&chunk).map_err(|error| StoreError::Io {
            path: PathBuf::from(path),
            source: error,
        })?;
        at += chunk.len() as u64;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::FileStore;

    fn source(media_type: &str, extension: &str) -> PreviewSource {
        PreviewSource {
            storage_id: "derivative-1".to_string(),
            extension: Some(extension.to_string()),
            media_type: Some(media_type.to_string()),
            has_original: true,
        }
    }

    /// A type with no still and no decoder is a skip, with a typed reason, and
    /// the store is never touched.
    #[test]
    fn a_document_is_skipped_without_reading_it() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = FileStore::new(dir.path(), "/files");
        let work = make_preview(&store, &source("application/msword", "doc"), false);
        assert_eq!(
            work,
            DerivativeWork::Skipped {
                reason: "unsupportedType: application/msword".to_string(),
                tool: None,
            }
        );
    }

    /// A missing tool is a typed skip, not a failure: the install still stores
    /// the file and the state says why no preview exists.
    #[test]
    fn a_missing_tool_is_skipped_with_its_name() {
        let work = skipped(
            ThumbnailSkip::Still(ToolError::ToolMissing("pdftoppm")),
            Some("pdftoppm"),
        );
        assert_eq!(work.reason(), Some("toolMissing: pdftoppm"));
        assert_eq!(work.tool(), Some("pdftoppm"));
    }

    /// Bytes that claim an image and do not decode are a failure, with the
    /// decoder's reason, so a reader can show it and a retry can start again.
    #[test]
    fn a_decode_failure_is_failed_with_its_reason() {
        let work = skipped(
            ThumbnailSkip::Decode("the image has no pixels".to_string()),
            None,
        );
        assert_eq!(
            work.reason(),
            Some("the source did not decode as an image: the image has no pixels")
        );
    }

    /// The still a tool makes names the command in the outcome.
    #[test]
    fn a_tool_names_the_command_it_runs() {
        assert_eq!(tool_for("video/mp4"), Some("ffmpeg"));
        assert_eq!(tool_for("application/pdf"), Some("pdftoppm"));
        assert_eq!(tool_for("image/x-nikon-nef"), Some("ffmpeg"));
        assert_eq!(tool_for("image/jpeg"), None);
    }
}
