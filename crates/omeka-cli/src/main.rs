//! The `romeka` binary: clap parsing and command dispatch.

use std::future::Future;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::parser::ValueSource;
use clap::{Arg, ArgAction, ArgMatches, Command};
use omeka_db::Location;

use omeka_cli::commands::backup::{self as backup, BackupOptions};
use omeka_cli::commands::derivatives::{
    self as media_derivatives_cmd, DerivativeOptions, DerivativeReport,
};
use omeka_cli::commands::export::{self as export, CsvOptions as ExportCsvOptions, JsonldOptions};
use omeka_cli::commands::fixity::{self as fixity_cmd, CheckOptions as FixityOptions};
use omeka_cli::commands::import::list::{self as import_list, ListOptions as ImportListOptions};
use omeka_cli::commands::import::run::{
    self as import_run, RunOptions as ImportRunOptions, RunOutcome,
};
use omeka_cli::commands::import::undo::{self as import_undo, UndoOptions, UndoOutcome};
use omeka_cli::commands::import::{Format, TemplateOptions, Where};
use omeka_cli::commands::install::{self, InstallOptions};
use omeka_cli::commands::job::{self as job_cmd};
use omeka_cli::commands::key::{
    self, AddOptions as KeyAddOptions, ListOptions as KeyListOptions,
    RevokeOptions as KeyRevokeOptions,
};
use omeka_cli::commands::restore::{self as restore, RestoreOptions};
use omeka_cli::commands::sweep::{self as media_sweep, MIN_AGE, SweepOptions};
use omeka_cli::commands::thumbnails::{
    self as media_thumbnails, ThumbnailOptions, ThumbnailStatus,
};
use omeka_cli::commands::user::{
    self, AddOptions, GrantAdminOptions, ListOptions, SetPasswordOptions,
};
use omeka_cli::{CliError, commands};

fn main() -> ExitCode {
    // clap 4 exits 2 on a usage error; `romeka` has always exited 1, as clap 2 did.
    let matches = match build_app().try_get_matches() {
        Ok(matches) => matches,
        Err(error) => {
            let _ = error.print();
            return if error.use_stderr() {
                ExitCode::FAILURE
            } else {
                ExitCode::SUCCESS
            };
        }
    };
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("romeka: cannot start the async runtime: {error}");
            return ExitCode::FAILURE;
        }
    };
    match runtime.block_on(dispatch(&matches)) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("romeka: {error}");
            ExitCode::FAILURE
        }
    }
}

fn build_app() -> Command {
    group("romeka")
        .version(env!("CARGO_PKG_VERSION"))
        .about("Romeka S command line")
        .propagate_version(true)
        .subcommand(install_command())
        .subcommand(user_command())
        .subcommand(key_command())
        .subcommand(import_command())
        .subcommand(export_command())
        .subcommand(media_command())
        .subcommand(fixity_command())
        .subcommand(backup_cli())
        .subcommand(restore_cli())
        .subcommand(job_command())
        .subcommand(serve_command())
}

/// A command whose subcommand is required, so it prints its help without one.
fn group(name: &'static str) -> Command {
    Command::new(name)
        .subcommand_required(true)
        .arg_required_else_help(true)
}

/// An option that takes one value. clap 2 named the value after the argument
/// when no `value_name` was given; the explicit name keeps that help text.
fn option(name: &'static str) -> Arg {
    Arg::new(name).long(name).value_name(name)
}

fn flag(name: &'static str) -> Arg {
    Arg::new(name).long(name).action(ArgAction::SetTrue)
}

/// `--database PATH`, or `--database-url URL` (or `ROMEKA_DATABASE_URL`) for a
/// MySQL or MariaDB server; one of them is required.
fn database_args() -> [Arg; 2] {
    [
        option("database")
            .short('d')
            .value_name("PATH")
            .required_unless_present("database-url")
            .help("SQLite database file"),
        option("database-url")
            .value_name("URL")
            .env(omeka_cli::jobs::DATABASE_URL_ENV)
            .hide_env_values(true)
            .help("MySQL or MariaDB server, mysql://user:password@host:port/database"),
    ]
}

fn optional_store_arg() -> Arg {
    option("store")
        .value_name("DIR")
        .help("File store root; defaults to `files` beside the database")
}

fn archive_root_arg() -> Arg {
    option("archive-root")
        .value_name("DIR")
        .help("Folder a plan's sources read, named `archive`; the configured roots otherwise")
}

fn email_arg() -> Arg {
    option("email").value_name("EMAIL").required(true)
}

