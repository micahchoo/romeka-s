//! Behaviour tests for `romeka import`.

use super::*;
use crate::testing::{refuse_value, seeded_db, site_items, synced_site, temp_db};
use omeka_db::ids::PropertyId;
use omeka_db::repos::{ItemFilter, Paging, ValueRepo};

async fn write(dir: &std::path::Path, name: &str, contents: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, contents).expect("write fixture");
    path
}

async fn title_property(pool: &sqlx::SqlitePool) -> PropertyId {
    crate::terms::resolve_property(pool, "dcterms:title")
        .await
        .expect("dcterms:title")
}

#[tokio::test]
async fn template_import_creates_the_template_and_its_fields() {
    let db = seeded_db().await;
    let title = title_property(&db.pool).await;
    let creator = crate::terms::resolve_property(&db.pool, "dcterms:creator")
        .await
        .expect("dcterms:creator");
    let json = format!(
        r#"{{"o:label":"Imported","o:resource_template_property":[
            {{"o:property":{{"o:id":{}}}, "o:alternate_label":"Title", "o:is_required":true}},
            {{"o:property":{{"o:id":{}}}, "o:data_type":["literal"]}}
        ]}}"#,
        title.0, creator.0
    );
    let file = write(db.path.parent().expect("dir"), "template.json", &json).await;

    let template = import_template(&TemplateOptions {
        database: db.path.clone(),
        file,
    })
    .await
    .expect("import template");
    assert_eq!(template.label, "Imported");

    let fields = TemplateRepo::new(&db.pool)
        .list_properties(template.id)
        .await
        .expect("list properties");
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0].alternate_label.as_deref(), Some("Title"));
    assert!(fields[0].is_required);
    assert_eq!(fields[1].position, Some(1));
}

#[tokio::test]
async fn template_import_rejects_a_duplicate_label() {
    let db = seeded_db().await;
    let json = r#"{"o:label":"Base Resource"}"#;
    let file = write(db.path.parent().expect("dir"), "dupe.json", json).await;
    let error = import_template(&TemplateOptions {
        database: db.path.clone(),
        file,
    })
    .await
    .expect_err("duplicate label");
    assert!(error.to_string().contains("already exists"), "got {error}");
}

#[tokio::test]
async fn csv_import_creates_items_with_values_in_the_item_set() {
    let db = seeded_db().await;
    let item_set = omeka_db::repos::ItemSetRepo::new(&db.pool)
        .create(&omeka_db::rows::NewItemSet {
            resource: NewResource {
                title: Some("Set".to_string()),
                ..NewResource::default()
            },
            is_open: false,
        })
        .await
        .expect("create item set");
    let csv = "dcterms:title,dcterms:creator\nFirst work,Ada\nSecond work,Alan\n";
    let file = write(db.path.parent().expect("dir"), "items.csv", csv).await;

    let report = import_csv(&CsvOptions {
        database: db.path.clone(),
        file,
        item_set_id: item_set.id().0,
    })
    .await
    .expect("import csv");
    assert_eq!(report.imported, 2);
    assert!(report.errors.is_empty(), "errors: {:?}", report.errors);

    let items = ItemRepo::new(&db.pool);
    let listed = items
        .list(
            &ItemFilter {
                item_set_id: Some(item_set.id()),
                ..ItemFilter::default()
            },
            &Paging::default(),
        )
        .await
        .expect("list items");
    assert_eq!(listed.len(), 2);
    assert_eq!(listed[0].resource.title.as_deref(), Some("First work"));

    let values = ValueRepo::new(&db.pool)
        .list_for_resource(listed[0].resource.id)
        .await
        .expect("values");
    assert_eq!(values.len(), 2);
    assert!(values.iter().any(|v| v.value.as_deref() == Some("Ada")));
}

#[tokio::test]
async fn csv_import_reports_a_short_row_and_keeps_the_rest() {
    let db = seeded_db().await;
    let item_set = omeka_db::repos::ItemSetRepo::new(&db.pool)
        .create(&omeka_db::rows::NewItemSet::default())
        .await
        .expect("create item set");
    let csv = "dcterms:title,dcterms:creator\nGood,Ada\nBroken\nAlso good,Alan\n";
    let file = write(db.path.parent().expect("dir"), "short.csv", csv).await;

    let report = import_csv(&CsvOptions {
        database: db.path.clone(),
        file,
        item_set_id: item_set.id().0,
    })
    .await
    .expect("import csv");
    assert_eq!(report.imported, 2);
    assert_eq!(report.errors.len(), 1);
    assert_eq!(report.errors[0].line, 3);
    assert!(report.errors[0].message.contains("expected 2 columns"));
}

