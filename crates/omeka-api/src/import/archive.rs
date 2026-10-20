//! The archive: the server folder an import reads files from.
//!
//! [`ArchiveRoot`] is built from the configured path (the server reads
//! `ROMEKA_ARCHIVE_ROOT` and passes it in). Every relative path from a client
//! goes through [`ArchiveRoot::folder`] or [`ArchiveFolder::file`], which
//! canonicalize it and refuse anything outside the root, symlinks that leave
//! it included. A symlink to a folder is never followed, so a walk cannot loop.
//! Names that start with a dot and names that are not UTF-8 are not listed.

use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::Serialize;

/// Why a path in the archive cannot be used.
#[derive(Debug, thiserror::Error)]
pub enum ArchiveError {
    #[error("the archive root {0} is not a folder")]
    NoRoot(PathBuf),
    #[error("{0:?} is outside the archive")]
    Outside(String),
    #[error("{0:?} is not in the archive")]
    NotFound(String),
    #[error("{0:?} is not a folder")]
    NotAFolder(String),
    #[error("{0:?} is not a file")]
    NotAFile(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// The canonical archive root.
#[derive(Debug, Clone)]
pub struct ArchiveRoot {
    root: PathBuf,
}

/// A folder inside the root, by its path relative to the root.
#[derive(Debug, Clone)]
pub struct ArchiveFolder {
    root: PathBuf,
    relative: String,
    absolute: PathBuf,
}

/// One file of an import folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveFile {
    /// Relative to the import folder, `/` between folders.
    pub path: String,
    pub size: u64,
    pub absolute: PathBuf,
}

/// One entry of a folder listing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Entry {
    pub name: String,
    pub kind: EntryKind,
    /// For a folder, the number of files anywhere under it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub files: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum EntryKind {
    Dir,
    File,
}

impl ArchiveRoot {
    /// # Errors
    /// [`ArchiveError::NoRoot`] when `path` is not an existing folder.
    pub fn new(path: impl AsRef<Path>) -> Result<Self, ArchiveError> {
        let path = path.as_ref();
        let root = fs::canonicalize(path).map_err(|_| ArchiveError::NoRoot(path.to_path_buf()))?;
        if !root.is_dir() {
            return Err(ArchiveError::NoRoot(path.to_path_buf()));
        }
        Ok(Self { root })
    }

    /// The folder at `relative` (`""` is the root itself).
    ///
    /// # Errors
    /// [`ArchiveError::Outside`] for an absolute path or one that resolves
    /// outside the root, [`ArchiveError::NotFound`] or
    /// [`ArchiveError::NotAFolder`] otherwise.
    pub fn folder(&self, relative: &str) -> Result<ArchiveFolder, ArchiveError> {
        let absolute = inside(&self.root, &self.root, relative)?;
        if !absolute.is_dir() {
            return Err(ArchiveError::NotAFolder(relative.to_string()));
        }
        let relative = slash_path(absolute.strip_prefix(&self.root).unwrap_or(Path::new("")));
        Ok(ArchiveFolder {
            root: self.root.clone(),
            relative,
            absolute,
        })
    }
}

impl ArchiveFolder {
    /// The canonical path relative to the archive root.
    #[must_use]
    pub fn relative(&self) -> &str {
        &self.relative
    }

    /// The folder's direct entries, folders first, each group by name.
    ///
    /// # Errors
    /// [`ArchiveError::Io`] when the folder cannot be read.
    pub fn entries(&self) -> Result<Vec<Entry>, ArchiveError> {
        let mut entries = Vec::new();
        for (name, path, kind) in children(&self.root, &self.absolute)? {
            let files = match kind {
                EntryKind::Dir => {
                    let mut found = Vec::new();
                    walk(&self.root, &path, &name, &mut found)?;
                    Some(found.len())
                }
                EntryKind::File => None,
            };
            entries.push(Entry { name, kind, files });
        }
        entries.sort_by(|a, b| {
            (a.kind == EntryKind::File, &a.name).cmp(&(b.kind == EntryKind::File, &b.name))
        });
        Ok(entries)
    }

