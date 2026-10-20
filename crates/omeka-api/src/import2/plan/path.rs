//! Paths relative to a source: the only path type the plan holds.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use unicode_normalization::UnicodeNormalization;

/// A path relative to a source, `/`-separated, stored NFC.
///
/// The empty path is the source itself. Every path member of the plan is
/// one, so a path that could leave its source never gets past the boundary.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RelPath(String);

/// Why a path is not a [`RelPath`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RelPathError {
    #[error("a path segment is \"..\"")]
    ParentSegment,
    #[error("the path holds a NUL")]
    Nul,
    #[error("the path holds a \"\\\"")]
    Backslash,
    #[error("the path starts with \"/\"")]
    Absolute,
}

impl RelPath {
    /// # Errors
    /// A [`RelPathError`] when the text, its NFC form, or either of them
    /// percent-decoded once has a `..` segment, a NUL, a `\` or a leading `/`.
    pub fn parse(text: &str) -> Result<Self, RelPathError> {
        let nfc: String = text.nfc().collect();
        for form in [text, nfc.as_str()] {
            refuse(form.as_bytes())?;
            refuse(&percent_decode(form.as_bytes()))?;
        }
        Ok(Self(nfc))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Works on bytes because a decoded `%FF` need not be UTF-8, and a path
/// that is not UTF-8 after decoding must still be checked.
fn refuse(bytes: &[u8]) -> Result<(), RelPathError> {
    if bytes.contains(&0) {
        return Err(RelPathError::Nul);
    }
    if bytes.contains(&b'\\') {
        return Err(RelPathError::Backslash);
    }
    if bytes.first() == Some(&b'/') {
        return Err(RelPathError::Absolute);
    }
    if bytes
        .split(|byte| *byte == b'/')
        .any(|segment| segment == b"..")
    {
        return Err(RelPathError::ParentSegment);
    }
    Ok(())
}

/// One pass of `%XX` decoding. A `%` not followed by two hex digits stays.
fn percent_decode(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%'
            && let (Some(high), Some(low)) = (
                bytes.get(index + 1).and_then(|b| hex(*b)),
                bytes.get(index + 2).and_then(|b| hex(*b)),
            )
        {
            out.push(high << 4 | low);
            index += 3;
        } else {
            out.push(bytes[index]);
            index += 1;
        }
    }
    out
}

fn hex(byte: u8) -> Option<u8> {
    char::from(byte)
        .to_digit(16)
        .and_then(|digit| u8::try_from(digit).ok())
}

impl fmt::Display for RelPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for RelPath {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for RelPath {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Self::parse(&text)
            .map_err(|error| serde::de::Error::custom(format!("path {text:?}: {error}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_paths_of_any_script_are_kept_nfc() {
        let decomposed = "Box 1/Cafe\u{301}/0001.tif";
        let path = RelPath::parse(decomposed).map(|p| p.as_str().to_string());
        assert_eq!(path, Ok("Box 1/Caf\u{e9}/0001.tif".to_string()));
        assert!(RelPath::parse("").is_ok());
        assert!(RelPath::parse("दस्तावेज़/..a/b..").is_ok());
        assert!(RelPath::parse("100%/a%2").is_ok());
    }

    #[test]
    fn paths_that_leave_the_source_are_refused_before_and_after_decoding() {
        let cases = [
            ("..", RelPathError::ParentSegment),
            ("a/../b", RelPathError::ParentSegment),
            ("a/%2e%2E/b", RelPathError::ParentSegment),
            ("a/.%2e", RelPathError::ParentSegment),
            ("a\0b", RelPathError::Nul),
            ("a%00b", RelPathError::Nul),
            ("a\\b", RelPathError::Backslash),
            ("a%5cb", RelPathError::Backslash),
            ("/etc", RelPathError::Absolute),
            ("%2Fetc", RelPathError::Absolute),
        ];
        for (text, expected) in cases {
            assert_eq!(RelPath::parse(text), Err(expected), "{text:?}");
        }
    }
}
