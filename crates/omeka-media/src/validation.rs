//! File validation, the port of `Omeka\File\Validator`.
//!
//! Upstream refuses a file whose media type or resolved extension is not on
//! an allow list. The lists and the off switch are three global settings
//! (`media_type_whitelist`, `extension_whitelist`, `disable_file_validation`),
//! whose defaults are `SettingForm::MEDIA_TYPE_WHITELIST` and
//! `SettingForm::EXTENSION_WHITELIST`. This fork's defaults add the modern
//! kinds ([`DEFAULT_MEDIA_TYPES`], [`DEFAULT_EXTENSIONS`]).
//! [`crate::ingest::ingest_file`] runs the check for every file it stores, as
//! `TempFile::mediaIngestFile` does.

use std::fmt;

/// `SettingForm::MEDIA_TYPE_WHITELIST` (`Form/SettingForm.php:22-83`), with
/// this fork's additions. Upstream's list carries a stray comma in
/// `video/x-ms-asf,`, so an ASF file is refused there; this fork repairs it.
///
/// The additions are the types `media_type` answers for the formats a modern
/// collection holds: FLAC, Matroska, AVIF, HEIC, SVG, glTF (both forms),
/// WebVTT, SubRip, the eleven camera RAW formats, the formats a JPEG access
/// master makes viewable (`image/vnd.adobe.photoshop`, `image/x-photoshop`,
/// `image/x-exr`, `image/vnd.radiance`, `application/postscript`,
/// `application/illustrator`, `image/x-eps`), and the audio and video
/// containers the access-copy table names (`video/3gpp`, `video/x-dv`,
/// `video/x-flv`, `video/x-ms-vob`, `audio/amr`, `audio/x-ape`,
/// `audio/x-monkeys-audio`). Each is a deliberate difference from upstream's
/// default and is listed in `docs/compat.md`.
pub const DEFAULT_MEDIA_TYPES: &[&str] = &[
    "application/illustrator",
    "application/msword",
    "application/ogg",
    "application/pdf",
    "application/postscript",
    "application/rtf",
    "application/vnd.ms-access",
    "application/vnd.ms-excel",
    "application/vnd.ms-powerpoint",
    "application/vnd.ms-project",
    "application/vnd.ms-write",
    "application/vnd.oasis.opendocument.chart",
    "application/vnd.oasis.opendocument.database",
    "application/vnd.oasis.opendocument.formula",
    "application/vnd.oasis.opendocument.graphics",
    "application/vnd.oasis.opendocument.presentation",
    "application/vnd.oasis.opendocument.spreadsheet",
    "application/vnd.oasis.opendocument.text",
    "application/vnd.openxmlformats-officedocument.presentationml.presentation",
    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
    "application/x-gzip",
    "application/x-ms-wmp",
    "application/x-msdownload",
    "application/x-shockwave-flash",
    "application/x-subrip",
    "application/x-tar",
    "application/zip",
    "audio/amr",
    "audio/flac",
    "audio/midi",
    "audio/mp4",
    "audio/mpeg",
    "audio/ogg",
    "audio/x-aac",
    "audio/x-aiff",
    "audio/x-ape",
    "audio/x-monkeys-audio",
    "audio/x-ms-wax",
    "audio/x-ms-wma",
    "audio/x-realaudio",
    "audio/x-wav",
    "image/avif",
    "image/bmp",
    "image/gif",
    "image/heic",
    "image/jp2",
    "image/jpeg",
    "image/pjpeg",
    "image/png",
    "image/svg+xml",
    "image/tiff",
    "image/vnd.adobe.photoshop",
    "image/vnd.radiance",
    "image/webp",
    "image/x-adobe-dng",
    "image/x-canon-cr2",
    "image/x-canon-cr3",
    "image/x-eps",
    "image/x-exr",
    "image/x-fuji-raf",
    "image/x-icon",
    "image/x-nikon-nef",
    "image/x-olympus-orf",
    "image/x-panasonic-rw2",
    "image/x-pentax-pef",
    "image/x-samsung-srw",
    "image/x-sigma-x3f",
    "image/x-sony-arw",
    "model/gltf+json",
    "model/gltf-binary",
    "text/css",
    "text/plain",
    "text/richtext",
    "text/vtt",
    "video/3gpp",
    "video/divx",
    "video/dvd",
    "video/mp4",
    "video/mpeg",
    "video/ogg",
    "video/quicktime",
    "video/webm",
    "video/x-dv",
    "video/x-flv",
    "video/x-matroska",
    "video/x-ms-asf",
    "video/x-ms-vob",
    "video/x-ms-wmv",
    "video/x-msvideo",
];

