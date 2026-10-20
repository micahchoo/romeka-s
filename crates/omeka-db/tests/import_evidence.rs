//! What an import holds before it writes (migration 0006): its uploads (`romeka_import_upload`)
//! and its evidence index (`romeka_import_entry`). Every test runs on SQLite and, with the `mysql`
//! feature, on MySQL.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;
mod import_db;

use chrono::{NaiveDate, NaiveDateTime, TimeDelta, Utc};
use import_db::{draft, on_both};
use omeka_db::repos::{EntryRepo, ImportRepo, Paging, UploadAdvance, UploadRepo};
use omeka_db::rows::{NewEntry, NewScan, NewUpload};
use serde_json::json;

fn upload(import: omeka_db::ids::ImportId, id: &str, path: &str, length: u64) -> NewUpload {
    NewUpload {
        import_id: import,
        id: id.to_string(),
        path: path.to_string(),
        length,
    }
}

fn file(source: &str, path: &str, size: u64) -> NewEntry {
    NewEntry {
        source: source.to_string(),
        path: path.to_string(),
        size,
        modified: NaiveDate::from_ymd_opt(2026, 10, 1)
            .and_then(|day| day.and_hms_opt(12, 30, 5))
            .unwrap(),
        media_type: Some("image/jpeg".to_string()),
        flags: json!([]),
    }
}

fn now() -> NaiveDateTime {
    Utc::now().naive_utc()
}

on_both!(an_upload_advances_only_from_the_stored_offset, |pool| {
    let id = draft!(pool, "a");
    let uploads = UploadRepo::<Db>::new(pool);
    let created = uploads
        .create(&upload(id, "u1", "Box/L_1.jpg", 10))
        .await
        .unwrap();
    assert_eq!(created.received, 0);
    assert_eq!(uploads.get(id, "u1").await.unwrap(), Some(created.clone()));

    assert_eq!(
        uploads.advance(id, "u1", 0, 4).await.unwrap(),
        UploadAdvance::Advanced(4)
    );
    assert_eq!(
        uploads.advance(id, "u1", 0, 4).await.unwrap(),
        UploadAdvance::Conflict { received: 4 },
        "a PATCH from an old offset answers 409"
    );
    assert_eq!(
        uploads.advance(id, "u1", 4, 7).await.unwrap(),
        UploadAdvance::PastLength { length: 10 }
    );
    assert_eq!(
        uploads.advance(id, "u1", 4, 6).await.unwrap(),
        UploadAdvance::Advanced(10)
    );
    assert_eq!(
        uploads.advance(id, "nope", 0, 1).await.unwrap(),
        UploadAdvance::Missing
    );
    assert_eq!(uploads.get(id, "u1").await.unwrap().unwrap().received, 10);
});

on_both!(uploads_list_per_import_and_delete_one, |pool| {
    let id = draft!(pool, "a");
    let other = draft!(pool, "b");
    let uploads = UploadRepo::<Db>::new(pool);
    uploads.create(&upload(id, "u1", "A.jpg", 1)).await.unwrap();
    uploads.create(&upload(id, "u2", "a.jpg", 1)).await.unwrap();
    uploads
        .create(&upload(other, "u1", "B.jpg", 1))
        .await
        .unwrap();
    assert!(
        uploads.create(&upload(id, "u1", "C.jpg", 1)).await.is_err(),
        "an upload id is unique within its import"
    );
    let paths: Vec<String> = uploads
        .list(id)
        .await
        .unwrap()
        .into_iter()
        .map(|row| row.path)
        .collect();
    assert_eq!(paths, ["A.jpg", "a.jpg"]);
    assert!(uploads.delete(id, "u1").await.unwrap());
    assert!(!uploads.delete(id, "u1").await.unwrap());
    assert_eq!(uploads.list(id).await.unwrap().len(), 1);
    assert_eq!(uploads.list(other).await.unwrap().len(), 1);
});

