//! Content-based media type detection, the port of `File/TempFile::getMediaType`.
//!
//! Upstream reads the type from the file's bytes with PHP's `finfo`
//! (libmagic), then maps `TempFile::MEDIA_TYPE_ALIASES`. The port has no
//! libmagic. It reads binary signatures with the `infer` crate and renames
//! the few answers where `infer` and libmagic disagree ([`FINFO_NAMES`]), and
//! it reads UTF-8 text itself (JSON, XML, SVG, HTML, RTF, plain). The answer
//! then goes through the same alias table upstream uses. `tests/media_type.rs`
//! pins each format against libmagic's answer on the oracle.
//!
//! [`extension_for_media_type`] is the `File/TempFile` `$mediaTypeMap` fallback
//! for a source name with no usable extension.

/// The answer for bytes nothing recognises, as libmagic gives it.
const OCTET_STREAM: &str = "application/octet-stream";

/// The `infer` answers libmagic 5.46 names differently, measured on the
/// oracle. `infer` reports a HEIC file as `image/heif`, an Opus file by its
/// codec where libmagic names the Ogg container, and a compound file that is
/// not a known Office document (a `Thumbs.db`) as `x-ole-storage`.
const FINFO_NAMES: &[(&str, &str)] = &[
    ("image/heif", "image/heic"),
    ("audio/opus", "audio/ogg"),
    ("audio/m4a", "audio/x-m4a"),
    ("audio/x-flac", "audio/flac"),
    (
        "application/vnd.microsoft.portable-executable",
        "application/x-dosexec",
    ),
    ("application/x-ole-storage", "application/CDFV2"),
];

/// `TempFile::MEDIA_TYPE_ALIASES` (`File/TempFile.php:19-58`), verbatim.
const MEDIA_TYPE_ALIASES: &[(&str, &str)] = &[
    ("application/x-ogg", "application/ogg"),
    ("text/rtf", "application/rtf"),
    ("audio/mid", "audio/midi"),
    ("audio/x-midi", "audio/midi"),
    ("audio/mp3", "audio/mpeg"),
    ("audio/mpeg3", "audio/mpeg"),
    ("audio/x-mp3", "audio/mpeg"),
    ("audio/x-mpeg", "audio/mpeg"),
    ("audio/x-mpeg3", "audio/mpeg"),
    ("audio/x-mpegaudio", "audio/mpeg"),
    ("audio/x-mpg", "audio/mpeg"),
    ("audio/x-ogg", "audio/ogg"),
    ("audio/aac", "audio/x-aac"),
    ("audio/aiff", "audio/x-aiff"),
    ("audio/x-wma", "audio/x-ms-wma"),
    ("audio/wma", "audio/x-ms-wma"),
    ("audio/x-mp4", "audio/mp4"),
    ("audio/x-m4a", "audio/mp4"),
    ("audio/wav", "audio/x-wav"),
    ("image/x-ms-bmp", "image/bmp"),
    ("image/icon", "image/x-icon"),
    ("video/x-m4v", "video/mp4"),
    ("video/asf", "video/x-ms-asf"),
    ("video/wmv", "video/x-ms-wmv"),
    ("video/avi", "video/x-msvideo"),
    ("video/msvideo", "video/x-msvideo"),
];

/// The media type of a byte slice, as `TempFile::getMediaType` reads it.
#[must_use]
pub fn detected_media_type(bytes: &[u8]) -> String {
    let found = match infer::get(bytes) {
        // `infer` reads text by its first bytes only, so an SVG is `text/xml`
        // to it; text is read here instead.
        Some(kind) if kind.mime_type() == "application/zip" => {
            office_open_xml(bytes).unwrap_or("application/zip")
        }
        Some(kind) if kind.matcher_type() != infer::MatcherType::Text => {
            lookup(FINFO_NAMES, kind.mime_type())
        }
        _ => text_media_type(bytes).unwrap_or(OCTET_STREAM),
    };
    lookup(MEDIA_TYPE_ALIASES, found).to_string()
}

/// The media type of a file whose source name carries `extension`.
///
/// The byte read alone cannot name every format: WebVTT and SubRip are plain
/// text, a glTF document is JSON and a GLB is a binary nothing recognises. For
/// those the extension is the better authority, and only when the byte read
/// answered the container (see [`EXTENSION_TYPES`]). A `.png` holding a PNG
/// stays `image/png`, whatever the name says.
#[must_use]
pub fn detected_media_type_of(bytes: &[u8], extension: Option<&str>) -> String {
    let detected = detected_media_type(bytes);
    let Some(extension) = extension else {
        return detected;
    };
    EXTENSION_TYPES
        .iter()
        .find(|(container, name, _)| *container == detected && *name == extension)
        .map_or(detected, |(_, _, media_type)| (*media_type).to_string())
}

