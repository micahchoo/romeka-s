//! Embedded metadata of an image: the tags the archive import offers.
//!
//! [`image_metadata`] is the one parse boundary. It takes a seekable reader
//! over an image and answers, by key, the EXIF, IPTC and XMP tags the import
//! offers as value sources, plus the pixel size. Anything it cannot read is
//! absent from the map, so a file with no metadata, a truncated header or a
//! format without metadata is an empty (or size-only) map, never an error:
//! a bad tag must not stop an import.
//!
//! It reads only the headers and directories, never the pixels, because an
//! archive master can be a 60 MB TIFF. A TIFF's directories are read here as
//! raw entries (`tiff_ifd`), by seeking; a JPEG's segments are walked by their
//! lengths up to the scan; other formats' EXIF goes through `kamadak-exif`,
//! which stops at the EXIF segment.
//!
//! Text that is not UTF-8 is read as Windows-1252, the encoding of the old
//! Windows tools that wrote it, and its [`Embedded::windows_1252`] is set so
//! the import can flag it.

mod iptc;
#[cfg(test)]
mod tests;
mod tiff_ifd;
mod xmp;

use std::collections::BTreeMap;
use std::io::{BufRead, Read, Seek, SeekFrom};

use encoding_rs::WINDOWS_1252;
use exif::{Exif, In, Reader, Tag, Value};

/// One tag: its values in file order (more than one only where the standard
/// repeats it, as IPTC Keywords and XMP bags do), and whether its text was
/// not UTF-8 and was read as Windows-1252.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Embedded {
    pub values: Vec<String>,
    pub windows_1252: bool,
}

impl Embedded {
    fn one(text: String, windows_1252: bool) -> Self {
        Self {
            values: vec![text],
            windows_1252,
        }
    }
}

/// The tags of one image, by key.
///
/// Keys: `Artist`, `Copyright`, `DateTimeOriginal` (as written, `YYYY:MM:DD
/// hh:mm:ss`), `ImageDescription`, `Model`, `GPS` (`19.070000, 72.880000`,
/// signed decimal degrees), `Width` and `Height`; `IPTC Object Name`, `IPTC
/// Keywords`, `IPTC Date Created` (ISO), `IPTC By-line`, `IPTC City`, `IPTC
/// Country`, `IPTC Caption-Abstract`; `XMP dc:title`, `XMP dc:description`,
/// `XMP dc:creator`, `XMP dc:subject`, `XMP dc:rights`, `XMP
/// photoshop:DateCreated` (as written), `XMP lr:hierarchicalSubject`. A key is
/// present only when the image holds a non-empty value for it.
pub type Metadata = BTreeMap<&'static str, Embedded>;

/// EXIF text tags: key, tag. `DateTimeOriginal` lives in the EXIF
/// directory, the rest in the first.
const TEXT_TAGS: [(&str, Tag); 5] = [
    ("Artist", Tag::Artist),
    ("Copyright", Tag::Copyright),
    ("DateTimeOriginal", Tag::DateTimeOriginal),
    ("ImageDescription", Tag::ImageDescription),
    ("Model", Tag::Model),
];

const TIFF_IPTC: u16 = 33723;
const TIFF_PHOTOSHOP: u16 = 34377;
const TIFF_XMP: u16 = 700;
const XMP_SIGNATURE: &[u8] = b"http://ns.adobe.com/xap/1.0/\0";
const PHOTOSHOP_SIGNATURE: &[u8] = b"Photoshop 3.0\0";

#[must_use]
pub fn image_metadata<R: BufRead + Seek>(reader: &mut R) -> Metadata {
    let mut tags = Metadata::new();
    let Ok(head) = reader.fill_buf() else {
        return tags;
    };
    let is_tiff = matches!(
        head.get(..4),
        Some(b"II*\0" | b"MM\0*" | b"II+\0" | b"MM\0+")
    );
    let is_jpeg = head.starts_with(&[0xFF, 0xD8]);
    let is_png = head.starts_with(b"\x89PNG\r\n\x1a\n");
    let size = if is_tiff {
        tiff_metadata(reader, &mut tags)
    } else {
        if let Ok(exif) = Reader::new().read_from_container(reader) {
            exif_metadata(&exif, &mut tags);
        }
        if reader.seek(SeekFrom::Start(0)).is_err() {
            None
        } else if is_jpeg {
            jpeg_segments(reader, &mut tags)
        } else {
            if is_png {
                png_xmp(reader, &mut tags);
            }
            reader
                .seek(SeekFrom::Start(0))
                .ok()
                .and_then(|_| header_size(reader))
        }
    };
    if let Some((width, height)) = size {
        tags.insert("Width", Embedded::one(width.to_string(), false));
        tags.insert("Height", Embedded::one(height.to_string(), false));
    }
    tags
}

