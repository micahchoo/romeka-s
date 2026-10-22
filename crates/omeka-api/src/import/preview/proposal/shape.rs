//! Places, key shapes and the pattern writer (design sections 2 and 3).
//!
//! A place is one run of letters or one run of digits in a name; every
//! other character only separates places. The writer takes a key, never
//! text: the fragments and the regex syntax below are the only text it
//! holds, and `lint_tests` keeps it so. The same two class fragments split
//! names into places and write patterns, so what a detector counts and what
//! the pattern matches cannot drift apart.

use std::sync::LazyLock;

use regex::Regex;

use crate::import::grouping::pattern::{IDENTIFIER, PART};
use crate::import::plan::NAME_ALONE;

const LETTERS: &str = r"[\p{L}\p{M}]+";
const DIGITS: &str = r"\p{Nd}+";
const SEPARATORS: &str = r"[^\p{L}\p{M}\p{Nd}/]*";
/// Two places of one class are two places only with a separator between.
const SEPARATOR: &str = r"[^\p{L}\p{M}\p{Nd}/]+";
const FOLDERS: &str = r"(?:[^/]+/)*";
const FOLDER: &str = r"[^/]+";
const EXTENSION: &str = r"\.[^./]+";

const START: &str = "^";
const END: &str = "$";
const SLASH: &str = "/";
const GROUP: &str = "(";
const NAMED: &str = "(?<";
const NAMED_END: &str = ">";
const UNNAMED: &str = "(?:";
const CLOSE: &str = ")";
const CLOSE_OPTIONAL: &str = ")?";
const OR: &str = "|";

/// Letters, or digits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PlaceClass {
    Letters,
    Digits,
}

impl PlaceClass {
    fn fragment(self) -> &'static str {
        match self {
            Self::Letters => LETTERS,
            Self::Digits => DIGITS,
        }
    }
}

/// One place of a name: its class and its bytes in the text it was cut from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Place {
    pub class: PlaceClass,
    pub start: usize,
    pub end: usize,
}

static PLACES: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&[GROUP, LETTERS, CLOSE, OR, GROUP, DIGITS, CLOSE].concat())
        .unwrap_or_else(|_| unreachable!())
});

/// The places of `text`, in order.
pub fn places(text: &str) -> Vec<Place> {
    PLACES
        .captures_iter(text)
        .filter_map(|found| {
            let class = if found.get(1).is_some() {
                PlaceClass::Letters
            } else {
                PlaceClass::Digits
            };
            let all = found.get(0)?;
            Some(Place {
                class,
                start: all.start(),
                end: all.end(),
            })
        })
        .collect()
}

/// The classes of a key's places, in order.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct KeyShape(Vec<PlaceClass>);

impl KeyShape {
    /// The shape of these places.
    pub fn of(places: &[Place]) -> Self {
        Self(places.iter().map(|place| place.class).collect())
    }

    /// Its number of places.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether a name with these places starts with this shape.
    pub fn starts(&self, places: &[Place]) -> bool {
        places.len() >= self.0.len()
            && self
                .0
                .iter()
                .zip(places)
                .all(|(class, place)| *class == place.class)
    }
}

/// Where a series' item key ends, with what the pattern needs to say it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Key {
    /// Each name is one item.
    Whole,
    /// The key is the name's first places, of this shape.
    Places(KeyShape),
    /// The key is the folder this many levels under the scope.
    Folder(u8),
}

impl Key {
    /// The series pattern. `Places`: any folders, then the key's places with
    /// any separators between them, then the parts, then the extension.
    /// `Folder`: the key folders, then the rest of the path as the part.
    pub fn pattern(&self) -> String {
        match self {
            Self::Whole => NAME_ALONE.to_owned(),
            Self::Places(shape) => {
                let mut out = vec![START, FOLDERS, SEPARATORS, NAMED, IDENTIFIER, NAMED_END];
                let mut before: Option<PlaceClass> = None;
                for class in &shape.0 {
                    match before {
                        Some(last) if last == *class => out.push(SEPARATOR),
                        Some(_) => out.push(SEPARATORS),
                        None => {}
                    }
                    out.push(class.fragment());
                    before = Some(*class);
                }
                out.extend([
                    CLOSE,
                    UNNAMED,
                    SEPARATORS,
                    NAMED,
                    PART,
                    NAMED_END,
                    FOLDER,
                    CLOSE,
                    CLOSE_OPTIONAL,
                    SEPARATORS,
                    EXTENSION,
                    END,
                ]);
                out.concat()
            }
            Self::Folder(depth) => {
                let mut out = vec![START, NAMED, IDENTIFIER, NAMED_END, FOLDER];
                for _ in 1..*depth {
                    out.extend([SLASH, FOLDER]);
                }
                out.extend([
                    CLOSE, SLASH, NAMED, PART, NAMED_END, FOLDERS, FOLDER, CLOSE, EXTENSION, END,
                ]);
                out.concat()
            }
        }
    }
}
