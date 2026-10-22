//! The `romeka` binary: clap parsing and command dispatch.

use std::future::Future;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{App, AppSettings, Arg, ArgMatches, SubCommand};
use omeka_db::Location;

use omeka_cli::commands::backup::{self as backup, BackupOptions};
use omeka_cli::commands::derivatives::{
    self as media_derivatives_cmd, DerivativeOptions, DerivativeReport,
};
use omeka_cli::commands::export::{self as export, CsvOptions as ExportCsvOptions, JsonldOptions};
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
    let matches = build_app().get_matches();
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

fn build_app<'a, 'b>() -> App<'a, 'b> {
    App::new("romeka")
        .version(env!("CARGO_PKG_VERSION"))
        .about("Romeka S command line")
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .subcommand(install_command())
        .subcommand(user_command())
        .subcommand(key_command())
        .subcommand(import_command())
        .subcommand(export_command())
        .subcommand(media_command())
        .subcommand(backup_cli())
        .subcommand(restore_cli())
        .subcommand(job_command())
        .subcommand(serve_command())
}

/// `--database PATH`, or `--database-url URL` (or `ROMEKA_DATABASE_URL`) for a
/// MySQL or MariaDB server; one of them is required.
fn database_args<'a, 'b>() -> [Arg<'a, 'b>; 2] {
    [
        Arg::with_name("database")
            .short("d")
            .long("database")
            .value_name("PATH")
            .takes_value(true)
            .required_unless("database-url")
            .conflicts_with("database-url")
            .help("SQLite database file"),
        Arg::with_name("database-url")
            .long("database-url")
            .value_name("URL")
            .takes_value(true)
            .env(omeka_cli::jobs::DATABASE_URL_ENV)
            .hide_env_values(true)
            .help("MySQL or MariaDB server, mysql://user:password@host:port/database"),
    ]
}

fn optional_store_arg<'a, 'b>() -> Arg<'a, 'b> {
    Arg::with_name("store")
        .long("store")
        .value_name("DIR")
        .takes_value(true)
        .help("File store root; defaults to `files` beside the database")
}

fn archive_root_arg<'a, 'b>() -> Arg<'a, 'b> {
    Arg::with_name("archive-root")
        .long("archive-root")
        .value_name("DIR")
        .takes_value(true)
        .help("Folder a plan's sources read, named `archive`; the configured roots otherwise")
}

fn email_arg<'a, 'b>() -> Arg<'a, 'b> {
    Arg::with_name("email")
        .long("email")
        .value_name("EMAIL")
        .takes_value(true)
        .required(true)
}

fn install_command<'a, 'b>() -> App<'a, 'b> {
    SubCommand::with_name("install")
        .about("Create a database and seed the default data")
        .args(&database_args())
        .arg(email_arg())
        .arg(
            Arg::with_name("name")
                .long("name")
                .value_name("NAME")
                .takes_value(true)
                .required(true),
        )
        .arg(
            Arg::with_name("password")
                .long("password")
                .value_name("PASSWORD")
                .takes_value(true)
                .required(true),
        )
        .arg(
            Arg::with_name("title")
                .long("title")
                .takes_value(true)
                .default_value("Romeka S"),
        )
        .arg(
            Arg::with_name("time-zone")
                .long("time-zone")
                .takes_value(true)
                .default_value("UTC"),
        )
        .arg(
            Arg::with_name("locale")
                .long("locale")
                .takes_value(true)
                .default_value("en"),
        )
}

