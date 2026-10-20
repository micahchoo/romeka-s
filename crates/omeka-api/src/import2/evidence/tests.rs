#![allow(clippy::expect_used)]

use std::fs;
use std::path::Path;

use super::*;

const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR";

fn write(root: &Path, path: &str, bytes: &[u8]) {
    let full = root.join(path);
    fs::create_dir_all(full.parent().expect("parent")).expect("mkdir");
    fs::write(full, bytes).expect("write");
}

fn source(root: &Path) -> SourceDir {
    SourceDir::standalone(root).expect("source")
}

fn paths(entries: &[FileEntry]) -> Vec<&str> {
    entries.iter().map(|entry| entry.path.as_str()).collect()
}

#[test]
fn files_are_listed_by_path_with_size_time_and_sniffed_type() {
    let dir = tempfile::tempdir().expect("tempdir");
    write(dir.path(), "Photographs/1950s/b.png", PNG);
    write(dir.path(), "Artifacts/a.jpg", PNG);
    write(dir.path(), "notes.md", b"plain words");
    write(dir.path(), "blob.bin", b"\0\x01\x02");
    let walk = walk(&source(dir.path())).expect("walk");
    assert_eq!(
        paths(&walk.entries),
        [
            "Artifacts/a.jpg",
            "Photographs/1950s/b.png",
            "blob.bin",
            "notes.md"
        ]
    );
    let png = &walk.entries[1];
    assert_eq!(png.size, PNG.len() as u64);
    assert_eq!(png.sniffed, "image/png");
    assert!(png.modified.is_some());
    assert_eq!(png.name(), "b.png");
    assert_eq!(png.folder(), "Photographs/1950s");
    assert_eq!(
        walk.entries[0].sniffed, "image/png",
        "the bytes, not the name"
    );
    assert_eq!(walk.entries[2].sniffed, "application/octet-stream");
    assert_eq!(walk.entries[3].sniffed, "text/plain");
    assert_eq!(
        walk.anomalies.identical[0].paths,
        ["Artifacts/a.jpg", "Photographs/1950s/b.png"],
        "both hold the same bytes"
    );
    assert_eq!(
        Anomalies {
            identical: Vec::new(),
            ..walk.anomalies.clone()
        },
        Anomalies::default()
    );
}

#[test]
fn text_cut_inside_a_character_at_the_sniff_limit_is_still_text() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut text = "a".repeat(SNIFF_BYTES - 1).into_bytes();
    text.extend("दस्तावेज़".as_bytes());
    write(dir.path(), "long.md", &text);
    let walk = walk(&source(dir.path())).expect("walk");
    assert_eq!(walk.entries[0].sniffed, "text/plain");
}

#[test]
fn a39_sheets_and_workbooks_are_listed_apart_and_are_never_file_entries() {
    let dir = tempfile::tempdir().expect("tempdir");
    write(dir.path(), "register.csv", b"id,title\nA1,Lamp\n");
    write(dir.path(), "rows/mixed.TSV", b"id\ttitle\n");
    write(
        dir.path(),
        "export.txt",
        b"id|title|date\nA1|Lamp|1890\nA2|Sign|1925\n",
    );
    write(
        dir.path(),
        "readme.txt",
        b"These scans came from the harbour office.\n",
    );
    write(dir.path(), "book.ods", b"PK\x03\x04");
    write(dir.path(), "ledger.xlsx", b"PK\x03\x04");
    write(dir.path(), "old/LEDGER.XLS", b"\xD0\xCF\x11\xE0");
    write(dir.path(), "ledger.docx", b"PK\x03\x04");
    let walk = walk(&source(dir.path())).expect("walk");
    assert_eq!(
        paths(&walk.sheets),
        [
            "book.ods",
            "export.txt",
            "ledger.xlsx",
            "old/LEDGER.XLS",
            "register.csv",
            "rows/mixed.TSV"
        ]
    );
    assert_eq!(paths(&walk.entries), ["ledger.docx", "readme.txt"]);
}