async fn count(pool: &sqlx::SqlitePool, table: &str) -> i64 {
    sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table}"))
        .fetch_one(pool)
        .await
        .expect("count rows")
}

async fn import(db: &crate::testing::TempDb, csv: &str) -> CsvReport {
    let item_set = omeka_db::repos::ItemSetRepo::new(&db.pool)
        .create(&omeka_db::rows::NewItemSet::default())
        .await
        .expect("create item set");
    let file = write(db.path.parent().expect("dir"), "items.csv", csv).await;
    import_csv(&CsvOptions {
        database: db.path.clone(),
        file,
        item_set_id: item_set.id().0,
    })
    .await
    .expect("import csv")
}

#[tokio::test]
async fn csv_import_leaves_nothing_of_a_row_whose_value_fails() {
    let db = seeded_db().await;
    refuse_value(&db.pool, "refused").await;

    let report = import(
        &db,
        "dcterms:title,dcterms:creator\nGood,Ada\nBad,refused\n",
    )
    .await;

    assert_eq!(report.imported, 1);
    assert_eq!(report.errors.len(), 1);
    assert_eq!(report.errors[0].line, 3);
    assert_eq!(count(&db.pool, "item").await, 1, "only the good row's item");
    assert_eq!(count(&db.pool, "item_item_set").await, 1);
    assert_eq!(count(&db.pool, "value").await, 2, "Good and Ada");
    assert_eq!(
        count(&db.pool, "resource").await,
        2,
        "the item set and the good item"
    );
}

#[tokio::test]
async fn csv_import_joins_an_item_to_the_synced_sites_whose_pool_selects_it() {
    let db = seeded_db().await;
    let by_creator = |name: &str| {
        serde_json::json!({"property": [
            {"property": "dcterms:creator", "type": "eq", "text": name}
        ]})
    };
    let ada_site = synced_site(&db.pool, "ada", by_creator("Ada")).await;
    let grace_site = synced_site(&db.pool, "grace", by_creator("Grace")).await;
    let unsynced = omeka_db::repos::SiteRepo::new(&db.pool)
        .create(&omeka_db::rows::NewSite {
            slug: "manual".to_string(),
            title: "Manual".to_string(),
            ..omeka_db::rows::NewSite::default()
        })
        .await
        .expect("create site")
        .id;

    let report = import(
        &db,
        "dcterms:title,dcterms:creator\nFirst,Ada\nSecond,Alan\n",
    )
    .await;
    assert_eq!(report.imported, 2);

    let first = ItemRepo::new(&db.pool)
        .list(&ItemFilter::default(), &Paging::default())
        .await
        .expect("list items")
        .into_iter()
        .find(|item| item.resource.title.as_deref() == Some("First"))
        .expect("First imported")
        .id();
    assert_eq!(site_items(&db.pool, ada_site).await, [first]);
    assert!(site_items(&db.pool, grace_site).await.is_empty());
    assert!(site_items(&db.pool, unsynced).await.is_empty());
}

#[tokio::test]
async fn csv_import_ignores_a_column_whose_header_starts_with_a_hash() {
    let db = seeded_db().await;
    let report = import(
        &db,
        "dcterms:title,#note,dcterms:creator,# also\nFirst,check the date,Ada,x\n",
    )
    .await;
    assert_eq!(report.imported, 1);
    assert!(report.errors.is_empty(), "errors: {:?}", report.errors);
    let values: Vec<String> = sqlx::query_scalar("SELECT value FROM value ORDER BY id")
        .fetch_all(&db.pool)
        .await
        .expect("values");
    assert_eq!(values, ["First", "Ada"]);
}

#[tokio::test]
async fn csv_import_fails_on_an_unknown_header_term() {
    let db = seeded_db().await;
    let item_set = omeka_db::repos::ItemSetRepo::new(&db.pool)
        .create(&omeka_db::rows::NewItemSet::default())
        .await
        .expect("create item set");
    let csv = "nope:title\nvalue\n";
    let file = write(db.path.parent().expect("dir"), "bad.csv", csv).await;
    let error = import_csv(&CsvOptions {
        database: db.path.clone(),
        file,
        item_set_id: item_set.id().0,
    })
    .await
    .expect_err("unknown prefix");
    assert!(
        error.to_string().contains("unknown vocabulary prefix"),
        "got {error}"
    );
}

#[tokio::test]
async fn csv_import_fails_when_the_item_set_is_missing() {
    let db = temp_db().await;
    let file = write(
        db.path.parent().expect("dir"),
        "x.csv",
        "dcterms:title\nA\n",
    )
    .await;
    let error = import_csv(&CsvOptions {
        database: db.path.clone(),
        file,
        item_set_id: 999,
    })
    .await
    .expect_err("missing item set");
    assert!(error.to_string().contains("does not exist"), "got {error}");
}
