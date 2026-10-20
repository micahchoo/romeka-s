//! How long the preview route takes on a large archive, for the requests the
//! admin sends on each plan edit. Ignored by default; run it in release:
//!
//! ```sh
//! cargo test --release -p omeka-server --test imports_preview_scale -- --ignored --nocapture
//! ```
//!
//! `ROMEKA_SCALE_ITEMS` (default `1000,10000`) and `ROMEKA_SCALE_RUNS`
//! (default 7) change the sizes and the runs per request. Each size prints
//! one line per request: median, min and max in milliseconds, and the answer
//! numbers that prove the work ran.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::fmt::Write as _;
use std::path::Path;
use std::time::Instant;

use axum::Router;
use axum::http::StatusCode;
use common::{append_key, json_request, seeded_pool, send};
use omeka_api::jsonld::BaseUrl;
use omeka_db::repos::PropertyRepo;
use omeka_db::rows::NewProperty;
use omeka_server::{Config, Services};
use serde_json::{Value, json};
use sqlx::SqlitePool;

const FOLDERS: [&str; 5] = ["Letters", "Photographs", "Postcards", "Diaries", "Maps"];

/// A JPEG of 2 x 2 pixels whose EXIF holds only `DateTimeOriginal`: SOI, an
/// APP1 segment with a little-endian TIFF block (IFD0 pointing at the EXIF
/// IFD), a frame header, and EOI.
fn jpeg(date: &str) -> Vec<u8> {
    let mut tiff = b"II*\0".to_vec();
    tiff.extend(8_u32.to_le_bytes());
    tiff.extend(1_u16.to_le_bytes());
    tiff.extend(0x8769_u16.to_le_bytes());
    tiff.extend(4_u16.to_le_bytes());
    tiff.extend(1_u32.to_le_bytes());
    tiff.extend(26_u32.to_le_bytes());
    tiff.extend(0_u32.to_le_bytes());
    tiff.extend(1_u16.to_le_bytes());
    tiff.extend(0x9003_u16.to_le_bytes());
    tiff.extend(2_u16.to_le_bytes());
    tiff.extend(20_u32.to_le_bytes());
    tiff.extend(44_u32.to_le_bytes());
    tiff.extend(0_u32.to_le_bytes());
    tiff.extend(date.as_bytes());
    tiff.push(0);
    let mut bytes = vec![0xFF, 0xD8, 0xFF, 0xE1];
    bytes.extend(u16::try_from(tiff.len() + 8).unwrap().to_be_bytes());
    bytes.extend(b"Exif\0\0");
    bytes.extend(tiff);
    bytes.extend([
        0xFF, 0xC0, 0x00, 0x11, 8, 0, 2, 0, 2, 3, 1, 0x11, 0, 2, 0x11, 0, 3, 0x11, 0,
    ]);
    bytes.extend([0xFF, 0xD9]);
    bytes
}

/// `items` items over five folders, every other one with a front and a back,
/// plus one stray file per hundred items, and a spreadsheet with a row per
/// item and a few percent of faults. Answers the file count.
fn write_archive(root: &Path, items: usize) -> usize {
    let mut files = 0;
    for folder in FOLDERS {
        std::fs::create_dir_all(root.join(folder)).unwrap();
    }
    for n in 0..items {
        let folder = root.join(FOLDERS[n % FOLDERS.len()]);
        let date = format!("19{:02}:05:14 11:02:10", n % 100);
        if n % 2 == 0 {
            std::fs::write(folder.join(format!("BHC_{n:05}.jpg")), jpeg(&date)).unwrap();
            files += 1;
        } else {
            for part in ["front", "back"] {
                std::fs::write(folder.join(format!("BHC_{n:05}_{part}.jpg")), jpeg(&date)).unwrap();
                files += 1;
            }
        }
        if n % 100 == 0 {
            std::fs::write(folder.join(format!("notes_{n}.txt")), b"stray").unwrap();
            files += 1;
        }
    }
    let mut csv = String::from("Accession No,Title,Date,Description,Creator\n");
    for n in 0..items {
        let date = match n % 100 {
            0 => "12/03/1890".to_string(),
            1 => "1890-13-01".to_string(),
            _ => format!("19{:02}-05-14", n % 100),
        };
        let title = match n % 100 {
            2 => format!("  Letter {n} "),
            3 => format!("CafÃ© {n}"),
            _ => format!("Letter {n}"),
        };
        if n % 200 == 4 {
            writeln!(csv, "BHC_{n:05},{title},{date}").unwrap();
        } else {
            writeln!(
                csv,
                "BHC_{n:05},{title},{date},A letter about the harbour,R. Mehta"
            )
            .unwrap();
        }
    }
    for n in 0..items / 200 {
        writeln!(csv, "BHC_9{n:04}X,Orphan {n},1901,No file,Unknown").unwrap();
    }
    std::fs::write(root.join("catalogue.csv"), csv).unwrap();
    files
}

