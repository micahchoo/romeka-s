//! What a run writes into each item: its display title, and one value per
//! distinct value its files give.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use omeka_db::ids::PropertyId;
use omeka_db::repos::TemplateRepo;
use omeka_db::rows::NewTemplate;
use serde_json::json;

use super::plan::ImportPlan;
use super::run::find_property;
use super::run_tests::{Fixture, fixture, plan};

fn plan_with(change: impl FnOnce(&mut serde_json::Value)) -> ImportPlan {
    let mut value = plan().to_json();
    change(&mut value);
    ImportPlan::from_json(&value).expect("plan")
}

async fn property(fixture: &Fixture, term: &str) -> PropertyId {
    find_property(&fixture.pool, term)
        .await
        .expect("lookup")
        .expect("property")
}

/// The denormalised title of the item holding `identifier`.
async fn title(fixture: &Fixture, identifier: &str) -> Option<String> {
    sqlx::query_scalar(
        "SELECT r.title FROM resource r JOIN value v ON v.resource_id = r.id
         WHERE v.property_id = ? AND v.value = ?",
    )
    .bind(property(fixture, "dcterms:identifier").await.0 as i64)
    .bind(identifier)
    .fetch_one(&fixture.pool)
    .await
    .expect("item")
}

/// The values of `term` on the item holding `identifier`, in write order.
async fn values(fixture: &Fixture, identifier: &str, term: &str) -> Vec<String> {
    sqlx::query_scalar(
        "SELECT w.value FROM value v JOIN value w ON w.resource_id = v.resource_id
         WHERE v.property_id = ? AND v.value = ? AND w.property_id = ? ORDER BY w.id",
    )
    .bind(property(fixture, "dcterms:identifier").await.0 as i64)
    .bind(identifier)
    .bind(property(fixture, term).await.0 as i64)
    .fetch_all(&fixture.pool)
    .await
    .expect("values")
}

#[tokio::test]
async fn a_person_made_by_the_run_takes_its_title_from_its_name() {
    let fixture = fixture().await;
    let plan = plan_with(|value| {
        value["typeFor"] = json!({ "Letters": { "starter": "person", "name": "Correspondent" } });
        value["csv"]["columns"]["Title"] = json!("foaf:name");
    });
    let (_, summary, _) = fixture.run_plan(plan).await;
    assert_eq!(summary.items, 3);

    let template = TemplateRepo::new(&fixture.pool)
        .find_by_label("Correspondent")
        .await
        .expect("find")
        .expect("the new type");
    assert_eq!(
        template.title_property_id,
        Some(property(&fixture, "foaf:name").await)
    );
    assert_eq!(
        title(&fixture, "L_001").await.as_deref(),
        Some("First letter")
    );
    assert_eq!(
        title(&fixture, "P_001").await,
        None,
        "no template, so dcterms:title, which it lacks"
    );
}

#[tokio::test]
async fn an_existing_template_with_a_title_property_names_the_item() {
    let mut fixture = fixture().await;
    let template = TemplateRepo::new(&fixture.pool)
        .create(&NewTemplate {
            owner_id: None,
            resource_class_id: None,
            title_property_id: Some(property(&fixture, "dcterms:alternative").await),
            description_property_id: None,
            label: "Shelved".to_string(),
        })
        .await
        .expect("template");
    fixture.catalog = super::catalog::load(&fixture.pool).await.expect("catalog");
    let plan = plan_with(|value| {
        value["typeFor"] = json!({ "Letters": template.id.0 });
        value["csv"]["columns"]["Shelf mark"] = json!("dcterms:alternative");
    });
    fixture.run_plan(plan).await;

    assert_eq!(title(&fixture, "L_001").await.as_deref(), Some("A-1"));
    assert_eq!(title(&fixture, "P_001").await.as_deref(), Some("Harbour"));
}

#[tokio::test]
async fn a_tag_read_from_every_file_writes_one_value_per_distinct_file_value() {
    let fixture = fixture().await;
    let plan = plan_with(|value| {
        value["meta"] = json!([
            { "tag": "File name", "term": "dcterms:source", "on": "item", "from": "all" },
            { "tag": "Format", "term": "dcterms:format", "on": "item", "from": "all" }
        ]);
    });
    fixture.run_plan(plan).await;

    assert_eq!(
        values(&fixture, "L_001", "dcterms:source").await,
        ["L_001_back.tif", "L_001_front.jpg"]
    );
    assert_eq!(
        values(&fixture, "L_001", "dcterms:format").await,
        ["TIFF", "JPEG"]
    );
    assert_eq!(values(&fixture, "P_001", "dcterms:format").await, ["JPEG"]);
}
