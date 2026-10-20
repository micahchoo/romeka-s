//! Media type detection over tiny generated files, against upstream's answer.
//!
//! Each fixture is the smallest byte string that both libmagic and this crate
//! read as a real file of its kind. `EXPECTED` is what upstream's
//! `TempFile::getMediaType` (PHP `finfo`, then `MEDIA_TYPE_ALIASES`) answers
//! for the same bytes, measured on the oracle with `tests/oracle-finfo.sh`.
//! To re-measure, dump the fixtures with
//! `MEDIA_FIXTURE_DIR=<dir> cargo test -p omeka-media --test media_type -- --ignored`
//! and run the script over that folder.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::io::{Cursor, Write};

use omeka_media::media_type::detected_media_type;

/// The fixture name (its file name in the dump) and upstream's media type.
const EXPECTED: &[(&str, &str)] = &[
    ("bare.svg", "image/svg+xml"),
    ("empty.txt", "application/x-empty"),
    ("page.html", "text/html"),
    ("sample.avi", "video/x-msvideo"),
    ("sample.bin", "application/octet-stream"),
    ("sample.bmp", "image/bmp"),
    ("sample.doc", "application/msword"),
    (
        "sample.docx",
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
    ),
    ("sample.exe", "application/x-dosexec"),
    ("sample.flac", "audio/flac"),
    ("sample.gif", "image/gif"),
    ("sample.gz", "application/gzip"),
    ("sample.heic", "image/heic"),
    ("sample.jp2", "image/jp2"),
    ("sample.jpg", "image/jpeg"),
    ("sample.json", "application/json"),
    ("sample.m4a", "audio/mp4"),
    ("sample.mkv", "video/x-matroska"),
    ("sample.mov", "video/quicktime"),
    ("sample.mp3", "audio/mpeg"),
    ("sample.mp4", "video/mp4"),
    (
        "sample.ods",
        "application/vnd.oasis.opendocument.spreadsheet",
    ),
    ("sample.odt", "application/vnd.oasis.opendocument.text"),
    ("sample.ogg", "audio/ogg"),
    ("sample.pdf", "application/pdf"),
    ("sample.png", "image/png"),
    ("sample.ppt", "application/vnd.ms-powerpoint"),
    (
        "sample.pptx",
        "application/vnd.openxmlformats-officedocument.presentationml.presentation",
    ),
    ("sample.rtf", "application/rtf"),
    ("sample.svg", "image/svg+xml"),
    ("sample.tif", "image/tiff"),
    ("sample.txt", "text/plain"),
    ("sample.wav", "audio/x-wav"),
    ("sample.webm", "video/webm"),
    ("sample.webp", "image/webp"),
    ("sample.xls", "application/vnd.ms-excel"),
    (
        "sample.xlsx",
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
    ),
    ("sample.xml", "text/xml"),
    ("sample.zip", "application/zip"),
    ("Thumbs.db", "application/CDFV2"),
];

