//! The fields and item types a run makes before its records, on the record's transaction
//! (`docs/import-v2.md`, "Run details"): they commit with their ledger row or not at all. Every
//! test runs on SQLite and, with the `mysql` feature, on MySQL.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;
mod import_db;
mod import_writes_db;

use import_db::on_both;
use import_writes_db::world;
use omeka_db::repos::ImportWrites;

on_both!(
    a_made_field_and_type_live_and_die_with_the_transaction,
    |pool| {
        use omeka_db::repos::{PropertyRepo, TemplateRepo};
        use omeka_db::rows::{NewProperty, NewTemplate, NewTemplateProperty};
        let w = world!(pool);
        let vocabulary = PropertyRepo::<Db>::new(pool)
            .get(w.title)
            .await
            .unwrap()
            .unwrap()
            .vocabulary_id;
        let field = NewProperty {
            owner_id: None,
            vocabulary_id: vocabulary,
            local_name: "shelf".to_string(),
            label: "Shelf".to_string(),
            comment: None,
        };
        let template = NewTemplate {
            owner_id: None,
            resource_class_id: None,
            title_property_id: Some(w.title),
            description_property_id: None,
            label: "Shelf list".to_string(),
        };
        let fields = |template| {
            vec![NewTemplateProperty {
                resource_template_id: template,
                property_id: w.title,
                alternate_label: None,
                alternate_comment: None,
                position: Some(1),
                data_type: None,
                is_required: true,
                is_private: false,
                default_lang: None,
            }]
        };
        let mut tx = pool.begin().await.unwrap();
        let mut writes = ImportWrites::<Db>::new(&mut tx);
        let property = writes.create_property(&field).await.unwrap();
        let made = writes
            .create_template(&template, &fields(omeka_db::ids::TemplateId(0)))
            .await
            .unwrap();
        tx.rollback().await.unwrap();
        assert_eq!(
            PropertyRepo::<Db>::new(pool)
                .get(property.id)
                .await
                .unwrap(),
            None
        );
        assert_eq!(
            TemplateRepo::<Db>::new(pool).get(made.id).await.unwrap(),
            None
        );

        let mut tx = pool.begin().await.unwrap();
        let made = ImportWrites::<Db>::new(&mut tx)
            .create_template(&template, &fields(omeka_db::ids::TemplateId(0)))
            .await
            .unwrap();
        tx.commit().await.unwrap();
        let listed = TemplateRepo::<Db>::new(pool)
            .list_properties(made.id)
            .await
            .unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].resource_template_id, made.id);
        assert!(listed[0].is_required);
    }
);
