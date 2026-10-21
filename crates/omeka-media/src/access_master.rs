//! The JPEG access master one stored original makes, and the derivatives from
//! it.
//!
//! [`crate::access`] decides that a stored file needs a copy a browser can show
//! and names the chain of commands that can make it. This module runs the first
//! of that chain this install has, writes the result at the source's own size
//! under the `access` prefix, and makes the three usual thumbnails from the
//! master. The original is never changed: it stays the preservation copy.
//!
//! The chain is tried in order and a command that runs and fails falls through
//! to the next one, which is what a camera RAW needs when `exiftool` finds no
//! embedded preview and `libraw` has to decode the sensor data. A command this
//! install does not have is skipped with its name, never an error.
//!
//! `libraw` writes a TIFF and cannot write a JPEG, so its decode is two runs:
//! `dcraw_emu` to standard output, then `vips copy` to the master.

use std::io::Write;
use std::path::Path;

use image::codecs::jpeg::JpegEncoder;
use image::metadata::Orientation;

use crate::access::{self, Access, AccessTarget, StoredFile};
use crate::derivative::COPY_CHUNK;
use crate::store::{Store, StoreError, StorePrefix, storage_path};
use crate::thumbnailer::{ImageThumbnailer, thumbnail_path};
use crate::tool::{ACCESS_JPEG_QUALITY, AccessTool, Still, ToolError, Tools, image_jpeg_args};

/// The stored type every access master answers with.
pub const MASTER_MEDIA_TYPE: &str = "image/jpeg";

/// The most one command may write to standard output while it decodes a RAW
/// file. A 6016x4016 frame at 16 bits a sample is 145 MB as an uncompressed
/// TIFF, which is five times the still limit.
const DECODED_LIMIT: usize = 512 * 1024 * 1024;

/// The facts of one stored original, owned so the work can run on the blocking
/// pool.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MasterSource {
    pub storage_id: String,
    pub extension: Option<String>,
    pub media_type: Option<String>,
    pub has_original: bool,
    pub size: Option<u64>,
}

/// What one media's access master work did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MasterWork {
    /// The master and the three thumbnails are in the store.
    Ready {
        /// The command that made it.
        tool: &'static str,
    },
    /// No copy is made for this media, or no command that makes one is
    /// installed.
    Skipped {
        reason: String,
        tool: Option<&'static str>,
    },
    /// The store or a command that ran refused.
    Failed {
        reason: String,
        tool: Option<&'static str>,
    },
}

impl MasterWork {
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
            Self::Ready { tool } => Some(*tool),
            Self::Skipped { tool, .. } | Self::Failed { tool, .. } => *tool,
        }
    }
}

