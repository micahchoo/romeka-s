//! The pairs list round trip and the entries filters the list screens count
//! with, through the routes (`docs/import-v2.md`, "Pairs list round trip",
//! "Filters").

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;
mod imports_fixture;

use axum::http::StatusCode;
use imports_fixture::{BHC_PATTERN, Fixture, JPEG, bhc_006, fixture};
use serde_json::{Value, json};

/// The S1 archive, scanned, with two files of one name in two folders that
/// both fit the pattern, two others of one name that do not, and a list of
/// four rows none of which names an item: four unpaired rows.
async fn four_unpaired_rows(fixture: &Fixture) -> Value {
    fixture.write_objects(&[("Dup/a/same.jpg", JPEG), ("Dup/b/same.jpg", JPEG)]);
    fixture.write(&[("Extra/BHC_006_AF_001_front.jpg", &[JPEG, b"x"].concat())]);
    bhc_006(fixture.archive.path());
    let draft = fixture.draft().await;
    let saved = fixture
        .save(&draft, |plan| {
            plan["grouping"]["pattern"] = json!(BHC_PATTERN)
        })
        .await;
    assert_eq!(saved.status, StatusCode::OK, "{}", saved.body);
    let id = draft["id"].clone();
    let scanned = fixture
        .call("POST", &format!("/api/romeka/imports/{id}/scan"), None)
        .await;
    assert_eq!(scanned.status, StatusCode::OK, "{}", scanned.body);
    let list = "Identifier,Title [dcterms:title]\r\n\
                ROW-1,Brass bowl\r\n\
                ROW-2,=1+1\r\n\
                ROW-3,Third\r\n\
                ROW-4,Fourth\r\n";
    let uploaded = fixture.upload(&id, "list.csv", list.as_bytes()).await;
    assert_eq!(uploaded.status, StatusCode::CREATED, "{}", uploaded.body);
    id
}

async fn pairs_csv(fixture: &Fixture, id: &Value) -> String {
    let answer = fixture
        .call("GET", &format!("/api/romeka/imports/{id}/pairs.csv"), None)
        .await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    assert_eq!(answer.headers["content-type"], "text/csv; charset=utf-8");
    answer.body
}

#[tokio::test]
async fn the_pairs_list_goes_down_with_the_unpaired_rows_and_comes_back_as_pair_decisions() {
    let fixture = fixture().await;
    let id = four_unpaired_rows(&fixture).await;

    let down = pairs_csv(&fixture, &id).await;
    let lines: Vec<&str> = down.split("\r\n").collect();
    assert_eq!(
        lines[0],
        "\u{feff}Row [sheet:key],Identifier,Title [dcterms:title],Files"
    );
    assert_eq!(lines[1], "list-1:ROW-1#1,ROW-1,Brass bowl,");
    assert_eq!(
        lines[2], "list-1:ROW-2#1,ROW-2,'=1+1,",
        "a formula is quoted so a spreadsheet does not run it"
    );
    assert_eq!(lines.len(), 1 + 4 + 1, "a line per unpaired row");

    let filled = [
        lines[0].to_string(),
        "list-1:ROW-1#1,ROW-1,Brass bowl,Artifacts/BHC_006_AF_001_front.jpg".into(),
        "list-1:ROW-2#1,ROW-2,'=1+1,nothing.jpg".into(),
        "list-1:ROW-3#1,ROW-3,Third,Artifacts/BHC_006_AF_002_front.jpg".into(),
        "list-1:ROW-4#1,ROW-4,Fourth,BHC_006_AF_001_front.jpg".into(),
        "list-1:ROW-4#1,ROW-4,Fourth,same.jpg".into(),
        "list-9:ROW-9#1,,,BHC_006_AF_003_front.jpg".into(),
        String::new(),
    ]
    .join("\r\n");
    let before = fixture.row(&id).await.revision;
    let up = fixture
        .upload_to(&id, "pairs", "pairs.csv", filled.as_bytes())
        .await;
    assert_eq!(up.status, StatusCode::OK, "{}", up.body);
    let answer = up.json();
    assert_eq!(answer["added"], 2);
    assert_eq!(answer["already"], 0);
    assert_eq!(answer["unmatchedTotal"], 4);
    assert_eq!(
        answer["unmatched"],
        json!([
            { "row": "list-1:ROW-2#1", "text": "nothing.jpg", "why": "noFile" },
            { "row": "list-1:ROW-4#1", "text": "BHC_006_AF_001_front.jpg", "why": "manyFiles" },
            { "row": "list-1:ROW-4#1", "text": "same.jpg", "why": "noItem" },
            { "row": "list-9:ROW-9#1", "text": "BHC_006_AF_003_front.jpg", "why": "noRow" },
        ])
    );
    let plan = &answer["import"]["plan"];
    assert_eq!(answer["import"]["revision"], before + 1);
    assert_eq!(
        plan["decisions"],
        json!([
            { "kind": "pair",
              "row": { "sheet": "list-1", "key": "ROW-1#1" },
              "file": { "source": "s1", "path": "Artifacts/BHC_006_AF_001_front.jpg" } },
            { "kind": "pair",
              "row": { "sheet": "list-1", "key": "ROW-3#1" },
              "file": { "source": "s1", "path": "Artifacts/BHC_006_AF_002_front.jpg" } },
        ])
    );

    let preview = fixture
        .preview(&id, &json!({ "include": ["unpaired"] }))
        .await;
    assert_eq!(
        preview.json()["unpaired"]["rows"]["total"],
        2,
        "two rows paired"
    );

    let again = fixture
        .upload_to(&id, "pairs", "pairs.csv", filled.as_bytes())
        .await;
    assert_eq!(again.status, StatusCode::OK, "{}", again.body);
    assert_eq!(again.json()["added"], 0);
    assert_eq!(
        again.json()["already"],
        2,
        "the same list adds nothing twice"
    );
    assert_eq!(
        again.json()["import"]["revision"],
        before + 1,
        "nothing to save"
    );
}

