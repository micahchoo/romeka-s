//! `romeka import list`: the history of imports, newest first.

use std::io::Write;

use omeka_db::repos::{ImportRepo, Paging};
use omeka_db::with_db;
use serde_json::Value;

use super::session::{Session, Where};
use crate::CliError;

/// Input for `romeka import list`.
#[derive(Debug, Clone)]
pub struct ListOptions {
    pub at: Where,
    pub page: u64,
    pub per_page: u64,
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
    for row in with_db!(&session.pool, |db| ImportRepo::new(db).page(&paging).await)? {
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
    writeln!(
        out,
        "page {page}, {per_page} per page, {} imports",
        with_db!(&session.pool, |db| ImportRepo::new(db).count().await)?
    )?;
    Ok(())
}