on_both!(unpacked_files_arrive_complete_in_one_batch, |pool| {
    let id = draft!(pool, "a");
    let other = draft!(pool, "b");
    let uploads = UploadRepo::<Db>::new(pool);
    uploads
        .insert_complete(&[
            upload(id, "z1", "Box/a.jpg", 3),
            upload(id, "z2", "Box/b.jpg", 0),
        ])
        .await
        .unwrap();
    let rows = uploads.list(id).await.unwrap();
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().all(|row| row.is_complete()));
    assert_eq!(rows[0].received, 3);
    assert!(
        uploads
            .insert_complete(&[upload(id, "z3", "c.jpg", 1), upload(id, "z1", "d.jpg", 1)])
            .await
            .is_err(),
        "a batch with a taken id fails"
    );
    assert_eq!(
        uploads.list(id).await.unwrap().len(),
        2,
        "and none of it is stored"
    );
    uploads
        .create(&upload(other, "u1", "B.jpg", 1))
        .await
        .unwrap();
    assert_eq!(uploads.imports().await.unwrap(), vec![id, other]);
});

on_both!(
    the_sweep_finds_incomplete_uploads_older_than_a_time,
    |pool| {
        let id = draft!(pool, "a");
        let uploads = UploadRepo::<Db>::new(pool);
        uploads
            .create(&upload(id, "done", "a.jpg", 2))
            .await
            .unwrap();
        uploads.advance(id, "done", 0, 2).await.unwrap();
        uploads
            .create(&upload(id, "half", "b.jpg", 2))
            .await
            .unwrap();
        uploads.advance(id, "half", 0, 1).await.unwrap();
        uploads
            .create(&upload(id, "empty", "c.jpg", 0))
            .await
            .unwrap();

        let later = now() + TimeDelta::hours(1);
        let ids: Vec<String> = uploads
            .incomplete_before(later)
            .await
            .unwrap()
            .into_iter()
            .map(|row| row.id)
            .collect();
        assert_eq!(ids, ["half"], "a zero-length upload is complete at once");
        let earlier = now() - TimeDelta::hours(1);
        assert_eq!(uploads.incomplete_before(earlier).await.unwrap(), []);
    }
);

on_both!(a_rescan_replaces_the_whole_index_in_one_go, |pool| {
    let id = draft!(pool, "a");
    let other = draft!(pool, "b");
    let entries = EntryRepo::<Db>::new(pool);
    entries
        .replace(id, &[], &[file("box", "old.jpg", 1)])
        .await
        .unwrap();
    entries
        .replace(other, &[], &[file("box", "kept.jpg", 1)])
        .await
        .unwrap();
    let scan: Vec<NewEntry> = (0..1200)
        .map(|n| file("box", &format!("Letters/L_{n:04}.jpg"), n))
        .collect();
    entries.replace(id, &[], &scan).await.unwrap();
    assert_eq!(entries.count(id).await.unwrap(), 1200);
    assert_eq!(entries.get(id, "box", "old.jpg").await.unwrap(), None);
    assert_eq!(
        entries.count(other).await.unwrap(),
        1,
        "another import's index stays"
    );

    let failing = [file("box", "x.jpg", 1), file("box", "x.jpg", 2)];
    assert!(entries.replace(id, &[], &failing).await.is_err());
    assert_eq!(
        entries.count(id).await.unwrap(),
        1200,
        "a failed rescan leaves the old index"
    );
    entries.replace(id, &[], &[]).await.unwrap();
    assert_eq!(entries.count(id).await.unwrap(), 0);
});