fn install_command() -> Command {
    Command::new("install")
        .about("Create a database and seed the default data")
        .args(database_args())
        .arg(email_arg())
        .arg(option("name").value_name("NAME").required(true))
        .arg(option("password").value_name("PASSWORD").required(true))
        .arg(option("title").default_value("Romeka S"))
        .arg(option("time-zone").default_value("UTC"))
        .arg(option("locale").default_value("en"))
}

fn user_command() -> Command {
    group("user")
        .about("Manage user accounts")
        .subcommand(
            Command::new("add")
                .about("Add a user")
                .args(database_args())
                .arg(email_arg())
                .arg(option("name").required(true))
                .arg(option("password"))
                .arg(option("role").default_value("researcher"))
                .arg(
                    option("base-url")
                        .value_name("URL")
                        .default_value("http://localhost:8080")
                        .help("Origin the invite link is built from"),
                ),
        )
        .subcommand(
            Command::new("list")
                .about("List users")
                .args(database_args())
                .arg(option("role")),
        )
        .subcommand(
            Command::new("set-password")
                .about("Replace a user's password")
                .args(database_args())
                .arg(email_arg())
                .arg(option("password").required(true)),
        )
        .subcommand(
            Command::new("grant-admin")
                .about("Set a user's role to global_admin")
                .args(database_args())
                .arg(email_arg()),
        )
}

fn key_command() -> Command {
    group("key")
        .about("Manage API keys")
        .subcommand(
            Command::new("add")
                .about("Mint an API key and print its credential once")
                .args(database_args())
                .arg(option("owner").value_name("EMAIL").required(true))
                .arg(option("label").value_name("TEXT").default_value("")),
        )
        .subcommand(
            Command::new("list")
                .about("List API keys")
                .args(database_args()),
        )
        .subcommand(
            Command::new("revoke")
                .about("Delete an API key")
                .args(database_args())
                .arg(option("identity").value_name("IDENTITY").required(true)),
        )
}

fn import_command() -> Command {
    group("import")
        .about("Import data")
        .subcommand(
            Command::new("template")
                .about("Import a resource template from JSON")
                .args(database_args())
                .arg(option("file").value_name("FILE").required(true)),
        )
        .subcommand(
            Command::new("run")
                .about("Check an import plan and run it in this process")
                .args(database_args())
                .arg(
                    option("plan")
                        .value_name("FILE")
                        .required(true)
                        .help("An import v2 plan, as JSON"),
                )
                .arg(
                    option("as")
                        .value_name("EMAIL")
                        .required(true)
                        .help("The administrator who owns the import"),
                )
                .arg(flag("dry-run").help("Print the Check summary and write nothing"))
                .arg(
                    option("confirm")
                        .value_name("COUNT")
                        .help("The number of resources a delete plan removes"),
                )
                .arg(json_arg())
                .arg(optional_store_arg())
                .arg(archive_root_arg()),
        )
        .subcommand(
            Command::new("undo")
                .about("Undo an import, or say what undoing it would do")
                .args(database_args())
                .arg(
                    Arg::new("id")
                        .value_name("ID")
                        .required(true)
                        .help("Import id"),
                )
                .arg(
                    option("force")
                        .value_name("all")
                        .value_parser(["all"])
                        .help("Restore or delete resources edited since the import wrote them"),
                )
                .arg(flag("preview").help("Print what the undo would do and write nothing"))
                .arg(json_arg())
                .arg(optional_store_arg()),
        )
        .subcommand(
            Command::new("list")
                .about("List imports, newest first")
                .args(database_args())
                .arg(option("page").value_name("N").default_value("1"))
                .arg(option("per-page").value_name("N").default_value("25"))
                .arg(json_arg()),
        )
}

fn export_command() -> Command {
    group("export")
        .about("Write the API documents or an item CSV out of the database")
        .subcommand(
            Command::new("jsonld")
                .about("Write the documents the read layer serves")
                .args(database_args())
                .arg(
                    option("out")
                        .value_name("PATH")
                        .required(true)
                        .help("An existing directory (one file per document) or the file to write"),
                )
                .arg(
                    option("resource")
                        .value_name("NAME")
                        .default_value("items")
                        .help("API resource name"),
                )
                .arg(
                    option("item-set")
                        .value_name("ID")
                        .help("Keep only items in this item set"),
                )
                .arg(
                    option("base-url")
                        .default_value("http://localhost")
                        .help("Origin for every @id in the documents"),
                ),
        )
        .subcommand(
            Command::new("csv")
                .about("Write items as CSV, one column per value")
                .args(database_args())
                .arg(
                    option("out")
                        .value_name("FILE")
                        .required(true)
                        .help("The CSV file to write"),
                )
                .arg(
                    option("resource")
                        .value_name("NAME")
                        .default_value("items")
                        .help("Items only"),
                )
                .arg(
                    option("item-set")
                        .value_name("ID")
                        .help("Keep only items in this item set"),
                )
                .arg(
                    option("columns")
                        .value_name("TERMS")
                        .help("Comma-separated property terms; defaults to every term in the data"),
                ),
        )
}

