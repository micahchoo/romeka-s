//! Search behaviour: the FTS5 index queries and the advanced query filters.
//!
//! The upstream sources are `Module::searchFulltext` (the `fulltext_search`
//! match), `Stdlib\FulltextSearch::save` and `::delete` (the write half), and
//! `AbstractResourceEntityAdapter::buildPropertyQuery` plus
//! `ItemAdapter::buildQuery` (the property, item-set, site and media filters).

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use omeka_db::ids::{PropertyId, ResourceId, SiteId};
use omeka_db::repos::{
    FulltextRepo, ItemFilter, ItemRepo, ItemSetFilter, ItemSetRepo, Joiner, MatchKind, MediaRepo,
    MediaVisibility, Paging, PropertyRepo, QueryFilter, SiteRepo, ValueQuery, ValueRepo,
    VocabularyRepo,
};
use omeka_db::rows::{
    FulltextSearchRow, NewItem, NewItemSet, NewMedia, NewProperty, NewResource, NewSite, NewValue,
    NewVocabulary,
};
use sqlx::SqlitePool;

/// One item with a literal title value on `dcterms:title`.
async fn item_with_title(pool: &SqlitePool, title: &str, property: PropertyId) -> ResourceId {
    let item = ItemRepo::new(pool)
        .create(&NewItem {
            resource: NewResource {
                title: Some(title.to_string()),
                ..NewResource::default()
            },
            primary_media_id: None,
        })
        .await
        .unwrap();
    ValueRepo::new(pool)
        .create(&NewValue {
            resource_id: item.resource.id,
            property_id: property,
            value_resource_id: None,
            value_annotation_id: None,
            r#type: "literal".to_string(),
            lang: None,
            value: Some(title.to_string()),
            uri: None,
            is_public: true,
        })
        .await
        .unwrap();
    item.resource.id
}

/// A `dcterms:title` property plus a `dcterms:description` property.
async fn properties(pool: &SqlitePool) -> (PropertyId, PropertyId) {
    let vocabulary = VocabularyRepo::new(pool)
        .create(&NewVocabulary {
            owner_id: None,
            namespace_uri: "http://purl.org/dc/terms/".to_string(),
            prefix: "dcterms".to_string(),
            label: "Dublin Core".to_string(),
            comment: None,
        })
        .await
        .unwrap();
    let repo = PropertyRepo::new(pool);
    let mut ids = Vec::new();
    for (local_name, label) in [("title", "Title"), ("description", "Description")] {
        ids.push(
            repo.create(&NewProperty {
                owner_id: None,
                vocabulary_id: vocabulary.id,
                local_name: local_name.to_string(),
                label: label.to_string(),
                comment: None,
            })
            .await
            .unwrap()
            .id,
        );
    }
    (ids[0], ids[1])
}

async fn index(pool: &SqlitePool, id: ResourceId, resource: &str, title: &str, text: &str) {
    FulltextRepo::new(pool)
        .save(&FulltextSearchRow {
            id,
            resource: resource.to_string(),
            owner_id: None,
            is_public: true,
            title: Some(title.to_string()),
            text: Some(text.to_string()),
        })
        .await
        .unwrap();
}

fn contains(text: &str) -> ValueQuery {
    ValueQuery {
        property: None,
        kind: MatchKind::In,
        negated: false,
        text: text.to_string(),
        joiner: Joiner::And,
    }
}

#[tokio::test]
async fn fulltext_match_finds_a_row_by_a_word_in_its_title_or_text() {
    let pool = common::memory().await;
    index(
        &pool,
        ResourceId(1),
        "items",
        "Bronze helmet",
        "armour gallery",
    )
    .await;
    index(
        &pool,
        ResourceId(2),
        "items",
        "Silver cup",
        "drinking vessel",
    )
    .await;

    let repo = FulltextRepo::new(&pool);
    let by_title = repo.search("items", "helmet").await.unwrap();
    assert_eq!(by_title.len(), 1);
    assert_eq!(by_title[0].id, ResourceId(1));
    let by_text = repo.search("items", "drinking").await.unwrap();
    assert_eq!(by_text.len(), 1);
    assert_eq!(by_text[0].id, ResourceId(2));
    assert!(repo.search("items", "absent").await.unwrap().is_empty());
}

