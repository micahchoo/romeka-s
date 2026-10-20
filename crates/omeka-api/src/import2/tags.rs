//! File tags and file facts: the values a file offers to a slot
//! (`docs/import-v2.md`, `SourceRef` `tag` and `file`).
//!
//! File facts and the file tags (`File name`, `Folder`, `Size`, `Format`)
//! come from the evidence index and never open the file. EXIF, IPTC and XMP
//! tags and `Pixel dimensions` read the file's headers through
//! `omeka_media::image_metadata`, which seeks and never loads the pixels.
//! [`TagCache`] reads each file once per (path, modified time, size), the
//! three the index already holds, so a cached answer costs no system call.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::SystemTime;

use super::archive::SourceDir;
use super::evidence::FileEntry;
pub use omeka_media::image_metadata::{Embedded, Metadata};

use super::plan::{FileFact, TagId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagKind {
    /// Read from inside the file, from its EXIF block.
    Exif,
    /// Read from inside the file, from its IPTC block.
    Iptc,
    /// Read from inside the file, from its XMP packet.
    Xmp,
    /// A fact about the file.
    File,
}

impl TagKind {
    /// The name the wire gives the kind.
    #[must_use]
    pub const fn wire(self) -> &'static str {
        match self {
            Self::Exif => "exif",
            Self::Iptc => "iptc",
            Self::Xmp => "xmp",
            Self::File => "file",
        }
    }

    /// Whether the tag is read from inside the file.
    #[must_use]
    pub const fn is_embedded(self) -> bool {
        !matches!(self, Self::File)
    }
}

/// The tags the Sources tab lists, in its order.
pub const TAGS: [(TagId, TagKind); 25] = [
    (TagId::DateTimeOriginal, TagKind::Exif),
    (TagId::Artist, TagKind::Exif),
    (TagId::Copyright, TagKind::Exif),
    (TagId::ImageDescription, TagKind::Exif),
    (TagId::Gps, TagKind::Exif),
    (TagId::Model, TagKind::Exif),
    (TagId::IptcByline, TagKind::Iptc),
    (TagId::IptcCaption, TagKind::Iptc),
    (TagId::IptcKeywords, TagKind::Iptc),
    (TagId::IptcObjectName, TagKind::Iptc),
    (TagId::IptcDateCreated, TagKind::Iptc),
    (TagId::IptcCity, TagKind::Iptc),
    (TagId::IptcCountry, TagKind::Iptc),
    (TagId::XmpTitle, TagKind::Xmp),
    (TagId::XmpDescription, TagKind::Xmp),
    (TagId::XmpCreator, TagKind::Xmp),
    (TagId::XmpSubject, TagKind::Xmp),
    (TagId::XmpRights, TagKind::Xmp),
    (TagId::XmpDateCreated, TagKind::Xmp),
    (TagId::XmpHierarchicalSubject, TagKind::Xmp),
    (TagId::FileName, TagKind::File),
    (TagId::Folder, TagKind::File),
    (TagId::Size, TagKind::File),
    (TagId::PixelDimensions, TagKind::File),
    (TagId::Format, TagKind::File),
];

/// The `image_metadata` key of a tag read from inside the file.
fn embedded_key(tag: TagId) -> Option<&'static str> {
    Some(match tag {
        TagId::DateTimeOriginal => "DateTimeOriginal",
        TagId::Artist => "Artist",
        TagId::Copyright => "Copyright",
        TagId::ImageDescription => "ImageDescription",
        TagId::Gps => "GPS",
        TagId::Model => "Model",
        TagId::IptcByline => "IPTC By-line",
        TagId::IptcCaption => "IPTC Caption-Abstract",
        TagId::IptcKeywords => "IPTC Keywords",
        TagId::IptcObjectName => "IPTC Object Name",
        TagId::IptcDateCreated => "IPTC Date Created",
        TagId::IptcCity => "IPTC City",
        TagId::IptcCountry => "IPTC Country",
        TagId::XmpTitle => "XMP dc:title",
        TagId::XmpDescription => "XMP dc:description",
        TagId::XmpCreator => "XMP dc:creator",
        TagId::XmpSubject => "XMP dc:subject",
        TagId::XmpRights => "XMP dc:rights",
        TagId::XmpDateCreated => "XMP photoshop:DateCreated",
        TagId::XmpHierarchicalSubject => "XMP lr:hierarchicalSubject",
        TagId::FileName | TagId::Folder | TagId::Size | TagId::PixelDimensions | TagId::Format => {
            return None;
        }
    })
}