#[cfg(unix)]
#[test]
fn a18_symlinked_folders_and_dot_names_are_counted_not_followed() {
    use std::os::unix::fs::symlink;
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().join("root");
    write(&root, "years/1950/a.png", PNG);
    write(&root, ".DS_Store", b"x");
    write(&root, ".originals/a.png", PNG);
    write(&root, ".originals/b.png", PNG);
    write(dir.path(), "outside.png", PNG);
    symlink(root.join("years/1950"), root.join("latest")).expect("folder link");
    symlink(dir.path().join("outside.png"), root.join("leak.png")).expect("escape link");
    symlink(root.join("gone.png"), root.join("broken.png")).expect("broken link");
    symlink(root.join("years/1950/a.png"), root.join("same.png")).expect("file link");
    let walk = walk(&source(&root)).expect("walk");
    assert_eq!(paths(&walk.entries), ["same.png", "years/1950/a.png"]);
    assert_eq!(
        walk.anomalies.symlinks, 3,
        "folder, escape and broken links"
    );
    assert_eq!(walk.anomalies.dot, 2, "a dot folder counts once");
}

#[cfg(unix)]
#[test]
fn a18_names_that_are_not_utf8_are_decoded_and_kept() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;
    let dir = tempfile::tempdir().expect("tempdir");
    let folder = dir.path().join(OsStr::from_bytes(b"A\xA4o 1950"));
    fs::create_dir(&folder).expect("mkdir");
    fs::write(folder.join(OsStr::from_bytes(b"caf\xE9.png")), PNG).expect("write");
    let walk = walk(&source(dir.path())).expect("walk");
    let entry = &walk.entries[0];
    assert_eq!(entry.path.as_str(), "Año 1950/café.png");
    let decoded = entry.decoded.as_ref().expect("decoded");
    assert_eq!(decoded.raw, b"A\xA4o 1950/caf\xE9.png");
    assert_eq!(
        decoded.encoding,
        NameEncoding::Cp437,
        "the first name that needed decoding sets the flag"
    );
    assert_eq!(
        entry.sniffed, "image/png",
        "the file is read by its disk name"
    );
    let names: Vec<_> = walk
        .decoded_names()
        .map(|(raw, path)| (raw, path.to_string()))
        .collect();
    assert_eq!(
        names,
        [(
            "A\u{FFFD}o 1950/caf\u{FFFD}.png".to_string(),
            "Año 1950/café.png".to_string()
        )]
    );
    let disk = source(dir.path()).locate(&entry.disk_path());
    assert!(disk.is_ok(), "{disk:?}");
}

#[cfg(unix)]
#[test]
fn names_a_path_refuses_and_names_that_collide_are_named() {
    let dir = tempfile::tempdir().expect("tempdir");
    write(dir.path(), "back\\slash.png", PNG);
    write(dir.path(), "Cafe\u{301}.png", PNG);
    write(dir.path(), "Caf\u{e9}.png", PNG);
    let walk = walk(&source(dir.path())).expect("walk");
    assert_eq!(paths(&walk.entries), ["Caf\u{e9}.png"]);
    assert_eq!(walk.anomalies.unusable_names, ["back\\slash.png"]);
    assert_eq!(walk.anomalies.collisions.len(), 1);
}

#[cfg(unix)]
#[test]
fn a_folder_the_walk_cannot_read_is_named() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().expect("tempdir");
    write(dir.path(), "locked/a.png", PNG);
    write(dir.path(), "open/b.png", PNG);
    let locked = dir.path().join("locked");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).expect("chmod");
    let readable = fs::read_dir(&locked).is_ok();
    let walk = walk(&source(dir.path())).expect("walk");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).expect("chmod back");
    if readable {
        return;
    }
    assert_eq!(paths(&walk.entries), ["open/b.png"]);
    assert_eq!(walk.anomalies.unreadable, ["locked"]);
}

/// The contract's scan figure: 100,000 files within 30 s on the NVMe.
/// `ROMEKA_SCALE_DIR` puts the tree on the disk to measure; the default
/// temp folder may be memory.
#[test]
#[ignore = "scale: run with --release -- --ignored"]
fn walk_100k_files_within_30_seconds() {
    let base = std::env::var_os("ROMEKA_SCALE_DIR").map_or_else(std::env::temp_dir, Into::into);
    let dir = tempfile::tempdir_in(base).expect("tempdir");
    for folder in 0..100 {
        let path = dir.path().join(format!("box {folder:03}"));
        fs::create_dir(&path).expect("mkdir");
        for file in 0..1000 {
            fs::write(path.join(format!("BHC_{folder:03}_{file:04}.png")), PNG).expect("write");
        }
    }
    let started = std::time::Instant::now();
    let walk = walk(&source(dir.path())).expect("walk");
    let took = started.elapsed();
    eprintln!("walked {} files in {took:?}", walk.entries.len());
    assert_eq!(walk.entries.len(), 100_000);
    assert!(took.as_secs() < 30, "{took:?}");
}
