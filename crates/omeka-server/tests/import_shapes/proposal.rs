//! The `proposal` preview section through the route: it is read-only, it is
//! written only when asked for, and `placement_of` reads the preview the way
//! `omeka-api`'s `Placement::of_model` reads a model.

use serde_json::{Value, json};

use omeka_api::import::placement::differences;
use omeka_api::import::plan::ImportPlan;

use super::{add_terms, bhc006, materialize, placement_of, run, v3_plan, v3_plan_of};
use crate::imports_fixture::{self, fixture};

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
    // No folder of the archive has this name, so the edit finds nothing.
    let leave_out =
        json!({ "edit": "leaveOut", "scope": { "source": "s1", "folder": "no such folder" } });
    tried["edits"] = json!([leave_out.clone()]);
    let tried = fixture
        .preview(&id, &json!({ "include": ["proposal"], "plan": tried }))
        .await;
    assert_eq!(tried.status, 200, "{}", tried.body);
    assert_eq!(
        tried.json()["proposal"]["edits"],
        json!([{ "edit": leave_out, "applied": false, "reason": "gone" }]),
        "a tried edit is replayed and kept even when it finds nothing"
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

/// The edits a person makes on BHC006's proposal, each a click it offers.
/// No count tells an extras folder from a series. And no count tells PN's 26
/// rows that name no file from S5's one row that makes an item, so the
/// person takes the rival grain the proposal offers.
fn bhc006_edits() -> Value {
    json!([
        { "edit": "leaveOut", "scope": { "source": "s1", "folder": "Extras" } },
        {
            "edit": "grain",
            "list": { "source": "s1", "file": "BHC006_GMT_PN.xlsx" },
            "describes": "media"
        }
    ])
}

#[tokio::test]
async fn bhc006_places_as_the_frozen_outcome_after_its_edits() {
    let shape = bhc006::shape();
    let frozen = fixture().await;
    let preview = run(&frozen, &shape, &bhc006::SLOTS, v3_plan(BHC006)).await;
    let want = placement_of(&preview, &v3_plan_of(BHC006), frozen.archive.path());

    let asked = fixture().await;
    add_terms(&asked, &bhc006::SLOTS).await;
    materialize(&shape, asked.archive.path());
    let draft = asked.draft().await;
    let id = draft["id"].clone();
    let scanned = asked
        .call("POST", &format!("/api/romeka/imports/{id}/scan"), None)
        .await;
    assert_eq!(scanned.status, 200, "{}", scanned.body);
    // The draft's plan as v3, the only version that holds edits.
    let as_found = json!([imports_fixture::v3_series("all", "")]);
    let mut tried = imports_fixture::v3_plan(&draft["plan"], as_found);
    tried["edits"] = bhc006_edits();
    let answer = asked
        .preview(&id, &json!({ "include": ["proposal"], "plan": tried }))
        .await;
    assert_eq!(answer.status, 200, "{}", answer.body);
    let proposal = answer.json()["proposal"].clone();
    let applied: Vec<&Value> = proposal["edits"]
        .as_array()
        .unwrap()
        .iter()
        .map(|edit| &edit["applied"])
        .collect();
    assert_eq!(applied, [true, true], "the proposal offers both edits");

    let mut confirmed = imports_fixture::v3_plan(&draft["plan"], json!([]));
    confirmed["structure"] = proposal["structure"].clone();
    confirmed["edits"] = bhc006_edits();
    let text = confirmed.to_string();
    let plan = ImportPlan::from_json(&text).unwrap();
    let saved = fixture().await;
    let got_preview = run(&saved, &shape, &bhc006::SLOTS, |draft_plan: &mut Value| {
        let mut plan: Value = serde_json::from_str(&text).unwrap();
        plan["sources"] = draft_plan["sources"].clone();
        *draft_plan = plan;
    })
    .await;
    let got = placement_of(&got_preview, &plan, saved.archive.path());
    let found = differences(&want, &got);
    assert!(found.is_empty(), "{} differences: {found:#?}", found.len());
}
