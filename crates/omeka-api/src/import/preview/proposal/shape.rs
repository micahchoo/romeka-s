//! Places and key shapes (design section 2).
//!
//! A place is one run of letters or one run of digits in a name; every
//! other character only separates places. The class fragments below are
//! text `lint_tests` allows in this file only.

use std::sync::LazyLock;

use regex::Regex;

const LETTERS: &str = r"[\p{L}\p{M}]+";
const DIGITS: &str = r"\p{Nd}+";
const GROUP: &str = "(";
const CLOSE: &str = ")";
const OR: &str = "|";

/// Letters, or digits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PlaceClass {
    Letters,
    Digits,
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
}
