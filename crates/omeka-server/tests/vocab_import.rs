//! `POST /api/vocabularies/import`, `POST /api/vocabularies/{id}/diff` and
//! `POST /api/vocabularies/{id}/diff/apply`: the admin's vocabulary import and update.
//!
//! Every test drives the router against a migrated in-memory database and reads the vocabulary
//! back through the read endpoints, so it asserts what a caller sees. The upstream rule each test
//! freezes is named in its doc comment (`Stdlib/RdfImporter`, `Admin/VocabularyController`).

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::Router;
use axum::http::StatusCode;
use common::{Answer, append_key, get, json_request, seeded_router, send};
use serde_json::{Value, json};

const FIXTURES: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../omeka-api/tests/fixtures/rdf"
);
const FOAF: &str = "http://xmlns.com/foaf/0.1/";
const BIBO: &str = "http://purl.org/ontology/bibo/";
const EX: &str = "http://example.org/ns#";

fn fixture(name: &str) -> String {
    std::fs::read_to_string(format!("{FIXTURES}/{name}")).unwrap()
}

fn upload(text: &str) -> Value {
    json!({ "import_type": "upload", "file": text, "format": "guess" })
}

async fn post(app: &Router, uri: &str, body: &Value) -> Answer {
    send(
        app.clone(),
        json_request("POST", &append_key(uri), &body.to_string()),
    )
    .await
}

async fn import(app: &Router, namespace: &str, prefix: &str, source: Value) -> Answer {
    let body = json!({
        "o:namespace_uri": namespace,
        "o:prefix": prefix,
        "o:label": format!("{prefix} vocabulary"),
        "source": source,
    });
    post(app, "/api/vocabularies/import", &body).await
}

/// `(local_name, label, comment)` of one kind of member, in id order.
async fn members(app: &Router, resource: &str, vocabulary: u64) -> Vec<(String, String, Value)> {
    let answer = send(
        app.clone(),
        get(
            &format!("/api/{resource}?vocabulary_id={vocabulary}&per_page=1000&sort_by=id"),
            false,
        ),
    )
    .await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    answer
        .json()
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            (
                row["o:local_name"].as_str().unwrap().to_string(),
                row["o:label"].as_str().unwrap().to_string(),
                row["o:comment"].clone(),
            )
        })
        .collect()
}

/// The oracle's answer for a fixture, in the shape [`members`] reads.
fn oracle(name: &str, kind: &str) -> Vec<(String, String, Value)> {
    let document: Value = serde_json::from_str(&fixture(&format!("oracle/{name}.json"))).unwrap();
    document[kind]
        .as_array()
        .unwrap()
        .iter()
        .map(|term| {
            (
                term["local_name"].as_str().unwrap().to_string(),
                term["label"].as_str().unwrap().to_string(),
                term["comment"].clone(),
            )
        })
        .collect()
}

fn source_errors(answer: &Answer) -> Vec<String> {
    answer.json()["errors"]["source"]
        .as_array()
        .unwrap_or_else(|| panic!("no source errors: {}", answer.body))
        .iter()
        .map(|message| message.as_str().unwrap().to_string())
        .collect()
}

/// `RdfImporter::import`: the vocabulary and every member upstream extracts from `bibo.owl` are
/// stored, in upstream's order, owned by the caller.
#[tokio::test]
async fn an_uploaded_file_imports_every_member_upstream_stores() {
    let (app, seed) = seeded_router().await;
    let answer = import(&app, BIBO, "bibo", upload(&fixture("bibo.owl"))).await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    let vocabulary = answer.json();
    assert_eq!(vocabulary["o:prefix"], "bibo");
    assert_eq!(vocabulary["o:namespace_uri"], BIBO);
    assert_eq!(vocabulary["o:owner"]["o:id"], seed.user.0);
    let id = vocabulary["o:id"].as_u64().unwrap();
    assert_eq!(
        members(&app, "resource_classes", id).await,
        oracle("bibo", "classes")
    );
    assert_eq!(
        members(&app, "properties", id).await,
        oracle("bibo", "properties")
    );
}

/// Upstream stores FOAF's `givenName` and `givenname` side by side (its `local_name` is
/// `utf8mb4_bin`), and so does this schema: the import stores all 62 properties upstream does.
#[tokio::test]
async fn local_names_that_differ_only_in_case_import_as_two_members() {
    let (app, _seed) = seeded_router().await;
    let answer = import(&app, FOAF, "foaf", upload(&fixture("foaf.rdf"))).await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    let id = answer.json()["o:id"].as_u64().unwrap();
    let properties = members(&app, "properties", id).await;
    assert_eq!(properties, oracle("foaf", "properties"));
    for local_name in ["givenName", "givenname"] {
        assert!(
            properties.iter().any(|(name, ..)| name == local_name),
            "foaf:{local_name} is stored"
        );
    }
    assert_eq!(
        members(&app, "resource_classes", id).await,
        oracle("foaf", "classes")
    );
}

