//! Places, key shapes and the pattern writer (design sections 2 and 3).
//!
//! A place is one run of letters or one run of digits in a name; every
//! other character only separates places. The writer takes a key, never
//! text: the fragments and the regex syntax below are the only text it
//! holds, and `lint_tests` keeps it so. The two class fragments are
//! `import::places`', which splits names into places for the detectors and
//! for the join's `lastPlaces` fold, so what a detector counts, what the
//! pattern matches and what the join compares cannot drift apart.

use crate::import::grouping::pattern::{IDENTIFIER, PART, SET};
pub use crate::import::places::{Place, PlaceClass, places};
use crate::import::plan::NAME_ALONE;

const SEPARATORS: &str = r"[^\p{L}\p{M}\p{Nd}/]*";
/// Two places of one class are two places only with a separator between.
const SEPARATOR: &str = r"[^\p{L}\p{M}\p{Nd}/]+";
const FOLDERS: &str = r"(?:[^/]+/)*";
const FOLDER: &str = r"[^/]+";
const EXTENSION: &str = r"\.[^./]+";
/// The rest of a name, as little as can be.
const REST: &str = r"[^/]*?";
/// An ending as `NAME_ALONE` reads it: a dot and anything but a dot.
const ENDING: &str = r"(?:\.[^./]*)?";

const START: &str = "^";
const END: &str = "$";
/// Between folder names, in a pattern and in a key.
pub const SLASH: &str = "/";
const NAMED: &str = "(?<";
const NAMED_END: &str = ">";
const UNNAMED: &str = "(?:";
const CLOSE: &str = ")";
const CLOSE_OPTIONAL: &str = ")?";

/// The classes of a key's places, in order.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct KeyShape(Vec<PlaceClass>);

impl KeyShape {
    /// The shape of these places.
    pub fn of(places: &[Place]) -> Self {
        Self(places.iter().map(|place| place.class).collect())
    }

    /// The shape of places of these classes.
    pub fn of_classes(classes: impl IntoIterator<Item = PlaceClass>) -> Self {
        Self(classes.into_iter().collect())
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
    /// Each file is one item, keyed by its folders and its name, so the same
    /// name in two folders is two items.
    Path,
    /// The key is the name's first places, of this shape.
    Places(KeyShape),
    /// The key is the folder `depth` levels under the scope. With `lead`,
    /// only that folder name's first places, of this shape: a list names a
    /// folder `B01-F03 Letters 1950` by `B01-F03`.
    Folder { depth: u8, lead: Option<KeyShape> },
}

impl Key {
    /// The series pattern. `Places`: any folders, then the key's places with
    /// any separators between them, then the parts, then the extension.
    /// `Folder`: the key folders, then the rest of the path as the part.
    pub fn pattern(&self) -> String {
        self.written(None)
    }

    /// The series pattern with the place `set` ends at also captured as the
    /// group [`SET`]: `set` is the classes of the key's places up to that
    /// place. A `Folder` key has no name place, so its pattern is the plain
    /// one.
    pub fn pattern_with_set(&self, set: &KeyShape) -> String {
        self.written(Some(set))
    }

    fn written(&self, set: Option<&KeyShape>) -> String {
        match self {
            Self::Whole => match set {
                None => NAME_ALONE.to_owned(),
                // The key is still the whole name before its ending, as
                // `NAME_ALONE` reads it, so the items are the same.
                Some(set) => {
                    let mut out = vec![START, FOLDERS, NAMED, IDENTIFIER, NAMED_END, SEPARATORS];
                    write_places(&mut out, &set.0, set.0.len().checked_sub(1));
                    out.extend([REST, CLOSE, ENDING, END]);
                    out.concat()
                }
            },
            // A name place cannot be read beside the folders, so a set
            // place is not captured.
            Self::Path => [
                START, NAMED, IDENTIFIER, NAMED_END, FOLDERS, REST, CLOSE, ENDING, END,
            ]
            .concat(),
            Self::Places(shape) => {
                let mut out = vec![START, FOLDERS, SEPARATORS, NAMED, IDENTIFIER, NAMED_END];
                let set_at = set.and_then(|set| set.0.len().checked_sub(1));
                write_places(&mut out, &shape.0, set_at);
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
            Self::Folder { depth, lead: None } => {
                let mut out = vec![START, NAMED, IDENTIFIER, NAMED_END, FOLDER];
                for _ in 1..*depth {
                    out.extend([SLASH, FOLDER]);
                }
                out.extend([
                    CLOSE, SLASH, NAMED, PART, NAMED_END, FOLDERS, FOLDER, CLOSE, EXTENSION, END,
                ]);
                out.concat()
            }
            Self::Folder {
                depth,
                lead: Some(lead),
            } => {
                let mut out = vec![START];
                for _ in 1..*depth {
                    out.extend([FOLDER, SLASH]);
                }
                out.extend([SEPARATORS, NAMED, IDENTIFIER, NAMED_END]);
                write_places(&mut out, &lead.0, None);
                out.extend([
                    CLOSE, REST, SLASH, NAMED, PART, NAMED_END, FOLDERS, FOLDER, CLOSE, EXTENSION,
                    END,
                ]);
                out.concat()
            }
        }
    }
}

/// Places of these classes with the separators between them; the place
/// `set_at` as the group [`SET`].
fn write_places(out: &mut Vec<&'static str>, classes: &[PlaceClass], set_at: Option<usize>) {
    let mut before: Option<PlaceClass> = None;
    for (at, class) in classes.iter().enumerate() {
        match before {
            Some(last) if last == *class => out.push(SEPARATOR),
            Some(_) => out.push(SEPARATORS),
            None => {}
        }
        if Some(at) == set_at {
            out.extend([NAMED, SET, NAMED_END, class.fragment(), CLOSE]);
        } else {
            out.push(class.fragment());
        }
        before = Some(*class);
    }
}
