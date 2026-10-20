//! `DatabaseRepo`: the whole-database operations (the readiness ping, a table's row count and the
//! consistent snapshot a backup archives), on SQLite and, with the `mysql` feature, on MySQL and
//! MariaDB.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;
mod import_db;

use import_db::on_both;
use omeka_db::repos::{DatabaseRepo, ItemRepo};
use omeka_db::rows::NewItem;
use omeka_db::{Backend, DbError};

on_both!(a_ping_answers_and_a_count_reads_the_table, |pool| {
    let database = DatabaseRepo::<Db>::new(pool);
    database.ping().await.unwrap();
    assert_eq!(database.count_rows("item").await.unwrap(), 0);
    let items = ItemRepo::<Db>::new(pool);
    items.create(&NewItem::default()).await.unwrap();
    items.create(&NewItem::default()).await.unwrap();
    assert_eq!(database.count_rows("item").await.unwrap(), 2);
    assert_eq!(database.count_rows("resource").await.unwrap(), 2);
    assert_eq!(
        database.count_rows("value").await.unwrap(),
        0,
        "MariaDB reserves `value`"
    );
    assert_eq!(database.count_rows("user").await.unwrap(), 0);
});

on_both!(a_closed_pool_fails_the_ping, |pool| {
    pool.close().await;
    assert!(DatabaseRepo::<Db>::new(pool).ping().await.is_err());
});

on_both!(a_snapshot_is_sqlite_only, |pool| {
    ItemRepo::<Db>::new(pool)
        .create(&NewItem::default())
        .await
        .unwrap();
    let target = common::test_path("snapshot");
    common::remove_db(&target);
    let outcome = DatabaseRepo::<Db>::new(pool).snapshot_into(&target).await;
    if <Db as Backend>::NAME == "sqlite" {
        outcome.unwrap();
        let copy = omeka_db::open_file(&target).await.unwrap();
        assert_eq!(
            DatabaseRepo::new(&copy).count_rows("item").await.unwrap(),
            1,
            "the copy holds the rows the source held"
        );
        copy.close().await;
        common::remove_db(&target);
    } else {
        assert!(
            matches!(
                outcome,
                Err(DbError::Unsupported {
                    operation: "snapshot",
                    ..
                })
            ),
            "MySQL backup is the operator's: {outcome:?}"
        );
        assert!(!target.exists(), "a refused snapshot writes no file");
    }
});
