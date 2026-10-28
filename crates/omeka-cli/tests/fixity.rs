//! The fixity check end to end: an administrator starts "Check stored files", the job reads every
//! original through the store, and each count of the result equals the total of the Media list it
//! opens, for the same caller. `romeka fixity check` runs the same job for a cron.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod bulk_site;
mod import_site;

use std::time::{Duration, Instant};

use bulk_site::*;
use omeka_cli::commands::fixity::{CheckOptions, check};
use omeka_db::ids::ItemId;
use omeka_db::repos::{ItemRepo, JobRepo, MediaRepo};
use omeka_db::rows::{NewItem, NewJob, NewMedia, NewResource};
use omeka_media::store::sha256_hex;
use serde_json::Value;

/// The originals the site holds, by what the check must find.
struct Shelf {
    ok: Vec<u64>,
    differs: u64,
    missing: u64,
    unreadable: u64,
    /// A media with no original: never read, never counted.
    no_original: u64,
    /// An original with no recorded SHA-256: nothing to compare against.
    unrecorded: u64,
}

async fn media(
    site: &Site,
    item: ItemId,
    storage_id: &str,
    sha256: Option<String>,
    public: bool,
) -> u64 {
    MediaRepo::new(&site.pool)
        .create(&NewMedia {
            resource: NewResource {
                is_public: public,
                ..NewResource::default()
            },
            item_id: item,
            ingester: "upload".to_string(),
            renderer: "file".to_string(),
            source: Some(format!("{storage_id}.bin")),
            media_type: Some("application/octet-stream".to_string()),
            storage_id: Some(storage_id.to_string()),
            extension: Some("bin".to_string()),
            sha256,
            has_original: true,
            ..NewMedia::default()
        })
        .await
        .unwrap()
        .id()
        .0
}

fn store_file(site: &Site, storage_id: &str, bytes: &[u8]) {
    let dir = site.files.join("original");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(format!("{storage_id}.bin")), bytes).unwrap();
}

/// One file of each outcome, one of them longer than a read chunk, and one private.
async fn shelf(site: &Site) -> Shelf {
    let item = ItemRepo::new(&site.pool)
        .create(&NewItem::default())
        .await
        .unwrap()
        .id();
    let small = b"a letter, scanned".to_vec();
    let large: Vec<u8> = (0..9 * 1024 * 1024 + 7).map(|n| (n % 251) as u8).collect();
    store_file(site, "small", &small);
    store_file(site, "large", &large);
    store_file(site, "private", &small);
    let ok = vec![
        media(site, item, "small", Some(sha256_hex(&small)), true).await,
        media(
            site,
            item,
            "large",
            Some(sha256_hex(&large).to_uppercase()),
            true,
        )
        .await,
        media(site, item, "private", Some(sha256_hex(&small)), false).await,
    ];
    store_file(site, "changed", b"the bytes after a disk fault");
    let differs = media(
        site,
        item,
        "changed",
        Some(sha256_hex(b"the bytes at ingest")),
        true,
    )
    .await;
    let missing = media(site, item, "gone", Some(sha256_hex(b"never stored")), true).await;
    // A folder where the file should be: the store finds something and cannot read it.
    std::fs::create_dir_all(site.files.join("original/folder.bin")).unwrap();
    let unreadable = media(site, item, "folder", Some(sha256_hex(b"x")), true).await;
    let no_original = MediaRepo::new(&site.pool)
        .create(&NewMedia {
            item_id: item,
            ingester: "url".to_string(),
            renderer: "file".to_string(),
            has_original: false,
            ..NewMedia::default()
        })
        .await
        .unwrap()
        .id()
        .0;
    store_file(site, "unrecorded", &small);
    let unrecorded = media(site, item, "unrecorded", None, true).await;
    Shelf {
        ok,
        differs,
        missing,
        unreadable,
        no_original,
        unrecorded,
    }
}

