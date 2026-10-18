//! The write boundary's reading of a reference id, measured against upstream.
//!
//! Every row is an input and the status upstream Omeka S at `5e413f0` answered
//! for it over HTTP; the expected stored id is the id the oracle's read-back
//! carried on a 200. The probe, the full table and the divergences are in
//! `REPORT.md`. A PHP form post sends ids as strings, so the string rows are the
//! ones the port used to fail: `Value::as_u64` did not accept `"12"` and cleared
//! the reference where upstream resolved it.
//!
//! One group of rows is the oracle's own crash rather than a defined status.
//! `o:item_set` and `o:site` feed each entry id to
//! `Doctrine\Common\Collections\ArrayCollection::get(string|int)`, whose
//! `string|int` parameter under `declare(strict_types=1)` is a `TypeError` for a
//! JSON float or bool, so upstream answers 500. The port reads such an id as its
//! number instead; those rows carry a comment naming the oracle's 500.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::Router;
use axum::http::StatusCode;
use common::{Answer, Seed, append_key, json_request, router, seed_system, seeded_pool, send};
use serde_json::{Value, json};

fn keyed(uri: &str) -> String {
    append_key(uri)
}

/// One measured input.
struct Row {
    label: &'static str,
    input: Value,
    status: StatusCode,
    /// The id the read-back carries on a 200; `None` means the member is empty.
    stored: Option<u64>,
}

/// A migrated database with the shared seed, and the router over it.
async fn seeded() -> (Router, Seed) {
    let (pool, seed) = seeded_pool().await;
    (router(pool), seed)
}

/// The inputs for a member guarded by `is_numeric` and looked up by the value
/// (`o:resource_class`, `o:resource_template`, `o:thumbnail`, `o:homepage`).
///
/// A value PHP is not numeric clears the member; a numeric one is compared with
/// the id column, so `"12.5"` names no row and `"12.0"` names 12.
fn numeric_rows(target: u64) -> Vec<Row> {
    let t = target;
    let missing = StatusCode::NOT_FOUND;
    let found = StatusCode::OK;
    vec![
        Row {
            label: "int",
            input: json!(t),
            status: found,
            stored: Some(t),
        },
        Row {
            label: "float_whole",
            input: json!(t as f64),
            status: found,
            stored: Some(t),
        },
        Row {
            label: "float_part",
            input: json!(t as f64 + 0.5),
            status: missing,
            stored: None,
        },
        Row {
            label: "int_zero",
            input: json!(0),
            status: missing,
            stored: None,
        },
        Row {
            label: "int_negative",
            input: json!(-1),
            status: missing,
            stored: None,
        },
        Row {
            label: "str",
            input: json!(t.to_string()),
            status: found,
            stored: Some(t),
        },
        Row {
            label: "str_leading_space",
            input: json!(format!(" {t}")),
            status: found,
            stored: Some(t),
        },
        Row {
            label: "str_trailing_space",
            input: json!(format!("{t} ")),
            status: found,
            stored: Some(t),
        },
        Row {
            label: "str_float",
            input: json!(format!("{t}.0")),
            status: found,
            stored: Some(t),
        },
        Row {
            label: "str_float_part",
            input: json!(format!("{t}.5")),
            status: missing,
            stored: None,
        },
        Row {
            label: "str_exp",
            input: json!(format!("{t}e0")),
            status: found,
            stored: Some(t),
        },
        Row {
            label: "str_plus",
            input: json!(format!("+{t}")),
            status: found,
            stored: Some(t),
        },
        Row {
            label: "str_leading_zero",
            input: json!(format!("0{t}")),
            status: found,
            stored: Some(t),
        },
        Row {
            label: "str_prefix",
            input: json!(format!("{t}abc")),
            status: found,
            stored: None,
        },
        Row {
            label: "str_null",
            input: json!("null"),
            status: found,
            stored: None,
        },
        Row {
            label: "str_alpha",
            input: json!("abc"),
            status: found,
            stored: None,
        },
        Row {
            label: "str_empty",
            input: json!(""),
            status: found,
            stored: None,
        },
        Row {
            label: "str_zero",
            input: json!("0"),
            status: missing,
            stored: None,
        },
        Row {
            label: "str_zero_float",
            input: json!("0.0"),
            status: missing,
            stored: None,
        },
        Row {
            label: "str_negative",
            input: json!("-1"),
            status: missing,
            stored: None,
        },
        Row {
            label: "str_missing",
            input: json!("99999999"),
            status: missing,
            stored: None,
        },
        Row {
            label: "null",
            input: Value::Null,
            status: found,
            stored: None,
        },
        Row {
            label: "true",
            input: json!(true),
            status: found,
            stored: None,
        },
        Row {
            label: "false",
            input: json!(false),
            status: found,
            stored: None,
        },
    ]
}

