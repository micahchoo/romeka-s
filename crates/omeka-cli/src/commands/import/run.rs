//! `romeka import run --plan plan.json --as <email> [--dry-run]`: the
//! command line's Run button (`docs/import-v2.md`, "Command line").
//!
//! The command makes a draft from the plan, then does what the run route
//! does: rescan, derive, check, refuse on a blocking flag or an unconfirmed
//! delete, start, and run the job body in this process. A refused run, and a
//! dry run, delete the draft again, so they leave no import behind.
//!
//! With `--json` the command writes one document when it ends:
//! `{"import": id | null, "check": <Check>, "status": <status> | null,
//! "summary": <summary> | null}`. A dry run has no import and no status.

use std::future::Future;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;

use omeka_api::import2::check::{Check, Does};
use omeka_api::import2::draft::{self, DraftError, Start};
use omeka_api::import2::plan::ImportPlanV2;
use omeka_api::import2::plan::PlanError;
use omeka_api::import2::preview::Caches;
use omeka_api::import2::run::RUN_JOB;
use omeka_api::import2::start::{self, Go, Refusal, StartError};
use omeka_api::messages;
use omeka_db::ids::ImportId;
use omeka_db::rows::{ImportRow, ImportStatus};
use omeka_db::with_db;
use serde::Serialize;
use serde_json::{Value, json};

use super::session::{Admin, Format, Session, Where, Work};
use crate::CliError;

/// Input for `romeka import run`.
#[derive(Debug, Clone)]
pub struct RunOptions {
    pub at: Where,
    pub plan: PathBuf,
    /// `--as`: the email of the administrator who owns the import.
    pub actor: String,
    pub dry_run: bool,
    /// `--confirm`: the count a delete run removes.
    pub confirm: Option<u64>,
    pub format: Format,
}

/// How a run command ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunOutcome {
    /// A dry run, or a run that wrote every record.
    Clean,
    /// The run ended, but a record failed or nothing was written.
    Failed,
}

/// Check the plan, and run it unless `options.dry_run`. Everything the
/// operator reads goes to `out`. When `stop` resolves during the run, the run
/// is asked to stop and ends `run_failed` after the record in hand.
///
/// # Errors
/// A [`CliError::Message`] for a plan, user or start the contract refuses;
/// the database and job errors otherwise.
pub async fn run(
    options: &RunOptions,
    stop: impl Future<Output = ()>,
    out: &mut dyn Write,
) -> Result<RunOutcome, CliError> {
    let session = Session::open(&options.at).await?;
    let admin = session.admin(&options.actor).await?;
    let plan = read_plan(&options.plan)?;
    let import = make_draft(&session, &admin, &plan).await?;
    let ran = go(&session, &admin, import, plan, options, stop, out).await;
    let (check, ended) = match ran {
        Ok(Ran { check, ended: None }) => {
            discard(&session, import).await?;
            (check, None)
        }
        Ok(Ran { check, ended }) => (check, ended),
        Err(error) => {
            // A run that started is no draft any more; its error is the one to show.
            let _ = discard(&session, import).await;
            return Err(error);
        }
    };
    let outcome = ended.as_ref().map_or(RunOutcome::Clean, outcome_of);
    match options.format {
        Format::Text => {
            if let Some(row) = &ended {
                write_ending(row, out)?;
            }
        }
        Format::Json => {
            let document = json!({
                "import": ended.as_ref().map(|row| row.id.0),
                "check": check,
                "status": ended.as_ref().map(|row| row.status.as_str()),
                "summary": ended.as_ref().and_then(|row| row.summary.clone()),
            });
            writeln!(out, "{document}")?;
        }
    }
    Ok(outcome)
}

fn read_plan(path: &std::path::Path) -> Result<ImportPlanV2, CliError> {
    let text = std::fs::read_to_string(path)
        .map_err(|error| CliError::message(format!("cannot read {}: {error}", path.display())))?;
    ImportPlanV2::from_json(&text).map_err(|error| match error {
        PlanError::V1Plan => CliError::message(messages::IMPORT_V1_PLAN),
        other => CliError::message(messages::import_plan_refused(other)),
    })
}

/// A draft holding `plan`, saved through the boundary the route saves through.
async fn make_draft(
    session: &Session,
    admin: &Admin,
    plan: &ImportPlanV2,
) -> Result<ImportId, CliError> {
    let made = with_db!(&session.pool, |db| draft::create(
        db,
        Some(admin.id),
        Start::Sources(plan.sources.clone()),
    )
    .await)
    .map_err(draft_error)?;
    let value: Value = serde_json::from_str(&plan.to_json())?;
    if let Err(error) = with_db!(&session.pool, |db| draft::save(
        db,
        made.id,
        &value,
        made.revision
    )
    .await)
    {
        discard(session, made.id).await?;
        return Err(draft_error(error));
    }
    Ok(made.id)
}

fn draft_error(error: DraftError) -> CliError {
    CliError::message(match error {
        DraftError::V1Plan => messages::IMPORT_V1_PLAN.to_string(),
        DraftError::TypeExists(label) => messages::import_type_exists(&label),
        DraftError::Plan(error) => messages::import_plan_refused(error.to_string()),
        other => other.to_string(),
    })
}

async fn discard(session: &Session, import: ImportId) -> Result<(), CliError> {
    with_db!(&session.pool, |db| draft::delete(db, import).await).map_err(draft_error)
}

/// What [`go`] did: the Check it read, and the import row after the run, or
/// `None` when nothing ran and the draft goes.
struct Ran {
    check: Check,
    ended: Option<ImportRow>,
}