#[tokio::test]
async fn fulltext_match_keeps_any_word_of_a_multi_word_query() {
    let pool = common::memory().await;
    index(&pool, ResourceId(1), "items", "Bronze helmet", "armour").await;
    index(&pool, ResourceId(2), "items", "Silver cup", "vessel").await;
    index(&pool, ResourceId(3), "items", "Iron sword", "weapon").await;

    let matches = FulltextRepo::new(&pool)
        .search("items", "helmet cup")
        .await
        .unwrap();
    let ids: Vec<u64> = matches.iter().map(|hit| hit.id.0).collect();
    assert!(ids.contains(&1) && ids.contains(&2), "ids were {ids:?}");
    assert!(!ids.contains(&3), "ids were {ids:?}");
}

#[tokio::test]
async fn fulltext_match_ranks_the_closer_row_first() {
    let pool = common::memory().await;
    index(
        &pool,
        ResourceId(1),
        "items",
        "Notes",
        "bronze bronze bronze bronze bronze",
    )
    .await;
    index(&pool, ResourceId(2), "items", "Bronze helmet", "one bronze").await;

    let matches = FulltextRepo::new(&pool)
        .search("items", "bronze")
        .await
        .unwrap();
    assert_eq!(matches.len(), 2);
    assert!(
        matches[0].score >= matches[1].score,
        "scores were {matches:?}"
    );
}

#[tokio::test]
async fn fulltext_match_is_scoped_to_one_resource_name() {
    let pool = common::memory().await;
    index(&pool, ResourceId(1), "items", "Bronze helmet", "").await;
    index(&pool, ResourceId(2), "item_sets", "Bronze collection", "").await;

    let repo = FulltextRepo::new(&pool);
    assert_eq!(repo.search("items", "bronze").await.unwrap().len(), 1);
    assert_eq!(repo.search("item_sets", "bronze").await.unwrap().len(), 1);
    assert_eq!(repo.search("media", "bronze").await.unwrap().len(), 0);
}

#[tokio::test]
async fn fulltext_save_overwrites_one_resource_row_and_delete_removes_it() {
    let pool = common::memory().await;
    let repo = FulltextRepo::new(&pool);
    index(&pool, ResourceId(1), "items", "First title", "first text").await;
    index(&pool, ResourceId(1), "items", "Second title", "second text").await;

    assert!(repo.search("items", "first").await.unwrap().is_empty());
    assert_eq!(repo.search("items", "second").await.unwrap().len(), 1);

    assert!(repo.delete(ResourceId(1), "items").await.unwrap());
    assert!(repo.search("items", "second").await.unwrap().is_empty());
    assert!(!repo.delete(ResourceId(1), "items").await.unwrap());
}

#[tokio::test]
async fn property_filter_contains_matches_a_literal_value() {
    let pool = common::memory().await;
    let (title, _) = properties(&pool).await;
    let hit = item_with_title(&pool, "Bronze helmet", title).await;
    item_with_title(&pool, "Silver cup", title).await;

    let rows = ItemRepo::new(&pool)
        .list_filtered(
            &ItemFilter::default(),
            &QueryFilter {
                value: vec![contains("helmet")],
                ..QueryFilter::default()
            },
            &Paging::default(),
        )
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].resource.id, hit);
}

