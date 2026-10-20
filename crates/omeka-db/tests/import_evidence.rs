//! What an import holds before it writes (migration 0006): its uploads (`romeka_import_upload`)
//! and its evidence index (`romeka_import_entry`). Every test runs on SQLite and, with the `mysql`
//! feature, on MySQL.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;
mod import_db;

use chrono::{NaiveDate, NaiveDateTime, TimeDelta, Utc};
use import_db::{draft, on_both};
use omeka_db::repos::{EntryRepo, ImportRepo, Paging, UploadAdvance, UploadRepo};
use omeka_db::rows::{NewEntry, NewUpload};
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
        .replace(id, &[file("box", "old.jpg", 1)])
        .await
        .unwrap();
    entries
        .replace(other, &[file("box", "kept.jpg", 1)])
        .await
        .unwrap();
    let scan: Vec<NewEntry> = (0..1200)
        .map(|n| file("box", &format!("Letters/L_{n:04}.jpg"), n))
        .collect();
    entries.replace(id, &scan).await.unwrap();
    assert_eq!(entries.count(id).await.unwrap(), 1200);
    assert_eq!(entries.get(id, "box", "old.jpg").await.unwrap(), None);
    assert_eq!(
        entries.count(other).await.unwrap(),
        1,
        "another import's index stays"
    );

    let failing = [file("box", "x.jpg", 1), file("box", "x.jpg", 2)];
    assert!(entries.replace(id, &failing).await.is_err());
    assert_eq!(
        entries.count(id).await.unwrap(),
        1200,
        "a failed rescan leaves the old index"
    );
    entries.replace(id, &[]).await.unwrap();
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
        .replace(id, &[file("box", "a.jpg", 1)])
        .await
        .unwrap();
    assert!(ImportRepo::<Db>::new(pool).delete_draft(id).await.unwrap());
    assert_eq!(UploadRepo::<Db>::new(pool).list(id).await.unwrap(), []);
    assert_eq!(EntryRepo::<Db>::new(pool).count(id).await.unwrap(), 0);
});
