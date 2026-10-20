//! `romeka_import` (migration 0006): the draft, plan saves by revision, the frozen plan, the run
//! and undo starts with one live import install-wide, the endings, and how a dead worker is
//! recorded. Every test runs on SQLite and, with the `mysql` feature, on MySQL.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;
mod import_db;

use import_db::{draft, exec_id, new_import, on_both};
use omeka_db::ids::{ImportId, JobId};
use omeka_db::repos::{DIED, HEARTBEAT, ImportRepo, JobRepo, Paging, PlanSave, StartOutcome};
use omeka_db::rows::{ImportEnding, ImportStatus, NewImport, NewJob};
use serde_json::json;

fn job(status: &str) -> NewJob {
    NewJob {
        owner_id: None,
        pid: None,
        status: Some(status.to_string()),
        class: "Romeka\\Job\\Import".to_string(),
        args: None,
        log: None,
        started: None,
    }
}

const AGE_HEARTBEAT: &str =
    "UPDATE romeka_import SET summary = json_set(summary, '$.heartbeat', '2000-01-01 00:00:00')
     WHERE id = ?";

on_both!(a_draft_reads_back_as_written_with_revision_zero, |pool| {
    let imports = ImportRepo::<Db>::new(pool);
    let created = imports.create_draft(&new_import("a")).await.unwrap();
    let read = imports.get(created.id).await.unwrap().unwrap();
    assert_eq!(read, created);
    assert_eq!(read.status, ImportStatus::Draft);
    assert_eq!(read.revision, 0);
    assert_eq!(read.ran_plan, None);
    assert_eq!(read.sources[0]["path"], "a");
    assert_eq!(imports.get(ImportId(999)).await.unwrap(), None);
});

on_both!(
    the_history_pages_newest_first_and_counts_every_import,
    |pool| {
        let imports = ImportRepo::<Db>::new(pool);
        let first = draft!(pool, "a");
        let second = draft!(pool, "b");
        let third = draft!(pool, "c");
        let ids = |rows: Vec<omeka_db::rows::ImportRow>| -> Vec<ImportId> {
            rows.into_iter().map(|row| row.id).collect()
        };
        assert_eq!(
            ids(imports.page(&Paging::new(2, 0)).await.unwrap()),
            [third, second]
        );
        assert_eq!(
            ids(imports.page(&Paging::new(2, 2)).await.unwrap()),
            [first]
        );
        assert_eq!(imports.count().await.unwrap(), 3);
    }
);

on_both!(
    a_copy_keeps_its_source_until_the_source_is_deleted,
    |pool| {
        let imports = ImportRepo::<Db>::new(pool);
        let source = draft!(pool, "a");
        let copy = imports
            .create_draft(&NewImport {
                copied_from: Some(source),
                ..new_import("b")
            })
            .await
            .unwrap();
        assert_eq!(copy.copied_from, Some(source));
        assert!(imports.delete_draft(source).await.unwrap());
        assert_eq!(
            imports.get(copy.id).await.unwrap().unwrap().copied_from,
            None
        );
    }
);

on_both!(
    a_plan_save_with_an_old_revision_is_stale_and_changes_nothing,
    |pool| {
        let imports = ImportRepo::<Db>::new(pool);
        let id = draft!(pool, "a");
        let edited = json!({ "version": 2, "tag": "edited" });
        assert_eq!(
            imports.save_plan(id, &edited, 0).await.unwrap(),
            PlanSave::Saved(1)
        );
        assert_eq!(
            imports
                .save_plan(id, &json!({ "version": 2, "tag": "late" }), 0)
                .await
                .unwrap(),
            PlanSave::Stale(1)
        );
        let row = imports.get(id).await.unwrap().unwrap();
        assert_eq!((row.plan, row.revision), (edited, 1));
        assert_eq!(
            imports
                .save_plan(ImportId(999), &json!({}), 0)
                .await
                .unwrap(),
            PlanSave::Missing
        );
    }
);