fn media_command() -> Command {
    group("media")
        .about("Manage media")
        .subcommand(
            Command::new("thumbnails")
                .about("Generate the thumbnails a media row is missing")
                .args(database_args())
                .arg(optional_store_arg())
                .arg(flag("force").help("Rebuild thumbnails that already exist")),
        )
        .subcommand(
            Command::new("derivatives")
                .about("Queue the derivatives a media is missing, then run the worker")
                .args(database_args())
                .arg(optional_store_arg())
                .arg(flag("retry").help(
                    "Queue rows a tool failed on or skipped; a row that failed 3 times stays failed",
                ))
                .arg(
                    option("redo")
                        .value_name("KIND")
                        .num_args(0..=1)
                        .value_parser(["video", "audio", "image", "document"])
                        .help(
                            "Make ready copies again: video, audio, image or document; \
                             every kind when the word is left out",
                        ),
                )
                .arg(option("media").value_name("ID").help("Only this media")),
        )
        .subcommand(
            Command::new("sweep")
                .about("List, or with --delete remove, store files no media or asset row names")
                .args(database_args())
                .arg(optional_store_arg())
                .arg(flag("delete").help("Remove the files; without it the run only reports them")),
        )
}

fn backup_cli() -> Command {
    Command::new("backup")
        .about("Archive the database and the store into one file")
        .args(database_args())
        .arg(store_arg())
        .arg(
            option("out")
                .value_name("FILE")
                .required(true)
                .help("The archive to write"),
        )
}

fn restore_cli() -> Command {
    Command::new("restore")
        .about("Write an archive back into a database and a store")
        .args(database_args())
        .arg(store_arg())
        .arg(
            option("in")
                .value_name("FILE")
                .required(true)
                .help("The archive to read"),
        )
        .arg(flag("force").help("Overwrite an occupied database or store"))
}

fn store_arg() -> Arg {
    option("store")
        .value_name("DIR")
        .required(true)
        .help("File store root, the directory holding original/ and the thumbnails")
}

fn fixity_command() -> Command {
    group("fixity")
        .about("Check stored files against the SHA-256 their media records")
        .subcommand(
            Command::new("check")
                .about(
                    "Read every original, compare its SHA-256, and record the result as a job; \
                     exits 1 when a file differs, is missing or cannot be read",
                )
                .args(database_args())
                .arg(optional_store_arg()),
        )
}

fn job_command() -> Command {
    group("job")
        .about("Run and inspect background jobs")
        .subcommand(
            Command::new("run")
                .about("Run one job in this process")
                .args(database_args())
                .arg(id_arg())
                .arg(optional_store_arg())
                .arg(
                    option("base-url")
                        .help("Base URL of the installation; defaults to http://localhost"),
                )
                .arg(
                    option("archive-root")
                        .value_name("DIR")
                        .help("Archive import root; the archive import is off without it"),
                ),
        )
        .subcommand(
            Command::new("list")
                .about("List jobs")
                .args(database_args()),
        )
        .subcommand(
            Command::new("show")
                .about("Show a job and its captured log")
                .args(database_args())
                .arg(id_arg()),
        )
        .subcommand(
            Command::new("stop")
                .about("Ask a running job to stop")
                .args(database_args())
                .arg(id_arg()),
        )
}

fn id_arg() -> Arg {
    Arg::new("id")
        .value_name("ID")
        .required(true)
        .help("Job id")
}

fn serve_command() -> Command {
    Command::new("serve")
        .about("Start the HTTP server")
        .args(database_args())
        .arg(option("host").default_value("127.0.0.1"))
        .arg(option("port").default_value("8080"))
        .arg(
            option("base-url").help("Base URL for JSON-LD links; defaults to http://<host>:<port>"),
        )
        .arg(
            option("store")
                .value_name("DIR")
                .help("File store root; defaults to `files` in the working directory"),
        )
}

