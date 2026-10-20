//! One test body, run on both engines: the import v2 suites (`docs/import-v2.md`, "Database").
//!
//! [`on_both!`] turns one body into a SQLite test and, with the `mysql` feature, a MySQL test, so the
//! two engines answer the same assertions. Inside the body `Db` names the engine and the named
//! pool is a `&Pool<Db>`. SQLite uses a file database, because a concurrent start needs more than
//! the one connection an in-memory pool holds.
#![allow(
    dead_code,
    unused_macros,
    unused_imports,
    clippy::unwrap_used,
    clippy::expect_used
)]

use omeka_db::rows::NewImport;
use serde_json::json;

macro_rules! on_both {
    ($name:ident, |$pool:ident| $body:block) => {
        mod $name {
            #[allow(unused_imports)]
            use super::*;

            #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
            async fn sqlite() {
                #[allow(dead_code)]
                type Db = sqlx::Sqlite;
                let (pool, path) = common::file(stringify!($name)).await;
                {
                    let $pool: &sqlx::Pool<Db> = &pool;
                    $body
                }
                pool.close().await;
                common::remove_db(&path);
            }

            #[cfg(feature = "mysql")]
            #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
            async fn mysql() {
                #[allow(dead_code)]
                type Db = sqlx::MySql;
                let Some(db) = common::mysql().await else {
                    return;
                };
                {
                    let $pool: &sqlx::Pool<Db> = &db.pool;
                    $body
                }
                db.cleanup().await;
            }
        }
    };
}

pub(crate) use on_both;

/// A draft whose sources and plan name `tag`, so two drafts differ.
pub fn new_import(tag: &str) -> NewImport {
    NewImport {
        owner_id: None,
        sources: json!([{ "id": tag, "kind": "folder", "root": "archive", "path": tag }]),
        plan: json!({ "version": 2, "tag": tag }),
        copied_from: None,
    }
}

/// A new draft's id. Expands where `Db` names the engine, inside an [`on_both!`] body.
macro_rules! draft {
    ($pool:expr, $tag:expr) => {
        omeka_db::repos::ImportRepo::<Db>::new($pool)
            .create_draft(&import_db::new_import($tag))
            .await
            .unwrap()
            .id
    };
}

pub(crate) use draft;

/// Run one statement that both engines accept, binding one id.
macro_rules! exec_id {
    ($pool:expr, $sql:expr, $id:expr) => {
        sqlx::query::<Db>($sql)
            .bind($id as i64)
            .execute($pool)
            .await
            .unwrap()
    };
}

pub(crate) use exec_id;
