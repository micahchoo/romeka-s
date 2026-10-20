//! The file-name pattern: a regex whose named groups read a file's path.
//!
//! The admin compiles a marked example to a regex, or the user types one. The
//! server compiles it with the `regex` crate, which accepts most of what a
//! JavaScript `RegExp` does. What it refuses is lookaround (`(?=`, `(?!`,
//! `(?<=`, `(?<!`) and backreferences (`\1`, `\k<name>`); those are syntax
//! errors here. `\d` and `\w` match Unicode digits and word characters here,
//! where JavaScript without the `u` flag matches only ASCII.

use std::collections::BTreeMap;

use regex::Regex;

/// The group that decides which files are one item.
pub const IDENTIFIER: &str = "identifier";
/// The group that tells the files of one item apart.
pub const PART: &str = "part";
/// The group the `group` set level reads.
pub const SET: &str = "set";

/// Why a plan's pattern cannot match files.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PatternError {
    #[error("the plan has no pattern yet")]
    Missing,
    #[error("the pattern is not a valid regex: {0}")]
    Syntax(String),
    #[error("the pattern has no group named identifier")]
    NoIdentifier,
}

impl PatternError {
    /// The key the admin translates.
    #[must_use]
    pub fn key(&self) -> &'static str {
        match self {
            Self::Missing => "patternMissing",
            Self::Syntax(_) => "patternSyntax",
            Self::NoIdentifier => "patternIdentifier",
        }
    }

    /// The regex compiler's own words, for a syntax error.
    #[must_use]
    pub fn detail(&self) -> Option<String> {
        match self {
            Self::Syntax(message) => Some(message.clone()),
            Self::Missing | Self::NoIdentifier => None,
        }
    }
}

/// A compiled pattern with an `identifier` group.
#[derive(Debug, Clone)]
pub struct FilePattern {
    regex: Regex,
    groups: Vec<String>,
}

/// One named group that took part in a match.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    pub text: String,
    /// Start and end in UTF-16 code units, so JavaScript can slice the path.
    pub utf16: (usize, usize),
}

/// The named groups of one matching path, by name.
pub type Groups = BTreeMap<String, Group>;

impl FilePattern {
    /// Compile the plan's pattern.
    ///
    /// # Errors
    /// [`PatternError::Missing`] for no pattern, [`PatternError::Syntax`] for
    /// one the `regex` crate refuses, [`PatternError::NoIdentifier`] for one
    /// without an `identifier` group.
    pub fn compile(source: Option<&str>) -> Result<Self, PatternError> {
        let source = source.ok_or(PatternError::Missing)?;
        let regex = Regex::new(source).map_err(|error| PatternError::Syntax(error.to_string()))?;
        let groups: Vec<String> = regex.capture_names().flatten().map(String::from).collect();
        if !groups.iter().any(|name| name == IDENTIFIER) {
            return Err(PatternError::NoIdentifier);
        }
        Ok(Self { regex, groups })
    }

    /// The group names in the order the pattern writes them.
    #[must_use]
    pub fn group_names(&self) -> &[String] {
        &self.groups
    }

    /// The groups of `path`, or `None` when the path does not match or the
    /// identifier group is empty.
    #[must_use]
    pub fn groups(&self, path: &str) -> Option<Groups> {
        let captures = self.regex.captures(path)?;
        let mut groups = Groups::new();
        for name in &self.groups {
            if let Some(found) = captures.name(name) {
                let start = utf16_len(&path[..found.start()]);
                let end = start + utf16_len(found.as_str());
                groups.insert(
                    name.clone(),
                    Group {
                        text: found.as_str().to_string(),
                        utf16: (start, end),
                    },
                );
            }
        }
        groups
            .get(IDENTIFIER)
            .is_some_and(|group| !group.text.is_empty())
            .then_some(groups)
    }
}

fn utf16_len(text: &str) -> usize {
    text.encode_utf16().count()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;

    #[test]
    fn a_match_gives_its_groups_with_utf16_offsets() {
        let pattern = FilePattern::compile(Some(
            r"^(?<set>[^/]+)/(?<identifier>[^_]+)(?:_(?<part>[a-z]+))?\.jpg$",
        ))
        .expect("compile");
        assert_eq!(pattern.group_names(), ["set", "identifier", "part"]);
        let groups = pattern.groups("Fotos😀/Ä001_front.jpg").expect("match");
        assert_eq!(groups["set"].utf16, (0, 7));
        assert_eq!(groups["identifier"].text, "Ä001");
        assert_eq!(groups["identifier"].utf16, (8, 12));
        assert_eq!(groups["part"].utf16, (13, 18));
        let plain = pattern.groups("Fotos/Ä002.jpg").expect("match");
        assert!(!plain.contains_key("part"));
        assert!(pattern.groups("Fotos/x.png").is_none());
    }

    #[test]
    fn the_errors_name_what_is_wrong() {
        assert_eq!(
            FilePattern::compile(None).err(),
            Some(PatternError::Missing)
        );
        assert_eq!(
            FilePattern::compile(Some("^(?<name>[^/]+)$")).err(),
            Some(PatternError::NoIdentifier)
        );
        for rust_refuses in [
            r"^(?<identifier>a)(?=b)",
            r"^(?<identifier>a)\1$",
            "^(?<identifier>a",
        ] {
            let error = FilePattern::compile(Some(rust_refuses)).err();
            assert!(
                matches!(error, Some(PatternError::Syntax(_))),
                "{rust_refuses}: {error:?}"
            );
        }
    }

    #[test]
    fn an_empty_identifier_is_no_match() {
        let pattern = FilePattern::compile(Some(r"^(?<identifier>[0-9]*)x$")).expect("compile");
        assert!(pattern.groups("x").is_none());
        assert!(pattern.groups("12x").is_some());
    }
}
