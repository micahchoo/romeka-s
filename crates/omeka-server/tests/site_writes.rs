//! Site-write fidelity: the site permission roles, the item site membership,
//! the navigation and homepage validators, the site thumbnail reference, and
//! the `target_blank` a URL navigation link renders.
//!
//! Every test drives the router against a migrated in-memory database, so what
//! it asserts is what an API caller or a site visitor sees over HTTP. The
//! expected statuses and error messages were captured from upstream at
//! `5e413f009c447f4b587803ab23a9cec7d228f6e8` on the oracle; the matrix is in
//! `REPORT.md`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::Router;
use axum::http::StatusCode;
use common::{append_key, get, json_request, seeded_pool, send};
use omeka_db::ids::{ItemId, PageId, SiteId, UserId};
use omeka_db::repos::{ApiKeyRepo, SiteRepo, UserRepo};
use omeka_db::rows::{NewApiKey, NewSite, NewSitePage, NewUser};
use serde_json::{Value, json};
use sqlx::SqlitePool;

/// The credential every throwaway user's key verifies against.
const CREDENTIAL: &str = "site-writes-credential";

/// A 32 character identity from `label`, the shape `api_key.id` takes.
fn identity(label: &str) -> String {
    let mut out: String = label.chars().filter(char::is_ascii_alphanumeric).collect();
    out.truncate(32);
    while out.len() < 32 {
        out.push('0');
    }
    out
}

/// A keyed URI for one user's identity.
fn keyed_for(uri: &str, identity: &str) -> String {
    let separator = if uri.contains('?') { '&' } else { '?' };
    format!("{uri}{separator}key_identity={identity}&key_credential={CREDENTIAL}")
}

/// One user with a role and an API key.
struct Actor {
    name: &'static str,
    id: UserId,
    identity: String,
}

async fn add_actor(pool: &SqlitePool, name: &'static str, role: &str) -> Actor {
    let user = UserRepo::new(pool)
        .create(&NewUser {
            email: format!("{name}@example.com"),
            name: name.to_string(),
            password_hash: None,
            role: role.to_string(),
            is_active: true,
            created: None,
        })
        .await
        .unwrap();
    let key = identity(name);
    ApiKeyRepo::new(pool)
        .create(&NewApiKey {
            id: key.clone(),
            owner_id: user.id,
            label: "site writes".to_string(),
            credential_hash: bcrypt::hash(CREDENTIAL, 4).unwrap(),
            last_ip: None,
            last_accessed: None,
            created: None,
        })
        .await
        .unwrap();
    Actor {
        name,
        id: user.id,
        identity: key,
    }
}

async fn add_site(pool: &SqlitePool, slug: &str, owner: Option<UserId>) -> SiteId {
    SiteRepo::new(pool)
        .create(&NewSite {
            owner_id: owner,
            slug: slug.to_string(),
            theme: "default".to_string(),
            title: slug.to_string(),
            ..NewSite::default()
        })
        .await
        .unwrap()
        .id
}

async fn add_page(pool: &SqlitePool, site: SiteId, slug: &str) -> PageId {
    SiteRepo::new(pool)
        .add_page(&NewSitePage {
            site_id: site,
            slug: slug.to_string(),
            title: slug.to_string(),
            is_public: true,
            ..NewSitePage::default()
        })
        .await
        .unwrap()
        .id
}

/// A site owned by the seeded admin, one user per permission level, and one
/// page every actor can target.
struct Scenario {
    pool: SqlitePool,
    app: Router,
    site: SiteId,
    global_admin: Actor,
    site_admin: Actor,
    site_editor: Actor,
    site_viewer: Actor,
    no_permission: Actor,
}

async fn scenario() -> Scenario {
    let (pool, seed) = seeded_pool().await;
    let site = add_site(&pool, "matrix", Some(seed.user)).await;
    let global_admin = add_actor(&pool, "globalAdmin", "global_admin").await;
    let site_admin = add_actor(&pool, "siteAdmin", "researcher").await;
    let site_editor = add_actor(&pool, "siteEditor", "researcher").await;
    let site_viewer = add_actor(&pool, "siteViewer", "researcher").await;
    let no_permission = add_actor(&pool, "noPermission", "researcher").await;

    let repo = SiteRepo::new(&pool);
    repo.set_permission(site, site_admin.id, "admin")
        .await
        .unwrap();
    repo.set_permission(site, site_editor.id, "editor")
        .await
        .unwrap();
    repo.set_permission(site, site_viewer.id, "viewer")
        .await
        .unwrap();

    Scenario {
        app: common::router(pool.clone()),
        pool,
        site,
        global_admin,
        site_admin,
        site_editor,
        site_viewer,
        no_permission,
    }
}