/// Every fixture, by the file name the dump writes it under.
fn fixtures() -> Vec<(&'static str, Vec<u8>)> {
    vec![
        ("sample.pdf", pdf()),
        ("sample.mp3", mp3()),
        ("sample.wav", wav()),
        ("sample.ogg", ogg()),
        ("sample.flac", flac()),
        ("sample.mp4", iso_bmff(b"isom", &[b"isom", b"mp41"])),
        ("sample.m4a", iso_bmff(b"M4A ", &[b"M4A ", b"mp42", b"isom"])),
        ("sample.mov", iso_bmff(b"qt  ", &[b"qt  "])),
        ("sample.heic", iso_bmff(b"heic", &[b"mif1", b"heic"])),
        ("sample.webm", ebml(b"webm")),
        ("sample.mkv", ebml(b"matroska")),
        ("sample.avi", avi()),
        ("sample.docx", ooxml("word/document.xml")),
        ("sample.xlsx", ooxml("xl/workbook.xml")),
        ("sample.pptx", ooxml("ppt/presentation.xml")),
        ("sample.odt", odf("application/vnd.oasis.opendocument.text")),
        (
            "sample.ods",
            odf("application/vnd.oasis.opendocument.spreadsheet"),
        ),
        (
            "sample.doc",
            compound("00020906-0000-0000-c000-000000000046", "Microsoft Office Word"),
        ),
        (
            "sample.xls",
            compound("00020820-0000-0000-c000-000000000046", "Microsoft Excel"),
        ),
        (
            "sample.ppt",
            compound(
                "64818d10-4f9b-11cf-86ea-00aa00b929e8",
                "Microsoft Office PowerPoint",
            ),
        ),
        ("Thumbs.db", thumbs_db()),
        ("sample.zip", zip(&[("notes.txt", b"plain notes\n")])),
        ("sample.gz", gzip()),
        ("sample.json", b"{\"title\": \"Example\", \"count\": 1}\n".to_vec()),
        (
            "sample.xml",
            b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<record><title>Example</title></record>\n"
                .to_vec(),
        ),
        (
            "sample.svg",
            b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1\" height=\"1\"></svg>\n"
                .to_vec(),
        ),
        ("sample.rtf", b"{\\rtf1\\ansi\\deff0 Hello}\n".to_vec()),
        ("sample.jp2", jp2()),
        ("sample.bmp", bmp()),
        (
            "sample.png",
            [b"\x89PNG\r\n\x1a\n".as_slice(), &[0, 0, 0, 13], b"IHDR"].concat(),
        ),
        ("sample.jpg", vec![0xFF, 0xD8, 0xFF, 0xE0, 0, 16, b'J', b'F', b'I', b'F', 0]),
        ("sample.gif", b"GIF89a\x01\x00\x01\x00\x00\x00\x00".to_vec()),
        ("sample.webp", b"RIFF\x1a\x00\x00\x00WEBPVP8 ".to_vec()),
        (
            "sample.tif",
            b"II*\x00\x08\x00\x00\x00\x00\x00\x00\x00\x00\x00".to_vec(),
        ),
        ("sample.txt", b"Plain text notes.\n".to_vec()),
        ("sample.exe", exe()),
        ("sample.bin", vec![0x00, 0x01, 0x02, 0x03, 0xFE, 0xFF]),
        ("empty.txt", Vec::new()),
        ("page.html", b"<html><body>x</body></html>\n".to_vec()),
        (
            "bare.svg",
            b"<svg xmlns=\"http://www.w3.org/2000/svg\"></svg>\n".to_vec(),
        ),
    ]
}

fn pdf() -> Vec<u8> {
    b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n1 0 obj\n<< /Type /Catalog >>\nendobj\ntrailer\n<< /Root 1 0 R >>\n%%EOF\n"
        .to_vec()
}

/// An ID3v2.3 tag followed by one MPEG-1 Layer III frame header.
fn mp3() -> Vec<u8> {
    let mut bytes = b"ID3\x03\x00\x00\x00\x00\x00\x00".to_vec();
    bytes.extend_from_slice(&[0xFF, 0xFB, 0x90, 0x64]);
    bytes.resize(bytes.len() + 413, 0);
    bytes
}

/// A RIFF/WAVE file: a PCM `fmt ` chunk and an empty `data` chunk.
fn wav() -> Vec<u8> {
    let mut body = b"WAVEfmt ".to_vec();
    body.extend_from_slice(&16u32.to_le_bytes());
    body.extend_from_slice(&[1, 0, 1, 0]);
    body.extend_from_slice(&44_100u32.to_le_bytes());
    body.extend_from_slice(&88_200u32.to_le_bytes());
    body.extend_from_slice(&[2, 0, 16, 0]);
    body.extend_from_slice(b"data\x00\x00\x00\x00");
    riff(body)
}

fn avi() -> Vec<u8> {
    let mut body = b"AVI LIST".to_vec();
    body.extend_from_slice(&4u32.to_le_bytes());
    body.extend_from_slice(b"hdrl");
    riff(body)
}

