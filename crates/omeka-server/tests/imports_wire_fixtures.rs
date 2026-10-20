//! The import wire as the admin's TypeScript client reads it: every route and
//! every preview section is called once, and the answer is the fixture
//! `web/admin/tests/fixtures/import/<name>`. The admin's tests parse these
//! files, so a server shape the client's types do not know fails there.
//!
//! `UPDATE_FIXTURES=1 cargo test -p omeka-server --test imports_wire_fixtures`
//! rewrites the files. Without it the test fails when a committed file is not
//! what the server answers now, or when a file in the folder is not produced.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;
mod imports_fixture;
mod imports_wire;

use std::os::unix::ffi::OsStrExt;

use axum::http::StatusCode;
use imports_fixture::{BHC_PATTERN, Fixture, JPEG, TIFF, bhc_006, fixture};
use imports_wire::recorder::Fixtures;
use imports_wire::scene::{
    REGISTER, add_date, at, checked, existing_item, ok, pairs_answers, refused, rich_plan,
    upload_list,
};
use omeka_db::ids::ImportId;
use omeka_db::repos::ImportRepo;
use omeka_db::rows::ImportEnding;
use omeka_db::with_db;
use serde_json::{Value, json};

#[tokio::test]
async fn the_wire_fixtures_are_what_the_routes_answer() {
    let mut out = Fixtures::new();
    let fixture = fixture().await;
    add_date(&fixture).await;
    existing_item(&fixture).await;
    bhc_006(fixture.archive.path());
    fixture.write(&[
        ("Misc/loose.jpg", JPEG),
        ("register.csv", REGISTER.as_bytes()),
        (".hidden", b"x"),
    ]);
    // Pages of different bytes: identical bytes are copies, and a copy is left out of the group.
    let pages: Vec<Vec<u8>> = (1..=7u8).map(|n| [TIFF, &[n][..]].concat()).collect();
    let bulk: Vec<(String, &[u8])> = pages
        .iter()
        .zip(1..)
        .map(|(bytes, n)| (format!("Bulk/BHC_006_BK_001_p{n}.tif"), bytes.as_slice()))
        .collect();
    let bulk: Vec<(&str, &[u8])> = bulk
        .iter()
        .map(|(path, bytes)| (path.as_str(), *bytes))
        .collect();
    fixture.write(&bulk);
    let root = fixture.archive.path();
    std::fs::write(
        root.join(std::ffi::OsStr::from_bytes(b"Misc/caf\xe9.jpg")),
        JPEG,
    )
    .unwrap();
    std::os::unix::fs::symlink(root.join("Misc"), root.join("Linked")).unwrap();
    // A name the plan's paths refuse. Two names that are one after NFC are left out: which
    // one the walk reports depends on the directory order of the disk.
    std::fs::write(root.join("Misc/back\\slash.jpg"), JPEG).unwrap();

    out.json(
        "archive.json",
        &ok(fixture.call("GET", "/api/romeka/archive", None).await).await,
    );
    out.json(
        "archive-folder.json",
        &ok(fixture
            .call(
                "GET",
                "/api/romeka/archive?root=archive&path=Artifacts",
                None,
            )
            .await)
        .await,
    );

    let draft = fixture.draft().await;
    let id = draft["id"].clone();
    out.json("import-draft.json", &draft);
    let saved = ok(fixture.save(&draft, rich_plan).await).await;
    out.json("import-saved.json", &saved);
    out.json(
        "import-show.json",
        &ok(fixture.call("GET", &at(&id, ""), None).await).await,
    );
    out.json(
        "scan.json",
        &ok(fixture.call("POST", &at(&id, "/scan"), None).await).await,
    );

    let sections: [(&str, Value); 13] = [
        ("entries", json!({ "entries": { "perPage": 50 } })),
        ("sheets", json!({})),
        ("tags", json!({})),
        (
            "records",
            json!({ "records": { "kind": "item", "perPage": 50 } }),
        ),
        ("flags", json!({})),
        ("anomalies", json!({})),
        ("orphans", json!({})),
        ("check", json!({})),
        ("shapes", json!({})),
        ("histogram", json!({})),
        ("identical", json!({ "identical": { "perPage": 1 } })),
        (
            "unpaired",
            json!({ "unpaired": { "rows": { "perPage": 2 }, "files": { "perPage": 2 } } }),
        ),
        ("values", json!({})),
    ];
    for (section, extra) in sections {
        let mut body = json!({ "include": [section], "seed": 2 });
        body.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        let answer = ok(fixture.preview(&id, &body).await).await;
        out.json(&format!("preview-{section}.json"), &answer);
    }
    let media = fixture
        .preview(
            &id,
            &json!({ "include": ["records"], "records": { "kind": "media", "perPage": 2 } }),
        )
        .await;
    out.json("preview-records-media.json", &ok(media).await);
    let words = fixture
        .preview(
            &id,
            &json!({
                "include": ["nameWords"],
                "nameWords": { "path": "Artifacts/BHC_006_AF_001_front.jpg", "perPage": 3 }
            }),
        )
        .await;
    out.json("preview-name-words.json", &ok(words).await);
    let matches = fixture
        .preview(&id, &json!({ "include": ["columnMatches"] }))
        .await;
    out.json("preview-column-matches.json", &ok(matches).await);
    anomaly_answers(&fixture, &mut out, &id).await;

    let template = fixture
        .call(
            "GET",
            &at(&id, "/template.csv?columns=dcterms:title,dcterms:date"),
            None,
        )
        .await;
    assert_eq!(template.status, StatusCode::OK, "{}", template.body);
    out.text("template.csv", &template.body);
    std::fs::create_dir_all(fixture.files.path().join(format!("imports/{id}"))).unwrap();
    std::fs::write(
        fixture.files.path().join(format!("imports/{id}/left.csv")),
        "kind,source,path,record,#reason\nfile,s1,_scratch/notes.txt,,not-media\n",
    )
    .unwrap();
    let left = fixture.call("GET", &at(&id, "/left.csv"), None).await;
    assert_eq!(left.status, StatusCode::OK, "{}", left.body);
    out.text("left.csv", &left.body);

    // The requests the admin's types can build, which the server must take.
    for request in out.read("requests.json").as_array().unwrap() {
        let answer = fixture.preview(&id, request).await;
        assert_eq!(answer.status, StatusCode::OK, "{request}: {}", answer.body);
    }

    conflicts_before_a_run(&fixture, &mut out, &draft, &id).await;
    runs(&fixture, &mut out, &id).await;
    pairs_answers(&fixture, &mut out, &id).await;
    out.finish();
}