async fn dispatch(matches: &ArgMatches) -> Result<ExitCode, CliError> {
    match matches.subcommand() {
        Some(("install", args)) => run_install(args).await,
        Some(("user", args)) => run_user(args).await,
        Some(("key", args)) => run_key(args).await,
        Some(("import", args)) => run_import(args).await,
        Some(("export", args)) => run_export(args).await,
        Some(("media", args)) => run_media(args).await,
        Some(("fixity", args)) => run_fixity(args).await,
        Some(("backup", args)) => run_backup(args).await,
        Some(("restore", args)) => run_restore(args).await,
        Some(("job", args)) => run_job(args).await,
        Some(("serve", args)) => run_serve(args).await,
        _ => Ok(ExitCode::SUCCESS),
    }
}

async fn run_install(args: &ArgMatches) -> Result<ExitCode, CliError> {
    let options = InstallOptions {
        database: location(args)?,
        email: value(args, "email")?.to_string(),
        name: value(args, "name")?.to_string(),
        password: value(args, "password")?.to_string(),
        title: value(args, "title")?.to_string(),
        time_zone: value(args, "time-zone")?.to_string(),
        locale: value(args, "locale")?.to_string(),
    };
    let report = install::run(&options).await?;
    println!("installed {}", report.database);
    println!(
        "  vocabularies: {}  classes: {}  properties: {}  members corrected: {}",
        report.vocabularies_inserted,
        report.classes_inserted,
        report.properties_inserted,
        report.vocabulary_members_corrected
    );
    println!(
        "  default template: {}",
        if report.template_created {
            "created"
        } else {
            "already present"
        }
    );
    println!(
        "  admin user: {}",
        if report.user_created {
            "created"
        } else {
            "already present"
        }
    );
    Ok(ExitCode::SUCCESS)
}

async fn run_user(args: &ArgMatches) -> Result<ExitCode, CliError> {
    match args.subcommand() {
        Some(("add", m)) => {
            let added = user::add(&AddOptions {
                database: location(m)?,
                email: value(m, "email")?.to_string(),
                name: value(m, "name")?.to_string(),
                password: value_of(m, "password").map(str::to_string),
                role: value(m, "role")?.to_string(),
                base_url: value(m, "base-url")?.to_string(),
            })
            .await?;
            println!(
                "added user {} (id {}) with role {}",
                added.user.email, added.user.id.0, added.user.role
            );
            if let Some(invite) = &added.invite {
                println!("  the account is inactive until this link sets a password:");
                println!("  {}", invite.link);
                println!("  hand it over; the link is shown once and expires in two weeks");
            }
        }
        Some(("list", m)) => {
            let rows = user::list(&ListOptions {
                database: location(m)?,
                role: value_of(m, "role").map(str::to_string),
            })
            .await?;
            if rows.is_empty() {
                println!("no users");
            }
            for row in rows {
                println!(
                    "{}\t{}\t{}\t{}",
                    row.id.0, row.email, row.role, row.is_active
                );
            }
        }
        Some(("set-password", m)) => {
            let row = user::set_password(&SetPasswordOptions {
                database: location(m)?,
                email: value(m, "email")?.to_string(),
                password: value(m, "password")?.to_string(),
            })
            .await?;
            println!("password updated for {}", row.email);
        }
        Some(("grant-admin", m)) => {
            let row = user::grant_admin(&GrantAdminOptions {
                database: location(m)?,
                email: value(m, "email")?.to_string(),
            })
            .await?;
            println!("{} is now {}", row.email, row.role);
        }
        _ => {}
    }
    Ok(ExitCode::SUCCESS)
}

async fn run_key(args: &ArgMatches) -> Result<ExitCode, CliError> {
    match args.subcommand() {
        Some(("add", m)) => {
            let minted = key::add(&KeyAddOptions {
                database: location(m)?,
                owner: value(m, "owner")?.to_string(),
                label: value(m, "label")?.to_string(),
            })
            .await?;
            println!("added key {} for {}", minted.identity, minted.owner_email);
            println!("  key_identity={}", minted.identity);
            println!("  key_credential={}", minted.credential);
            println!("  the credential is shown once; store it now");
        }
        Some(("list", m)) => {
            let keys = key::list(&KeyListOptions {
                database: location(m)?,
            })
            .await?;
            if keys.is_empty() {
                println!("no keys");
            }
            for row in keys {
                println!(
                    "{}\t{}\t{}\t{}\t{}",
                    row.identity,
                    row.owner_email,
                    row.label,
                    row.created,
                    row.last_accessed
                        .map_or_else(|| "-".to_string(), |at| at.to_string())
                );
            }
        }
        Some(("revoke", m)) => {
            let identity = value(m, "identity")?;
            let removed = key::revoke(&KeyRevokeOptions {
                database: location(m)?,
                identity: identity.to_string(),
            })
            .await?;
            if removed {
                println!("revoked key {identity}");
            } else {
                eprintln!("no key with identity {identity}");
                return Ok(ExitCode::from(1));
            }
        }
        _ => {}
    }
    Ok(ExitCode::SUCCESS)
}