/// Text as written, trimmed of the NUL padding and spaces writers add:
/// UTF-8 when it is, else Windows-1252 and `true`. `None` when empty.
fn decode_text(bytes: &[u8]) -> Option<(String, bool)> {
    let (text, windows_1252) = match std::str::from_utf8(bytes) {
        Ok(text) => (std::borrow::Cow::Borrowed(text), false),
        Err(_) => (WINDOWS_1252.decode_without_bom_handling(bytes).0, true),
    };
    let text = text.trim_matches(|c: char| c == '\0' || c.is_whitespace());
    (!text.is_empty()).then(|| (text.to_string(), windows_1252))
}

/// An EXIF ASCII field's parts, NUL-separated, joined by a space.
fn ascii(parts: &[&[u8]]) -> Option<Embedded> {
    let parts: Vec<&[u8]> = parts
        .iter()
        .flat_map(|part| part.split(|byte| *byte == 0))
        .filter(|part| !part.is_empty())
        .collect();
    let (text, windows_1252) = decode_text(&parts.join(&b' '))?;
    Some(Embedded::one(text, windows_1252))
}

fn exif_metadata(exif: &Exif, tags: &mut Metadata) {
    for (name, tag) in TEXT_TAGS {
        let Some(Value::Ascii(parts)) = exif.get_field(tag, In::PRIMARY).map(|field| &field.value)
        else {
            continue;
        };
        let parts: Vec<&[u8]> = parts.iter().map(Vec::as_slice).collect();
        if let Some(text) = ascii(&parts) {
            tags.insert(name, text);
        }
    }
    let rationals = |tag| match &exif.get_field(tag, In::PRIMARY)?.value {
        Value::Rational(parts) => Some(parts.iter().map(exif::Rational::to_f64).collect()),
        _ => None,
    };
    let letter = |tag| match &exif.get_field(tag, In::PRIMARY)?.value {
        Value::Ascii(parts) => parts.first()?.first().copied(),
        _ => None,
    };
    let position = Position {
        latitude: rationals(Tag::GPSLatitude),
        latitude_ref: letter(Tag::GPSLatitudeRef),
        longitude: rationals(Tag::GPSLongitude),
        longitude_ref: letter(Tag::GPSLongitudeRef),
    };
    if let Some(gps) = position.text() {
        tags.insert("GPS", Embedded::one(gps, false));
    }
}

/// The tags and the size of a TIFF, read directory by directory.
fn tiff_metadata<R: Read + Seek>(reader: R, tags: &mut Metadata) -> Option<(u32, u32)> {
    use tiff_ifd::{Entry, Tiff, find};
    fn text<R: Read + Seek>(tiff: &mut Tiff<R>, entries: &[Entry], tag: Tag) -> Option<Embedded> {
        let bytes = tiff.bytes(find(entries, tag.number())?)?;
        ascii(&[&bytes])
    }
    let mut tiff = Tiff::open(reader)?;
    let first = tiff.first()?;
    for (name, tag) in TEXT_TAGS {
        if let Some(value) = text(&mut tiff, &first, tag) {
            tags.insert(name, value);
        }
    }
    if let Some(exif) = tiff.directory_of(find(&first, Tag::ExifIFDPointer.number()))
        && let Some(value) = text(&mut tiff, &exif, Tag::DateTimeOriginal)
    {
        tags.insert("DateTimeOriginal", value);
    }
    if let Some(gps) = tiff.directory_of(find(&first, Tag::GPSInfoIFDPointer.number())) {
        let get = |tag: Tag| find(&gps, tag.number());
        let mut letter = |entry: Option<Entry>| tiff.bytes(entry?)?.first().copied();
        let latitude_ref = letter(get(Tag::GPSLatitudeRef));
        let longitude_ref = letter(get(Tag::GPSLongitudeRef));
        let position = Position {
            latitude: get(Tag::GPSLatitude).and_then(|entry| tiff.rationals(entry)),
            latitude_ref,
            longitude: get(Tag::GPSLongitude).and_then(|entry| tiff.rationals(entry)),
            longitude_ref,
        };
        if let Some(gps) = position.text() {
            tags.insert("GPS", Embedded::one(gps, false));
        }
    }
    let mut raw = |tag: u16| find(&first, tag).and_then(|entry| tiff.bytes(entry));
    match (raw(TIFF_IPTC), raw(TIFF_PHOTOSHOP)) {
        (Some(block), _) => iptc::read(&block, tags),
        (None, Some(resources)) => {
            iptc::read(iptc::from_resources(&resources).unwrap_or_default(), tags);
        }
        (None, None) => {}
    }
    if let Some(packet) = raw(TIFF_XMP) {
        xmp::read(&packet, tags);
    }
    let mut dimension = |tag: Tag| {
        let value = *tiff.unsigned(find(&first, tag.number())?)?.first()?;
        u32::try_from(value).ok()
    };
    Some((dimension(Tag::ImageWidth)?, dimension(Tag::ImageLength)?))
}

