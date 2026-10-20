//! `--database-url` through the `romeka` binary: what it accepts, what backup and restore refuse
//! on a server, and that a job child gets the URL through the environment. The MySQL steps run
//! when `ROMEKA_MYSQL_URL` names a server (no database); they print `SKIP` otherwise.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::process::{Command, Output};

use omeka_cli::jobs::{DATABASE_URL_ENV, JobCommand};
use omeka_db::Location;
use omeka_db::ids::JobId;

fn romeka(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_romeka"))
        .args(args)
        .env_remove(DATABASE_URL_ENV)
        .output()
        .expect("run romeka")
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn a_sqlite_path_is_not_a_database_url() {
    let output = romeka(&["user", "list", "--database-url", "romeka.db"]);
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("--database-url takes a mysql:// or mariadb:// URL"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn the_two_database_arguments_exclude_each_other() {
    let output = romeka(&[
        "user",
        "list",
        "--database",
        "romeka.db",
        "--database-url",
        "mysql://127.0.0.1/omeka",
    ]);
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("cannot be used with"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn a_database_flag_wins_over_the_url_in_the_environment() {
    let dir = tempfile::tempdir().unwrap();
    let database = dir.path().join("romeka.db");
    let output = Command::new(env!("CARGO_BIN_EXE_romeka"))
        .args(["user", "list", "--database", database.to_str().unwrap()])
        .env(DATABASE_URL_ENV, "mysql://romeka:s3cret@127.0.0.1:1/omeka")
        .output()
        .expect("run romeka");
    assert!(
        database.exists(),
        "the SQLite file was opened: {}",
        stderr(&output)
    );
}

#[test]
fn restore_refuses_a_server_before_it_reads_the_archive() {
    let dir = tempfile::tempdir().unwrap();
    let store = dir.path().join("files");
    let input = dir.path().join("missing.tar");
    let output = romeka(&[
        "restore",
        "--database-url",
        "mysql://romeka:s3cret@127.0.0.1:1/omeka",
        "--store",
        store.to_str().unwrap(),
        "--in",
        input.to_str().unwrap(),
    ]);
    assert!(!output.status.success());
    let said = stderr(&output);
    assert!(said.contains("restore writes a SQLite file"), "{said}");
    assert!(
        said.contains("mysql://romeka:***@127.0.0.1:1/omeka"),
        "{said}"
    );
    assert!(
        !said.contains("s3cret"),
        "the password never prints: {said}"
    );
    assert!(!store.exists(), "a refused restore writes nothing");
}

#[test]
fn a_job_child_reads_a_server_url_from_its_environment_not_its_arguments() {
    let url = "mysql://romeka:s3cret@db:3306/omeka";
    let command = JobCommand::new(Location::parse(url))
        .command(JobId::from(7))
        .unwrap();
    let args: Vec<String> = command
        .get_args()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    assert_eq!(args, ["job", "run", "7"]);
    let env: Vec<(String, Option<String>)> = command
        .get_envs()
        .map(|(key, value)| {
            (
                key.to_string_lossy().into_owned(),
                value.map(|value| value.to_string_lossy().into_owned()),
            )
        })
        .collect();
    assert_eq!(env, [(DATABASE_URL_ENV.to_string(), Some(url.to_string()))]);

    let file = JobCommand::new(Location::parse("/srv/romeka.db"))
        .command(JobId::from(7))
        .unwrap();
    let args: Vec<String> = file
        .get_args()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    assert_eq!(args, ["job", "run", "7", "--database", "/srv/romeka.db"]);
}

/// A fresh database on `ROMEKA_MYSQL_URL`'s server, as a URL, or `None` to skip.
fn mysql_database(name: &str) -> Option<String> {
    mysql_admin(&format!("CREATE DATABASE `{name}_{}`", std::process::id()))?;
    let base = std::env::var("ROMEKA_MYSQL_URL").ok()?;
    Some(format!(
        "{}/{name}_{}",
        base.trim_end_matches('/'),
        std::process::id()
    ))
}

/// Run one statement on `ROMEKA_MYSQL_URL`'s server, or `None` to skip.
fn mysql_admin(statement: &str) -> Option<()> {
    let Ok(base) = std::env::var("ROMEKA_MYSQL_URL") else {
        eprintln!("SKIP: ROMEKA_MYSQL_URL is not set");
        return None;
    };
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        let Location::Url(_) = Location::parse(&base) else {
            panic!("ROMEKA_MYSQL_URL must be a mysql:// URL");
        };
        let admin = Location::parse(&base).open().await.unwrap();
        let omeka_db::DbPool::MySql(admin) = admin else {
            unreachable!("a mysql:// URL opens MySQL");
        };
        sqlx::query(statement).execute(&admin).await.unwrap();
        Some(())
    })
}

#[test]
fn backup_on_a_server_refuses_with_the_operators_tool_and_writes_nothing() {
    let Some(url) = mysql_database("romeka_cli_backup") else {
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("backup.tar");
    let store = dir.path().join("files");
    std::fs::create_dir_all(&store).unwrap();
    let installed = romeka(&[
        "install",
        "--database-url",
        &url,
        "--email",
        "admin@example.org",
        "--name",
        "Admin",
        "--password",
        "correct horse battery",
    ]);
    assert!(installed.status.success(), "{}", stderr(&installed));
    let output = romeka(&[
        "backup",
        "--database-url",
        &url,
        "--store",
        store.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
    ]);
    assert!(!output.status.success());
    let said = stderr(&output);
    assert!(said.contains("mysqldump --single-transaction"), "{said}");
    assert!(!out.exists(), "a refused backup writes no archive");
    assert_eq!(
        std::fs::read_dir(dir.path()).unwrap().count(),
        1,
        "and no staging folder"
    );
    mysql_admin(&format!(
        "DROP DATABASE `romeka_cli_backup_{}`",
        std::process::id()
    ));
}