/// `RdfImporter::update` adds a member whose name differs from a stored one only in case.
#[tokio::test]
async fn an_update_adds_a_member_differing_only_in_case() {
    let (app, _seed) = seeded_router().await;
    let turtle = |members: &str| {
        json!({
            "import_type": "upload",
            "format": "turtle",
            "file": format!(
                "@prefix rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#> .\n\
                 @prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .\n\
                 @prefix ex: <{EX}> .\n{members}"
            ),
        })
    };
    let upper = "ex:givenName a rdf:Property ; rdfs:label \"Given name\" .\n";
    let lower = "ex:givenname a rdf:Property ; rdfs:label \"Given name (old)\" .\n";
    let created = import(&app, EX, "ex", turtle(upper)).await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.body);
    let id = created.json()["o:id"].as_u64().unwrap();
    let diff = post(
        &app,
        &format!("/api/vocabularies/{id}/diff"),
        &json!({ "source": turtle(&format!("{upper}{lower}")) }),
    )
    .await;
    assert_eq!(diff.status, StatusCode::OK, "{}", diff.body);
    let diff = diff.json();
    assert_eq!(
        diff["properties"]["new"],
        json!([{ "o:local_name": "givenname", "o:label": "Given name (old)", "o:comment": null }])
    );
    let applied = post(&app, &format!("/api/vocabularies/{id}/diff/apply"), &diff).await;
    assert_eq!(applied.status, StatusCode::OK, "{}", applied.body);
    assert_eq!(
        members(&app, "properties", id).await,
        [
            (
                "givenName".to_string(),
                "Given name".to_string(),
                Value::Null
            ),
            (
                "givenname".to_string(),
                "Given name (old)".to_string(),
                Value::Null
            ),
        ]
    );
}

/// `RdfImporter::import`: a file with no member in the namespace is refused with upstream's
/// message, and nothing is created.
#[tokio::test]
async fn a_namespace_the_file_does_not_use_imports_nothing() {
    let (app, _seed) = seeded_router().await;
    let answer = import(
        &app,
        "http://xmlns.com/foaf/0.1",
        "foaf",
        upload(&fixture("foaf.rdf")),
    )
    .await;
    assert_eq!(
        answer.status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "{}",
        answer.body
    );
    assert_eq!(
        source_errors(&answer),
        ["No classes or properties found. Are you sure you used the correct namespace URI?"]
    );
    let list = send(app.clone(), get("/api/vocabularies?prefix=foaf", false)).await;
    assert_eq!(list.json(), json!([]));
}

/// `RdfImporter::getMembers`: a file the parser refuses gives upstream's message, then the
/// parser's own reason.
#[tokio::test]
async fn a_broken_file_reports_why_it_did_not_parse() {
    let (app, _seed) = seeded_router().await;
    let source = json!({ "import_type": "upload", "file": "@prefix ex: <http://e/> .\nex:a ex:b", "format": "turtle" });
    let answer = import(&app, EX, "ex", source).await;
    assert_eq!(
        answer.status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "{}",
        answer.body
    );
    let errors = source_errors(&answer);
    assert_eq!(errors[0], "Could not parse vocabulary file.");
    assert_eq!(errors.len(), 2, "{errors:?}");
}

/// The import creates through `POST /api/vocabularies`, so the vocabulary rules hold: a taken
/// prefix is refused under `o:prefix`.
#[tokio::test]
async fn the_vocabulary_rules_apply_to_an_import() {
    let (app, _seed) = seeded_router().await;
    let answer = import(&app, EX, "dcterms", upload(&fixture("lang.ttl"))).await;
    assert_eq!(
        answer.status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "{}",
        answer.body
    );
    assert_eq!(
        answer.json()["errors"]["o:prefix"],
        json!(["The prefix \"dcterms\" is already taken."])
    );
}

/// A URL that names the server's own network is never fetched.
#[tokio::test]
async fn a_url_on_a_private_address_is_refused() {
    let (app, _seed) = seeded_router().await;
    for url in [
        "http://127.0.0.1:9/vocab.ttl",
        "http://169.254.169.254/latest/",
        "file:///etc/passwd",
    ] {
        let source = json!({ "import_type": "url", "url": url });
        let answer = import(&app, EX, "ex", source).await;
        assert_eq!(
            answer.status,
            StatusCode::UNPROCESSABLE_ENTITY,
            "{url}: {}",
            answer.body
        );
        assert_eq!(
            source_errors(&answer)[0],
            "Could not load vocabulary from URL.",
            "{url}"
        );
    }
    let answer = import(
        &app,
        EX,
        "ex",
        json!({ "import_type": "url", "url": "http://localhost/" }),
    )
    .await;
    assert_eq!(
        source_errors(&answer)[1],
        "The URL names a private or local network address, which the server does not fetch."
    );
}