/// A JPEG's IPTC and XMP segments and its size from its frame header,
/// walking the marker segments by their lengths up to the scan, so the
/// entropy-coded data is never read.
fn jpeg_segments<R: Read>(reader: &mut R, tags: &mut Metadata) -> Option<(u32, u32)> {
    let mut size = None;
    let mut resources = Vec::new();
    let mut two = [0_u8; 2];
    reader.read_exact(&mut two).ok()?;
    loop {
        if reader.read_exact(&mut two).is_err() || two[0] != 0xFF {
            break;
        }
        let mut marker = two[1];
        while marker == 0xFF {
            if reader.read_exact(&mut two[..1]).is_err() {
                break;
            }
            marker = two[0];
        }
        if matches!(marker, 0x01 | 0xD0..=0xD7) {
            continue;
        }
        if matches!(marker, 0xD9 | 0xDA) || reader.read_exact(&mut two).is_err() {
            break;
        }
        let Some(length) = u16::from_be_bytes(two).checked_sub(2) else {
            break;
        };
        let is_frame = matches!(marker, 0xC0..=0xCF) && !matches!(marker, 0xC4 | 0xC8 | 0xCC);
        if is_frame || matches!(marker, 0xE1 | 0xED) {
            let mut segment = vec![0_u8; usize::from(length)];
            if reader.read_exact(&mut segment).is_err() {
                break;
            }
            if is_frame {
                if size.is_none() && segment.len() >= 5 {
                    let height = u16::from_be_bytes([segment[1], segment[2]]);
                    let width = u16::from_be_bytes([segment[3], segment[4]]);
                    size = Some((u32::from(width), u32::from(height)));
                }
            } else if let Some(packet) = segment.strip_prefix(XMP_SIGNATURE) {
                xmp::read(packet, tags);
            } else if let Some(block) = segment.strip_prefix(PHOTOSHOP_SIGNATURE) {
                resources.extend_from_slice(block);
            }
        } else if std::io::copy(&mut reader.take(u64::from(length)), &mut std::io::sink()).is_err()
        {
            // A segment is at most 64 KB, so it is read past, not sought
            // past: a seek would drop the reader's buffer each time.
            break;
        }
    }
    if let Some(block) = iptc::from_resources(&resources) {
        iptc::read(block, tags);
    }
    size
}

/// A PNG's XMP packet, from the `iTXt` chunk Adobe names. Chunks are
/// skipped by their lengths; only that chunk is read.
fn png_xmp<R: Read + Seek>(reader: &mut R, tags: &mut Metadata) {
    const KEYWORD: &[u8] = b"XML:com.adobe.xmp\0";
    const MAX_CHUNK: u32 = 1024 * 1024;
    if reader.seek(SeekFrom::Start(8)).is_err() {
        return;
    }
    let mut head = [0_u8; 8];
    while reader.read_exact(&mut head).is_ok() {
        let length = u32::from_be_bytes([head[0], head[1], head[2], head[3]]);
        let kind = &head[4..8];
        if kind == b"IEND" {
            return;
        }
        if kind == b"iTXt" && length <= MAX_CHUNK {
            let mut chunk = vec![0_u8; length as usize];
            if reader.read_exact(&mut chunk).is_err() {
                return;
            }
            // After the keyword: an uncompressed flag (0), the method, then
            // the language and the translated keyword, each NUL-ended.
            if let Some([0, _, rest @ ..]) = chunk.strip_prefix(KEYWORD)
                && let Some(text) = rest.splitn(3, |byte| *byte == 0).nth(2)
            {
                xmp::read(text, tags);
                return;
            }
            if reader.seek(SeekFrom::Current(4)).is_err() {
                return;
            }
        } else if reader
            .seek(SeekFrom::Current(i64::from(length) + 4))
            .is_err()
        {
            return;
        }
    }
}

/// Pixel size from the image header alone, for the formats whose `image`
/// decoder reads only the header to answer it.
fn header_size<R: BufRead + Seek>(reader: &mut R) -> Option<(u32, u32)> {
    image::ImageReader::new(reader)
        .with_guessed_format()
        .ok()?
        .into_dimensions()
        .ok()
}

/// The GPS fields as read, whichever reader found them.
struct Position {
    latitude: Option<Vec<f64>>,
    latitude_ref: Option<u8>,
    longitude: Option<Vec<f64>>,
    longitude_ref: Option<u8>,
}

impl Position {
    /// `19.070000, 72.880000`: signed decimal degrees to 6 places, about
    /// 11 cm, south and west negative (`docs/import-v2.md`, "GPS").
    fn text(&self) -> Option<String> {
        let latitude = coordinate(self.latitude.as_deref()?, self.latitude_ref?, *b"NS", 90.0)?;
        let longitude = coordinate(
            self.longitude.as_deref()?,
            self.longitude_ref?,
            *b"EW",
            180.0,
        )?;
        Some(format!("{latitude:.6}, {longitude:.6}"))
    }
}

/// Degrees from degrees, minutes and seconds, negative for the second of
/// the two hemisphere letters.
fn coordinate(dms: &[f64], letter: u8, [plus, minus]: [u8; 2], limit: f64) -> Option<f64> {
    let [degrees, minutes, seconds] = dms else {
        return None;
    };
    let degrees = degrees + minutes / 60.0 + seconds / 3600.0;
    if !degrees.is_finite() || degrees > limit {
        return None;
    }
    match letter {
        _ if letter == plus => Some(degrees),
        _ if letter == minus => Some(-degrees),
        _ => None,
    }
}