fn riff(body: Vec<u8>) -> Vec<u8> {
    let mut bytes = b"RIFF".to_vec();
    bytes.extend_from_slice(&u32::try_from(body.len()).unwrap().to_le_bytes());
    bytes.extend(body);
    bytes
}

/// The first Ogg page of a Vorbis stream: its identification header.
fn ogg() -> Vec<u8> {
    let mut bytes = b"OggS\x00\x02".to_vec();
    bytes.extend_from_slice(&[0; 8]);
    bytes.extend_from_slice(&1u32.to_le_bytes());
    bytes.extend_from_slice(&[0; 4]);
    bytes.extend_from_slice(&[0; 4]);
    bytes.extend_from_slice(&[1, 30]);
    bytes.extend_from_slice(b"\x01vorbis");
    bytes.extend_from_slice(&[0; 4]);
    bytes.push(2);
    bytes.extend_from_slice(&44_100u32.to_le_bytes());
    bytes.extend_from_slice(&[0; 12]);
    bytes.extend_from_slice(&[0xB8, 1]);
    bytes
}

/// `fLaC` and a STREAMINFO block for 44.1 kHz stereo 16-bit.
fn flac() -> Vec<u8> {
    let mut bytes = b"fLaC\x80\x00\x00\x22".to_vec();
    bytes.extend_from_slice(&[0x10, 0x00, 0x10, 0x00]);
    bytes.extend_from_slice(&[0; 6]);
    bytes.extend_from_slice(&[0x0A, 0xC4, 0x42, 0xF0, 0, 0, 0, 0]);
    bytes.extend_from_slice(&[0; 16]);
    bytes
}

/// An ISO base media file: one `ftyp` box with a major and compatible brands.
fn iso_bmff(major: &[u8; 4], compatible: &[&[u8; 4]]) -> Vec<u8> {
    let size = 16 + 4 * compatible.len();
    let mut bytes = u32::try_from(size).unwrap().to_be_bytes().to_vec();
    bytes.extend_from_slice(b"ftyp");
    bytes.extend_from_slice(major);
    bytes.extend_from_slice(&[0, 0, 2, 0]);
    for brand in compatible {
        bytes.extend_from_slice(*brand);
    }
    bytes.extend_from_slice(&[0, 0, 0, 8]);
    bytes.extend_from_slice(b"free");
    bytes
}

/// An EBML header naming a Matroska `DocType`.
fn ebml(doc_type: &[u8]) -> Vec<u8> {
    let mut body = vec![0x42, 0x86, 0x81, 0x01, 0x42, 0xF7, 0x81, 0x01];
    body.extend_from_slice(&[0x42, 0xF2, 0x81, 0x04, 0x42, 0xF3, 0x81, 0x08]);
    body.extend_from_slice(&[0x42, 0x82, 0x80 | u8::try_from(doc_type.len()).unwrap()]);
    body.extend_from_slice(doc_type);
    body.extend_from_slice(&[0x42, 0x87, 0x81, 0x04, 0x42, 0x85, 0x81, 0x02]);
    let mut bytes = vec![
        0x1A,
        0x45,
        0xDF,
        0xA3,
        0x80 | u8::try_from(body.len()).unwrap(),
    ];
    bytes.extend(body);
    // A Segment whose first child is a Void element, so the file is as long
    // as the shortest real one `infer` reads (more than 256 bytes).
    bytes.extend_from_slice(&[
        0x18, 0x53, 0x80, 0x67, 0x01, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    ]);
    bytes.extend_from_slice(&[0xEC, 0x41, 0x2C]);
    bytes.resize(bytes.len() + 300, 0);
    bytes
}

/// A JPEG 2000 signature box and its `ftyp` box.
fn jp2() -> Vec<u8> {
    let mut bytes = vec![0, 0, 0, 12];
    bytes.extend_from_slice(b"jP  \r\n\x87\n");
    bytes.extend_from_slice(&[0, 0, 0, 20]);
    bytes.extend_from_slice(b"ftypjp2 \x00\x00\x00\x00jp2 ");
    bytes
}