/// A file fact. `size` is the byte count; `format` is the sniffed media
/// type, never a guess from the extension.
#[must_use]
pub fn file_fact(entry: &FileEntry, fact: FileFact) -> String {
    match fact {
        FileFact::Name => entry.name().to_string(),
        FileFact::Path => entry.path.to_string(),
        FileFact::Size => entry.size.to_string(),
        FileFact::Format => entry.sniffed.clone(),
    }
}

/// Reads the embedded metadata of one file, keyed as
/// `omeka_media::image_metadata` keys it. Tests supply their own.
pub trait TagReader: Send + Sync {
    fn read(&self, file: &Path) -> Metadata;
}

/// Reads from disk, as far as the headers. An unreadable file has no tags.
#[derive(Debug, Clone, Copy, Default)]
pub struct FsTagReader;

impl TagReader for FsTagReader {
    fn read(&self, file: &Path) -> Metadata {
        std::fs::File::open(file)
            .map(|handle| omeka_media::image_metadata(&mut std::io::BufReader::new(handle)))
            .unwrap_or_default()
    }
}

/// The most files a cache remembers before it starts over.
const CAPACITY: usize = 100_000;

type FileKey = (PathBuf, Option<SystemTime>, u64);
type Read = Arc<Metadata>;

/// Tag values of files, each file read once while its index entry stays
/// the same. Shared across previews, so asking again does not reread a
/// large TIFF.
pub struct TagCache<R = FsTagReader> {
    reader: R,
    seen: Mutex<HashMap<FileKey, Read>>,
}

impl Default for TagCache {
    fn default() -> Self {
        Self::new(FsTagReader)
    }
}

impl<R: TagReader> TagCache<R> {
    pub fn new(reader: R) -> Self {
        Self {
            reader,
            seen: Mutex::new(HashMap::new()),
        }
    }

    /// The value of `tag` for `entry`, `None` when the file has none. A
    /// tag that repeats (IPTC Keywords, an XMP bag) gives its values joined
    /// by `; `; [`TagCache::tag`] gives them apart.
    #[must_use]
    pub fn value(&self, source: &SourceDir, entry: &FileEntry, tag: TagId) -> Option<String> {
        Some(self.tag(source, entry, tag)?.values.join("; "))
    }

    /// The values of `tag` for `entry`, apart, and whether the file's text
    /// was read as Windows-1252 (`encoding-suspect`). `None` when the file
    /// has no such tag; a file fact has one value and is never suspect.
    #[must_use]
    pub fn tag(&self, source: &SourceDir, entry: &FileEntry, tag: TagId) -> Option<Embedded> {
        let fact = match tag {
            TagId::FileName => entry.name().to_string(),
            TagId::Folder => entry.folder().to_string(),
            TagId::Size => human_size(entry.size),
            TagId::Format => entry.sniffed.clone(),
            TagId::PixelDimensions => {
                let tags = self.read(source, entry);
                let one = |key| tags.get(key)?.values.first().cloned();
                format!("{} × {}", one("Width")?, one("Height")?)
            }
            _ => {
                let mut found = self.embedded(source, entry, tag)?;
                found.values.retain(|value| !value.is_empty());
                return (!found.values.is_empty()).then_some(found);
            }
        };
        (!fact.is_empty()).then(|| Embedded {
            values: vec![fact],
            windows_1252: false,
        })
    }

    /// The values of a tag read from inside the file, apart, and whether
    /// its text was read as Windows-1252 (`encoding-suspect`). `None` for
    /// a file fact and when the file has no such tag.
    #[must_use]
    pub fn embedded(&self, source: &SourceDir, entry: &FileEntry, tag: TagId) -> Option<Embedded> {
        self.read(source, entry).get(embedded_key(tag)?).cloned()
    }

    fn read(&self, source: &SourceDir, entry: &FileEntry) -> Read {
        let disk = entry.disk_path();
        let key = (source.dir().join(&disk), entry.modified, entry.size);
        let seen = || self.seen.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(found) = seen().get(&key) {
            return Arc::clone(found);
        }
        let tags = Arc::new(
            source
                .locate(&disk)
                .map(|file| self.reader.read(&file))
                .unwrap_or_default(),
        );
        let mut seen = seen();
        if seen.len() >= CAPACITY {
            seen.clear();
        }
        seen.insert(key, Arc::clone(&tags));
        tags
    }
}