fn plan() -> Value {
    json!({
        "pattern": r"^(?<set>[^/]+)/(?<identifier>BHC_\d{5})(?:_(?<part>[a-z]+))?\.jpg$",
        "example": null, "setLevel": "level1", "sets": {}, "typeFor": {},
        "csv": {
            "file": "catalogue.csv", "delimiter": ",", "headerRow": 1, "windows1252": false,
            "trim": false, "joinColumn": "Accession No",
            "columns": {
                "Title": "dcterms:title", "Date": "dcterms:date",
                "Description": "dcterms:description", "Creator": "dcterms:creator"
            }
        },
        "meta": [{ "tag": "DateTimeOriginal", "term": "dcterms:created", "on": "item", "from": "first" }],
        "priority": "csv", "edits": {}, "skip": [], "withoutFiles": [], "csvRow": {},
        "problemRows": "leave"
    })
}

/// The bodies the admin sends, by tab, each with the unsaved plan.
fn requests() -> Vec<(&'static str, Value)> {
    let plan = plan();
    vec![
        (
            "folder: files page 50",
            json!({ "include": ["files"], "files": { "filter": "all", "offset": 0, "limit": 50 }, "plan": plan }),
        ),
        (
            "folder: counts (limit 0)",
            json!({ "include": ["files"], "files": { "filter": "match", "offset": 0, "limit": 0 }, "plan": plan }),
        ),
        (
            "sources: csv+tags+counts",
            json!({ "include": ["csv", "tags", "files", "rows"],
                    "files": { "filter": "match", "offset": 0, "limit": 0 },
                    "rows": { "problemsOnly": false, "offset": 0, "limit": 0 }, "plan": plan }),
        ),
        (
            "grid: rows page 50 + kinds",
            json!({ "include": ["rows"], "rows": { "problemsOnly": false, "offset": 0, "limit": 50 }, "plan": plan }),
        ),
        (
            "grid: date filter page 50",
            json!({ "include": ["rows"], "rows": { "problemsOnly": true, "kind": "date",
                    "term": "dcterms:date", "offset": 0, "limit": 50 }, "plan": plan }),
        ),
        ("check", json!({ "include": ["check"], "plan": plan })),
    ]
}

async fn app(dir: &Path) -> (Router, SqlitePool) {
    let (pool, seed) = seeded_pool().await;
    for term in ["identifier", "date", "description", "creator", "created"] {
        PropertyRepo::new(&pool)
            .create(&NewProperty {
                owner_id: None,
                vocabulary_id: seed.vocabulary,
                local_name: term.to_string(),
                label: term.to_string(),
                comment: None,
            })
            .await
            .unwrap();
    }
    let config = Config {
        base_url: BaseUrl::parse("http://localhost").unwrap(),
        archive_root: Some(dir.join("archive")),
        ..Config::default()
    };
    let services = Services::new(omeka_server::local_store(dir.join("files")));
    (
        omeka_server::build_router_with_services(pool.clone(), &config, services),
        pool,
    )
}

