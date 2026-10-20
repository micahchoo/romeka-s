#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::fs;
use std::sync::Arc;

use omeka_db::ids::ImportId;
use omeka_db::repos::ImportRepo;
use omeka_db::rows::{EntryRow, NewImport, NewScan, ScanRow};

use super::*;
use crate::import::plan::RelPath;

fn folder(id: &str, root: &str) -> SourcePlan {
    SourcePlan::Folder {
        id: SourceId(id.into()),
        root: root.into(),
        path: RelPath::parse("").expect("root"),
    }
}

fn entry(path: &str) -> FileEntry {
    FileEntry {
        path: RelPath::parse(path).expect("path"),
        size: 12,
        modified: Some(UNIX_EPOCH + Duration::from_secs(1_700_000_000)),
        sniffed: "image/jpeg".into(),
        decoded: None,
    }
}

/// What `EntryRepo::scans` would read back from `scans`.
fn scans_back(scans: Vec<NewScan>) -> Vec<ScanRow> {
    scans
        .into_iter()
        .map(|scan| ScanRow {
            import_id: ImportId(1),
            source: scan.source,
            anomalies: scan.anomalies,
            error: scan.error,
            token: scan.token,
            scanned: scan.scanned,
        })
        .collect()
}

/// What `EntryRepo::page` would read back from `rows`.
fn read_back(rows: Vec<NewEntry>) -> Vec<EntryRow> {
    rows.into_iter()
        .map(|row| EntryRow {
            import_id: ImportId(1),
            source: row.source,
            path: row.path,
            size: row.size,
            modified: row.modified,
            media_type: row.media_type,
            flags: row.flags,
        })
        .collect()
}

#[test]
fn a_walk_survives_the_rows_with_its_sheets_decoded_names_and_anomalies() {
    let mut decoded = entry("Año/a.jpg");
    decoded.decoded = Some(DecodedName {
        encoding: NameEncoding::Cp437,
        raw: b"A\xA4o/a.jpg".to_vec(),
    });
    let mut undated = entry("b.jpg");
    undated.modified = None;
    let walk = Walk {
        entries: vec![decoded.clone(), undated.clone()],
        sheets: vec![entry("register.csv")],
        anomalies: Anomalies {
            symlinks: 2,
            dot: 3,
            collisions: vec!["x".into()],
            ..Anomalies::default()
        },
    };
    let walked = vec![
        Walked {
            source: SourceId("s1".into()),
            walk: Ok(walk.clone()),
        },
        Walked {
            source: SourceId("s2".into()),
            walk: Err("no archive root is named \"gone\"".into()),
        },
    ];
    let (scans, rows) = rows(&walked, epoch());
    let plan = [folder("s1", "a"), folder("s2", "gone"), folder("s3", "a")];

    let stored = index_of(scans_back(scans), read_back(rows), &plan).expect("decodes");

    let s1 = &stored.index.sources[0];
    assert_eq!(s1.id, SourceId("s1".into()));
    let mut expected = walk.entries.clone();
    expected.sort_by(|a, b| a.path.cmp(&b.path));
    assert_eq!(
        s1.walk.entries, expected,
        "decoded bytes and a missing time"
    );
    assert_eq!(s1.walk.sheets, walk.sheets);
    assert_eq!(s1.walk.anomalies, walk.anomalies);
    assert_eq!(stored.index.sources.len(), 2, "s3 was never scanned");
    assert_eq!(stored.sources.len(), 3);
    assert_eq!(stored.sources[0].files, 2);
    assert_eq!(stored.sources[0].sheet_files, ["register.csv"]);
    assert!(stored.sources[1].error.as_deref().unwrap().contains("gone"));
    assert!(stored.sources[1].scanned.is_some());
    assert_eq!(stored.sources[2].scanned, None);
    assert_eq!(stored.size(), 3);
}

async fn draft(pool: &sqlx::SqlitePool) -> ImportId {
    ImportRepo::new(pool)
        .create_draft(&NewImport {
            owner_id: None,
            sources: serde_json::json!([]),
            plan: serde_json::json!({ "version": 2 }),
            copied_from: None,
        })
        .await
        .unwrap()
        .id
}