/// The job's row once it has ended.
async fn ended(site: &Site, job: u64) -> Value {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let (status, row) = admin(site, "GET", &format!("/api/jobs/{job}"), None).await;
        assert_eq!(status, 200, "{row}");
        if !row["o:ended"].is_null() {
            return row;
        }
        assert!(Instant::now() < deadline, "the check ended: {row}");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// The ids and the total of the Media list a count opens, as `person` reads it.
async fn listed(person: &Person, shown: &str, job: u64) -> (Vec<u64>, u64) {
    let path = format!("/api/media?romeka_fixity={shown}&romeka_fixity_job={job}&sort_by=id");
    let (status, head, body) = person.raw("GET", &path, None).await;
    assert_eq!(status, 200, "{body}");
    let total = head
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("omeka-s-total-results")
                .then(|| value.trim().parse::<u64>().unwrap())
        })
        .expect("a total");
    let ids = serde_json::from_str::<Value>(&body)
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .map(|media| media["o:id"].as_u64().unwrap())
        .collect();
    (ids, total)
}

const COUNTS: [&str; 5] = ["checked", "ok", "differs", "missing", "unreadable"];

#[tokio::test(flavor = "multi_thread")]
async fn a_check_records_each_outcome_and_each_count_opens_its_list() {
    let site = site().await;
    let shelf = shelf(&site).await;
    let admin_person = Person::new(&site, "global_admin").await;

    let (status, started) = admin_person.send("POST", "/api/romeka/fixity", None).await;
    assert_eq!(status, 201, "{started}");
    let job = started["job"].as_u64().unwrap();
    let row = ended(&site, job).await;
    assert_eq!(row["o:status"], "completed", "{row}");
    assert_eq!(row["o:job_class"], "Romeka\\Job\\FixityCheck");

    let (status, result) = admin_person
        .send("GET", &format!("/api/romeka/fixity/{job}"), None)
        .await;
    assert_eq!(status, 200, "{result}");
    assert_eq!(result["checked"], 6, "{result}");
    assert_eq!(result["ok"], 3, "{result}");
    assert_eq!(result["differs"], 1, "{result}");
    assert_eq!(result["missing"], 1, "{result}");
    assert_eq!(result["unreadable"], 1, "{result}");
    assert!(result["ended"].is_string(), "{result}");

    let (ids, _) = listed(&admin_person, "ok", job).await;
    assert_eq!(ids, shelf.ok);
    assert_eq!(
        listed(&admin_person, "differs", job).await.0,
        [shelf.differs]
    );
    assert_eq!(
        listed(&admin_person, "missing", job).await.0,
        [shelf.missing]
    );
    assert_eq!(
        listed(&admin_person, "unreadable", job).await.0,
        [shelf.unreadable]
    );
    let (checked, _) = listed(&admin_person, "checked", job).await;
    assert!(
        !checked.contains(&shelf.no_original),
        "a media with no original is not read"
    );
    assert!(
        !checked.contains(&shelf.unrecorded),
        "an original with no SHA-256 is not judged"
    );

    for word in COUNTS {
        let (_, total) = listed(&admin_person, word, job).await;
        assert_eq!(
            total,
            result[word].as_u64().unwrap(),
            "{word} equals its list total"
        );
    }

    let (_, log) = admin_person
        .raw("GET", &format!("/api/romeka/jobs/{job}/log"), None)
        .await
        .into_split_body();
    assert!(
        log.contains(&format!("media {}: differs", shelf.differs)),
        "{log}"
    );
    assert!(
        log.contains(&format!("media {}: missing", shelf.missing)),
        "{log}"
    );
    assert!(
        log.contains("Checked 6 files: 3 ok, 1 differ, 1 missing, 1 unreadable."),
        "{log}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn admins_start_a_check_readers_of_jobs_see_it_and_no_one_else_does() {
    let site = site().await;
    let shelf = shelf(&site).await;
    let site_admin = Person::new(&site, "site_admin").await;
    let editor = Person::new(&site, "editor").await;
    let author = Person::new(&site, "author").await;

    let (status, _) = editor.send("POST", "/api/romeka/fixity", None).await;
    assert_eq!(status, 403, "an editor may not start a check");
    let (status, _) = author.send("POST", "/api/romeka/fixity", None).await;
    assert_eq!(status, 403, "an author may not start a check");
    let (status, started) = site_admin.send("POST", "/api/romeka/fixity", None).await;
    assert_eq!(status, 201, "a site administrator may: {started}");
    let job = started["job"].as_u64().unwrap();
    ended(&site, job).await;

    let (status, latest) = editor.send("GET", "/api/romeka/fixity", None).await;
    assert_eq!(status, 200, "{latest}");
    assert_eq!(latest["check"]["job"], job, "{latest}");
    assert!(latest["running"].is_null(), "{latest}");
    for word in COUNTS {
        let (_, total) = listed(&editor, word, job).await;
        assert_eq!(
            total,
            latest["check"][word].as_u64().unwrap(),
            "{word} for the editor"
        );
    }

    let (status, _) = author.send("GET", "/api/romeka/fixity", None).await;
    assert_eq!(status, 403, "an author may not read jobs");
    let (status, _) = author
        .send("GET", &format!("/api/romeka/fixity/{job}"), None)
        .await;
    assert_eq!(status, 403);
    assert_eq!(
        listed(&author, "differs", job).await,
        (vec![], 0),
        "the filter keeps nothing"
    );
    assert_eq!(
        listed(&author, "differs", job + 1000).await,
        (vec![], 0),
        "the same as a check that does not exist"
    );
    assert_eq!(listed(&editor, "differs", job + 1000).await, (vec![], 0));
    assert_eq!(
        listed(&editor, "sideways", job).await,
        (vec![], 0),
        "an unknown word keeps nothing"
    );

    let path = "/api/media?romeka_fixity=differs";
    let (_, body) = editor.raw("GET", path, None).await.into_split_body();
    let latest_ids: Vec<u64> = serde_json::from_str::<Value>(&body)
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .map(|media| media["o:id"].as_u64().unwrap())
        .collect();
    assert_eq!(latest_ids, [shelf.differs], "no job means the latest check");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_second_start_answers_the_check_already_running() {
    let site = site().await;
    let admin_person = Person::new(&site, "global_admin").await;
    let running = JobRepo::new(&site.pool)
        .create(&NewJob {
            owner_id: None,
            pid: None,
            status: Some("in_progress".to_string()),
            class: "Romeka\\Job\\FixityCheck".to_string(),
            args: Some(serde_json::json!({})),
            log: None,
            started: None,
        })
        .await
        .unwrap();
    let (status, again) = admin_person.send("POST", "/api/romeka/fixity", None).await;
    assert_eq!(status, 200, "{again}");
    assert_eq!(
        again["job"], running.id.0,
        "no second read of the store starts"
    );
    let (_, latest) = admin_person.send("GET", "/api/romeka/fixity", None).await;
    assert_eq!(latest["running"], running.id.0, "{latest}");
    assert!(
        latest["check"].is_null(),
        "a check with no result yet is not the latest: {latest}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn the_cli_check_records_a_job_and_reports_the_problems() {
    let site = site().await;
    let shelf = shelf(&site).await;
    let report = check(&CheckOptions {
        database: site.dir.path().join("romeka.db").into(),
        store: Some(site.files.clone()),
    })
    .await
    .unwrap();
    assert!(report.has_problems());
    assert_eq!(
        report.record.differs.ids().collect::<Vec<_>>(),
        [shelf.differs]
    );
    assert_eq!(report.record.ok.len(), 3);
    let (status, result) = admin(
        &site,
        "GET",
        &format!("/api/romeka/fixity/{}", report.job.0),
        None,
    )
    .await;
    assert_eq!(status, 200, "{result}");
    assert_eq!(
        result["checked"], 6,
        "the CLI run is a check like any other: {result}"
    );
}

trait SplitBody {
    fn into_split_body(self) -> (u16, String);
}

impl SplitBody for (u16, String, String) {
    fn into_split_body(self) -> (u16, String) {
        (self.0, self.2)
    }
}