fn errors(answer: &common::Answer) -> Value {
    answer.json().get("errors").cloned().unwrap_or(Value::Null)
}

/// `AclFactory::addRulesForSites`: a site update needs the owner or a site
/// `admin` permission; a page write needs the owner or a site `editor`. A site
/// `viewer` and a user with no permission write nothing. A global admin who
/// does not own the site writes everything.
#[tokio::test]
async fn site_permissions_decide_the_site_and_page_writes() {
    let scenario = scenario().await;
    let app = scenario.app;

    let rows: Vec<(&Actor, [StatusCode; 4])> = vec![
        (
            &scenario.global_admin,
            [
                StatusCode::OK,
                StatusCode::OK,
                StatusCode::OK,
                StatusCode::OK,
            ],
        ),
        (
            &scenario.site_admin,
            [
                StatusCode::OK,
                StatusCode::OK,
                StatusCode::OK,
                StatusCode::OK,
            ],
        ),
        (
            &scenario.site_editor,
            [
                StatusCode::FORBIDDEN,
                StatusCode::OK,
                StatusCode::OK,
                StatusCode::OK,
            ],
        ),
        (
            &scenario.site_viewer,
            [
                StatusCode::FORBIDDEN,
                StatusCode::FORBIDDEN,
                StatusCode::FORBIDDEN,
                StatusCode::FORBIDDEN,
            ],
        ),
        (
            &scenario.no_permission,
            [
                StatusCode::FORBIDDEN,
                StatusCode::FORBIDDEN,
                StatusCode::FORBIDDEN,
                StatusCode::FORBIDDEN,
            ],
        ),
    ];

    for (actor, expected) in rows {
        let update_target = add_page(&scenario.pool, scenario.site, actor.name).await;
        let delete_target =
            add_page(&scenario.pool, scenario.site, &format!("{}-d", actor.name)).await;
        let pages: Vec<Value> = SiteRepo::new(&scenario.pool)
            .list_pages(scenario.site)
            .await
            .unwrap()
            .into_iter()
            .map(|page| json!({ "o:id": page.id.0 }))
            .collect();

        let site_update = send(
            app.clone(),
            json_request(
                "PUT",
                &keyed_for(&format!("/api/sites/{}", scenario.site.0), &actor.identity),
                &json!({
                    "o:title": "Matrix",
                    "o:slug": "matrix",
                    "o:theme": "default",
                    "o:is_public": true,
                    "o:page": pages,
                })
                .to_string(),
            ),
        )
        .await;

        let page_create = send(
            app.clone(),
            json_request(
                "POST",
                &keyed_for("/api/site_pages", &actor.identity),
                &json!({
                    "o:site": { "o:id": scenario.site.0 },
                    "o:title": "Created",
                    "o:slug": format!("created-{}", actor.name),
                })
                .to_string(),
            ),
        )
        .await;

        let page_update = send(
            app.clone(),
            json_request(
                "PUT",
                &keyed_for(
                    &format!("/api/site_pages/{}", update_target.0),
                    &actor.identity,
                ),
                &json!({ "o:title": "Updated", "o:slug": actor.name }).to_string(),
            ),
        )
        .await;

        let page_delete = send(
            app.clone(),
            common::request(
                "DELETE",
                &keyed_for(
                    &format!("/api/site_pages/{}", delete_target.0),
                    &actor.identity,
                ),
            ),
        )
        .await;

        let observed = [
            site_update.status,
            page_create.status,
            page_update.status,
            page_delete.status,
        ];
        assert_eq!(
            observed, expected,
            "{}: site update, page create, page update, page delete",
            actor.name
        );

        if expected[0] == StatusCode::FORBIDDEN {
            assert_eq!(
                errors(&site_update),
                json!({"error":
                    "Permission denied for the current user to update the \
                     Omeka\\Entity\\Site resource."
                }),
                "{}",
                site_update.body
            );
        }
        if expected[1] == StatusCode::FORBIDDEN {
            assert_eq!(
                errors(&page_create),
                json!({"error":
                    "Permission denied for the current user to add-page the \
                     Omeka\\Entity\\Site resource."
                }),
                "{}",
                page_create.body
            );
        }
        if expected[2] == StatusCode::FORBIDDEN {
            assert_eq!(
                errors(&page_update),
                json!({"error":
                    "Permission denied for the current user to update the \
                     Omeka\\Entity\\SitePage resource."
                }),
                "{}",
                page_update.body
            );
        }
        if expected[3] == StatusCode::FORBIDDEN {
            assert_eq!(
                errors(&page_delete),
                json!({"error":
                    "Permission denied for the current user to delete the \
                     Omeka\\Entity\\SitePage resource."
                }),
                "{}",
                page_delete.body
            );
        }
    }
}

