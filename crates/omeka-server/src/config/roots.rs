//! The archive roots import v2 reads (`docs/import-v2.md`, "Arrival").
//!
//! `ROMEKA_ARCHIVE_ROOTS` holds `name=path` pairs separated by `;`; the file
//! key `archive_roots` is an object of the same pairs, and the variable
//! replaces it. The v1 `ROMEKA_ARCHIVE_ROOT` stays as the root named
//! `archive`. Every path is made absolute here, at load, so a later change
//! of working directory cannot move a root.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use omeka_api::import2::archive::{ArchiveRoots, V1_ROOT_NAME};

use super::{ARCHIVE_ROOTS_ENV, ConfigError, invalid};

/// The pairs of a `ROMEKA_ARCHIVE_ROOTS` value. Empty pieces, as from a
/// trailing `;`, are skipped, so an empty value names no root.
pub(super) fn parse(value: &str) -> Result<Vec<(String, String)>, ConfigError> {
    value
        .split(';')
        .map(str::trim)
        .filter(|piece| !piece.is_empty())
        .map(|piece| {
            piece
                .split_once('=')
                .map(|(name, path)| (name.trim().to_string(), path.trim().to_string()))
                .ok_or_else(|| invalid(ARCHIVE_ROOTS_ENV, value, "a root is not name=path"))
        })
        .collect()
}

/// The pairs of the file key.
pub(super) fn from_file(roots: &BTreeMap<String, String>) -> Vec<(String, String)> {
    roots
        .iter()
        .map(|(name, path)| (name.trim().to_string(), path.trim().to_string()))
        .collect()
}

/// The named roots and the v1 root as `archive`, each path absolute.
pub(super) fn build(
    named: Vec<(String, String)>,
    v1: Option<&Path>,
) -> Result<ArchiveRoots, ConfigError> {
    let mut roots = Vec::new();
    for (name, path) in named {
        if path.is_empty() {
            return Err(invalid(ARCHIVE_ROOTS_ENV, &name, "the root has no path"));
        }
        roots.push((name, absolute(&path)?));
    }
    if let Some(path) = v1 {
        roots.push((V1_ROOT_NAME.to_string(), path.to_path_buf()));
    }
    ArchiveRoots::new(roots).map_err(|error| invalid(ARCHIVE_ROOTS_ENV, "", error))
}

/// `path` made absolute against the working directory, without touching it:
/// a root that does not exist yet is reported when a source uses it.
pub(super) fn absolute(path: &str) -> Result<PathBuf, ConfigError> {
    std::path::absolute(path).map_err(|error| invalid(ARCHIVE_ROOTS_ENV, path, error))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::super::{ARCHIVE_ROOT_ENV, Config};
    use super::*;

    fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> + use<> {
        let pairs: Vec<(String, String)> = pairs
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect();
        move |key| {
            pairs
                .iter()
                .find(|(name, _)| name == key)
                .map(|(_, value)| value.clone())
        }
    }

    fn names(config: &Config) -> Vec<(String, PathBuf)> {
        config
            .archive_roots
            .iter()
            .map(|(name, path)| (name.to_string(), path.to_path_buf()))
            .collect()
    }

    #[test]
    fn named_roots_come_from_the_variable_and_the_v1_root_is_archive() {
        let config = Config::from_sources(
            env(&[
                (
                    ARCHIVE_ROOTS_ENV,
                    "photos=/srv/photos; scans = /srv/scans ;",
                ),
                (ARCHIVE_ROOT_ENV, "/srv/archive"),
            ]),
            None,
        )
        .unwrap();
        assert_eq!(
            names(&config),
            [
                ("photos".to_string(), PathBuf::from("/srv/photos")),
                ("scans".to_string(), PathBuf::from("/srv/scans")),
                ("archive".to_string(), PathBuf::from("/srv/archive")),
            ]
        );
        assert!(Config::default().archive_roots.is_empty());
    }

    #[test]
    fn relative_roots_are_made_absolute_at_load() {
        let config = Config::from_sources(
            env(&[
                (ARCHIVE_ROOTS_ENV, "photos=data/photos"),
                (ARCHIVE_ROOT_ENV, "data/archive"),
            ]),
            None,
        )
        .unwrap();
        let here = std::env::current_dir().unwrap();
        assert_eq!(config.archive_root, Some(here.join("data/archive")));
        assert_eq!(
            config.archive_roots.get("photos"),
            Some(here.join("data/photos").as_path())
        );
        assert_eq!(
            config.archive_roots.get("archive"),
            Some(here.join("data/archive").as_path())
        );
    }

    #[test]
    fn the_variable_replaces_the_files_roots() {
        let file = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(file.path(), r#"{"archive_roots":{"photos":"/srv/photos"}}"#).unwrap();
        let from_file = Config::from_sources(|_| None, Some(file.path())).unwrap();
        assert_eq!(
            from_file.archive_roots.get("photos"),
            Some(Path::new("/srv/photos"))
        );
        let replaced = Config::from_sources(
            env(&[(ARCHIVE_ROOTS_ENV, "films=/srv/films")]),
            Some(file.path()),
        )
        .unwrap();
        assert_eq!(replaced.archive_roots.get("photos"), None);
        assert!(replaced.archive_roots.get("films").is_some());
    }

    #[test]
    fn a_malformed_or_repeated_root_names_the_variable() {
        for value in ["photos", "=/srv/a", "a=", "a=/srv/a;a=/srv/b"] {
            let error = Config::from_sources(env(&[(ARCHIVE_ROOTS_ENV, value)]), None).unwrap_err();
            assert!(
                error.to_string().contains(ARCHIVE_ROOTS_ENV),
                "{value}: {error}"
            );
        }
        let clash = Config::from_sources(
            env(&[
                (ARCHIVE_ROOTS_ENV, "archive=/srv/a"),
                (ARCHIVE_ROOT_ENV, "/srv/b"),
            ]),
            None,
        );
        assert!(clash.is_err(), "the v1 root owns the name archive");
    }
}