/// Decimal units, as a file manager shows them: `812 B`, `12 KB`, `4.1 MB`.
#[must_use]
pub fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["KB", "MB", "GB", "TB"];
    if bytes < 1000 {
        return format!("{bytes} B");
    }
    #[allow(clippy::cast_precision_loss)]
    let mut value = bytes as f64 / 1000.0;
    let mut unit = 0;
    while value >= 1000.0 && unit < UNITS.len() - 1 {
        value /= 1000.0;
        unit += 1;
    }
    if value < 10.0 {
        format!("{value:.1} {}", UNITS[unit])
    } else {
        format!("{value:.0} {}", UNITS[unit])
    }
}

/// `2019:05:14 11:02:10` as the ISO date `2019-05-14`. `None` for text that
/// is not an EXIF date, and for the all-zero date cameras write when they
/// do not know.
#[must_use]
pub fn exif_date_to_iso(value: &str) -> Option<String> {
    let date = value.get(..10)?;
    let shape = date.bytes().enumerate().all(|(index, byte)| match index {
        4 | 7 => byte == b':',
        _ => byte.is_ascii_digit(),
    });
    if !shape || date.starts_with("0000") {
        return None;
    }
    Some(date.replace(':', "-"))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;
    use crate::import2::evidence::walk;

    struct Counting(AtomicUsize);

    fn one(text: &str) -> Embedded {
        Embedded {
            values: vec![text.to_string()],
            windows_1252: false,
        }
    }

    impl TagReader for Counting {
        fn read(&self, _: &Path) -> Metadata {
            self.0.fetch_add(1, Ordering::SeqCst);
            Metadata::from([
                ("Artist", one("R. Mehta")),
                ("Width", one("7200")),
                ("Height", one("5400")),
                (
                    "IPTC Keywords",
                    Embedded {
                        values: vec!["harbour".into(), "मुंबई".into()],
                        windows_1252: false,
                    },
                ),
                (
                    "XMP dc:creator",
                    Embedded {
                        values: vec!["André".into()],
                        windows_1252: true,
                    },
                ),
            ])
        }
    }

    fn scanned(bytes: &[u8]) -> (tempfile::TempDir, SourceDir, FileEntry) {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(dir.path().join("Photographs/1950s")).expect("mkdir");
        std::fs::write(dir.path().join("Photographs/1950s/BHC_101.jpg"), bytes).expect("write");
        let source = SourceDir::standalone(dir.path()).expect("source");
        let entry = walk(&source).expect("walk").entries.remove(0);
        (dir, source, entry)
    }

    #[test]
    fn a57_the_listed_tags_are_the_contracts_six_exif_seven_iptc_seven_xmp_and_five_facts() {
        let of = |wanted| TAGS.iter().filter(|(_, kind)| *kind == wanted).count();
        assert_eq!(TAGS.len(), 25);
        assert_eq!(
            [
                of(TagKind::Exif),
                of(TagKind::Iptc),
                of(TagKind::Xmp),
                of(TagKind::File)
            ],
            [6, 7, 7, 5]
        );
        for (tag, kind) in TAGS {
            assert_eq!(embedded_key(tag).is_some(), kind.is_embedded(), "{tag:?}");
            let name = serde_json::to_value(tag).expect("name");
            if let Some(key) = embedded_key(tag) {
                assert_eq!(name, key, "a tag is named as the reader keys it");
            }
            assert_eq!(serde_json::from_value::<TagId>(name).expect("parse"), tag);
        }
        let kinds: Vec<_> = TAGS.iter().map(|(_, kind)| kind.wire()).collect();
        assert_eq!(kinds[0], "exif");
        assert_eq!(kinds[6], "iptc");
        assert_eq!(kinds[13], "xmp");
        assert_eq!(kinds[24], "file");
    }

    #[test]
    fn a57_a_repeating_tag_gives_its_values_apart_or_joined_and_a58_marks_1252() {
        let (_dir, source, entry) = scanned(b"\x89PNG\r\n\x1a\n");
        let cache = TagCache::new(Counting(AtomicUsize::new(0)));
        let keywords = cache
            .embedded(&source, &entry, TagId::IptcKeywords)
            .expect("keywords");
        assert_eq!(keywords.values, ["harbour", "मुंबई"]);
        assert_eq!(
            cache.value(&source, &entry, TagId::IptcKeywords).as_deref(),
            Some("harbour; मुंबई")
        );
        let creator = cache
            .embedded(&source, &entry, TagId::XmpCreator)
            .expect("creator");
        assert!(creator.windows_1252);
        assert_eq!(cache.embedded(&source, &entry, TagId::IptcCity), None);
        assert_eq!(cache.embedded(&source, &entry, TagId::FileName), None);
        assert_eq!(cache.reader.0.load(Ordering::SeqCst), 1, "opened once");
    }

    #[test]
    fn a60_file_facts_and_tags_come_from_the_index_and_the_format_is_sniffed() {
        let (_dir, source, entry) = scanned(b"\x89PNG\r\n\x1a\n");
        let cache = TagCache::new(Counting(AtomicUsize::new(0)));
        assert_eq!(file_fact(&entry, FileFact::Name), "BHC_101.jpg");
        assert_eq!(
            file_fact(&entry, FileFact::Path),
            "Photographs/1950s/BHC_101.jpg"
        );
        assert_eq!(file_fact(&entry, FileFact::Size), "8");
        assert_eq!(file_fact(&entry, FileFact::Format), "image/png");
        let tag = |tag| cache.value(&source, &entry, tag);
        assert_eq!(tag(TagId::FileName).as_deref(), Some("BHC_101.jpg"));
        assert_eq!(tag(TagId::Folder).as_deref(), Some("Photographs/1950s"));
        assert_eq!(tag(TagId::Size).as_deref(), Some("8 B"));
        assert_eq!(tag(TagId::Format).as_deref(), Some("image/png"));
        assert_eq!(cache.reader.0.load(Ordering::SeqCst), 0, "no file opened");
        assert_eq!(tag(TagId::Artist).as_deref(), Some("R. Mehta"));
        assert_eq!(tag(TagId::PixelDimensions).as_deref(), Some("7200 × 5400"));
        assert_eq!(tag(TagId::Model), None);
        assert_eq!(cache.reader.0.load(Ordering::SeqCst), 1, "opened once");
    }

    #[test]
    fn a_file_is_read_again_only_when_its_index_entry_changes() {
        let (_dir, source, entry) = scanned(b"\x89PNG\r\n\x1a\n");
        let cache = TagCache::new(Counting(AtomicUsize::new(0)));
        let _ = cache.value(&source, &entry, TagId::Artist);
        let _ = cache.value(&source, &entry, TagId::Artist);
        assert_eq!(cache.reader.0.load(Ordering::SeqCst), 1);
        let rescanned = FileEntry {
            size: entry.size + 1,
            ..entry.clone()
        };
        let _ = cache.value(&source, &rescanned, TagId::Artist);
        assert_eq!(cache.reader.0.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn the_disk_reader_reads_a_real_jpeg_and_a_missing_file_has_no_tags() {
        let pixels = image::RgbImage::from_pixel(16, 12, image::Rgb([90, 120, 150]));
        let mut bytes = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(pixels)
            .write_to(&mut bytes, image::ImageFormat::Jpeg)
            .expect("encode");
        let (dir, source, entry) = scanned(&bytes.into_inner());
        let cache = TagCache::default();
        assert_eq!(
            cache
                .value(&source, &entry, TagId::PixelDimensions)
                .as_deref(),
            Some("16 × 12")
        );
        std::fs::remove_file(dir.path().join(entry.path.as_str())).expect("rm");
        let gone = FileEntry { size: 1, ..entry };
        assert_eq!(cache.value(&source, &gone, TagId::Artist), None);
    }

    #[test]
    fn sizes_read_like_a_file_manager_and_exif_dates_become_iso() {
        assert_eq!(human_size(812), "812 B");
        assert_eq!(human_size(12_000), "12 KB");
        assert_eq!(human_size(4_100_000), "4.1 MB");
        assert_eq!(human_size(61_000_000), "61 MB");
        assert_eq!(
            exif_date_to_iso("2019:05:14 11:02:10").as_deref(),
            Some("2019-05-14")
        );
        assert_eq!(exif_date_to_iso("0000:00:00 00:00:00"), None);
        assert_eq!(exif_date_to_iso("14/05/2019"), None);
    }
}
