//! Archive roots: the named server folders a folder source reads
//! (`docs/import-v2.md`, "Arrival").
//!
//! The server builds [`ArchiveRoots`] from `ROMEKA_ARCHIVE_ROOTS` and the v1
//! `ROMEKA_ARCHIVE_ROOT`. A plan names a root and a [`RelPath`] under it;
//! [`ArchiveRoots::source`] turns the pair into a [`SourceDir`], and every
//! file read after that goes through [`SourceDir::locate`]. Both
//! canonicalize and refuse a result outside the root, so a symlink that
//! points out of the archive is refused wherever it sits in the path.

use std::fs;
use std::path::{Path, PathBuf};

use super::plan::RelPath;

/// The root a v1 `ROMEKA_ARCHIVE_ROOT` becomes.
pub const V1_ROOT_NAME: &str = "archive";

/// The configured roots, each name once.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ArchiveRoots(Vec<(String, PathBuf)>);

/// Why a root list cannot be built.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RootsError {
    #[error("the archive root name {0:?} is used twice")]
    Duplicate(String),
    #[error("an archive root has an empty name")]
    EmptyName,
    #[error("the archive root {0:?} has a path that is not absolute")]
    NotAbsolute(String),
}

/// Why a source or a file in it cannot be used.
#[derive(Debug, thiserror::Error)]
pub enum ArchiveError {
    #[error("no archive root is named {0:?}")]
    UnknownRoot(String),
    #[error("the archive root {0} is not a folder")]
    NoRoot(PathBuf),
    #[error("{0:?} is outside the archive root")]
    Outside(String),
    #[error("{0:?} is not in the source")]
    NotFound(String),
    #[error("{0:?} is not a folder")]
    NotAFolder(String),
    #[error("{0:?} is not a file")]
    NotAFile(String),
    #[error(transparent)]
    Io(std::io::Error),
}

impl ArchiveRoots {
    /// Paths must already be absolute: the config makes them so at load,
    /// so a later change of working directory cannot move a root.
    ///
    /// # Errors
    /// A [`RootsError`] for an empty or repeated name or a relative path.
    pub fn new(roots: impl IntoIterator<Item = (String, PathBuf)>) -> Result<Self, RootsError> {
        let mut out: Vec<(String, PathBuf)> = Vec::new();
        for (name, path) in roots {
            if name.is_empty() {
                return Err(RootsError::EmptyName);
            }
            if !path.is_absolute() {
                return Err(RootsError::NotAbsolute(name));
            }
            if out.iter().any(|(known, _)| *known == name) {
                return Err(RootsError::Duplicate(name));
            }
            out.push((name, path));
        }
        Ok(Self(out))
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The roots in configured order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &Path)> {
        self.0
            .iter()
            .map(|(name, path)| (name.as_str(), path.as_path()))
    }

    #[must_use]
    pub fn get(&self, name: &str) -> Option<&Path> {
        self.iter()
            .find(|(known, _)| *known == name)
            .map(|(_, path)| path)
    }

    /// The folder at `path` under the root `root`.
    ///
    /// # Errors
    /// [`ArchiveError::UnknownRoot`], [`ArchiveError::NoRoot`] when the root
    /// is not a folder, [`ArchiveError::Outside`] when the folder resolves
    /// outside the root, and [`ArchiveError::NotFound`] or
    /// [`ArchiveError::NotAFolder`] otherwise.
    pub fn source(&self, root: &str, path: &RelPath) -> Result<SourceDir, ArchiveError> {
        let base = self
            .get(root)
            .ok_or_else(|| ArchiveError::UnknownRoot(root.to_string()))?;
        let boundary = canonical_folder(base)?;
        let dir = inside(&boundary, &boundary.join(path.as_str()), path.as_str())?;
        if !dir.is_dir() {
            return Err(ArchiveError::NotAFolder(path.to_string()));
        }
        Ok(SourceDir { boundary, dir })
    }
}

/// A source's folder on disk, with the boundary its files must stay under:
/// the archive root for a folder source, the folder itself for staging.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceDir {
    boundary: PathBuf,
    dir: PathBuf,
}

impl SourceDir {
    /// A folder that is its own boundary, such as an import's staging area.
    ///
    /// # Errors
    /// [`ArchiveError::NoRoot`] when `dir` is not a folder.
    pub fn standalone(dir: &Path) -> Result<Self, ArchiveError> {
        let dir = canonical_folder(dir)?;
        Ok(Self {
            boundary: dir.clone(),
            dir,
        })
    }

    /// The canonical folder.
    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The canonical path a file must stay under.
    #[must_use]
    pub fn boundary(&self) -> &Path {
        &self.boundary
    }