async fn post(app: &Router, uri: &str, body: &Value) -> (f64, Value) {
    let started = Instant::now();
    let answer = send(
        app.clone(),
        json_request("POST", &append_key(uri), &body.to_string()),
    )
    .await;
    let elapsed = started.elapsed().as_secs_f64() * 1000.0;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    (elapsed, answer.json())
}

/// What proves the request did its work: the counts its answer carries.
fn proof(answer: &Value) -> String {
    let mut out = Vec::new();
    if let Some(files) = answer.get("files") {
        out.push(format!(
            "files {}/{} matched, {} items, page {}",
            files["matched"],
            files["total"],
            files["items"],
            files["page"].as_array().map_or(0, Vec::len)
        ));
    }
    if let Some(tags) = answer.get("tags") {
        out.push(format!("DateTimeOriginal found {}", tags[0]["found"]));
    }
    if let Some(rows) = answer.get("rows") {
        out.push(format!(
            "rows {} total, {} with problems, {} filtered, {} kinds, page {}",
            rows["total"],
            rows["withProblems"],
            rows["filtered"],
            rows["kinds"].as_array().map_or(0, Vec::len),
            rows["page"].as_array().map_or(0, Vec::len)
        ));
    }
    if let Some(check) = answer.get("check") {
        out.push(format!(
            "check create {}, canRun {}",
            check["create"], check["canRun"]
        ));
    }
    if let Some(error) = answer.get("error") {
        out.push(format!("error {error}"));
    }
    out.join("; ")
}

/// User plus system time of this process so far, from `/proc/self/stat`
/// (clock ticks of 10 ms).
fn cpu_ms() -> f64 {
    let stat = std::fs::read_to_string("/proc/self/stat").unwrap_or_default();
    let fields: Vec<&str> = stat
        .rsplit_once(')')
        .map_or("", |(_, rest)| rest)
        .split_whitespace()
        .collect();
    let ticks = |at: usize| {
        fields
            .get(at)
            .and_then(|f| f.parse::<f64>().ok())
            .unwrap_or(0.0)
    };
    (ticks(11) + ticks(12)) * 10.0
}

fn env_list(name: &str, default: &str) -> Vec<usize> {
    std::env::var(name)
        .unwrap_or_else(|_| default.to_string())
        .split(',')
        .map(|n| n.trim().parse().unwrap())
        .collect()
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "a measurement: run in release with --ignored --nocapture"]
async fn preview_route_at_scale() {
    let runs = env_list("ROMEKA_SCALE_RUNS", "7")[0];
    for items in env_list("ROMEKA_SCALE_ITEMS", "1000,10000") {
        let dir = tempfile::tempdir().unwrap();
        let files = write_archive(&dir.path().join("archive"), items);
        let (app, _) = app(dir.path()).await;
        let created = send(
            app.clone(),
            json_request(
                "POST",
                &append_key("/api/romeka/imports"),
                &json!({ "folder": "" }).to_string(),
            ),
        )
        .await;
        assert_eq!(created.status, StatusCode::OK, "{}", created.body);
        let id = created.json()["id"].as_u64().unwrap();
        let uri = format!("/api/romeka/imports/{id}/preview");

        let started = Instant::now();
        let auth = send(
            app.clone(),
            common::get(&format!("/api/romeka/imports/{id}"), true),
        )
        .await;
        assert_eq!(auth.status, StatusCode::OK);
        let floor = started.elapsed().as_secs_f64() * 1000.0;
        let (cold, _) = post(&app, &uri, &requests()[2].1).await;
        println!(
            "\n{items} items, {files} files, {runs} runs; GET import {floor:.1} ms; \
             first sources (tags read cold) {cold:.0} ms"
        );
        let requests = requests();
        let mut times = vec![Vec::new(); requests.len()];
        let mut proofs = vec![String::new(); requests.len()];
        let cpu_before = cpu_ms();
        for _ in 0..runs {
            for (index, (_, body)) in requests.iter().enumerate() {
                let (elapsed, answer) = post(&app, &uri, body).await;
                times[index].push(elapsed);
                proofs[index] = proof(&answer);
            }
        }
        let cpu = cpu_ms() - cpu_before;
        let wall: f64 = times.iter().flatten().sum();
        println!("  process CPU {cpu:.0} ms over {wall:.0} ms of requests");
        for (index, (name, _)) in requests.iter().enumerate() {
            let samples = &mut times[index];
            samples.sort_by(f64::total_cmp);
            println!(
                "  {name:<28} median {:>7.1} ms  range {:>7.1} to {:>7.1}  | {}",
                samples[samples.len() / 2],
                samples[0],
                samples[samples.len() - 1],
                proofs[index]
            );
        }
    }
}