/// `AclFactory`: only the admin roles write vocabularies, so a caller with no key is refused.
#[tokio::test]
async fn an_anonymous_caller_may_not_import() {
    let (app, _seed) = seeded_router().await;
    let body = json!({
        "o:namespace_uri": EX, "o:prefix": "ex", "o:label": "Ex", "source": upload(&fixture("lang.ttl")),
    });
    let answer = send(
        app.clone(),
        json_request("POST", "/api/vocabularies/import", &body.to_string()),
    )
    .await;
    assert_eq!(answer.status, StatusCode::FORBIDDEN, "{}", answer.body);
}

/// `VocabularyController::editAction`: Dublin Core cannot be updated from a file.
#[tokio::test]
async fn a_permanent_vocabulary_is_not_compared() {
    let (app, seed) = seeded_router().await;
    let uri = format!("/api/vocabularies/{}/diff", seed.vocabulary.0);
    let answer = post(
        &app,
        &uri,
        &json!({ "source": upload(&fixture("dcterms.rdf")) }),
    )
    .await;
    assert_eq!(answer.status, StatusCode::FORBIDDEN, "{}", answer.body);
    assert_eq!(
        answer.json()["errors"]["error"],
        "Cannot edit a permanent vocabulary"
    );
}

/// The second version of `lang.ttl`: `Place` relabelled, `age` recommented, `email` added and
/// `Titled` dropped.
fn second_version() -> String {
    fixture("lang.ttl")
        .replace(
            "skos:prefLabel \"Lieu\"@fr",
            "skos:prefLabel \"Endroit\"@fr",
        )
        .replace("\"Years.\"@en", "\"Age in years.\"@en")
        .replace(
            "ex:Titled a owl:Class ;\n  dc11:title \"From DC elements\" .",
            "ex:email a rdf:Property ; rdfs:label \"email\" .",
        )
}

/// `RdfImporter::getDiff` and `::update`: the diff lists the new member and the changed texts,
/// reports the dropped class as kept, and applying it changes only those rows. The dropped class
/// stays, and so does the item that uses it.
#[tokio::test]
async fn an_update_shows_its_diff_and_applies_only_what_it_showed() {
    let (app, _seed) = seeded_router().await;
    let created = import(&app, EX, "ex", upload(&fixture("lang.ttl"))).await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.body);
    let id = created.json()["o:id"].as_u64().unwrap();
    let titled = send(
        app.clone(),
        get(
            &format!("/api/resource_classes?vocabulary_id={id}&local_name=Titled"),
            false,
        ),
    )
    .await
    .json()[0]["o:id"]
        .as_u64()
        .unwrap();
    let item = post(
        &app,
        "/api/items",
        &json!({ "o:resource_class": { "o:id": titled } }),
    )
    .await;
    assert_eq!(item.status, StatusCode::OK, "{}", item.body);

    let diff = post(
        &app,
        &format!("/api/vocabularies/{id}/diff"),
        &json!({ "source": upload(&second_version()) }),
    )
    .await;
    assert_eq!(diff.status, StatusCode::OK, "{}", diff.body);
    let diff = diff.json();
    assert_eq!(
        diff["properties"]["new"],
        json!([{ "o:local_name": "email", "o:label": "email", "o:comment": null }])
    );
    assert_eq!(
        diff["properties"]["comment"],
        json!([{ "o:local_name": "age", "from": "Years.", "to": "Age in years." }])
    );
    assert_eq!(
        diff["classes"]["label"],
        json!([{ "o:local_name": "Place", "from": "Lieu", "to": "Endroit" }])
    );
    assert_eq!(
        diff["classes"]["removed"],
        json!([{ "o:local_name": "Titled", "o:label": "From DC elements" }])
    );

    let applied = post(&app, &format!("/api/vocabularies/{id}/diff/apply"), &diff).await;
    assert_eq!(applied.status, StatusCode::OK, "{}", applied.body);
    let properties = members(&app, "properties", id).await;
    assert!(properties.contains(&("email".into(), "email".into(), Value::Null)));
    assert!(properties.contains(&("age".into(), "age".into(), json!("Age in years."))));
    let classes = members(&app, "resource_classes", id).await;
    assert!(
        classes.iter().any(|(name, ..)| name == "Titled"),
        "{classes:?}"
    );
    assert!(classes.contains(&("Place".into(), "Endroit".into(), json!("Somewhere."))));
    let item_id = item.json()["o:id"].as_u64().unwrap();
    let read = send(app.clone(), get(&format!("/api/items/{item_id}"), true)).await;
    assert_eq!(read.json()["o:resource_class"]["o:id"], titled);

    // The same diff again: `age` no longer reads "Years." and `email` exists, so nothing applies.
    let again = post(&app, &format!("/api/vocabularies/{id}/diff/apply"), &diff).await;
    assert_eq!(
        again.status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "{}",
        again.body
    );
    assert_eq!(
        again.json()["errors"]["o:property"],
        json!([
            "\"age\" has changed since the comparison. Compare the file again.",
            "The local name \"email\" is already taken."
        ])
    );
}