#[tokio::test]
async fn property_filter_narrows_to_the_named_property() {
    let pool = common::memory().await;
    let (title, description) = properties(&pool).await;
    let item = item_with_title(&pool, "Bronze helmet", title).await;
    ValueRepo::new(&pool)
        .create(&NewValue {
            resource_id: item,
            property_id: description,
            value_resource_id: None,
            value_annotation_id: None,
            r#type: "literal".to_string(),
            lang: None,
            value: Some("Excavated at Vindolanda".to_string()),
            uri: None,
            is_public: true,
        })
        .await
        .unwrap();

    let rows = ItemRepo::new(&pool)
        .list_filtered(
            &ItemFilter::default(),
            &QueryFilter {
                value: vec![ValueQuery {
                    property: Some(description),
                    kind: MatchKind::In,
                    negated: false,
                    text: "Vindolanda".to_string(),
                    joiner: Joiner::And,
                }],
                ..QueryFilter::default()
            },
            &Paging::default(),
        )
        .await
        .unwrap();
    assert_eq!(rows.len(), 1, "the description holds the word");

    let rows = ItemRepo::new(&pool)
        .list_filtered(
            &ItemFilter::default(),
            &QueryFilter {
                value: vec![ValueQuery {
                    property: Some(title),
                    kind: MatchKind::In,
                    negated: false,
                    text: "Vindolanda".to_string(),
                    joiner: Joiner::And,
                }],
                ..QueryFilter::default()
            },
            &Paging::default(),
        )
        .await
        .unwrap();
    assert!(rows.is_empty(), "the title does not hold the word");
}

#[tokio::test]
async fn negative_property_filter_excludes_a_resource_with_that_value() {
    let pool = common::memory().await;
    let (title, _) = properties(&pool).await;
    let plain = item_with_title(&pool, "Silver cup", title).await;
    item_with_title(&pool, "Bronze helmet", title).await;

    let rows = ItemRepo::new(&pool)
        .list_filtered(
            &ItemFilter::default(),
            &QueryFilter {
                value: vec![ValueQuery {
                    property: None,
                    kind: MatchKind::In,
                    negated: true,
                    text: "bronze".to_string(),
                    joiner: Joiner::And,
                }],
                ..QueryFilter::default()
            },
            &Paging::default(),
        )
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].resource.id, plain);
}

#[tokio::test]
async fn item_set_membership_filters_both_ways() {
    let pool = common::memory().await;
    let member = ItemRepo::new(&pool)
        .create(&NewItem::default())
        .await
        .unwrap();
    let loose = ItemRepo::new(&pool)
        .create(&NewItem::default())
        .await
        .unwrap();
    let set = ItemSetRepo::new(&pool)
        .create(&NewItemSet::default())
        .await
        .unwrap();
    ItemRepo::new(&pool)
        .add_to_item_set(member.id(), set.id())
        .await
        .unwrap();

    let repo = ItemRepo::new(&pool);
    let inside = repo
        .list_filtered(
            &ItemFilter::default(),
            &QueryFilter {
                item_set_ids: vec![set.id()],
                ..QueryFilter::default()
            },
            &Paging::default(),
        )
        .await
        .unwrap();
    assert_eq!(inside.len(), 1);
    assert_eq!(inside[0].id(), member.id());

    let outside = repo
        .list_filtered(
            &ItemFilter::default(),
            &QueryFilter {
                not_item_set_ids: vec![set.id()],
                ..QueryFilter::default()
            },
            &Paging::default(),
        )
        .await
        .unwrap();
    assert_eq!(outside.len(), 1);
    assert_eq!(outside[0].id(), loose.id());
}

#[tokio::test]
async fn has_media_filter_selects_items_with_media() {
    let pool = common::memory().await;
    let with = ItemRepo::new(&pool)
        .create(&NewItem::default())
        .await
        .unwrap();
    let without = ItemRepo::new(&pool)
        .create(&NewItem::default())
        .await
        .unwrap();
    MediaRepo::new(&pool)
        .create(&NewMedia {
            resource: NewResource::default(),
            item_id: with.id(),
            ..NewMedia::default()
        })
        .await
        .unwrap();

    let repo = ItemRepo::new(&pool);
    let rows = repo
        .list_filtered(
            &ItemFilter::default(),
            &QueryFilter {
                has_media: Some(true),
                ..QueryFilter::default()
            },
            &Paging::default(),
        )
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id(), with.id());

    let rows = repo
        .list_filtered(
            &ItemFilter::default(),
            &QueryFilter {
                has_media: Some(false),
                ..QueryFilter::default()
            },
            &Paging::default(),
        )
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id(), without.id());
}

