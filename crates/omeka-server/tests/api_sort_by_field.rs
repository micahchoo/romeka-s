//! `sort_by=<term>` and the label sorts order a list in shelf order.
//!
//! Upstream `AbstractResourceEntityAdapter::sortQuery` orders by a property's
//! values (`GROUP_CONCAT` in value id order), by `resource_class_label`, by
//! `resource_template_label` and by `owner_name`. The port compares the text in
//! natural order: a run of digits compares as a number and the rest folds case,
//! so `AF_2` comes before `AF_10`. A resource with no such text sorts last in
//! both directions, and a tie falls back to the id in the requested direction,
//! as upstream's trailing `addOrderBy('omeka_root.id', $sort_order)` does.
//! `docs/compat.md` records where that differs from upstream's byte order.
//!
//! The values that decide the order are the ones the caller may see
//! (`ValueVisibilityFilter`): a private value never moves a row for a caller
//! who cannot read it.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::{Answer, Seed, caller, get, header, router, seeded_pool, send};
use omeka_db::ids::{ClassId, ItemId, PropertyId, TemplateId, UserId};
use omeka_db::repos::{
    ClassRepo, ItemRepo, ItemSetRepo, MediaRepo, PropertyRepo, TemplateRepo, ValueRepo,
};
use omeka_db::rows::{
    NewClass, NewItem, NewItemSet, NewMedia, NewProperty, NewResource, NewTemplate, NewValue,
};
use sqlx::SqlitePool;

/// The seeded database, `dcterms:identifier`, and an empty item set that
/// narrows every items list to the rows a test makes.
struct Shelf {
    pool: SqlitePool,
    seed: Seed,
    identifier: PropertyId,
    set: u64,
}

async fn shelf() -> Shelf {
    let (pool, seed) = seeded_pool().await;
    let identifier = PropertyRepo::new(&pool)
        .create(&NewProperty {
            owner_id: None,
            vocabulary_id: seed.vocabulary,
            local_name: "identifier".to_string(),
            label: "Identifier".to_string(),
            comment: None,
        })
        .await
        .unwrap()
        .id;
    let set = ItemSetRepo::new(&pool)
        .create(&NewItemSet {
            resource: NewResource {
                owner_id: Some(seed.user),
                title: Some("Shelf".to_string()),
                is_public: true,
                ..NewResource::default()
            },
            is_open: false,
        })
        .await
        .unwrap()
        .id()
        .0;
    Shelf {
        pool,
        seed,
        identifier,
        set,
    }
}

/// One item on the shelf.
struct Book<'a> {
    owner: UserId,
    title: Option<&'a str>,
    class: Option<ClassId>,
    template: Option<TemplateId>,
    /// Each identifier value and whether it is public.
    identifiers: &'a [(&'a str, bool)],
}

impl Shelf {
    fn book(&self) -> Book<'static> {
        Book {
            owner: self.seed.user,
            title: None,
            class: None,
            template: None,
            identifiers: &[],
        }
    }

    async fn add(&self, book: Book<'_>) -> u64 {
        let item = ItemRepo::new(&self.pool)
            .create(&NewItem {
                resource: NewResource {
                    owner_id: Some(book.owner),
                    resource_class_id: book.class,
                    resource_template_id: book.template,
                    title: book.title.map(str::to_string),
                    is_public: true,
                    ..NewResource::default()
                },
                primary_media_id: None,
            })
            .await
            .unwrap();
        ItemRepo::new(&self.pool)
            .add_to_item_set(item.id(), omeka_db::ids::ItemSetId(self.set))
            .await
            .unwrap();
        self.identify(item.id().0, book.identifiers).await;
        item.id().0
    }

    async fn identify(&self, resource: u64, identifiers: &[(&str, bool)]) {
        for (text, is_public) in identifiers {
            ValueRepo::new(&self.pool)
                .create(&NewValue {
                    resource_id: omeka_db::ids::ResourceId(resource),
                    property_id: self.identifier,
                    value_resource_id: None,
                    value_annotation_id: None,
                    r#type: "literal".to_string(),
                    lang: None,
                    value: Some((*text).to_string()),
                    uri: None,
                    is_public: *is_public,
                })
                .await
                .unwrap();
        }
    }

    /// The shelf's item ids in the order `GET /api/items` answers for `query`.
    async fn items(&self, query: &str, keyed: bool) -> Vec<u64> {
        let uri = format!("/api/items?item_set_id={}&{query}", self.set);
        ids(&send(router(self.pool.clone()), get(&uri, keyed)).await)
    }
}

fn ids(answer: &Answer) -> Vec<u64> {
    assert_eq!(answer.status, 200, "{}", answer.body);
    let ids: Vec<u64> = answer
        .json()
        .as_array()
        .expect("a list body is an array")
        .iter()
        .map(|document| document["o:id"].as_u64().expect("every document has an id"))
        .collect();
    let total: usize = header(answer, "Omeka-S-Total-Results")
        .expect("a list names its total")
        .parse()
        .unwrap();
    assert_eq!(total, ids.len(), "the total counts the list it stands for");
    ids
}