fn median(samples: &mut [f64]) -> (f64, f64, f64) {
    samples.sort_by(f64::total_cmp);
    (
        samples[samples.len() / 2],
        samples[0],
        samples[samples.len() - 1],
    )
}

/// The route's steps timed one by one, outside HTTP, so the share of each
/// shows where a preview's time goes.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "a measurement: run in release with --ignored --nocapture"]
async fn preview_steps_at_scale() {
    use omeka_api::import::archive::ArchiveRoot;
    use omeka_api::import::inputs::Inputs;
    use omeka_api::import::plan::ImportPlan;
    use omeka_api::import::preview::{self, PreviewContext, PreviewRequest};
    use omeka_api::import::tags::{SharedTagReader, TagCache};

    let runs = env_list("ROMEKA_SCALE_RUNS", "7")[0];
    for items in env_list("ROMEKA_SCALE_ITEMS", "1000,10000") {
        let dir = tempfile::tempdir().unwrap();
        write_archive(&dir.path().join("archive"), items);
        let (_, pool) = app(dir.path()).await;
        let plan = ImportPlan::from_json(&plan()).unwrap();
        let reader = SharedTagReader::default();
        let folder = ArchiveRoot::new(dir.path().join("archive"))
            .unwrap()
            .folder("")
            .unwrap();
        let bodies = requests();
        let names = [
            "catalog::load",
            "walk",
            "Inputs::load",
            "derive",
            "build grid",
            "build check",
            "build sources",
            "serialize grid",
            "drop model",
        ];
        let mut times = vec![Vec::new(); names.len()];
        for _ in 0..=runs {
            let mut step = Vec::new();
            let clock = Instant::now();
            let catalog = omeka_api::import::catalog::load(&pool).await.unwrap();
            step.push(clock.elapsed());
            let clock = Instant::now();
            let walked = folder.files().unwrap();
            step.push(clock.elapsed());
            assert!(walked.len() > items);
            let clock = Instant::now();
            let inputs = Inputs::load(&folder, &plan).unwrap();
            step.push(clock.elapsed());
            let tags = TagCache::new(&reader);
            let clock = Instant::now();
            let model = omeka_api::import::derive::derive(&plan, &inputs, &tags, &catalog);
            step.push(clock.elapsed());
            let context = PreviewContext {
                plan: &plan,
                inputs: &inputs,
                model: &model,
                tags: &tags,
                catalog: &catalog,
            };
            let mut grid = None;
            for index in [3, 5, 2] {
                let request: PreviewRequest =
                    serde_json::from_value(bodies[index].1.clone()).unwrap();
                let clock = Instant::now();
                let built = preview::build(&context, &request);
                step.push(clock.elapsed());
                if index == 3 {
                    grid = Some(built);
                }
            }
            let clock = Instant::now();
            let text = serde_json::to_string(&grid.unwrap()).unwrap();
            step.push(clock.elapsed());
            assert!(text.len() > 1000);
            let clock = Instant::now();
            drop(model);
            step.push(clock.elapsed());
            for (index, elapsed) in step.into_iter().enumerate() {
                times[index].push(elapsed.as_secs_f64() * 1000.0);
            }
        }
        println!("\n{items} items, {runs} runs after one warm-up (tags warm)");
        for (index, name) in names.iter().enumerate() {
            let (mid, low, high) = median(&mut times[index][1..]);
            println!("  {name:<16} median {mid:>7.2} ms  range {low:>7.2} to {high:>7.2}");
        }
    }
}
