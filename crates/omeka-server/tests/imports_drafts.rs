//! The draft routes of import v2: who may call them, making a draft from
//! sources or a copy, saving its plan by revision (scenario S20), deleting
//! it, the history, and the archive folder listing.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;
mod imports_fixture;

use axum::http::StatusCode;
use imports_fixture::{EDITOR, JPEG, SECOND_ADMIN, fixture};
use omeka_db::ids::ImportId;
use omeka_db::repos::{ImportRepo, TemplateRepo};
use omeka_db::rows::{NewImport, NewTemplate};
use serde_json::json;

/// Every import route, with a method and a body the route would accept.
const ROUTES: &[(&str, &str)] = &[
    ("GET", "/api/romeka/archive"),
    ("GET", "/api/romeka/imports"),
    ("POST", "/api/romeka/imports"),
    ("GET", "/api/romeka/imports/1"),
    ("PUT", "/api/romeka/imports/1"),
    ("DELETE", "/api/romeka/imports/1"),
    ("POST", "/api/romeka/imports/1/scan"),
    ("POST", "/api/romeka/imports/1/preview"),
    ("POST", "/api/romeka/imports/1/run"),
    ("POST", "/api/romeka/imports/1/stop"),
    ("POST", "/api/romeka/imports/1/undo"),
    ("GET", "/api/romeka/imports/1/left.csv"),
];

#[tokio::test]
async fn every_route_needs_a_signed_in_admin() {
    let fixture = fixture().await;
    fixture.draft().await;
    for (method, uri) in ROUTES {
        let anonymous = fixture.anonymous(method, uri).await;
        assert_eq!(anonymous.status, StatusCode::UNAUTHORIZED, "{method} {uri}");
        let editor = fixture.call_as(EDITOR, method, uri, Some(&json!({}))).await;
        assert_eq!(editor.status, StatusCode::FORBIDDEN, "{method} {uri}");
        assert_eq!(
            editor.json()["errors"]["error"],
            "Only a global or site administrator may import an archive."
        );
    }
}

#[tokio::test]
async fn a_draft_starts_with_the_default_plan_on_its_sources() {
    let fixture = fixture().await;
    std::fs::create_dir(fixture.archive.path().join("Box 1")).unwrap();
    let made = fixture
        .call(
            "POST",
            "/api/romeka/imports",
            Some(&json!({ "sources": [{ "kind": "folder", "id": "s1", "root": "archive", "path": "Box 1" }] })),
        )
        .await;
    assert_eq!(made.status, StatusCode::CREATED, "{}", made.body);
    let draft = made.json();
    assert_eq!(draft["status"], "draft");
    assert_eq!(draft["revision"], 0);
    assert_eq!(draft["plan"]["version"], 2);
    assert_eq!(draft["plan"]["sources"][0]["path"], "Box 1");
    assert_eq!(draft["plan"]["defaults"]["visibility"], "private");
    assert_eq!(draft["sources"], draft["plan"]["sources"]);

    let shown = fixture
        .call("GET", &format!("/api/romeka/imports/{}", draft["id"]), None)
        .await;
    assert_eq!(shown.status, StatusCode::OK);
    assert_eq!(shown.json()["plan"], draft["plan"]);
}