on_both!(the_index_pages_in_path_order_and_finds_one_entry, |pool| {
    let id = draft!(pool, "a");
    let entries = EntryRepo::<Db>::new(pool);
    let long = format!("{}/deep.tif", "nested-folder".repeat(80));
    let mut flagged = file("staging", "b.JPG", 7);
    flagged.media_type = None;
    flagged.flags = json!([{ "kind": "format-mismatch" }]);
    entries
        .replace(
            id,
            &[],
            &[
                file("box", "a.jpg", 1),
                file("box", "A.jpg", 2),
                file("box", &long, 3),
                flagged.clone(),
            ],
        )
        .await
        .unwrap();
    let page: Vec<(String, String)> = entries
        .page(id, &Paging::new(3, 0))
        .await
        .unwrap()
        .into_iter()
        .map(|row| (row.source, row.path))
        .collect();
    assert_eq!(
        page,
        [
            ("box".to_string(), "A.jpg".to_string()),
            ("box".to_string(), "a.jpg".to_string()),
            ("box".to_string(), long.clone()),
        ]
    );
    let rest = entries.page(id, &Paging::new(3, 3)).await.unwrap();
    assert_eq!(rest.len(), 1);
    assert_eq!(rest[0].flags, flagged.flags);

    let found = entries.get(id, "staging", "b.JPG").await.unwrap().unwrap();
    assert_eq!(
        (found.size, found.media_type, found.modified),
        (7, None, flagged.modified)
    );
    assert_eq!(
        entries.get(id, "box", &long).await.unwrap().unwrap().size,
        3
    );
    assert_eq!(
        entries.get(id, "box", "a.jpg").await.unwrap().unwrap().size,
        1
    );
    assert_eq!(
        entries.get(id, "box", "A.jpg").await.unwrap().unwrap().size,
        2
    );
    assert_eq!(entries.get(id, "staging", "a.jpg").await.unwrap(), None);
});

on_both!(deleting_a_draft_removes_its_uploads_and_index, |pool| {
    let id = draft!(pool, "a");
    UploadRepo::<Db>::new(pool)
        .create(&upload(id, "u1", "a.jpg", 1))
        .await
        .unwrap();
    EntryRepo::<Db>::new(pool)
        .replace(id, &[], &[file("box", "a.jpg", 1)])
        .await
        .unwrap();
    assert!(ImportRepo::<Db>::new(pool).delete_draft(id).await.unwrap());
    assert_eq!(UploadRepo::<Db>::new(pool).list(id).await.unwrap(), []);
    assert_eq!(EntryRepo::<Db>::new(pool).count(id).await.unwrap(), 0);
});

fn scan_of(source: &str, token: &str) -> NewScan {
    NewScan {
        source: source.to_string(),
        anomalies: json!({ "dot": 2, "unreadable": ["Box/locked"] }),
        error: None,
        token: token.to_string(),
        scanned: now(),
    }
}

on_both!(
    a_scan_record_is_not_a_file_and_is_replaced_with_the_index,
    |pool| {
        let id = draft!(pool, "a");
        let other = draft!(pool, "b");
        let entries = EntryRepo::<Db>::new(pool);
        let mut broken = scan_of("gone", "t1");
        broken.error = Some("no archive root is named \"gone\"".to_string());
        broken.anomalies = json!({});
        entries
            .replace(
                id,
                &[scan_of("box", "t1"), broken.clone()],
                &[file("box", "a.jpg", 1), file("box", "b.jpg", 2)],
            )
            .await
            .unwrap();
        entries
            .replace(other, &[scan_of("box", "kept")], &[])
            .await
            .unwrap();
        assert_eq!(
            entries.count(id).await.unwrap(),
            2,
            "the count equals the files"
        );
        assert_eq!(entries.page(id, &Paging::default()).await.unwrap().len(), 2);
        assert_eq!(entries.all(id).await.unwrap().len(), 2);

        let scans = entries.scans(id).await.unwrap();
        assert_eq!(
            scans
                .iter()
                .map(|row| row.source.as_str())
                .collect::<Vec<_>>(),
            ["box", "gone"]
        );
        assert_eq!(scans[0].anomalies["dot"], 2);
        assert_eq!(scans[0].error, None);
        assert_eq!(scans[1].error, broken.error);

        let failing = [file("box", "x.jpg", 1), file("box", "x.jpg", 2)];
        assert!(
            entries
                .replace(id, &[scan_of("box", "t2")], &failing)
                .await
                .is_err()
        );
        assert_eq!(
            entries.scans(id).await.unwrap()[0].token,
            "t1",
            "a failed rescan leaves the old scan records"
        );

        entries
            .replace(id, &[scan_of("box", "t3")], &[])
            .await
            .unwrap();
        let scans = entries.scans(id).await.unwrap();
        assert_eq!(scans.len(), 1, "a source the new scan lacks is dropped");
        assert_eq!(scans[0].token, "t3");
        assert_eq!(entries.scans(other).await.unwrap()[0].token, "kept");
        assert!(ImportRepo::<Db>::new(pool).delete_draft(id).await.unwrap());
        assert_eq!(entries.scans(id).await.unwrap(), []);
    }
);