fn title(text: &str) -> Value {
    json!([{ "property_id": "auto", "type": "literal", "@value": text }])
}

/// The `item_site` rows of one item, read as the join table stores them.
async fn membership(pool: &SqlitePool, item: ItemId) -> Vec<u64> {
    sqlx::query_scalar::<_, i64>("SELECT site_id FROM item_site WHERE item_id = ? ORDER BY site_id")
        .bind(item.0 as i64)
        .fetch_all(pool)
        .await
        .unwrap()
        .into_iter()
        .map(|raw| u64::try_from(raw).unwrap())
        .collect()
}

/// `ItemAdapter::hydrate`'s `o:site`: an item created with a site joins it,
/// and a full update replaces the membership.
#[tokio::test]
async fn an_item_write_joins_the_sites_it_names() {
    let (pool, seed) = seeded_pool().await;
    let app = common::router(pool.clone());
    let named = add_site(&pool, "named", Some(seed.user)).await;
    let other = add_site(&pool, "other", Some(seed.user)).await;

    let created = send(
        app.clone(),
        json_request(
            "POST",
            &append_key("/api/items"),
            &json!({
                "o:is_public": true,
                "o:site": [{ "o:id": named.0 }],
                "dcterms:title": title("Joined"),
            })
            .to_string(),
        ),
    )
    .await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.body);
    let id = ItemId(created.json()["o:id"].as_u64().unwrap());
    assert_eq!(membership(&pool, id).await, vec![named.0]);

    // The membership is what the site's own browse queries, so the item shows
    // up there for a caller reading the wire.
    let browse = send(
        app.clone(),
        get(&format!("/api/items?site_id={}", named.0), true),
    )
    .await;
    assert_eq!(browse.status, StatusCode::OK, "{}", browse.body);
    assert!(
        browse
            .json()
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["o:id"].as_u64() == Some(id.0)),
        "the item does not show in its site's browse: {}",
        browse.body
    );

    let updated = send(
        app.clone(),
        json_request(
            "PUT",
            &append_key(&format!("/api/items/{}", id.0)),
            &json!({
                "o:is_public": true,
                "o:site": [{ "o:id": other.0 }],
                "dcterms:title": title("Moved"),
            })
            .to_string(),
        ),
    )
    .await;
    assert_eq!(updated.status, StatusCode::OK, "{}", updated.body);
    assert_eq!(
        membership(&pool, id).await,
        vec![other.0],
        "a full update must replace the site membership"
    );
}

/// `ItemAdapter::hydrate`: a create that does not name `o:site` takes every
/// site where `assign_new_items` is true, and one that names an empty array
/// takes none.
#[tokio::test]
async fn an_item_create_takes_the_sites_that_assign_new_items() {
    let (pool, seed) = seeded_pool().await;
    let app = common::router(pool.clone());
    let auto = add_site(&pool, "auto", Some(seed.user)).await;
    let mut row = SiteRepo::new(&pool).get(auto).await.unwrap().unwrap();
    row.assign_new_items = true;
    SiteRepo::new(&pool).update(&row).await.unwrap();

    let created = send(
        app.clone(),
        json_request(
            "POST",
            &append_key("/api/items"),
            &json!({
                "o:is_public": true,
                "dcterms:title": title("Automatic"),
            })
            .to_string(),
        ),
    )
    .await;
    assert_eq!(created.status, StatusCode::OK, "{}", created.body);
    let id = ItemId(created.json()["o:id"].as_u64().unwrap());
    assert_eq!(
        membership(&pool, id).await,
        vec![auto.0],
        "only the site that assigns new items is joined"
    );

    let opted_out = send(
        app,
        json_request(
            "POST",
            &append_key("/api/items"),
            &json!({
                "o:is_public": true,
                "o:site": [],
                "dcterms:title": title("Opted out"),
            })
            .to_string(),
        ),
    )
    .await;
    assert_eq!(opted_out.status, StatusCode::OK, "{}", opted_out.body);
    let id = ItemId(opted_out.json()["o:id"].as_u64().unwrap());
    assert_eq!(
        membership(&pool, id).await,
        Vec::<u64>::new(),
        "an empty o:site array joins nothing"
    );
}