/// Make the JPEG access master for one stored original, and the three
/// thumbnails from it.
///
/// # Errors
/// None: a store that refuses or a command that fails is a [`MasterWork`], the
/// way a still is a [`crate::derivative::DerivativeWork`].
#[must_use]
pub fn make_master(store: &dyn Store, tools: &Tools, source: &MasterSource) -> MasterWork {
    let media_type = source.media_type.clone().unwrap_or_default();
    let original_path = storage_path(
        StorePrefix::Original,
        &source.storage_id,
        source.extension.as_deref(),
    );
    if !source.has_original {
        return MasterWork::Failed {
            reason: "the row stored no original".to_string(),
            tool: None,
        };
    }
    // The one table, asked again for the file just stored: a type whose layout
    // the decoder now reads is no copy, and a chain names the commands that can
    // make one.
    let facts = access::facts(
        store,
        tools,
        &StoredFile {
            storage_path: &original_path,
            media_type: source.media_type.as_deref(),
            extension: source.extension.as_deref(),
            size: source.size,
        },
    );
    let chain = match Access::of(&facts) {
        Some(Access::Copy {
            target: AccessTarget::Jpeg,
            tools: chain,
        }) => chain,
        _ => {
            return MasterWork::Skipped {
                reason: format!("no JPEG access master for {media_type}"),
                tool: None,
            };
        }
    };

    let dir = match tempfile::tempdir() {
        Ok(dir) => dir,
        Err(error) => {
            return MasterWork::Failed {
                reason: format!("no temporary folder: {error}"),
                tool: None,
            };
        }
    };
    // The extension names a camera RAW's maker to `exiftool`, which reads a
    // file by its maker's type (`tool::Still::make_from_bytes`).
    let original = dir.path().join(match source.extension.as_deref() {
        Some(extension) if !extension.is_empty() => format!("source.{extension}"),
        _ => "source".to_string(),
    });
    if let Err(error) = stream_out(store, &original_path, &original) {
        return MasterWork::Failed {
            reason: format!("cannot read {original_path}: {error}"),
            tool: None,
        };
    }

    let mut missing = None;
    let mut failed = None;
    for &tool in chain {
        match attempt(tools, tool, &original, dir.path()) {
            Ok((bytes, orientation)) => {
                let bytes = match upright(&bytes, orientation) {
                    Ok(bytes) => bytes,
                    Err(reason) => {
                        failed = Some((tool.command(), reason));
                        continue;
                    }
                };
                return write_master(store, source, &bytes, tool);
            }
            Err(Attempt::Missing(name)) => missing = Some(name),
            Err(Attempt::Failed(name, error)) => failed = Some((name, error.to_string())),
        }
    }
    match failed {
        Some((tool, reason)) => MasterWork::Failed {
            reason,
            tool: Some(tool),
        },
        None => {
            let name = missing.unwrap_or(chain[0].command());
            MasterWork::Skipped {
                reason: format!("needs {name}, not installed"),
                tool: Some(name),
            }
        }
    }
}

