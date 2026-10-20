#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::io::Write;

use omeka_db::repos::UploadRepo;

use super::*;
use crate::import2::evidence::NameEncoding;
use crate::import2::plan::{SourceId, SourcePlan};
use crate::import2::scan;
use crate::import2::staging::tests::site;

/// One entry for [`zip_of`].
struct Item<'a> {
    name: &'a [u8],
    utf8: bool,
    data: Vec<u8>,
    deflate: bool,
    link: bool,
}

fn file<'a>(name: &'a [u8], data: &[u8]) -> Item<'a> {
    Item {
        name,
        utf8: false,
        data: data.to_vec(),
        deflate: false,
        link: false,
    }
}

/// A ZIP archive, written the way APPNOTE lays it out: local headers and
/// data, then the central directory and its end record.
fn zip_of(items: &[Item<'_>]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    for item in items {
        let mut crc = flate2::Crc::new();
        crc.update(&item.data);
        let packed = if item.deflate {
            let mut encoder =
                flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::best());
            encoder.write_all(&item.data).unwrap();
            encoder.finish().unwrap()
        } else {
            item.data.clone()
        };
        let flags: u16 = if item.utf8 { 0x0800 } else { 0 };
        let method: u16 = if item.deflate { 8 } else { 0 };
        let offset = out.len() as u32;
        let name_len = item.name.len() as u16;
        out.extend_from_slice(&0x0403_4b50_u32.to_le_bytes());
        out.extend_from_slice(&20_u16.to_le_bytes());
        out.extend_from_slice(&flags.to_le_bytes());
        out.extend_from_slice(&method.to_le_bytes());
        out.extend_from_slice(&[0; 4]);
        out.extend_from_slice(&crc.sum().to_le_bytes());
        out.extend_from_slice(&(packed.len() as u32).to_le_bytes());
        out.extend_from_slice(&(item.data.len() as u32).to_le_bytes());
        out.extend_from_slice(&name_len.to_le_bytes());
        out.extend_from_slice(&0_u16.to_le_bytes());
        out.extend_from_slice(item.name);
        out.extend_from_slice(&packed);
        let made_by: u16 = (3 << 8) | 20;
        let mode: u32 = if item.link { 0o120_777 } else { 0o100_644 };
        central.extend_from_slice(&0x0201_4b50_u32.to_le_bytes());
        central.extend_from_slice(&made_by.to_le_bytes());
        central.extend_from_slice(&20_u16.to_le_bytes());
        central.extend_from_slice(&flags.to_le_bytes());
        central.extend_from_slice(&method.to_le_bytes());
        central.extend_from_slice(&[0; 4]);
        central.extend_from_slice(&crc.sum().to_le_bytes());
        central.extend_from_slice(&(packed.len() as u32).to_le_bytes());
        central.extend_from_slice(&(item.data.len() as u32).to_le_bytes());
        central.extend_from_slice(&name_len.to_le_bytes());
        central.extend_from_slice(&[0; 8]);
        central.extend_from_slice(&(mode << 16).to_le_bytes());
        central.extend_from_slice(&offset.to_le_bytes());
        central.extend_from_slice(item.name);
    }
    let directory_at = out.len() as u32;
    out.extend_from_slice(&central);
    out.extend_from_slice(&0x0605_4b50_u32.to_le_bytes());
    out.extend_from_slice(&[0; 4]);
    out.extend_from_slice(&(items.len() as u16).to_le_bytes());
    out.extend_from_slice(&(items.len() as u16).to_le_bytes());
    out.extend_from_slice(&(central.len() as u32).to_le_bytes());
    out.extend_from_slice(&directory_at.to_le_bytes());
    out.extend_from_slice(&0_u16.to_le_bytes());
    out
}

const ROOMY: Limits = Limits {
    max_file: 64 * 1024 * 1024,
    max_total: 64 * 1024 * 1024,
};

fn names(rows: &[UploadRow]) -> Vec<&str> {
    let mut names: Vec<&str> = rows.iter().map(|row| row.path.as_str()).collect();
    names.sort_unstable();
    names
}