/// The types the byte read cannot tell from their container, as
/// `(detected, extension, media type)`. The stored type is what the download
/// answers with, and what a file renderer picks, so the correction is made
/// here rather than at serving time.
const EXTENSION_TYPES: &[(&str, &str, &str)] = &[
    ("text/plain", "vtt", "text/vtt"),
    ("text/plain", "srt", "application/x-subrip"),
    ("application/json", "gltf", "model/gltf+json"),
    ("application/octet-stream", "glb", "model/gltf-binary"),
    ("audio/ogg", "ogv", "video/ogg"),
];

/// The first extension `File/TempFile`'s `$mediaTypeMap` lists for a media
/// type (`application/data/media-types/media-type-map.php`).
///
/// Upstream's map is 3,300 lines; the port carries the entries for the types
/// [`detected_media_type`] can answer, read from the oracle's copy. A type
/// the map has no extension for (`audio/flac`, `text/vtt`, `model/gltf+json`)
/// is `None`, as it is upstream.
#[must_use]
pub fn extension_for_media_type(media_type: &str) -> Option<&'static str> {
    Some(match media_type {
        "application/json" => "json",
        "application/msword" => "doc",
        "application/octet-stream" => "bin",
        "application/pdf" => "pdf",
        "application/rtf" => "rtf",
        "application/vnd.ms-excel" => "xls",
        "application/vnd.ms-powerpoint" => "ppt",
        "application/vnd.oasis.opendocument.presentation" => "odp",
        "application/vnd.oasis.opendocument.spreadsheet" => "ods",
        "application/vnd.oasis.opendocument.text" => "odt",
        "application/vnd.openxmlformats-officedocument.presentationml.presentation" => "pptx",
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" => "xlsx",
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document" => "docx",
        "application/x-shockwave-flash" => "swf",
        "application/x-subrip" => "srt",
        "application/x-tar" => "tar",
        "application/zip" => "zip",
        "audio/midi" => "mid",
        "audio/mp4" => "m4a",
        "audio/mpeg" => "mp3",
        "audio/ogg" => "oga",
        "audio/x-aac" => "aac",
        "audio/x-aiff" => "aif",
        "audio/x-wav" => "wav",
        "image/avif" => "avif",
        "image/bmp" => "bmp",
        "image/gif" => "gif",
        "image/heic" => "heic",
        "image/jp2" => "jp2",
        "image/jpeg" => "jpeg",
        "image/png" => "png",
        "image/svg+xml" => "svg",
        "image/tiff" => "tiff",
        "image/webp" => "webp",
        "text/html" => "html",
        "text/plain" => "txt",
        "video/mp4" => "mp4",
        "video/mpeg" => "mpeg",
        "video/ogg" => "ogv",
        "video/quicktime" => "qt",
        "video/webm" => "webm",
        "video/x-flv" => "flv",
        "video/x-matroska" => "mkv",
        "video/x-ms-wmv" => "wmv",
        "video/x-msvideo" => "avi",
        _ => return None,
    })
}

fn lookup<'a>(table: &[(&str, &'a str)], name: &'a str) -> &'a str {
    table
        .iter()
        .find(|(from, _)| *from == name)
        .map_or(name, |(_, to)| to)
}

/// The Office Open XML type of a zip, by libmagic's `msooxml` rule: the
/// first entry is package metadata, and the third entry's name, else the
/// fourth's, starts with the part folder. `infer` has the same rule but
/// reads a package of exactly three entries as a plain zip.
fn office_open_xml(bytes: &[u8]) -> Option<&'static str> {
    const PACKAGE: [&[u8]; 4] = [
        b"[Content_Types].xml",
        b"_rels/.rels",
        b"docProps",
        b"customXml",
    ];
    // A local file header is 30 bytes before its name; libmagic's `&26`
    // after matching the 4-byte signature lands on the name.
    const NAME: usize = 30;
    let first = bytes.get(NAME..)?;
    if let Some(found) = part_type(first) {
        return Some(found);
    }
    if !PACKAGE.iter().any(|name| first.starts_with(name)) {
        return None;
    }
    let first_size = u32::from_le_bytes(bytes.get(18..22)?.try_into().ok()?);
    let second = next_header(bytes, usize::try_from(first_size).ok()?.checked_add(49)?)?;
    let third = next_header(bytes, second + NAME)?;
    part_type(bytes.get(third + NAME..)?).or_else(|| {
        let fourth = next_header(bytes, third + NAME + 26)?;
        part_type(bytes.get(fourth + NAME..)?)
    })
}

