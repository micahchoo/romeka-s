//! The scene the fixtures are taken from: the install, the archive folder, the plan
//! with a decision of each kind, and the small helpers the calls share.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::{Value, json};

use crate::common;
use crate::imports_fixture::{ADMIN, BHC_PATTERN, Fixture};
use omeka_db::repos::{ItemRepo, PropertyRepo, ValueRepo, VocabularyRepo};
use omeka_db::rows::{NewItem, NewProperty, NewValue};

pub const REGISTER: &str = "Id,Title,Date,Notes,Notes\nBHC_006_AF_001,Brass lamp,1950,a,b\nBHC_006_AF_002,Copper pot,circa 1950s?,,\nBHC_006_PH_101,Portrait,1951-03,,\nBHC_006_ZZ_999,Ghost row,1960,,\nBHC_006_AF_003,Short row\n";

/// the create plan's match scope, and a target for the delete plan.
pub async fn existing_item(fixture: &Fixture) {
    let dcterms = VocabularyRepo::new(&fixture.pool)
        .get_by_prefix("dcterms")
        .await
        .unwrap()
        .unwrap();
    let identifier = PropertyRepo::new(&fixture.pool)
        .create(&NewProperty {
            owner_id: None,
            vocabulary_id: dcterms.id,
            local_name: "identifier".into(),
            label: "Identifier".into(),
            comment: None,
        })
        .await
        .unwrap();
    let item = ItemRepo::new(&fixture.pool)
        .create(&NewItem::default())
        .await
        .unwrap();
    ValueRepo::new(&fixture.pool)
        .create(&NewValue {
            resource_id: item.resource.id,
            property_id: identifier.id,
            value_resource_id: None,
            value_annotation_id: None,
            r#type: "literal".into(),
            lang: None,
            value: Some("BHC_006_PH_101".into()),
            uri: None,
            is_public: true,
        })
        .await
        .unwrap();
}

pub async fn add_date(fixture: &Fixture) {
    let dcterms = VocabularyRepo::new(&fixture.pool)
        .get_by_prefix("dcterms")
        .await
        .unwrap()
        .unwrap();
    PropertyRepo::new(&fixture.pool)
        .create(&NewProperty {
            owner_id: None,
            vocabulary_id: dcterms.id,
            local_name: "date".to_string(),
            label: "Date".to_string(),
            comment: None,
        })
        .await
        .unwrap();
}

pub fn column(sheet: &str, index: u32, header: &str) -> Value {
    json!({ "from": "column", "sheet": sheet, "column": { "index": index, "header": header } })
}

pub fn file(path: &str) -> Value {
    json!({ "file": { "source": "s1", "path": path } })
}

