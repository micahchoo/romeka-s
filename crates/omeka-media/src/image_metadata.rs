//! Embedded metadata of an image: the tags the archive import offers.
//!
//! [`image_metadata`] is the one parse boundary. It takes a seekable reader
//! over an image and answers the few EXIF tags the import offers as value
//! sources, as strings, plus the pixel size. Anything it cannot read is absent
//! from the map, so a file with no EXIF, a truncated header or a format without
//! EXIF is an empty (or size-only) map, never an error: a bad tag must not stop
//! an import.
//!
//! It reads only the headers and directories, never the pixels, because an
//! archive master can be a 60 MB TIFF. A TIFF goes through the `tiff` decoder,
//! which seeks to each directory wherever the writer put it; `kamadak-exif`
//! would read a TIFF whole. Other formats go through `kamadak-exif`, which
//! stops at the EXIF segment, and a JPEG's size comes from its frame header,
//! because the `image` JPEG decoder also reads the whole file.

use std::collections::BTreeMap;
use std::io::{BufRead, Read, Seek, SeekFrom};

use exif::{Exif, In, Reader, Tag, Value};
use tiff::decoder::{Decoder as TiffDecoder, ifd::Value as TiffValue};
use tiff::tags::Tag as TiffTag;

/// Tags read as written: a string field, trimmed of the padding EXIF adds.
/// `DateTimeOriginal` lives in the EXIF directory, the rest in the first.
const TEXT_TAGS: [(&str, Tag); 5] = [
    ("Artist", Tag::Artist),
    ("Copyright", Tag::Copyright),
    ("DateTimeOriginal", Tag::DateTimeOriginal),
    ("ImageDescription", Tag::ImageDescription),
    ("Model", Tag::Model),
];

/// The tags the import offers for one image, by tag name.
///
/// Keys are `Artist`, `Copyright`, `DateTimeOriginal` (as written,
/// `YYYY:MM:DD hh:mm:ss`), `ImageDescription`, `Model`, `GPS` (`19.07 N,
/// 72.88 E`), and `Width` and `Height` in pixels. A key is present only when
/// the image holds a non-empty value for it.
#[must_use]
pub fn image_metadata<R: BufRead + Seek>(reader: &mut R) -> BTreeMap<&'static str, String> {
    let mut tags = BTreeMap::new();
    let Ok(head) = reader.fill_buf() else {
        return tags;
    };
    let is_tiff = matches!(
        head.get(..4),
        Some(b"II*\0" | b"MM\0*" | b"II+\0" | b"MM\0+")
    );
    let is_jpeg = head.starts_with(&[0xFF, 0xD8]);
    let size = if is_tiff {
        tiff_metadata(reader, &mut tags)
    } else {
        if let Ok(exif) = Reader::new().read_from_container(reader) {
            exif_metadata(&exif, &mut tags);
        }
        if reader.seek(SeekFrom::Start(0)).is_err() {
            None
        } else if is_jpeg {
            jpeg_size(reader)
        } else {
            header_size(reader)
        }
    };
    if let Some((width, height)) = size {
        tags.insert("Width", width.to_string());
        tags.insert("Height", height.to_string());
    }
    tags
}

