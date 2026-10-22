//! Places: a place is one run of letters or one run of digits in a name;
//! every other character only separates places. A letter is any `L` or `M`
//! character, so a Devanagari vowel sign stays in its word; a digit is any
//! `Nd` character, so Tamil digits count. `BHC_006_GMT_AF_140b.JPG` has the
//! places `BHC`, `006`, `GMT`, `AF`, `140`, `b` and `JPG`.
//!
//! The proposer counts places and writes patterns from these two class
//! fragments, and the join's `lastPlaces` fold compares by them, so the
//! three cannot drift apart.

use std::sync::LazyLock;

use regex::Regex;

/// The regex fragment of a letter place.
pub const LETTERS: &str = r"[\p{L}\p{M}]+";
/// The regex fragment of a digit place.
pub const DIGITS: &str = r"\p{Nd}+";

/// Letters, or digits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PlaceClass {
    Letters,
    Digits,
}

impl PlaceClass {
    /// The regex fragment of a place of this class.
    #[must_use]
    pub fn fragment(self) -> &'static str {
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
    Regex::new(&["(", LETTERS, ")|(", DIGITS, ")"].concat()).unwrap_or_else(|_| unreachable!())
});

/// The places of `text`, in order.
#[must_use]
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

/// A file name with its folders and its ending left out: the text the
/// places of a name are counted in.
#[must_use]
pub fn stem(path: &str) -> &str {
    let name = path.rsplit('/').next().unwrap_or(path);
    match name.rsplit_once('.') {
        Some((stem, _)) if !stem.is_empty() => stem,
        _ => name,
    }
}

/// The last `count` places of a file name, its folders and its ending left
/// out, joined by `/`, which no place holds. Empty when the name has fewer
/// places. `Documents/BHC006_KHAN_Donor20170616_D01.JPG` and
/// `BHC006_GMT_D01.JPG` both end in `D/01`.
#[must_use]
pub fn last_places(path: &str, count: usize) -> String {
    let stem = stem(path);
    let found = places(stem);
    let Some(from) = found.len().checked_sub(count).filter(|_| count > 0) else {
        return String::new();
    };
    let texts: Vec<&str> = found[from..]
        .iter()
        .map(|place| &stem[place.start..place.end])
        .collect();
    texts.join("/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_last_places_leave_out_folders_and_the_ending() {
        assert_eq!(
            last_places("Documents (D)/BHC006_KHAN_Donor20170616_D01.JPG", 2),
            "D/01"
        );
        assert_eq!(last_places("BHC006_GMT_D01.JPG", 2), "D/01");
        assert_eq!(last_places("BHC006_GMT_D01.JPG", 1), "01");
        assert_eq!(last_places("BHC006_GMT_D01", 2), "D/01", "no ending");
        assert_eq!(last_places("D01", 3), "", "fewer places than asked");
        assert_eq!(last_places(".hidden", 1), "hidden");
        assert_eq!(last_places("पत्र_001.jpg", 2), "पत्र/001");
    }
}