on_both!(only_a_draft_saves_its_plan_or_is_deleted, |pool| {
    let imports = ImportRepo::<Db>::new(pool);
    let id = draft!(pool, "a");
    assert_eq!(
        imports.start_run(id, 0).await.unwrap(),
        StartOutcome::Started
    );
    assert_eq!(
        imports.save_plan(id, &json!({}), 0).await.unwrap(),
        PlanSave::NotDraft(ImportStatus::Running)
    );
    assert!(!imports.delete_draft(id).await.unwrap());
    assert!(imports.get(id).await.unwrap().is_some());
    assert!(!imports.delete_draft(ImportId(999)).await.unwrap());
});

on_both!(
    a_run_freezes_the_plan_and_a_retry_runs_the_frozen_plan,
    |pool| {
        let imports = ImportRepo::<Db>::new(pool);
        let id = draft!(pool, "a");
        let plan = imports.get(id).await.unwrap().unwrap().plan;
        assert_eq!(
            imports.start_run(id, 0).await.unwrap(),
            StartOutcome::Started
        );
        let running = imports.get(id).await.unwrap().unwrap();
        assert_eq!(running.status, ImportStatus::Running);
        assert_eq!(running.ran_plan.as_ref(), Some(&plan));
        assert!(running.summary.unwrap()[HEARTBEAT].is_string());

        exec_id!(
            pool,
            "UPDATE romeka_import SET plan = '{\"tag\":\"drifted\"}' WHERE id = ?",
            id.0
        );
        assert!(imports.finish(id, ImportEnding::RunFailed).await.unwrap());
        assert_eq!(
            imports.start_run(id, 0).await.unwrap(),
            StartOutcome::Started
        );
        let retried = imports.get(id).await.unwrap().unwrap();
        assert_eq!(
            retried.ran_plan,
            Some(plan),
            "a retry keeps the frozen plan"
        );
        assert_eq!(retried.finished, None, "a new attempt has not finished");

        assert!(imports.finish(id, ImportEnding::Done).await.unwrap());
        assert_eq!(
            imports.start_run(id, 0).await.unwrap(),
            StartOutcome::Refused(ImportStatus::Done)
        );
        assert_eq!(
            imports.start_run(ImportId(999), 0).await.unwrap(),
            StartOutcome::Missing
        );
    }
);

on_both!(
    an_undo_starts_from_done_run_failed_or_undo_failed_only,
    |pool| {
        let imports = ImportRepo::<Db>::new(pool);
        let id = draft!(pool, "a");
        assert_eq!(
            imports.start_undo(id).await.unwrap(),
            StartOutcome::Refused(ImportStatus::Draft)
        );
        imports.start_run(id, 0).await.unwrap();
        assert_eq!(
            imports.start_undo(id).await.unwrap(),
            StartOutcome::Refused(ImportStatus::Running)
        );
        for ending in [ImportEnding::Done, ImportEnding::RunFailed] {
            exec_id!(
                pool,
                "UPDATE romeka_import SET status = 'running' WHERE id = ?",
                id.0
            );
            assert!(imports.finish(id, ending).await.unwrap());
            assert_eq!(imports.start_undo(id).await.unwrap(), StartOutcome::Started);
            assert_eq!(
                imports.get(id).await.unwrap().unwrap().status,
                ImportStatus::Undoing
            );
            assert!(imports.finish(id, ImportEnding::UndoFailed).await.unwrap());
            assert_eq!(imports.start_undo(id).await.unwrap(), StartOutcome::Started);
            assert!(imports.finish(id, ImportEnding::Undone).await.unwrap());
            assert_eq!(
                imports.start_undo(id).await.unwrap(),
                StartOutcome::Refused(ImportStatus::Undone)
            );
        }
    }
);

