//! `romeka media sweep` over a real store: referenced files stay, old orphans go,
//! young orphans and symlinks are left alone, and a second run removes nothing.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::fs::{self, File};
use std::path::Path;
use std::time::{Duration, SystemTime};

use omeka_cli::commands::install::{InstallOptions, run as install};
use omeka_cli::commands::sweep::{MIN_AGE, SweepOptions, SweepReport, run as sweep};
use omeka_db::ids::ItemId;
use omeka_db::repos::{AssetRepo, ItemRepo, MediaRepo};
use omeka_db::rows::{NewAsset, NewItem, NewMedia};
use tempfile::TempDir;

fn put(root: &Path, relative: &str, age: Duration) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, b"bytes").unwrap();
    File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_modified(SystemTime::now() - age)
        .unwrap();
}

struct Fixture {
    dir: TempDir,
    outside: TempDir,
}

impl Fixture {
    fn database(&self) -> std::path::PathBuf {
        self.dir.path().join("romeka.db")
    }
    fn store(&self) -> std::path::PathBuf {
        self.dir.path().join("files")
    }
    fn exists(&self, relative: &str) -> bool {
        fs::symlink_metadata(self.store().join(relative)).is_ok()
    }
    async fn sweep(&self, delete: bool) -> SweepReport {
        sweep(&SweepOptions {
            database: self.database(),
            store: Some(self.store()),
            delete,
            min_age: MIN_AGE,
        })
        .await
        .unwrap()
    }
}

async fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let database = dir.path().join("romeka.db");
    install(&InstallOptions {
        database: database.clone(),
        email: "admin@example.org".to_string(),
        name: "Admin".to_string(),
        password: "a long test password".to_string(),
        title: "Test".to_string(),
        time_zone: "UTC".to_string(),
        locale: "en".to_string(),
    })
    .await
    .unwrap();
    let pool = omeka_db::open_file(&database).await.unwrap();
    let item = ItemRepo::new(&pool)
        .create(&NewItem::default())
        .await
        .unwrap();
    MediaRepo::new(&pool)
        .create(&NewMedia {
            item_id: ItemId::from(item.resource.id.0),
            ingester: "upload".to_string(),
            renderer: "file".to_string(),
            storage_id: Some("kept".to_string()),
            extension: Some("png".to_string()),
            has_original: true,
            has_thumbnails: true,
            ..NewMedia::default()
        })
        .await
        .unwrap();
    AssetRepo::new(&pool)
        .create(&NewAsset {
            owner_id: None,
            name: "logo".to_string(),
            media_type: "image/png".to_string(),
            storage_id: "logo".to_string(),
            extension: Some("png".to_string()),
            alt_text: None,
        })
        .await
        .unwrap();

    let store = dir.path().join("files");
    let old = Duration::from_secs(3600);
    put(&store, "original/kept.png", old);
    for kind in ["large", "medium", "square"] {
        put(&store, &format!("{kind}/kept.jpg"), old);
    }
    put(&store, "asset/logo.png", old);
    put(&store, "original/lost.png", old);
    for kind in ["large", "medium", "square"] {
        put(&store, &format!("{kind}/lost.jpg"), old);
    }
    put(&store, "original/fresh.png", Duration::from_secs(30));
    // A symlinked file and a symlinked directory, both pointing outside the store.
    fs::write(outside.path().join("secret.png"), b"keep me").unwrap();
    std::os::unix::fs::symlink(
        outside.path().join("secret.png"),
        store.join("original/link.png"),
    )
    .unwrap();
    fs::remove_dir_all(store.join("medium")).unwrap();
    std::os::unix::fs::symlink(outside.path(), store.join("medium")).unwrap();
    // Named by no row and old: it would be an orphan if the linked directory were followed.
    fs::write(outside.path().join("lost.jpg"), b"keep me too").unwrap();
    File::options()
        .write(true)
        .open(outside.path().join("lost.jpg"))
        .unwrap()
        .set_modified(SystemTime::now() - old)
        .unwrap();
    Fixture { dir, outside }
}

fn sorted(mut paths: Vec<String>) -> Vec<String> {
    paths.sort();
    paths
}

#[tokio::test]
async fn the_dry_run_lists_exactly_the_old_orphans_and_removes_nothing() {
    let f = fixture().await;
    let report = f.sweep(false).await;
    assert_eq!(
        sorted(report.orphans),
        ["large/lost.jpg", "original/lost.png", "square/lost.jpg"]
    );
    assert_eq!(report.young, 1);
    assert!(f.exists("original/lost.png"));
}

#[tokio::test]
async fn delete_removes_exactly_the_old_orphans_and_a_second_run_removes_nothing() {
    let f = fixture().await;
    let report = f.sweep(true).await;
    assert_eq!(
        sorted(report.orphans),
        ["large/lost.jpg", "original/lost.png", "square/lost.jpg"]
    );
    assert!(report.failed.is_empty());
    for gone in ["original/lost.png", "large/lost.jpg", "square/lost.jpg"] {
        assert!(!f.exists(gone), "{gone} should be gone");
    }
    for kept in [
        "original/kept.png",
        "large/kept.jpg",
        "square/kept.jpg",
        "asset/logo.png",
        "original/fresh.png",
        "original/link.png",
    ] {
        assert!(f.exists(kept), "{kept} should remain");
    }
    let again = f.sweep(true).await;
    assert!(again.orphans.is_empty());
    assert_eq!(again.young, 1);
}

#[tokio::test]
async fn nothing_outside_the_store_is_touched() {
    let f = fixture().await;
    f.sweep(true).await;
    f.sweep(true).await;
    assert_eq!(
        fs::read(f.outside.path().join("secret.png")).unwrap(),
        b"keep me"
    );
    assert_eq!(
        fs::read(f.outside.path().join("lost.jpg")).unwrap(),
        b"keep me too"
    );
    assert!(
        fs::symlink_metadata(f.store().join("medium"))
            .unwrap()
            .file_type()
            .is_symlink()
    );
}