#[tokio::test]
async fn a_field_sort_puts_digit_runs_in_number_order_and_empty_last() {
    let shelf = shelf().await;
    let ten = shelf
        .add(Book {
            identifiers: &[("AF_10", true)],
            ..shelf.book()
        })
        .await;
    let none = shelf.add(shelf.book()).await;
    let two_lower = shelf
        .add(Book {
            identifiers: &[("af_2", true)],
            ..shelf.book()
        })
        .await;
    let one = shelf
        .add(Book {
            identifiers: &[("AF_1", true)],
            ..shelf.book()
        })
        .await;
    let two_upper = shelf
        .add(Book {
            identifiers: &[("AF_2", true)],
            ..shelf.book()
        })
        .await;

    assert_eq!(
        shelf.items("sort_by=dcterms:identifier", false).await,
        [one, two_lower, two_upper, ten, none],
        "ascending: 1 < 2 < 10, case folded, a tie on id, no identifier last",
    );
    assert_eq!(
        shelf
            .items("sort_by=dcterms:identifier&sort_order=desc", false)
            .await,
        [ten, two_upper, two_lower, one, none],
        "descending: a tie on id descending, no identifier still last",
    );
}

#[tokio::test]
async fn a_second_value_breaks_a_tie_on_the_first() {
    let shelf = shelf().await;
    let later = shelf
        .add(Book {
            identifiers: &[("B 1", true), ("x 9", true)],
            ..shelf.book()
        })
        .await;
    let earlier = shelf
        .add(Book {
            identifiers: &[("B 1", true), ("x 10", true)],
            ..shelf.book()
        })
        .await;
    let single = shelf
        .add(Book {
            identifiers: &[("B 1", true)],
            ..shelf.book()
        })
        .await;

    assert_eq!(
        shelf.items("sort_by=dcterms:identifier", false).await,
        [single, later, earlier],
    );
}

#[tokio::test]
async fn a_private_value_decides_nothing_for_a_caller_who_cannot_see_it() {
    let shelf = shelf().await;
    let researcher = caller(&shelf.pool, "researcher").await;
    let visible = shelf
        .add(Book {
            identifiers: &[("B_1", true)],
            ..shelf.book()
        })
        .await;
    let hidden = shelf
        .add(Book {
            identifiers: &[("A_0", false)],
            ..shelf.book()
        })
        .await;
    let owned = shelf
        .add(Book {
            owner: researcher.id,
            identifiers: &[("A_5", false)],
            ..shelf.book()
        })
        .await;

    assert_eq!(
        shelf.items("sort_by=dcterms:identifier", false).await,
        [visible, hidden, owned],
        "anonymous: both private identifiers are absent, so those items sort last",
    );
    let uri = format!(
        "{}&item_set_id={}&sort_by=dcterms:identifier",
        researcher.uri("/api/items"),
        shelf.set
    );
    assert_eq!(
        ids(&send(router(shelf.pool.clone()), get(&uri, false)).await),
        [owned, visible, hidden],
        "a signed-in owner sees the private value on their own item, not on another's",
    );
    assert_eq!(
        shelf.items("sort_by=dcterms:identifier", true).await,
        [hidden, owned, visible],
        "a caller who may view everything sees every value",
    );
}

#[tokio::test]
async fn title_sort_is_shelf_order_with_untitled_last() {
    let shelf = shelf().await;
    let ten = shelf
        .add(Book {
            title: Some("Box 10"),
            ..shelf.book()
        })
        .await;
    let untitled = shelf.add(shelf.book()).await;
    let two = shelf
        .add(Book {
            title: Some("box 2"),
            ..shelf.book()
        })
        .await;

    assert_eq!(
        shelf.items("sort_by=title", false).await,
        [two, ten, untitled]
    );
    assert_eq!(
        shelf.items("sort_by=title&sort_order=desc", false).await,
        [ten, two, untitled]
    );
}

#[tokio::test]
async fn label_sorts_order_by_class_item_type_and_owner() {
    let shelf = shelf().await;
    let class = |label: &'static str| {
        let pool = shelf.pool.clone();
        let vocabulary = shelf.seed.vocabulary;
        async move {
            ClassRepo::new(&pool)
                .create(&NewClass {
                    owner_id: None,
                    vocabulary_id: vocabulary,
                    local_name: label.replace(' ', ""),
                    label: label.to_string(),
                    comment: None,
                })
                .await
                .unwrap()
                .id
        }
    };
    let template = |label: &'static str| {
        let pool = shelf.pool.clone();
        async move {
            TemplateRepo::new(&pool)
                .create(&NewTemplate {
                    owner_id: None,
                    resource_class_id: None,
                    title_property_id: None,
                    description_property_id: None,
                    label: label.to_string(),
                })
                .await
                .unwrap()
                .id
        }
    };
    let (map_ten, map_two) = (template("Map 10").await, template("map 2").await);
    let (reel_ten, reel_two) = (class("Reel 10").await, class("reel 2").await);
    let researcher = caller(&shelf.pool, "researcher").await;
    let editor = caller(&shelf.pool, "editor").await;

    let a = shelf
        .add(Book {
            owner: researcher.id,
            class: Some(reel_ten),
            template: Some(map_two),
            ..shelf.book()
        })
        .await;
    let b = shelf
        .add(Book {
            owner: editor.id,
            ..shelf.book()
        })
        .await;
    let c = shelf
        .add(Book {
            class: Some(reel_two),
            template: Some(map_ten),
            ..shelf.book()
        })
        .await;

    assert_eq!(
        shelf.items("sort_by=resource_class_label", true).await,
        [c, a, b]
    );
    assert_eq!(
        shelf.items("sort_by=resource_template_label", true).await,
        [a, c, b]
    );
    assert_eq!(
        shelf.items("sort_by=owner_name", true).await,
        [c, b, a],
        "Admin < editor < researcher",
    );
}