/// S11, first half: a cp437 name is decoded and flagged, an escaping entry
/// and a link are left out and named, a nested ZIP stays a file, capitals
/// clash into `~1`, and the ZIP itself is gone.
#[tokio::test]
async fn a_zip_unpacks_with_decoded_names_and_without_its_escaping_entries() {
    let site = site().await;
    let inner = zip_of(&[file(b"inner.txt", b"inside")]);
    let archive = zip_of(&[
        file(b"A\xA4o 1950/foto.jpg", b"\xFF\xD8\xFF\xE0jpeg"),
        file(b"../../etc/evil.txt", b"evil"),
        file(b"/abs.txt", b"abs"),
        Item {
            link: true,
            ..file(b"link.jpg", b"/etc/passwd")
        },
        file(b"nested.zip", &inner),
        Item {
            utf8: true,
            deflate: true,
            ..file("Café/a.JPG".as_bytes(), &[7; 5000])
        },
        file(b"caf\x82/A.jpg", b"second"),
        file(b"Caf\xC3\xA9/b.jpg", b"utf8 without the flag"),
        file(b"win\\path.txt", b"backslash"),
    ]);
    let zip = site.upload_within("Scans/batch.zip", &archive, ROOMY).await;

    let unpacked = unzip_upload(&site.pool, &site.store, site.import, &zip.id, ROOMY)
        .await
        .unwrap();

    let rows = UploadRepo::new(&site.pool).list(site.import).await.unwrap();
    assert_eq!(
        names(&rows),
        [
            "Scans/Año 1950/foto.jpg",
            "Scans/Café/a.JPG",
            "Scans/Café/b.jpg",
            "Scans/café/A~1.jpg",
            "Scans/nested.zip",
            "Scans/win/path.txt",
        ]
    );
    assert_eq!(unpacked.files, 6);
    assert!(
        unpacked
            .notes
            .contains(&Note::Escapes("../../etc/evil.txt".into())),
        "{:?}",
        unpacked.notes
    );
    assert!(unpacked.notes.contains(&Note::Escapes("/abs.txt".into())));
    assert!(unpacked.notes.contains(&Note::Link("link.jpg".into())));
    assert!(unpacked.notes.contains(&Note::Renamed {
        name: "Scans/café/A.jpg".into(),
        renamed: "Scans/café/A~1.jpg".into(),
    }));
    assert!(
        !site
            .store
            .exists(&staging::file_path(site.import, &zip.id))
            .unwrap(),
        "the ZIP is gone once unpacked"
    );

    let sources = vec![SourcePlan::Staging {
        id: SourceId("up".into()),
    }];
    let roots = crate::import2::archive::ArchiveRoots::new(Vec::new()).unwrap();
    scan::scan(&site.pool, &roots, &site.store, site.import, &sources)
        .await
        .unwrap();
    let stored = scan::load(
        &site.pool,
        &scan::IndexCache::default(),
        site.import,
        &sources,
    )
    .await
    .unwrap();
    let walk = &stored.index.sources[0].walk;
    let decoded = walk
        .entries
        .iter()
        .find(|entry| entry.path.as_str() == "Scans/Año 1950/foto.jpg")
        .unwrap();
    let name = decoded.decoded.as_ref().expect("flags name-encoding");
    assert_eq!(name.encoding, NameEncoding::Cp437);
    assert_eq!(name.raw, b"A\xA4o 1950/foto.jpg");
    let plain = walk
        .entries
        .iter()
        .find(|entry| entry.path.as_str() == "Scans/Café/b.jpg")
        .unwrap();
    assert!(plain.decoded.is_none(), "UTF-8 bytes read as UTF-8");
    assert_eq!(walk.anomalies.symlinks, 1, "the link is counted");
    assert_eq!(
        walk.anomalies.unusable_names,
        ["../../etc/evil.txt", "/abs.txt"]
    );
    assert_eq!(
        walk.anomalies.collisions,
        ["Scans/café/A.jpg -> Scans/café/A~1.jpg"]
    );
    let nested = walk
        .entries
        .iter()
        .find(|entry| entry.path.as_str() == "Scans/nested.zip")
        .unwrap();
    assert_eq!(nested.size, inner.len() as u64, "a nested ZIP stays a file");
}

/// S11, second half: a bomb entry refuses the whole ZIP, and nothing from it
/// stays, not even the entries before the bomb.
#[tokio::test]
async fn a_bomb_entry_refuses_the_zip_and_keeps_nothing() {
    let site = site().await;
    let archive = zip_of(&[
        file(b"A\xA4o/first.jpg", b"\xFF\xD8\xFF\xE0jpeg"),
        file(b"../escape.txt", b"evil"),
        Item {
            deflate: true,
            ..file(b"bomb.bin", &vec![0; 11 * 1024 * 1024])
        },
    ]);
    assert!(archive.len() < 64 * 1024, "{} bytes", archive.len());
    let zip = site.upload_within("bomb.zip", &archive, ROOMY).await;

    let refused = unzip_upload(&site.pool, &site.store, site.import, &zip.id, ROOMY)
        .await
        .expect_err("a bomb");

    assert!(
        matches!(&refused, UnzipError::Refused(Refusal::Bomb { name }) if name == "bomb.bin"),
        "{refused:?}"
    );
    assert!(refused.to_string().contains("100 times"), "{refused}");
    assert!(
        UploadRepo::new(&site.pool)
            .list(site.import)
            .await
            .unwrap()
            .is_empty(),
        "nothing from the ZIP stays, and the ZIP goes too"
    );
    let files = site
        .store
        .list(&format!("{}/files", staging::staging_path(site.import)))
        .unwrap();
    assert!(files.is_empty(), "{files:?}");
}

/// The header can lie about the sizes: the guard counts what really
/// unpacks.
#[test]
fn the_guard_refuses_on_the_bytes_that_arrive() {
    assert!(is_bomb(10 * 1024 * 1024 + 1, 1024));
    assert!(!is_bomb(10 * 1024 * 1024, 1));
    assert!(!is_bomb(20 * 1024 * 1024, 1024 * 1024));
}

