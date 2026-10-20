//! The `romeka` binary: clap parsing and command dispatch.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{App, AppSettings, Arg, ArgMatches, SubCommand};

use omeka_cli::commands::backup::{self as backup, BackupOptions};
use omeka_cli::commands::export::{self as export, CsvOptions as ExportCsvOptions, JsonldOptions};
use omeka_cli::commands::import::{CsvOptions, TemplateOptions};
use omeka_cli::commands::install::{self, InstallOptions};
use omeka_cli::commands::job::{self as job_cmd};
use omeka_cli::commands::key::{
    self, AddOptions as KeyAddOptions, ListOptions as KeyListOptions,
    RevokeOptions as KeyRevokeOptions,
};
use omeka_cli::commands::media::{self, MediaOptions, MediaStatus};
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

fn database_arg<'a, 'b>() -> Arg<'a, 'b> {
    Arg::with_name("database")
        .short("d")
        .long("database")
        .value_name("PATH")
        .takes_value(true)
        .required(true)
        .help("SQLite database file")
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
        .arg(database_arg())
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
                .arg(database_arg())
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
                .arg(database_arg())
                .arg(Arg::with_name("role").long("role").takes_value(true)),
        )
        .subcommand(
            SubCommand::with_name("set-password")
                .about("Replace a user's password")
                .arg(database_arg())
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
                .arg(database_arg())
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
                .arg(database_arg())
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
                .arg(database_arg()),
        )
        .subcommand(
            SubCommand::with_name("revoke")
                .about("Delete an API key")
                .arg(database_arg())
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
                .arg(database_arg())
                .arg(
                    Arg::with_name("file")
                        .long("file")
                        .value_name("FILE")
                        .takes_value(true)
                        .required(true),
                ),
        )
        .subcommand(
            SubCommand::with_name("csv")
                .about("Import items from a CSV into an item set")
                .arg(database_arg())
                .arg(
                    Arg::with_name("file")
                        .long("file")
                        .value_name("FILE")
                        .takes_value(true)
                        .required(true),
                )
                .arg(
                    Arg::with_name("item-set")
                        .long("item-set")
                        .value_name("ID")
                        .takes_value(true)
                        .required(true),
                ),
        )
        .subcommand(
            SubCommand::with_name("media")
                .about("Import media files and link each to its item")
                .arg(database_arg())
                .arg(
                    Arg::with_name("source")
                        .long("source")
                        .value_name("DIR")
                        .takes_value(true)
                        .help("Directory of media files; the base for a manifest's paths"),
                )
                .arg(
                    Arg::with_name("manifest")
                        .long("manifest")
                        .value_name("FILE")
                        .takes_value(true)
                        .help("A media.json manifest (verification/acceptance/media.py)"),
                )
                .arg(
                    Arg::with_name("store")
                        .long("store")
                        .value_name("DIR")
                        .takes_value(true)
                        .help("File store root; defaults to `files` beside the database"),
                )
                .arg(
                    Arg::with_name("item-set")
                        .long("item-set")
                        .value_name("ID")
                        .takes_value(true)
                        .help("Restrict the identifier lookup to this item set"),
                )
                .arg(
                    Arg::with_name("item")
                        .long("item")
                        .value_name("ID")
                        .takes_value(true)
                        .help("Link every file to this item, ignoring the identifier"),
                ),
        )
}

