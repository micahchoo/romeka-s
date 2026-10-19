//! The locale-aware date formatter behind [`super::Translator::format_datetime`].
//!
//! Upstream's `View\Helper\I18n::dateFormat` prefers PHP `intl` (which formats
//! in the locale) and falls back to a fixed English PHP date pattern when `intl`
//! is absent (`application/src/View/Helper/I18n.php:93-140`). The port has no
//! ICU, so it carries one [`DateNames`] table per locale: the names, and each
//! `date_format`/`time_format` style as PHP date characters. English reproduces
//! the fallback branch. Spanish dates follow ICU's `es` data, the branch `intl`
//! would take, and keep the fallback's times. The Indic tables are generated
//! from ICU (`dates_cldr.rs`) and take ICU's times as well.

use chrono::{Datelike, NaiveDateTime, Timelike};

/// One locale's date names and patterns. Indexed by month (one-based) and
/// weekday (Sunday is zero).
pub(super) struct DateNames {
    pub(super) months: [&'static str; 12],
    pub(super) months_short: [&'static str; 12],
    pub(super) weekdays: [&'static str; 7],
    pub(super) short: &'static str,
    pub(super) medium: &'static str,
    pub(super) long: &'static str,
    pub(super) full: &'static str,
    pub(super) time_short: &'static str,
    pub(super) time_medium: &'static str,
    pub(super) time_long: &'static str,
    pub(super) time_full: &'static str,
    /// What `a` prints before and after noon.
    pub(super) day_periods: [&'static str; 2],
    /// The digits of the locale's default numbering system, zero first.
    pub(super) digits: [char; 10],
}

const LATIN: [char; 10] = ['0', '1', '2', '3', '4', '5', '6', '7', '8', '9'];

/// The English fallback branch: `I18n::dateFormat`'s patterns and the language's
/// own names.
pub(super) static EN_NAMES: DateNames = DateNames {
    months: [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ],
    months_short: [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ],
    weekdays: [
        "Sunday",
        "Monday",
        "Tuesday",
        "Wednesday",
        "Thursday",
        "Friday",
        "Saturday",
    ],
    short: "n/j/y",
    medium: "M j, Y",
    long: "F j, Y",
    full: "l, F j, Y",
    time_short: "g:ia",
    time_medium: "g:ia",
    time_long: "g:i:sa",
    time_full: "g:i:sa T",
    day_periods: ["am", "pm"],
    digits: LATIN,
};

/// The Spanish names and ICU `es` date patterns, the branch `intl` would format
/// in. Times keep the fallback's patterns.
pub(super) static ES_NAMES: DateNames = DateNames {
    months: [
        "enero",
        "febrero",
        "marzo",
        "abril",
        "mayo",
        "junio",
        "julio",
        "agosto",
        "septiembre",
        "octubre",
        "noviembre",
        "diciembre",
    ],
    months_short: [
        "ene", "feb", "mar", "abr", "may", "jun", "jul", "ago", "sept", "oct", "nov", "dic",
    ],
    weekdays: [
        "domingo",
        "lunes",
        "martes",
        "miércoles",
        "jueves",
        "viernes",
        "sábado",
    ],
    short: "j/n/y",
    medium: "j M Y",
    long: "j \\d\\e F \\d\\e Y",
    full: "l, j \\d\\e F \\d\\e Y",
    time_short: "g:ia",
    time_medium: "g:ia",
    time_long: "g:i:sa",
    time_full: "g:i:sa T",
    day_periods: ["am", "pm"],
    digits: LATIN,
};

/// `I18n::dateFormat`: the `date_format` and `time_format` a `pageDateTime`
/// block stores, applied to a naive timestamp with one locale's table.
pub(super) fn format_datetime(
    value: NaiveDateTime,
    date_format: &str,
    time_format: &str,
    names: &DateNames,
) -> String {
    let date = match date_format {
        "none" => "",
        "short" => names.short,
        "long" => names.long,
        "full" => names.full,
        _ => names.medium,
    };
    let time = match time_format {
        "full" => names.time_full,
        "long" => names.time_long,
        "medium" => names.time_medium,
        "short" => names.time_short,
        _ => "",
    };
    let pattern = format!("{date} {time}");
    format_php_date(value, pattern.trim(), names)
}

/// Apply the PHP date characters the tables use: `l F M j d Y n m y g h G H i s
/// a T`, with a backslash escaping the next character. `T` has no value here and
/// prints nothing. Numbers print in the table's digits.
fn format_php_date(value: NaiveDateTime, pattern: &str, names: &DateNames) -> String {
    let mut out = String::with_capacity(pattern.len());
    let mut chars = pattern.chars();
    let hour12 = match value.hour() % 12 {
        0 => 12,
        hour => hour,
    };
    while let Some(ch) = chars.next() {
        let number = match ch {
            '\\' => {
                if let Some(literal) = chars.next() {
                    out.push(literal);
                }
                continue;
            }
            'l' => {
                out.push_str(weekday_name(value, names));
                continue;
            }
            'F' => {
                out.push_str(month_name(value, names.months));
                continue;
            }
            'M' => {
                out.push_str(month_name(value, names.months_short));
                continue;
            }
            'a' => {
                out.push_str(names.day_periods[usize::from(value.hour() >= 12)]);
                continue;
            }
            'T' => continue,
            'j' => value.day().to_string(),
            'd' => format!("{:02}", value.day()),
            'Y' => value.format("%Y").to_string(),
            'n' => value.month().to_string(),
            'm' => format!("{:02}", value.month()),
            'y' => value.format("%y").to_string(),
            'g' => hour12.to_string(),
            'h' => format!("{hour12:02}"),
            'G' => value.hour().to_string(),
            'H' => format!("{:02}", value.hour()),
            'i' => format!("{:02}", value.minute()),
            's' => format!("{:02}", value.second()),
            other => {
                out.push(other);
                continue;
            }
        };
        out.extend(number.chars().map(|digit| localise_digit(digit, names)));
    }
    out
}

fn localise_digit(digit: char, names: &DateNames) -> char {
    digit
        .to_digit(10)
        .and_then(|n| names.digits.get(usize::try_from(n).ok()?).copied())
        .unwrap_or(digit)
}

fn weekday_name(value: NaiveDateTime, names: &DateNames) -> &'static str {
    let index = usize::try_from(value.weekday().num_days_from_sunday()).unwrap_or(0);
    names.weekdays.get(index).copied().unwrap_or("")
}

fn month_name(value: NaiveDateTime, months: [&'static str; 12]) -> &'static str {
    let index = usize::try_from(value.month0()).unwrap_or(0);
    months.get(index).copied().unwrap_or("")
}