#[tokio::test]
async fn unpacked_files_count_against_the_import_limit() {
    let site = site().await;
    site.upload("already.bin", &[1; 600]).await;
    let packed = |name, byte| Item {
        deflate: true,
        ..file(name, &[byte; 500])
    };
    let archive = zip_of(&[
        packed(b"a.bin".as_slice(), 2),
        packed(b"b.bin".as_slice(), 3),
    ]);
    // The ZIP goes once unpacked, so what it unpacks to must fit beside the
    // other uploads: here 100 bytes more than the ZIP itself.
    let tight = Limits {
        max_file: 4096,
        max_total: 600 + archive.len() as u64 + 100,
    };
    let zip = site.upload_within("two.zip", &archive, tight).await;
    let refused = unzip_upload(&site.pool, &site.store, site.import, &zip.id, tight)
        .await
        .expect_err("1000 unpacked bytes pass the room left");
    assert!(
        matches!(refused, UnzipError::Refused(Refusal::Full { .. })),
        "{refused:?}"
    );
    let rows = UploadRepo::new(&site.pool).list(site.import).await.unwrap();
    assert_eq!(names(&rows), ["already.bin"]);
}

/// The archive `zip_of` wrote, with a ZIP64 end record that claims `count`
/// entries.
fn claiming(mut archive: Vec<u8>, count: u64) -> Vec<u8> {
    let end = archive.split_off(archive.len() - 22);
    let directory_size = u64::from(u32::from_le_bytes(end[12..16].try_into().unwrap()));
    let directory_at = u64::from(u32::from_le_bytes(end[16..20].try_into().unwrap()));
    let record_at = archive.len() as u64;
    archive.extend_from_slice(&0x0606_4b50_u32.to_le_bytes());
    archive.extend_from_slice(&44_u64.to_le_bytes());
    archive.extend_from_slice(&[45, 0, 45, 0]);
    archive.extend_from_slice(&[0; 8]);
    archive.extend_from_slice(&count.to_le_bytes());
    archive.extend_from_slice(&count.to_le_bytes());
    archive.extend_from_slice(&directory_size.to_le_bytes());
    archive.extend_from_slice(&directory_at.to_le_bytes());
    archive.extend_from_slice(&0x0706_4b50_u32.to_le_bytes());
    archive.extend_from_slice(&[0; 4]);
    archive.extend_from_slice(&record_at.to_le_bytes());
    archive.extend_from_slice(&1_u32.to_le_bytes());
    let mut end = end;
    end[8..12].copy_from_slice(&[0xFF; 4]);
    archive.extend_from_slice(&end);
    archive
}

#[tokio::test]
async fn an_archive_with_more_than_100000_entries_is_refused_from_its_directory() {
    let site = site().await;
    let archive = claiming(zip_of(&[file(b"a.txt", b"a")]), MAX_ENTRIES + 1);
    let zip = site.upload_within("many.zip", &archive, ROOMY).await;
    let refused = unzip_upload(&site.pool, &site.store, site.import, &zip.id, ROOMY)
        .await
        .expect_err("too many");
    assert!(
        matches!(
            refused,
            UnzipError::Refused(Refusal::TooMany { count: 100_001 })
        ),
        "{refused:?}"
    );
}

#[tokio::test]
async fn a_zip64_directory_is_read() {
    let site = site().await;
    let archive = claiming(zip_of(&[file(b"a.txt", b"a")]), 1);
    let zip = site.upload_within("one.zip", &archive, ROOMY).await;
    let unpacked = unzip_upload(&site.pool, &site.store, site.import, &zip.id, ROOMY)
        .await
        .unwrap();
    assert_eq!(unpacked.files, 1);
}

#[test]
fn a_zip_is_known_by_its_extension() {
    assert!(is_zip("a/B.ZIP"));
    assert!(is_zip("x.zip"));
    assert!(!is_zip("zip"));
    assert!(!is_zip("a.zip/b.jpg"));
    assert!(!is_zip("a.tar.gz"));
}

/// A header that hides the unpacked size passes the first check; the guard
/// still refuses on the bytes that arrive.
#[tokio::test]
async fn a_bomb_that_lies_about_its_size_is_refused_as_it_unpacks() {
    let site = site().await;
    let mut archive = zip_of(&[Item {
        deflate: true,
        ..file(b"liar.bin", &vec![0; 11 * 1024 * 1024])
    }]);
    let central = archive.len() - 22 - (46 + 8);
    archive[central + 24..central + 28].copy_from_slice(&1000_u32.to_le_bytes());
    let zip = site.upload_within("liar.zip", &archive, ROOMY).await;
    let refused = unzip_upload(&site.pool, &site.store, site.import, &zip.id, ROOMY)
        .await
        .expect_err("a bomb");
    assert!(
        matches!(&refused, UnzipError::Refused(Refusal::Bomb { name }) if name == "liar.bin"),
        "{refused:?}"
    );
}
