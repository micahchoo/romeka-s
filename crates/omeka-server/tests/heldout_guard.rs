//! The held-out arrangement, checked: the manifest (`heldout/HELDOUT`) lists
//! every file of the held-out folder, and no file outside it names a
//! held-out shape. A held-out shape is the stem of a pin under
//! `heldout/outcome_pins/`. This target names none of them; it reads them
//! from the manifest, so it runs in a build tree with the held-out files
//! stripped.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

const MANIFEST: &str = "crates/omeka-server/tests/heldout/HELDOUT";
const HELDOUT_DIR: &str = "crates/omeka-server/tests/heldout/";
const PINS_DIR: &str = "crates/omeka-server/tests/heldout/outcome_pins/";

/// The folders a build reads: the crates, and the admin's sources and tests.
const SEARCHED: [&str; 4] = ["crates", "web/admin/src", "web/admin/tests", "scripts"];

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap()
        .to_path_buf()
}

/// The manifest's repository paths, and its paths outside the repository.
fn manifest() -> (BTreeSet<String>, BTreeSet<String>) {
    let text = std::fs::read_to_string(repo().join(MANIFEST)).unwrap();
    let lines = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'));
    let (outside, inside): (Vec<&str>, Vec<&str>) = lines.partition(|line| line.starts_with("../"));
    let set = |lines: Vec<&str>| lines.into_iter().map(str::to_string).collect();
    (set(inside), set(outside))
}

/// The words a file outside the manifest must not hold: each shape's name,
/// and each outside path's folder name.
fn names(inside: &BTreeSet<String>, outside: &BTreeSet<String>) -> BTreeSet<String> {
    let shapes = inside
        .iter()
        .filter_map(|path| path.strip_prefix(PINS_DIR)?.strip_suffix(".txt"))
        .map(str::to_string);
    let folders = outside
        .iter()
        .filter_map(|path| path.trim_end_matches('/').rsplit('/').next())
        .map(str::to_string);
    shapes.chain(folders).collect()
}

/// Every file under `dir`, as a path from the repository root. Build output
/// and installed packages are not the build tree's own files.
fn walk(root: &Path, dir: &Path, files: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy();
        if path.is_dir() {
            if !matches!(name.as_ref(), "target" | "node_modules" | "dist") {
                walk(root, &path, files);
            }
        } else {
            let relative = path.strip_prefix(root).unwrap();
            files.push(relative.to_string_lossy().replace('\\', "/"));
        }
    }
}

fn contains(haystack: &[u8], needle: &str) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle.as_bytes())
}

#[test]
fn the_build_tree_names_no_heldout_file() {
    let root = repo();
    let (inside, outside) = manifest();
    let names = names(&inside, &outside);
    assert!(!names.is_empty(), "the manifest names held-out shapes");
    let mut files = Vec::new();
    for dir in SEARCHED {
        walk(&root, &root.join(dir), &mut files);
    }
    let mut naming = Vec::new();
    for file in files.iter().filter(|file| !inside.contains(*file)) {
        if file == MANIFEST {
            continue;
        }
        if file.starts_with(HELDOUT_DIR) {
            naming.push(format!(
                "{file}: in the held-out folder, not in the manifest"
            ));
            continue;
        }
        let bytes = std::fs::read(root.join(file)).unwrap();
        for name in &names {
            if contains(file.as_bytes(), name) || contains(&bytes, name) {
                naming.push(format!("{file}: names a held-out shape"));
            }
        }
    }
    assert!(naming.is_empty(), "{}", naming.join("\n"));
}