/// A one-pixel, 24-bit Windows bitmap.
fn bmp() -> Vec<u8> {
    let mut bytes = b"BM".to_vec();
    bytes.extend_from_slice(&58u32.to_le_bytes());
    bytes.extend_from_slice(&[0; 4]);
    bytes.extend_from_slice(&54u32.to_le_bytes());
    bytes.extend_from_slice(&40u32.to_le_bytes());
    bytes.extend_from_slice(&1i32.to_le_bytes());
    bytes.extend_from_slice(&1i32.to_le_bytes());
    bytes.extend_from_slice(&[1, 0, 24, 0]);
    bytes.extend_from_slice(&[0; 24]);
    bytes.extend_from_slice(&[0xFF, 0xFF, 0xFF, 0]);
    bytes
}

/// A DOS stub whose `e_lfanew` points at a PE header for an i386 image.
fn exe() -> Vec<u8> {
    let mut bytes = vec![0; 0x80];
    bytes[0] = b'M';
    bytes[1] = b'Z';
    bytes[0x3C] = 0x40;
    bytes[0x40..0x44].copy_from_slice(b"PE\x00\x00");
    bytes[0x44] = 0x4C;
    bytes[0x45] = 0x01;
    bytes
}

/// A property set stream with one section, the summary information, holding
/// one property: `PIDSI_APPNAME` (0x12) as a `VT_LPSTR`.
fn summary_information(application: &str) -> Vec<u8> {
    let mut text = application.as_bytes().to_vec();
    text.push(0);
    text.resize(text.len().next_multiple_of(4), 0);
    let mut property = 0x1Eu32.to_le_bytes().to_vec();
    property.extend_from_slice(&u32::try_from(application.len() + 1).unwrap().to_le_bytes());
    property.extend(text);
    let mut section = u32::try_from(16 + property.len())
        .unwrap()
        .to_le_bytes()
        .to_vec();
    section.extend_from_slice(&1u32.to_le_bytes());
    section.extend_from_slice(&0x12u32.to_le_bytes());
    section.extend_from_slice(&16u32.to_le_bytes());
    section.extend(property);
    let mut bytes = vec![0xFE, 0xFF, 0, 0, 0x05, 0x01, 0x02, 0x00];
    bytes.extend_from_slice(&[0; 16]);
    bytes.extend_from_slice(&1u32.to_le_bytes());
    let fmtid = uuid::Uuid::parse_str("f29f85e0-4ff9-1068-ab91-08002b27b3d9").unwrap();
    bytes.extend_from_slice(&fmtid.to_bytes_le());
    bytes.extend_from_slice(&48u32.to_le_bytes());
    bytes.extend(section);
    bytes
}

fn gzip() -> Vec<u8> {
    vec![
        0x1F, 0x8B, 0x08, 0x00, 0, 0, 0, 0, 0x00, 0x03, 0x03, 0x00, 0, 0, 0, 0, 0, 0, 0, 0,
    ]
}

/// An Office Open XML package: the content types first, as Office writes it.
fn ooxml(part: &str) -> Vec<u8> {
    zip(&[
        ("[Content_Types].xml", b"<?xml version=\"1.0\"?><Types/>"),
        ("_rels/.rels", b"<?xml version=\"1.0\"?><Relationships/>"),
        (part, b"<?xml version=\"1.0\"?><document/>"),
    ])
}

/// An OpenDocument package: the stored `mimetype` entry first.
fn odf(mimetype: &str) -> Vec<u8> {
    zip(&[
        ("mimetype", mimetype.as_bytes()),
        (
            "content.xml",
            b"<?xml version=\"1.0\"?><office:document-content/>",
        ),
    ])
}

/// A compound file whose root storage carries `clsid` and whose summary
/// information names the application that wrote it, as Office writes both.
fn compound(clsid: &str, application: &str) -> Vec<u8> {
    let mut file = cfb::CompoundFile::create(Cursor::new(Vec::new())).unwrap();
    file.set_storage_clsid("/", uuid::Uuid::parse_str(clsid).unwrap())
        .unwrap();
    file.create_stream("/\u{5}SummaryInformation")
        .unwrap()
        .write_all(&summary_information(application))
        .unwrap();
    file.flush().unwrap();
    file.into_inner().into_inner()
}

