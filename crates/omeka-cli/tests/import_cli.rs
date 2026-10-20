//! `romeka import run`, `undo` and `list` as an operator types them
//! (`docs/import-v2.md`, "Command line"): the binary on the BHC_006 demo, then
//! what the install holds.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod import_site;

use std::path::PathBuf;
use std::process::{Command, Output};
use std::time::Duration;

use import_site::{Site, jpeg, rule, site, tiff};
use omeka_api::import2::plan::{SourcePlan, default_plan};
use omeka_db::repos::ImportRepo;
use omeka_db::rows::ImportStatus;
use serde_json::{Value, json};

const BHC_PATTERN: &str = r"^(?<set>[^/]+)/(?:[^/]+/)*(?<identifier>BHC_006_[A-Z]{2}_\d{3})(?:_(?<part>[a-z0-9]+))?\.(?:jpg|tif)$";
const ADMIN: &str = "admin@example.org";

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
}

/// A plan file on the whole archive root; `edit` changes the default plan.
fn plan_file(site: &Site, edit: impl FnOnce(&mut Value)) -> PathBuf {
    let sources: Vec<SourcePlan> = serde_json::from_value(
        json!([{ "kind": "folder", "id": "s1", "root": "archive", "path": "" }]),
    )
    .unwrap();
    let mut plan: Value = serde_json::from_str(&default_plan(sources).unwrap().to_json()).unwrap();
    edit(&mut plan);
    let path = site.dir.path().join("plan.json");
    std::fs::write(&path, plan.to_string()).unwrap();
    path
}

fn bhc_plan(plan: &mut Value) {
    plan["grouping"]["pattern"] = json!(BHC_PATTERN);
    plan["slots"]["item"]["sets"] = rule(json!([{ "from": "group", "name": "set" }]), "first");
}

/// The binary with the install's database and store; `run` also reads the
/// archive root.
fn romeka(site: &Site, args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_romeka"));
    command
        .args(args)
        .arg("--database")
        .arg(site.dir.path().join("romeka.db"));
    if args[..2] != ["import", "list"] {
        command.arg("--store").arg(&site.files);
    }
    if args[..2] == ["import", "run"] {
        command.arg("--archive-root").arg(&site.archive);
    }
    command.output().expect("start romeka")
}

fn run_args(plan: &std::path::Path) -> Vec<String> {
    [
        "import",
        "run",
        "--plan",
        plan.to_str().unwrap(),
        "--as",
        ADMIN,
    ]
    .map(String::from)
    .to_vec()
}