on_both!(an_ending_needs_the_state_it_ends, |pool| {
    let imports = ImportRepo::<Db>::new(pool);
    let id = draft!(pool, "a");
    assert!(!imports.finish(id, ImportEnding::Done).await.unwrap());
    imports.start_run(id, 0).await.unwrap();
    assert!(!imports.finish(id, ImportEnding::Undone).await.unwrap());
    assert!(imports.finish(id, ImportEnding::Done).await.unwrap());
    let done = imports.get(id).await.unwrap().unwrap();
    assert_eq!(done.status, ImportStatus::Done);
    assert!(done.finished.is_some());
    assert!(!imports.finish(id, ImportEnding::RunFailed).await.unwrap());
});

on_both!(one_import_runs_or_undoes_at_a_time, |pool| {
    let imports = ImportRepo::<Db>::new(pool);
    let first = draft!(pool, "a");
    let second = draft!(pool, "b");
    imports.start_run(first, 0).await.unwrap();
    assert_eq!(
        imports.start_run(second, 0).await.unwrap(),
        StartOutcome::Busy(first)
    );
    imports.finish(first, ImportEnding::Done).await.unwrap();
    imports.start_undo(first).await.unwrap();
    assert_eq!(
        imports.start_run(second, 0).await.unwrap(),
        StartOutcome::Busy(first)
    );
    imports.finish(first, ImportEnding::Undone).await.unwrap();
    assert_eq!(
        imports.start_run(second, 0).await.unwrap(),
        StartOutcome::Started
    );
});

on_both!(two_concurrent_starts_never_both_win, |pool| {
    for round in 0..20 {
        let first = draft!(pool, &format!("a{round}"));
        let second = draft!(pool, &format!("b{round}"));
        let start = |id: ImportId| {
            let pool = pool.clone();
            tokio::spawn(
                async move { ImportRepo::<Db>::new(&pool).start_run(id, 0).await.unwrap() },
            )
        };
        let (left, right) = tokio::join!(start(first), start(second));
        let mut outcomes = [left.unwrap(), right.unwrap()];
        outcomes.sort_by_key(|outcome| matches!(outcome, StartOutcome::Busy(_)));
        assert_eq!(outcomes[0], StartOutcome::Started, "round {round}");
        assert!(
            matches!(outcomes[1], StartOutcome::Busy(_)),
            "round {round}: {outcomes:?}"
        );
        let running: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM romeka_import WHERE status = 'running'")
                .fetch_one(pool)
                .await
                .unwrap();
        assert_eq!(running, 1, "round {round}");
        sqlx::query("UPDATE romeka_import SET status = 'done' WHERE status = 'running'")
            .execute(pool)
            .await
            .unwrap();
    }
});

on_both!(a_summary_write_is_also_a_heartbeat, |pool| {
    let imports = ImportRepo::<Db>::new(pool);
    let id = draft!(pool, "a");
    imports.start_run(id, 0).await.unwrap();
    assert!(
        imports
            .set_summary(id, &json!({ "items": 2 }))
            .await
            .unwrap()
    );
    let summary = imports.get(id).await.unwrap().unwrap().summary.unwrap();
    assert_eq!(summary["items"], 2);
    assert!(summary[HEARTBEAT].is_string());
    assert!(
        !imports
            .set_summary(ImportId(999), &json!({}))
            .await
            .unwrap()
    );
});

on_both!(
    a_heartbeat_lands_only_for_the_import_s_own_live_job,
    |pool| {
        let imports = ImportRepo::<Db>::new(pool);
        let jobs = JobRepo::<Db>::new(pool);
        let id = draft!(pool, "a");
        imports.start_run(id, 0).await.unwrap();
        let worker = jobs.create(&job("in_progress")).await.unwrap().id;
        let other = jobs.create(&job("in_progress")).await.unwrap().id;
        assert!(imports.set_job(id, worker).await.unwrap());
        assert_eq!(imports.get(id).await.unwrap().unwrap().job_id, Some(worker));
        exec_id!(pool, AGE_HEARTBEAT, id.0);
        assert!(imports.heartbeat(id, worker).await.unwrap());
        assert!(!imports.heartbeat(id, other).await.unwrap());
        let beat = imports.get(id).await.unwrap().unwrap().summary.unwrap()[HEARTBEAT].clone();
        assert_ne!(beat, json!("2000-01-01 00:00:00"));
        imports.finish(id, ImportEnding::Done).await.unwrap();
        assert!(
            !imports.heartbeat(id, worker).await.unwrap(),
            "an ended import has no worker"
        );
        assert!(!imports.set_job(ImportId(999), worker).await.unwrap());
    }
);