/// A copy of the import with its anomalies answered away from the defaults,
/// so the original's revision stays what the other fixtures show. The records
/// preview leaves out the items no row joined (`noRow`).
async fn anomaly_answers(fixture: &Fixture, out: &mut Fixtures, id: &Value) {
    let copy = ok(fixture
        .call(
            "POST",
            "/api/romeka/imports",
            Some(&json!({ "copyFrom": id })),
        )
        .await)
    .await;
    ok(fixture.call("POST", &at(&copy["id"], "/scan"), None).await).await;
    let answers =
        json!({ "identical": "keepAll", "unmatchedFiles": "own", "itemsWithoutRow": "leave" });
    let saved = ok(fixture
        .save(&copy, |plan| plan["anomalies"] = answers.clone())
        .await)
    .await;
    assert_eq!(saved["plan"]["anomalies"], answers);
    out.json("import-saved-anomalies.json", &saved);
    let left = ok(fixture
        .preview(
            &copy["id"],
            &json!({ "include": ["records"], "records": { "kind": "item", "perPage": 50 } }),
        )
        .await)
    .await;
    assert!(
        left.to_string().contains("\"noRow\""),
        "an item without a row is left out: {left}"
    );
    out.json("preview-records-no-row.json", &left);
    let removed = fixture.call("DELETE", &at(&copy["id"], ""), None).await;
    assert_eq!(removed.status, StatusCode::NO_CONTENT);
}

