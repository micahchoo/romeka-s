//! `romeka import undo <id> [--force all] [--preview]`: put the install back
//! as the import found it, or say what that would do (`docs/import-v2.md`,
//! "Lifecycle and the ledger", "Undo details").
//!
//! With `--json` the command writes one document when it ends: `{"import": id,
//! "preview": <report>}` for a preview, `{"import": id, "status": <status>,
//! "report": <report>}` for an undo. The report is the summary's `undo`
//! member, the shape the undo routes answer.

use std::future::Future;
use std::io::Write;

use omeka_api::import::undo::{Force, UNDO_JOB, UndoError, may_undo, preview_undo};
use omeka_api::messages;
use omeka_db::ids::ImportId;
use omeka_db::repos::{ImportRepo, StartOutcome};
use omeka_db::rows::ImportStatus;
use omeka_db::with_db;
use serde_json::{Value, json};

use super::session::{Format, Session, Where, Work};
use crate::CliError;

/// Input for `romeka import undo`.
#[derive(Debug, Clone)]
pub struct UndoOptions {
    pub at: Where,
    pub id: u64,
    /// `--force all`: restore or delete touched resources too.
    pub force_all: bool,
    /// `--preview`: say what an undo would do and write nothing.
    pub preview: bool,
    pub format: Format,
}

/// How an undo command ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UndoOutcome {
    /// Undone, partly undone with the kept resources listed, or previewed.
    Clean,
    /// The undo stopped before the end; running it again goes on.
    Failed,
}

/// Undo import `options.id`, or preview the undo. When `stop` resolves during
/// the undo, the undo is asked to stop after the resource in hand.
///
/// # Errors
/// A [`CliError::Message`] when the import does not exist, is not in a state
/// an undo may start from, deleted resources, or another import is busy.
pub async fn undo(
    options: &UndoOptions,
    stop: impl Future<Output = ()>,
    out: &mut dyn Write,
) -> Result<UndoOutcome, CliError> {
    let session = Session::open(&options.at).await?;
    let row = session.import(options.id).await?;
    if options.preview {
        if !may_undo(row.status) {
            return Err(cannot_undo());
        }
        let force = if options.force_all {
            Force::All
        } else {
            Force::Nothing
        };
        let previewed = with_db!(&session.pool, |db| preview_undo(db, row.id, force).await)
            .map_err(|error| match error {
                UndoError::Deleted(_) => CliError::message(messages::IMPORT_DELETE_NOT_UNDONE),
                other => CliError::message(other.to_string()),
            })?;
        let report = serde_json::to_value(previewed)?;
        match options.format {
            Format::Text => write_report(&report, "would remove", "would restore", out)?,
            Format::Json => writeln!(out, "{}", json!({ "import": row.id.0, "preview": report }))?,
        }
        return Ok(UndoOutcome::Clean);
    }
    start(&session, row.id).await?;
    let work = Work {
        owner: None,
        class: UNDO_JOB,
        import: row.id,
        force: options.force_all.then(|| json!("all")),
        member: Some("undo"),
    };
    let mut quiet = std::io::sink();
    let progress: &mut dyn Write = match options.format {
        Format::Text => out,
        Format::Json => &mut quiet,
    };
    session.perform(work, stop, progress).await?;
    let ended = session.import(options.id).await?;
    let report = ended
        .summary
        .as_ref()
        .and_then(|summary| summary.get("undo"))
        .cloned()
        .unwrap_or(Value::Null);
    match options.format {
        Format::Text => {
            write_report(&report, "removed", "restored", out)?;
            writeln!(out, "import {} {}", ended.id.0, ended.status.as_str())?;
        }
        Format::Json => {
            let document = json!({
                "import": ended.id.0,
                "status": ended.status.as_str(),
                "report": report,
            });
            writeln!(out, "{document}")?;
        }
    }
    Ok(if ended.status == ImportStatus::UndoFailed {
        UndoOutcome::Failed
    } else {
        UndoOutcome::Clean
    })
}

fn cannot_undo() -> CliError {
    CliError::message(messages::IMPORT_CANNOT_UNDO)
}

async fn start(session: &Session, id: ImportId) -> Result<(), CliError> {
    match with_db!(&session.pool, |db| ImportRepo::new(db).start_undo(id).await)? {
        StartOutcome::Started => Ok(()),
        StartOutcome::Refused(_) => Err(cannot_undo()),
        StartOutcome::Deleted => Err(CliError::message(messages::IMPORT_DELETE_NOT_UNDONE)),
        StartOutcome::Busy(_) => Err(CliError::message(messages::IMPORT_BUSY)),
        StartOutcome::Missing => Err(CliError::message(messages::import_not_found(id.0))),
        StartOutcome::Changed { .. } => Err(CliError::message(
            "an undo start answered a revision change",
        )),
    }
}

/// `removed` and `restored` are the words for the two counts: a report says
/// what was done, a preview what would be.
fn write_report(
    report: &Value,
    removed: &str,
    restored: &str,
    out: &mut dyn Write,
) -> Result<(), CliError> {
    let member = |first: &str, second: &str| {
        report
            .get(first)
            .or_else(|| report.get(second))
            .cloned()
            .unwrap_or(Value::Null)
    };
    write_counts(removed, &member("removed", "remove"), out)?;
    write_counts(restored, &member("restored", "restore"), out)?;
    for kept in list(report, "kept") {
        writeln!(out, "kept {}", resource(kept))?;
        for reason in list(kept, "reasons") {
            let detail = reason.get("detail").and_then(Value::as_str).unwrap_or("");
            writeln!(out, "  {detail}")?;
        }
    }
    for gone in list(report, "skipped") {
        writeln!(out, "already gone {}", resource(gone))?;
    }
    for forced in list(report, "forced") {
        writeln!(out, "forced {}", resource(forced))?;
    }
    if report.get("stopped").and_then(Value::as_bool) == Some(true) {
        writeln!(out, "the undo stopped on request")?;
    }
    if let Some(detail) = report
        .get("error")
        .and_then(|error| error.get("detail"))
        .and_then(Value::as_str)
    {
        writeln!(out, "the undo stopped: {detail}")?;
    }
    Ok(())
}

fn list<'a>(value: &'a Value, member: &str) -> &'a [Value] {
    value
        .get(member)
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

fn resource(value: &Value) -> String {
    let kind = value.get("kind").and_then(Value::as_str).unwrap_or("?");
    let id = value.get("id").and_then(Value::as_u64).unwrap_or(0);
    format!("{kind} {id}")
}

fn write_counts(label: &str, counts: &Value, out: &mut dyn Write) -> Result<(), CliError> {
    let parts: Vec<String> = [
        ("items", "items"),
        ("media", "media"),
        ("itemSets", "item sets"),
        ("templates", "item types"),
        ("properties", "fields"),
    ]
    .iter()
    .filter_map(|(member, words)| {
        let count = counts.get(member).and_then(Value::as_u64).unwrap_or(0);
        (count > 0).then(|| format!("{count} {words}"))
    })
    .collect();
    let line = if parts.is_empty() {
        "nothing".to_string()
    } else {
        parts.join(", ")
    };
    writeln!(out, "{label}: {line}")?;
    Ok(())
}
