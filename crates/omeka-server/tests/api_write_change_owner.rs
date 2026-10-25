//! `AbstractEntityAdapter::hydrateOwner` (`Api/Adapter/AbstractEntityAdapter.php:859-887`):
//! a body whose `o:owner.o:id` differs from the owner the entity holds before
//! the write is authorized for `change-owner`, which only the two admin roles
//! hold. The check runs before the user lookup, so a refused caller who names
//! a missing user is told 403, not 404.
//!
//! Every answer here was first read from the upstream oracle at `5e413f0`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::Router;
use axum::http::StatusCode;
use common::{append_key, caller, json_request, seeded_pool, send};
use serde_json::{Value, json};

async fn write(app: &Router, method: &str, uri: &str, body: Value) -> (StatusCode, Value) {
    let answer = send(app.clone(), json_request(method, uri, &body.to_string())).await;
    (answer.status, answer.json())
}

fn refusal(class: &str) -> Value {
    json!({"errors": {"error": format!(
        "Permission denied for the current user to change-owner the {class} resource."
    )}})
}

const ITEM: &str = "Omeka\\Entity\\Item";

/// Names the owner: `$newOwnerId !== $oldOwnerId` holds on a create for any
/// numeric id, the caller's own and `0` included, because the new entity holds
/// none. A null or non-numeric id is no change, and the caller owns the row.
#[tokio::test]
async fn hydrate_owner_refuses_an_author_who_names_any_owner_on_create() {
    let (pool, seed) = seeded_pool().await;
    let author = caller(&pool, "author").await;
    let app = common::router(pool);
    let uri = author.uri("/api/items");

    for named in [
        json!(author.id.0),
        json!(0),
        json!(seed.user.0),
        json!(99999),
    ] {
        let (status, body) = write(&app, "POST", &uri, json!({"o:owner": {"o:id": named}})).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "o:id {named}: {body}");
        assert_eq!(body, refusal(ITEM), "o:id {named}");
    }
    for unchanged in [json!({"o:id": null}), json!({"o:id": "abc"}), json!({})] {
        let (status, body) = write(&app, "POST", &uri, json!({"o:owner": unchanged})).await;
        assert_eq!(status, StatusCode::OK, "{unchanged}: {body}");
        assert_eq!(body["o:owner"]["o:id"], author.id.0, "{unchanged}");
    }
}

