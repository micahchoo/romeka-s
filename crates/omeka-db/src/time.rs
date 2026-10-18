//! Datetime storage, upstream format.
//!
//! Upstream stores `DATETIME` columns as `YYYY-MM-DD HH:MM:SS` strings in UTC. PHP and MySQL both
//! write that shape, and a dump read back into MySQL only accepts it, so SQLite stores the same
//! bytes in a `TEXT` column. Parsing and formatting happen here, at the row boundary.

use chrono::{NaiveDateTime, Timelike, Utc};

/// The one accepted storage format, matching MySQL's `datetime` literal output.
pub(crate) const FORMAT: &str = "%Y-%m-%d %H:%M:%S";

pub(crate) fn parse(stored: &str) -> Result<NaiveDateTime, String> {
    NaiveDateTime::parse_from_str(stored, FORMAT)
        .map_err(|error| format!("`{stored}` is not `{FORMAT}`: {error}"))
}

pub(crate) fn format(value: NaiveDateTime) -> String {
    value.format(FORMAT).to_string()
}

/// The port's clock for `created`/`modified`, matching upstream `new DateTime('now')`
/// (`application/src/Api/Adapter/AbstractEntityAdapter.php`, `updateTimestamps`).
///
/// Upstream writes through Doctrine's `datetime` type, which formats to whole seconds. Floor the
/// clock to a second here so the struct a create or update returns equals the row a later read
/// parses back; sub-second precision would live in one and vanish from the other.
pub(crate) fn now() -> NaiveDateTime {
    let now = Utc::now().naive_utc();
    now.with_nanosecond(0).unwrap_or(now)
}

/// The clock for `session.modified`, which upstream stores as a Unix timestamp in seconds
/// (`Omeka\Session\SaveHandler\Db::write`), not as a datetime string.
pub(crate) fn now_unix() -> i64 {
    Utc::now().timestamp()
}
