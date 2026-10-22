//! The `proposal` preview section through the route: it is read-only, it is
//! written only when asked for, and `placement_of` reads the preview the way
//! `omeka-api`'s `Placement::of_model` reads a model.

use serde_json::{Value, json};

use super::{add_terms, bhc006, materialize, placement_of, run, v3_plan, v3_plan_of};
use crate::imports_fixture::fixture;

const BHC006: &str = include_str!("../../../omeka-api/src/import/fixtures/v3/bhc006.json");

#[tokio::test]
async fn the_proposal_section_writes_nothing() {
    let fixture = fixture().await;
    let shape = bhc006::shape();
    add_terms(&fixture, &bhc006::SLOTS).await;
    materialize(&shape, fixture.archive.path());
    let draft = fixture.draft().await;
    let id = draft["id"].clone();
    let saved = fixture.save(&draft, v3_plan(BHC006)).await;
    assert_eq!(saved.status, 200, "{}", saved.body);
    let scanned = fixture
        .call("POST", &format!("/api/romeka/imports/{id}/scan"), None)
        .await;
    assert_eq!(scanned.status, 200, "{}", scanned.body);
    let before = fixture.row(&id).await;

    let plain = fixture.preview(&id, &json!({ "include": ["check"] })).await;
    assert_eq!(plain.status, 200, "{}", plain.body);
    assert!(
        plain.json().get("proposal").is_none(),
        "an admin that does not ask sees no new member"
    );

    let asked = fixture
        .preview(&id, &json!({ "include": ["proposal"] }))
        .await;
    assert_eq!(asked.status, 200, "{}", asked.body);
    let proposal = asked.json()["proposal"].clone();
    assert_eq!(
        proposal["matchesPlan"], false,
        "the frozen tree is not the archive as found"
    );
    assert_eq!(proposal["edits"], json!([]));
    let node = &proposal["nodes"][0];
    assert_eq!(node["kind"], "series");
    assert_eq!(
        node["scope"]["scope"],
        json!({ "source": "s1", "folder": "Artifacts" })
    );
    assert!(node["items"]["chosen"]["counts"]["items"].as_u64().unwrap() > 0);

    let mut tried = before.plan.clone();
    // tiny-iiif is outside by its counts, so no proposal offers to leave it out.
    let leave_out =
        json!({ "edit": "leaveOut", "scope": { "source": "s1", "folder": "tiny-iiif" } });
    tried["edits"] = json!([leave_out.clone()]);
    let tried = fixture
        .preview(&id, &json!({ "include": ["proposal"], "plan": tried }))
        .await;
    assert_eq!(tried.status, 200, "{}", tried.body);
    assert_eq!(
        tried.json()["proposal"]["edits"],
        json!([{ "edit": leave_out, "applied": false }]),
        "a tried edit is replayed and kept even when nothing offers it"
    );

    let after = fixture.row(&id).await;
    assert_eq!(after.plan.to_string(), before.plan.to_string());
    assert_eq!(
        (after.revision, after.saved),
        (before.revision, before.saved)
    );
}

#[tokio::test]
async fn placement_of_reads_the_frozen_bhc006_outcome() {
    let fixture = fixture().await;
    let shape = bhc006::shape();
    let preview = run(&fixture, &shape, &bhc006::SLOTS, v3_plan(BHC006)).await;
    let plan = v3_plan_of(BHC006);
    let placement = placement_of(&preview, &plan, fixture.archive.path());

    let made = |kind: &str| -> Vec<Value> {
        preview[kind]
            .as_array()
            .unwrap()
            .iter()
            .filter(|record| record["fate"].get("left").is_none())
            .cloned()
            .collect()
    };
    assert_eq!(
        placement.items.len(),
        made("items").len(),
        "every item is placed once"
    );
    assert_eq!(placement.series.len(), 5, "five series make items");
    assert_eq!(
        placement.lists.len(),
        3,
        "three catalogues describe a series"
    );
    assert!(placement.lists.values().all(|items| !items.is_empty()));
    let media_rows: usize = made("media")
        .iter()
        .map(|record| record["rows"].as_array().unwrap().len())
        .sum();
    let item_rows: usize = made("items")
        .iter()
        .map(|record| record["rows"].as_array().unwrap().len())
        .sum();
    assert_eq!(
        placement.rows.len(),
        media_rows + item_rows,
        "each paired row is placed once"
    );
    let outside = shape
        .files
        .iter()
        .map(|(path, _)| path)
        .filter(|path| path.starts_with("Extras/") || path.starts_with("tiny-iiif/"));
    for path in outside {
        assert!(
            placement
                .left_out
                .iter()
                .any(|(_, file)| file.as_str() == path),
            "{path} is in no item"
        );
    }
}