/// A Windows thumbnail cache: a compound file with a `Catalog` stream and no
/// summary information.
fn thumbs_db() -> Vec<u8> {
    let mut file = cfb::CompoundFile::create(Cursor::new(Vec::new())).unwrap();
    file.create_stream("/Catalog")
        .unwrap()
        .write_all(&[0x10, 0, 0x07, 0, 0, 0, 0, 0])
        .unwrap();
    file.flush().unwrap();
    file.into_inner().into_inner()
}

/// A zip archive of stored (uncompressed) entries.
fn zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut central = Vec::new();
    for (name, data) in entries {
        let offset = u32::try_from(bytes.len()).unwrap();
        let crc = crc32(data);
        let size = u32::try_from(data.len()).unwrap();
        let name_len = u16::try_from(name.len()).unwrap();
        let mut common = Vec::new();
        common.extend_from_slice(&[20, 0, 0, 0, 0, 0, 0, 0, 0x21, 0]);
        common.extend_from_slice(&crc.to_le_bytes());
        common.extend_from_slice(&size.to_le_bytes());
        common.extend_from_slice(&size.to_le_bytes());
        common.extend_from_slice(&name_len.to_le_bytes());
        common.extend_from_slice(&0u16.to_le_bytes());
        bytes.extend_from_slice(b"PK\x03\x04");
        bytes.extend_from_slice(&common);
        bytes.extend_from_slice(name.as_bytes());
        bytes.extend_from_slice(data);
        central.extend_from_slice(b"PK\x01\x02\x14\x00");
        central.extend_from_slice(&common);
        central.extend_from_slice(&[0; 10]);
        central.extend_from_slice(&offset.to_le_bytes());
        central.extend_from_slice(name.as_bytes());
    }
    let central_offset = u32::try_from(bytes.len()).unwrap();
    let central_size = u32::try_from(central.len()).unwrap();
    let count = u16::try_from(entries.len()).unwrap();
    bytes.extend(central);
    bytes.extend_from_slice(b"PK\x05\x06\x00\x00\x00\x00");
    bytes.extend_from_slice(&count.to_le_bytes());
    bytes.extend_from_slice(&count.to_le_bytes());
    bytes.extend_from_slice(&central_size.to_le_bytes());
    bytes.extend_from_slice(&central_offset.to_le_bytes());
    bytes.extend_from_slice(&[0, 0]);
    bytes
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = !0u32;
    for byte in data {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

/// Every fixture reads as the media type upstream's `finfo` gives the same
/// bytes, and every fixture has a measured answer.
#[test]
fn each_fixture_reads_as_upstream_reads_it() {
    let fixtures = fixtures();
    let mut wrong = Vec::new();
    for (name, bytes) in &fixtures {
        let Some((_, expected)) = EXPECTED.iter().find(|(known, _)| known == name) else {
            wrong.push(format!("{name}: no measured answer"));
            continue;
        };
        let detected = detected_media_type(bytes);
        if detected != *expected {
            wrong.push(format!("{name}: detected {detected}, upstream {expected}"));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    assert_eq!(
        EXPECTED.len(),
        fixtures.len(),
        "a measured answer has no fixture"
    );
}

/// Write each fixture under `MEDIA_FIXTURE_DIR`, for `tests/oracle-finfo.sh`.
#[test]
#[ignore = "writes the fixtures for an oracle measurement"]
fn dump_fixtures_for_the_oracle() {
    let dir = std::env::var("MEDIA_FIXTURE_DIR").expect("MEDIA_FIXTURE_DIR names a folder");
    std::fs::create_dir_all(&dir).unwrap();
    for (name, bytes) in fixtures() {
        std::fs::write(std::path::Path::new(&dir).join(name), bytes).unwrap();
    }
}