/// The inputs for `o:owner`, which `hydrateOwner` casts with `(int)` before the
/// lookup: `"12.5"` names user 12 and only a zero id clears the owner.
fn owner_rows(target: u64) -> Vec<Row> {
    let t = target;
    let mut rows = numeric_rows(t);
    for row in &mut rows {
        match row.label {
            // The `(int)` cast keeps the whole part, so a fractional value
            // names the same user.
            "float_part" | "str_float_part" => {
                row.status = StatusCode::OK;
                row.stored = Some(t);
            }
            // A zero id, whatever its shape, clears the owner instead.
            "int_zero" | "str_zero" | "str_zero_float" => {
                row.status = StatusCode::OK;
                row.stored = None;
            }
            _ => {}
        }
    }
    rows
}

/// The inputs for `o:item_set` and `o:site` on an item, whose entries are looked
/// up in the item's collections.
fn list_rows(target: u64) -> Vec<Row> {
    let t = target;
    let mut rows = numeric_rows(t);
    for row in &mut rows {
        match row.label {
            // Upstream 500. The port reads 12.0 as 12.
            "float_whole" => row.status = StatusCode::OK,
            // Upstream 500. The port reads the fraction as naming no row.
            "float_part" => row.status = StatusCode::NOT_FOUND,
            // Upstream 500 for both. The port reads true as the id 1.
            "true" => {
                row.status = StatusCode::OK;
                row.stored = Some(1);
            }
            "false" => row.status = StatusCode::NOT_FOUND,
            // A non-numeric string still reaches MySQL's reading of a string,
            // so `"12abc"` names 12 rather than clearing the member.
            "str_prefix" => row.stored = Some(t),
            "str_null" | "str_alpha" | "str_empty" => row.status = StatusCode::NOT_FOUND,
            // A null id names no entry, so the list is emptied.
            "null" => {
                row.status = StatusCode::OK;
                row.stored = None;
            }
            _ => {}
        }
    }
    rows
}

/// The inputs for `o:item` on a media create, whose id reaches `findEntity`
/// with no `is_numeric` guard.
fn media_rows(target: u64) -> Vec<Row> {
    let t = target;
    let mut rows = numeric_rows(t);
    // The oracle had no row 1 or 0, so a bool's MySQL reading (1 or 0) answered
    // 404 there; the reading itself is the numeric one, and the port's seed does
    // hold row 1.
    rows.retain(|row| !matches!(row.label, "true" | "false"));
    for row in &mut rows {
        match row.label {
            "str_prefix" => row.stored = Some(t),
            "str_null" | "str_alpha" | "str_empty" => {
                row.status = StatusCode::NOT_FOUND;
                row.stored = None;
            }
            "null" => {
                row.status = StatusCode::UNPROCESSABLE_ENTITY;
                row.stored = None;
            }
            _ => {}
        }
    }
    rows
}