    /// The file at `relative` (a [`RelPath`], or the on-disk bytes of a
    /// decoded name), canonical and under the boundary. The index can be
    /// older than the disk, so this checks again on every read.
    ///
    /// # Errors
    /// [`ArchiveError::Outside`], [`ArchiveError::NotFound`] or
    /// [`ArchiveError::NotAFile`].
    pub fn locate(&self, relative: &Path) -> Result<PathBuf, ArchiveError> {
        let shown = relative.to_string_lossy();
        let file = inside(&self.boundary, &self.dir.join(relative), &shown)?;
        if !file.is_file() {
            return Err(ArchiveError::NotAFile(shown.into_owned()));
        }
        Ok(file)
    }
}

fn canonical_folder(path: &Path) -> Result<PathBuf, ArchiveError> {
    match fs::canonicalize(path) {
        Ok(dir) if dir.is_dir() => Ok(dir),
        _ => Err(ArchiveError::NoRoot(path.to_path_buf())),
    }
}

fn inside(boundary: &Path, path: &Path, shown: &str) -> Result<PathBuf, ArchiveError> {
    let resolved = fs::canonicalize(path).map_err(|error| match error.kind() {
        std::io::ErrorKind::NotFound => ArchiveError::NotFound(shown.to_string()),
        _ => ArchiveError::Io(error),
    })?;
    if !resolved.starts_with(boundary) {
        return Err(ArchiveError::Outside(shown.to_string()));
    }
    Ok(resolved)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;

    fn rel(text: &str) -> RelPath {
        RelPath::parse(text).expect("path")
    }

    fn tree() -> (tempfile::TempDir, ArchiveRoots) {
        let dir = tempfile::tempdir().expect("tempdir");
        for path in ["photos/BHC_006/a.jpg", "photos/notes.txt", "scans/b.tif"] {
            let full = dir.path().join(path);
            fs::create_dir_all(full.parent().expect("parent")).expect("mkdir");
            fs::write(&full, b"12345").expect("write");
        }
        fs::write(dir.path().join("secret.txt"), b"no").expect("write");
        let roots = ArchiveRoots::new([
            ("photos".to_string(), dir.path().join("photos")),
            ("scans".to_string(), dir.path().join("scans")),
        ])
        .expect("roots");
        (dir, roots)
    }

    #[test]
    fn a_root_name_is_used_once_and_its_path_is_absolute() {
        let twice = ArchiveRoots::new([
            ("a".to_string(), PathBuf::from("/srv/a")),
            ("a".to_string(), PathBuf::from("/srv/b")),
        ]);
        assert_eq!(twice, Err(RootsError::Duplicate("a".to_string())));
        let relative = ArchiveRoots::new([("a".to_string(), PathBuf::from("srv/a"))]);
        assert_eq!(relative, Err(RootsError::NotAbsolute("a".to_string())));
        let empty = ArchiveRoots::new([(String::new(), PathBuf::from("/srv/a"))]);
        assert_eq!(empty, Err(RootsError::EmptyName));
    }

    #[test]
    fn a_source_resolves_by_root_name_and_relative_path() {
        let (dir, roots) = tree();
        let source = roots.source("photos", &rel("BHC_006")).expect("source");
        let photos = fs::canonicalize(dir.path().join("photos")).expect("canonical");
        assert_eq!(source.dir(), photos.join("BHC_006"));
        assert_eq!(source.boundary(), photos);
        assert!(source.locate(Path::new("a.jpg")).is_ok());
        assert!(roots.source("scans", &rel("")).is_ok());
        assert!(matches!(
            roots.source("films", &rel("")),
            Err(ArchiveError::UnknownRoot(_))
        ));
        assert!(matches!(
            roots.source("photos", &rel("missing")),
            Err(ArchiveError::NotFound(_))
        ));
        assert!(matches!(
            roots.source("photos", &rel("notes.txt")),
            Err(ArchiveError::NotAFolder(_))
        ));
        assert!(matches!(
            source.locate(Path::new("../../secret.txt")),
            Err(ArchiveError::Outside(_))
        ));
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_out_of_the_root_is_refused_for_folders_and_files() {
        let (dir, roots) = tree();
        let photos = dir.path().join("photos");
        std::os::unix::fs::symlink(dir.path(), photos.join("escape")).expect("dir link");
        std::os::unix::fs::symlink(dir.path().join("secret.txt"), photos.join("leak.txt"))
            .expect("file link");
        std::os::unix::fs::symlink(photos.join("BHC_006"), photos.join("inside"))
            .expect("inside link");
        assert!(matches!(
            roots.source("photos", &rel("escape")),
            Err(ArchiveError::Outside(_))
        ));
        let top = roots.source("photos", &rel("")).expect("top");
        assert!(matches!(
            top.locate(Path::new("leak.txt")),
            Err(ArchiveError::Outside(_))
        ));
        assert!(top.locate(Path::new("inside/a.jpg")).is_ok());
    }

    #[test]
    fn a_staging_folder_is_its_own_boundary() {
        let (dir, _) = tree();
        let staging = SourceDir::standalone(&dir.path().join("photos/BHC_006")).expect("staging");
        assert!(staging.locate(Path::new("a.jpg")).is_ok());
        assert!(matches!(
            staging.locate(Path::new("../notes.txt")),
            Err(ArchiveError::Outside(_))
        ));
    }
}
