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

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

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