/// `SiteAdapter::validateNavigation` and `validateEntity`: a navigation tree,
/// item pool or homepage the site cannot hold is a 422 with upstream's
/// messages, and nothing is written.
#[tokio::test]
async fn a_site_write_rejects_navigation_the_site_cannot_hold() {
    let (pool, seed) = seeded_pool().await;
    let app = common::router(pool.clone());
    let site = add_site(&pool, "validated", Some(seed.user)).await;
    let foreign_site = add_site(&pool, "foreign", Some(seed.user)).await;
    let foreign_page = add_page(&pool, foreign_site, "elsewhere").await;
    let own_page = add_page(&pool, site, "own").await;

    let cases: Vec<(&str, Value, Value)> = vec![
        (
            "not a list",
            json!("browse"),
            json!(["Invalid navigation: navigation must be an array"]),
        ),
        (
            "link without a type",
            json!([{ "data": { "label": "x" } }]),
            json!(["Invalid navigation: link missing type"]),
        ),
        (
            "link without data",
            json!([{ "type": "browse" }]),
            json!(["Invalid navigation: link missing data"]),
        ),
        (
            "url without a label",
            json!([{ "type": "url", "data": { "url": "https://x.test" } }]),
            json!([
                "Invalid navigation: URL link missing label",
                "Invalid navigation: invalid link data"
            ]),
        ),
        (
            "url without a url",
            json!([{ "type": "url", "data": { "label": "x" } }]),
            json!([
                "Invalid navigation: URL link missing URL",
                "Invalid navigation: invalid link data"
            ]),
        ),
        (
            "javascript scheme",
            json!([{ "type": "url", "data": { "label": "x", "url": "javascript:alert(1)" } }]),
            json!([
                "Invalid navigation: URL link invalid scheme",
                "Invalid navigation: invalid link data"
            ]),
        ),
        (
            "a repeated page",
            json!([
                { "type": "page", "data": { "id": 1 } },
                { "type": "page", "data": { "id": 1 } }
            ]),
            json!(["Invalid navigation: page links must be unique"]),
        ),
        (
            "a child list that is not a list",
            json!([{ "type": "browse", "data": { "label": "x" }, "links": "no" }]),
            json!(["Invalid navigation: links must be an array"]),
        ),
    ];

    for (name, navigation, expected) in cases {
        let mut body = base_site_body("validated", &pool, site).await;
        body["o:navigation"] = navigation;
        let answer = send(
            app.clone(),
            json_request(
                "PUT",
                &append_key(&format!("/api/sites/{}", site.0)),
                &body.to_string(),
            ),
        )
        .await;
        assert_eq!(
            answer.status,
            StatusCode::UNPROCESSABLE_ENTITY,
            "{name}: {}",
            answer.body
        );
        assert_eq!(
            errors(&answer),
            json!({ "o:navigation": expected }),
            "{name}"
        );
    }

    let mut body = base_site_body("validated", &pool, site).await;
    body["o:item_pool"] = json!("not a list");
    let answer = send(
        app.clone(),
        json_request(
            "PUT",
            &append_key(&format!("/api/sites/{}", site.0)),
            &body.to_string(),
        ),
    )
    .await;
    assert_eq!(
        answer.status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "{}",
        answer.body
    );
    assert_eq!(
        errors(&answer),
        json!({"o:item_pool": ["A site must have item pool data."]})
    );

    let mut body = base_site_body("validated", &pool, site).await;
    body["o:homepage"] = json!({ "o:id": foreign_page.0 });
    let answer = send(
        app.clone(),
        json_request(
            "PUT",
            &append_key(&format!("/api/sites/{}", site.0)),
            &body.to_string(),
        ),
    )
    .await;
    assert_eq!(
        answer.status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "{}",
        answer.body
    );
    assert_eq!(
        errors(&answer),
        json!({"o:homepage": ["A homepage must belong to its parent site."]})
    );

    let mut body = base_site_body("validated", &pool, site).await;
    body["o:navigation"] = json!([{ "type": "browse", "data": { "label": "Browse" } }]);
    body["o:homepage"] = json!({ "o:id": own_page.0 });
    let answer = send(
        app,
        json_request(
            "PUT",
            &append_key(&format!("/api/sites/{}", site.0)),
            &body.to_string(),
        ),
    )
    .await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    assert_eq!(answer.json()["o:homepage"]["o:id"], own_page.0);
}

