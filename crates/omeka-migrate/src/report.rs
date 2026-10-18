//! The run report: one row count per table, read, written and skipped.

use std::collections::BTreeMap;
use std::fmt;

/// The port tables a run visits, in the order it writes them.
///
/// The order is the dependency order: a parent before its children, so every foreign key resolves.
/// The report prints in this order.
pub(crate) const TABLES: &[&str] = &[
    "user",
    "vocabulary",
    "resource_class",
    "property",
    "resource_template",
    "resource_template_property",
    "item",
    "item_set",
    "media",
    "value",
    "item_item_set",
    "site",
    "site_page",
    "site_page_block",
    "site_item_set",
    "site_permission",
    "api_key",
];

/// How a run behaves.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MigrateOptions {
    /// Read and map the source, count what a run would write, and leave the target untouched.
    pub dry_run: bool,
}

/// One table's counts for a run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableReport {
    /// The upstream table name, which the port keeps.
    pub table: &'static str,
    /// Rows read from the source.
    pub read: u64,
    /// Rows written to the target, or that a run would write under `--dry-run`.
    pub written: u64,
    /// Rows read but not written, because a foreign key they need is not in the source.
    pub skipped: u64,
}

/// The whole run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationReport {
    /// True when the target was left untouched.
    pub dry_run: bool,
    /// One entry per port table, in write order.
    pub tables: Vec<TableReport>,
}

impl MigrationReport {
    /// The summed counts across every table.
    pub fn totals(&self) -> (u64, u64, u64) {
        self.tables
            .iter()
            .fold((0, 0, 0), |(read, written, skipped), t| {
                (read + t.read, written + t.written, skipped + t.skipped)
            })
    }

    /// One table's counts, by name.
    pub fn table(&self, name: &str) -> Option<&TableReport> {
        self.tables.iter().find(|entry| entry.table == name)
    }
}

impl fmt::Display for MigrationReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mode = if self.dry_run { "dry run" } else { "written" };
        writeln!(
            f,
            "{:<24} {:>7} {:>8} {:>8}",
            "table", "read", mode, "skipped"
        )?;
        for table in &self.tables {
            writeln!(
                f,
                "{:<24} {:>7} {:>8} {:>8}",
                table.table, table.read, table.written, table.skipped
            )?;
        }
        let (read, written, skipped) = self.totals();
        write!(
            f,
            "{:<24} {:>7} {:>8} {:>8}",
            "total", read, written, skipped
        )
    }
}

/// Running counts, keyed by table, assembled into a [`MigrationReport`] at the end.
#[derive(Debug, Default)]
pub(crate) struct Ledger {
    counts: BTreeMap<&'static str, Counts>,
}

#[derive(Debug, Default, Clone, Copy)]
struct Counts {
    read: u64,
    written: u64,
    skipped: u64,
}

impl Ledger {
    /// Record the rows read from one table.
    pub(crate) fn read(&mut self, table: &'static str, rows: u64) {
        self.counts.entry(table).or_default().read = rows;
    }

    /// Record one row written.
    pub(crate) fn written(&mut self, table: &'static str) {
        self.counts.entry(table).or_default().written += 1;
    }

    /// Record one row read but not written.
    pub(crate) fn skipped(&mut self, table: &'static str) {
        self.counts.entry(table).or_default().skipped += 1;
    }

    /// Assemble the report in [`TABLES`] order.
    pub(crate) fn into_report(self, dry_run: bool) -> MigrationReport {
        let tables = TABLES
            .iter()
            .filter_map(|name| {
                self.counts.get(name).map(|counts| TableReport {
                    table: name,
                    read: counts.read,
                    written: counts.written,
                    skipped: counts.skipped,
                })
            })
            .collect();
        MigrationReport { dry_run, tables }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_report_totals_its_tables_and_names_them_in_write_order() {
        let mut ledger = Ledger::default();
        ledger.read("user", 3);
        ledger.written("user");
        ledger.written("user");
        ledger.skipped("user");
        ledger.read("value", 5);
        ledger.written("value");
        let report = ledger.into_report(false);

        assert_eq!(
            report.tables.iter().map(|t| t.table).collect::<Vec<_>>(),
            vec!["user", "value"],
            "a table with no read stays out, the rest keep write order"
        );
        assert_eq!(report.table("user").map(|t| t.written), Some(2));
        assert_eq!(report.totals(), (8, 3, 1));
        let rendered = report.to_string();
        assert!(rendered.contains("written"), "a live run labels the column");
        assert!(!rendered.contains("dry run"));
    }
}