fn romeka_run(site: &Site, plan: &std::path::Path, extra: &[&str]) -> Output {
    let mut args = run_args(plan);
    args.extend(extra.iter().map(|arg| (*arg).to_string()));
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    romeka(site, &args)
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

async fn status_of(site: &Site, id: u64) -> ImportStatus {
    site.row(id).await.status
}

async fn imports(site: &Site) -> u64 {
    ImportRepo::new(&site.pool).count().await.unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_dry_run_prints_check_and_leaves_no_import_behind() {
    let site = site().await;
    bhc_006(&site);
    let plan = plan_file(&site, bhc_plan);

    let output = romeka_run(&site, &plan, &["--dry-run"]);

    let text = stdout(&output);
    assert!(output.status.success(), "{text}{}", stderr(&output));
    assert!(text.contains("create item: 9"), "{text}");
    assert!(text.contains("create media: 15"), "{text}");
    assert!(text.contains("new item set: Photographs"), "{text}");
    assert!(text.contains("the import can run"), "{text}");
    assert_eq!(imports(&site).await, 0, "the draft is deleted");
    assert!(site.items().await.is_empty());
    assert!(site.stored_files().is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn run_writes_the_demo_and_list_shows_it() {
    let site = site().await;
    bhc_006(&site);
    let plan = plan_file(&site, bhc_plan);

    let output = romeka_run(&site, &plan, &[]);

    let text = stdout(&output);
    assert!(output.status.success(), "{text}{}", stderr(&output));
    assert!(text.contains("done: 24 of 24 records, 0 failed"), "{text}");
    assert_eq!(site.items().await.len(), 9);
    assert_eq!(site.media().await.len(), 15);
    assert_eq!(site.item_sets().await.len(), 3);
    let row = site.row(1).await;
    assert_eq!(row.status, ImportStatus::Done);
    assert_eq!(row.owner_id.map(|owner| owner.0), Some(1), "the --as user");

    let listed = romeka(&site, &["import", "list"]);
    let text = stdout(&listed);
    assert!(listed.status.success(), "{text}{}", stderr(&listed));
    assert!(text.contains("1\tdone\t"), "{text}");
    assert!(text.contains("24 of 24 records"), "{text}");
    assert!(text.contains("1 imports"), "{text}");
}

#[tokio::test(flavor = "multi_thread")]
async fn undo_previews_then_removes_what_the_run_wrote() {
    let site = site().await;
    bhc_006(&site);
    let plan = plan_file(&site, bhc_plan);
    assert!(romeka_run(&site, &plan, &[]).status.success());

    let preview = romeka(&site, &["import", "undo", "1", "--preview"]);
    let text = stdout(&preview);
    assert!(preview.status.success(), "{text}{}", stderr(&preview));
    assert!(
        text.contains("would remove: 9 items, 15 media, 3 item sets"),
        "{text}"
    );
    assert_eq!(site.items().await.len(), 9, "a preview writes nothing");
    assert_eq!(status_of(&site, 1).await, ImportStatus::Done);

    let undone = romeka(&site, &["import", "undo", "1"]);
    let text = stdout(&undone);
    assert!(undone.status.success(), "{text}{}", stderr(&undone));
    assert!(
        text.contains("removed: 9 items, 15 media, 3 item sets"),
        "{text}"
    );
    assert!(text.contains("import 1 undone"), "{text}");
    assert!(site.items().await.is_empty());
    assert!(site.media().await.is_empty());
    assert!(site.item_sets().await.is_empty());
    assert!(
        site.stored_files()
            .iter()
            .all(|path| !path.starts_with("original"))
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn undo_keeps_an_edited_item_until_force_all() {
    let site = site().await;
    bhc_006(&site);
    let plan = plan_file(&site, bhc_plan);
    assert!(romeka_run(&site, &plan, &[]).status.success());
    // Times are whole seconds: an edit in the import's own second is not seen.
    tokio::time::sleep(Duration::from_millis(1100)).await;
    let edited = site.items().await[0].id().0;
    let patched = site
        .send(
            "PATCH",
            &format!("/api/items/{edited}"),
            Some(json!({ "o:is_public": true })),
        )
        .await;
    assert_eq!(patched.status, 200, "{}", patched.body);

    let kept = romeka(&site, &["import", "undo", "1"]);
    let text = stdout(&kept);
    assert!(kept.status.success(), "{text}{}", stderr(&kept));
    assert!(text.contains(&format!("kept item {edited}")), "{text}");
    assert!(
        text.contains("This was edited after the import wrote it."),
        "{text}"
    );
    assert_eq!(status_of(&site, 1).await, ImportStatus::PartlyUndone);
    assert_eq!(site.items().await.len(), 1);

    let preview = romeka(
        &site,
        &["import", "undo", "1", "--preview", "--force", "all"],
    );
    let text = stdout(&preview);
    assert!(preview.status.success(), "{text}{}", stderr(&preview));
    assert!(text.contains("would remove: 1 items"), "{text}");
    assert_eq!(site.items().await.len(), 1, "a preview writes nothing");

    let forced = romeka(&site, &["import", "undo", "1", "--force", "all"]);
    let text = stdout(&forced);
    assert!(forced.status.success(), "{text}{}", stderr(&forced));
    assert!(text.contains("import 1 undone"), "{text}");
    assert!(site.items().await.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_refused_run_leaves_no_import_and_says_why() {
    let site = site().await;
    let plan = plan_file(&site, bhc_plan);

    let empty = romeka_run(&site, &plan, &[]);
    assert!(!empty.status.success());
    assert!(
        stderr(&empty).contains("nothing to write"),
        "{}",
        stderr(&empty)
    );

    bhc_006(&site);
    let mut args = run_args(&plan);
    let at = args.iter().position(|arg| arg == ADMIN).unwrap();
    args[at] = "nobody@example.org".to_string();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let stranger = romeka(&site, &args);
    assert!(!stranger.status.success());
    assert!(
        stderr(&stranger).contains("no user has the email nobody@example.org"),
        "{}",
        stderr(&stranger)
    );

    assert_eq!(imports(&site).await, 0, "a refusal keeps no draft");
}

#[tokio::test(flavor = "multi_thread")]
async fn the_old_commands_are_gone() {
    let site = site().await;
    let help = romeka(&site, &["import", "--help"]);
    let text = stdout(&help);
    for kept in ["template", "run", "undo", "list"] {
        assert!(text.contains(kept), "{text}");
    }
    for gone in ["csv", "media"] {
        let words: Vec<&str> = text.split_whitespace().collect();
        assert!(!words.contains(&gone), "{gone} is still listed: {text}");
        let output = Command::new(env!("CARGO_BIN_EXE_romeka"))
            .args(["import", gone])
            .output()
            .unwrap();
        assert!(!output.status.success(), "import {gone} still runs");
    }
}

fn delete_plan(plan: &mut Value) {
    plan["grouping"]["pattern"] = json!(BHC_PATTERN);
    plan["action"]["action"] = json!("delete");
    plan["action"]["match"]["item"]["scope"] = json!("all");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_delete_runs_only_with_its_exact_count() {
    let site = site().await;
    bhc_006(&site);
    assert!(
        romeka_run(&site, &plan_file(&site, bhc_plan), &[])
            .status
            .success()
    );
    let plan = plan_file(&site, delete_plan);

    for extra in [&[][..], &["--confirm", "8"][..]] {
        let refused = romeka_run(&site, &plan, extra);
        assert!(!refused.status.success(), "{extra:?} ran");
        assert!(
            stderr(&refused).contains("Pass --confirm 9."),
            "{extra:?}: {}",
            stderr(&refused)
        );
    }
    assert_eq!(
        site.items().await.len(),
        9,
        "a refused delete removes nothing"
    );
    assert_eq!(imports(&site).await, 1, "a refused delete keeps no draft");

    let deleted = romeka_run(&site, &plan, &["--confirm", "9"]);
    let text = stdout(&deleted);
    assert!(deleted.status.success(), "{text}{}", stderr(&deleted));
    assert!(site.items().await.is_empty());
}

/// The whole of stdout, as the one document a `--json` command writes.
fn document(output: &Output) -> Value {
    let text = stdout(output);
    assert!(output.status.success(), "{text}{}", stderr(output));
    serde_json::from_str(&text).unwrap_or_else(|error| panic!("{error}: {text}"))
}

#[tokio::test(flavor = "multi_thread")]
async fn json_writes_one_document_per_command() {
    let site = site().await;
    bhc_006(&site);
    let plan = plan_file(&site, bhc_plan);

    let dry = document(&romeka_run(&site, &plan, &["--dry-run", "--json"]));
    assert_eq!(dry["import"], Value::Null, "{dry}");
    assert_eq!(dry["status"], Value::Null, "{dry}");
    assert_eq!(dry["check"]["canRun"], true, "{dry}");

    let ran = document(&romeka_run(&site, &plan, &["--json"]));
    assert_eq!(ran["import"], 2, "{ran}");
    assert_eq!(ran["status"], "done", "{ran}");
    assert_eq!(ran["summary"]["progress"]["done"], 24, "{ran}");
    assert_eq!(ran["check"]["digest"], dry["check"]["digest"], "{ran}");

    let listed = document(&romeka(&site, &["import", "list", "--json"]));
    assert_eq!(listed["total"], 1, "{listed}");
    assert_eq!(listed["imports"][0]["id"], 2, "{listed}");
    assert_eq!(listed["imports"][0]["status"], "done", "{listed}");
    assert_eq!(listed["imports"][0]["progress"]["total"], 24, "{listed}");

    let preview = document(&romeka(
        &site,
        &["import", "undo", "2", "--preview", "--json"],
    ));
    assert_eq!(preview["import"], 2, "{preview}");
    assert_eq!(preview["preview"]["remove"]["items"], 9, "{preview}");

    let undone = document(&romeka(&site, &["import", "undo", "2", "--json"]));
    assert_eq!(undone["status"], "undone", "{undone}");
    assert_eq!(undone["report"]["removed"]["items"], 9, "{undone}");
}

/// A run under `romeka import run`'s stop seam: the stop resolves as soon as
/// the job runs, as a Ctrl-C at that moment would.
#[tokio::test(flavor = "multi_thread")]
async fn a_stop_during_the_run_ends_it_run_failed() {
    use omeka_cli::commands::import::run::{RunOptions, RunOutcome, run};
    use omeka_cli::commands::import::{Format, Where};

    let site = site().await;
    bhc_006(&site);
    let options = RunOptions {
        at: Where {
            database: site.dir.path().join("romeka.db"),
            store: Some(site.files.clone()),
            archive_root: Some(site.archive.clone()),
        },
        plan: plan_file(&site, bhc_plan),
        actor: ADMIN.to_string(),
        dry_run: false,
        confirm: None,
        format: Format::Text,
    };
    let mut out = Vec::new();

    let outcome = run(&options, std::future::ready(()), &mut out)
        .await
        .unwrap();

    let text = String::from_utf8(out).unwrap();
    assert_eq!(outcome, RunOutcome::Failed, "{text}");
    assert!(text.contains("stop requested"), "{text}");
    assert!(text.contains("the run stopped on request"), "{text}");
    let row = site.row(1).await;
    assert_eq!(row.status, ImportStatus::RunFailed);
    let done = row.summary.as_ref().unwrap()["progress"]["done"].as_u64();
    assert!(done < Some(24), "the run went on after the stop: {text}");
}

/// The binary itself: a SIGINT while the job runs is the stop request.
#[tokio::test(flavor = "multi_thread")]
async fn ctrl_c_stops_the_run() {
    use std::io::{BufRead, BufReader};
    use std::process::Stdio;

    let site = site().await;
    let jpeg = jpeg();
    for n in 0..300 {
        site.write_object(&format!("Artifacts/BHC_006_AF_{n:03}_front.jpg"), &jpeg);
    }
    let plan = plan_file(&site, bhc_plan);
    let mut child = Command::new(env!("CARGO_BIN_EXE_romeka"))
        .args(run_args(&plan))
        .arg("--database")
        .arg(site.dir.path().join("romeka.db"))
        .arg("--store")
        .arg(&site.files)
        .arg("--archive-root")
        .arg(&site.archive)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
    let mut text = String::new();
    for line in lines.by_ref() {
        let line = line.unwrap();
        text.push_str(&line);
        text.push('\n');
        if line.ends_with(" running") {
            break;
        }
    }
    let interrupted = Command::new("kill")
        .args(["-INT", &child.id().to_string()])
        .status()
        .unwrap();
    assert!(interrupted.success());
    for line in lines {
        text.push_str(&line.unwrap());
        text.push('\n');
    }
    let status = child.wait().unwrap();

    assert_eq!(status.code(), Some(1), "{text}");
    assert!(text.contains("the run stopped on request"), "{text}");
    let row = site.row(1).await;
    assert_eq!(row.status, ImportStatus::RunFailed, "{text}");
    let done = row.summary.as_ref().unwrap()["progress"]["done"].as_u64();
    assert!(done < Some(300), "the run went on after the stop: {text}");
}
