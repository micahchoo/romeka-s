//! Import v2 runs end to end (`docs/import-v2.md`, "Scenarios"): the routes
//! over a real socket, then `Romeka\Job\ImportRun` through the job runner,
//! then what the install holds.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod import_site;

use std::time::Duration;

use import_site::{PNG, Site, col, jpeg, rule, sheet_plan, site, tiff};
use omeka_db::ids::{ImportId, JobId, PropertyId};
use omeka_db::repos::{BeforeImageRepo, ImportRepo, JobRepo, SettingRepo, ValueRepo};
use omeka_db::rows::{ImportChange, ImportStatus};
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const BHC_PATTERN: &str = r"^(?<set>[^/]+)/(?:[^/]+/)*(?<identifier>BHC_006_[A-Z]{2}_\d{3})(?:_(?<part>[a-z0-9]+))?\.(?:jpg|tif)$";

/// The BHC_006 demo with real image bytes: four artifacts with a front and a
/// back, four photographs in a decade folder, one scan in three pages, and
/// a scratch folder.
fn bhc_006(site: &Site) {
    let (jpeg, tiff) = (jpeg(), tiff());
    for n in 1..=4 {
        site.write_object(&format!("Artifacts/BHC_006_AF_00{n}_front.jpg"), &jpeg);
        site.write_object(&format!("Artifacts/BHC_006_AF_00{n}_back.jpg"), &jpeg);
        site.write_object(&format!("Photographs/1950s/BHC_006_PH_10{n}.tif"), &tiff);
    }
    for page in ["p1", "p2", "p10"] {
        site.write_object(&format!("Scans/BHC_006_SC_001_{page}.tif"), &tiff);
    }
    site.write("_scratch/notes.txt", b"call the donor about AF_003\n");
    site.write(
        "_scratch/Thumbs.db",
        b"\xD0\xCF\x11\xE0\xA1\xB1\x1A\xE1\x00\x00",
    );
}

fn bhc_plan(plan: &mut Value) {
    plan["grouping"]["pattern"] = json!(BHC_PATTERN);
    plan["slots"]["item"]["sets"] = rule(json!([{ "from": "group", "name": "set" }]), "first");
}

fn summary(row: &omeka_db::rows::ImportRow) -> Value {
    row.summary.clone().unwrap_or(Value::Null)
}

#[tokio::test(flavor = "multi_thread")]
async fn s1_the_bhc_006_demo_is_written_private_with_thumbnails() {
    let site = site().await;
    bhc_006(&site);
    let id = site.draft(bhc_plan).await;

    let row = site.run(id).await;

    assert_eq!(row.status, ImportStatus::Done, "{}", summary(&row));
    assert_eq!(summary(&row)["failures"], json!([]));
    assert_eq!(
        summary(&row)["progress"],
        json!({ "done": 24, "total": 24 })
    );
    let items = site.items().await;
    assert_eq!(items.len(), 9);
    assert!(items.iter().all(|item| !item.resource.is_public), "private");
    let sets = site.item_sets().await;
    let mut titles: Vec<_> = sets
        .iter()
        .map(|set| set.resource.title.clone().unwrap_or_default())
        .collect();
    titles.sort();
    assert_eq!(titles, ["Artifacts", "Photographs", "Scans"]);
    assert!(
        sets.iter().all(|set| set.is_open),
        "a set named by title is open"
    );
    let media = site.media().await;
    assert_eq!(media.len(), 15, "the scratch files are not media");
    assert!(
        media.iter().all(|media| media.has_thumbnails),
        "JPEG and TIFF thumbnailed"
    );
    let stored = site.stored_files();
    for media in &media {
        let id = media.storage_id.as_deref().unwrap();
        for prefix in ["original", "large", "medium", "square"] {
            assert!(
                stored
                    .iter()
                    .any(|path| path.starts_with(prefix) && path.contains(id)),
                "{prefix} of {id}"
            );
        }
    }
    let ledger = site.ledger(id).await;
    let records = ledger
        .iter()
        .filter(|row| row.record.starts_with("group:") || row.record.starts_with("file:"))
        .count();
    assert_eq!(records, 24, "one ledger row per item and media");
    assert!(ledger.iter().all(|row| row.change == ImportChange::Created));
    let left = site
        .send("GET", &format!("/api/romeka/imports/{id}/left.csv"), None)
        .await;
    assert_eq!(left.status, 200, "{}", left.body);
    assert!(left.body.starts_with("kind,source,path,record,#reason\r\n"));
    assert!(left.body.contains("_scratch/notes.txt"), "{}", left.body);
    assert!(left.body.contains("_scratch/Thumbs.db"), "{}", left.body);
}

