//! The tags a file offers as value sources: EXIF read from inside the file,
//! and facts about the file itself.
//!
//! [`TAGS`] is the catalogue, in the order the Sources tab lists it. Reading
//! a file is the slow part, so [`TagCache`] reads each file at most once and
//! only when a value asks for an EXIF tag or the pixel size. File name, Folder,
//! Size and Format never open the file.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use serde::Serialize;

use super::archive::ArchiveFile;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TagKind {
    Exif,
    File,
}

pub const DATE_TIME_ORIGINAL: &str = "DateTimeOriginal";
const PIXEL_DIMENSIONS: &str = "Pixel dimensions";

/// Every tag the import reads, in display order.
pub const TAGS: [(&str, TagKind); 11] = [
    (DATE_TIME_ORIGINAL, TagKind::Exif),
    ("Artist", TagKind::Exif),
    ("Copyright", TagKind::Exif),
    ("ImageDescription", TagKind::Exif),
    ("GPS", TagKind::Exif),
    ("Model", TagKind::Exif),
    ("File name", TagKind::File),
    ("Folder", TagKind::File),
    ("Size", TagKind::File),
    (PIXEL_DIMENSIONS, TagKind::File),
    ("Format", TagKind::File),
];

/// The kind of a tag the import reads, `None` for any other name.
#[must_use]
pub fn kind_of(tag: &str) -> Option<TagKind> {
    TAGS.iter()
        .find(|(name, _)| *name == tag)
        .map(|(_, kind)| *kind)
}

/// Reads the embedded metadata of one file, as `omeka_media::image_metadata`
/// answers it. Tests supply their own.
pub trait TagReader {
    fn read(&self, file: &ArchiveFile) -> BTreeMap<&'static str, String>;
}

/// Reads the file from disk, only as far as its headers and directories. An
/// unreadable file has no tags.
#[derive(Debug, Clone, Copy, Default)]
pub struct FsTagReader;

impl TagReader for FsTagReader {
    fn read(&self, file: &ArchiveFile) -> BTreeMap<&'static str, String> {
        std::fs::File::open(&file.absolute)
            .map(|handle| omeka_media::image_metadata(&mut std::io::BufReader::new(handle)))
            .unwrap_or_default()
    }
}

/// The most files a [`SharedTagReader`] remembers before it starts over.
const SHARED_CAPACITY: usize = 100_000;

type FileKey = (PathBuf, Option<SystemTime>, u64);

/// A disk reader that remembers each file's tags across previews, so the
/// Sources tab asking again does not reread large TIFFs. A file is the same
/// while its path, modification time and size are; a changed file is read
/// again. Clones share one memory.
#[derive(Debug, Clone, Default)]
pub struct SharedTagReader {
    seen: Arc<Mutex<HashMap<FileKey, BTreeMap<&'static str, String>>>>,
}

impl TagReader for SharedTagReader {
    fn read(&self, file: &ArchiveFile) -> BTreeMap<&'static str, String> {
        let Ok(metadata) = std::fs::metadata(&file.absolute) else {
            return BTreeMap::new();
        };
        let key = (
            file.absolute.clone(),
            metadata.modified().ok(),
            metadata.len(),
        );
        if let Some(found) = self
            .seen
            .lock()
            .ok()
            .and_then(|seen| seen.get(&key).cloned())
        {
            return found;
        }
        let tags = FsTagReader.read(file);
        if let Ok(mut seen) = self.seen.lock() {
            if seen.len() >= SHARED_CAPACITY {
                seen.clear();
            }
            seen.insert(key, tags.clone());
        }
        tags
    }
}

/// Tag values of files, each file read once.
pub struct TagCache<'r> {
    reader: &'r dyn TagReader,
    read: RefCell<HashMap<String, Rc<BTreeMap<&'static str, String>>>>,
}

impl<'r> TagCache<'r> {
    #[must_use]
    pub fn new(reader: &'r dyn TagReader) -> Self {
        Self {
            reader,
            read: RefCell::new(HashMap::new()),
        }
    }

    fn embedded(&self, file: &ArchiveFile) -> Rc<BTreeMap<&'static str, String>> {
        if let Some(found) = self.read.borrow().get(&file.path) {
            return Rc::clone(found);
        }
        let tags = Rc::new(self.reader.read(file));
        self.read
            .borrow_mut()
            .insert(file.path.clone(), Rc::clone(&tags));
        tags
    }

    /// The value of `tag` for `file`, `""` when the file has none.
    #[must_use]
    pub fn value(&self, file: &ArchiveFile, tag: &str) -> String {
        let (folder, name) = file.path.rsplit_once('/').unwrap_or(("", &file.path));
        match tag {
            "File name" => name.to_string(),
            "Folder" => folder.to_string(),
            "Size" => human_size(file.size),
            "Format" => format_of(name),
            PIXEL_DIMENSIONS => {
                let tags = self.embedded(file);
                match (tags.get("Width"), tags.get("Height")) {
                    (Some(width), Some(height)) => format!("{width} × {height}"),
                    _ => String::new(),
                }
            }
            _ => self.embedded(file).get(tag).cloned().unwrap_or_default(),
        }
    }
}