#[tokio::test]
async fn an_unknown_term_orders_by_id() {
    let shelf = shelf().await;
    let first = shelf
        .add(Book {
            identifiers: &[("Z", true)],
            ..shelf.book()
        })
        .await;
    let second = shelf
        .add(Book {
            identifiers: &[("A", true)],
            ..shelf.book()
        })
        .await;

    assert_eq!(
        shelf.items("sort_by=dcterms:nonesuch", false).await,
        [first, second]
    );
}

#[tokio::test]
async fn item_sets_and_media_sort_by_a_field() {
    let shelf = shelf().await;
    let mut sets = Vec::new();
    for identifier in ["S 10", "S 2"] {
        let set = ItemSetRepo::new(&shelf.pool)
            .create(&NewItemSet {
                resource: NewResource {
                    owner_id: Some(shelf.seed.user),
                    is_public: true,
                    ..NewResource::default()
                },
                is_open: false,
            })
            .await
            .unwrap()
            .id()
            .0;
        shelf.identify(set, &[(identifier, true)]).await;
        sets.push(set);
    }
    let uri = format!(
        "/api/item_sets?id={},{}&sort_by=dcterms:identifier",
        sets[0], sets[1]
    );
    assert_eq!(
        ids(&send(router(shelf.pool.clone()), get(&uri, false)).await),
        [sets[1], sets[0]]
    );

    let item = shelf.add(shelf.book()).await;
    let mut media = Vec::new();
    for identifier in ["M 10", "M 2"] {
        let row = MediaRepo::new(&shelf.pool)
            .create(&NewMedia {
                resource: NewResource {
                    owner_id: Some(shelf.seed.user),
                    is_public: true,
                    ..NewResource::default()
                },
                item_id: ItemId(item),
                ingester: "html".to_string(),
                renderer: "html".to_string(),
                ..NewMedia::default()
            })
            .await
            .unwrap();
        shelf.identify(row.id().0, &[(identifier, true)]).await;
        media.push(row.id().0);
    }
    let uri = format!("/api/media?item_id={item}&sort_by=dcterms:identifier");
    assert_eq!(
        ids(&send(router(shelf.pool.clone()), get(&uri, false)).await),
        [media[1], media[0]]
    );
}

/// List latency on 10 000 items, per sort. Run it with
/// `cargo test --release -p omeka-server --test api_sort_by_field -- --ignored --nocapture`.
#[tokio::test]
#[ignore = "a measurement, not a check"]
async fn list_latency_on_ten_thousand_items() {
    const ITEMS: usize = 10_000;
    const RUNS: usize = 21;
    let shelf = shelf().await;
    for n in 0..ITEMS {
        let item = ItemRepo::new(&shelf.pool)
            .create(&NewItem {
                resource: NewResource {
                    owner_id: Some(shelf.seed.user),
                    title: Some(format!("Ledger {}", (n * 7919) % ITEMS)),
                    is_public: true,
                    ..NewResource::default()
                },
                primary_media_id: None,
            })
            .await
            .unwrap();
        let identifier = format!("AF_{}", (n * 104_729) % ITEMS);
        // One in ten identifiers is private, so the anonymous read drops them.
        shelf
            .identify(item.id().0, &[(identifier.as_str(), n % 10 != 0)])
            .await;
    }
    for query in [
        "sort_by=id",
        "sort_by=id&sort_order=desc",
        "sort_by=title",
        "sort_by=dcterms:identifier",
        "sort_by=dcterms:identifier&sort_order=desc",
    ] {
        for keyed in [false, true] {
            let uri = format!("/api/items?per_page=25&page=200&{query}");
            let mut samples = Vec::with_capacity(RUNS);
            for _ in 0..RUNS {
                let app = router(shelf.pool.clone());
                let start = std::time::Instant::now();
                let answer = send(app, get(&uri, keyed)).await;
                samples.push(start.elapsed());
                assert_eq!(answer.status, 200);
            }
            samples.sort();
            println!(
                "{query:<45} keyed={keyed:<5} median {:>9.2?}  min {:>9.2?}  max {:>9.2?}",
                samples[RUNS / 2],
                samples[0],
                samples[RUNS - 1]
            );
        }
    }
}