fn user_command<'a, 'b>() -> App<'a, 'b> {
    SubCommand::with_name("user")
        .about("Manage user accounts")
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .subcommand(
            SubCommand::with_name("add")
                .about("Add a user")
                .args(&database_args())
                .arg(email_arg())
                .arg(
                    Arg::with_name("name")
                        .long("name")
                        .takes_value(true)
                        .required(true),
                )
                .arg(
                    Arg::with_name("password")
                        .long("password")
                        .takes_value(true),
                )
                .arg(
                    Arg::with_name("role")
                        .long("role")
                        .takes_value(true)
                        .default_value("researcher"),
                )
                .arg(
                    Arg::with_name("base-url")
                        .long("base-url")
                        .value_name("URL")
                        .takes_value(true)
                        .default_value("http://localhost:8080")
                        .help("Origin the invite link is built from"),
                ),
        )
        .subcommand(
            SubCommand::with_name("list")
                .about("List users")
                .args(&database_args())
                .arg(Arg::with_name("role").long("role").takes_value(true)),
        )
        .subcommand(
            SubCommand::with_name("set-password")
                .about("Replace a user's password")
                .args(&database_args())
                .arg(email_arg())
                .arg(
                    Arg::with_name("password")
                        .long("password")
                        .takes_value(true)
                        .required(true),
                ),
        )
        .subcommand(
            SubCommand::with_name("grant-admin")
                .about("Set a user's role to global_admin")
                .args(&database_args())
                .arg(email_arg()),
        )
}

fn key_command<'a, 'b>() -> App<'a, 'b> {
    SubCommand::with_name("key")
        .about("Manage API keys")
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .subcommand(
            SubCommand::with_name("add")
                .about("Mint an API key and print its credential once")
                .args(&database_args())
                .arg(
                    Arg::with_name("owner")
                        .long("owner")
                        .value_name("EMAIL")
                        .takes_value(true)
                        .required(true),
                )
                .arg(
                    Arg::with_name("label")
                        .long("label")
                        .value_name("TEXT")
                        .takes_value(true)
                        .default_value(""),
                ),
        )
        .subcommand(
            SubCommand::with_name("list")
                .about("List API keys")
                .args(&database_args()),
        )
        .subcommand(
            SubCommand::with_name("revoke")
                .about("Delete an API key")
                .args(&database_args())
                .arg(
                    Arg::with_name("identity")
                        .long("identity")
                        .value_name("IDENTITY")
                        .takes_value(true)
                        .required(true),
                ),
        )
}

fn import_command<'a, 'b>() -> App<'a, 'b> {
    SubCommand::with_name("import")
        .about("Import data")
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .subcommand(
            SubCommand::with_name("template")
                .about("Import a resource template from JSON")
                .args(&database_args())
                .arg(
                    Arg::with_name("file")
                        .long("file")
                        .value_name("FILE")
                        .takes_value(true)
                        .required(true),
                ),
        )
        .subcommand(
            SubCommand::with_name("run")
                .about("Check an import plan and run it in this process")
                .args(&database_args())
                .arg(
                    Arg::with_name("plan")
                        .long("plan")
                        .value_name("FILE")
                        .takes_value(true)
                        .required(true)
                        .help("An import v2 plan, as JSON"),
                )
                .arg(
                    Arg::with_name("as")
                        .long("as")
                        .value_name("EMAIL")
                        .takes_value(true)
                        .required(true)
                        .help("The administrator who owns the import"),
                )
                .arg(
                    Arg::with_name("dry-run")
                        .long("dry-run")
                        .help("Print the Check summary and write nothing"),
                )
                .arg(
                    Arg::with_name("confirm")
                        .long("confirm")
                        .value_name("COUNT")
                        .takes_value(true)
                        .help("The number of resources a delete plan removes"),
                )
                .arg(json_arg())
                .arg(optional_store_arg())
                .arg(archive_root_arg()),
        )
        .subcommand(
            SubCommand::with_name("undo")
                .about("Undo an import, or say what undoing it would do")
                .args(&database_args())
                .arg(
                    Arg::with_name("id")
                        .value_name("ID")
                        .takes_value(true)
                        .required(true)
                        .help("Import id"),
                )
                .arg(
                    Arg::with_name("force")
                        .long("force")
                        .value_name("all")
                        .takes_value(true)
                        .possible_values(&["all"])
                        .help("Restore or delete resources edited since the import wrote them"),
                )
                .arg(
                    Arg::with_name("preview")
                        .long("preview")
                        .help("Print what the undo would do and write nothing"),
                )
                .arg(json_arg())
                .arg(optional_store_arg()),
        )
        .subcommand(
            SubCommand::with_name("list")
                .about("List imports, newest first")
                .args(&database_args())
                .arg(
                    Arg::with_name("page")
                        .long("page")
                        .value_name("N")
                        .takes_value(true)
                        .default_value("1"),
                )
                .arg(
                    Arg::with_name("per-page")
                        .long("per-page")
                        .value_name("N")
                        .takes_value(true)
                        .default_value("25"),
                )
                .arg(json_arg()),
        )
}