/// Assess, print, and start.
async fn go(
    session: &Session,
    admin: &Admin,
    import: ImportId,
    plan: ImportPlanV2,
    options: &RunOptions,
    stop: impl Future<Output = ()>,
    out: &mut dyn Write,
) -> Result<Ran, CliError> {
    let files = omeka_api::settings::file_validation(&session.pool).await?;
    let assessed = with_db!(&session.pool, |db| start::assess(
        db,
        &session.env.archive_roots,
        &session.env.store,
        &Arc::new(Caches::default()),
        files,
        admin.owners,
        import,
        plan,
    )
    .await)
    .map_err(|error| CliError::message(error.to_string()))?;
    let check = assessed.check;
    let mut quiet = std::io::sink();
    let progress: &mut dyn Write = match options.format {
        Format::Text => out,
        Format::Json => &mut quiet,
    };
    write_check(&check, progress)?;
    if options.dry_run {
        return Ok(Ran { check, ended: None });
    }
    let row = session.import(import.0).await?;
    let go = Go {
        digest: None,
        confirm: options.confirm,
    };
    with_db!(&session.pool, |db| start::begin(db, &row, &check, go).await)
        .map_err(refused)?;
    writeln!(progress, "import {} running", import.0)?;
    let work = Work {
        owner: Some(admin.id),
        class: RUN_JOB,
        import,
        force: None,
        member: None,
    };
    session.perform(work, stop, progress).await?;
    let ended = session.import(import.0).await?;
    Ok(Ran {
        check,
        ended: Some(ended),
    })
}

fn refused(error: StartError) -> CliError {
    let refusal = match error {
        StartError::Db(error) => return error.into(),
        StartError::Refused(refusal) => refusal,
    };
    CliError::message(match refusal {
        Refusal::Blocked(_) => messages::IMPORT_BLOCKED.to_string(),
        Refusal::Nothing => messages::IMPORT_NOTHING_TO_RUN.to_string(),
        Refusal::Confirm { deletes } => format!(
            "{} Pass --confirm {deletes}.",
            messages::import_confirm_delete(deletes)
        ),
        Refusal::Busy(_) => messages::IMPORT_BUSY.to_string(),
        Refusal::CannotRun(_) => messages::IMPORT_CANNOT_RUN.to_string(),
        Refusal::DigestChanged { .. } | Refusal::RevisionChanged { .. } => {
            messages::IMPORT_CHANGED.to_string()
        }
        Refusal::Missing | Refusal::Deleted => "the draft vanished while it started".to_string(),
    })
}

/// A serialised enum or struct's plain name, for a line of text.
fn name(value: &impl Serialize) -> String {
    match serde_json::to_value(value) {
        Ok(Value::String(text)) => text,
        Ok(other) => other.to_string(),
        Err(error) => error.to_string(),
    }
}

/// The Check summary: counts per action and kind, flags by policy, and the
/// reasons the run cannot start.
pub(super) fn write_check(check: &Check, out: &mut dyn Write) -> Result<(), CliError> {
    writeln!(out, "action: {}", name(&check.action))?;
    for count in &check.counts {
        let does = match count.does {
            Does::Create => "create".to_string(),
            Does::Leave => "leave".to_string(),
            Does::Act(action) => name(&action),
        };
        writeln!(out, "  {does} {}: {}", name(&count.kind), count.count)?;
    }
    for set in &check.new_sets {
        writeln!(out, "  new item set: {}", set.title)?;
    }
    for new_type in &check.new_types {
        writeln!(out, "  new item type: {}", new_type.label)?;
    }
    for field in &check.new_fields {
        writeln!(out, "  new field: {}", field.label)?;
    }
    for (policy, count) in &check.flags {
        writeln!(out, "  flags {policy}: {count}")?;
    }
    for flag in &check.blocking {
        writeln!(out, "  blocking: {}", name(flag))?;
    }
    if check.without_template > 0 {
        writeln!(out, "  without a template: {}", check.without_template)?;
    }
    if check.default_owner > 0 {
        writeln!(out, "  with the default owner: {}", check.default_owner)?;
    }
    if check.deletes > 0 {
        writeln!(out, "  deletes: {}", check.deletes)?;
    }
    writeln!(out, "digest: {}", check.digest)?;
    writeln!(
        out,
        "{}",
        if check.can_run {
            "the import can run"
        } else {
            "the import cannot run"
        }
    )?;
    Ok(())
}

/// A run is clean when it ended `done` with no failed record.
fn outcome_of(row: &ImportRow) -> RunOutcome {
    let failed = row
        .summary
        .as_ref()
        .and_then(|summary| summary.get("failures"))
        .and_then(Value::as_array)
        .is_some_and(|failures| !failures.is_empty());
    if row.status == ImportStatus::Done && !failed {
        RunOutcome::Clean
    } else {
        RunOutcome::Failed
    }
}

fn write_ending(row: &ImportRow, out: &mut dyn Write) -> Result<(), CliError> {
    let summary = row.summary.as_ref();
    let count = |member: &str| {
        summary
            .and_then(|summary| summary.get("progress"))
            .and_then(|progress| progress.get(member))
            .and_then(Value::as_u64)
            .unwrap_or(0)
    };
    let failures: &[Value] = summary
        .and_then(|summary| summary.get("failures"))
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice);
    writeln!(
        out,
        "import {} {}: {} of {} records, {} failed",
        row.id.0,
        row.status.as_str(),
        count("done"),
        count("total"),
        failures.len()
    )?;
    for failure in failures {
        let text = |member: &str| failure.get(member).and_then(Value::as_str).unwrap_or("");
        writeln!(out, "  failed {}: {}", text("record"), text("detail"))?;
    }
    let stopped = summary
        .and_then(|summary| summary.get("stopped"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if stopped {
        writeln!(out, "the run stopped on request")?;
    }
    Ok(())
}