fn json_arg() -> Arg {
    flag("json").help("Write one JSON document when the command ends, and no progress lines")
}

fn format(args: &ArgMatches) -> Format {
    if args.get_flag("json") {
        Format::Json
    } else {
        Format::Text
    }
}

/// Resolves on the first Ctrl-C, so the running job is asked to stop and the
/// command waits for it to end. A second Ctrl-C ends the process at once; the
/// import is then settled `run_failed` or `undo_failed` once its heartbeat
/// lapses.
fn stop_on_ctrl_c() -> impl Future<Output = ()> {
    let (tell, told) = tokio::sync::oneshot::channel();
    tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_err() {
            return;
        }
        let _ = tell.send(());
        if tokio::signal::ctrl_c().await.is_ok() {
            eprintln!("romeka: stopped without waiting for the job");
            std::process::exit(130);
        }
    });
    async move {
        if told.await.is_err() {
            std::future::pending::<()>().await;
        }
    }
}

/// The database, store and archive root an import command names. `undo`
/// takes no `--archive-root` and `list` neither path, and clap 4 panics on
/// reading an argument the command does not define.
fn import_at(args: &ArgMatches) -> Result<Where, CliError> {
    let path = |name: &str| {
        args.try_get_one::<String>(name)
            .ok()
            .flatten()
            .map(PathBuf::from)
    };
    Ok(Where {
        database: location(args)?,
        store: path("store"),
        archive_root: path("archive-root"),
    })
}

async fn run_import(args: &ArgMatches) -> Result<ExitCode, CliError> {
    match args.subcommand() {
        Some(("template", m)) => {
            let template = commands::import::import_template(&TemplateOptions {
                database: location(m)?,
                file: PathBuf::from(value(m, "file")?),
            })
            .await?;
            println!(
                "imported template \"{}\" (id {})",
                template.label, template.id.0
            );
        }
        Some(("run", m)) => {
            let outcome = import_run::run(
                &ImportRunOptions {
                    at: import_at(m)?,
                    plan: PathBuf::from(value(m, "plan")?),
                    actor: value(m, "as")?.to_string(),
                    dry_run: m.get_flag("dry-run"),
                    confirm: optional_id(m, "confirm")?,
                    format: format(m),
                },
                stop_on_ctrl_c(),
                &mut std::io::stdout().lock(),
            )
            .await?;
            if outcome == RunOutcome::Failed {
                return Ok(ExitCode::from(1));
            }
        }
        Some(("undo", m)) => {
            let outcome = import_undo::undo(
                &UndoOptions {
                    at: import_at(m)?,
                    id: value(m, "id")?.parse().map_err(|error| {
                        CliError::message(format!("invalid import id: {error}"))
                    })?,
                    force_all: value_of(m, "force") == Some("all"),
                    preview: m.get_flag("preview"),
                    format: format(m),
                },
                stop_on_ctrl_c(),
                &mut std::io::stdout().lock(),
            )
            .await?;
            if outcome == UndoOutcome::Failed {
                return Ok(ExitCode::from(1));
            }
        }
        Some(("list", m)) => {
            import_list::list(
                &ImportListOptions {
                    at: import_at(m)?,
                    page: optional_id(m, "page")?.unwrap_or(1),
                    per_page: optional_id(m, "per-page")?.unwrap_or(25),
                    format: format(m),
                },
                &mut std::io::stdout().lock(),
            )
            .await?;
        }
        _ => {}
    }
    Ok(ExitCode::SUCCESS)
}