#[tokio::test]
async fn a_scan_is_stored_and_a_warm_load_reuses_it_until_the_next_scan() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join("Box 1")).unwrap();
    fs::write(dir.path().join("Box 1/a.jpg"), b"\xFF\xD8\xFF\xE0jpeg").unwrap();
    fs::write(dir.path().join(".DS_Store"), b"x").unwrap();
    let roots = ArchiveRoots::new([("a".to_string(), dir.path().to_path_buf())]).unwrap();
    let pool = crate::testing::temp_db().await;
    let import = draft(&pool).await;
    let sources = [folder("s1", "a")];
    let cache = IndexCache::default();

    let scanned = scan(&pool, &roots, &no_store(), import, &sources)
        .await
        .unwrap();
    assert_eq!(scanned.sources[0].files, 1);
    assert_eq!(
        EntryRepo::new(&pool).count(import).await.unwrap(),
        1,
        "the anomalies are not an entry row"
    );

    let first = load(&pool, &cache, import, &sources).await.unwrap();
    assert_eq!(
        first.index.sources[0].walk.entries[0].path.as_str(),
        "Box 1/a.jpg"
    );
    assert_eq!(first.index.sources[0].walk.anomalies.dot, 1);
    let again = load(&pool, &cache, import, &sources).await.unwrap();
    assert!(Arc::ptr_eq(&first.index, &again.index), "warm");

    fs::write(dir.path().join("Box 1/b.jpg"), b"\xFF\xD8\xFF\xE0jpeg").unwrap();
    scan(&pool, &roots, &no_store(), import, &sources)
        .await
        .unwrap();
    let after = load(&pool, &cache, import, &sources).await.unwrap();
    assert!(!Arc::ptr_eq(&first.index, &after.index), "a rescan is new");
    assert_eq!(after.index.sources[0].walk.entries.len(), 2);
}

#[tokio::test]
async fn the_scan_job_rescans_from_the_stored_plan() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.jpg"), b"\xFF\xD8\xFF\xE0jpeg").unwrap();
    let roots = ArchiveRoots::new([("a".to_string(), dir.path().to_path_buf())]).unwrap();
    let pool = crate::testing::temp_db().await;
    let plan = crate::import::plan::default_plan(vec![folder("s1", "a")]).unwrap();
    let import = ImportRepo::new(&pool)
        .create_draft(&NewImport {
            owner_id: None,
            sources: serde_json::json!([]),
            plan: serde_json::from_str(&plan.to_json()).unwrap(),
            copied_from: None,
        })
        .await
        .unwrap()
        .id;
    let scanned = scan_import(&pool, &roots, &no_store(), import)
        .await
        .unwrap();
    assert_eq!(scanned.sources[0].files, 1);
    assert!(matches!(
        scan_import(&pool, &roots, &no_store(), ImportId(99)).await,
        Err(ScanError::Missing(99))
    ));
    let v1 = draft(&pool).await;
    assert!(matches!(
        scan_import(&pool, &roots, &no_store(), v1).await,
        Err(ScanError::Plan(_))
    ));
}

#[test]
fn identical_files_survive_the_rows_and_an_older_row_reads_as_none() {
    let set = Identical {
        size: 12,
        sha256: "ab".repeat(32),
        paths: vec!["a.jpg".into(), "copy/a.jpg".into()],
    };
    let walk = Walk {
        entries: vec![entry("a.jpg"), entry("copy/a.jpg")],
        anomalies: Anomalies {
            identical: vec![set.clone()],
            ..Anomalies::default()
        },
        ..Walk::default()
    };
    let walked = vec![Walked {
        source: SourceId("s1".into()),
        walk: Ok(walk),
    }];
    let (scans, rows) = rows(&walked, epoch());
    let plan = [folder("s1", "a")];
    let stored = index_of(scans_back(scans.clone()), read_back(rows.clone()), &plan).unwrap();
    assert_eq!(stored.index.sources[0].walk.anomalies.identical, [set]);

    let mut older = scans_back(scans);
    older[0].anomalies = serde_json::json!({ "symlinks": 1, "dot": 0, "special": 0,
        "unreadable": [], "unusableNames": [], "collisions": [] });
    let stored = index_of(older, read_back(rows), &plan).expect("a row from before the member");
    assert!(stored.index.sources[0].walk.anomalies.identical.is_empty());
    assert_eq!(stored.index.sources[0].walk.anomalies.symlinks, 1);
}

/// A store for scans whose plans have no staging source: nothing reads it.
fn no_store() -> std::sync::Arc<dyn omeka_media::store::Store> {
    std::sync::Arc::new(omeka_media::store::FileStore::new(
        std::env::temp_dir().join("romeka-scan-tests-unused"),
        "/files",
    ))
}