/// On an update the baseline is the stored owner: naming it again is no change,
/// naming anyone else (or null) is refused, for a `PATCH` as for a `PUT`.
#[tokio::test]
async fn hydrate_owner_lets_an_author_keep_but_not_change_their_items_owner() {
    let (pool, seed) = seeded_pool().await;
    let author = caller(&pool, "author").await;
    let app = common::router(pool);
    let (_, created) = write(&app, "POST", &author.uri("/api/items"), json!({})).await;
    let item = author.uri(&format!("/api/items/{}", created["o:id"]));

    let (status, body) = write(
        &app,
        "PUT",
        &item,
        json!({"o:owner": {"o:id": author.id.0}}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["o:owner"]["o:id"], author.id.0);

    for (method, named) in [
        ("PUT", json!(seed.user.0)),
        ("PUT", Value::Null),
        ("PATCH", json!(seed.user.0)),
    ] {
        let (status, body) = write(&app, method, &item, json!({"o:owner": {"o:id": named}})).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{method} {named}: {body}");
        assert_eq!(body, refusal(ITEM));
    }
    let read = send(
        app,
        common::get(
            &append_key(&format!("/api/items/{}", created["o:id"])),
            false,
        ),
    )
    .await;
    assert_eq!(
        read.json()["o:owner"]["o:id"],
        author.id.0,
        "a refused write changed the owner"
    );
}

/// `AclFactory`: editors and reviewers may update any item, but no rule grants
/// them `change-owner`; the admin roles' blanket allows do. An admin who names
/// a missing user passes the check and then meets `findEntity`'s 404.
#[tokio::test]
async fn hydrate_owner_lets_only_the_admin_roles_change_an_owner() {
    let (pool, seed) = seeded_pool().await;
    let editor = caller(&pool, "editor").await;
    let reviewer = caller(&pool, "reviewer").await;
    let site_admin = caller(&pool, "site_admin").await;
    let app = common::router(pool);
    let path = format!("/api/items/{}", seed.item.0);

    for refused in [&editor, &reviewer] {
        let (status, body) = write(
            &app,
            "PUT",
            &refused.uri(&path),
            json!({"o:owner": {"o:id": refused.id.0}}),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
        assert_eq!(body, refusal(ITEM));
    }

    let (status, body) = write(
        &app,
        "PUT",
        &site_admin.uri(&path),
        json!({"o:owner": {"o:id": editor.id.0}}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["o:owner"]["o:id"], editor.id.0);

    let (status, body) = write(
        &app,
        "PUT",
        &append_key(&path),
        json!({"o:owner": {"o:id": reviewer.id.0}}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["o:owner"]["o:id"], reviewer.id.0);

    let (status, body) = write(
        &app,
        "PUT",
        &append_key(&path),
        json!({"o:owner": {"o:id": 99999}}),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert_eq!(
        body,
        json!({"errors": {"error": "Omeka\\Entity\\User entity with criteria {\"id\":99999} not found"}})
    );
}

/// Every adapter that calls `hydrateOwner` refuses the same way, naming its own
/// entity class: item sets and templates (`AbstractResourceEntityAdapter`,
/// `ResourceTemplateAdapter`) for an author, sites (`SiteAdapter`) for an
/// editor, who may create one.
#[tokio::test]
async fn hydrate_owner_refuses_on_every_owned_entity() {
    let (pool, _seed) = seeded_pool().await;
    let author = caller(&pool, "author").await;
    let editor = caller(&pool, "editor").await;
    let app = common::router(pool);

    let cases = [
        (
            &author,
            "/api/item_sets",
            json!({}),
            "Omeka\\Entity\\ItemSet",
        ),
        (
            &author,
            "/api/resource_templates",
            json!({"o:label": "Owned template"}),
            "Omeka\\Entity\\ResourceTemplate",
        ),
        (
            &editor,
            "/api/sites",
            json!({"o:title": "Owned site", "o:slug": "owned", "o:theme": "default"}),
            "Omeka\\Entity\\Site",
        ),
    ];
    for (who, path, mut body, class) in cases {
        body["o:owner"] = json!({"o:id": who.id.0});
        let (status, answer) = write(&app, "POST", &who.uri(path), body).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{path}: {answer}");
        assert_eq!(answer, refusal(class), "{path}");
    }
}

/// `MediaAdapter::hydrateOwner` gives a new media its item's owner before the
/// parent runs, so naming that owner is no change and anyone else is refused.
/// An inline media in an item create compares against the new item's owner.
#[tokio::test]
async fn media_adapter_compares_a_named_owner_with_the_items_owner() {
    let (pool, seed) = seeded_pool().await;
    let author = caller(&pool, "author").await;
    let app = common::router(pool);
    let (_, item) = write(&app, "POST", &author.uri("/api/items"), json!({})).await;
    let media = |owner: u64| {
        json!({
            "o:ingester": "html",
            "html": "<p>owned</p>",
            "o:item": {"o:id": item["o:id"]},
            "o:owner": {"o:id": owner},
        })
    };

    let (status, body) = write(&app, "POST", &author.uri("/api/media"), media(author.id.0)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["o:owner"]["o:id"], author.id.0);

    let (status, body) = write(&app, "POST", &author.uri("/api/media"), media(seed.user.0)).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(body, refusal("Omeka\\Entity\\Media"));

    let inline = json!({"o:media": [{
        "o:ingester": "html",
        "html": "<p>inline</p>",
        "o:owner": {"o:id": author.id.0},
    }]});
    let (status, body) = write(&app, "POST", &author.uri("/api/items"), inline).await;
    assert_eq!(status, StatusCode::OK, "{body}");
}

const SITE: &str = "Omeka\\Entity\\Site";

/// `SiteAdapter::hydrate` calls `hydrateOwner` on an update too
/// (`SiteAdapter.php:59`). An editor who owns a site may name its stored owner
/// again, and is refused any other owner, a null and a missing user included.
/// A global admin may hand the site to anyone; the editor, still a site admin,
/// may then name the new owner as no change. Measured on the oracle.
#[tokio::test]
async fn site_adapter_checks_change_owner_on_update() {
    let (pool, seed) = seeded_pool().await;
    let editor = caller(&pool, "editor").await;
    let author = caller(&pool, "author").await;
    let app = common::router(pool);
    let (status, created) = write(
        &app,
        "POST",
        &editor.uri("/api/sites"),
        json!({"o:title": "Editor site", "o:slug": "editor-site", "o:theme": "default"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{created}");
    assert_eq!(created["o:owner"]["o:id"], editor.id.0);
    let path = format!("/api/sites/{}", created["o:id"]);

    let (status, body) = write(
        &app,
        "PATCH",
        &editor.uri(&path),
        json!({"o:owner": {"o:id": editor.id.0}}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    for named in [json!(seed.user.0), Value::Null, json!(99999)] {
        let (status, body) = write(
            &app,
            "PATCH",
            &editor.uri(&path),
            json!({"o:owner": {"o:id": named}}),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN, "o:id {named}: {body}");
        assert_eq!(body, refusal(SITE), "o:id {named}");
    }
    let (status, body) = write(
        &app,
        "PATCH",
        &editor.uri(&path),
        json!({"o:title": "Renamed"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["o:owner"]["o:id"], editor.id.0);

    let (status, body) = write(
        &app,
        "PATCH",
        &append_key(&path),
        json!({"o:owner": {"o:id": author.id.0}}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["o:owner"]["o:id"], author.id.0);
    let (status, body) = write(
        &app,
        "PATCH",
        &editor.uri(&path),
        json!({"o:owner": {"o:id": author.id.0}}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["o:owner"]["o:id"], author.id.0);
}

const ANNOTATION: &str = "Omeka\\Entity\\ValueAnnotation";

/// A `dcterms:title` value whose annotation carries `owner` as its `o:owner`
/// member, or no `o:owner` at all.
fn annotated(owner: Option<Value>) -> Value {
    let mut annotation = json!({"dcterms:title": [
        {"property_id": "auto", "type": "literal", "@value": "a note"}
    ]});
    if let Some(owner) = owner {
        annotation["o:owner"] = json!({"o:id": owner});
    }
    json!({"dcterms:title": [
        {"property_id": "auto", "type": "literal", "@value": "Annotated", "@annotation": annotation}
    ]})
}

/// The owners of an item's value annotations, in value order. The item
/// document carries an annotation's values, not its owner
/// (`ValueRepresentation::jsonSerialize`), so the test reads the rows.
async fn annotation_owners(pool: &sqlx::SqlitePool, item: &Value) -> Vec<Option<i64>> {
    sqlx::query_scalar(
        "SELECT r.owner_id FROM value v JOIN resource r ON r.id = v.value_annotation_id
         WHERE v.resource_id = ? ORDER BY v.id",
    )
    .bind(item.as_i64().unwrap())
    .fetch_all(pool)
    .await
    .unwrap()
}

/// `ValueHydrator` reuses the stored values in order and hydrates a reused
/// value's annotation as an UPDATE (`ValueHydrator.php:134-148`), so
/// `hydrateOwner` compares a named owner with the stored annotation's owner,
/// and an annotation body without `o:owner` keeps it. A new annotation is a
/// create, compared with no owner. Measured on the oracle.
#[tokio::test]
async fn value_hydrator_compares_an_annotation_owner_with_the_stored_one() {
    let (pool, seed) = seeded_pool().await;
    let editor = caller(&pool, "editor").await;
    let app = common::router(pool.clone());
    let admin = i64::try_from(seed.user.0).unwrap();
    let (status, created) = write(&app, "POST", &append_key("/api/items"), annotated(None)).await;
    assert_eq!(status, StatusCode::OK, "{created}");
    let item = created["o:id"].clone();
    assert_eq!(annotation_owners(&pool, &item).await, [Some(admin)]);
    let path = editor.uri(&format!("/api/items/{item}"));

    let (status, body) = write(&app, "PUT", &path, annotated(Some(json!(admin)))).await;
    assert_eq!(status, StatusCode::OK, "stored owner named again: {body}");
    let (status, body) = write(&app, "PUT", &path, annotated(Some(json!(editor.id.0)))).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(body, refusal(ANNOTATION));
    let (status, body) = write(&app, "PUT", &path, annotated(None)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        annotation_owners(&pool, &item).await,
        [Some(admin)],
        "an annotation body without o:owner keeps the stored owner"
    );

    let mut two = annotated(None);
    two["dcterms:title"]
        .as_array_mut()
        .unwrap()
        .push(annotated(Some(json!(admin)))["dcterms:title"][0].clone());
    let (status, body) = write(&app, "PUT", &path, two).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "a new annotation: {body}");
    assert_eq!(body, refusal(ANNOTATION));
}