fn export_command<'a, 'b>() -> App<'a, 'b> {
    SubCommand::with_name("export")
        .about("Write the API documents or an item CSV out of the database")
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .subcommand(
            SubCommand::with_name("jsonld")
                .about("Write the documents the read layer serves")
                .args(&database_args())
                .arg(
                    Arg::with_name("out")
                        .long("out")
                        .value_name("PATH")
                        .takes_value(true)
                        .required(true)
                        .help("An existing directory (one file per document) or the file to write"),
                )
                .arg(
                    Arg::with_name("resource")
                        .long("resource")
                        .value_name("NAME")
                        .takes_value(true)
                        .default_value("items")
                        .help("API resource name"),
                )
                .arg(
                    Arg::with_name("item-set")
                        .long("item-set")
                        .value_name("ID")
                        .takes_value(true)
                        .help("Keep only items in this item set"),
                )
                .arg(
                    Arg::with_name("base-url")
                        .long("base-url")
                        .takes_value(true)
                        .default_value("http://localhost")
                        .help("Origin for every @id in the documents"),
                ),
        )
        .subcommand(
            SubCommand::with_name("csv")
                .about("Write items as CSV, one column per value")
                .args(&database_args())
                .arg(
                    Arg::with_name("out")
                        .long("out")
                        .value_name("FILE")
                        .takes_value(true)
                        .required(true)
                        .help("The CSV file to write"),
                )
                .arg(
                    Arg::with_name("resource")
                        .long("resource")
                        .value_name("NAME")
                        .takes_value(true)
                        .default_value("items")
                        .help("Items only"),
                )
                .arg(
                    Arg::with_name("item-set")
                        .long("item-set")
                        .value_name("ID")
                        .takes_value(true)
                        .help("Keep only items in this item set"),
                )
                .arg(
                    Arg::with_name("columns")
                        .long("columns")
                        .value_name("TERMS")
                        .takes_value(true)
                        .help("Comma-separated property terms; defaults to every term in the data"),
                ),
        )
}

fn media_command<'a, 'b>() -> App<'a, 'b> {
    SubCommand::with_name("media")
        .about("Manage media")
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .subcommand(
            SubCommand::with_name("thumbnails")
                .about("Generate the thumbnails a media row is missing")
                .args(&database_args())
                .arg(
                    Arg::with_name("store")
                        .long("store")
                        .value_name("DIR")
                        .takes_value(true)
                        .help("File store root; defaults to `files` beside the database"),
                )
                .arg(
                    Arg::with_name("force")
                        .long("force")
                        .takes_value(false)
                        .help("Rebuild thumbnails that already exist"),
                ),
        )
        .subcommand(
            SubCommand::with_name("derivatives")
                .about("Queue the derivatives a media is missing, then run the worker")
                .args(&database_args())
                .arg(optional_store_arg())
                .arg(
                    Arg::with_name("retry")
                        .long("retry")
                        .takes_value(false)
                        .help("Queue rows a tool failed on or skipped; a row that failed 3 times stays failed"),
                )
                .arg(
                    Arg::with_name("redo")
                        .long("redo")
                        .value_name("KIND")
                        .takes_value(true)
                        .min_values(0)
                        .max_values(1)
                        .possible_values(&["video", "audio", "image", "document"])
                        .help(
                            "Make ready copies again: video, audio, image or document; \
                             every kind when the word is left out",
                        ),
                )
                .arg(
                    Arg::with_name("media")
                        .long("media")
                        .value_name("ID")
                        .takes_value(true)
                        .help("Only this media"),
                ),
        )
        .subcommand(
            SubCommand::with_name("sweep")
                .about("List, or with --delete remove, store files no media or asset row names")
                .args(&database_args())
                .arg(
                    Arg::with_name("store")
                        .long("store")
                        .value_name("DIR")
                        .takes_value(true)
                        .help("File store root; defaults to `files` beside the database"),
                )
                .arg(
                    Arg::with_name("delete")
                        .long("delete")
                        .takes_value(false)
                        .help("Remove the files; without it the run only reports them"),
                ),
        )
}