#[tokio::test]
async fn a_pairs_list_without_its_columns_or_of_a_wrong_type_is_refused() {
    let fixture = fixture().await;
    let id = four_unpaired_rows(&fixture).await;
    let before = fixture.row(&id).await.revision;
    let no_row = fixture
        .upload_to(
            &id,
            "pairs",
            "pairs.csv",
            b"Identifier,Files\r\nROW-1,x.jpg\r\n",
        )
        .await;
    assert_eq!(no_row.status, StatusCode::BAD_REQUEST, "{}", no_row.body);
    let no_files = fixture
        .upload_to(
            &id,
            "pairs",
            "pairs.csv",
            b"Row [sheet:key]\r\nlist-1:ROW-1#1\r\n",
        )
        .await;
    assert_eq!(
        no_files.status,
        StatusCode::BAD_REQUEST,
        "{}",
        no_files.body
    );
    let pdf = fixture
        .upload_to(&id, "pairs", "pairs.pdf", b"%PDF-1.4")
        .await;
    assert_eq!(pdf.status, StatusCode::BAD_REQUEST, "{}", pdf.body);
    let json_body = fixture
        .call(
            "POST",
            &format!("/api/romeka/imports/{id}/pairs"),
            Some(&json!({})),
        )
        .await;
    assert_eq!(
        json_body.status,
        StatusCode::BAD_REQUEST,
        "{}",
        json_body.body
    );
    let none = fixture
        .call("GET", "/api/romeka/imports/999999/pairs.csv", None)
        .await;
    assert_eq!(none.status, StatusCode::NOT_FOUND, "{}", none.body);
    assert_eq!(
        fixture.row(&id).await.revision,
        before,
        "a refused list saves nothing"
    );
}

#[tokio::test]
async fn entries_are_counted_by_kind_and_by_sheet() {
    let fixture = fixture().await;
    let id = four_unpaired_rows(&fixture).await;
    let entries = |query: Value| {
        let fixture = &fixture;
        let id = &id;
        async move {
            let answer = fixture
                .preview(id, &json!({ "include": ["entries"], "entries": query }))
                .await;
            assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
            answer.json()["entries"].clone()
        }
    };
    let rows = entries(json!({ "sheet": "list-1", "paired": false, "perPage": 2 })).await;
    assert_eq!(rows["total"], 4, "every row of the list has no file");
    assert_eq!(rows["items"].as_array().unwrap().len(), 2);
    assert_eq!(entries(json!({ "sheet": "list-9" })).await["total"], 0);

    let files = entries(json!({ "kind": "file", "perPage": 1 })).await;
    assert_eq!(
        files["total"],
        12 + 3 + 2 + 2 + 1,
        "every file of the archive"
    );
    let only_rows = entries(json!({ "kind": "row" })).await;
    assert_eq!(only_rows["total"], 4);
}