#[tokio::test]
async fn site_filter_selects_items_attached_to_a_site() {
    let pool = common::memory().await;
    let attached = ItemRepo::new(&pool)
        .create(&NewItem::default())
        .await
        .unwrap();
    let loose = ItemRepo::new(&pool)
        .create(&NewItem::default())
        .await
        .unwrap();
    let site = SiteRepo::new(&pool)
        .create(&NewSite {
            slug: "museum".to_string(),
            theme: "default".to_string(),
            title: "Museum".to_string(),
            ..NewSite::default()
        })
        .await
        .unwrap();
    sqlx::query("INSERT INTO item_site (item_id, site_id) VALUES (?, ?)")
        .bind(i64::try_from(attached.id().0).unwrap())
        .bind(i64::try_from(site.id.0).unwrap())
        .execute(&pool)
        .await
        .unwrap();

    let repo = ItemRepo::new(&pool);
    let rows = repo
        .list_filtered(
            &ItemFilter::default(),
            &QueryFilter {
                site_id: Some(SiteId(site.id.0)),
                ..QueryFilter::default()
            },
            &Paging::default(),
        )
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id(), attached.id());

    let rows = repo
        .list_filtered(
            &ItemFilter::default(),
            &QueryFilter {
                in_sites: Some(false),
                ..QueryFilter::default()
            },
            &Paging::default(),
        )
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id(), loose.id());
}

#[tokio::test]
async fn item_set_site_filter_selects_sets_assigned_to_a_site() {
    let pool = common::memory().await;
    let assigned = ItemSetRepo::new(&pool)
        .create(&NewItemSet::default())
        .await
        .unwrap();
    ItemSetRepo::new(&pool)
        .create(&NewItemSet::default())
        .await
        .unwrap();
    let site = SiteRepo::new(&pool)
        .create(&NewSite {
            slug: "museum".to_string(),
            theme: "default".to_string(),
            title: "Museum".to_string(),
            ..NewSite::default()
        })
        .await
        .unwrap();
    SiteRepo::new(&pool)
        .add_item_set(site.id, assigned.id(), Some(1))
        .await
        .unwrap();

    let rows = ItemSetRepo::new(&pool)
        .list_filtered(
            &ItemSetFilter::default(),
            &QueryFilter {
                site_id: Some(SiteId(site.id.0)),
                ..QueryFilter::default()
            },
            &Paging::default(),
        )
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id(), assigned.id());
}

#[tokio::test]
async fn media_site_filter_follows_the_item_attachment() {
    let pool = common::memory().await;
    let item = ItemRepo::new(&pool)
        .create(&NewItem::default())
        .await
        .unwrap();
    let media = MediaRepo::new(&pool)
        .create(&NewMedia {
            resource: NewResource::default(),
            item_id: item.id(),
            ..NewMedia::default()
        })
        .await
        .unwrap();
    let site = SiteRepo::new(&pool)
        .create(&NewSite {
            slug: "museum".to_string(),
            theme: "default".to_string(),
            title: "Museum".to_string(),
            ..NewSite::default()
        })
        .await
        .unwrap();
    sqlx::query("INSERT INTO item_site (item_id, site_id) VALUES (?, ?)")
        .bind(i64::try_from(item.id().0).unwrap())
        .bind(i64::try_from(site.id.0).unwrap())
        .execute(&pool)
        .await
        .unwrap();

    let rows = MediaRepo::new(&pool)
        .list_filtered(
            &Default::default(),
            &QueryFilter {
                site_id: Some(SiteId(site.id.0)),
                ..QueryFilter::default()
            },
            &Paging::default(),
        )
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id(), media.id());
}

#[tokio::test]
async fn media_property_filter_matches_the_media_values() {
    let pool = common::memory().await;
    let (title, _) = properties(&pool).await;
    let item = ItemRepo::new(&pool)
        .create(&NewItem::default())
        .await
        .unwrap();
    let media = MediaRepo::new(&pool)
        .create(&NewMedia {
            resource: NewResource::default(),
            item_id: item.id(),
            ..NewMedia::default()
        })
        .await
        .unwrap();
    ValueRepo::new(&pool)
        .create(&NewValue {
            resource_id: media.resource.id,
            property_id: title,
            value_resource_id: None,
            value_annotation_id: None,
            r#type: "literal".to_string(),
            lang: None,
            value: Some("Bronze helmet".to_string()),
            uri: None,
            is_public: true,
        })
        .await
        .unwrap();

    let rows = MediaRepo::new(&pool)
        .list_filtered(
            &Default::default(),
            &QueryFilter {
                value: vec![contains("helmet")],
                ..QueryFilter::default()
            },
            &Paging::default(),
        )
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id(), media.id());
}

