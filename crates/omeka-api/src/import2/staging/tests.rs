#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::io::{self, Cursor, Read};
use std::sync::Arc;

use omeka_db::ids::ImportId;
use omeka_db::repos::{ImportRepo, UploadRepo};
use omeka_db::rows::NewImport;
use omeka_media::store::{FileStore, Store};
use sqlx::SqlitePool;

use super::*;
use crate::import2::plan::{SourceId, SourcePlan, default_plan};
use crate::import2::scan;

pub(crate) const LIMITS: Limits = Limits {
    max_file: 1000,
    max_total: 2000,
};

pub(crate) struct Site {
    pub pool: SqlitePool,
    pub store: Arc<dyn Store>,
    pub import: ImportId,
    _root: tempfile::TempDir,
}

pub(crate) async fn site() -> Site {
    let root = tempfile::tempdir().unwrap();
    let pool = crate::testing::temp_db().await;
    let plan = default_plan(vec![SourcePlan::Staging {
        id: SourceId("up".into()),
    }])
    .unwrap();
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
    Site {
        pool,
        store: Arc::new(FileStore::new(root.path(), "/files")),
        import,
        _root: root,
    }
}

pub(crate) fn body(bytes: &[u8]) -> Box<dyn Read + Send> {
    Box::new(Cursor::new(bytes.to_vec()))
}

/// A body that yields its bytes and then fails, as a dropped connection does.
fn dropped(bytes: &[u8]) -> Box<dyn Read + Send> {
    struct Dropped(Cursor<Vec<u8>>);
    impl Read for Dropped {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            match self.0.read(buffer)? {
                0 => Err(io::Error::new(
                    io::ErrorKind::ConnectionReset,
                    "client gone",
                )),
                read => Ok(read),
            }
        }
    }
    Box::new(Dropped(Cursor::new(bytes.to_vec())))
}

impl Site {
    pub(crate) async fn upload(&self, path: &str, bytes: &[u8]) -> UploadRow {
        self.upload_within(path, bytes, LIMITS).await
    }

    pub(crate) async fn upload_within(
        &self,
        path: &str,
        bytes: &[u8],
        limits: Limits,
    ) -> UploadRow {
        let row = create(&self.pool, self.import, path, bytes.len() as u64, limits)
            .await
            .unwrap();
        let appended = append(
            &self.pool,
            &self.store,
            self.import,
            &row.id,
            0,
            body(bytes),
        )
        .await
        .unwrap();
        appended.completed.expect("one request brings every byte")
    }

    async fn received(&self, upload: &str) -> u64 {
        UploadRepo::new(&self.pool)
            .get(self.import, upload)
            .await
            .unwrap()
            .unwrap()
            .received
    }
}

#[tokio::test]
async fn a_dropped_patch_keeps_nothing_and_the_upload_resumes_from_the_stored_offset() {
    let site = site().await;
    let row = create(&site.pool, site.import, "Box 1/0001.tif", 10, LIMITS)
        .await
        .unwrap();
    let first = append(
        &site.pool,
        &site.store,
        site.import,
        &row.id,
        0,
        body(b"0123"),
    )
    .await
    .unwrap();
    assert_eq!(first.received, 4);
    assert!(first.completed.is_none());

    let broken = append(
        &site.pool,
        &site.store,
        site.import,
        &row.id,
        4,
        dropped(b"456"),
    )
    .await
    .expect_err("the body broke off");
    assert!(matches!(broken, StagingError::Interrupted(_)), "{broken:?}");
    assert_eq!(
        site.received(&row.id).await,
        4,
        "HEAD answers the stored offset"
    );

    let replayed = append(
        &site.pool,
        &site.store,
        site.import,
        &row.id,
        0,
        body(b"0123"),
    )
    .await
    .expect_err("a PATCH from an old offset");
    assert!(
        matches!(replayed, StagingError::OffsetConflict { received: 4 }),
        "{replayed:?}"
    );

    let last = append(
        &site.pool,
        &site.store,
        site.import,
        &row.id,
        4,
        body(b"456789"),
    )
    .await
    .unwrap();
    assert_eq!(last.received, 10);
    let complete = last.completed.expect("complete");
    assert_eq!(
        site.store
            .read(&file_path(site.import, &complete.id))
            .unwrap(),
        b"0123456789"
    );
    assert!(
        part_files(site.store.as_ref(), site.import, &row.id)
            .unwrap()
            .is_empty(),
        "the parts are gone once the file is whole"
    );
}