fn export_command<'a, 'b>() -> App<'a, 'b> {
    SubCommand::with_name("export")
        .about("Write the API documents or an item CSV out of the database")
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .subcommand(
            SubCommand::with_name("jsonld")
                .about("Write the documents the read layer serves")
                .arg(database_arg())
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
                .arg(database_arg())
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
                        .help("Items only; the round trip reads back through import csv"),
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
                .arg(database_arg())
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
            SubCommand::with_name("sweep")
                .about("List, or with --delete remove, store files no media or asset row names")
                .arg(database_arg())
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
        .arg(database_arg())
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
        .arg(database_arg())
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
                .arg(database_arg())
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
                .arg(database_arg()),
        )
        .subcommand(
            SubCommand::with_name("show")
                .about("Show a job and its captured log")
                .arg(database_arg())
                .arg(id_arg()),
        )
        .subcommand(
            SubCommand::with_name("stop")
                .about("Ask a running job to stop")
                .arg(database_arg())
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
        .arg(database_arg())
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
        database: PathBuf::from(value(args, "database")?),
        email: value(args, "email")?.to_string(),
        name: value(args, "name")?.to_string(),
        password: value(args, "password")?.to_string(),
        title: value(args, "title")?.to_string(),
        time_zone: value(args, "time-zone")?.to_string(),
        locale: value(args, "locale")?.to_string(),
    };
    let report = install::run(&options).await?;
    println!("installed {}", report.database.display());
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
                database: PathBuf::from(value(m, "database")?),
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
                database: PathBuf::from(value(m, "database")?),
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
                database: PathBuf::from(value(m, "database")?),
                email: value(m, "email")?.to_string(),
                password: value(m, "password")?.to_string(),
            })
            .await?;
            println!("password updated for {}", row.email);
        }
        ("grant-admin", Some(m)) => {
            let row = user::grant_admin(&GrantAdminOptions {
                database: PathBuf::from(value(m, "database")?),
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
                database: PathBuf::from(value(m, "database")?),
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
                database: PathBuf::from(value(m, "database")?),
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
                database: PathBuf::from(value(m, "database")?),
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

async fn run_import(args: &ArgMatches<'_>) -> Result<ExitCode, CliError> {
    match args.subcommand() {
        ("template", Some(m)) => {
            let template = commands::import::import_template(&TemplateOptions {
                database: PathBuf::from(value(m, "database")?),
                file: PathBuf::from(value(m, "file")?),
            })
            .await?;
            println!(
                "imported template \"{}\" (id {})",
                template.label, template.id.0
            );
        }
        ("csv", Some(m)) => {
            let item_set_id: u64 = value(m, "item-set")?
                .parse()
                .map_err(|error| CliError::message(format!("invalid --item-set: {error}")))?;
            let report = commands::import::import_csv(&CsvOptions {
                database: PathBuf::from(value(m, "database")?),
                file: PathBuf::from(value(m, "file")?),
                item_set_id,
            })
            .await?;
            println!(
                "imported {} item(s) into item set {}",
                report.imported, report.item_set
            );
            for error in &report.errors {
                eprintln!("line {}: {}", error.line, error.message);
            }
            if !report.errors.is_empty() {
                return Ok(ExitCode::from(1));
            }
        }
        ("media", Some(m)) => {
            let report = media::run(&MediaOptions {
                database: PathBuf::from(value(m, "database")?),
                source: m.value_of("source").map(PathBuf::from),
                manifest: m.value_of("manifest").map(PathBuf::from),
                store: m.value_of("store").map(PathBuf::from),
                item_set: optional_id(m, "item-set")?,
                item: optional_id(m, "item")?,
            })
            .await?;
            for result in &report.results {
                match result.status {
                    MediaStatus::Imported => println!(
                        "imported {} -> item {}, media {}",
                        result.file,
                        display_id(result.item),
                        display_id(result.media)
                    ),
                    MediaStatus::Skipped => println!(
                        "skipped {} (item {} already has it)",
                        result.file,
                        display_id(result.item)
                    ),
                    MediaStatus::Failed => println!(
                        "failed {}: {}",
                        result.file,
                        result.message.as_deref().unwrap_or("no reason given")
                    ),
                }
            }
            println!(
                "imported {}, skipped {}, failed {} of {} ({} with thumbnails)",
                report.imported,
                report.skipped,
                report.failed,
                report.results.len(),
                report.thumbnails
            );
            if report.failed > 0 {
                return Ok(ExitCode::from(1));
            }
        }
        _ => {}
    }
    Ok(ExitCode::SUCCESS)
}

async fn run_export(args: &ArgMatches<'_>) -> Result<ExitCode, CliError> {
    match args.subcommand() {
        ("jsonld", Some(m)) => {
            let report = export::run_jsonld(&JsonldOptions {
                database: PathBuf::from(value(m, "database")?),
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
                    "CSV export reads back through `import csv`, so it writes items, not {resource}"
                )));
            }
            let columns = m.value_of("columns").map(parse_columns).unwrap_or_default();
            let report = export::run_csv(&ExportCsvOptions {
                database: PathBuf::from(value(m, "database")?),
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
            database: PathBuf::from(value(m, "database")?),
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
    if let ("thumbnails", Some(m)) = args.subcommand() {
        let report = media_thumbnails::run(&ThumbnailOptions {
            database: PathBuf::from(value(m, "database")?),
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

async fn run_backup(args: &ArgMatches<'_>) -> Result<ExitCode, CliError> {
    let report = backup::run(&BackupOptions {
        database: PathBuf::from(value(args, "database")?),
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
    let report = restore::run(&RestoreOptions {
        database: PathBuf::from(value(args, "database")?),
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
            let database = PathBuf::from(value(m, "database")?);
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
                database: PathBuf::from(value(m, "database")?),
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
                database: PathBuf::from(value(m, "database")?),
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
                database: PathBuf::from(value(m, "database")?),
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
    let database = value(args, "database")?;
    let base_url = args
        .value_of("base-url")
        .map_or_else(|| format!("http://{host}:{port}"), str::to_string);
    println!("serving on {host}:{port} with {database}");
    let store = args.value_of("store").map(PathBuf::from);
    commands::serve::run(host, port, database, &base_url, store.as_deref()).await?;
    Ok(ExitCode::SUCCESS)
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

fn display_id(id: Option<u64>) -> String {
    id.map_or_else(|| "-".to_string(), |id| id.to_string())
}