#[tokio::test]
async fn a_draft_on_a_missing_root_or_folder_is_refused() {
    let fixture = fixture().await;
    let create = |root: &str, path: &str| json!({ "sources": [{ "kind": "folder", "id": "s1", "root": root, "path": path }] });
    let unknown = fixture
        .call("POST", "/api/romeka/imports", Some(&create("films", "")))
        .await;
    assert_eq!(unknown.status, StatusCode::NOT_FOUND);
    assert_eq!(
        unknown.json()["errors"]["error"],
        "There is no archive folder named \"films\"."
    );
    let missing = fixture
        .call(
            "POST",
            "/api/romeka/imports",
            Some(&create("archive", "Box 9")),
        )
        .await;
    assert_eq!(missing.status, StatusCode::NOT_FOUND);
    let escaping = fixture
        .call(
            "POST",
            "/api/romeka/imports",
            Some(&create("archive", "../x")),
        )
        .await;
    assert_eq!(
        escaping.status,
        StatusCode::BAD_REQUEST,
        "{}",
        escaping.body
    );
    let neither = fixture
        .call(
            "POST",
            "/api/romeka/imports",
            Some(&json!({ "folder": "x" })),
        )
        .await;
    assert_eq!(neither.status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn a_save_carries_the_revision_and_a_second_admin_on_an_old_one_gets_stale() {
    let fixture = fixture().await;
    let draft = fixture.draft().await;
    let uri = format!("/api/romeka/imports/{}", draft["id"]);
    let mut plan = draft["plan"].clone();
    plan["grouping"]["pattern"] = json!(r"^(?<identifier>.+)\.jpg$");

    let first = fixture
        .call("PUT", &uri, Some(&json!({ "plan": plan, "revision": 0 })))
        .await;
    assert_eq!(first.status, StatusCode::OK, "{}", first.body);
    assert_eq!(first.json()["revision"], 1);
    assert_eq!(
        first.json()["plan"]["grouping"]["pattern"],
        plan["grouping"]["pattern"]
    );

    let second = fixture
        .call_as(
            SECOND_ADMIN,
            "PUT",
            &uri,
            Some(&json!({ "plan": plan, "revision": 0 })),
        )
        .await;
    assert_eq!(second.status, StatusCode::CONFLICT);
    let refused = second.json();
    assert_eq!(refused["code"], "stale");
    assert_eq!(refused["revision"], 1);
    assert_eq!(
        refused["errors"]["error"],
        "Someone else saved this import after you opened it. Reload it to see their changes."
    );
}

#[tokio::test]
async fn a_save_refuses_a_v1_plan_a_bad_plan_and_a_type_that_exists() {
    let fixture = fixture().await;
    let draft = fixture.draft().await;
    let uri = format!("/api/romeka/imports/{}", draft["id"]);
    let v1 = fixture
        .call(
            "PUT",
            &uri,
            Some(&json!({ "plan": { "setLevel": 1 }, "revision": 0 })),
        )
        .await;
    assert_eq!(v1.status, StatusCode::CONFLICT);
    assert_eq!(v1.json()["code"], "v1Plan");

    let bad = fixture
        .save(&draft, |plan| plan["surprise"] = json!(true))
        .await;
    assert_eq!(bad.status, StatusCode::BAD_REQUEST);
    assert!(
        bad.json()["errors"]["error"]
            .as_str()
            .unwrap()
            .starts_with("The import's settings cannot be saved: ")
    );

    let keys = fixture
        .save(&draft, |plan| {
            plan["slots"]["item"]["template"] = json!({
                "sources": [{ "from": "constant", "value": "AF" }],
                "combine": "first",
                "map": { "AF": "Artifact", "af": "Photograph" }
            });
        })
        .await;
    assert_eq!(
        keys.status,
        StatusCode::BAD_REQUEST,
        "two map keys read as one"
    );

    TemplateRepo::new(&fixture.pool)
        .create(&NewTemplate {
            owner_id: None,
            resource_class_id: None,
            title_property_id: None,
            description_property_id: None,
            label: "Letter".into(),
        })
        .await
        .unwrap();
    let exists = fixture
        .save(&draft, |plan| {
            plan["newTypes"] =
                json!([{ "label": " letter ", "from": { "starter": "letter" }, "class": null }]);
        })
        .await;
    assert_eq!(exists.status, StatusCode::CONFLICT);
    assert_eq!(exists.json()["code"], "typeExists");
    assert_eq!(fixture.row(&draft["id"]).await.revision, 0, "nothing saved");
}

#[tokio::test]
async fn a_started_import_keeps_its_plan_and_cannot_be_deleted() {
    let fixture = fixture().await;
    let draft = fixture.draft().await;
    let id = ImportId(draft["id"].as_u64().unwrap());
    ImportRepo::new(&fixture.pool)
        .start_run(id, 0)
        .await
        .unwrap();

    let saved = fixture.save(&draft, |_| {}).await;
    assert_eq!(saved.status, StatusCode::CONFLICT);
    assert_eq!(saved.json()["code"], "notDraft");
    assert_eq!(saved.json()["status"], "running");

    let deleted = fixture
        .call("DELETE", &format!("/api/romeka/imports/{}", id.0), None)
        .await;
    assert_eq!(deleted.status, StatusCode::CONFLICT);
    assert_eq!(
        deleted.json()["errors"]["error"],
        "Only an import that is not added yet can be deleted. An import that has run stays in Past imports."
    );
}

#[tokio::test]
async fn a_draft_is_deleted_once() {
    let fixture = fixture().await;
    let draft = fixture.draft().await;
    let uri = format!("/api/romeka/imports/{}", draft["id"]);
    assert_eq!(
        fixture.call("DELETE", &uri, None).await.status,
        StatusCode::NO_CONTENT
    );
    let again = fixture.call("DELETE", &uri, None).await;
    assert_eq!(again.status, StatusCode::NOT_FOUND);
    assert_eq!(
        again.json()["errors"]["error"],
        format!("No import has the id {}.", draft["id"])
    );
    assert_eq!(
        fixture.call("GET", &uri, None).await.status,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn a_copy_keeps_the_plan_with_its_decisions() {
    let fixture = fixture().await;
    let draft = fixture.draft().await;
    let decided = fixture
        .save(&draft, |plan| {
            plan["decisions"] = json!([{ "kind": "leave",
                "entry": { "file": { "source": "s1", "path": "a.jpg" } } }]);
        })
        .await
        .json();
    let copy = fixture
        .call(
            "POST",
            "/api/romeka/imports",
            Some(&json!({ "copyFrom": draft["id"] })),
        )
        .await;
    assert_eq!(copy.status, StatusCode::CREATED, "{}", copy.body);
    let copy = copy.json();
    assert_ne!(copy["id"], draft["id"]);
    assert_eq!(copy["plan"], decided["plan"]);
    assert_eq!(copy["copiedFrom"], draft["id"]);
    assert_eq!(copy["revision"], 0);

    let v1 = ImportRepo::new(&fixture.pool)
        .create_draft(&NewImport {
            owner_id: None,
            sources: json!([]),
            plan: json!({ "setLevel": 1, "folder": "x" }),
            copied_from: None,
        })
        .await
        .unwrap();
    let refused = fixture
        .call(
            "POST",
            "/api/romeka/imports",
            Some(&json!({ "copyFrom": v1.id.0 })),
        )
        .await;
    assert_eq!(refused.status, StatusCode::CONFLICT);
    assert_eq!(refused.json()["code"], "v1Plan");
    let missing = fixture
        .call(
            "POST",
            "/api/romeka/imports",
            Some(&json!({ "copyFrom": 999 })),
        )
        .await;
    assert_eq!(missing.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn the_history_is_newest_first_and_paged() {
    let fixture = fixture().await;
    let first = fixture.draft().await;
    let second = fixture.draft().await;
    let third = fixture.draft().await;
    let page = fixture
        .call("GET", "/api/romeka/imports?page=1&perPage=2", None)
        .await
        .json();
    assert_eq!(page["total"], 3);
    assert_eq!(page["items"][0]["id"], third["id"]);
    assert_eq!(page["items"][1]["id"], second["id"]);
    assert!(
        page["items"][0].get("plan").is_none(),
        "the list leaves plans out"
    );
    let next = fixture
        .call("GET", "/api/romeka/imports?page=2&perPage=2", None)
        .await
        .json();
    assert_eq!(next["items"][0]["id"], first["id"]);
}

#[tokio::test]
async fn the_archive_lists_one_folder_of_a_named_root() {
    let fixture = fixture().await;
    fixture.write(&[
        ("Box 1/a.jpg", JPEG),
        ("notes.txt", b"x"),
        (".DS_Store", b"x"),
    ]);
    let top = fixture.call("GET", "/api/romeka/archive", None).await;
    assert_eq!(top.status, StatusCode::OK, "{}", top.body);
    let top = top.json();
    assert_eq!(top["roots"], json!(["archive"]));
    assert_eq!(top["root"], "archive");
    assert_eq!(
        top["entries"],
        json!([
            { "kind": "folder", "name": "Box 1", "path": "Box 1", "size": null },
            { "kind": "file", "name": "notes.txt", "path": "notes.txt", "size": 1 }
        ])
    );
    let inside = fixture
        .call("GET", "/api/romeka/archive?root=archive&path=Box%201", None)
        .await
        .json();
    assert_eq!(inside["entries"][0]["path"], "Box 1/a.jpg");
    let outside = fixture
        .call("GET", "/api/romeka/archive?path=../", None)
        .await;
    assert_eq!(outside.status, StatusCode::BAD_REQUEST);
    let unknown = fixture
        .call("GET", "/api/romeka/archive?root=films", None)
        .await;
    assert_eq!(unknown.status, StatusCode::NOT_FOUND);
}