    /// Every file under the folder, sorted by path.
    ///
    /// # Errors
    /// [`ArchiveError::Io`] when a folder cannot be read.
    pub fn files(&self) -> Result<Vec<ArchiveFile>, ArchiveError> {
        let mut found = Vec::new();
        walk(&self.root, &self.absolute, "", &mut found)?;
        found.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(found)
    }

    /// The file at `relative` inside this folder.
    ///
    /// # Errors
    /// [`ArchiveError::Outside`] for a path outside the archive root,
    /// [`ArchiveError::NotFound`] or [`ArchiveError::NotAFile`] otherwise.
    pub fn file(&self, relative: &str) -> Result<PathBuf, ArchiveError> {
        let absolute = inside(&self.root, &self.absolute, relative)?;
        if !absolute.is_file() {
            return Err(ArchiveError::NotAFile(relative.to_string()));
        }
        Ok(absolute)
    }
}

/// `base` joined with `relative`, canonical, refused unless under `root`.
fn inside(root: &Path, base: &Path, relative: &str) -> Result<PathBuf, ArchiveError> {
    let relative_path = Path::new(relative);
    if relative_path
        .components()
        .any(|part| matches!(part, Component::RootDir | Component::Prefix(_)))
    {
        return Err(ArchiveError::Outside(relative.to_string()));
    }
    let absolute =
        fs::canonicalize(base.join(relative_path)).map_err(|error| match error.kind() {
            std::io::ErrorKind::NotFound => ArchiveError::NotFound(relative.to_string()),
            _ => ArchiveError::Io(error),
        })?;
    if !absolute.starts_with(root) {
        return Err(ArchiveError::Outside(relative.to_string()));
    }
    Ok(absolute)
}

/// The listable children of `folder`: no dot names, no non-UTF-8 names, no
/// symlink that leaves the root, no symlink to a folder.
fn children(root: &Path, folder: &Path) -> Result<Vec<(String, PathBuf, EntryKind)>, ArchiveError> {
    let mut out = Vec::new();
    for entry in fs::read_dir(folder)? {
        let entry = entry?;
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        if name.starts_with('.') {
            continue;
        }
        let path = entry.path();
        let file_type = entry.file_type()?;
        let kind = if file_type.is_symlink() {
            match fs::canonicalize(&path) {
                Ok(target) if target.starts_with(root) && target.is_file() => EntryKind::File,
                _ => continue,
            }
        } else if file_type.is_dir() {
            EntryKind::Dir
        } else if file_type.is_file() {
            EntryKind::File
        } else {
            continue;
        };
        out.push((name, path, kind));
    }
    Ok(out)
}

fn walk(
    root: &Path,
    folder: &Path,
    prefix: &str,
    found: &mut Vec<ArchiveFile>,
) -> Result<(), ArchiveError> {
    for (name, path, kind) in children(root, folder)? {
        let relative = if prefix.is_empty() {
            name
        } else {
            format!("{prefix}/{name}")
        };
        match kind {
            EntryKind::Dir => walk(root, &path, &relative, found)?,
            EntryKind::File => found.push(ArchiveFile {
                path: relative,
                size: fs::metadata(&path)?.len(),
                absolute: path,
            }),
        }
    }
    Ok(())
}

