//! `TermRepo`: the vocabulary prefixes and the property and class terms a request can print, on
//! SQLite and, with the `mysql` feature, on MySQL and MariaDB.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;
mod import_db;

use import_db::on_both;
use omeka_db::repos::{
    ClassRepo, ItemRepo, PropertyRepo, TemplateRepo, TermRepo, TermScope, ValueRepo, VocabularyRepo,
};
use omeka_db::rows::{
    NewClass, NewItem, NewProperty, NewResource, NewTemplate, NewTemplateProperty, NewVocabulary,
    ValueContent,
};

on_both!(the_referenced_scope_keeps_the_terms_a_row_names, |pool| {
    let vocabulary = VocabularyRepo::<Db>::new(pool)
        .create(&NewVocabulary {
            owner_id: None,
            namespace_uri: "http://xmlns.com/foaf/0.1/".into(),
            prefix: "foaf".into(),
            label: "Friend of a Friend".into(),
            comment: None,
        })
        .await
        .unwrap();
    let properties = PropertyRepo::<Db>::new(pool);
    let mut made = Vec::new();
    for local_name in ["givenName", "givenname", "templated", "unused"] {
        made.push(
            properties
                .create(&NewProperty {
                    owner_id: None,
                    vocabulary_id: vocabulary.id,
                    local_name: local_name.into(),
                    label: format!("Label {local_name}"),
                    comment: None,
                })
                .await
                .unwrap(),
        );
    }
    let classes = ClassRepo::<Db>::new(pool);
    let person = classes
        .create(&NewClass {
            owner_id: None,
            vocabulary_id: vocabulary.id,
            local_name: "Person".into(),
            label: "Person".into(),
            comment: None,
        })
        .await
        .unwrap();
    let unused_class = classes
        .create(&NewClass {
            owner_id: None,
            vocabulary_id: vocabulary.id,
            local_name: "Agent".into(),
            label: "Agent".into(),
            comment: None,
        })
        .await
        .unwrap();
    let item = ItemRepo::<Db>::new(pool)
        .create(&NewItem {
            resource: NewResource {
                resource_class_id: Some(person.id),
                ..NewResource::default()
            },
            primary_media_id: None,
        })
        .await
        .unwrap();
    let values = ValueRepo::<Db>::new(pool);
    for property in &made[..2] {
        values
            .create(&ValueContent::literal(property.id, "Ada").on(item.resource.id))
            .await
            .unwrap();
    }
    let templates = TemplateRepo::<Db>::new(pool);
    let template = templates
        .create(&NewTemplate {
            owner_id: None,
            resource_class_id: None,
            title_property_id: None,
            description_property_id: None,
            label: "People".into(),
        })
        .await
        .unwrap();
    templates
        .add_property(&NewTemplateProperty {
            resource_template_id: template.id,
            property_id: made[2].id,
            alternate_label: None,
            alternate_comment: None,
            position: Some(1),
            data_type: None,
            is_required: false,
            is_private: false,
            default_lang: None,
        })
        .await
        .unwrap();

    let terms = TermRepo::<Db>::new(pool);
    assert_eq!(
        terms.prefixes().await.unwrap(),
        [(vocabulary.id, "foaf".to_string())]
    );

    let mut referenced: Vec<_> = terms
        .properties(TermScope::Referenced)
        .await
        .unwrap()
        .into_iter()
        .map(|term| (term.id, term.local_name, term.vocabulary_label))
        .collect();
    referenced.sort();
    assert_eq!(
        referenced,
        [
            (made[0].id, "givenName".into(), "Friend of a Friend".into()),
            (made[1].id, "givenname".into(), "Friend of a Friend".into()),
            (made[2].id, "templated".into(), "Friend of a Friend".into()),
        ],
        "a value or a template names each; the case pair stays two terms"
    );
    let all = terms.properties(TermScope::All).await.unwrap();
    assert_eq!(all.len(), 4);
    let unused = all.iter().find(|term| term.id == made[3].id).unwrap();
    assert_eq!(unused.label, "Label unused");
    assert_eq!(unused.vocabulary_id, vocabulary.id);

    let referenced: Vec<_> = terms
        .classes(TermScope::Referenced)
        .await
        .unwrap()
        .into_iter()
        .map(|term| (term.id, term.vocabulary_id, term.local_name))
        .collect();
    assert_eq!(referenced, [(person.id, vocabulary.id, "Person".into())]);
    let mut all: Vec<_> = terms
        .classes(TermScope::All)
        .await
        .unwrap()
        .into_iter()
        .map(|term| term.id)
        .collect();
    all.sort();
    assert_eq!(all, [person.id, unused_class.id]);
});