async fn run_export(args: &ArgMatches) -> Result<ExitCode, CliError> {
    match args.subcommand() {
        Some(("jsonld", m)) => {
            let report = export::run_jsonld(&JsonldOptions {
                database: location(m)?,
                out: PathBuf::from(value(m, "out")?),
                resource: value(m, "resource")?.to_string(),
                item_set: optional_id(m, "item-set")?,
                base_url: value(m, "base-url")?.to_string(),
            })
            .await?;
            println!(
                "exported {} {} document(s) to {}",
                report.documents,
                report.resource,
                report.out.display()
            );
            if report.per_file {
                println!("  one file per document, named for its o:id");
            }
        }
        Some(("csv", m)) => {
            let resource = value(m, "resource")?;
            if resource != "items" {
                return Err(CliError::message(format!(
                    "CSV export writes items, not {resource}"
                )));
            }
            let columns = value_of(m, "columns")
                .map(parse_columns)
                .unwrap_or_default();
            let report = export::run_csv(&ExportCsvOptions {
                database: location(m)?,
                out: PathBuf::from(value(m, "out")?),
                item_set: optional_id(m, "item-set")?,
                columns,
            })
            .await?;
            println!(
                "exported {} item(s) to {}",
                report.items,
                report.out.display()
            );
            println!("  columns: {}", report.columns.join(", "));
        }
        _ => {}
    }
    Ok(ExitCode::SUCCESS)
}

/// Split `--columns` into property terms, dropping empty cells.
fn parse_columns(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(|part| part.trim().to_string())
        .filter(|part| !part.is_empty())
        .collect()
}

async fn run_media(args: &ArgMatches) -> Result<ExitCode, CliError> {
    if let Some(("sweep", m)) = args.subcommand() {
        let delete = m.get_flag("delete");
        let report = media_sweep::run(&SweepOptions {
            database: location(m)?,
            store: value_of(m, "store").map(PathBuf::from),
            delete,
            min_age: MIN_AGE,
        })
        .await?;
        for path in &report.orphans {
            match report.failed.iter().find(|(failed, _)| failed == path) {
                Some((_, reason)) => println!("failed to remove {path}: {reason}"),
                None if delete => println!("removed {path}"),
                None => println!("orphan {path}"),
            }
        }
        let verb = if delete { "removed" } else { "found" };
        println!(
            "{verb} {}, failed {}, skipped {} younger than 10 minutes{}",
            report.orphans.len() - report.failed.len(),
            report.failed.len(),
            report.young,
            if delete || report.orphans.is_empty() {
                ""
            } else {
                "; run again with --delete to remove them"
            }
        );
        if !report.failed.is_empty() {
            return Ok(ExitCode::from(1));
        }
        return Ok(ExitCode::SUCCESS);
    }
    if let Some(("derivatives", m)) = args.subcommand() {
        let report = media_derivatives_cmd::run(&DerivativeOptions {
            database: location(m)?,
            store: value_of(m, "store").map(PathBuf::from),
            retry: m.get_flag("retry"),
            redo: redo_of(m)?,
            media: optional_id(m, "media")?,
        })
        .await?;
        print_derivative_report(&report);
        let failed = report
            .rows
            .iter()
            .filter(|row| row.state == omeka_db::rows::DerivativeState::Failed)
            .count();
        if failed > 0 {
            return Ok(ExitCode::from(1));
        }
    }
    if let Some(("thumbnails", m)) = args.subcommand() {
        let report = media_thumbnails::run(&ThumbnailOptions {
            database: location(m)?,
            store: value_of(m, "store").map(PathBuf::from),
            force: m.get_flag("force"),
        })
        .await?;
        for result in &report.results {
            let name = result.file.as_deref().unwrap_or("-");
            match result.status {
                ThumbnailStatus::Generated => {
                    println!("generated thumbnails for media {} ({name})", result.media);
                }
                ThumbnailStatus::Skipped => println!(
                    "skipped media {} ({name}): {}",
                    result.media,
                    result.message.as_deref().unwrap_or("nothing to do")
                ),
                ThumbnailStatus::Failed => println!(
                    "failed media {} ({name}): {}",
                    result.media,
                    result.message.as_deref().unwrap_or("no reason given")
                ),
            }
        }
        println!(
            "generated {}, skipped {}, failed {} of {}",
            report.generated,
            report.skipped,
            report.failed,
            report.results.len()
        );
        if report.failed > 0 {
            return Ok(ExitCode::from(1));
        }
    }
    Ok(ExitCode::SUCCESS)
}

/// What `romeka media derivatives` found and did, one line per media and kind.
fn print_derivative_report(report: &DerivativeReport) {
    for row in &report.rows {
        let name = row.file.as_deref().unwrap_or("-");
        let reason = row
            .reason
            .as_deref()
            .map(|reason| format!(": {reason}"))
            .unwrap_or_default();
        println!(
            "media {} ({name}): {} {}{reason}",
            row.media,
            row.kind.name(),
            row.state.name()
        );
    }
    println!(
        "queued {}, retried {}, redone {}, waiting {}, already ready {}, nothing to do {}",
        report.queued, report.retried, report.redone, report.waiting, report.ready, report.skipped
    );
    if report.given_up > 0 {
        println!(
            "left {} failed after {} attempts; retry one by hand from its media page",
            report.given_up,
            media_derivatives_cmd::RETRY_ATTEMPTS
        );
    }
}