/// The offset of the next zip local file header within 6000 bytes of
/// `start`, libmagic's `search/6000`.
fn next_header(bytes: &[u8], start: usize) -> Option<usize> {
    let window = bytes.get(start..bytes.len().min(start + 6000))?;
    window
        .windows(4)
        .position(|candidate| candidate == b"PK\x03\x04")
        .map(|found| start + found)
}

fn part_type(name: &[u8]) -> Option<&'static str> {
    if name.starts_with(b"word/") {
        Some("application/vnd.openxmlformats-officedocument.wordprocessingml.document")
    } else if name.starts_with(b"ppt/") {
        Some("application/vnd.openxmlformats-officedocument.presentationml.presentation")
    } else if name.starts_with(b"xl/") {
        Some("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet")
    } else {
        None
    }
}

/// The type of bytes that read as text: valid UTF-8 with no NUL. `None` for
/// anything else.
fn text_media_type(bytes: &[u8]) -> Option<&'static str> {
    if bytes.is_empty() {
        return Some("application/x-empty");
    }
    if bytes.contains(&0) {
        return None;
    }
    let text = std::str::from_utf8(bytes).ok()?;
    let start = text.trim_start_matches('\u{feff}').trim_start();
    Some(if start.starts_with("{\\rtf") {
        "text/rtf"
    } else if (start.starts_with('{') || start.starts_with('['))
        && serde_json::from_str::<serde_json::Value>(text).is_ok()
    {
        "application/json"
    } else if start.starts_with("<svg") || (infer::text::is_xml(bytes) && start.contains("<svg")) {
        "image/svg+xml"
    } else if infer::text::is_xml(bytes) {
        "text/xml"
    } else if infer::text::is_html(bytes) {
        "text/html"
    } else {
        "text/plain"
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `$mediaTypeMap`'s first extension, and `None` for a type the map has no
    /// entry for.
    #[test]
    fn extension_fallback_follows_the_media_type_map() {
        assert_eq!(extension_for_media_type("image/jpeg"), Some("jpeg"));
        assert_eq!(extension_for_media_type("application/pdf"), Some("pdf"));
        assert_eq!(extension_for_media_type("text/plain"), Some("txt"));
        assert_eq!(extension_for_media_type("video/ogg"), Some("ogv"));
        assert_eq!(extension_for_media_type("video/x-matroska"), Some("mkv"));
        assert_eq!(
            extension_for_media_type("application/x-subrip"),
            Some("srt")
        );
        assert_eq!(extension_for_media_type("audio/flac"), None);
        assert_eq!(extension_for_media_type("text/vtt"), None);
    }

    /// The formats a byte read cannot name are read by their source
    /// extension, and only when the bytes answered the container.
    #[test]
    fn the_extension_names_what_the_bytes_cannot() {
        let vtt = b"WEBVTT\n\n00:00:00.000 --> 00:00:00.400\nA line.\n";
        assert_eq!(detected_media_type(vtt), "text/plain");
        assert_eq!(detected_media_type_of(vtt, Some("vtt")), "text/vtt");

        let srt = b"1\n00:00:00,000 --> 00:00:00,400\nA line.\n";
        assert_eq!(
            detected_media_type_of(srt, Some("srt")),
            "application/x-subrip"
        );

        let gltf = b"{\"asset\":{\"version\":\"2.0\"}}";
        assert_eq!(detected_media_type(gltf), "application/json");
        assert_eq!(
            detected_media_type_of(gltf, Some("gltf")),
            "model/gltf+json"
        );

        let glb = b"glTF\x02\x00\x00\x00\x0c\x00\x00\x00";
        assert_eq!(
            detected_media_type_of(glb, Some("glb")),
            "model/gltf-binary"
        );

        let ogg = b"OggS\x00\x02\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00";
        assert_eq!(detected_media_type(ogg), "audio/ogg");
        assert_eq!(detected_media_type_of(ogg, Some("ogv")), "video/ogg");
    }

    /// A name that disagrees with the bytes does not change a type the byte
    /// read is sure of.
    #[test]
    fn a_misleading_extension_does_not_override_a_signature() {
        let png = b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR";
        assert_eq!(detected_media_type_of(png, Some("srt")), "image/png");
        assert_eq!(detected_media_type_of(png, None), "image/png");
    }

    /// Opus rides in an Ogg container, and libmagic names the container, not
    /// the codec.
    #[test]
    fn an_opus_stream_is_the_ogg_container() {
        let mut eye = b"OggS\x00\x02\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x01\x13OpusHead\x01\x01\x00\x00\x80\xbb\x00\x00\x00\x00\x00".to_vec();
        eye.extend_from_slice(&[0; 8]);
        assert_eq!(detected_media_type(&eye), "audio/ogg");
    }
}
