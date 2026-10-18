//! `module`: the install, activate, upgrade and uninstall writes, the list and the unique key.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use omeka_db::repos::ModuleRepo;

#[tokio::test]
async fn installing_a_module_writes_an_active_row() {
    let pool = common::memory().await;
    let modules = ModuleRepo::new(&pool);

    assert_eq!(modules.get("OmekaModuleFoo").await.unwrap(), None);
    let installed = modules.install("OmekaModuleFoo", "1.2.0").await.unwrap();
    assert!(installed.is_active, "a new install is active");
    assert_eq!(
        modules.get("OmekaModuleFoo").await.unwrap(),
        Some(installed.clone())
    );
    assert_eq!(modules.list().await.unwrap(), vec![installed]);
}

#[tokio::test]
async fn a_module_deactivates_and_upgrades() {
    let pool = common::memory().await;
    let modules = ModuleRepo::new(&pool);
    modules.install("OmekaModuleFoo", "1.2.0").await.unwrap();

    assert!(modules.set_active("OmekaModuleFoo", false).await.unwrap());
    let inactive = modules.get("OmekaModuleFoo").await.unwrap().unwrap();
    assert!(!inactive.is_active);
    assert_eq!(inactive.version, "1.2.0", "deactivating keeps the version");

    assert!(
        modules
            .set_version("OmekaModuleFoo", "1.3.0")
            .await
            .unwrap()
    );
    assert_eq!(
        modules
            .get("OmekaModuleFoo")
            .await
            .unwrap()
            .unwrap()
            .version,
        "1.3.0"
    );
    assert!(
        !modules
            .set_version("OmekaModuleBar", "1.0.0")
            .await
            .unwrap(),
        "no row means no upgrade"
    );
}

#[tokio::test]
async fn the_list_is_in_id_order() {
    let pool = common::memory().await;
    let modules = ModuleRepo::new(&pool);
    modules.install("OmekaModuleZeta", "1.0.0").await.unwrap();
    modules.install("OmekaModuleAlpha", "2.0.0").await.unwrap();

    assert_eq!(
        modules
            .list()
            .await
            .unwrap()
            .iter()
            .map(|row| row.id.as_str())
            .collect::<Vec<_>>(),
        vec!["OmekaModuleAlpha", "OmekaModuleZeta"]
    );
}

#[tokio::test]
async fn installing_the_same_module_twice_is_refused() {
    let pool = common::memory().await;
    let modules = ModuleRepo::new(&pool);
    modules.install("OmekaModuleFoo", "1.2.0").await.unwrap();

    let error = modules
        .install("OmekaModuleFoo", "1.2.0")
        .await
        .unwrap_err();
    assert!(error.is_unique_violation(), "{error}");
}

#[tokio::test]
async fn uninstalling_removes_the_row() {
    let pool = common::memory().await;
    let modules = ModuleRepo::new(&pool);
    modules.install("OmekaModuleFoo", "1.2.0").await.unwrap();

    assert!(modules.delete("OmekaModuleFoo").await.unwrap());
    assert_eq!(modules.get("OmekaModuleFoo").await.unwrap(), None);
    assert!(!modules.delete("OmekaModuleFoo").await.unwrap());
}