/// Every plan member the wire has, with a decision of each kind.
pub fn rich_plan(plan: &mut Value) {
    plan["grouping"]["pattern"] = json!(BHC_PATTERN);
    let lost = json!({
        "id": "lost", "source": "s1", "file": "lost.csv", "hash": null,
        "read": { "encoding": "windows-1252", "delimiter": ",", "quote": "'", "headerLine": 2, "trim": false },
        "rowKind": "media", "join": null, "itemKey": null, "columns": []
    });
    plan["sheets"] = json!([{
        "id": "register", "source": "s1", "file": "register.csv", "hash": "stale-hash",
        "read": { "encoding": "auto", "delimiter": "auto", "quote": "\"", "headerLine": 1, "trim": true },
        "rowKind": "item",
        "join": { "on": "identifier", "column": { "index": 0, "header": "Id" },
                  "normalize": { "caseFold": true, "leadingZeros": false, "separators": false } },
        "itemKey": null,
        "columns": [
            { "column": { "index": 1, "header": "Title" }, "options": { "split": null, "type": "literal", "lang": "en", "private": false } },
            { "column": { "index": 2, "header": "Date" }, "options": { "split": "|", "type": { "dataType": "edtf" }, "lang": null, "private": false } }
        ]
    }, lost]);
    plan["slots"]["item"] = json!({
        "sets": { "sources": [{ "from": "group", "name": "set" }], "combine": "first", "map": { "Scans": "" } },
        "template": { "sources": [{ "from": "constant", "value": "Artifact" }], "combine": "first" },
        "dcterms:title": { "sources": [column("register", 1, "Title"), { "from": "group", "name": "identifier" }], "combine": "first" },
        "dcterms:date": { "sources": [column("register", 2, "Date")], "combine": "all" },
        "local:shelfMark": { "sources": [{ "from": "folder", "level": 1 }, { "from": "folder", "level": "path" }], "combine": "all" }
    });
    plan["slots"]["media"] = json!({
        "dcterms:title": { "sources": [{ "from": "file", "fact": "name" }, { "from": "tag", "tag": "File name", "files": "first" }], "combine": "first" }
    });
    plan["flags"] = json!({ "space-at-ends": "keep", "unmatched": "leave" });
    plan["newTypes"] = json!([{ "label": "Artifact", "from": "mapping", "class": null }]);
    plan["newTerms"] = json!([{ "label": "Shelf mark" }]);
    plan["example"] = json!({ "path": "Artifacts/BHC_006_AF_001_front.jpg", "labels": { "n0": "identifier" }, "optional": ["part"] });
    plan["decisions"] = json!([
        { "kind": "move", "file": file("Artifacts/BHC_006_AF_001_back.jpg"), "to": { "with": file("Artifacts/BHC_006_AF_002_front.jpg") } },
        { "kind": "move", "file": file("Photographs/1950s/BHC_006_PH_102.tif"), "to": { "with": file("Artifacts/BHC_006_AF_004_front.jpg") } },
        { "kind": "set", "entry": file("Artifacts/BHC_006_AF_004_front.jpg"), "target": "item", "slot": "dcterms:title",
          "values": [{ "type": "literal", "text": "Front title", "lang": null, "label": null }] },
        { "kind": "set", "entry": file("Artifacts/BHC_006_AF_004_back.jpg"), "target": "item", "slot": "dcterms:title",
          "values": [{ "type": "literal", "text": "Back title", "lang": null, "label": null }] },
        { "kind": "move", "file": file("Artifacts/BHC_006_AF_003_back.jpg"), "to": "own" },
        { "kind": "set", "entry": file("Photographs/1950s/BHC_006_PH_101.tif"), "target": "item", "slot": "dcterms:title",
          "values": [{ "type": "literal", "text": "Studio portrait", "lang": "en", "label": null },
                     { "type": "uri", "text": "http://x.org/a", "lang": null, "label": "Portrait" }] },
        { "kind": "match", "entry": { "row": { "sheet": "register", "key": "BHC_006_AF_001#1" } }, "target": "item", "resource": null },
        { "kind": "flag", "entry": file("Photographs/1950s/BHC_006_PH_102.tif"), "flag": "format-mismatch", "slot": null, "policy": "leave" },
        { "kind": "pair", "row": { "sheet": "register", "key": "BHC_006_ZZ_999#1" }, "file": { "source": "s1", "path": "Misc/loose.jpg" } },
        { "kind": "many", "entries": [file("Photographs/1950s/BHC_006_PH_103.tif"), file("Photographs/1950s/BHC_006_PH_104.tif")],
          "change": { "kind": "set", "target": "item", "slot": "dcterms:title", "values": [] } },
        { "kind": "many", "entries": [file("Scans/BHC_006_SC_001_p1.tif")], "change": { "kind": "leave" } },
        { "kind": "leave", "entry": file("gone/missing.jpg") }
    ]);
}

pub async fn upload_list(
    fixture: &Fixture,
    id: &Value,
    name: &str,
    bytes: &[u8],
) -> common::Answer {
    let boundary = "romeka-wire-boundary";
    let mut body = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{name}\"\r\nContent-Type: text/csv\r\n\r\n"
    )
    .into_bytes();
    body.extend_from_slice(bytes);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    let (identity, credential) = ADMIN;
    let request = Request::builder()
        .method("POST")
        .uri(format!(
            "/api/romeka/imports/{id}/sheets?key_identity={identity}&key_credential={credential}"
        ))
        .header(
            "content-type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(body))
        .unwrap();
    common::send(fixture.app.clone(), request).await
}

pub async fn ok(answer: common::Answer) -> Value {
    assert!(
        answer.status.is_success(),
        "{} {}",
        answer.status,
        answer.body
    );
    if answer.body.is_empty() {
        Value::Null
    } else {
        answer.json()
    }
}

pub async fn refused(answer: common::Answer, status: StatusCode, code: Option<&str>) -> Value {
    assert_eq!(answer.status, status, "{}", answer.body);
    let body = answer.json();
    if let Some(code) = code {
        assert_eq!(body["code"], code, "{body}");
    }
    body
}

pub fn at(id: &Value, tail: &str) -> String {
    format!("/api/romeka/imports/{id}{tail}")
}

pub async fn checked(fixture: &Fixture, id: &Value) -> Value {
    let answer = fixture.preview(id, &json!({ "include": ["check"] })).await;
    ok(answer).await["check"].clone()
}
