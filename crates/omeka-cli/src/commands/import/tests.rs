//! Behaviour tests for `romeka import`.

use super::*;
use crate::testing::seeded_db;
use omeka_db::ids::PropertyId;

async fn write(dir: &std::path::Path, name: &str, contents: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, contents).expect("write fixture");
    path
}

async fn title_property(pool: &sqlx::SqlitePool) -> PropertyId {
    crate::terms::resolve_property(&omeka_db::DbPool::from(pool.clone()), "dcterms:title")
        .await
        .expect("dcterms:title")
}

#[tokio::test]
async fn template_import_creates_the_template_and_its_fields() {
    let db = seeded_db().await;
    let title = title_property(&db.pool).await;
    let creator = crate::terms::resolve_property(&db.db_pool, "dcterms:creator")
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
        database: db.path.clone().into(),
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
        database: db.path.clone().into(),
        file,
    })
    .await
    .expect_err("duplicate label");
    assert!(error.to_string().contains("already exists"), "got {error}");
}
