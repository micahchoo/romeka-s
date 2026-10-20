use super::*;
use exif::experimental::Writer;
use exif::{Field, Rational};
use std::io::{BufReader, Cursor};

/// Each tag's values joined, for the tests that read one value a tag.
fn texts(tags: Metadata) -> BTreeMap<&'static str, String> {
    tags.into_iter()
        .map(|(key, embedded)| (key, embedded.values.join("; ")))
        .collect()
}

fn read(bytes: &[u8]) -> BTreeMap<&'static str, String> {
    texts(image_metadata(&mut Cursor::new(bytes)))
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
    let tags = texts(image_metadata(&mut reader));
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
/// 72 deg 52 min 48 s E is 72.88, both exactly.
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
    assert_eq!(tags["GPS"], "19.070000, 72.880000");
    assert_eq!(
        (tags["Width"].as_str(), tags["Height"].as_str()),
        ("64", "32")
    );
}

#[test]
fn a59_a_southern_western_position_is_negative_to_six_places() {
    let fields = [
        ascii(Tag::GPSLatitudeRef, "S"),
        dms(Tag::GPSLatitude, 33, 51, 0),
        ascii(Tag::GPSLongitudeRef, "W"),
        dms(Tag::GPSLongitude, 151, 12, 0),
    ];
    let tags = read(&jpeg_with_exif(&fields));
    assert_eq!(tags["GPS"], "-33.850000, -151.200000");
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

fn raw_ascii(tag: Tag, bytes: &[u8]) -> Field {
    Field {
        tag,
        ifd_num: In::PRIMARY,
        value: Value::Ascii(vec![bytes.to_vec()]),
    }
}

fn undefined(number: u16, bytes: &[u8]) -> Field {
    Field {
        tag: Tag(exif::Context::Tiff, number),
        ifd_num: In::PRIMARY,
        value: Value::Undefined(bytes.to_vec(), 0),
    }
}

/// A JPEG with `segments` (marker, payload) inserted after SOI.
fn jpeg_with_segments(segments: &[(u8, Vec<u8>)]) -> Vec<u8> {
    let plain = pixels(64, 32, image::ImageFormat::Jpeg);
    let mut bytes = plain[..2].to_vec();
    for (marker, payload) in segments {
        bytes.extend([0xFF, *marker]);
        bytes.extend(u16::try_from(payload.len() + 2).unwrap().to_be_bytes());
        bytes.extend(payload);
    }
    bytes.extend(&plain[2..]);
    bytes
}

/// IIM record 2 datasets.
fn iim(datasets: &[(u8, &[u8])]) -> Vec<u8> {
    let mut block = vec![0x1C, 1, 90, 0, 3, 0x1B, 0x25, 0x47];
    for (number, value) in datasets {
        block.extend([0x1C, 2, *number]);
        block.extend(u16::try_from(value.len()).unwrap().to_be_bytes());
        block.extend(*value);
    }
    block
}

/// A Photoshop resource block holding `iim` as resource 0x0404.
fn photoshop(iim: &[u8]) -> Vec<u8> {
    let mut segment = b"Photoshop 3.0\0".to_vec();
    segment.extend(b"8BIM\x03\xED\0\0");
    segment.extend([0, 0, 0, 2, 0, 0]);
    segment.extend(b"8BIM\x04\x04\0\0");
    segment.extend(u32::try_from(iim.len()).unwrap().to_be_bytes());
    segment.extend(iim);
    if iim.len() % 2 == 1 {
        segment.push(0);
    }
    segment
}

const IIM: [(u8, &[u8]); 9] = [
    (80, b"Homai Vyarawalla"),
    (80, b"R. Mehta"),
    (25, b"harbour"),
    (25, "मुंबई".as_bytes()),
    (25, b"ships"),
    (120, b"Ships at Ballard Pier"),
    (5, b"BHC_101"),
    (55, b"19500304"),
    (101, b"India"),
];

const XMP: &str = r#"<?xpacket begin="" id="W5M0MpCehiHzreSzNTczkc9d"?>
<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
<rdf:Description rdf:about="" xmlns:dc="http://purl.org/dc/elements/1.1/"
  xmlns:ps="http://ns.adobe.com/photoshop/1.0/" xmlns:lr="http://ns.adobe.com/lightroom/1.0/"
  ps:DateCreated="1950-03-04">
 <dc:title><rdf:Alt><rdf:li xml:lang="mr">बंदर</rdf:li><rdf:li xml:lang="x-default">The harbour</rdf:li></rdf:Alt></dc:title>
 <dc:description><rdf:Alt><rdf:li xml:lang="x-default">Ships &amp; boats</rdf:li></rdf:Alt></dc:description>
 <dc:creator><rdf:Seq><rdf:li>Homai Vyarawalla</rdf:li><rdf:li>R. Mehta</rdf:li></rdf:Seq></dc:creator>
 <dc:subject><rdf:Bag><rdf:li>harbour</rdf:li><rdf:li>मुंबई</rdf:li><rdf:li>ships</rdf:li></rdf:Bag></dc:subject>
 <dc:rights><rdf:Alt><rdf:li xml:lang="en">CC BY 4.0</rdf:li></rdf:Alt></dc:rights>
 <lr:hierarchicalSubject><rdf:Bag><rdf:li>Places|India|Mumbai</rdf:li><rdf:li>Subjects|Ships</rdf:li></rdf:Bag></lr:hierarchicalSubject>
</rdf:Description></rdf:RDF></x:xmpmeta><?xpacket end="w"?>"#;

fn values(tags: &Metadata, key: &str) -> Vec<String> {
    tags.get(key)
        .map(|tag| tag.values.clone())
        .unwrap_or_default()
}

fn assert_iptc(tags: &Metadata) {
    assert_eq!(
        values(tags, "IPTC By-line"),
        ["Homai Vyarawalla", "R. Mehta"]
    );
    assert_eq!(values(tags, "IPTC Keywords"), ["harbour", "मुंबई", "ships"]);
    assert_eq!(
        values(tags, "IPTC Caption-Abstract"),
        ["Ships at Ballard Pier"]
    );
    assert_eq!(values(tags, "IPTC Object Name"), ["BHC_101"]);
    assert_eq!(values(tags, "IPTC Date Created"), ["1950-03-04"]);
    assert_eq!(values(tags, "IPTC Country"), ["India"]);
    assert!(!tags.contains_key("IPTC City"));
}

fn assert_xmp(tags: &Metadata) {
    assert_eq!(
        values(tags, "XMP dc:title"),
        ["The harbour"],
        "x-default wins"
    );
    assert_eq!(values(tags, "XMP dc:description"), ["Ships & boats"]);
    assert_eq!(
        values(tags, "XMP dc:creator"),
        ["Homai Vyarawalla", "R. Mehta"]
    );
    assert_eq!(values(tags, "XMP dc:subject"), ["harbour", "मुंबई", "ships"]);
    assert_eq!(
        values(tags, "XMP dc:rights"),
        ["CC BY 4.0"],
        "else the first"
    );
    assert_eq!(values(tags, "XMP photoshop:DateCreated"), ["1950-03-04"]);
    assert_eq!(
        values(tags, "XMP lr:hierarchicalSubject"),
        ["Places|India|Mumbai", "Subjects|Ships"]
    );
    assert!(tags.values().all(|tag| !tag.windows_1252));
}

#[test]
fn a57_a_jpeg_gives_its_iptc_datasets_repeated_where_iim_repeats_them() {
    let bytes = jpeg_with_segments(&[(0xED, photoshop(&iim(&IIM)))]);
    let tags = image_metadata(&mut Cursor::new(bytes));
    assert_iptc(&tags);
    assert_eq!(values(&tags, "Width"), ["64"]);
}

#[test]
fn a57_a_jpeg_gives_its_xmp_properties_by_namespace() {
    let mut packet = b"http://ns.adobe.com/xap/1.0/\0".to_vec();
    packet.extend(XMP.as_bytes());
    let bytes = jpeg_with_segments(&[(0xE1, packet)]);
    assert_xmp(&image_metadata(&mut Cursor::new(bytes)));
}

#[test]
fn a57_a_tiff_gives_iptc_and_xmp_from_its_first_directory() {
    let bytes = tiff_with_exif(
        16,
        8,
        &[undefined(33723, &iim(&IIM)), undefined(700, XMP.as_bytes())],
    );
    let tags = image_metadata(&mut Cursor::new(bytes));
    assert_iptc(&tags);
    assert_xmp(&tags);
    assert_eq!(values(&tags, "Width"), ["16"]);
}

#[test]
fn a57_a_png_gives_the_xmp_of_its_adobe_itxt_chunk() {
    let plain = pixels(8, 4, image::ImageFormat::Png);
    let mut data = b"XML:com.adobe.xmp\0\0\0\0\0".to_vec();
    data.extend(XMP.as_bytes());
    let mut chunk = u32::try_from(data.len()).unwrap().to_be_bytes().to_vec();
    chunk.extend(b"iTXt");
    chunk.extend(&data);
    chunk.extend([0, 0, 0, 0]);
    let mut bytes = plain[..33].to_vec();
    bytes.extend(chunk);
    bytes.extend(&plain[33..]);
    let tags = image_metadata(&mut Cursor::new(bytes));
    assert_xmp(&tags);
    assert_eq!(values(&tags, "Height"), ["4"]);
}

#[test]
fn a58_exif_text_that_is_not_utf8_is_read_as_1252_and_marked() {
    let fields = [
        raw_ascii(Tag::Artist, b"Andr\xE9 Kert\xE9sz"),
        raw_ascii(Tag::Copyright, "© Asha Rao".as_bytes()),
    ];
    for bytes in [jpeg_with_exif(&fields), tiff_with_exif(16, 8, &fields)] {
        let tags = image_metadata(&mut Cursor::new(bytes));
        assert_eq!(
            tags["Artist"],
            Embedded {
                values: vec!["André Kertész".to_string()],
                windows_1252: true
            }
        );
        assert_eq!(values(&tags, "Copyright"), ["© Asha Rao"]);
        assert!(!tags["Copyright"].windows_1252);
    }
}

#[test]
fn a58_iptc_and_xmp_text_that_is_not_utf8_is_read_as_1252_and_marked() {
    let iptc = iim(&[(80, b"Andr\xE9")]);
    let mut packet = b"http://ns.adobe.com/xap/1.0/\0".to_vec();
    packet.extend(
        XMP.replace("R. Mehta", "Ren\u{e9}")
            .replace("मुंबई", "x")
            .as_bytes(),
    );
    let at = packet
        .windows(4)
        .position(|w| w == "Ren\u{e9}".as_bytes().get(..4).unwrap())
        .unwrap();
    packet.splice(at + 3..at + 5, [0xE9]);
    let bytes = jpeg_with_segments(&[(0xED, photoshop(&iptc)), (0xE1, packet)]);
    let tags = image_metadata(&mut Cursor::new(bytes));
    assert_eq!(values(&tags, "IPTC By-line"), ["André"]);
    assert!(tags["IPTC By-line"].windows_1252);
    assert_eq!(
        values(&tags, "XMP dc:creator"),
        ["Homai Vyarawalla", "René"]
    );
    assert!(tags["XMP dc:creator"].windows_1252);
}

#[test]
fn a59_gps_keeps_six_places() {
    let seconds = Field {
        tag: Tag::GPSLatitude,
        ifd_num: In::PRIMARY,
        value: Value::Rational(vec![
            Rational { num: 19, denom: 1 },
            Rational { num: 4, denom: 1 },
            Rational {
                num: 123_456,
                denom: 10_000,
            },
        ]),
    };
    let fields = [
        ascii(Tag::GPSLatitudeRef, "N"),
        seconds,
        ascii(Tag::GPSLongitudeRef, "E"),
        dms(Tag::GPSLongitude, 72, 52, 48),
    ];
    assert_eq!(
        read(&jpeg_with_exif(&fields))["GPS"],
        "19.070096, 72.880000"
    );
    assert_eq!(
        read(&tiff_with_exif(4, 4, &fields))["GPS"],
        "19.070096, 72.880000"
    );
}

#[test]
fn a_large_jpeg_with_iptc_and_xmp_is_read_only_as_far_as_its_scan() {
    let mut packet = b"http://ns.adobe.com/xap/1.0/\0".to_vec();
    packet.extend(XMP.as_bytes());
    let mut bytes = jpeg_with_segments(&[(0xED, photoshop(&iim(&IIM))), (0xE1, packet)]);
    bytes.extend(vec![0_u8; 4_000_000]);
    let size = bytes.len() as u64;
    let (tags, read) = read_counted(bytes);
    assert_eq!(tags["IPTC Keywords"], "harbour; मुंबई; ships");
    assert_eq!(tags["XMP dc:title"], "The harbour");
    assert!(read < BOUND, "read {read} of {size} bytes");
}