#[tokio::test]
async fn bytes_past_the_declared_length_are_refused_and_not_kept() {
    let site = site().await;
    let row = create(&site.pool, site.import, "a.jpg", 3, LIMITS)
        .await
        .unwrap();
    let error = append(
        &site.pool,
        &site.store,
        site.import,
        &row.id,
        0,
        body(b"abcd"),
    )
    .await
    .expect_err("past the length");
    assert!(
        matches!(error, StagingError::PastLength { length: 3 }),
        "{error:?}"
    );
    assert_eq!(site.received(&row.id).await, 0);
    assert!(
        part_files(site.store.as_ref(), site.import, &row.id)
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn the_limits_count_uploads_still_in_progress() {
    let site = site().await;
    let big = create(&site.pool, site.import, "big.tif", 1000, LIMITS)
        .await
        .unwrap();
    append(
        &site.pool,
        &site.store,
        site.import,
        &big.id,
        0,
        body(b"part"),
    )
    .await
    .unwrap();
    create(&site.pool, site.import, "second.tif", 900, LIMITS)
        .await
        .unwrap();
    let full = create(&site.pool, site.import, "third.tif", 101, LIMITS)
        .await
        .expect_err("2001 bytes would pass 2000");
    assert!(
        matches!(
            full,
            StagingError::ImportFull {
                limit: 2000,
                used: 1900
            }
        ),
        "{full:?}"
    );
    create(&site.pool, site.import, "fits.tif", 100, LIMITS)
        .await
        .expect("exactly the limit fits");
    let large = create(&site.pool, site.import, "huge.tif", 1001, LIMITS)
        .await
        .expect_err("over the file limit");
    assert!(
        matches!(large, StagingError::FileTooLarge { limit: 1000 }),
        "{large:?}"
    );
}

#[tokio::test]
async fn a_name_the_path_rules_refuse_is_refused() {
    let site = site().await;
    for name in [
        "../a.jpg",
        "/a.jpg",
        "a\\b.jpg",
        "",
        "a//b.jpg",
        "a/./b.jpg",
        "a%2F..%2Fb",
    ] {
        let error = create(&site.pool, site.import, name, 1, LIMITS)
            .await
            .expect_err(name);
        assert!(
            matches!(error, StagingError::Path { .. }),
            "{name}: {error:?}"
        );
    }
}

#[tokio::test]
async fn an_unfinished_upload_holds_its_path_and_a_finished_one_is_replaced() {
    let site = site().await;
    let open = create(&site.pool, site.import, "a.jpg", 2, LIMITS)
        .await
        .unwrap();
    let held = create(&site.pool, site.import, "a.jpg", 2, LIMITS)
        .await
        .expect_err("held");
    assert!(
        matches!(&held, StagingError::InProgress { upload } if *upload == open.id),
        "{held:?}"
    );
    append(
        &site.pool,
        &site.store,
        site.import,
        &open.id,
        0,
        body(b"v1"),
    )
    .await
    .unwrap();
    let second = site.upload("a.jpg", b"v2").await;
    let rows = UploadRepo::new(&site.pool).list(site.import).await.unwrap();
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert_eq!(rows[0].id, second.id);
    assert!(
        !site
            .store
            .exists(&file_path(site.import, &open.id))
            .unwrap()
    );
    assert_eq!(
        site.store
            .read(&file_path(site.import, &second.id))
            .unwrap(),
        b"v2"
    );
}

#[tokio::test]
async fn a_scan_walks_the_finished_uploads_through_the_store() {
    let site = site().await;
    site.upload("Box 1/0001.jpg", b"\xFF\xD8\xFF\xE0jpeg").await;
    site.upload("Box 1/0002.jpg", b"\xFF\xD8\xFF\xE0jpeg").await;
    site.upload("list.csv", b"Identifier,Title\nBox 1,First box\n")
        .await;
    site.upload(".DS_Store", b"junk").await;
    create(&site.pool, site.import, "Box 2/0001.jpg", 9, LIMITS)
        .await
        .unwrap();
    let sources = vec![SourcePlan::Staging {
        id: SourceId("up".into()),
    }];
    let roots = crate::import2::archive::ArchiveRoots::new(Vec::new()).unwrap();
    let scanned = scan::scan(&site.pool, &roots, &site.store, site.import, &sources)
        .await
        .unwrap();
    assert_eq!(
        scanned.sources[0].files, 2,
        "the unfinished upload is not a file yet"
    );
    assert_eq!(scanned.sources[0].sheet_files, ["list.csv"]);
    let stored = scan::load(
        &site.pool,
        &scan::IndexCache::default(),
        site.import,
        &sources,
    )
    .await
    .unwrap();
    let walk = &stored.index.sources[0].walk;
    assert_eq!(walk.entries[0].path.as_str(), "Box 1/0001.jpg");
    assert_eq!(walk.entries[0].sniffed, "image/jpeg");
    assert_eq!(walk.entries[0].size, 8);
    assert_eq!(walk.anomalies.dot, 1);
    assert_eq!(
        walk.anomalies.identical.len(),
        1,
        "the two same files are found"
    );
    assert_eq!(
        walk.anomalies.identical[0].paths,
        ["Box 1/0001.jpg", "Box 1/0002.jpg"]
    );
}

#[tokio::test]
async fn removing_the_staging_area_leaves_nothing_in_the_store() {
    let site = site().await;
    let kept = site.upload("a.jpg", b"aa").await;
    let open = create(&site.pool, site.import, "b.jpg", 4, LIMITS)
        .await
        .unwrap();
    append(
        &site.pool,
        &site.store,
        site.import,
        &open.id,
        0,
        body(b"bb"),
    )
    .await
    .unwrap();
    site.store
        .put(b"orphan", &file_path(site.import, "no-row"))
        .unwrap();
    assert_eq!(
        remove_all(&site.pool, &site.store, site.import)
            .await
            .unwrap(),
        2
    );
    assert!(
        UploadRepo::new(&site.pool)
            .list(site.import)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        !site
            .store
            .exists(&file_path(site.import, &kept.id))
            .unwrap()
    );
    assert!(
        !site
            .store
            .exists(&file_path(site.import, "no-row"))
            .unwrap()
    );
    assert!(
        part_files(site.store.as_ref(), site.import, &open.id)
            .unwrap()
            .is_empty()
    );
}