/// Why one command in the chain made no image.
enum Attempt {
    /// The install does not have the command, or a command the decode needs.
    Missing(&'static str),
    /// The command ran and refused.
    Failed(&'static str, ToolError),
}

/// Run one command of the chain. `Ok` is the image and the turn its container
/// asks for, when the command's output carries none of its own.
fn attempt(
    tools: &Tools,
    tool: AccessTool,
    original: &Path,
    dir: &Path,
) -> Result<(Vec<u8>, Option<Orientation>), Attempt> {
    if tools.access_program(tool).is_err() {
        return Err(Attempt::Missing(tool.command()));
    }
    if tool == AccessTool::ExifTool {
        // The one place the preview's second tag lives: `JpgFromRaw` first, then
        // the smaller `PreviewImage`, with the container's orientation beside
        // it (`tool::Still::RawPreview`).
        return match Still::RawPreview.make(tools, original) {
            Ok(still) => Ok((still.bytes, still.orientation)),
            Err(error) => Err(Attempt::Failed("exiftool", error)),
        };
    }
    let output = dir.join(format!("master.{}", AccessTarget::Jpeg.suffix()));
    let Some(arguments) = image_jpeg_args(tool, original, &output) else {
        return Err(Attempt::Missing(tool.command()));
    };
    match tool {
        AccessTool::LibRaw => decode_raw(tools, &arguments, &output, dir),
        AccessTool::Vips | AccessTool::Ghostscript => {
            tools
                .run_access(tool, &arguments, DECODED_LIMIT)
                .map_err(|error| Attempt::Failed(tool.command(), error))?;
            read_output(&output, tool.command()).map(|bytes| (bytes, None))
        }
        AccessTool::Ffmpeg | AccessTool::ExifTool => Err(Attempt::Missing(tool.command())),
    }
}

/// `libraw` writes a TIFF to standard output; `vips` encodes the master from
/// it, which is the second run the chain's documentation names.
fn decode_raw(
    tools: &Tools,
    arguments: &[std::ffi::OsString],
    output: &Path,
    dir: &Path,
) -> Result<(Vec<u8>, Option<Orientation>), Attempt> {
    if tools.access_program(AccessTool::Vips).is_err() {
        return Err(Attempt::Missing("vips"));
    }
    let tiff = tools
        .run_access(AccessTool::LibRaw, arguments, DECODED_LIMIT)
        .map_err(|error| Attempt::Failed(tools.libraw_name(), error))?;
    if tiff.is_empty() {
        return Err(Attempt::Failed(
            tools.libraw_name(),
            ToolError::Failed(
                tools.libraw_name(),
                "the decoder wrote no image".to_string(),
            ),
        ));
    }
    let decoded = dir.join("decoded.tif");
    std::fs::write(&decoded, tiff).map_err(|error| {
        Attempt::Failed(
            tools.libraw_name(),
            ToolError::Failed(tools.libraw_name(), error.to_string()),
        )
    })?;
    let Some(arguments) = image_jpeg_args(AccessTool::Vips, &decoded, output) else {
        return Err(Attempt::Missing("vips"));
    };
    tools
        .run_access(AccessTool::Vips, &arguments, DECODED_LIMIT)
        .map_err(|error| Attempt::Failed("vips", error))?;
    read_output(output, "vips").map(|bytes| (bytes, None))
}

/// Read what a command wrote to `output`.
fn read_output(output: &Path, tool: &'static str) -> Result<Vec<u8>, Attempt> {
    let bytes = std::fs::read(output).map_err(|error| {
        Attempt::Failed(
            tool,
            ToolError::Failed(tool, format!("no image was written: {error}")),
        )
    })?;
    if bytes.is_empty() {
        return Err(Attempt::Failed(
            tool,
            ToolError::Failed(tool, "the image is empty".to_string()),
        ));
    }
    Ok(bytes)
}

/// Turn the pixels as the container's EXIF orientation says.
fn upright(bytes: &[u8], orientation: Option<Orientation>) -> Result<Vec<u8>, String> {
    let Some(orientation) = orientation else {
        return Ok(bytes.to_vec());
    };
    let mut image = image::load_from_memory(bytes).map_err(|error| error.to_string())?;
    image.apply_orientation(orientation);
    let mut out = Vec::new();
    JpegEncoder::new_with_quality(&mut out, ACCESS_JPEG_QUALITY)
        .encode_image(&image::DynamicImage::ImageRgb8(image.to_rgb8()))
        .map_err(|error| error.to_string())?;
    Ok(out)
}

/// Store the master, then the three thumbnails made from it.
fn write_master(
    store: &dyn Store,
    source: &MasterSource,
    bytes: &[u8],
    tool: AccessTool,
) -> MasterWork {
    let path = access::copy_path(&source.storage_id, AccessTarget::Jpeg);
    if let Err(error) = store.put(bytes, &path, Some(MASTER_MEDIA_TYPE)) {
        return MasterWork::Failed {
            reason: format!("cannot write {path}: {error}"),
            tool: Some(tool.command()),
        };
    }
    let derivatives = match ImageThumbnailer::new().derivative_bytes(bytes, Some(MASTER_MEDIA_TYPE))
    {
        Ok(derivatives) => derivatives,
        Err(skip) => {
            return MasterWork::Failed {
                reason: skip.to_string(),
                tool: Some(tool.command()),
            };
        }
    };
    for (kind, thumbnail) in derivatives {
        let path = thumbnail_path(&source.storage_id, kind);
        if let Err(error) = store.put(&thumbnail, &path, Some(MASTER_MEDIA_TYPE)) {
            return MasterWork::Failed {
                reason: format!("cannot write {path}: {error}"),
                tool: Some(tool.command()),
            };
        }
    }
    MasterWork::Ready {
        tool: tool.command(),
    }
}

/// Copy a stored original to `path` in [`COPY_CHUNK`] pieces, so an original in
/// an object store costs one piece at a time.
fn stream_out(store: &dyn Store, storage_path: &str, path: &Path) -> Result<u64, StoreError> {
    let mut file = std::fs::File::create(path).map_err(|error| StoreError::Io {
        path: path.to_path_buf(),
        source: error,
    })?;
    let mut at = 0u64;
    loop {
        let chunk = store.read_range(storage_path, at..at.saturating_add(COPY_CHUNK))?;
        if chunk.is_empty() {
            return Ok(at);
        }
        file.write_all(&chunk).map_err(|error| StoreError::Io {
            path: path.to_path_buf(),
            source: error,
        })?;
        at += chunk.len() as u64;
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use std::path::Path;

    use super::*;
    use crate::file_render::ThumbnailType;
    use crate::store::FileStore;

    fn corpus(name: &str) -> Vec<u8> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/media-kinds")
            .join(name);
        std::fs::read(&path).unwrap_or_else(|error| panic!("{name}: {error}"))
    }

    /// The name the row gives one corpus file: the corpus name without its
    /// extension, which the row keeps beside it.
    fn stem(name: &str) -> &str {
        name.rsplit_once('.').map_or(name, |(stem, _)| stem)
    }

    /// Store one corpus file as an original and answer its source.
    fn source(store: &FileStore, name: &str, media_type: &str, extension: &str) -> MasterSource {
        let bytes = corpus(name);
        let storage_id = stem(name);
        store
            .put(&bytes, &format!("original/{name}"), Some(media_type))
            .expect("store");
        MasterSource {
            storage_id: storage_id.to_string(),
            extension: Some(extension.to_string()),
            media_type: Some(media_type.to_string()),
            has_original: true,
            size: Some(bytes.len() as u64),
        }
    }

    fn master(store: &FileStore, name: &str) -> Vec<u8> {
        store
            .read(&access::copy_path(stem(name), AccessTarget::Jpeg))
            .expect("the master")
    }

    fn size(bytes: &[u8]) -> (u32, u32) {
        let image = image::load_from_memory(bytes).expect("an image");
        (image.width(), image.height())
    }

    /// A camera RAW's embedded preview is the master, at the preview's own size
    /// and turned upright by the container's EXIF orientation. The large
    /// derivative is made from it, so a canvas of the master is larger than the
    /// derivative a canvas used before this unit.
    #[test]
    fn a_raw_preview_becomes_an_upright_master_at_full_size() {
        let tools = Tools::on_path();
        if tools.exiftool().is_none() {
            eprintln!("skipping: no exiftool on PATH");
            return;
        }
        let dir = tempfile::tempdir().expect("tempdir");
        let store = FileStore::new(dir.path(), "/files");
        let source = source(&store, "image.nef", "image/x-nikon-nef", "nef");
        assert_eq!(
            make_master(&store, tools, &source),
            MasterWork::Ready { tool: "exiftool" }
        );
        // The fixture's preview is 1200x800 with Orientation 6.
        assert_eq!(size(&master(&store, "image.nef")), (800, 1200));
        let large = store
            .read(&thumbnail_path(stem("image.nef"), ThumbnailType::Large))
            .expect("the large derivative");
        assert_eq!(size(&large), (533, 800), "the derivative is capped at 800");
    }

    /// A 16-bit TIFF the image crate refuses gets its master through `vips`, at
    /// the source's own 1200x800.
    #[test]
    fn a_16_bit_tiff_gets_a_master_through_vips() {
        let tools = Tools::on_path();
        if tools.vips().is_none() {
            eprintln!("skipping: no vips on PATH");
            return;
        }
        let dir = tempfile::tempdir().expect("tempdir");
        let store = FileStore::new(dir.path(), "/files");
        let source = source(&store, "image-deep.tif", "image/tiff", "tif");
        assert_eq!(
            make_master(&store, tools, &source),
            MasterWork::Ready { tool: "vips" }
        );
        assert_eq!(size(&master(&store, "image-deep.tif")), (1200, 800));
    }

    /// The three TIFF layouts and the Photoshop document all reach a master
    /// through `vips`.
    #[test]
    fn the_odd_tiffs_and_the_psd_get_a_master_through_vips() {
        let tools = Tools::on_path();
        if tools.vips().is_none() {
            eprintln!("skipping: no vips on PATH");
            return;
        }
        let dir = tempfile::tempdir().expect("tempdir");
        let store = FileStore::new(dir.path(), "/files");
        for (name, media_type, extension) in [
            ("image-cmyk.tif", "image/tiff", "tif"),
            ("image-pages.tif", "image/tiff", "tif"),
            ("image.psd", "image/vnd.adobe.photoshop", "psd"),
        ] {
            let source = source(&store, name, media_type, extension);
            assert_eq!(
                make_master(&store, tools, &source),
                MasterWork::Ready { tool: "vips" },
                "{name}"
            );
            assert_eq!(size(&master(&store, name)), (120, 80), "{name}");
        }
    }

    /// A PostScript page's first page is rasterised by `ghostscript` at its own
    /// chosen resolution, and the master is a JPEG.
    #[test]
    fn an_eps_page_gets_a_master_through_ghostscript() {
        let tools = Tools::on_path();
        if tools.ghostscript().is_none() {
            eprintln!("skipping: no gs on PATH");
            return;
        }
        let dir = tempfile::tempdir().expect("tempdir");
        let store = FileStore::new(dir.path(), "/files");
        let source = source(&store, "image.eps", "application/postscript", "eps");
        assert_eq!(
            make_master(&store, tools, &source),
            MasterWork::Ready { tool: "gs" }
        );
        let (width, height) = size(&master(&store, "image.eps"));
        assert!(width > 1000 && height > 1000, "{width}x{height}");
    }

    /// A RAW with no embedded preview falls through to `libraw`: `dcraw_emu`
    /// writes a TIFF to standard output and `vips` encodes the master from it.
    /// The decoder stands in so the two runs are what the test proves.
    #[cfg(unix)]
    #[test]
    fn a_raw_decode_runs_libraw_then_vips() {
        use std::os::unix::fs::PermissionsExt as _;

        let Some(vips) = Tools::on_path().vips().map(std::path::Path::to_path_buf) else {
            eprintln!("skipping: no vips on PATH");
            return;
        };
        let dir = tempfile::tempdir().expect("tempdir");
        let store = FileStore::new(dir.path(), "/files");
        let decoded = dir.path().join("decoded.tif");
        std::fs::write(&decoded, corpus("image-deep.tif")).expect("the decoded frame");
        let log = dir.path().join("args");
        let decoder = dir.path().join("dcraw_emu");
        std::fs::write(
            &decoder,
            format!(
                "#!/bin/sh\nprintf '%s\\n' \"$*\" > {log}\ncat {decoded}\n",
                log = log.display(),
                decoded = decoded.display()
            ),
        )
        .expect("the stand-in decoder");
        std::fs::set_permissions(&decoder, std::fs::Permissions::from_mode(0o755))
            .expect("make it executable");

        let tools = Tools::none().with_libraw(&decoder).with_vips(&vips);
        let source = source(&store, "image.nef", "image/x-nikon-nef", "nef");
        assert_eq!(
            make_master(&store, &tools, &source),
            MasterWork::Ready { tool: "dcraw_emu" }
        );
        assert_eq!(size(&master(&store, "image.nef")), (1200, 800));
        let arguments = std::fs::read_to_string(&log).expect("the arguments");
        assert!(arguments.contains("-Z -"), "{arguments}");
    }

    /// A copy whose command this install does not have is a typed skip naming
    /// it, not a failure.
    #[test]
    fn a_missing_tool_is_a_typed_skip() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = FileStore::new(dir.path(), "/files");
        let source = source(&store, "image.psd", "image/vnd.adobe.photoshop", "psd");
        let work = make_master(&store, &Tools::none(), &source);
        assert_eq!(
            work,
            MasterWork::Skipped {
                reason: "needs vips, not installed".to_string(),
                tool: Some("vips"),
            }
        );
        assert!(
            !store
                .exists(&access::copy_path(stem("image.psd"), AccessTarget::Jpeg))
                .expect("exists")
        );
    }

    /// A type the table does not make a JPEG copy for is not a master, even if
    /// a row was queued for it.
    #[test]
    fn an_image_the_browser_shows_is_no_master() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = FileStore::new(dir.path(), "/files");
        let source = source(&store, "image.png", "image/png", "png");
        let work = make_master(&store, Tools::on_path(), &source);
        assert!(matches!(work, MasterWork::Skipped { .. }), "{work:?}");
    }
}