fn backup_cli<'a, 'b>() -> App<'a, 'b> {
    SubCommand::with_name("backup")
        .about("Archive the database and the store into one file")
        .args(&database_args())
        .arg(store_arg())
        .arg(
            Arg::with_name("out")
                .long("out")
                .value_name("FILE")
                .takes_value(true)
                .required(true)
                .help("The archive to write"),
        )
}

fn restore_cli<'a, 'b>() -> App<'a, 'b> {
    SubCommand::with_name("restore")
        .about("Write an archive back into a database and a store")
        .args(&database_args())
        .arg(store_arg())
        .arg(
            Arg::with_name("in")
                .long("in")
                .value_name("FILE")
                .takes_value(true)
                .required(true)
                .help("The archive to read"),
        )
        .arg(
            Arg::with_name("force")
                .long("force")
                .takes_value(false)
                .help("Overwrite an occupied database or store"),
        )
}

fn store_arg<'a, 'b>() -> Arg<'a, 'b> {
    Arg::with_name("store")
        .long("store")
        .value_name("DIR")
        .takes_value(true)
        .required(true)
        .help("File store root, the directory holding original/ and the thumbnails")
}

fn job_command<'a, 'b>() -> App<'a, 'b> {
    SubCommand::with_name("job")
        .about("Run and inspect background jobs")
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .subcommand(
            SubCommand::with_name("run")
                .about("Run one job in this process")
                .args(&database_args())
                .arg(id_arg())
                .arg(
                    Arg::with_name("store")
                        .long("store")
                        .value_name("DIR")
                        .takes_value(true)
                        .help("File store root; defaults to `files` beside the database"),
                )
                .arg(
                    Arg::with_name("base-url")
                        .long("base-url")
                        .takes_value(true)
                        .help("Base URL of the installation; defaults to http://localhost"),
                )
                .arg(
                    Arg::with_name("archive-root")
                        .long("archive-root")
                        .value_name("DIR")
                        .takes_value(true)
                        .help("Archive import root; the archive import is off without it"),
                ),
        )
        .subcommand(
            SubCommand::with_name("list")
                .about("List jobs")
                .args(&database_args()),
        )
        .subcommand(
            SubCommand::with_name("show")
                .about("Show a job and its captured log")
                .args(&database_args())
                .arg(id_arg()),
        )
        .subcommand(
            SubCommand::with_name("stop")
                .about("Ask a running job to stop")
                .args(&database_args())
                .arg(id_arg()),
        )
}

fn id_arg<'a, 'b>() -> Arg<'a, 'b> {
    Arg::with_name("id")
        .value_name("ID")
        .takes_value(true)
        .required(true)
        .help("Job id")
}

fn serve_command<'a, 'b>() -> App<'a, 'b> {
    SubCommand::with_name("serve")
        .about("Start the HTTP server")
        .args(&database_args())
        .arg(
            Arg::with_name("host")
                .long("host")
                .takes_value(true)
                .default_value("127.0.0.1"),
        )
        .arg(
            Arg::with_name("port")
                .long("port")
                .takes_value(true)
                .default_value("8080"),
        )
        .arg(
            Arg::with_name("base-url")
                .long("base-url")
                .takes_value(true)
                .help("Base URL for JSON-LD links; defaults to http://<host>:<port>"),
        )
        .arg(
            Arg::with_name("store")
                .long("store")
                .value_name("DIR")
                .takes_value(true)
                .help("File store root; defaults to `files` in the working directory"),
        )
}