/// `SettingForm::EXTENSION_WHITELIST` (`Form/SettingForm.php:89-97`), with this
/// fork's additions (`3gp`, `amr`, `ape`, `arw`, `avif`, `cr2`, `cr3`, `dng`,
/// `dv`, `flac`, `flv`, `glb`, `gltf`, `heic`, `mkv`, `nef`, `ogv`, `orf`,
/// `pef`, `raf`, `rw2`, `srw`, `srt`, `svg`, `vob`, `vtt`, `x3f`, plus the
/// page and dynamic-range formats an access master covers: `ai`, `eps`, `exr`,
/// `hdr`, `ps`, `psd`).
pub const DEFAULT_EXTENSIONS: &[&str] = &[
    "3gp", "aac", "ai", "aif", "aiff", "amr", "ape", "arw", "asf", "asx", "avi", "avif", "bmp",
    "c", "cc", "class", "cr2", "cr3", "css", "divx", "dng", "doc", "docx", "dv", "eps", "exe",
    "exr", "flac", "flv", "gif", "glb", "gltf", "gz", "gzip", "h", "hdr", "heic", "ico", "j2k",
    "jp2", "jpe", "jpeg", "jpg", "m4a", "m4v", "mdb", "mid", "midi", "mkv", "mov", "mp2", "mp3",
    "mp4", "mpa", "mpe", "mpeg", "mpg", "mpp", "nef", "odb", "odc", "odf", "odg", "odp", "ods",
    "odt", "ogg", "ogv", "opus", "orf", "pdf", "pef", "png", "pot", "pps", "ppt", "pptx", "ps",
    "psd", "qt", "ra", "raf", "ram", "rtf", "rtx", "rw2", "srt", "srw", "svg", "swf", "tar", "tif",
    "tiff", "txt", "vob", "vtt", "wav", "wax", "webm", "webp", "wma", "wmv", "wmx", "wri", "x3f",
    "xla", "xls", "xlsx", "xlt", "xlw", "zip",
];

/// What a store may accept, the state `Omeka\File\Validator` is built with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileValidation {
    /// `disable_file_validation` is on: every file passes.
    Disabled,
    /// A file passes when both lists hold its media type and its resolved
    /// extension. An empty list refuses every file, as `in_array` does.
    Allow {
        /// Media types, compared exactly.
        media_types: Vec<String>,
        /// Lower-case extensions, compared exactly.
        extensions: Vec<String>,
    },
}

impl Default for FileValidation {
    /// Upstream's two default lists, what a fresh install stores.
    fn default() -> Self {
        Self::Allow {
            media_types: owned(DEFAULT_MEDIA_TYPES),
            extensions: owned(DEFAULT_EXTENSIONS),
        }
    }
}

impl FileValidation {
    /// The reasons a file is refused, in upstream's order (media type, then
    /// extension); empty when the file passes. `extension` is the resolved
    /// extension (`TempFile::getExtension`), `None` when there is none.
    #[must_use]
    pub fn refusals(
        &self,
        source_name: &str,
        media_type: &str,
        extension: Option<&str>,
    ) -> Vec<FileRefusal> {
        let Self::Allow {
            media_types,
            extensions,
        } = self
        else {
            return Vec::new();
        };
        let mut refusals = Vec::new();
        if !media_types.iter().any(|allowed| allowed == media_type) {
            refusals.push(FileRefusal::MediaType {
                file: source_name.to_string(),
                media_type: media_type.to_string(),
            });
        }
        if !extension.is_some_and(|extension| extensions.iter().any(|allowed| allowed == extension))
        {
            refusals.push(FileRefusal::Extension {
                file: source_name.to_string(),
                extension: extension.unwrap_or_default().to_string(),
            });
        }
        refusals
    }
}

/// One reason a file is refused. The display text is upstream's message, the
/// msgid the admin translates (`omeka_api::messages::asset_media_type` and
/// `asset_extension` register it).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileRefusal {
    /// The media type is not on the list.
    MediaType {
        /// The source name.
        file: String,
        /// The detected media type.
        media_type: String,
    },
    /// The resolved extension is not on the list; empty when there is none.
    Extension {
        /// The source name.
        file: String,
        /// The resolved extension.
        extension: String,
    },
}

impl fmt::Display for FileRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MediaType { file, media_type } => write!(
                f,
                "Error validating \"{file}\". Cannot store files with the media type \"{media_type}\"."
            ),
            Self::Extension { file, extension } => write!(
                f,
                "Error validating \"{file}\". Cannot store files with the resolved extension \"{extension}\"."
            ),
        }
    }
}