fn exif_metadata(exif: &Exif, tags: &mut BTreeMap<&'static str, String>) {
    for (name, tag) in TEXT_TAGS {
        if let Some(text) = text_field(exif, tag) {
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
        tags.insert("GPS", gps);
    }
}

/// The text tags, the position and the size of a TIFF, read directory by
/// directory. A TIFF the decoder refuses has no tags.
fn tiff_metadata<R: Read + Seek>(
    reader: R,
    tags: &mut BTreeMap<&'static str, String>,
) -> Option<(u32, u32)> {
    let mut decoder = TiffDecoder::new(reader).ok()?;
    for (name, tag) in TEXT_TAGS {
        let tag = TiffTag::from_u16_exhaustive(tag.number());
        if let Some(text) = decoder.find_tag(tag).ok().flatten().and_then(tiff_text) {
            tags.insert(name, text);
        }
    }
    let size = decoder.dimensions().ok();
    if let Some(text) =
        sub_directory_tag(&mut decoder, TiffTag::ExifDirectory, Tag::DateTimeOriginal)
            .and_then(tiff_text)
    {
        tags.insert("DateTimeOriginal", text);
    }
    let mut gps = |tag: Tag| sub_directory_tag(&mut decoder, TiffTag::GpsDirectory, tag);
    let letter = |value: Option<TiffValue>| tiff_text(value?)?.bytes().next();
    let position = Position {
        latitude: gps(Tag::GPSLatitude).and_then(tiff_rationals),
        latitude_ref: letter(gps(Tag::GPSLatitudeRef)),
        longitude: gps(Tag::GPSLongitude).and_then(tiff_rationals),
        longitude_ref: letter(gps(Tag::GPSLongitudeRef)),
    };
    if let Some(text) = position.text() {
        tags.insert("GPS", text);
    }
    size
}

/// The value of `tag` in the directory the first directory's `pointer` names.
fn sub_directory_tag<R: Read + Seek>(
    decoder: &mut TiffDecoder<R>,
    pointer: TiffTag,
    tag: Tag,
) -> Option<TiffValue> {
    let at = decoder.find_tag(pointer).ok()??.into_ifd_pointer().ok()?;
    let directory = decoder.read_directory(at).ok()?;
    decoder
        .read_directory_tags(&directory)
        .find_tag(TiffTag::from_u16_exhaustive(tag.number()))
        .ok()?
}

fn tiff_text(value: TiffValue) -> Option<String> {
    let text = value.into_string().ok()?;
    let parts: Vec<&str> = text.split('\0').filter(|part| !part.is_empty()).collect();
    clean(&parts.join(" "))
}

fn tiff_rationals(value: TiffValue) -> Option<Vec<f64>> {
    let TiffValue::List(parts) = value else {
        return None;
    };
    parts
        .into_iter()
        .map(|part| match part {
            TiffValue::Rational(num, denom) => Some(f64::from(num) / f64::from(denom)),
            _ => None,
        })
        .collect()
}

/// A JPEG's size from its first frame header, walking the marker segments
/// by their lengths so the entropy-coded data is never read.
fn jpeg_size<R: Read + Seek>(reader: &mut R) -> Option<(u32, u32)> {
    let mut two = [0_u8; 2];
    reader.read_exact(&mut two).ok()?;
    loop {
        reader.read_exact(&mut two).ok()?;
        if two[0] != 0xFF {
            return None;
        }
        let mut marker = two[1];
        while marker == 0xFF {
            reader.read_exact(&mut two[..1]).ok()?;
            marker = two[0];
        }
        if matches!(marker, 0x01 | 0xD0..=0xD7) {
            continue;
        }
        if matches!(marker, 0xD9 | 0xDA) {
            return None;
        }
        reader.read_exact(&mut two).ok()?;
        let length = u16::from_be_bytes(two);
        if matches!(marker, 0xC0..=0xCF) && !matches!(marker, 0xC4 | 0xC8 | 0xCC) {
            let mut frame = [0_u8; 5];
            reader.read_exact(&mut frame).ok()?;
            let height = u16::from_be_bytes([frame[1], frame[2]]);
            let width = u16::from_be_bytes([frame[3], frame[4]]);
            return Some((u32::from(width), u32::from(height)));
        }
        reader
            .seek(SeekFrom::Current(i64::from(length.checked_sub(2)?)))
            .ok()?;
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

fn text_field(exif: &Exif, tag: Tag) -> Option<String> {
    let field = exif.get_field(tag, In::PRIMARY)?;
    let Value::Ascii(parts) = &field.value else {
        return None;
    };
    let joined = parts
        .iter()
        .map(|part| String::from_utf8_lossy(part))
        .collect::<Vec<_>>()
        .join(" ");
    clean(&joined)
}

fn clean(text: &str) -> Option<String> {
    let text = text.trim_matches(|c: char| c == '\0' || c.is_whitespace());
    (!text.is_empty()).then(|| text.to_string())
}

/// The GPS fields as read, whichever reader found them.
struct Position {
    latitude: Option<Vec<f64>>,
    latitude_ref: Option<u8>,
    longitude: Option<Vec<f64>>,
    longitude_ref: Option<u8>,
}

impl Position {
    /// `19.07 N, 72.88 E`: decimal degrees to two places and the hemisphere letter.
    fn text(&self) -> Option<String> {
        let latitude = coordinate(self.latitude.as_deref()?, self.latitude_ref?, 90.0)?;
        let longitude = coordinate(self.longitude.as_deref()?, self.longitude_ref?, 180.0)?;
        Some(format!("{latitude}, {longitude}"))
    }
}

fn coordinate(dms: &[f64], letter: u8, limit: f64) -> Option<String> {
    let [degrees, minutes, seconds] = dms else {
        return None;
    };
    let degrees = degrees + minutes / 60.0 + seconds / 3600.0;
    if !degrees.is_finite() || degrees > limit {
        return None;
    }
    let letter = match letter {
        b @ (b'N' | b'S' | b'E' | b'W') => char::from(b),
        _ => return None,
    };
    Some(format!("{degrees:.2} {letter}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use exif::experimental::Writer;
    use exif::{Field, Rational};
    use std::io::{BufReader, Cursor};

    fn read(bytes: &[u8]) -> BTreeMap<&'static str, String> {
        image_metadata(&mut Cursor::new(bytes))
    }

    /// A source that counts the bytes it hands out, as a file would be read.
    struct Counted {
        inner: Cursor<Vec<u8>>,
        read: u64,
    }

    impl Read for Counted {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            let n = self.inner.read(buf)?;
            self.read += n as u64;
            Ok(n)
        }
    }

    impl Seek for Counted {
        fn seek(&mut self, to: SeekFrom) -> std::io::Result<u64> {
            self.inner.seek(to)
        }
    }

    /// The tags of `bytes` read through an 8 KiB buffer, and the bytes the
    /// source handed out.
    fn read_counted(bytes: Vec<u8>) -> (BTreeMap<&'static str, String>, u64) {
        let mut reader = BufReader::new(Counted {
            inner: Cursor::new(bytes),
            read: 0,
        });
        let tags = image_metadata(&mut reader);
        (tags, reader.into_inner().read)
    }

    fn ascii(tag: Tag, text: &str) -> Field {
        Field {
            tag,
            ifd_num: In::PRIMARY,
            value: Value::Ascii(vec![text.as_bytes().to_vec()]),
        }
    }

    fn dms(tag: Tag, degrees: u32, minutes: u32, seconds: u32) -> Field {
        let rational = |num| Rational { num, denom: 1 };
        Field {
            tag,
            ifd_num: In::PRIMARY,
            value: Value::Rational(vec![
                rational(degrees),
                rational(minutes),
                rational(seconds),
            ]),
        }
    }

    /// The fields the tests write: 19 deg 4 min 12 s N is 19.07, and
    /// 72 deg 52 min 48 s E is 72.88.
    fn written_fields() -> Vec<Field> {
        vec![
            ascii(Tag::Artist, "Asha Rao"),
            ascii(Tag::Copyright, "CC BY 4.0"),
            ascii(Tag::DateTimeOriginal, "2024:05:06 07:08:09"),
            ascii(Tag::ImageDescription, "A harbour at dawn"),
            ascii(Tag::Model, "Nikon F3"),
            ascii(Tag::GPSLatitudeRef, "N"),
            dms(Tag::GPSLatitude, 19, 4, 12),
            ascii(Tag::GPSLongitudeRef, "E"),
            dms(Tag::GPSLongitude, 72, 52, 48),
        ]
    }

    fn exif_block(fields: &[Field]) -> Vec<u8> {
        let mut writer = Writer::new();
        for field in fields {
            writer.push_field(field);
        }
        let mut block = Cursor::new(Vec::new());
        writer.write(&mut block, false).expect("write exif");
        block.into_inner()
    }

    fn pixels(width: u32, height: u32, format: image::ImageFormat) -> Vec<u8> {
        let image = image::RgbImage::from_fn(width, height, |x, y| {
            image::Rgb([(x % 256) as u8, (y % 256) as u8, 90])
        });
        let mut bytes = Vec::new();
        image::DynamicImage::ImageRgb8(image)
            .write_to(&mut Cursor::new(&mut bytes), format)
            .expect("encode");
        bytes
    }

    /// A JPEG with the EXIF block inserted as an APP1 segment after SOI.
    fn jpeg_with_exif(fields: &[Field]) -> Vec<u8> {
        let plain = pixels(64, 32, image::ImageFormat::Jpeg);
        let block = exif_block(fields);
        let length = u16::try_from(block.len() + 8).expect("small block");
        let mut bytes = plain[..2].to_vec();
        bytes.extend([0xFF, 0xE1]);
        bytes.extend(length.to_be_bytes());
        bytes.extend(b"Exif\0\0");
        bytes.extend(&block);
        bytes.extend(&plain[2..]);
        bytes
    }

    /// An uncompressed RGB TIFF whose primary IFD carries the given fields
    /// besides the layout tags.
    fn tiff_with_exif(width: u32, height: u32, extra: &[Field]) -> Vec<u8> {
        let raw = vec![200_u8; (width * height * 3) as usize];
        let strips: [&[u8]; 1] = [&raw];
        let long = |tag, value| Field {
            tag,
            ifd_num: In::PRIMARY,
            value: Value::Long(vec![value]),
        };
        let short = |tag, values: Vec<u16>| Field {
            tag,
            ifd_num: In::PRIMARY,
            value: Value::Short(values),
        };
        let layout = [
            long(Tag::ImageWidth, width),
            long(Tag::ImageLength, height),
            short(Tag::BitsPerSample, vec![8, 8, 8]),
            short(Tag::Compression, vec![1]),
            short(Tag::PhotometricInterpretation, vec![2]),
            short(Tag::SamplesPerPixel, vec![3]),
            long(Tag::RowsPerStrip, height),
            long(Tag::StripByteCounts, raw.len() as u32),
            long(Tag::StripOffsets, 0),
        ];
        let mut writer = Writer::new();
        for field in layout.iter().chain(extra) {
            writer.push_field(field);
        }
        writer.set_strips(&strips, In::PRIMARY);
        let mut out = Cursor::new(Vec::new());
        writer.write(&mut out, false).expect("write tiff");
        out.into_inner()
    }

    #[test]
    fn a_jpeg_yields_the_written_tags_and_its_size() {
        let tags = read(&jpeg_with_exif(&written_fields()));
        assert_eq!(tags["Artist"], "Asha Rao");
        assert_eq!(tags["Copyright"], "CC BY 4.0");
        assert_eq!(tags["DateTimeOriginal"], "2024:05:06 07:08:09");
        assert_eq!(tags["ImageDescription"], "A harbour at dawn");
        assert_eq!(tags["Model"], "Nikon F3");
        assert_eq!(tags["GPS"], "19.07 N, 72.88 E");
        assert_eq!(
            (tags["Width"].as_str(), tags["Height"].as_str()),
            ("64", "32")
        );
    }

    #[test]
    fn a_southern_western_position_keeps_its_letters_and_loses_the_sign() {
        let fields = [
            ascii(Tag::GPSLatitudeRef, "S"),
            dms(Tag::GPSLatitude, 33, 51, 0),
            ascii(Tag::GPSLongitudeRef, "W"),
            dms(Tag::GPSLongitude, 151, 12, 0),
        ];
        let tags = read(&jpeg_with_exif(&fields));
        assert_eq!(tags["GPS"], "33.85 S, 151.20 W");
    }

    #[test]
    fn a_tiff_yields_the_tags_written_into_it() {
        let bytes = tiff_with_exif(
            16,
            8,
            &[
                ascii(Tag::Artist, "Asha Rao"),
                ascii(Tag::Model, "Epson V600"),
            ],
        );
        let tags = read(&bytes);
        assert_eq!(tags["Artist"], "Asha Rao");
        assert_eq!(tags["Model"], "Epson V600");
        assert_eq!(
            (tags["Width"].as_str(), tags["Height"].as_str()),
            ("16", "8")
        );
    }

    #[test]
    fn a_tiff_and_a_jpeg_with_the_same_fields_give_the_same_tags() {
        let from_jpeg = read(&jpeg_with_exif(&written_fields()));
        let from_tiff = read(&tiff_with_exif(64, 32, &written_fields()));
        assert_eq!(from_jpeg.len(), 8, "{from_jpeg:?}");
        assert_eq!(from_tiff, from_jpeg);
    }

    #[test]
    fn an_image_without_exif_has_only_its_size() {
        let tags = read(&pixels(10, 6, image::ImageFormat::Jpeg));
        assert_eq!(
            tags.into_iter().collect::<Vec<_>>(),
            [("Height", "6".to_string()), ("Width", "10".to_string())]
        );
    }

    #[test]
    fn bytes_that_are_not_an_image_give_an_empty_map() {
        assert!(read(b"not an image").is_empty());
        assert!(read(&[]).is_empty());
        let truncated = jpeg_with_exif(&written_fields());
        assert!(read(&truncated[..12]).is_empty());
    }

    /// 2400 x 1800 RGB is 12.96 MB of pixels. The bound is what the headers
    /// and directories need, far below the pixel data.
    const LARGE: (u32, u32) = (2400, 1800);
    const BOUND: u64 = 64 * 1024;

    #[test]
    fn a_large_tiff_with_exif_before_its_pixels_is_read_only_as_far_as_its_tags() {
        let bytes = tiff_with_exif(
            LARGE.0,
            LARGE.1,
            &[
                ascii(Tag::Artist, "Asha Rao"),
                ascii(Tag::Model, "Epson V600"),
            ],
        );
        let size = bytes.len() as u64;
        let (tags, read) = read_counted(bytes);
        assert_eq!(tags["Artist"], "Asha Rao");
        assert_eq!(tags["Model"], "Epson V600");
        assert_eq!(
            (tags["Width"].as_str(), tags["Height"].as_str()),
            ("2400", "1800")
        );
        assert!(read < BOUND, "read {read} of {size} bytes");
    }

    #[test]
    fn a_large_tiff_whose_directory_follows_its_pixels_is_read_only_at_both_ends() {
        let bytes = pixels(LARGE.0, LARGE.1, image::ImageFormat::Tiff);
        let size = bytes.len() as u64;
        let (tags, read) = read_counted(bytes);
        assert_eq!(
            (tags["Width"].as_str(), tags["Height"].as_str()),
            ("2400", "1800")
        );
        assert!(read < BOUND, "read {read} of {size} bytes");
    }

    #[test]
    fn a_large_jpeg_is_read_only_as_far_as_its_frame_header() {
        let mut bytes = jpeg_with_exif(&written_fields());
        bytes.extend(vec![0_u8; 4_000_000]);
        let size = bytes.len() as u64;
        let (tags, read) = read_counted(bytes);
        assert_eq!(tags["Artist"], "Asha Rao");
        assert_eq!(
            (tags["Width"].as_str(), tags["Height"].as_str()),
            ("64", "32")
        );
        assert!(read < BOUND, "read {read} of {size} bytes");
    }

    #[test]
    fn a_half_written_position_gives_no_gps() {
        let fields = [
            ascii(Tag::GPSLatitudeRef, "N"),
            dms(Tag::GPSLatitude, 19, 4, 12),
        ];
        let tags = read(&jpeg_with_exif(&fields));
        assert!(!tags.contains_key("GPS"));
    }
}