// Timing behind the choice of read under the evidence loader (`docs/import-v2.md`, "Database"):
// `page` with and without an index on the path, and `all`.
// 40,001 rows, the median of 7 reads. It does nothing unless `ROMEKA_MEASURE` is set. Run by
// hand, in release mode: `ROMEKA_MEASURE=1 ROMEKA_MYSQL_URL=... cargo test --release -p omeka-db
// --features mysql --test import_evidence measure_the_page -- --nocapture`.
on_both!(measure_the_page_read_with_and_without_an_index, |pool| {
    use std::time::{Duration, Instant};

    if std::env::var_os("ROMEKA_MEASURE").is_none() {
        return;
    }

    let id = draft!(pool, "a");
    let entries = EntryRepo::<Db>::new(pool);
    let rows: Vec<NewEntry> = (0..40_001)
        .map(|n| file("s1", &format!("Box {}/OBJ_{n:06}.jpg", n / 500), n))
        .collect();
    entries.replace(id, &[], &rows).await.unwrap();

    async fn median(entries: &EntryRepo<'_, Db>, id: omeka_db::ids::ImportId) -> Duration {
        let mut times = Vec::new();
        for _ in 0..7 {
            let started = Instant::now();
            let page = entries.page(id, &Paging::default()).await.unwrap();
            times.push(started.elapsed());
            assert_eq!(page.len(), 40_001);
        }
        times.sort();
        times[3]
    }
    let engine = std::any::type_name::<Db>();
    println!("{engine} no extra index: {:?}", median(&entries, id).await);
    let mut times = Vec::new();
    for _ in 0..7 {
        let started = Instant::now();
        assert_eq!(entries.all(id).await.unwrap().len(), 40_001);
        times.push(started.elapsed());
    }
    times.sort();
    println!("{engine} unordered read (EntryRepo::all): {:?}", times[3]);

    let variants = [
        (
            "path in the key",
            "CREATE INDEX idx_probe ON romeka_import_entry (import_id, source, path)",
        ),
        (
            "path prefix 255 in the key",
            "CREATE INDEX idx_probe ON romeka_import_entry (import_id, source, path(255))",
        ),
    ];
    for (name, ddl) in variants {
        if sqlx::query::<Db>(ddl).execute(pool).await.is_err() {
            println!("{engine} {name}: the engine refuses it");
            continue;
        }
        for analyze in [
            "ANALYZE TABLE romeka_import_entry",
            "ANALYZE romeka_import_entry",
        ] {
            if sqlx::query::<Db>(analyze).execute(pool).await.is_ok() {
                break;
            }
        }
        println!("{engine} {name}: {:?}", median(&entries, id).await);
        for drop in [
            "DROP INDEX idx_probe ON romeka_import_entry",
            "DROP INDEX idx_probe",
        ] {
            if sqlx::query::<Db>(drop).execute(pool).await.is_ok() {
                break;
            }
        }
    }
});