async fn conflicts_before_a_run(fixture: &Fixture, out: &mut Fixtures, draft: &Value, id: &Value) {
    let current = ok(fixture.call("GET", &at(id, ""), None).await).await;
    let stale = fixture
        .call(
            "PUT",
            &at(id, ""),
            Some(&json!({ "plan": current["plan"], "revision": 0 })),
        )
        .await;
    out.json(
        "conflict-stale.json",
        &refused(stale, StatusCode::CONFLICT, Some("stale")).await,
    );

    let v1 = fixture
        .call(
            "PUT",
            &at(id, ""),
            Some(&json!({ "plan": { "setLevel": 1 }, "revision": current["revision"] })),
        )
        .await;
    out.json(
        "conflict-v1-plan.json",
        &refused(v1, StatusCode::CONFLICT, Some("v1Plan")).await,
    );

    let exists = fixture
        .save(&current, |plan| {
            plan["newTypes"] = json!([{ "label": "Picture", "from": "mapping", "class": null }]);
        })
        .await;
    out.json(
        "conflict-type-exists.json",
        &refused(exists, StatusCode::CONFLICT, Some("typeExists")).await,
    );

    let bad = fixture
        .save(&current, |plan| plan["surprise"] = json!(true))
        .await;
    out.json(
        "error-plan-refused.json",
        &refused(bad, StatusCode::BAD_REQUEST, None).await,
    );
    let missing = fixture.call("GET", "/api/romeka/imports/9999", None).await;
    out.json(
        "error-not-found.json",
        &refused(missing, StatusCode::NOT_FOUND, None).await,
    );

    let digest = checked(fixture, id).await["digest"].clone();
    let wrong = fixture
        .call(
            "POST",
            &at(id, "/run"),
            Some(&json!({ "digest": "0".repeat(64) })),
        )
        .await;
    out.json(
        "conflict-changed.json",
        &refused(wrong, StatusCode::CONFLICT, Some("changed")).await,
    );

    let mut unknown_term = current["plan"].clone();
    unknown_term["slots"]["item"]["dcterms:nothing"] =
        json!({ "sources": [{ "from": "constant", "value": "x" }], "combine": "first" });
    let unknown = fixture
        .preview(id, &json!({ "include": ["check"], "plan": unknown_term }))
        .await;
    let blocked_digest = ok(unknown).await["check"]["digest"].clone();
    let blocked = fixture
        .save(&current, |plan| {
            plan["slots"]["item"]["dcterms:nothing"] =
                json!({ "sources": [{ "from": "constant", "value": "x" }], "combine": "first" });
        })
        .await;
    let blocked_draft = ok(blocked).await;
    out.json(
        "preview-check-blocked.json",
        &ok(fixture.preview(id, &json!({ "include": ["check"] })).await).await,
    );
    let run = fixture
        .call(
            "POST",
            &at(id, "/run"),
            Some(&json!({ "digest": blocked_digest })),
        )
        .await;
    out.json(
        "conflict-blocked.json",
        &refused(run, StatusCode::CONFLICT, Some("blocked")).await,
    );
    let restored = fixture
        .save(&blocked_draft, |plan| {
            plan["slots"]["item"]
                .as_object_mut()
                .unwrap()
                .remove("dcterms:nothing");
        })
        .await;
    ok(restored).await;
    assert_eq!(
        checked(fixture, id).await["digest"],
        digest,
        "the plan is back to the checked one"
    );

    // A new draft makes one item per name; with no pattern it makes nothing.
    let made = fixture.draft().await;
    let empty = ok(fixture
        .save(&made, |plan| plan["grouping"]["pattern"] = json!(null))
        .await)
    .await;
    ok(fixture.call("POST", &at(&empty["id"], "/scan"), None).await).await;
    let nothing = fixture
        .call(
            "POST",
            &at(&empty["id"], "/run"),
            Some(&json!({ "digest": checked(fixture, &empty["id"]).await["digest"] })),
        )
        .await;
    out.json(
        "conflict-nothing.json",
        &refused(nothing, StatusCode::CONFLICT, Some("nothing")).await,
    );

    let delete = fixture
        .save(&empty, |plan| {
            plan["grouping"]["pattern"] = json!(BHC_PATTERN);
            plan["action"]["action"] = json!("delete");
            plan["action"]["match"]["item"] =
                json!({ "key": "dcterms:identifier", "scope": "all" });
        })
        .await;
    ok(delete).await;
    ok(fixture.call("POST", &at(&empty["id"], "/scan"), None).await).await;
    let delete_check = checked(fixture, &empty["id"]).await;
    assert_eq!(delete_check["deletes"], 1, "{delete_check}");
    let confirm = fixture
        .call(
            "POST",
            &at(&empty["id"], "/run"),
            Some(&json!({ "digest": delete_check["digest"] })),
        )
        .await;
    out.json(
        "conflict-confirm.json",
        &refused(confirm, StatusCode::CONFLICT, Some("confirm")).await,
    );
    out.json(
        "preview-check-delete.json",
        &ok(fixture
            .preview(&empty["id"], &json!({ "include": ["check"] }))
            .await)
        .await,
    );
    assert!(draft["id"] != empty["id"]);

    let not_running = fixture.call("POST", &at(&empty["id"], "/stop"), None).await;
    out.json(
        "conflict-not-running.json",
        &refused(not_running, StatusCode::CONFLICT, Some("notRunning")).await,
    );
    let cannot_undo = fixture
        .call("POST", &at(&empty["id"], "/undo"), Some(&json!({})))
        .await;
    out.json(
        "conflict-cannot-undo.json",
        &refused(cannot_undo, StatusCode::CONFLICT, Some("cannotUndo")).await,
    );

    let list = b"Identifier,Files,Title [dcterms:title],Date [dcterms:date]\nBHC_006_AF_001,,Brass lamp,1950\n";
    let listed = fixture.draft().await;
    let added = upload_list(fixture, &listed["id"], "list.csv", list).await;
    assert_eq!(added.status, StatusCode::CREATED, "{}", added.body);
    out.json("import-sheet-added.json", &added.json());

    // Beside the CSV list, so no import id moves in the other fixtures.
    let book = upload_list(fixture, &listed["id"], "register.xlsx", &workbook()).await;
    assert_eq!(book.status, StatusCode::CREATED, "{}", book.body);
    let view = ok(fixture
        .preview(&listed["id"], &json!({ "include": ["sheets"] }))
        .await)
    .await;
    assert_eq!(
        view["sheets"][1]["read"]["workbook"]["sheets"],
        json!(["Register", "Notes"])
    );
    out.json("preview-sheets-workbook.json", &view);
    for gone in [&empty, &listed] {
        let removed = fixture.call("DELETE", &at(&gone["id"], ""), None).await;
        assert_eq!(removed.status, StatusCode::NO_CONTENT);
    }
}