async fn dispatch(matches: &ArgMatches<'_>) -> Result<ExitCode, CliError> {
    match matches.subcommand() {
        ("install", Some(args)) => run_install(args).await,
        ("user", Some(args)) => run_user(args).await,
        ("key", Some(args)) => run_key(args).await,
        ("import", Some(args)) => run_import(args).await,
        ("export", Some(args)) => run_export(args).await,
        ("media", Some(args)) => run_media(args).await,
        ("backup", Some(args)) => run_backup(args).await,
        ("restore", Some(args)) => run_restore(args).await,
        ("job", Some(args)) => run_job(args).await,
        ("serve", Some(args)) => run_serve(args).await,
        _ => Ok(ExitCode::SUCCESS),
    }
}

async fn run_install(args: &ArgMatches<'_>) -> Result<ExitCode, CliError> {
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

async fn run_user(args: &ArgMatches<'_>) -> Result<ExitCode, CliError> {
    match args.subcommand() {
        ("add", Some(m)) => {
            let added = user::add(&AddOptions {
                database: location(m)?,
                email: value(m, "email")?.to_string(),
                name: value(m, "name")?.to_string(),
                password: m.value_of("password").map(str::to_string),
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
        ("list", Some(m)) => {
            let rows = user::list(&ListOptions {
                database: location(m)?,
                role: m.value_of("role").map(str::to_string),
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
        ("set-password", Some(m)) => {
            let row = user::set_password(&SetPasswordOptions {
                database: location(m)?,
                email: value(m, "email")?.to_string(),
                password: value(m, "password")?.to_string(),
            })
            .await?;
            println!("password updated for {}", row.email);
        }
        ("grant-admin", Some(m)) => {
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

async fn run_key(args: &ArgMatches<'_>) -> Result<ExitCode, CliError> {
    match args.subcommand() {
        ("add", Some(m)) => {
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
        ("list", Some(m)) => {
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
        ("revoke", Some(m)) => {
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

fn json_arg<'a, 'b>() -> Arg<'a, 'b> {
    Arg::with_name("json")
        .long("json")
        .help("Write one JSON document when the command ends, and no progress lines")
}

fn format(args: &ArgMatches<'_>) -> Format {
    if args.is_present("json") {
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

/// The database, store and archive root an import command names.
fn import_at(args: &ArgMatches<'_>) -> Result<Where, CliError> {
    Ok(Where {
        database: location(args)?,
        store: args.value_of("store").map(PathBuf::from),
        archive_root: args.value_of("archive-root").map(PathBuf::from),
    })
}

async fn run_import(args: &ArgMatches<'_>) -> Result<ExitCode, CliError> {
    match args.subcommand() {
        ("template", Some(m)) => {
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
        ("run", Some(m)) => {
            let outcome = import_run::run(
                &ImportRunOptions {
                    at: import_at(m)?,
                    plan: PathBuf::from(value(m, "plan")?),
                    actor: value(m, "as")?.to_string(),
                    dry_run: m.is_present("dry-run"),
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
        ("undo", Some(m)) => {
            let outcome = import_undo::undo(
                &UndoOptions {
                    at: import_at(m)?,
                    id: value(m, "id")?.parse().map_err(|error| {
                        CliError::message(format!("invalid import id: {error}"))
                    })?,
                    force_all: m.value_of("force") == Some("all"),
                    preview: m.is_present("preview"),
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
        ("list", Some(m)) => {
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

async fn run_export(args: &ArgMatches<'_>) -> Result<ExitCode, CliError> {
    match args.subcommand() {
        ("jsonld", Some(m)) => {
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
        ("csv", Some(m)) => {
            let resource = value(m, "resource")?;
            if resource != "items" {
                return Err(CliError::message(format!(
                    "CSV export writes items, not {resource}"
                )));
            }
            let columns = m.value_of("columns").map(parse_columns).unwrap_or_default();
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

async fn run_media(args: &ArgMatches<'_>) -> Result<ExitCode, CliError> {
    if let ("sweep", Some(m)) = args.subcommand() {
        let delete = m.is_present("delete");
        let report = media_sweep::run(&SweepOptions {
            database: location(m)?,
            store: m.value_of("store").map(PathBuf::from),
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
    if let ("derivatives", Some(m)) = args.subcommand() {
        let report = media_derivatives_cmd::run(&DerivativeOptions {
            database: location(m)?,
            store: m.value_of("store").map(PathBuf::from),
            retry: m.is_present("retry"),
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
    if let ("thumbnails", Some(m)) = args.subcommand() {
        let report = media_thumbnails::run(&ThumbnailOptions {
            database: location(m)?,
            store: m.value_of("store").map(PathBuf::from),
            force: m.is_present("force"),
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
fn redo_of(args: &ArgMatches<'_>) -> Result<Option<media_derivatives_cmd::Redo>, CliError> {
    if !args.is_present("redo") {
        return Ok(None);
    }
    match args.value_of("redo") {
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

async fn run_backup(args: &ArgMatches<'_>) -> Result<ExitCode, CliError> {
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

async fn run_restore(args: &ArgMatches<'_>) -> Result<ExitCode, CliError> {
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
        force: args.is_present("force"),
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

async fn run_job(args: &ArgMatches<'_>) -> Result<ExitCode, CliError> {
    match args.subcommand() {
        ("run", Some(m)) => {
            let id = required_id(m)?;
            let database = location(m)?;
            let options = job_cmd::RunOptions {
                database,
                id,
                store: m.value_of("store").map(PathBuf::from),
                base_url: m.value_of("base-url").map(str::to_string),
                archive_root: m.value_of("archive-root").map(PathBuf::from),
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
        ("list", Some(m)) => {
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
        ("show", Some(m)) => {
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
        ("stop", Some(m)) => {
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

async fn run_serve(args: &ArgMatches<'_>) -> Result<ExitCode, CliError> {
    let host = value(args, "host")?;
    let port: u16 = value(args, "port")?
        .parse()
        .map_err(|error| CliError::message(format!("invalid --port: {error}")))?;
    let database = location(args)?;
    let base_url = args
        .value_of("base-url")
        .map_or_else(|| format!("http://{host}:{port}"), str::to_string);
    println!("serving on {host}:{port} with {database}");
    let store = args.value_of("store").map(PathBuf::from);
    commands::serve::run(host, port, &database, &base_url, store.as_deref()).await?;
    Ok(ExitCode::SUCCESS)
}

/// The database the `--database` or `--database-url` arguments name. clap
/// refuses both flags at once, so `--database` here wins only over
/// `ROMEKA_DATABASE_URL`, as a flag wins over the environment everywhere else.
fn location(args: &ArgMatches<'_>) -> Result<Location, CliError> {
    if let Some(path) = args.value_of("database") {
        return Ok(Location::File(PathBuf::from(path)));
    }
    if let Some(url) = args.value_of("database-url") {
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

fn value<'a>(args: &'a ArgMatches<'_>, name: &str) -> Result<&'a str, CliError> {
    args.value_of(name)
        .ok_or_else(|| CliError::message(format!("missing argument --{name}")))
}

fn optional_id(args: &ArgMatches<'_>, name: &str) -> Result<Option<u64>, CliError> {
    args.value_of(name)
        .map(|value| {
            value
                .parse()
                .map_err(|error| CliError::message(format!("invalid --{name}: {error}")))
        })
        .transpose()
}

/// The required positional `id` of a `job` subcommand.
fn required_id(args: &ArgMatches<'_>) -> Result<u64, CliError> {
    value(args, "id")?
        .parse()
        .map_err(|error| CliError::message(format!("invalid job id: {error}")))
}