/// The `--redo` value: no flag, every ready copy, or one target kind.
fn redo_of(args: &ArgMatches) -> Result<Option<media_derivatives_cmd::Redo>, CliError> {
    if !args.contains_id("redo") {
        return Ok(None);
    }
    match value_of(args, "redo") {
        None | Some("") => Ok(Some(media_derivatives_cmd::Redo::All)),
        Some(value) => media_derivatives_cmd::RedoKind::parse(value)
            .map(|kind| Some(media_derivatives_cmd::Redo::Kind(kind)))
            .ok_or_else(|| {
                CliError::message(format!(
                    "--redo takes {} or no word, not \"{value}\"",
                    media_derivatives_cmd::RedoKind::ALL
                        .iter()
                        .map(|kind| kind.name())
                        .collect::<Vec<_>>()
                        .join(", ")
                ))
            }),
    }
}

async fn run_backup(args: &ArgMatches) -> Result<ExitCode, CliError> {
    let report = backup::run(&BackupOptions {
        database: location(args)?,
        store: PathBuf::from(value(args, "store")?),
        out: PathBuf::from(value(args, "out")?),
    })
    .await?;
    println!(
        "wrote {} ({} bytes: database {}, store {} files in {} bytes)",
        report.archive.display(),
        report.size,
        report.database_size,
        report.store_files,
        report.store_size
    );
    println!("  counts: {}", format_counts(&report.counts));
    Ok(ExitCode::SUCCESS)
}

async fn run_restore(args: &ArgMatches) -> Result<ExitCode, CliError> {
    let database = match location(args)? {
        Location::File(path) => path,
        server @ Location::Url(_) => {
            return Err(CliError::message(format!(
                "restore writes a SQLite file, so it cannot restore into {server}; load a MySQL \
                 dump with the server's own tools"
            )));
        }
    };
    let report = restore::run(&RestoreOptions {
        database,
        store: PathBuf::from(value(args, "store")?),
        input: PathBuf::from(value(args, "in")?),
        force: args.get_flag("force"),
    })
    .await?;
    println!(
        "restored {} store files from an archive written by commit {} (version {})",
        report.store_files, report.commit, report.version
    );
    println!("  counts: {}", format_counts(&report.counts));
    Ok(ExitCode::SUCCESS)
}

/// The manifest counts on one line, in table order.
fn format_counts(counts: &std::collections::BTreeMap<String, i64>) -> String {
    counts
        .iter()
        .map(|(table, count)| format!("{table} {count}"))
        .collect::<Vec<_>>()
        .join(", ")
}

async fn run_fixity(args: &ArgMatches) -> Result<ExitCode, CliError> {
    let Some(("check", m)) = args.subcommand() else {
        return Ok(ExitCode::SUCCESS);
    };
    let report = fixity_cmd::check(&FixityOptions {
        database: location(m)?,
        store: value_of(m, "store").map(PathBuf::from),
    })
    .await?;
    let record = &report.record;
    for (word, runs) in [
        ("differs", &record.differs),
        ("missing", &record.missing),
        ("unreadable", &record.unreadable),
    ] {
        for id in runs.ids() {
            println!("media {id}: {word}");
        }
    }
    println!(
        "job {} {}: checked {} files, {} ok, {} differ, {} missing, {} unreadable",
        report.job.0,
        report.status.as_str(),
        record.ok.len() + record.differs.len() + record.missing.len() + record.unreadable.len(),
        record.ok.len(),
        record.differs.len(),
        record.missing.len(),
        record.unreadable.len(),
    );
    Ok(if report.has_problems() {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    })
}