on_both!(
    a_dead_worker_is_recorded_failed_once_and_its_job_ended,
    |pool| {
        let imports = ImportRepo::<Db>::new(pool);
        let jobs = JobRepo::<Db>::new(pool);
        let claim = |status: &'static str, id: ImportId, job: JobId| {
            let imports = &imports;
            async move {
                exec_id!(
                    pool,
                    &format!("UPDATE romeka_import SET status = '{status}' WHERE id = ?"),
                    id.0
                );
                imports.set_job(id, job).await.unwrap();
                imports.set_summary(id, &json!({})).await.unwrap();
            }
        };
        let silent = draft!(pool, "silent");
        let silent_job = jobs.create(&job("in_progress")).await.unwrap().id;
        claim("running", silent, silent_job).await;
        exec_id!(pool, AGE_HEARTBEAT, silent.0);

        let ended = draft!(pool, "ended");
        let ended_job = jobs.create(&job("in_progress")).await.unwrap().id;
        claim("undoing", ended, ended_job).await;
        jobs.finish(ended_job, "error").await.unwrap();

        let alive = draft!(pool, "alive");
        let alive_job = jobs.create(&job("starting")).await.unwrap().id;
        claim("running", alive, alive_job).await;

        let mut settled = imports.settle_dead().await.unwrap();
        settled.sort();
        assert_eq!(settled, [silent, ended]);
        assert_eq!(imports.settle_dead().await.unwrap(), Vec::<ImportId>::new());

        let failed = imports.get(silent).await.unwrap().unwrap();
        assert_eq!(failed.status, ImportStatus::RunFailed);
        assert!(failed.finished.is_some());
        assert!(failed.summary.unwrap()[DIED].is_string());
        let silent_row = jobs.get(silent_job).await.unwrap().unwrap();
        assert_eq!(silent_row.status.as_deref(), Some("error"));
        assert!(
            silent_row
                .log
                .unwrap()
                .contains("no heartbeat since 2000-01-01 00:00:00")
        );
        assert_eq!(
            imports.get(ended).await.unwrap().unwrap().status,
            ImportStatus::UndoFailed
        );
        assert_eq!(
            imports.get(alive).await.unwrap().unwrap().status,
            ImportStatus::Running
        );
    }
);

on_both!(a_start_settles_the_dead_import_that_blocked_it, |pool| {
    let imports = ImportRepo::<Db>::new(pool);
    let dead = draft!(pool, "dead");
    let next = draft!(pool, "next");
    imports.start_run(dead, 0).await.unwrap();
    exec_id!(pool, AGE_HEARTBEAT, dead.0);
    assert_eq!(
        imports.start_run(next, 0).await.unwrap(),
        StartOutcome::Started
    );
    assert_eq!(
        imports.get(dead).await.unwrap().unwrap().status,
        ImportStatus::RunFailed
    );
    imports.finish(next, ImportEnding::Done).await.unwrap();
    assert_eq!(
        imports.start_run(dead, 0).await.unwrap(),
        StartOutcome::Started
    );
    let retried = imports.get(dead).await.unwrap().unwrap().summary.unwrap();
    assert!(retried.get(DIED).is_none(), "a new attempt is not dead");
});

on_both!(the_table_refuses_a_status_outside_the_v2_list, |pool| {
    let id = draft!(pool, "a");
    for status in ["failed", "paused"] {
        let outcome = sqlx::query("UPDATE romeka_import SET status = ? WHERE id = ?")
            .bind(status)
            .bind(id.0 as i64)
            .execute(pool)
            .await;
        assert!(outcome.is_err(), "{status} was stored");
    }
});

