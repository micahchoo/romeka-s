//! The undo states of `romeka_import` added with the import actions (`docs/import-v2.md`,
//! "Lifecycle and the ledger", "Undo details"): `partly_undone`, a delete that is never undone,
//! and a stop request that reaches an undo. Every test runs on SQLite and, with the `mysql`
//! feature, on MySQL.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;
mod import_db;

use import_db::{draft, on_both};
use omeka_db::repos::{ImportRepo, StartOutcome};
use omeka_db::rows::{ImportEnding, ImportStatus};

on_both!(
    a_partly_undone_import_reads_back_and_may_be_undone_again,
    |pool| {
        let imports = ImportRepo::<Db>::new(pool);
        let id = draft!(pool, "a");
        imports.start_run(id, 0).await.unwrap();
        imports.finish(id, ImportEnding::Done).await.unwrap();
        imports.start_undo(id).await.unwrap();
        assert!(
            imports
                .finish(id, ImportEnding::PartlyUndone)
                .await
                .unwrap()
        );
        assert_eq!(
            imports.get(id).await.unwrap().unwrap().status,
            ImportStatus::PartlyUndone
        );
        assert_eq!(imports.start_undo(id).await.unwrap(), StartOutcome::Started);
        assert!(imports.finish(id, ImportEnding::Undone).await.unwrap());
    }
);

on_both!(an_import_that_deleted_is_never_undone, |pool| {
    use omeka_db::ids::ItemId;
    use omeka_db::repos::ImportResourceRepo;
    use omeka_db::rows::{ImportChange, ImportTarget, NewImportResource};
    let imports = ImportRepo::<Db>::new(pool);
    let id = draft!(pool, "a");
    imports.start_run(id, 0).await.unwrap();
    ImportResourceRepo::<Db>::record(
        pool,
        id,
        &NewImportResource {
            target: ImportTarget::Item(ItemId(41)),
            record: "f:s1/a.jpg".to_string(),
            change: ImportChange::Deleted,
        },
    )
    .await
    .unwrap();
    imports.finish(id, ImportEnding::Done).await.unwrap();
    assert_eq!(imports.start_undo(id).await.unwrap(), StartOutcome::Deleted);
    assert_eq!(
        imports.get(id).await.unwrap().unwrap().status,
        ImportStatus::Done
    );
    let ledger = ImportResourceRepo::<Db>::new(pool)
        .list(id, None)
        .await
        .unwrap();
    assert_eq!(ledger[0].change, ImportChange::Deleted, "read back");
});

on_both!(a_stop_request_reaches_an_undo, |pool| {
    let imports = ImportRepo::<Db>::new(pool);
    let id = draft!(pool, "a");
    imports.start_run(id, 0).await.unwrap();
    imports.finish(id, ImportEnding::Done).await.unwrap();
    imports.start_undo(id).await.unwrap();
    assert!(imports.request_stop(id).await.unwrap());
    assert!(imports.stop_requested(id).await.unwrap());
    imports.finish(id, ImportEnding::UndoFailed).await.unwrap();
    imports.start_undo(id).await.unwrap();
    assert!(
        !imports.stop_requested(id).await.unwrap(),
        "a new undo starts without the old request"
    );
});
