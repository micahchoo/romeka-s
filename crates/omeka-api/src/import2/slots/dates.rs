//! Dates: which values are dates, and whether a date is EDTF level 0 or 1
//! (`docs/import-v2.md`, "Slot rules a worker must not guess", Dates).
//!
//! A value that is not EDTF is kept as written and flagged `date-form`; an
//! EDTF value is stored as written. Level 0 is ISO 8601 dates, date-times
//! and intervals of dates. Level 1 adds a `Y` year of more than four digits,
//! seasons (`21`–`24`), one trailing `?`, `~` or `%`, unspecified digits
//! (`201X`, `20XX`, `2004-XX`, `1985-04-XX`, `1985-XX-XX`), negative years,
//! and intervals with an open (`..`) or unknown (empty) end.

/// The nine Dublin Core date terms.
pub const DATE_TERMS: [&str; 9] = [
    "dcterms:date",
    "dcterms:created",
    "dcterms:issued",
    "dcterms:modified",
    "dcterms:valid",
    "dcterms:available",
    "dcterms:dateAccepted",
    "dcterms:dateCopyrighted",
    "dcterms:dateSubmitted",
];

/// Whether a data type name is a date type (`numeric:timestamp`,
/// `numeric:interval`, or any type that names a date).
#[must_use]
pub fn is_date_type(data_type: &str) -> bool {
    let name = data_type.to_ascii_lowercase();
    ["timestamp", "interval", "date"]
        .iter()
        .any(|part| name.contains(part))
}

#[must_use]
pub fn is_edtf(text: &str) -> bool {
    match text.split_once('/') {
        Some((start, end)) => {
            let side = |side: &str| side.is_empty() || side == ".." || date(side);
            !end.contains('/')
                && side(start)
                && side(end)
                && !(start.is_empty() && end.is_empty())
                && !(start == ".." && end == "..")
                && !(start.is_empty() && end == "..")
                && !(start == ".." && end.is_empty())
        }
        None => date(text) || date_time(text),
    }
}

/// A level 1 date, with at most one trailing qualifier.
fn date(text: &str) -> bool {
    let text = text.strip_suffix(['?', '~', '%']).unwrap_or(text);
    if let Some(digits) = text.strip_prefix('Y') {
        let digits = digits.strip_prefix('-').unwrap_or(digits);
        return digits.len() > 4 && digits.bytes().all(|byte| byte.is_ascii_digit());
    }
    let (negative, text) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text),
    };
    let mut parts = text.split('-');
    let year = parts.next().unwrap_or_default();
    let month = parts.next();
    let day = parts.next();
    if parts.next().is_some() || !year_ok(year, negative) {
        return false;
    }
    let year_unspecified = year.contains('X');
    match (month, day) {
        (None, None) => true,
        (Some(month), None) => {
            !year_unspecified
                && (month == "XX"
                    || month_number(month)
                        .is_some_and(|m| (1..=12).contains(&m) || (21..=24).contains(&m)))
        }
        (Some(month), Some(day)) => {
            if year_unspecified {
                return false;
            }
            if month == "XX" {
                return day == "XX";
            }
            let Some(month) = month_number(month).filter(|m| (1..=12).contains(m)) else {
                return false;
            };
            day == "XX"
                || month_number(day)
                    .is_some_and(|d| d >= 1 && d <= days_in(year.parse().unwrap_or(0), month))
        }
        (None, Some(_)) => false,
    }
}

/// Four digits; the last one or two may be `X` (not on a negative year).
fn year_ok(year: &str, negative: bool) -> bool {
    let bytes = year.as_bytes();
    if bytes.len() != 4 {
        return false;
    }
    let unspecified = bytes.iter().rev().take_while(|byte| **byte == b'X').count();
    let digits = &bytes[..4 - unspecified];
    unspecified <= 2 && !(negative && unspecified > 0) && digits.iter().all(u8::is_ascii_digit)
}

fn month_number(text: &str) -> Option<u32> {
    (text.len() == 2 && text.bytes().all(|byte| byte.is_ascii_digit()))
        .then(|| text.parse().ok())
        .flatten()
}

fn days_in(year: i64, month: u32) -> u32 {
    match month {
        4 | 6 | 9 | 11 => 30,
        2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
        2 => 28,
        _ => 31,
    }
}

/// A level 0 date and time: `YYYY-MM-DDThh:mm:ss`, then `Z` or `±hh[:mm]`.
fn date_time(text: &str) -> bool {
    let Some((day, time)) = text.split_once('T') else {
        return false;
    };
    let plain_day =
        day.len() == 10 && !day.contains('X') && date(day) && !day.ends_with(['?', '~', '%']);
    if !plain_day {
        return false;
    }
    let (clock, zone) = match time.find(['Z', '+', '-']) {
        Some(at) => time.split_at(at),
        None => (time, ""),
    };
    let fields: Vec<&str> = clock.split(':').collect();
    let two = |text: &str, max: u32| month_number(text).is_some_and(|n| n <= max);
    let clock_ok =
        fields.len() == 3 && two(fields[0], 23) && two(fields[1], 59) && two(fields[2], 59);
    let zone_ok = match zone {
        "" | "Z" => true,
        offset => {
            let offset = &offset[1..];
            match offset.split_once(':') {
                Some((hours, minutes)) => two(hours, 14) && two(minutes, 59),
                None => two(offset, 14),
            }
        }
    };
    clock_ok && zone_ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a62_edtf_level_0_and_1_pass_and_everything_else_is_flagged() {
        let edtf = [
            "1890",
            "1890-03",
            "1890-03-12",
            "2000-02-29",
            "1985-04-12T23:20:30",
            "1985-04-12T23:20:30Z",
            "1985-04-12T23:20:30+05:30",
            "1964/2008",
            "2004-06/2006-08",
            "Y170000002",
            "Y-170000002",
            "2001-21",
            "2001-24",
            "1984?",
            "2004-06~",
            "2004-06-11%",
            "201X",
            "20XX",
            "2004-XX",
            "1985-04-XX",
            "1985-XX-XX",
            "-1985",
            "../1985-04-12",
            "1985-04-12/..",
            "/1985-04-12",
            "1985-04-12/",
            "1984~/2004-06",
        ];
        for text in edtf {
            assert!(is_edtf(text), "{text} is EDTF");
        }
        let not = [
            "c. 1890",
            "1890s",
            "189",
            "31/12/1890",
            "n.d.",
            "1890-13",
            "1890-02-30",
            "1900-02-29",
            "2001-25",
            "2001-21-01",
            "2XXX",
            "201X-05",
            "1985-XX-12",
            "Y1234",
            "1890??",
            "",
            "/",
            "../..",
            "1985-04-12T25:00:00",
            "1985-04-12 10:00:00",
            "1964/2008/2010",
        ];
        for text in not {
            assert!(!is_edtf(text), "{text} is not EDTF");
        }
    }

    #[test]
    fn a63_dates_are_the_nine_dc_terms_and_date_data_types() {
        assert_eq!(DATE_TERMS.len(), 9);
        assert!(is_date_type("numeric:timestamp"));
        assert!(is_date_type("numeric:interval"));
        assert!(!is_date_type("literal"));
    }
}
