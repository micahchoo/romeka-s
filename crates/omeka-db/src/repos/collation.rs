//! The port's one rule for ordering text the way upstream does.
//!
//! Upstream reads and orders every text column through MySQL's `utf8mb4_unicode_ci` collation, so
//! `ORDER BY title asc` puts `annotated note` before `Item with media rows`. SQLite's default is
//! byte-wise, which puts `Item` first because `I` (0x49) is below `a` (0x61). Every sorter in the
//! port calls the comparison here so they all fold case the same way.
//!
//! The rule maps the ASCII letters `A-Z` to `a-z` and compares the mapped bytes. That is exactly
//! SQLite's built-in `COLLATE NOCASE` (and SQLite's `sqlite3StrICmp`), so a Rust sorter and a
//! `COLLATE NOCASE` clause in a repository statement produce the same order. It does not reach
//! what `utf8mb4_unicode_ci` reaches beyond the ASCII range:
//!
//! - accents stay distinct, so `A` and `Á` never tie, where the upstream collation folds the accent
//!   away and ties them;
//! - a non-ASCII case pair stays distinct, so `İ` and `i` do not tie, where the upstream collation
//!   folds them.
//!
//! Closing either gap needs a custom collation. `SCHEMA-NOTES.md` divergence 7 records the same
//! limit for the column collations the migration carries.
//!
//! [`compare_natural`] departs from upstream on purpose: it reads a digit run as a number, so a
//! resource list sorted by title or by a field puts `AF_2` before `AF_10` (`docs/compat.md`).

use std::cmp::Ordering;

/// Order two strings as upstream's collation orders them over the ASCII range.
///
/// The loop compares mapped bytes and allocates nothing, because a list sort calls this once per
/// comparison.
pub fn compare_text_ci(left: &str, right: &str) -> Ordering {
    let mut left = left.bytes();
    let mut right = right.bytes();
    loop {
        match (left.next(), right.next()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(left), Some(right)) => {
                match left.to_ascii_lowercase().cmp(&right.to_ascii_lowercase()) {
                    Ordering::Equal => {}
                    order => return order,
                }
            }
        }
    }
}

/// [`compare_text_ci`] over a nullable text column.
///
/// A missing value sorts first, which is MySQL's ascending `ORDER BY` treatment of `NULL` and the
/// order the port's `Option::cmp` already uses, so a title sort leaves untitled rows at the top.
pub fn compare_opt_text_ci(left: Option<&str>, right: Option<&str>) -> Ordering {
    match (left, right) {
        (Some(left), Some(right)) => compare_text_ci(left, right),
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Less,
        (Some(_), None) => Ordering::Greater,
    }
}

/// Order two strings the way a shelf orders call numbers: a run of ASCII digits compares as the
/// number it spells, and everything else compares as [`compare_text_ci`] does. So `AF_2` comes
/// before `AF_10`, and `Box 2` before `box 10`.
///
/// Leading zeros do not count, so `a01` and `a1` tie; the caller breaks a tie (a list sort falls
/// back to the id). A digit run of any length compares without overflow: by its length once the
/// zeros are gone, then digit by digit.
pub fn compare_natural(left: &str, right: &str) -> Ordering {
    let (mut left, mut right) = (left.as_bytes(), right.as_bytes());
    loop {
        match (left.first(), right.first()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(l), Some(r)) if l.is_ascii_digit() && r.is_ascii_digit() => {
                let (l_run, l_rest) = split_digits(left);
                let (r_run, r_rest) = split_digits(right);
                match compare_digit_runs(l_run, r_run) {
                    Ordering::Equal => (left, right) = (l_rest, r_rest),
                    order => return order,
                }
            }
            (Some(l), Some(r)) => match l.to_ascii_lowercase().cmp(&r.to_ascii_lowercase()) {
                Ordering::Equal => (left, right) = (&left[1..], &right[1..]),
                order => return order,
            },
        }
    }
}

fn split_digits(text: &[u8]) -> (&[u8], &[u8]) {
    let end = text
        .iter()
        .position(|byte| !byte.is_ascii_digit())
        .unwrap_or(text.len());
    text.split_at(end)
}

fn compare_digit_runs(left: &[u8], right: &[u8]) -> Ordering {
    let significant = |run: &[u8]| {
        run.iter()
            .position(|digit| *digit != b'0')
            .unwrap_or(run.len())
    };
    let (left, right) = (&left[significant(left)..], &right[significant(right)..]);
    left.len().cmp(&right.len()).then_with(|| left.cmp(right))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    fn shelved(mut words: Vec<&str>) -> Vec<&str> {
        words.sort_by(|left, right| compare_natural(left, right));
        words
    }

    #[test]
    fn a_digit_run_compares_as_its_number() {
        assert_eq!(
            shelved(vec!["AF_10", "AF_2", "AF_1", "AF_100"]),
            ["AF_1", "AF_2", "AF_10", "AF_100"]
        );
        assert_eq!(
            shelved(vec!["1950", "c. 1950", "950"]),
            ["950", "1950", "c. 1950"]
        );
    }

    #[test]
    fn natural_order_folds_case_like_the_collation() {
        assert_eq!(
            shelved(vec!["box 10", "Box 2", "apple"]),
            ["apple", "Box 2", "box 10"]
        );
        assert_eq!(compare_natural("AF_2", "af_2"), Ordering::Equal);
    }

    #[test]
    fn leading_zeros_tie_and_long_runs_never_overflow() {
        assert_eq!(compare_natural("a01", "a1"), Ordering::Equal);
        assert_eq!(compare_natural("0", "00"), Ordering::Equal);
        assert_eq!(
            compare_natural("99999999999999999999999999", "100000000000000000000000000"),
            Ordering::Less
        );
    }

    #[test]
    fn a_prefix_sorts_before_the_longer_text() {
        assert_eq!(compare_natural("AF", "AF_1"), Ordering::Less);
        assert_eq!(compare_natural("AF_1", "AF_1a"), Ordering::Less);
    }

    #[test]
    fn folds_case_across_the_ascii_gap_like_the_oracle() {
        // Upstream order measured on the oracle, 2026-10-17:
        // `apple` < `Banana` < `Zebra`.
        let mut words = ["Zebra", "apple", "Banana"];
        words.sort_by(|left, right| compare_text_ci(left, right));
        assert_eq!(words, ["apple", "Banana", "Zebra"]);
    }

    #[test]
    fn a_case_only_difference_ties() {
        assert_eq!(compare_text_ci("AnnotATED", "annotated"), Ordering::Equal);
    }

    #[test]
    fn a_missing_value_sorts_first() {
        assert_eq!(compare_opt_text_ci(None, Some("annotated")), Ordering::Less);
        assert_eq!(
            compare_opt_text_ci(Some("annotated"), None),
            Ordering::Greater
        );
        assert_eq!(compare_opt_text_ci(None, None), Ordering::Equal);
    }

    #[test]
    fn a_non_ascii_case_pair_stays_distinct() {
        // The documented gap: upstream's `utf8mb4_unicode_ci` folds this pair, the port does not.
        assert_ne!(compare_text_ci("İ", "i"), Ordering::Equal);
    }

    #[test]
    fn a_longer_string_with_an_equal_prefix_sorts_after_it() {
        assert_eq!(compare_text_ci("item", "items"), Ordering::Less);
    }
}