/// The inputs for `o:primary_media`, whose id indexes the item's media array by
/// PHP array key: only a whole number or a canonical integer string matches a
/// media, and anything else is the 422 `Primary media must belong to this
/// item.`
fn primary_media_rows(target: u64) -> Vec<Row> {
    let t = target;
    let ok = StatusCode::OK;
    let invalid = StatusCode::UNPROCESSABLE_ENTITY;
    vec![
        Row {
            label: "int",
            input: json!(t),
            status: ok,
            stored: Some(t),
        },
        Row {
            label: "float_whole",
            input: json!(t as f64),
            status: ok,
            stored: Some(t),
        },
        Row {
            label: "str",
            input: json!(t.to_string()),
            status: ok,
            stored: Some(t),
        },
        Row {
            label: "str_leading_space",
            input: json!(format!(" {t}")),
            status: invalid,
            stored: None,
        },
        Row {
            label: "str_trailing_space",
            input: json!(format!("{t} ")),
            status: invalid,
            stored: None,
        },
        Row {
            label: "str_float",
            input: json!(format!("{t}.0")),
            status: invalid,
            stored: None,
        },
        Row {
            label: "str_exp",
            input: json!(format!("{t}e0")),
            status: invalid,
            stored: None,
        },
        Row {
            label: "str_plus",
            input: json!(format!("+{t}")),
            status: invalid,
            stored: None,
        },
        Row {
            label: "str_leading_zero",
            input: json!(format!("0{t}")),
            status: invalid,
            stored: None,
        },
        Row {
            label: "str_prefix",
            input: json!(format!("{t}abc")),
            status: ok,
            stored: None,
        },
        Row {
            label: "str_missing",
            input: json!("99999999"),
            status: invalid,
            stored: None,
        },
        Row {
            label: "null",
            input: Value::Null,
            status: ok,
            stored: None,
        },
        Row {
            label: "true",
            input: json!(true),
            status: ok,
            stored: None,
        },
    ]
}

/// Patch one item with one `{"o:id": N}` member and return the answer.
async fn patch_item(app: &Router, item: u64, key: &str, input: &Value) -> Answer {
    let body = json!({ key: { "o:id": input } }).to_string();
    send(
        app.clone(),
        json_request("PATCH", &keyed(&format!("/api/items/{item}")), &body),
    )
    .await
}

/// The `o:id` a reference member's read-back carries.
fn read_ref(answer: &Answer, key: &str) -> Option<u64> {
    answer
        .json()
        .get(key)
        .and_then(|value| value.get("o:id"))
        .and_then(Value::as_u64)
}

/// The `o:id`s a list member's read-back carries.
fn read_list(answer: &Answer, key: &str) -> Vec<u64> {
    answer
        .json()
        .get(key)
        .and_then(|value| value.as_array())
        .map(|entries| {
            entries
                .iter()
                .filter_map(|entry| entry.get("o:id").and_then(Value::as_u64))
                .collect()
        })
        .unwrap_or_default()
}

/// Drive one reference member over every row and compare with the oracle.
async fn run_ref_member(app: &Router, item: u64, key: &str, rows: Vec<Row>) {
    for row in rows {
        let answer = patch_item(app, item, key, &row.input).await;
        assert_eq!(
            answer.status, row.status,
            "{key} {}: expected {}, got {}: {}",
            row.label, row.status, answer.status, answer.body
        );
        if answer.status == StatusCode::OK {
            assert_eq!(
                read_ref(&answer, key),
                row.stored,
                "{key} {} stored the wrong id: {}",
                row.label,
                answer.body
            );
        }
    }
}

/// Drive one list member over every row and compare with the oracle.
async fn run_list_member(app: &Router, item: u64, key: &str, rows: Vec<Row>) {
    for row in rows {
        let body = json!({ key: [{ "o:id": row.input }] }).to_string();
        let answer = send(
            app.clone(),
            json_request("PATCH", &keyed(&format!("/api/items/{item}")), &body),
        )
        .await;
        assert_eq!(
            answer.status, row.status,
            "{key} {}: expected {}, got {}: {}",
            row.label, row.status, answer.status, answer.body
        );
        if answer.status == StatusCode::OK {
            let expected = row.stored.map(|id| vec![id]).unwrap_or_default();
            assert_eq!(
                read_list(&answer, key),
                expected,
                "{key} {} stored the wrong ids: {}",
                row.label,
                answer.body
            );
        }
    }
}