fn owned(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| (*name).to_string()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A PDF and a JPEG pass the defaults; so does an `.exe` by extension,
    /// which upstream's default list names, but not by its media type.
    #[test]
    fn the_defaults_pass_documents_and_refuse_executables() {
        let defaults = FileValidation::default();
        assert!(
            defaults
                .refusals("report.pdf", "application/pdf", Some("pdf"))
                .is_empty()
        );
        assert!(
            defaults
                .refusals("photo.jpg", "image/jpeg", Some("jpg"))
                .is_empty()
        );
        assert_eq!(
            defaults.refusals("setup.exe", "application/x-dosexec", Some("exe")),
            vec![FileRefusal::MediaType {
                file: "setup.exe".to_string(),
                media_type: "application/x-dosexec".to_string(),
            }]
        );
    }

    /// `Validator::validate` checks the media type, then the extension, and
    /// keeps both messages.
    #[test]
    fn a_thumbnail_cache_is_refused_twice_in_upstream_order() {
        let refusals =
            FileValidation::default().refusals("Thumbs.db", "application/CDFV2", Some("db"));
        let messages: Vec<String> = refusals.iter().map(ToString::to_string).collect();
        assert_eq!(
            messages,
            [
                "Error validating \"Thumbs.db\". Cannot store files with the media type \"application/CDFV2\".",
                "Error validating \"Thumbs.db\". Cannot store files with the resolved extension \"db\".",
            ]
        );
    }

    /// A text file named `.ini` passes the media type and fails the
    /// extension; a file with no extension fails with an empty one.
    #[test]
    fn the_extension_is_checked_on_its_own() {
        let defaults = FileValidation::default();
        assert_eq!(
            defaults.refusals("desktop.ini", "text/plain", Some("ini")),
            vec![FileRefusal::Extension {
                file: "desktop.ini".to_string(),
                extension: "ini".to_string(),
            }]
        );
        assert_eq!(
            defaults.refusals("README", "text/plain", None),
            vec![FileRefusal::Extension {
                file: "README".to_string(),
                extension: String::new(),
            }]
        );
    }

    /// `disable_file_validation` passes anything; an empty list refuses
    /// everything.
    #[test]
    fn disabled_passes_and_an_empty_list_refuses() {
        assert!(
            FileValidation::Disabled
                .refusals("Thumbs.db", "application/CDFV2", Some("db"))
                .is_empty()
        );
        let empty = FileValidation::Allow {
            media_types: Vec::new(),
            extensions: Vec::new(),
        };
        assert_eq!(
            empty.refusals("photo.jpg", "image/jpeg", Some("jpg")).len(),
            2
        );
    }

    /// Upstream's list holds one entry with a stray comma, which refuses the
    /// type it names. Every entry here is a bare `type/subtype`.
    #[test]
    fn no_default_type_carries_a_stray_character() {
        for media_type in DEFAULT_MEDIA_TYPES {
            let (kind, subtype) = media_type
                .split_once('/')
                .unwrap_or_else(|| panic!("{media_type} is not a type"));
            assert!(!kind.is_empty() && !subtype.is_empty(), "{media_type}");
            assert!(
                subtype
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b".+-".contains(&byte)),
                "{media_type} carries a character a type does not"
            );
        }
    }

    /// The kinds this fork adds pass the defaults under the type the byte read
    /// answers and the extension the source carries. `audio.opus` is the Ogg
    /// container, which the detector names `audio/ogg`.
    #[test]
    fn the_defaults_accept_the_widened_kinds() {
        let defaults = FileValidation::default();
        for (name, media_type, extension) in [
            ("art.avif", "image/avif", "avif"),
            ("photo.heic", "image/heic", "heic"),
            ("drawing.svg", "image/svg+xml", "svg"),
            ("photo.psd", "image/vnd.adobe.photoshop", "psd"),
            ("plate.exr", "image/x-exr", "exr"),
            ("sky.hdr", "image/vnd.radiance", "hdr"),
            ("poster.eps", "application/postscript", "eps"),
            ("poster.eps", "image/x-eps", "eps"),
            ("artwork.ai", "application/illustrator", "ai"),
            ("page.ps", "application/postscript", "ps"),
            ("talk.flac", "audio/flac", "flac"),
            ("talk.opus", "audio/ogg", "opus"),
            ("clip.mkv", "video/x-matroska", "mkv"),
            ("clip.ogv", "video/ogg", "ogv"),
            ("scene.glb", "model/gltf-binary", "glb"),
            ("scene.gltf", "model/gltf+json", "gltf"),
            ("talk.vtt", "text/vtt", "vtt"),
            ("talk.srt", "application/x-subrip", "srt"),
            ("clip.3gp", "video/3gpp", "3gp"),
            ("clip.dv", "video/x-dv", "dv"),
            ("clip.flv", "video/x-flv", "flv"),
            ("clip.vob", "video/x-ms-vob", "vob"),
            ("reel.asf", "video/x-ms-asf", "asf"),
            ("talk.amr", "audio/amr", "amr"),
            ("talk.ape", "audio/x-ape", "ape"),
            ("talk.wma", "audio/x-ms-wma", "wma"),
            ("photo.nef", "image/x-nikon-nef", "nef"),
            ("photo.cr2", "image/x-canon-cr2", "cr2"),
            ("photo.cr3", "image/x-canon-cr3", "cr3"),
            ("photo.arw", "image/x-sony-arw", "arw"),
            ("photo.dng", "image/x-adobe-dng", "dng"),
            ("photo.orf", "image/x-olympus-orf", "orf"),
            ("photo.rw2", "image/x-panasonic-rw2", "rw2"),
            ("photo.raf", "image/x-fuji-raf", "raf"),
            ("photo.pef", "image/x-pentax-pef", "pef"),
            ("photo.srw", "image/x-samsung-srw", "srw"),
            ("photo.x3f", "image/x-sigma-x3f", "x3f"),
        ] {
            assert!(
                defaults
                    .refusals(name, media_type, Some(extension))
                    .is_empty(),
                "{name} was refused"
            );
        }
    }
}