#[tokio::test(flavor = "multi_thread")]
async fn s4_pages_in_natural_order_and_the_first_page_is_primary() {
    let site = site().await;
    let tiff = tiff();
    for page in ["2", "10", "1"] {
        site.write_object(&format!("Box 1/Folder 1/{page}.tif"), &tiff);
    }
    site.write_object("Box 1/Folder 2/1.tif", &tiff);
    let synced = omeka_db::repos::SiteRepo::new(&site.pool)
        .create(&omeka_db::rows::NewSite {
            slug: "synced".into(),
            theme: "default".into(),
            title: "Synced".into(),
            item_pool: json!({}),
            assign_new_items: true,
            ..omeka_db::rows::NewSite::default()
        })
        .await
        .unwrap();
    let id = site
        .draft(|plan| {
            plan["grouping"]["pattern"] =
                json!(r"^(?<identifier>Box \d+/Folder \d+)/(?<part>\d+)\.tif$");
        })
        .await;

    let row = site.run(id).await;

    assert_eq!(row.status, ImportStatus::Done, "{}", summary(&row));
    let items = site.items().await;
    assert_eq!(items.len(), 2);
    let mut folder = None;
    for item in items {
        if site.media_of(&item).await.len() == 3 {
            folder = Some(item);
        }
    }
    let folder = folder.expect("the folder with three pages");
    let media = site.media_of(&folder).await;
    let sources: Vec<_> = media
        .iter()
        .map(|media| media.source.clone().unwrap())
        .collect();
    assert_eq!(sources, ["1.tif", "2.tif", "10.tif"]);
    assert_eq!(
        media.iter().map(|media| media.position).collect::<Vec<_>>(),
        [Some(1), Some(2), Some(3)]
    );
    assert_eq!(folder.primary_media_id, Some(media[0].id()));
    let joined = omeka_db::repos::ItemSiteRepo::new(&site.pool)
        .site_items(synced.id)
        .await
        .unwrap();
    assert_eq!(joined.len(), 2, "created items join the synced site");
}

#[tokio::test(flavor = "multi_thread")]
async fn s6_media_rows_attach_by_item_key_with_their_own_values() {
    let site = site().await;
    let tiff = tiff();
    for name in ["s001", "s002", "s003"] {
        site.write_object(&format!("scans/{name}.tif"), &tiff);
    }
    site.write("objects.csv", b"Id,Title\nO1,Diary\nO2,Map\n");
    site.write(
        "images.csv",
        b"Object,File,Page\nO1,s002.tif,2\nO2,s003.tif,1\nO1,s001.tif,1\n",
    );
    let id = site
        .draft(|plan| {
            // The files make no items of their own: the register's rows are the items.
            plan["grouping"]["pattern"] = json!(null);
            plan["sheets"] = json!([
                sheet_plan("objects", &["Id", "Title"]),
                sheet_plan("images", &["Object", "File", "Page"])
            ]);
            plan["sheets"][0]["itemKey"] = json!({ "index": 0, "header": "Id" });
            plan["sheets"][1]["rowKind"] = json!("media");
            plan["sheets"][1]["join"] = json!({
                "on": "fileName", "column": { "index": 1, "header": "File" },
                "normalize": { "caseFold": false, "leadingZeros": false, "separators": false }
            });
            plan["sheets"][1]["itemKey"] = json!({ "index": 0, "header": "Object" });
            plan["slots"]["item"]["dcterms:title"] =
                rule(json!([col("objects", 1, "Title")]), "first");
            plan["slots"]["media"]["mediaOrder"] = rule(json!([col("images", 2, "Page")]), "first");
            plan["slots"]["media"]["dcterms:title"] =
                rule(json!([col("images", 1, "File")]), "first");
        })
        .await;

    let row = site.run(id).await;

    assert_eq!(row.status, ImportStatus::Done, "{}", summary(&row));
    let diary = site.item("Diary").await;
    let titles: Vec<_> = site
        .media_of(&diary)
        .await
        .iter()
        .map(|media| media.resource.title.clone().unwrap_or_default())
        .collect();
    assert_eq!(titles, ["s001.tif", "s002.tif"]);
    let map = site.item("Map").await;
    assert_eq!(site.media_of(&map).await.len(), 1);
    assert_eq!(site.media().await.len(), 3);
}