fn slash_path(path: &Path) -> String {
    path.components()
        .map(|part| part.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;

    fn tree() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("tempdir");
        for path in [
            "BHC_006/Artifacts/a.jpg",
            "BHC_006/Photographs/1950s/b.tif",
            "BHC_006/notes.txt",
            "BHC_006/.DS_Store",
            "other.txt",
        ] {
            let full = dir.path().join("archive").join(path);
            fs::create_dir_all(full.parent().expect("parent")).expect("mkdir");
            fs::write(&full, b"12345").expect("write");
        }
        fs::write(dir.path().join("secret.txt"), b"no").expect("write");
        dir
    }

    #[test]
    fn lists_entries_with_file_counts_and_walks_sorted_slash_paths() {
        let dir = tree();
        let root = ArchiveRoot::new(dir.path().join("archive")).expect("root");
        let top = root.folder("").expect("root folder");
        assert_eq!(top.relative(), "");
        let names: Vec<_> = top
            .entries()
            .expect("entries")
            .into_iter()
            .map(|e| (e.name, e.kind, e.files))
            .collect();
        assert_eq!(
            names,
            [
                ("BHC_006".to_string(), EntryKind::Dir, Some(3)),
                ("other.txt".to_string(), EntryKind::File, None)
            ]
        );

        let folder = root.folder("BHC_006/Photographs/..").expect("folder");
        assert_eq!(folder.relative(), "BHC_006");
        let files: Vec<_> = folder
            .files()
            .expect("files")
            .into_iter()
            .map(|f| (f.path, f.size))
            .collect();
        assert_eq!(
            files,
            [
                ("Artifacts/a.jpg".to_string(), 5),
                ("Photographs/1950s/b.tif".to_string(), 5),
                ("notes.txt".to_string(), 5)
            ]
        );
        assert!(folder.file("notes.txt").is_ok());
        assert!(matches!(
            folder.file("Artifacts"),
            Err(ArchiveError::NotAFile(_))
        ));
    }

    #[test]
    fn paths_outside_the_root_are_refused() {
        let dir = tree();
        let root = ArchiveRoot::new(dir.path().join("archive")).expect("root");
        assert!(matches!(root.folder(".."), Err(ArchiveError::Outside(_))));
        assert!(matches!(root.folder("/etc"), Err(ArchiveError::Outside(_))));
        assert!(matches!(
            root.folder("missing"),
            Err(ArchiveError::NotFound(_))
        ));
        assert!(matches!(
            root.folder("other.txt"),
            Err(ArchiveError::NotAFolder(_))
        ));
        let folder = root.folder("BHC_006").expect("folder");
        assert!(matches!(
            folder.file("../../secret.txt"),
            Err(ArchiveError::Outside(_))
        ));
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_that_escapes_the_root_is_refused_and_not_listed() {
        let dir = tree();
        let archive = dir.path().join("archive");
        std::os::unix::fs::symlink(dir.path(), archive.join("escape")).expect("dir link");
        std::os::unix::fs::symlink(
            dir.path().join("secret.txt"),
            archive.join("BHC_006/leak.txt"),
        )
        .expect("file link");
        std::os::unix::fs::symlink(
            archive.join("other.txt"),
            archive.join("BHC_006/inside.txt"),
        )
        .expect("inside link");
        let root = ArchiveRoot::new(&archive).expect("root");

        assert!(matches!(
            root.folder("escape"),
            Err(ArchiveError::Outside(_))
        ));
        assert!(matches!(
            root.folder("escape/secret.txt"),
            Err(ArchiveError::Outside(_))
        ));
        let folder = root.folder("BHC_006").expect("folder");
        assert!(matches!(
            folder.file("leak.txt"),
            Err(ArchiveError::Outside(_))
        ));
        let paths: Vec<_> = folder
            .files()
            .expect("files")
            .into_iter()
            .map(|f| f.path)
            .collect();
        assert!(!paths.contains(&"leak.txt".to_string()), "{paths:?}");
        assert!(paths.contains(&"inside.txt".to_string()), "{paths:?}");
        let top: Vec<_> = root
            .folder("")
            .expect("top")
            .entries()
            .expect("entries")
            .into_iter()
            .map(|e| e.name)
            .collect();
        assert!(!top.contains(&"escape".to_string()), "{top:?}");
    }
}
