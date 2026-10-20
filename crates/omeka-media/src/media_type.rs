//! Content-based media type detection, the port of `finfo(FILEINFO_MIME_TYPE)`
//! in `File/TempFile::getMediaType`.
//!
//! Upstream reads the type from the file's bytes with PHP's `finfo`, maps a
//! short alias table, and stores the result on the asset or media row. The port
//! has no libmagic, so it detects the image formats the image decoder already
//! knows (`image::guess_format`, `File/Thumbnailer/Gd.php` covers the same
//! four) plus a UTF-8 text guess, and answers `application/octet-stream` for
//! anything else. The divergence is only visible in the error message for a
//! file the whitelist already rejects.
//!
//! [`extension_for_media_type`] is the `File/TempFile` `$mediaTypeMap` fallback
//! for a source name with no usable extension, limited to the media types the
//! asset whitelist allows (`application/config/module.config.php:44-58`).

/// The media type of a byte slice, as `TempFile::getMediaType` reads it.
///
/// An image the `image` crate recognises keeps the crate's MIME name, a byte
/// slice that is valid UTF-8 and carries no NUL is `text/plain`, and anything
/// else is `application/octet-stream`.
#[must_use]
pub fn detected_media_type(bytes: &[u8]) -> String {
    if let Ok(format) = image::guess_format(bytes) {
        return format.to_mime_type().to_string();
    }
    if is_text(bytes) {
        return "text/plain".to_string();
    }
    "application/octet-stream".to_string()
}

/// The first extension `File/TempFile`'s `$mediaTypeMap` lists for a media type,
/// for the four types `api_assets.allowed_media_types` names.
///
/// Upstream's map is 3,300 lines; the port carries the entries the whitelist
/// can actually reach, so a rejected type falls back to no extension and is
/// refused by the extension check as well.
#[must_use]
pub fn extension_for_media_type(media_type: &str) -> Option<&'static str> {
    Some(match media_type {
        "image/png" => "png",
        "image/jpeg" => "jpeg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        _ => return None,
    })
}

/// Whether a byte slice reads as UTF-8 text: valid UTF-8, and no NUL byte.
fn is_text(bytes: &[u8]) -> bool {
    !bytes.contains(&0) && std::str::from_utf8(bytes).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The four image formats the asset whitelist allows are detected from
    /// their magic bytes, whatever the source name says.
    #[test]
    fn image_formats_are_detected_from_bytes() {
        assert_eq!(
            detected_media_type(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]),
            "image/png"
        );
        assert_eq!(detected_media_type(&[0xFF, 0xD8, 0xFF, 0xE0]), "image/jpeg");
        assert_eq!(detected_media_type(b"GIF89a\x01\x00"), "image/gif");
        assert_eq!(
            detected_media_type(b"RIFF\x1a\x00\x00\x00WEBPVP8 "),
            "image/webp"
        );
        // Both TIFF byte orders: `II*\0` and `MM\0*`.
        assert_eq!(
            detected_media_type(b"II*\x00\x08\x00\x00\x00"),
            "image/tiff"
        );
        assert_eq!(
            detected_media_type(b"MM\x00*\x00\x00\x00\x08"),
            "image/tiff"
        );
    }

    /// Text is `text/plain` and an unrecognised binary is
    /// `application/octet-stream`, the two `finfo` answers the message for a
    /// rejected file can carry.
    #[test]
    fn text_and_binary_fallbacks() {
        assert_eq!(detected_media_type(b"hello text\n"), "text/plain");
        assert_eq!(
            detected_media_type(&[0x00, 0x01, 0x02, 0x03]),
            "application/octet-stream"
        );
    }

    /// `$mediaTypeMap`'s first extension for the whitelist, and `None` for a
    /// type the map (as ported) does not carry.
    #[test]
    fn extension_fallback_for_whitelisted_types() {
        assert_eq!(extension_for_media_type("image/png"), Some("png"));
        assert_eq!(extension_for_media_type("image/jpeg"), Some("jpeg"));
        assert_eq!(extension_for_media_type("image/gif"), Some("gif"));
        assert_eq!(extension_for_media_type("image/webp"), Some("webp"));
        assert_eq!(extension_for_media_type("text/plain"), None);
    }
}