#[tokio::test]
async fn resource_class_reads_what_the_oracle_reads() {
    let (app, seed) = seeded().await;
    run_ref_member(
        &app,
        seed.item.0,
        "o:resource_class",
        numeric_rows(seed.class.0),
    )
    .await;
}

#[tokio::test]
async fn resource_template_reads_what_the_oracle_reads() {
    let (app, seed) = seeded().await;
    run_ref_member(
        &app,
        seed.item.0,
        "o:resource_template",
        numeric_rows(seed.template.0),
    )
    .await;
}

#[tokio::test]
async fn thumbnail_reads_what_the_oracle_reads() {
    let (pool, seed) = seeded_pool().await;
    let app = router(pool.clone());
    let asset = seed_system(&pool, &seed).await.asset;
    run_ref_member(&app, seed.item.0, "o:thumbnail", numeric_rows(asset)).await;
}

#[tokio::test]
async fn owner_reads_what_the_oracle_reads() {
    let (app, seed) = seeded().await;
    run_ref_member(&app, seed.item.0, "o:owner", owner_rows(seed.user.0)).await;
}

#[tokio::test]
async fn homepage_reads_what_the_oracle_reads() {
    let (app, seed) = seeded().await;
    let body = json!({
        "o:title": "Reference page",
        "o:slug": "reference-page",
        "o:site": { "o:id": seed.site.to_string() },
    })
    .to_string();
    let created = send(
        app.clone(),
        json_request("POST", &keyed("/api/site_pages"), &body),
    )
    .await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.body);
    let page = created.json()["o:id"].as_u64().unwrap();
    for row in numeric_rows(page) {
        let body = json!({ "o:homepage": { "o:id": row.input } }).to_string();
        let answer = send(
            app.clone(),
            json_request("PATCH", &keyed(&format!("/api/sites/{}", seed.site)), &body),
        )
        .await;
        assert_eq!(
            answer.status, row.status,
            "o:homepage {}: expected {}, got {}: {}",
            row.label, row.status, answer.status, answer.body
        );
        if answer.status == StatusCode::OK {
            assert_eq!(
                read_ref(&answer, "o:homepage"),
                row.stored,
                "o:homepage {} stored the wrong id: {}",
                row.label,
                answer.body
            );
        }
    }
}

#[tokio::test]
async fn item_set_reads_what_the_oracle_reads() {
    let (app, seed) = seeded().await;
    run_list_member(&app, seed.item.0, "o:item_set", list_rows(seed.item_set.0)).await;
}

#[tokio::test]
async fn site_reads_what_the_oracle_reads() {
    let (app, seed) = seeded().await;
    run_list_member(&app, seed.item.0, "o:site", list_rows(seed.site)).await;
}

#[tokio::test]
async fn media_item_reads_what_the_oracle_reads() {
    let (app, seed) = seeded().await;
    for row in media_rows(seed.item.0) {
        let body = json!({
            "o:ingester": "html",
            "o:item": { "o:id": row.input },
            "data": { "html": "<p>Reference</p>" },
        })
        .to_string();
        let answer = send(
            app.clone(),
            json_request("POST", &keyed("/api/media"), &body),
        )
        .await;
        assert_eq!(
            answer.status, row.status,
            "media o:item {}: expected {}, got {}: {}",
            row.label, row.status, answer.status, answer.body
        );
        if answer.status == StatusCode::OK {
            assert_eq!(
                read_ref(&answer, "o:item"),
                row.stored,
                "media o:item {} stored the wrong id: {}",
                row.label,
                answer.body
            );
        }
    }
}

