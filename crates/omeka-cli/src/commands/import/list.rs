//! `romeka import list`: the history of imports, newest first.
//!
//! With `--json` the command writes one document: `{"page": n, "perPage": n,
//! "total": n, "imports": [{"id", "status", "created", "progress"}]}`, where
//! `progress` is the summary's `{"done", "total"}` or null.

use std::io::Write;

use omeka_db::repos::{ImportRepo, Paging};
use omeka_db::with_db;
use serde_json::{Value, json};

use super::session::{Format, Session, Where};
use crate::CliError;

/// Input for `romeka import list`.
#[derive(Debug, Clone)]
pub struct ListOptions {
    pub at: Where,
    pub page: u64,
    pub per_page: u64,
    pub format: Format,
}

/// One line per import on the page, then the totals.
///
/// # Errors
/// A database error.
pub async fn list(options: &ListOptions, out: &mut dyn Write) -> Result<(), CliError> {
    let session = Session::open(&options.at).await?;
    let per_page = options.per_page.clamp(1, 500);
    let page = options.page.max(1);
    let paging = Paging::new(
        i64::try_from(per_page).unwrap_or(500),
        i64::try_from((page - 1).saturating_mul(per_page)).unwrap_or(i64::MAX),
    );
    let rows = with_db!(&session.pool, |db| ImportRepo::new(db).page(&paging).await)?;
    let total = with_db!(&session.pool, |db| ImportRepo::new(db).count().await)?;
    if options.format == Format::Json {
        let imports: Vec<Value> = rows
            .iter()
            .map(|row| {
                json!({
                    "id": row.id.0,
                    "status": row.status.as_str(),
                    "created": row.created.format("%Y-%m-%dT%H:%M:%S").to_string(),
                    "progress": row.summary.as_ref().and_then(|summary| summary.get("progress")),
                })
            })
            .collect();
        let document = json!({
            "page": page,
            "perPage": per_page,
            "total": total,
            "imports": imports,
        });
        writeln!(out, "{document}")?;
        return Ok(());
    }
    for row in rows {
        let count = |member: &str| {
            row.summary
                .as_ref()
                .and_then(|summary| summary.get("progress"))
                .and_then(|progress| progress.get(member))
                .and_then(Value::as_u64)
        };
        let records = match (count("done"), count("total")) {
            (Some(done), Some(total)) => format!("{done} of {total} records"),
            _ => "no records".to_string(),
        };
        writeln!(
            out,
            "{}\t{}\t{}\t{}",
            row.id.0,
            row.status.as_str(),
            row.created.format("%Y-%m-%d %H:%M:%S"),
            records
        )?;
    }
    writeln!(out, "page {page}, {per_page} per page, {total} imports")?;
    Ok(())
}