/// An HTTP server on loopback that answers every request with the PNG
/// fixture. Answers its base URL.
async fn png_server() -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            tokio::spawn(async move {
                let mut buffer = [0u8; 4096];
                let _ = stream.read(&mut buffer).await;
                let head = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    PNG.len()
                );
                let _ = stream.write_all(head.as_bytes()).await;
                let _ = stream.write_all(PNG).await;
            });
        }
    });
    format!("http://{addr}")
}

fn mixed_plan(plan: &mut Value) {
    let headers = ["Kind", "Id", "Title", "Set", "Item", "URL", "Related"];
    plan["sheets"] = json!([sheet_plan("mixed", &headers)]);
    plan["sheets"][0]["rowKind"] = json!({ "column": { "index": 0, "header": "Kind" } });
    plan["sheets"][0]["itemKey"] = json!({ "index": 4, "header": "Item" });
    plan["sheets"][0]["columns"][6]["options"]["type"] = json!({ "resource": "identifier" });
    for kind in ["item", "itemSet"] {
        plan["slots"][kind]["dcterms:identifier"] = rule(json!([col("mixed", 1, "Id")]), "first");
        plan["slots"][kind]["dcterms:title"] = rule(json!([col("mixed", 2, "Title")]), "first");
    }
    plan["slots"]["item"]["sets"] = rule(json!([col("mixed", 3, "Set")]), "first");
    plan["slots"]["item"]["dcterms:relation"] = rule(json!([col("mixed", 6, "Related")]), "first");
    plan["slots"]["media"]["ingester"] =
        rule(json!([{ "from": "constant", "value": "url" }]), "first");
    plan["slots"]["media"]["mediaSource"] = rule(json!([col("mixed", 5, "URL")]), "first");
}

async fn property(site: &Site, term: &str) -> PropertyId {
    let (prefix, local) = term.split_once(':').unwrap();
    let vocabulary = omeka_db::repos::VocabularyRepo::new(&site.pool)
        .get_by_prefix(prefix)
        .await
        .unwrap()
        .unwrap();
    omeka_db::repos::PropertyRepo::new(&site.pool)
        .find(vocabulary.id, local)
        .await
        .unwrap()
        .unwrap()
        .id
}