/// A site update body that keeps the site's existing pages, so a full `PUT`
/// does not remove the page a case names as its homepage.
async fn base_site_body(slug: &str, pool: &SqlitePool, site: SiteId) -> Value {
    let pages: Vec<Value> = SiteRepo::new(pool)
        .list_pages(site)
        .await
        .unwrap()
        .into_iter()
        .map(|page| json!({ "o:id": page.id.0 }))
        .collect();
    json!({
        "o:title": slug,
        "o:slug": slug,
        "o:theme": "default",
        "o:is_public": true,
        "o:page": pages,
    })
}

/// `SiteAdapter::hydrate`'s `hydrateThumbnail`: `o:thumbnail.o:id` is stored as
/// the site's `thumbnail_id`, and a full `PUT` without the member clears it.
#[tokio::test]
async fn a_site_write_stores_the_thumbnail_reference() {
    let (pool, seed) = seeded_pool().await;
    let app = common::router(pool.clone());
    let site = add_site(&pool, "thumbed", Some(seed.user)).await;
    let asset: i64 = sqlx::query_scalar(
        "INSERT INTO asset (owner_id, name, media_type, storage_id, extension, alt_text) \
         VALUES (?, ?, ?, ?, ?, NULL) RETURNING id",
    )
    .bind(seed.user.0 as i64)
    .bind("thumb.png")
    .bind("image/png")
    .bind("site-writes-thumb")
    .bind("png")
    .fetch_one(&pool)
    .await
    .unwrap();

    let mut body = base_site_body("thumbed", &pool, site).await;
    body["o:thumbnail"] = json!({ "o:id": asset });
    let answer = send(
        app.clone(),
        json_request(
            "PUT",
            &append_key(&format!("/api/sites/{}", site.0)),
            &body.to_string(),
        ),
    )
    .await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    assert_eq!(answer.json()["o:thumbnail"]["o:id"], asset);

    let read = send(app.clone(), get(&format!("/api/sites/{}", site.0), true)).await;
    assert_eq!(read.json()["o:thumbnail"]["o:id"], asset);

    let cleared = send(
        app,
        json_request(
            "PUT",
            &append_key(&format!("/api/sites/{}", site.0)),
            &base_site_body("thumbed", &pool, site).await.to_string(),
        ),
    )
    .await;
    assert_eq!(cleared.status, StatusCode::OK, "{}", cleared.body);
    assert_eq!(cleared.json()["o:thumbnail"], Value::Null);
}

/// `Link/Url.php::toZend`: a URL navigation link marked `target_blank` renders
/// its anchor with `target="_blank"`.
#[tokio::test]
async fn a_url_navigation_link_opens_in_a_new_tab() {
    let (pool, seed) = common::site_pool().await;
    let site = SiteId::from(seed.site);
    let repo = SiteRepo::new(&pool);
    let welcome = repo
        .list_pages(site)
        .await
        .unwrap()
        .into_iter()
        .find(|page| page.slug == "welcome")
        .expect("the seeded site has a welcome page");
    let mut row = repo.get(site).await.unwrap().unwrap();
    row.navigation = json!([{
        "type": "page",
        "data": { "id": welcome.id.0, "label": null },
        "links": [{
            "type": "url",
            "data": {
                "label": "Elsewhere",
                "url": "https://example.com/x",
                "target_blank": true
            }
        }]
    }]);
    repo.update(&row).await.unwrap();

    let answer = send(common::router(pool), get("/s/museum/page/welcome", false)).await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    assert!(
        answer
            .body
            .contains("<a href=\"https://example.com/x\" target=\"_blank\">Elsewhere</a>"),
        "{}",
        answer.body
    );
}
