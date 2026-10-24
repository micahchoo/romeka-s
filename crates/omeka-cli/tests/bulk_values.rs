//! Value edits in a bulk change through `/api/romeka/bulk` (`briefs/items-bulk-edit-design.md`,
//! phase 3): add, replace, clear and visibility, their preview, their run and their undo.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod bulk_site;
mod import_site;

use bulk_site::*;
use omeka_db::ids::{PropertyId, TemplateId};
use omeka_db::repos::TemplateRepo;
use omeka_db::rows::{NewTemplate, NewTemplateProperty};
use serde_json::{Value, json};

async fn property(site: &Site, term: &str) -> u64 {
    let (status, found) = admin(site, "GET", &format!("/api/properties?term={term}"), None).await;
    assert_eq!(status, 200, "{found}");
    found[0]["o:id"].as_u64().unwrap()
}

async fn item_json(site: &Site, id: u64) -> Value {
    let (status, item) = admin(site, "GET", &format!("/api/items/{id}"), None).await;
    assert_eq!(status, 200, "{item}");
    item
}

/// An item's values in `terms`, each as the columns a value carries, without anything a write
/// makes anew (a value id, a linked resource's title).
async fn values_of(site: &Site, id: u64, terms: &[&str]) -> Value {
    let item = item_json(site, id).await;
    let mut out = serde_json::Map::new();
    for term in terms {
        let values: Vec<Value> = item[*term]
            .as_array()
            .map(|values| {
                values
                    .iter()
                    .map(|value| {
                        json!({
                            "type": value["type"],
                            "@value": value["@value"],
                            "@language": value["@language"],
                            "@id": value["@id"],
                            "o:label": value["o:label"],
                            "value_resource_id": value["value_resource_id"],
                            "is_public": value["is_public"],
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        out.insert((*term).to_string(), Value::Array(values));
    }
    Value::Object(out)
}

fn literal(property: u64, text: &str, lang: Option<&str>) -> Value {
    json!({ "op": "add", "property": property,
            "value": { "type": "literal", "text": text, "lang": lang } })
}

#[tokio::test(flavor = "multi_thread")]
async fn a_value_added_to_every_matching_item_shows_on_the_item_and_in_search() {
    let site = site().await;
    let subject = property(&site, "dcterms:subject").await;
    let anna = item(&site, "Letter to Anna", &[]).await;
    item(&site, "Letter to Bob", &[]).await;
    let photo = item(&site, "Harbour photograph", &[]).await;
    let change = json!({
        "kind": "edit",
        "values": [literal(subject, "Correspondence", Some("en"))],
    });

    let previewed = preview(&site, json!({ "search": "Letter" }), change.clone()).await;
    assert_eq!(previewed["total"], 2, "{previewed}");
    assert_eq!(previewed["unchanged"], 0);
    let line = &previewed["samples"][0]["values"][0];
    assert_eq!(line["label"], "Subject", "{line}");
    assert_eq!(line["before"], json!([]));
    assert_eq!(
        line["after"],
        json!([{ "text": "Correspondence", "lang": "en", "uri": null, "resource": null,
                 "isPublic": true }])
    );

    let ran = run(&site, previewed["id"].as_u64().unwrap()).await;
    assert_eq!(ran["status"], "done", "{ran}");
    assert_eq!(ran["counts"]["done"], 2);
    let shown = item_json(&site, anna).await;
    assert_eq!(shown["dcterms:subject"][0]["@value"], "Correspondence");
    assert_eq!(shown["dcterms:subject"][0]["@language"], "en");
    assert_eq!(shown["dcterms:subject"][0]["type"], "literal");
    assert!(item_json(&site, photo).await["dcterms:subject"].is_null());
    let (status, found) = admin(
        &site,
        "GET",
        "/api/items?fulltext_search=Correspondence",
        None,
    )
    .await;
    assert_eq!(status, 200, "{found}");
    assert_eq!(
        found.as_array().unwrap().len(),
        2,
        "the index has the new value"
    );

    let again = preview(&site, json!({ "search": "Letter" }), change).await;
    assert_eq!(again["unchanged"], 2, "both items hold the value now");
    assert_eq!(again["samples"], json!([]));
}

#[tokio::test(flavor = "multi_thread")]
async fn undo_returns_the_exact_values_after_add_replace_clear_and_visibility() {
    let site = site().await;
    let [description, subject, relation, source] = [
        property(&site, "dcterms:description").await,
        property(&site, "dcterms:subject").await,
        property(&site, "dcterms:relation").await,
        property(&site, "dcterms:source").await,
    ];
    let photo = item(&site, "Harbour photograph", &[]).await;
    let bob = item(&site, "Letter to Bob", &[]).await;
    let (status, made) = admin(
        &site,
        "POST",
        "/api/items",
        Some(json!({
            "dcterms:title": title("Letter to Anna"),
            "dcterms:description": [
                { "type": "literal", "property_id": description, "@value": "Harbor",
                  "@language": "en", "is_public": false },
                { "type": "literal", "property_id": description, "@value": "Harbor view" },
            ],
            "dcterms:subject": [
                { "type": "uri", "property_id": subject, "@id": "https://example.org/maps",
                  "o:label": "Maps" },
            ],
            "dcterms:relation": [
                { "type": "resource:item", "property_id": relation, "value_resource_id": photo },
            ],
            "dcterms:source": [
                { "type": "literal", "property_id": source, "@value": "Box 4" },
            ],
        })),
    )
    .await;
    assert_eq!(status, 200, "{made}");
    let anna = made["o:id"].as_u64().unwrap();
    let terms = [
        "dcterms:title",
        "dcterms:description",
        "dcterms:subject",
        "dcterms:relation",
        "dcterms:source",
    ];
    let before = values_of(&site, anna, &terms).await;

    let previewed = preview(
        &site,
        json!({ "id": [anna] }),
        json!({ "kind": "edit", "values": [
            { "op": "replace", "property": description, "from": "Harbor", "to": "Harbour" },
            { "op": "visibility", "property": subject, "isPublic": false },
            literal(subject, "Rivers", None),
            { "op": "add", "property": relation, "value": { "type": "item", "item": bob } },
            { "op": "clear", "property": source },
        ] }),
    )
    .await;
    let lines = previewed["samples"][0]["values"].as_array().unwrap();
    let relation_line = lines
        .iter()
        .find(|line| line["property"] == relation)
        .unwrap();
    assert_eq!(relation_line["after"][1]["text"], "Letter to Bob");
    let id = previewed["id"].as_u64().unwrap();
    assert_eq!(run(&site, id).await["status"], "done");

    let after = values_of(&site, anna, &terms).await;
    assert_eq!(after["dcterms:description"][0]["@value"], "Harbour");
    assert_eq!(after["dcterms:description"][0]["@language"], "en");
    assert_eq!(after["dcterms:description"][0]["is_public"], false);
    assert_eq!(
        after["dcterms:description"][1]["@value"], "Harbor view",
        "only the exact text is replaced"
    );
    assert_eq!(after["dcterms:subject"][0]["is_public"], false);
    assert_eq!(after["dcterms:subject"][1]["@value"], "Rivers");
    assert_eq!(after["dcterms:subject"][1]["is_public"], true);
    assert_eq!(after["dcterms:relation"][1]["value_resource_id"], bob);
    assert_eq!(after["dcterms:source"], json!([]));
    assert_eq!(after["dcterms:title"], before["dcterms:title"]);

    let undone = undo(&site, id, false).await;
    assert_eq!(undone["status"], "undone", "{undone}");
    assert_eq!(values_of(&site, anna, &terms).await, before);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_change_that_would_empty_a_required_field_leaves_that_item() {
    let site = site().await;
    let description = property(&site, "dcterms:description").await;
    let templates = TemplateRepo::new(&site.pool);
    let letter = templates
        .create(&NewTemplate {
            owner_id: None,
            resource_class_id: None,
            title_property_id: None,
            description_property_id: None,
            label: "Letter".to_string(),
        })
        .await
        .unwrap();
    templates
        .add_property(&NewTemplateProperty {
            resource_template_id: letter.id,
            property_id: PropertyId(description),
            alternate_label: None,
            alternate_comment: None,
            position: Some(1),
            data_type: None,
            is_required: true,
            is_private: false,
            default_lang: None,
        })
        .await
        .unwrap();
    let described = |name: &str, template: Option<TemplateId>| {
        json!({
            "dcterms:title": title(name),
            "dcterms:description": [
                { "type": "literal", "property_id": description, "@value": "A letter" },
            ],
            "o:resource_template": template.map(|id| json!({ "o:id": id.0 })),
        })
    };
    let mut made = Vec::new();
    for body in [
        described("Letter to Anna", Some(letter.id)),
        described("Letter to Bob", None),
    ] {
        let (status, item) = admin(&site, "POST", "/api/items", Some(body)).await;
        assert_eq!(status, 200, "{item}");
        made.push(item["o:id"].as_u64().unwrap());
    }
    let [typed, plain] = made[..] else {
        unreachable!()
    };

    let previewed = preview(
        &site,
        json!({ "id": [typed, plain] }),
        json!({ "kind": "edit", "values": [{ "op": "clear", "property": description }] }),
    )
    .await;
    let ran = run(&site, previewed["id"].as_u64().unwrap()).await;

    assert_eq!(ran["counts"]["done"], 1, "{ran}");
    assert_eq!(ran["counts"]["failed"], 1);
    assert_eq!(ran["failures"][0]["item"], typed);
    assert_eq!(ran["failures"][0]["reason"], "removesRequired");
    assert_eq!(
        item_json(&site, typed).await["dcterms:description"][0]["@value"],
        "A letter"
    );
    assert!(item_json(&site, plain).await["dcterms:description"].is_null());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_value_edit_naming_a_missing_field_or_item_or_an_empty_text_is_refused() {
    let site = site().await;
    let subject = property(&site, "dcterms:subject").await;
    let anna = item(&site, "Letter to Anna", &[]).await;
    for (values, status) in [
        (json!([{ "op": "clear", "property": 99_999 }]), 404),
        (
            json!([{ "op": "add", "property": subject, "value": { "type": "item", "item": 99_999 } }]),
            404,
        ),
        (json!([literal(subject, " ", None)]), 400),
    ] {
        let (answered, body) = admin(
            &site,
            "POST",
            "/api/romeka/bulk",
            Some(json!({ "query": { "id": [anna] },
                         "change": { "kind": "edit", "values": values } })),
        )
        .await;
        assert_eq!(answered, status, "{values}: {body}");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn undo_keeps_an_item_taken_off_a_site_after_the_run_unless_forced() {
    let site = site().await;
    let (status, made) = admin(
        &site,
        "POST",
        "/api/sites",
        Some(json!({ "o:slug": "harbour", "o:title": "Harbour", "o:theme": "default" })),
    )
    .await;
    assert_eq!(status, 200, "{made}");
    let harbour = made["o:id"].as_u64().unwrap();
    let detached = item(&site, "Letter to Anna", &[]).await;
    let other = item(&site, "Letter to Bob", &[]).await;
    let previewed = preview(
        &site,
        json!({ "id": [detached, other] }),
        json!({ "kind": "edit", "isPublic": false, "sites": { "add": [harbour] } }),
    )
    .await;
    let id = previewed["id"].as_u64().unwrap();
    assert_eq!(run(&site, id).await["status"], "done");
    let (status, changed) = admin(
        &site,
        "POST",
        &format!("/api/sites/{harbour}/items"),
        Some(json!({ "action": "remove", "items": [detached] })),
    )
    .await;
    assert_eq!(status, 200, "{changed}");

    let (status, would) = admin(
        &site,
        "POST",
        &format!("/api/romeka/bulk/{id}/undo/preview"),
        None,
    )
    .await;
    assert_eq!(status, 200, "{would}");
    assert_eq!(would["editedItems"], json!([detached]), "{would}");

    let kept = undo(&site, id, false).await;
    assert_eq!(kept["status"], "partly_undone", "{kept}");
    assert_eq!(kept["counts"]["kept"], 1);
    assert_eq!(item_json(&site, detached).await["o:is_public"], false);
    assert_eq!(item_json(&site, other).await["o:is_public"], true);

    let forced = undo(&site, id, true).await;
    assert_eq!(forced["status"], "undone", "{forced}");
    let restored = item_json(&site, detached).await;
    assert_eq!(restored["o:is_public"], true);
    assert_eq!(restored["o:site"], json!([]));
}