#[tokio::test(flavor = "multi_thread")]
async fn s8_a_mixed_sheet_writes_sets_items_media_and_links_by_identifier() {
    let site = site().await;
    SettingRepo::new(&site.pool)
        .set(omeka_api::settings::IMPORT_ALLOW_PRIVATE_URLS, &json!(true))
        .await
        .unwrap();
    let base = png_server().await;
    site.write(
        "mixed.csv",
        format!(
            "Kind,Id,Title,Set,Item,URL,Related\n\
             item set,SET1,Letters,,,,\n\
             item,L1,First letter,SET1,,,\n\
             item,L2,Reply,SET1,,,L1\n\
             item,L3,Third,SET1,,,\n\
             media,,,,L1,{base}/1.png,\n\
             media,,,,L3,{base}/3.png,\n"
        )
        .as_bytes(),
    );
    let id = site.draft(mixed_plan).await;
    // The second original is the third letter's, after the reply is written
    // and before the links: the link write lands a second later.
    site.store.slow_after(1, Duration::from_millis(1100));

    let row = site.run(id).await;

    assert_eq!(row.status, ImportStatus::Done, "{}", summary(&row));
    let sets = site.item_sets().await;
    assert_eq!(sets.len(), 1);
    assert_eq!(sets[0].resource.title.as_deref(), Some("Letters"));
    assert!(!sets[0].is_open, "an item-set record is closed");
    let first = site.item("First letter").await;
    let reply = site.item("Reply").await;
    let media = site.media_of(&first).await;
    assert_eq!(media.len(), 1);
    assert_eq!(media[0].ingester, "url");
    assert!(media[0].has_original && media[0].has_thumbnails);
    assert_eq!(first.primary_media_id, Some(media[0].id()));
    let relation = property(&site, "dcterms:relation").await;
    let links = ValueRepo::new(&site.pool)
        .list_for_property(reply.resource.id, relation)
        .await
        .unwrap();
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].r#type, "resource:item");
    assert_eq!(links[0].value_resource_id, Some(first.resource.id));
    let modified = reply.resource.modified.unwrap();
    assert!(
        modified > reply.resource.created,
        "the link was written later"
    );
    let reply_row = site
        .ledger(id)
        .await
        .into_iter()
        .find(|row| row.target == omeka_db::rows::ImportTarget::Item(reply.id()))
        .unwrap();
    assert!(
        reply_row.written >= modified,
        "the link write moved the ledger's written time"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_url_source_on_loopback_is_refused_and_listed() {
    let site = site().await;
    let base = png_server().await;
    site.write(
        "mixed.csv",
        format!(
            "Kind,Id,Title,Set,Item,URL,Related\n\
             item,L1,First letter,,,,\n\
             media,,,,L1,{base}/1.png,\n"
        )
        .as_bytes(),
    );
    let id = site.draft(mixed_plan).await;

    let row = site.run(id).await;

    assert_eq!(row.status, ImportStatus::Done, "the item is written");
    let failures = summary(&row)["failures"].clone();
    assert_eq!(failures.as_array().unwrap().len(), 1, "{failures}");
    assert_eq!(failures[0]["key"], "server.importSourceRefused");
    assert!(
        failures[0]["record"]
            .as_str()
            .unwrap()
            .starts_with("row:mixed/")
    );
    assert!(site.media().await.is_empty());
    assert_eq!(site.stored_files(), ["imports/1/left.csv"]);
    let left = site.files.join("imports/1/left.csv");
    let left = std::fs::read_to_string(left).unwrap();
    assert!(left.contains("was not added"), "{left}");

    SettingRepo::new(&site.pool)
        .set(omeka_api::settings::IMPORT_ALLOW_PRIVATE_URLS, &json!(true))
        .await
        .unwrap();
    let retried = site.run(id).await;

    assert_eq!(retried.status, ImportStatus::Done, "{}", summary(&retried));
    assert_eq!(summary(&retried)["failures"], json!([]));
    let items = site.items().await;
    assert_eq!(
        items.len(),
        1,
        "the retry adds the media to the item it wrote"
    );
    let media = site.media_of(&items[0]).await;
    assert_eq!(media.len(), 1);
    assert_eq!(items[0].primary_media_id, Some(media[0].id()));
}

#[tokio::test(flavor = "multi_thread")]
async fn s9_revise_changes_values_and_keeps_before_images() {
    let site = site().await;
    bhc_006(&site);
    let first = site.draft(bhc_plan).await;
    assert_eq!(site.run(first).await.status, ImportStatus::Done);
    let register: String = std::iter::once("Id,Title\n".to_string())
        .chain((1..=4).map(|n| format!("BHC_006_AF_00{n},Artifact {n}\n")))
        .collect();
    site.write("register.csv", register.as_bytes());

    let second = site
        .draft(|plan| {
            bhc_plan(plan);
            plan["action"]["action"] = json!("revise");
            plan["sheets"] = json!([sheet_plan("register", &["Id", "Title"])]);
            plan["sheets"][0]["join"] = json!({
                "on": "identifier", "column": { "index": 0, "header": "Id" },
                "normalize": { "caseFold": false, "leadingZeros": false, "separators": false }
            });
            plan["slots"]["item"]["dcterms:title"] =
                rule(json!([col("register", 1, "Title")]), "first");
        })
        .await;
    let check = site.check(second).await;
    let row = site.run(second).await;

    assert_eq!(row.status, ImportStatus::Done, "{} {check}", summary(&row));
    for n in 1..=4 {
        let item = site.item(&format!("Artifact {n}")).await;
        assert!(!item.resource.is_public);
    }
    assert_eq!(site.items().await.len(), 9, "revise writes no new item");
    assert_eq!(site.media().await.len(), 15, "and no new media");
    let images = BeforeImageRepo::new(&site.pool)
        .list(ImportId(second))
        .await
        .unwrap();
    assert_eq!(images.len(), 9, "one before-image per revised item");
    let ledger = site.ledger(second).await;
    assert_eq!(ledger.len(), 9);
    assert!(ledger.iter().all(|row| row.change == ImportChange::Changed));
}

#[tokio::test(flavor = "multi_thread")]
async fn s17_a_killed_run_retried_ends_done_with_no_duplicate() {
    let site = site().await;
    bhc_006(&site);
    // No sets: a retry's derive then finds no record `exists` in scope, so
    // only the ledger keeps the first attempt's records from being written
    // again.
    let id = site
        .draft(|plan| plan["grouping"]["pattern"] = json!(BHC_PATTERN))
        .await;
    let gate = site.store.hold_after(5);
    let job = site.start(id).await;
    tokio::task::spawn_blocking(move || {
        gate.reached
            .recv_timeout(Duration::from_secs(30))
            .expect("the run reached the sixth file");
        gate
    })
    .await
    .map(|gate| {
        for task in site.jobs.tasks.lock().unwrap().drain(..) {
            task.abort();
        }
        drop(gate.release);
    })
    .unwrap();
    let partial = site.ledger(id).await.len();
    assert!(
        partial > 0 && partial < 24,
        "{partial} rows before the kill"
    );
    assert!(site.items().await.len() < 9);
    assert_eq!(site.row(id).await.status, ImportStatus::Running);
    JobRepo::new(&site.pool)
        .finish(JobId::from(job), "error")
        .await
        .unwrap();
    let read = site
        .send("GET", &format!("/api/romeka/imports/{id}"), None)
        .await;
    assert_eq!(read.json()["status"], "run_failed", "{}", read.body);

    let row = site.run(id).await;

    assert_eq!(row.status, ImportStatus::Done, "{}", summary(&row));
    assert_eq!(site.items().await.len(), 9, "no duplicate item");
    assert_eq!(site.media().await.len(), 15, "no duplicate media");
    let ledger = site.ledger(id).await;
    let mut keys: Vec<_> = ledger.iter().map(|row| row.record.clone()).collect();
    keys.sort();
    keys.dedup();
    assert_eq!(keys.len(), ledger.len(), "one ledger row per record");
    let left = site
        .send("GET", &format!("/api/romeka/imports/{id}/left.csv"), None)
        .await;
    assert!(!left.body.contains("BHC_006"), "{}", left.body);
    let stored = site.stored_files();
    for media in site.media().await {
        let id = media.storage_id.unwrap();
        assert!(
            stored
                .iter()
                .any(|path| path.starts_with("original") && path.contains(&id))
        );
    }
    let originals = stored
        .iter()
        .filter(|path| path.starts_with("original"))
        .count();
    // A killed worker cannot remove the bytes of the item it held; at most
    // that item's files stay, for `romeka media sweep`.
    assert!((15..=17).contains(&originals), "{originals} originals");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_stop_ends_the_run_failed_after_the_record_in_hand() {
    let site = site().await;
    bhc_006(&site);
    let id = site.draft(bhc_plan).await;
    let gate = site.store.hold_after(2);
    site.start(id).await;
    let gate = tokio::task::spawn_blocking(move || {
        gate.reached.recv_timeout(Duration::from_secs(30)).unwrap();
        gate
    })
    .await
    .unwrap();
    let stop = site
        .send("POST", &format!("/api/romeka/imports/{id}/stop"), None)
        .await;
    assert_eq!(stop.status, 202, "{}", stop.body);
    drop(gate.release);

    let row = site.ended(id).await;

    assert_eq!(row.status, ImportStatus::RunFailed, "{}", summary(&row));
    assert_eq!(summary(&row)["stopped"], true);
    assert!(site.items().await.len() < 9, "the run stopped early");
    assert!(
        site.stored_files()
            .iter()
            .any(|path| path == &format!("imports/{id}/left.csv")),
        "left.csv is written on a stop too"
    );
    let retried = site.run(id).await;
    assert_eq!(retried.status, ImportStatus::Done, "{}", summary(&retried));
    assert_eq!(site.items().await.len(), 9);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_hand_run_is_refused_while_the_heartbeat_is_young() {
    let site = site().await;
    bhc_006(&site);
    let id = site.draft(bhc_plan).await;
    let gate = site.store.hold_after(1);
    let job = site.start(id).await;
    let gate = tokio::task::spawn_blocking(move || {
        gate.reached.recv_timeout(Duration::from_secs(30)).unwrap();
        gate
    })
    .await
    .unwrap();

    let refused = omeka_cli::jobs::refuse_live_hand_run(
        &omeka_db::DbPool::from(site.pool.clone()),
        JobId::from(job),
    )
    .await;

    assert!(refused.is_err(), "a live worker holds the import");
    drop(gate.release);
    let row = site.ended(id).await;
    assert_eq!(row.status, ImportStatus::Done, "{}", summary(&row));
    assert_eq!(
        summary(&row)["failures"][0]["key"],
        "server.importFileNotStored",
        "the held file failed alone"
    );
    let ended = ImportRepo::new(&site.pool)
        .get(ImportId(id))
        .await
        .unwrap()
        .unwrap();
    assert!(ended.status != ImportStatus::Running);
    omeka_cli::jobs::refuse_live_hand_run(
        &omeka_db::DbPool::from(site.pool.clone()),
        JobId::from(job),
    )
    .await
    .expect("an ended import refuses nothing");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_run_writes_the_values_of_an_uploaded_list_from_the_staging_area() {
    let site = site().await;
    for name in ["A1.jpg", "A2.jpg", "A3.jpg"] {
        site.write_object(name, &jpeg());
    }
    let id = site
        .draft(|plan| {
            plan["grouping"]["pattern"] = json!(r"^(?<identifier>A\d+)\.jpg$");
        })
        .await;
    let template = site
        .send(
            "GET",
            &format!("/api/romeka/imports/{id}/template.csv?columns=dcterms:title"),
            None,
        )
        .await;
    assert_eq!(template.status, 200, "{}", template.body);
    let header = template.body.split("\r\n").next().unwrap().to_string();
    let list = format!("{header}\r\nA1,,दीपक\r\nA2,,'-5\r\nA3,,Cup\r\n");
    let uploaded = site.upload_list(id, "list.csv", list.as_bytes()).await;
    assert_eq!(uploaded.status, 201, "{}", uploaded.body);

    let row = site.run(id).await;

    assert_eq!(row.status, ImportStatus::Done, "{}", summary(&row));
    let mut titles: Vec<String> = site
        .items()
        .await
        .iter()
        .map(|item| item.resource.title.clone().unwrap_or_default())
        .collect();
    titles.sort();
    assert_eq!(titles, ["-5", "Cup", "दीपक"]);
}

/// A v3 plan of one series over the archive root, its items in the set
/// level "Citizen Curators" (described) and its series' set "Demo" (not).
fn described_sets(plan: &mut Value) {
    let v3: Value = serde_json::from_str(include_str!(
        "../../omeka-api/src/import/fixtures/v3/one_series.json"
    ))
    .unwrap();
    let mut series = v3["structure"]["levels"][0].clone();
    series["grouping"]["pattern"] = json!(BHC_PATTERN);
    series["slots"]["item"] = json!({});
    series["set"] = json!({ "title": "Demo", "description": null });
    let sources = plan["sources"].clone();
    *plan = v3;
    plan["sources"] = sources;
    plan["structure"]["levels"] = json!([{
        "kind": "set", "id": "project",
        "set": { "title": "Citizen Curators", "description": "Made with the community" },
        "levels": [series]
    }]);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_new_set_named_by_title_is_written_with_its_description() {
    let site = site().await;
    bhc_006(&site);
    let id = site.draft(described_sets).await;

    let row = site.run(id).await;

    assert_eq!(row.status, ImportStatus::Done, "{}", summary(&row));
    let sets = site.item_sets().await;
    let mut titles: Vec<_> = sets
        .iter()
        .map(|set| set.resource.title.clone().unwrap_or_default())
        .collect();
    titles.sort();
    assert_eq!(titles, ["Citizen Curators", "Demo"]);
    let description = property(&site, "dcterms:description").await;
    for set in &sets {
        let values = ValueRepo::new(&site.pool)
            .list_for_property(set.resource.id, description)
            .await
            .unwrap();
        let texts: Vec<_> = values
            .iter()
            .filter_map(|value| value.value.clone())
            .collect();
        let expected: &[&str] = match set.resource.title.as_deref() {
            Some("Citizen Curators") => &["Made with the community"],
            _ => &[],
        };
        assert_eq!(texts, expected, "{:?}", set.resource.title);
    }
}

/// W4's retry read "525 of 1,094": the items the killed attempt made were
/// matched as already in Omeka, so they left the total and the run counted
/// them again as it skipped them.
#[tokio::test(flavor = "multi_thread")]
async fn a_retry_after_a_kill_keeps_its_total_and_starts_at_the_units_done() {
    let site = site().await;
    bhc_006(&site);
    let id = site.draft(bhc_plan).await;
    let gate = site.store.hold_after(5);
    let job = site.start(id).await;
    tokio::task::spawn_blocking(move || {
        gate.reached
            .recv_timeout(Duration::from_secs(30))
            .expect("the run reached the sixth file");
        gate
    })
    .await
    .map(|gate| {
        for task in site.jobs.tasks.lock().unwrap().drain(..) {
            task.abort();
        }
        drop(gate.release);
    })
    .unwrap();
    let killed = summary(&site.row(id).await)["progress"].clone();
    assert_eq!(killed["total"], 24, "{killed}");
    assert!(killed["done"].as_u64().unwrap() > 0, "{killed}");
    JobRepo::new(&site.pool)
        .finish(JobId::from(job), "error")
        .await
        .unwrap();

    let gate = site.store.hold_after(0);
    site.start(id).await;
    let gate = tokio::task::spawn_blocking(move || {
        gate.reached
            .recv_timeout(Duration::from_secs(30))
            .expect("the retry reached its first new file");
        gate
    })
    .await
    .unwrap();
    let resumed = summary(&site.row(id).await)["progress"].clone();
    drop(gate.release);
    let row = site.ended(id).await;

    assert_eq!(resumed, killed, "the retry starts at the units done");
    assert_eq!(summary(&row)["progress"]["total"], 24, "{}", summary(&row));
    let ledger = site.ledger(id).await;
    let mut keys: Vec<_> = ledger.iter().map(|row| row.record.clone()).collect();
    keys.sort();
    keys.dedup();
    assert_eq!(keys.len(), ledger.len(), "one ledger row per record");
    assert_eq!(site.items().await.len(), 9, "no duplicate item");
}

/// A kill after an item's unit committed and before it joined its sites
/// left it outside them, and the retry skips its unit. The kill here is the
/// gate's; the lost join is the detach of every item it made.
#[tokio::test(flavor = "multi_thread")]
async fn an_item_made_before_a_kill_joins_its_sites_after_the_resume() {
    let site = site().await;
    bhc_006(&site);
    let synced = omeka_db::repos::SiteRepo::new(&site.pool)
        .create(&omeka_db::rows::NewSite {
            slug: "synced".into(),
            theme: "default".into(),
            title: "Synced".into(),
            item_pool: json!({}),
            assign_new_items: true,
            ..omeka_db::rows::NewSite::default()
        })
        .await
        .unwrap();
    let id = site.draft(bhc_plan).await;
    let gate = site.store.hold_after(5);
    let job = site.start(id).await;
    tokio::task::spawn_blocking(move || {
        gate.reached
            .recv_timeout(Duration::from_secs(30))
            .expect("the run reached the sixth file");
        gate
    })
    .await
    .map(|gate| {
        for task in site.jobs.tasks.lock().unwrap().drain(..) {
            task.abort();
        }
        drop(gate.release);
    })
    .unwrap();
    JobRepo::new(&site.pool)
        .finish(JobId::from(job), "error")
        .await
        .unwrap();
    let sites = omeka_db::repos::ItemSiteRepo::new(&site.pool);
    let made: Vec<_> = site.items().await.iter().map(|item| item.id()).collect();
    assert!(!made.is_empty() && made.len() < 9, "{} items", made.len());
    sites.detach(synced.id, &made).await.unwrap();

    let row = site.run(id).await;

    assert_eq!(row.status, ImportStatus::Done, "{}", summary(&row));
    let joined = sites.site_items(synced.id).await.unwrap();
    assert_eq!(joined.len(), 9, "every item the import made is in the site");
}
