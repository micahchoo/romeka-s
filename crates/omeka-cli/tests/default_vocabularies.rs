//! `romeka install`'s default vocabularies against upstream's: the four vocabularies, and every
//! class and property with its label, comment and order, as `InstallDefaultVocabulariesTask`
//! stores them.
//!
//! `fixtures/oracle-default-vocabularies.json` is a fresh upstream install read back through its
//! API (`fixtures/README.md` has the command). A test that sets `ROMEKA_UPGRADE_DB` also runs the
//! upgrade on a copy of that database; the file itself is only read.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};

use omeka_cli::commands::install::{InstallOptions, run};
use omeka_db::open_file;
use omeka_db::repos::{
    ClassFilter, ClassRepo, Paging, PropertyFilter, PropertyRepo, VocabularyRepo,
};
use serde::Deserialize;
use sqlx::SqlitePool;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct Term {
    local_name: String,
    label: String,
    comment: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct Vocabulary {
    prefix: String,
    namespace_uri: String,
    label: String,
    comment: Option<String>,
    classes: Vec<Term>,
    properties: Vec<Term>,
}

fn upstream() -> Vec<Vocabulary> {
    serde_json::from_str(include_str!("fixtures/oracle-default-vocabularies.json")).unwrap()
}

fn options(database: PathBuf) -> InstallOptions {
    InstallOptions {
        database,
        email: "admin@example.org".to_string(),
        name: "Admin".to_string(),
        password: "hunter2".to_string(),
        title: "Test Install".to_string(),
        time_zone: "UTC".to_string(),
        locale: "en".to_string(),
    }
}

/// The four default vocabularies as stored, members in id order.
async fn stored(pool: &SqlitePool) -> Vec<Vocabulary> {
    let mut found = Vec::new();
    for prefix in ["dcterms", "dctype", "bibo", "foaf"] {
        let row = VocabularyRepo::new(pool)
            .get_by_prefix(prefix)
            .await
            .unwrap()
            .unwrap_or_else(|| panic!("{prefix} is installed"));
        let classes = ClassRepo::new(pool)
            .list(
                &ClassFilter {
                    vocabulary_id: Some(row.id),
                    ..ClassFilter::default()
                },
                &Paging::default(),
            )
            .await
            .unwrap()
            .into_iter()
            .map(|class| Term {
                local_name: class.local_name,
                label: class.label,
                comment: class.comment,
            })
            .collect();
        let properties = PropertyRepo::new(pool)
            .list(
                &PropertyFilter {
                    vocabulary_id: Some(row.id),
                    ..PropertyFilter::default()
                },
                &Paging::default(),
            )
            .await
            .unwrap()
            .into_iter()
            .map(|property| Term {
                local_name: property.local_name,
                label: property.label,
                comment: property.comment,
            })
            .collect();
        found.push(Vocabulary {
            prefix: row.prefix,
            namespace_uri: row.namespace_uri,
            label: row.label,
            comment: row.comment,
            classes,
            properties,
        });
    }
    found
}

/// The vocabularies with each member list sorted by local name: an upgrade appends the members
/// it adds, as upstream's Update does, so only a fresh install has upstream's order.
fn unordered(mut vocabularies: Vec<Vocabulary>) -> Vec<Vocabulary> {
    for vocabulary in &mut vocabularies {
        vocabulary
            .classes
            .sort_by(|a, b| a.local_name.cmp(&b.local_name));
        vocabulary
            .properties
            .sort_by(|a, b| a.local_name.cmp(&b.local_name));
    }
    vocabularies
}

/// `InstallDefaultVocabulariesTask`: a fresh install stores what upstream's stores, in its order.
#[tokio::test]
async fn a_fresh_install_stores_upstreams_default_vocabularies() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("romeka.db");
    let report = run(&options(path.clone())).await.unwrap();
    assert_eq!(report.vocabularies_inserted, 4);
    assert_eq!(report.classes_inserted, 105);
    assert_eq!(report.properties_inserted, 184);

    let pool = open_file(&path).await.unwrap();
    assert_eq!(stored(&pool).await, upstream());
}

/// The text an older bundle wrote for every FOAF member: the local name as its label, no
/// comment, and no `givenname`.
async fn write_the_old_foaf_bundle(pool: &SqlitePool) {
    sqlx::query(
        "UPDATE property SET label = local_name, comment = NULL
         WHERE vocabulary_id = (SELECT id FROM vocabulary WHERE prefix = 'foaf')",
    )
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE resource_class SET label = local_name, comment = NULL
         WHERE vocabulary_id = (SELECT id FROM vocabulary WHERE prefix = 'foaf')",
    )
    .execute(pool)
    .await
    .unwrap();
    sqlx::query("DELETE FROM property WHERE local_name = 'givenname'")
        .execute(pool)
        .await
        .unwrap();
}

/// An install from the older bundle is corrected by the next `romeka install`: the missing
/// member is added and every label and comment the bundle left as a bare local name is
/// upstream's. A label someone wrote is kept, and a second run changes nothing.
#[tokio::test]
async fn an_install_from_the_old_bundle_is_corrected_and_an_edited_label_is_kept() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("romeka.db");
    run(&options(path.clone())).await.unwrap();
    let pool = open_file(&path).await.unwrap();
    write_the_old_foaf_bundle(&pool).await;
    sqlx::query("UPDATE property SET label = 'Full name' WHERE local_name = 'name'")
        .execute(&pool)
        .await
        .unwrap();

    let report = run(&options(path.clone())).await.unwrap();
    assert_eq!(report.vocabularies_inserted, 0);
    assert_eq!(report.properties_inserted, 1, "foaf:givenname is added");

    let mut expected = upstream();
    let foaf = expected.iter_mut().find(|v| v.prefix == "foaf").unwrap();
    let name = foaf
        .properties
        .iter_mut()
        .find(|term| term.local_name == "name")
        .unwrap();
    // The edited member keeps all its stored text, the comment the old bundle left empty too.
    name.label = "Full name".to_string();
    name.comment = None;
    assert_eq!(unordered(stored(&pool).await), unordered(expected));

    let before = stored(&pool).await;
    let again = run(&options(path)).await.unwrap();
    assert_eq!((again.classes_inserted, again.properties_inserted), (0, 0));
    assert_eq!(stored(&pool).await, before, "a second run changes nothing");
}

/// The upgrade on a copy of a real archive installed from the older bundle.
#[tokio::test]
async fn a_real_install_from_the_old_bundle_is_corrected() {
    let Some(source) = std::env::var_os("ROMEKA_UPGRADE_DB") else {
        eprintln!("ROMEKA_UPGRADE_DB is not set; skipping the real-database upgrade");
        return;
    };
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("romeka.db");
    std::fs::write(&path, std::fs::read(Path::new(&source)).unwrap()).unwrap();
    let report = run(&options(path.clone())).await.unwrap();
    let pool = open_file(&path).await.unwrap();
    assert_eq!(unordered(stored(&pool).await), unordered(upstream()));
    eprintln!(
        "upgraded a copy of {}: {} classes and {} properties added",
        Path::new(&source).display(),
        report.classes_inserted,
        report.properties_inserted
    );
}