#[tokio::test]
async fn primary_media_reads_what_the_oracle_reads() {
    let (app, seed) = seeded().await;
    for row in primary_media_rows(seed.media.0) {
        let answer = patch_item(&app, seed.item.0, "o:primary_media", &row.input).await;
        assert_eq!(
            answer.status, row.status,
            "o:primary_media {}: expected {}, got {}: {}",
            row.label, row.status, answer.status, answer.body
        );
    }
}

#[tokio::test]
async fn vocabulary_reads_what_the_oracle_reads() {
    let (app, seed) = seeded().await;
    let t = seed.vocabulary.0;
    // `ResourceClassAdapter`/`PropertyAdapter` guard `o:vocabulary` with
    // `is_numeric` on a create. A value PHP is not numeric leaves the property
    // with no vocabulary, which is the 422 `A vocabulary must be set.`
    for (label, input, status) in [
        ("int", json!(t), StatusCode::OK),
        ("str", json!(t.to_string()), StatusCode::OK),
        ("str_leading_space", json!(format!(" {t}")), StatusCode::OK),
        ("str_float", json!(format!("{t}.0")), StatusCode::OK),
        ("str_exp", json!(format!("{t}e0")), StatusCode::OK),
        ("str_plus", json!(format!("+{t}")), StatusCode::OK),
        ("str_leading_zero", json!(format!("0{t}")), StatusCode::OK),
        ("str_missing", json!("99999999"), StatusCode::NOT_FOUND),
        ("int_zero", json!(0), StatusCode::NOT_FOUND),
        ("str_zero", json!("0"), StatusCode::NOT_FOUND),
        ("str_negative", json!("-1"), StatusCode::NOT_FOUND),
        ("str_alpha", json!("abc"), StatusCode::UNPROCESSABLE_ENTITY),
        (
            "str_prefix",
            json!(format!("{t}abc")),
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
        ("null", Value::Null, StatusCode::UNPROCESSABLE_ENTITY),
        ("true", json!(true), StatusCode::UNPROCESSABLE_ENTITY),
    ] {
        let body = json!({
            "o:vocabulary": { "o:id": input },
            "o:local_name": format!("ref{label}"),
            "o:label": format!("Ref {label}"),
        })
        .to_string();
        let answer = send(
            app.clone(),
            json_request("POST", &keyed("/api/properties"), &body),
        )
        .await;
        assert_eq!(
            answer.status, status,
            "o:vocabulary {label}: expected {status}, got {}: {}",
            answer.status, answer.body
        );
    }
}

/// A `PATCH` that does not name `o:primary_media` keeps the stored media
/// (`shouldHydrate` is false for a partial update that omits the member).
#[tokio::test]
async fn a_partial_update_that_omits_primary_media_keeps_it() {
    let (app, seed) = seeded().await;
    // A second media, so the read-back can tell the kept primary apart from the
    // first-media fallback an omitted member would otherwise land on.
    let body = json!({
        "o:ingester": "html",
        "o:item": { "o:id": seed.item.0 },
        "data": { "html": "<p>Second</p>" },
        "position": 2,
    })
    .to_string();
    let created = send(
        app.clone(),
        json_request("POST", &keyed("/api/media"), &body),
    )
    .await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.body);
    let second = created.json()["o:id"].as_u64().unwrap();

    let set = patch_item(&app, seed.item.0, "o:primary_media", &json!(second)).await;
    assert_eq!(set.status, StatusCode::OK, "{}", set.body);
    assert_eq!(read_ref(&set, "o:primary_media"), Some(second));

    let body = json!({ "o:is_public": true }).to_string();
    let kept = send(
        app,
        json_request(
            "PATCH",
            &keyed(&format!("/api/items/{}", seed.item.0)),
            &body,
        ),
    )
    .await;
    assert_eq!(kept.status, StatusCode::OK, "{}", kept.body);
    assert_eq!(read_ref(&kept, "o:primary_media"), Some(second));
}
