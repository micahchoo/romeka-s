//! The engine a server test runs on: SQLite in memory, or a fresh MySQL or MariaDB database when
//! `ROMEKA_MYSQL_URL` names a server (`mysql://root:secret@127.0.0.1:13306`, no database).
//!
//! The same test bodies then run on either engine, so a MySQL run is
//! `ROMEKA_MYSQL_URL=... cargo test -p omeka-server --test api_read`. Each test gets its own
//! database, named `romeka_srv_<pid>_<n>`. A database outlives its test, because a test has no
//! async teardown; the first test of a later run drops every such database whose process is gone.

use std::sync::atomic::{AtomicU64, Ordering};

use omeka_db::DbPool;
use tokio::sync::OnceCell;

const PREFIX: &str = "romeka_srv_";

/// A migrated pool on the engine the environment chooses.
pub async fn pool() -> DbPool {
    match mysql_url() {
        Some(base) => mysql(&base).await,
        None => {
            let pool = omeka_db::open_memory().await.unwrap();
            omeka_db::migrate(&pool).await.unwrap();
            pool.into()
        }
    }
}

/// True when the tests run on MySQL, for an assertion that is engine-specific.
pub fn is_mysql() -> bool {
    mysql_url().is_some()
}

fn mysql_url() -> Option<String> {
    std::env::var("ROMEKA_MYSQL_URL")
        .ok()
        .map(|url| url.trim().to_string())
        .filter(|url| !url.is_empty())
}

async fn mysql(base: &str) -> DbPool {
    static SWEPT: OnceCell<()> = OnceCell::const_new();
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let admin = omeka_db::Location::parse(base)
        .open()
        .await
        .unwrap_or_else(|error| panic!("ROMEKA_MYSQL_URL does not answer: {error}"));
    let DbPool::MySql(admin) = admin else {
        panic!("ROMEKA_MYSQL_URL must be a mysql:// URL");
    };
    SWEPT.get_or_init(|| sweep(&admin)).await;
    let name = format!(
        "{PREFIX}{}_{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    );
    sqlx::query(&format!("DROP DATABASE IF EXISTS `{name}`"))
        .execute(&admin)
        .await
        .unwrap();
    sqlx::query(&format!(
        "CREATE DATABASE `{name}` CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci"
    ))
    .execute(&admin)
    .await
    .unwrap();
    admin.close().await;
    let pool = omeka_db::Location::parse(&with_database(base, &name))
        .open()
        .await
        .unwrap();
    omeka_db::migrate(&pool).await.unwrap();
    pool
}

/// Drop the databases of test processes that have ended. A process is alive while
/// `/proc/<pid>` exists.
async fn sweep(admin: &sqlx::MySqlPool) {
    let names: Vec<String> = sqlx::query_scalar(
        "SELECT CAST(schema_name AS CHAR) FROM information_schema.schemata
         WHERE schema_name LIKE 'romeka\\_srv\\_%'",
    )
    .fetch_all(admin)
    .await
    .unwrap();
    for name in names {
        let pid = name[PREFIX.len()..].split('_').next().unwrap_or_default();
        if !std::path::Path::new(&format!("/proc/{pid}")).exists() {
            let _ = sqlx::query(&format!("DROP DATABASE IF EXISTS `{name}`"))
                .execute(admin)
                .await;
        }
    }
}

/// Replace a URL's database with `name`, keeping the authority and any query string.
fn with_database(base: &str, name: &str) -> String {
    let (head, query) = match base.split_once('?') {
        Some((head, query)) => (head, Some(query)),
        None => (base, None),
    };
    let authority_end = head.find("://").map_or(0, |index| index + 3);
    let path_end = head[authority_end..]
        .find('/')
        .map_or(head.len(), |index| authority_end + index);
    let mut url = format!("{}/{name}", &head[..path_end]);
    if let Some(query) = query {
        url.push('?');
        url.push_str(query);
    }
    url
}