#[tokio::test]
async fn resource_value_filter_matches_the_referenced_resource_title() {
    let pool = common::memory().await;
    let (title, _) = properties(&pool).await;
    let target = item_with_title(&pool, "Bronze helmet", title).await;
    let source = ItemRepo::new(&pool)
        .create(&NewItem::default())
        .await
        .unwrap();
    ValueRepo::new(&pool)
        .create(&NewValue {
            resource_id: source.resource.id,
            property_id: title,
            value_resource_id: Some(target),
            value_annotation_id: None,
            r#type: "resource".to_string(),
            lang: None,
            value: None,
            uri: None,
            is_public: true,
        })
        .await
        .unwrap();

    let rows = ItemRepo::new(&pool)
        .list_filtered(
            &ItemFilter::default(),
            &QueryFilter {
                value: vec![ValueQuery {
                    property: None,
                    kind: MatchKind::Eq,
                    negated: false,
                    text: "Bronze helmet".to_string(),
                    joiner: Joiner::And,
                }],
                ..QueryFilter::default()
            },
            &Paging::default(),
        )
        .await
        .unwrap();
    // The target matches through its own title value and the source through the
    // reference, which is what `eq`'s `valueResource IN (SELECT id ... title = ?)`
    // reaches.
    let ids: Vec<u64> = rows.iter().map(|row| row.id().0).collect();
    assert!(ids.contains(&source.resource.id.0), "ids were {ids:?}");
    assert!(ids.contains(&target.0), "ids were {ids:?}");
}

#[tokio::test]
async fn has_media_filter_counts_only_the_media_the_caller_can_see() {
    let pool = common::memory().await;
    let item = ItemRepo::new(&pool)
        .create(&NewItem::default())
        .await
        .unwrap();
    MediaRepo::new(&pool)
        .create(&NewMedia {
            resource: NewResource {
                is_public: false,
                ..NewResource::default()
            },
            item_id: item.id(),
            ..NewMedia::default()
        })
        .await
        .unwrap();

    let filter = |visibility| QueryFilter {
        has_media: Some(true),
        media_visibility: visibility,
        ..QueryFilter::default()
    };
    let anonymous = ItemRepo::new(&pool)
        .list_filtered(
            &ItemFilter::default(),
            &filter(MediaVisibility::Actor(None)),
            &Paging::default(),
        )
        .await
        .unwrap();
    assert!(anonymous.is_empty(), "a private media is invisible");

    let privileged = ItemRepo::new(&pool)
        .list_filtered(
            &ItemFilter::default(),
            &filter(MediaVisibility::All),
            &Paging::default(),
        )
        .await
        .unwrap();
    assert_eq!(privileged.len(), 1, "view-all counts every media row");
}

#[tokio::test]
async fn empty_query_filter_leaves_the_plain_list_alone() {
    let pool = common::memory().await;
    let (title, _) = properties(&pool).await;
    item_with_title(&pool, "One", title).await;
    let plain = ItemRepo::new(&pool)
        .list(&ItemFilter::default(), &Paging::default())
        .await
        .unwrap();
    let filtered = ItemRepo::new(&pool)
        .list_filtered(
            &ItemFilter::default(),
            &QueryFilter::default(),
            &Paging::default(),
        )
        .await
        .unwrap();
    assert_eq!(plain.len(), filtered.len());
}

#[tokio::test]
async fn query_filter_is_empty_only_without_parameters() {
    assert!(QueryFilter::default().is_empty());
    assert!(
        !QueryFilter {
            value: vec![contains("x")],
            ..QueryFilter::default()
        }
        .is_empty()
    );
    assert!(
        !QueryFilter {
            has_media: Some(true),
            ..QueryFilter::default()
        }
        .is_empty()
    );
}