/// The format name of a file by its extension: `JPEG`, `TIFF`, or the
/// extension in capitals.
fn format_of(name: &str) -> String {
    let Some((_, extension)) = name.rsplit_once('.') else {
        return String::new();
    };
    match extension.to_ascii_lowercase().as_str() {
        "jpg" | "jpeg" => "JPEG".to_string(),
        "tif" | "tiff" => "TIFF".to_string(),
        "txt" => "Text".to_string(),
        other => other.to_ascii_uppercase(),
    }
}

/// Decimal units, as a file manager shows them: `812 B`, `12 KB`, `4.1 MB`,
/// `48 MB`.
fn human_size(bytes: u64) -> String {
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
    if value < 10.0 && unit > 0 {
        format!("{value:.1} {}", UNITS[unit])
    } else {
        format!("{value:.0} {}", UNITS[unit])
    }
}

/// `2019:05:14 11:02:10` as the ISO date `2019-05-14`. `None` for text that
/// is not an EXIF date, and for the all-zero date cameras write when they do
/// not know.
#[must_use]
pub fn exif_date_to_iso(value: &str) -> Option<String> {
    let date = value.get(..10)?;
    let bytes = date.as_bytes();
    let shape = bytes.iter().enumerate().all(|(index, byte)| match index {
        4 | 7 => *byte == b':',
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
    use super::*;
    use std::cell::Cell;
    use std::path::PathBuf;

    struct Counting(Cell<usize>);

    impl TagReader for Counting {
        fn read(&self, _: &ArchiveFile) -> BTreeMap<&'static str, String> {
            self.0.set(self.0.get() + 1);
            BTreeMap::from([
                ("Artist", "R. Mehta".to_string()),
                ("Width", "7200".to_string()),
                ("Height", "5400".to_string()),
            ])
        }
    }

    fn file(path: &str, size: u64) -> ArchiveFile {
        ArchiveFile {
            path: path.to_string(),
            size,
            absolute: PathBuf::from(path),
        }
    }

    #[test]
    fn file_facts_never_open_the_file_and_exif_opens_it_once() {
        let reader = Counting(Cell::new(0));
        let cache = TagCache::new(&reader);
        let tif = file("Photographs/1950s/BHC_006_PH_101.tif", 61_000_000);
        assert_eq!(cache.value(&tif, "File name"), "BHC_006_PH_101.tif");
        assert_eq!(cache.value(&tif, "Folder"), "Photographs/1950s");
        assert_eq!(cache.value(&tif, "Size"), "61 MB");
        assert_eq!(cache.value(&tif, "Format"), "TIFF");
        assert_eq!(reader.0.get(), 0);
        assert_eq!(cache.value(&tif, "Artist"), "R. Mehta");
        assert_eq!(cache.value(&tif, "Pixel dimensions"), "7200 × 5400");
        assert_eq!(cache.value(&tif, "Model"), "");
        assert_eq!(reader.0.get(), 1);
    }

    #[test]
    fn sizes_read_like_a_file_manager() {
        assert_eq!(human_size(812), "812 B");
        assert_eq!(human_size(12_000), "12 KB");
        assert_eq!(human_size(4_100_000), "4.1 MB");
        assert_eq!(human_size(48_000_000), "48 MB");
    }

    #[test]
    fn an_exif_date_becomes_an_iso_date() {
        assert_eq!(
            exif_date_to_iso("2019:05:14 11:02:10").as_deref(),
            Some("2019-05-14")
        );
        assert_eq!(exif_date_to_iso("0000:00:00 00:00:00"), None);
        assert_eq!(exif_date_to_iso("14/05/2019"), None);
    }

    #[test]
    fn the_disk_reader_answers_an_unreadable_file_with_no_tags() {
        assert!(FsTagReader.read(&file("/no/such/file.jpg", 0)).is_empty());
    }

    #[test]
    fn the_shared_reader_rereads_a_file_only_when_its_time_or_size_changes() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("photo.jpg");
        let pixels = image::RgbImage::from_pixel(16, 12, image::Rgb([90, 120, 150]));
        let mut bytes = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(pixels)
            .write_to(&mut bytes, image::ImageFormat::Jpeg)
            .expect("encode");
        let bytes = bytes.into_inner();
        std::fs::write(&path, &bytes).expect("write");
        let file = ArchiveFile {
            path: "photo.jpg".to_string(),
            size: bytes.len() as u64,
            absolute: path.clone(),
        };
        let reader = SharedTagReader::default();
        let first = reader.read(&file);
        assert_eq!(first.get("Width").map(String::as_str), Some("16"));

        let modified = std::fs::metadata(&path)
            .and_then(|m| m.modified())
            .expect("mtime");
        std::fs::write(&path, vec![0_u8; bytes.len()]).expect("same size, no image");
        let handle = std::fs::File::options()
            .write(true)
            .open(&path)
            .expect("open");
        handle.set_modified(modified).expect("keep the time");
        assert_eq!(
            reader.clone().read(&file),
            first,
            "an unchanged file is remembered"
        );

        handle
            .set_modified(modified + std::time::Duration::from_secs(60))
            .expect("touch");
        assert!(
            reader.read(&file).is_empty(),
            "a changed file is read again"
        );
    }
}