async fn runs(fixture: &Fixture, out: &mut Fixtures, id: &Value) {
    let digest = checked(fixture, id).await["digest"].clone();
    let copy = ok(fixture
        .call(
            "POST",
            "/api/romeka/imports",
            Some(&json!({ "copyFrom": id })),
        )
        .await)
    .await;
    out.json("import-copied.json", &copy);

    let started = ok(fixture
        .call("POST", &at(id, "/run"), Some(&json!({ "digest": digest })))
        .await)
    .await;
    out.json("run.json", &started);

    ok(fixture.call("POST", &at(&copy["id"], "/scan"), None).await).await;
    let busy_digest = checked(fixture, &copy["id"]).await["digest"].clone();
    let busy = fixture
        .call(
            "POST",
            &at(&copy["id"], "/run"),
            Some(&json!({ "digest": busy_digest })),
        )
        .await;
    out.json(
        "conflict-busy.json",
        &refused(busy, StatusCode::CONFLICT, Some("busy")).await,
    );

    let notdraft = fixture
        .call("PUT", &at(id, ""), Some(&json!({ "plan": ok(fixture.call("GET", &at(id, ""), None).await).await["plan"], "revision": 1 })))
        .await;
    out.json(
        "conflict-not-draft.json",
        &refused(notdraft, StatusCode::CONFLICT, Some("notDraft")).await,
    );

    let import = ImportId(id.as_u64().unwrap());
    with_db!(&fixture.pool, |db| ImportRepo::new(db)
        .set_summary(
            import,
            &json!({ "progress": { "done": 3, "total": 9 }, "failures": [], "stopped": false })
        )
        .await)
    .unwrap();
    out.json(
        "import-running.json",
        &ok(fixture.call("GET", &at(id, ""), None).await).await,
    );
    out.json(
        "stop.json",
        &ok(fixture.call("POST", &at(id, "/stop"), None).await).await,
    );
    out.json(
        "import-stopping.json",
        &ok(fixture.call("GET", &at(id, ""), None).await).await,
    );

    let failures = json!([{ "record": "group:BHC_006_AF_001", "key": "importFileUnreadable", "detail": "The file could not be read." }]);
    with_db!(&fixture.pool, |db| ImportRepo::new(db)
        .set_summary(
            import,
            &json!({ "progress": { "done": 8, "total": 9 }, "failures": failures, "stopped": true })
        )
        .await)
    .unwrap();
    with_db!(&fixture.pool, |db| ImportRepo::new(db)
        .finish(import, ImportEnding::RunFailed)
        .await)
    .unwrap();
    out.json(
        "import-run-failed.json",
        &ok(fixture.call("GET", &at(id, ""), None).await).await,
    );

    let done = ok(fixture
        .call(
            "POST",
            "/api/romeka/imports",
            Some(&json!({ "copyFrom": id })),
        )
        .await)
    .await;
    ok(fixture.call("POST", &at(&done["id"], "/scan"), None).await).await;
    let done_digest = checked(fixture, &done["id"]).await["digest"].clone();
    ok(fixture
        .call(
            "POST",
            &at(&done["id"], "/run"),
            Some(&json!({ "digest": done_digest })),
        )
        .await)
    .await;
    let done_id = ImportId(done["id"].as_u64().unwrap());
    with_db!(&fixture.pool, |db| ImportRepo::new(db)
        .set_summary(
            done_id,
            &json!({ "progress": { "done": 9, "total": 9 }, "failures": [], "stopped": false })
        )
        .await)
    .unwrap();
    with_db!(&fixture.pool, |db| ImportRepo::new(db)
        .finish(done_id, ImportEnding::Done)
        .await)
    .unwrap();
    out.json(
        "import-done.json",
        &ok(fixture.call("GET", &at(&done["id"], ""), None).await).await,
    );
    let again = fixture
        .call(
            "POST",
            &at(&done["id"], "/run"),
            Some(&json!({ "digest": done_digest })),
        )
        .await;
    out.json(
        "conflict-cannot-run.json",
        &refused(again, StatusCode::CONFLICT, Some("cannotRun")).await,
    );

    // Keyed in the query string, as a client without a session asks.
    out.json(
        "undo-preview.json",
        &ok(fixture
            .call("GET", &at(&done["id"], "/undo/preview"), None)
            .await)
        .await,
    );
    out.json(
        "undo.json",
        &ok(fixture
            .call(
                "POST",
                &at(&done["id"], "/undo"),
                Some(&json!({ "force": "all" })),
            )
            .await)
        .await,
    );
    out.json(
        "import-undoing.json",
        &ok(fixture.call("GET", &at(&done["id"], ""), None).await).await,
    );
    out.json(
        "history.json",
        &ok(fixture
            .call("GET", "/api/romeka/imports?perPage=10", None)
            .await)
        .await,
    );
}

/// A register on its first sheet and a notes sheet after it.
fn workbook() -> Vec<u8> {
    let mut book = rust_xlsxwriter::Workbook::new();
    // The file's hash is in the fixture, so its creation time is fixed.
    let created = rust_xlsxwriter::ExcelDateTime::from_ymd(2026, 10, 21).unwrap();
    book.set_properties(&rust_xlsxwriter::DocProperties::new().set_creation_datetime(&created));
    let register = book.add_worksheet().set_name("Register").unwrap();
    for (column, header) in ["Identifier", "Files", "Title [dcterms:title]"]
        .into_iter()
        .enumerate()
    {
        register.write(0, column as u16, header).unwrap();
    }
    register.write(1, 0, "BHC_006_AF_001").unwrap();
    register.write(1, 2, "Brass lamp").unwrap();
    book.add_worksheet().set_name("Notes").unwrap();
    book.save_to_buffer().unwrap()
}
