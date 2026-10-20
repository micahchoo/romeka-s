//! `romeka_import` and `romeka_import_resource`: the draft lifecycle, the single open undo
//! window, recording inside the caller's transaction, and the cascade from an import.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use omeka_db::ids::{ImportId, ItemId, ItemSetId, JobId, PropertyId, TemplateId};
use omeka_db::repos::{
    DIED, HEARTBEAT, ImportRepo, ImportResourceRepo, JobRepo, StartOutcome, UserRepo,
};
use omeka_db::rows::{ImportKind, ImportStatus, ImportTarget, NewImport, NewJob, NewUser};
use serde_json::json;
use sqlx::SqlitePool;

async fn draft(pool: &SqlitePool, folder: &str) -> ImportId {
    ImportRepo::new(pool)
        .create_draft(&NewImport {
            owner_id: None,
            folder: folder.to_string(),
            plan: json!({"pattern": null}),
        })
        .await
        .unwrap()
        .id
}

async fn open_windows(pool: &SqlitePool) -> Vec<i64> {
    sqlx::query_scalar("SELECT id FROM romeka_import WHERE undoable = 1 ORDER BY id")
        .fetch_all(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn a_fresh_database_has_both_import_tables() {
    let pool = common::memory().await;
    let tables: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM sqlite_master WHERE type = 'table' AND name LIKE 'romeka_import%'
         ORDER BY name",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(tables, ["romeka_import", "romeka_import_resource"]);
    common::assert_primary_key(
        &pool,
        "romeka_import_resource",
        &["import_id", "kind", "target_id"],
    )
    .await;
    common::assert_foreign_key(
        &pool,
        "romeka_import_resource",
        "import_id",
        "romeka_import",
        "id",
        "CASCADE",
    )
    .await;
}

#[tokio::test]
async fn a_draft_round_trips_and_lists_newest_first() {
    let pool = common::memory().await;
    let imports = ImportRepo::new(&pool);
    let first = draft(&pool, "letters/1920").await;
    let second = draft(&pool, "photos").await;

    let row = imports.get(first).await.unwrap().unwrap();
    assert_eq!(row.folder, "letters/1920");
    assert_eq!(row.plan, json!({"pattern": null}));
    assert_eq!(row.status, ImportStatus::Draft);
    assert!(!row.undoable);
    assert_eq!(row.summary, None);
    assert_eq!(row.finished, None);

    let ids: Vec<_> = imports
        .list()
        .await
        .unwrap()
        .into_iter()
        .map(|row| row.id)
        .collect();
    assert_eq!(ids, [second, first]);
    assert_eq!(imports.get(ImportId(999)).await.unwrap(), None);
}

#[tokio::test]
async fn the_plan_changes_only_while_the_import_is_a_draft() {
    let pool = common::memory().await;
    let imports = ImportRepo::new(&pool);
    let id = draft(&pool, "a").await;

    assert!(
        imports
            .update_plan(id, &json!({"pattern": "(?P<identifier>.+)"}))
            .await
            .unwrap()
    );
    assert_eq!(
        imports.get(id).await.unwrap().unwrap().plan,
        json!({"pattern": "(?P<identifier>.+)"})
    );

    imports.set_status(id, ImportStatus::Running).await.unwrap();
    assert!(!imports.update_plan(id, &json!({})).await.unwrap());
    assert_eq!(
        imports.get(id).await.unwrap().unwrap().plan,
        json!({"pattern": "(?P<identifier>.+)"})
    );
    assert!(!imports.delete_draft(id).await.unwrap());
}

#[tokio::test]
async fn a_run_records_its_job_summary_and_finish() {
    let pool = common::memory().await;
    let imports = ImportRepo::new(&pool);
    let id = draft(&pool, "a").await;
    let job = JobRepo::new(&pool)
        .create(&NewJob {
            owner_id: None,
            pid: None,
            status: Some("starting".to_string()),
            class: "Romeka\\Job\\ImportArchive".to_string(),
            args: Some(json!({"importId": id.0})),
            log: None,
            started: None,
        })
        .await
        .unwrap();

    imports.set_job(id, job.id).await.unwrap();
    imports
        .set_summary(
            id,
            &json!({"items": 3, "progress": {"done": 1, "total": 3}}),
        )
        .await
        .unwrap();
    imports.finish(id, ImportStatus::Failed).await.unwrap();

    let row = imports.get(id).await.unwrap().unwrap();
    assert_eq!(row.job_id, Some(job.id));
    assert_eq!(row.summary.unwrap()["progress"]["done"], 1);
    assert_eq!(row.status, ImportStatus::Failed);
    assert!(row.finished.is_some());
}

#[tokio::test]
async fn deleting_the_owner_keeps_the_import() {
    let pool = common::memory().await;
    let owner = UserRepo::new(&pool)
        .create(&NewUser {
            email: "ada@example.org".to_string(),
            name: "Ada".to_string(),
            password_hash: None,
            role: "global_admin".to_string(),
            is_active: true,
            created: None,
        })
        .await
        .unwrap()
        .id;
    let imports = ImportRepo::new(&pool);
    let id = imports
        .create_draft(&NewImport {
            owner_id: Some(owner),
            folder: "a".to_string(),
            plan: json!({}),
        })
        .await
        .unwrap()
        .id;
    sqlx::query("DELETE FROM user WHERE id = ?")
        .bind(owner.0 as i64)
        .execute(&pool)
        .await
        .unwrap();
    let row = imports.get(id).await.unwrap().unwrap();
    assert_eq!(row.owner_id, None);
}

/// Mark `id` done, as its job does when the run ends.
async fn ended(imports: &ImportRepo<'_>, id: ImportId) {
    assert!(imports.finish(id, ImportStatus::Done).await.unwrap());
}

/// A job row in `status` for `id`, started `started` (SQLite `datetime` modifiers).
async fn job_for(pool: &SqlitePool, id: ImportId, status: &str, started: &str) -> JobId {
    let job = JobRepo::new(pool)
        .create(&NewJob {
            owner_id: None,
            pid: None,
            status: Some(status.to_string()),
            class: "Romeka\\Job\\ImportArchive".to_string(),
            args: Some(json!({"importId": id.0})),
            log: None,
            started: None,
        })
        .await
        .unwrap()
        .id;
    sqlx::query("UPDATE job SET started = datetime('now', ?) WHERE id = ?")
        .bind(started)
        .bind(job.0 as i64)
        .execute(pool)
        .await
        .unwrap();
    assert!(ImportRepo::new(pool).set_job(id, job).await.unwrap());
    job
}

async fn beat_at(pool: &SqlitePool, id: ImportId, modifier: &str) {
    sqlx::query(
        "UPDATE romeka_import SET summary = json_set(summary, '$.heartbeat', datetime('now', ?))
         WHERE id = ?",
    )
    .bind(modifier)
    .bind(id.0 as i64)
    .execute(pool)
    .await
    .unwrap();
}

#[tokio::test]
async fn start_run_moves_a_draft_and_the_one_open_window() {
    let pool = common::memory().await;
    let imports = ImportRepo::new(&pool);
    let first = draft(&pool, "a").await;
    let second = draft(&pool, "b").await;

    assert_eq!(
        imports.start_run(first).await.unwrap(),
        StartOutcome::Started
    );
    let row = imports.get(first).await.unwrap().unwrap();
    assert_eq!((row.status, row.undoable), (ImportStatus::Running, true));
    assert!(
        row.summary.unwrap()[HEARTBEAT].is_string(),
        "the start is the first heartbeat"
    );
    assert_eq!(open_windows(&pool).await, [first.0 as i64]);

    assert_eq!(
        imports.start_run(second).await.unwrap(),
        StartOutcome::Busy(first)
    );
    assert_eq!(
        imports.start_run(first).await.unwrap(),
        StartOutcome::Refused {
            status: ImportStatus::Running,
            undoable: true
        }
    );
    ended(&imports, first).await;
    assert_eq!(
        imports.start_run(second).await.unwrap(),
        StartOutcome::Started
    );
    assert_eq!(open_windows(&pool).await, [second.0 as i64]);
    assert_eq!(imports.undoable().await.unwrap().unwrap().id, second);
    assert_eq!(
        imports.start_run(ImportId(999)).await.unwrap(),
        StartOutcome::Missing
    );

    assert!(imports.close_undo_window(second).await.unwrap());
    assert_eq!(imports.undoable().await.unwrap(), None);
}

#[tokio::test]
async fn start_undo_needs_the_window_an_ended_run_and_no_live_import() {
    let pool = common::memory().await;
    let imports = ImportRepo::new(&pool);
    let first = draft(&pool, "a").await;
    let second = draft(&pool, "b").await;
    assert_eq!(
        imports.start_undo(first).await.unwrap(),
        StartOutcome::Refused {
            status: ImportStatus::Draft,
            undoable: false
        }
    );
    imports.start_run(first).await.unwrap();
    assert_eq!(
        imports.start_undo(first).await.unwrap(),
        StartOutcome::Refused {
            status: ImportStatus::Running,
            undoable: true
        },
        "the run has not ended"
    );
    ended(&imports, first).await;
    imports.start_run(second).await.unwrap();
    assert_eq!(
        imports.start_undo(first).await.unwrap(),
        StartOutcome::Refused {
            status: ImportStatus::Done,
            undoable: false
        },
        "the next run took the window"
    );
    assert_eq!(
        imports.start_undo(second).await.unwrap(),
        StartOutcome::Refused {
            status: ImportStatus::Running,
            undoable: true
        }
    );
    imports.finish(second, ImportStatus::Failed).await.unwrap();
    assert_eq!(
        imports.start_undo(second).await.unwrap(),
        StartOutcome::Started
    );
    assert_eq!(
        imports.get(second).await.unwrap().unwrap().status,
        ImportStatus::Undoing
    );
}

#[tokio::test]
async fn a_run_whose_heartbeat_is_two_minutes_old_is_settled_as_failed_once() {
    let pool = common::memory().await;
    let imports = ImportRepo::new(&pool);
    let stale = draft(&pool, "a").await;
    let fresh = draft(&pool, "b").await;
    imports.start_run(stale).await.unwrap();
    let job = job_for(&pool, stale, "in_progress", "-1 hour").await;
    beat_at(&pool, stale, "-121 seconds").await;
    assert_eq!(imports.settle_dead().await.unwrap(), [stale]);
    assert_eq!(
        imports.settle_dead().await.unwrap(),
        [],
        "settling again does nothing"
    );

    let row = imports.get(stale).await.unwrap().unwrap();
    assert_eq!(row.status, ImportStatus::Failed);
    assert!(row.finished.is_some());
    assert!(row.summary.unwrap()[DIED].is_string());
    let job = JobRepo::new(&pool).get(job).await.unwrap().unwrap();
    assert_eq!(job.status.as_deref(), Some("error"));
    assert!(job.ended.is_some());
    assert!(
        job.log.unwrap().contains("no heartbeat since"),
        "the log says why"
    );

    assert_eq!(
        imports.start_run(fresh).await.unwrap(),
        StartOutcome::Started
    );
    let worker = job_for(&pool, fresh, "in_progress", "-1 hour").await;
    beat_at(&pool, fresh, "-60 seconds").await;
    assert_eq!(imports.settle_dead().await.unwrap(), []);
    assert!(imports.heartbeat(fresh, worker).await.unwrap());
    assert!(
        !imports.heartbeat(fresh, job.id).await.unwrap(),
        "only the import's own job beats for it"
    );
    assert!(
        !imports.heartbeat(stale, job.id).await.unwrap(),
        "a dead import's worker learns it lost the claim"
    );
}

#[tokio::test]
async fn without_a_heartbeat_the_job_start_is_the_pulse_and_an_ended_job_is_dead() {
    let pool = common::memory().await;
    let imports = ImportRepo::new(&pool);
    let old = draft(&pool, "a").await;
    imports
        .set_status(old, ImportStatus::Undoing)
        .await
        .unwrap();
    job_for(&pool, old, "in_progress", "-3 minutes").await;
    let ended_job = draft(&pool, "b").await;
    imports
        .set_status(ended_job, ImportStatus::Running)
        .await
        .unwrap();
    job_for(&pool, ended_job, "error", "-1 seconds").await;
    let young = draft(&pool, "c").await;
    imports
        .set_status(young, ImportStatus::Running)
        .await
        .unwrap();
    job_for(&pool, young, "starting", "-10 seconds").await;

    assert_eq!(imports.settle_dead().await.unwrap(), [old, ended_job]);
    assert_eq!(
        imports.get(young).await.unwrap().unwrap().status,
        ImportStatus::Running
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn two_concurrent_starts_never_both_win() {
    let (pool, path) = common::file("concurrent-starts").await;
    for round in 0..20 {
        let first = draft(&pool, &format!("a{round}")).await;
        let second = draft(&pool, &format!("b{round}")).await;
        let (left, right) = tokio::join!(
            tokio::spawn({
                let pool = pool.clone();
                async move { ImportRepo::new(&pool).start_run(first).await.unwrap() }
            }),
            tokio::spawn({
                let pool = pool.clone();
                async move { ImportRepo::new(&pool).start_run(second).await.unwrap() }
            }),
        );
        let mut outcomes = [left.unwrap(), right.unwrap()];
        outcomes.sort_by_key(|outcome| matches!(outcome, StartOutcome::Busy(_)));
        assert_eq!(outcomes[0], StartOutcome::Started, "round {round}");
        assert!(
            matches!(outcomes[1], StartOutcome::Busy(_)),
            "round {round}: {outcomes:?}"
        );
        let running: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM romeka_import WHERE status = 'running'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(running, 1, "round {round}");
        sqlx::query("UPDATE romeka_import SET status = 'done' WHERE status = 'running'")
            .execute(&pool)
            .await
            .unwrap();
    }
    pool.close().await;
    common::remove_db(&path);
}

#[tokio::test]
async fn the_partial_unique_index_refuses_a_second_open_window() {
    let pool = common::memory().await;
    let first = draft(&pool, "a").await;
    let second = draft(&pool, "b").await;
    ImportRepo::new(&pool).start_run(first).await.unwrap();

    let error = sqlx::query("UPDATE romeka_import SET undoable = 1 WHERE id = ?")
        .bind(second.0 as i64)
        .execute(&pool)
        .await
        .unwrap_err();
    assert!(
        error
            .as_database_error()
            .is_some_and(|database| database.is_unique_violation()),
        "{error}"
    );
    assert_eq!(open_windows(&pool).await, [first.0 as i64]);

    sqlx::query("UPDATE romeka_import SET undoable = 0 WHERE id = ?")
        .bind(second.0 as i64)
        .execute(&pool)
        .await
        .expect("any number of closed windows is allowed");
}

#[tokio::test]
async fn a_status_outside_the_list_is_refused_by_the_table() {
    let pool = common::memory().await;
    let id = draft(&pool, "a").await;
    let outcome = sqlx::query("UPDATE romeka_import SET status = 'paused' WHERE id = ?")
        .bind(id.0 as i64)
        .execute(&pool)
        .await;
    assert!(outcome.is_err());
}

#[tokio::test]
async fn recording_inside_a_rolled_back_transaction_leaves_no_row() {
    let pool = common::memory().await;
    let import = draft(&pool, "a").await;
    let resources = ImportResourceRepo::new(&pool);

    let mut tx = pool.begin().await.unwrap();
    ImportResourceRepo::record(&mut *tx, import, ImportTarget::Item(ItemId(7)))
        .await
        .unwrap();
    tx.rollback().await.unwrap();
    assert_eq!(resources.list(import, None).await.unwrap(), []);

    let mut tx = pool.begin().await.unwrap();
    ImportResourceRepo::record(&mut *tx, import, ImportTarget::Item(ItemId(8)))
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let rows = resources.list(import, None).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].target, ImportTarget::Item(ItemId(8)));
}

#[tokio::test]
async fn resources_list_by_import_and_kind_and_delete_one_at_a_time() {
    let pool = common::memory().await;
    let one = draft(&pool, "a").await;
    let other = draft(&pool, "b").await;
    let resources = ImportResourceRepo::new(&pool);
    for target in [
        ImportTarget::Item(ItemId(2)),
        ImportTarget::Item(ItemId(1)),
        ImportTarget::ItemSet(ItemSetId(3)),
        ImportTarget::ResourceTemplate(TemplateId(4)),
        ImportTarget::Property(PropertyId(5)),
    ] {
        ImportResourceRepo::record(&pool, one, target)
            .await
            .unwrap();
    }
    ImportResourceRepo::record(&pool, other, ImportTarget::Item(ItemId(9)))
        .await
        .unwrap();

    let items: Vec<_> = resources
        .list(one, Some(ImportKind::Item))
        .await
        .unwrap()
        .into_iter()
        .map(|row| row.target)
        .collect();
    assert_eq!(
        items,
        [ImportTarget::Item(ItemId(1)), ImportTarget::Item(ItemId(2))]
    );
    assert_eq!(resources.list(one, None).await.unwrap().len(), 5);
    assert_eq!(resources.list(other, None).await.unwrap().len(), 1);

    assert!(
        resources
            .delete(one, ImportTarget::Item(ItemId(1)))
            .await
            .unwrap()
    );
    assert!(
        !resources
            .delete(one, ImportTarget::Item(ItemId(1)))
            .await
            .unwrap()
    );
    assert_eq!(
        resources
            .list(one, Some(ImportKind::Item))
            .await
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn recording_the_same_target_twice_is_refused() {
    let pool = common::memory().await;
    let import = draft(&pool, "a").await;
    let target = ImportTarget::Item(ItemId(1));
    ImportResourceRepo::record(&pool, import, target)
        .await
        .unwrap();
    let error = ImportResourceRepo::record(&pool, import, target)
        .await
        .unwrap_err();
    assert!(error.is_unique_violation());
}

#[tokio::test]
async fn deleting_an_import_deletes_what_it_recorded() {
    let pool = common::memory().await;
    let doomed = draft(&pool, "a").await;
    let kept = draft(&pool, "b").await;
    ImportResourceRepo::record(&pool, doomed, ImportTarget::Item(ItemId(1)))
        .await
        .unwrap();
    ImportResourceRepo::record(&pool, kept, ImportTarget::Item(ItemId(2)))
        .await
        .unwrap();

    assert!(ImportRepo::new(&pool).delete_draft(doomed).await.unwrap());

    let resources = ImportResourceRepo::new(&pool);
    assert_eq!(resources.list(doomed, None).await.unwrap(), []);
    assert_eq!(resources.list(kept, None).await.unwrap().len(), 1);
}
