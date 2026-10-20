//! The point in time a timeline places a value at: an EDTF-style date.
//!
//! A value reads as a year, a year and month, or a full date, written the way
//! EDTF (ISO 8601-2) writes them: `1894`, `1894-03`, `1894-03-12`, `-0500`
//! for 500 BCE. The qualifiers `?`, `~` and `%` (uncertain, approximate) are
//! read past, an unspecified digit `X` reads as 0 (`189X` sorts as 1890), and
//! an interval `1890/1900` sorts by its start. Anything else is not a date,
//! and the timeline lists it at the end.

use std::cmp::Ordering;

/// A sortable date: a year, then an optional month and day.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Date {
    year: i64,
    month: Option<u8>,
    day: Option<u8>,
}

impl Date {
    /// The date a value's text holds, `None` when it holds none.
    pub(super) fn parse(text: &str) -> Option<Self> {
        let start = text.trim().split('/').next()?.trim();
        let start = start.trim_end_matches(['?', '~', '%']);
        let (negative, rest) = match start.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, start),
        };
        let mut parts = rest.split('-');
        let year_text = parts.next()?;
        if year_text.len() < 4 || !year_text.chars().all(|c| c.is_ascii_digit() || c == 'X') {
            return None;
        }
        let year: i64 = year_text.replace('X', "0").parse().ok()?;
        let month = match parts.next() {
            Some(part) => bounded(part, 12)?,
            None => None,
        };
        let day = match parts.next() {
            Some(part) => bounded(part, 31)?,
            None => None,
        };
        if parts.next().is_some() || (day.is_some() && month.is_none()) {
            return None;
        }
        Some(Self {
            year: if negative { -year } else { year },
            month,
            day,
        })
    }
}

/// A two-digit month or day from 1 to `max`, `Some(None)` for an unspecified
/// `XX`, `None` for anything else.
fn bounded(part: &str, max: u8) -> Option<Option<u8>> {
    let part = part.trim_end_matches(['?', '~', '%']);
    if part == "XX" {
        return Some(None);
    }
    if part.len() != 2 {
        return None;
    }
    let value: u8 = part.parse().ok()?;
    (1..=max).contains(&value).then_some(Some(value))
}

/// Earlier first; a year alone sorts before its months, a month before its
/// days.
impl Ord for Date {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.year, self.month, self.day).cmp(&(other.year, other.month, other.day))
    }
}

impl PartialOrd for Date {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(year: i64, month: Option<u8>, day: Option<u8>) -> Option<Date> {
        Some(Date { year, month, day })
    }

    /// The EDTF forms a curator writes parse; the qualifiers and the interval
    /// end are read past.
    #[test]
    fn edtf_forms_parse() {
        assert_eq!(Date::parse("1894"), date(1894, None, None));
        assert_eq!(Date::parse(" 1894-03 "), date(1894, Some(3), None));
        assert_eq!(Date::parse("1894-03-12"), date(1894, Some(3), Some(12)));
        assert_eq!(Date::parse("1894?"), date(1894, None, None));
        assert_eq!(Date::parse("1894-03~"), date(1894, Some(3), None));
        assert_eq!(Date::parse("189X"), date(1890, None, None));
        assert_eq!(Date::parse("1894-XX-XX"), date(1894, None, None));
        assert_eq!(Date::parse("1890/1900"), date(1890, None, None));
        assert_eq!(Date::parse("-0500"), date(-500, None, None));
        assert_eq!(Date::parse("10000"), date(10000, None, None));
    }

    /// Free text, a short year and an impossible month or day are not dates.
    #[test]
    fn other_text_is_not_a_date() {
        for text in [
            "",
            "circa 1894",
            "March 1894",
            "94",
            "1894-13",
            "1894-02-32",
            "1894-3",
            "1894-03-12-01",
            "unknown",
        ] {
            assert_eq!(Date::parse(text), None, "{text}");
        }
    }

    /// Dates order by year, then month, then day, and a bare year comes
    /// before a dated month of it.
    #[test]
    fn dates_order_from_the_earliest() {
        let mut dates: Vec<Date> = ["1894-03-12", "1894", "-0500", "1894-03", "1700"]
            .iter()
            .filter_map(|text| Date::parse(text))
            .collect();
        dates.sort();
        assert_eq!(
            dates,
            [
                Date::parse("-0500").unwrap(),
                Date::parse("1700").unwrap(),
                Date::parse("1894").unwrap(),
                Date::parse("1894-03").unwrap(),
                Date::parse("1894-03-12").unwrap(),
            ]
        );
    }
}