on_both!(a_run_start_names_the_revision_the_caller_checked, |pool| {
    let imports = ImportRepo::<Db>::new(pool);
    let id = draft!(pool, "a");
    assert_eq!(
        imports
            .save_plan(id, &json!({ "version": 2 }), 0)
            .await
            .unwrap(),
        PlanSave::Saved(1)
    );
    assert_eq!(
        imports.start_run(id, 0).await.unwrap(),
        StartOutcome::Changed { revision: 1 },
        "a save between Check and Run"
    );
    assert_eq!(
        imports.get(id).await.unwrap().unwrap().status,
        ImportStatus::Draft,
        "a refused start changes nothing"
    );
    assert_eq!(
        imports.start_run(id, 1).await.unwrap(),
        StartOutcome::Started
    );
});

on_both!(
    a_done_import_runs_again_only_when_its_summary_lists_failures,
    |pool| {
        let imports = ImportRepo::<Db>::new(pool);
        let id = draft!(pool, "a");
        imports.start_run(id, 0).await.unwrap();
        imports
            .set_summary(id, &json!({ "items": 2, "failures": [] }))
            .await
            .unwrap();
        assert!(imports.finish(id, ImportEnding::Done).await.unwrap());
        assert_eq!(
            imports.start_run(id, 0).await.unwrap(),
            StartOutcome::Refused(ImportStatus::Done),
            "done with an empty failure list"
        );

        imports
            .set_summary(
                id,
                &json!({ "failures": [{ "record": "A", "key": "k", "detail": "d" }] }),
            )
            .await
            .unwrap();
        assert_eq!(
            imports.start_run(id, 0).await.unwrap(),
            StartOutcome::Started
        );
        let retried = imports.get(id).await.unwrap().unwrap();
        assert_eq!(retried.status, ImportStatus::Running);
        assert_eq!(retried.finished, None);

        let bare = draft!(pool, "bare");
        imports.finish(id, ImportEnding::Done).await.unwrap();
        imports.start_run(bare, 0).await.unwrap();
        imports.finish(bare, ImportEnding::Done).await.unwrap();
        assert_eq!(
            imports.start_run(bare, 0).await.unwrap(),
            StartOutcome::Refused(ImportStatus::Done),
            "done with no failures member"
        );
    }
);

on_both!(
    a_stop_request_keeps_the_heartbeat_and_survives_summary_writes,
    |pool| {
        let imports = ImportRepo::<Db>::new(pool);
        let id = draft!(pool, "a");
        assert!(
            !imports.request_stop(id).await.unwrap(),
            "a draft does not run"
        );
        imports.start_run(id, 0).await.unwrap();
        assert!(!imports.stop_requested(id).await.unwrap());
        exec_id!(pool, AGE_HEARTBEAT, id.0);

        assert!(imports.request_stop(id).await.unwrap());
        assert!(imports.stop_requested(id).await.unwrap());
        let row = imports.get(id).await.unwrap().unwrap();
        assert!(row.stop_requested);
        assert_eq!(
            row.summary.unwrap()[HEARTBEAT],
            json!("2000-01-01 00:00:00"),
            "a stop request is not evidence of life"
        );

        imports
            .set_summary(id, &json!({ "items": 3 }))
            .await
            .unwrap();
        assert!(
            imports.stop_requested(id).await.unwrap(),
            "the worker's whole-summary write keeps the request"
        );
        assert!(!imports.stop_requested(ImportId(999)).await.unwrap());
        assert!(!imports.request_stop(ImportId(999)).await.unwrap());

        imports.finish(id, ImportEnding::RunFailed).await.unwrap();
        assert!(
            !imports.request_stop(id).await.unwrap(),
            "it no longer runs"
        );
        assert_eq!(
            imports.start_run(id, 0).await.unwrap(),
            StartOutcome::Started
        );
        assert!(
            !imports.stop_requested(id).await.unwrap(),
            "a new attempt starts without the old request"
        );
    }
);