async fn run_job(args: &ArgMatches) -> Result<ExitCode, CliError> {
    match args.subcommand() {
        Some(("run", m)) => {
            let id = required_id(m)?;
            let database = location(m)?;
            let options = job_cmd::RunOptions {
                database,
                id,
                store: value_of(m, "store").map(PathBuf::from),
                base_url: value_of(m, "base-url").map(str::to_string),
                archive_root: value_of(m, "archive-root").map(PathBuf::from),
            };
            match job_cmd::run(&options).await {
                Ok(status) => println!("job {} {}", id, status.as_str()),
                Err(CliError::Job(error)) => {
                    eprintln!("job {id} error: {error}");
                    return Ok(ExitCode::from(1));
                }
                Err(other) => return Err(other),
            }
        }
        Some(("list", m)) => {
            let rows = job_cmd::list(&job_cmd::ListOptions {
                database: location(m)?,
            })
            .await?;
            if rows.is_empty() {
                println!("no jobs");
            }
            for row in rows {
                println!(
                    "{}\t{}\t{}\t{}\t{}\t{}",
                    row.id.0,
                    row.status.as_deref().unwrap_or("-"),
                    row.class,
                    row.pid.as_deref().unwrap_or("-"),
                    row.started,
                    row.ended
                        .map_or_else(|| "-".to_string(), |ended| ended.to_string())
                );
            }
        }
        Some(("show", m)) => {
            let id = required_id(m)?;
            let row = job_cmd::show(&job_cmd::ShowOptions {
                database: location(m)?,
                id,
            })
            .await?;
            let Some(row) = row else {
                eprintln!("no job with id {id}");
                return Ok(ExitCode::from(1));
            };
            println!("id: {}", row.id.0);
            println!("status: {}", row.status.as_deref().unwrap_or("-"));
            println!("class: {}", row.class);
            println!("pid: {}", row.pid.as_deref().unwrap_or("-"));
            println!("started: {}", row.started);
            println!(
                "ended: {}",
                row.ended
                    .map_or_else(|| "-".to_string(), |ended| ended.to_string())
            );
            println!("log:");
            let log = row.log.as_deref().unwrap_or("");
            print!("{log}");
            if !log.is_empty() && !log.ends_with('\n') {
                println!();
            }
        }
        Some(("stop", m)) => {
            let id = required_id(m)?;
            let changed = job_cmd::stop(&job_cmd::StopOptions {
                database: location(m)?,
                id,
            })
            .await?;
            if changed {
                println!("stopping job {id}");
            } else {
                eprintln!("job {id} is not in progress, so it cannot be stopped");
                return Ok(ExitCode::from(1));
            }
        }
        _ => {}
    }
    Ok(ExitCode::SUCCESS)
}

async fn run_serve(args: &ArgMatches) -> Result<ExitCode, CliError> {
    let host = value(args, "host")?;
    let port: u16 = value(args, "port")?
        .parse()
        .map_err(|error| CliError::message(format!("invalid --port: {error}")))?;
    let database = location(args)?;
    let base_url =
        value_of(args, "base-url").map_or_else(|| format!("http://{host}:{port}"), str::to_string);
    println!("serving on {host}:{port} with {database}");
    let store = value_of(args, "store").map(PathBuf::from);
    commands::serve::run(host, port, &database, &base_url, store.as_deref()).await?;
    Ok(ExitCode::SUCCESS)
}

/// The database the `--database` or `--database-url` arguments name.
/// `--database` wins over `ROMEKA_DATABASE_URL`, as a flag wins over the
/// environment everywhere else, so only the two flags together are refused.
/// clap 4's `conflicts_with` counts the environment too, so the check is here.
fn location(args: &ArgMatches) -> Result<Location, CliError> {
    if let Some(path) = value_of(args, "database") {
        if args.value_source("database-url") == Some(ValueSource::CommandLine) {
            return Err(CliError::message(
                "the argument '--database <PATH>' cannot be used with '--database-url <URL>'",
            ));
        }
        return Ok(Location::File(PathBuf::from(path)));
    }
    if let Some(url) = value_of(args, "database-url") {
        return match Location::parse(url) {
            url @ Location::Url(_) => Ok(url),
            Location::File(_) => Err(CliError::message(
                "--database-url takes a mysql:// or mariadb:// URL; a SQLite file is --database",
            )),
        };
    }
    Err(CliError::message(
        "missing argument --database or --database-url",
    ))
}

fn value_of<'a>(args: &'a ArgMatches, name: &str) -> Option<&'a str> {
    args.get_one::<String>(name).map(String::as_str)
}

fn value<'a>(args: &'a ArgMatches, name: &str) -> Result<&'a str, CliError> {
    value_of(args, name).ok_or_else(|| CliError::message(format!("missing argument --{name}")))
}

fn optional_id(args: &ArgMatches, name: &str) -> Result<Option<u64>, CliError> {
    value_of(args, name)
        .map(|value| {
            value
                .parse()
                .map_err(|error| CliError::message(format!("invalid --{name}: {error}")))
        })
        .transpose()
}

/// The required positional `id` of a `job` subcommand.
fn required_id(args: &ArgMatches) -> Result<u64, CliError> {
    value(args, "id")?
        .parse()
        .map_err(|error| CliError::message(format!("invalid job id: {error}")))
}
