//! The reads and writes the job runner and the system API make (`romeka job`, the dispatcher,
//! `IndexFulltextSearch`, `/api/jobs`, `/api/assets`, `/api/modules`), on SQLite and, with the
//! `mysql` feature, on MySQL and MariaDB.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;
mod import_db;

use import_db::on_both;
use omeka_db::ids::ResourceId;
use omeka_db::repos::{
    AssetFilter, AssetRepo, FulltextRepo, ItemRepo, JobFilter, JobRepo, ModuleRepo, Paging,
    SettingRepo,
};
use omeka_db::rows::{FulltextSearchRow, NewAsset, NewItem, NewJob};
use serde_json::json;

on_both!(a_dispatched_job_runs_to_its_end, |pool| {
    let jobs = JobRepo::<Db>::new(pool);
    let job = jobs
        .create(&NewJob {
            owner_id: None,
            pid: None,
            status: Some("starting".into()),
            class: "Omeka\\Job\\BatchDelete".into(),
            args: Some(json!({"resource": "items", "ids": [1]})),
            log: None,
            started: None,
        })
        .await
        .unwrap();
    assert_eq!(
        jobs.status(job.id).await.unwrap().as_deref(),
        Some("starting")
    );
    assert!(jobs.start(job.id, "4242").await.unwrap());
    assert!(jobs.set_status(job.id, "in_progress").await.unwrap());
    assert!(jobs.request_stop(job.id).await.unwrap());
    assert!(!jobs.request_stop(job.id).await.unwrap());
    assert!(jobs.append_log(job.id, "one").await.unwrap());
    assert!(jobs.append_log(job.id, "two").await.unwrap());
    assert!(jobs.finish(job.id, "stopped").await.unwrap());
    assert!(jobs.clear_pid(job.id).await.unwrap());

    let row = jobs.get(job.id).await.unwrap().unwrap();
    assert_eq!(row.pid, None);
    assert_eq!(row.status.as_deref(), Some("stopped"));
    assert_eq!(row.log.as_deref(), Some("one\ntwo\n"));
    assert_eq!(row.args, Some(json!({"resource": "items", "ids": [1]})));
    assert!(row.ended.is_some());
    let listed = jobs
        .list(&JobFilter::default(), &Paging::default())
        .await
        .unwrap();
    assert_eq!(listed, [row]);
    assert_eq!(
        jobs.get(omeka_db::ids::JobId(job.id.0 + 1)).await.unwrap(),
        None
    );
});

on_both!(a_setting_the_runner_reads_round_trips, |pool| {
    let settings = SettingRepo::<Db>::new(pool);
    assert_eq!(settings.get("batch_chunk_size").await.unwrap(), None);
    settings.set("batch_chunk_size", &json!(50)).await.unwrap();
    settings
        .set("batch_chunk_size", &json!("75"))
        .await
        .unwrap();
    assert_eq!(
        settings.get("batch_chunk_size").await.unwrap(),
        Some(json!("75"))
    );
});

on_both!(the_fulltext_rebuild_clears_then_saves, |pool| {
    let item = ItemRepo::<Db>::new(pool)
        .create(&NewItem::default())
        .await
        .unwrap();
    let index = FulltextRepo::<Db>::new(pool);
    let row = |text: &str| FulltextSearchRow {
        id: ResourceId(item.resource.id.0),
        resource: "items".into(),
        owner_id: None,
        is_public: true,
        title: Some("Letter".into()),
        text: Some(text.into()),
    };
    index.save(&row("harbour crane")).await.unwrap();
    index.clear().await.unwrap();
    assert!(index.search("items", "harbour").await.unwrap().is_empty());
    index.save(&row("harbour crane")).await.unwrap();
    index.save(&row("lighthouse keeper")).await.unwrap();
    assert!(
        index.search("items", "harbour").await.unwrap().is_empty(),
        "a second save replaces the row"
    );
    let found = index.search("items", "lighthouse").await.unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].id, item.resource.id);
    assert_eq!(index.list().await.unwrap(), [row("lighthouse keeper")]);
});

on_both!(the_system_reads_list_and_get_by_id, |pool| {
    let assets = AssetRepo::<Db>::new(pool);
    let asset = assets
        .create(&NewAsset {
            owner_id: None,
            name: "logo".into(),
            media_type: "image/png".into(),
            storage_id: "logo0001".into(),
            extension: Some("png".into()),
            alt_text: Some("A logo".into()),
        })
        .await
        .unwrap();
    assert_eq!(assets.get(asset.id).await.unwrap(), Some(asset.clone()));
    assert_eq!(
        assets
            .list(&AssetFilter::default(), &Paging::default())
            .await
            .unwrap(),
        [asset]
    );

    let modules = ModuleRepo::<Db>::new(pool);
    modules.install("Mapping", "2.0.0").await.unwrap();
    modules.install("CSVImport", "2.6.0").await.unwrap();
    let ids: Vec<_> = modules
        .list()
        .await
        .unwrap()
        .into_iter()
        .map(|row| row.id)
        .collect();
    assert_eq!(ids, ["CSVImport", "Mapping"]);
    assert_eq!(
        modules.get("Mapping").await.unwrap().unwrap().version,
        "2.0.0"
    );
    assert_eq!(modules.get("Missing").await.unwrap(), None);
});
