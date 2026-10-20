//! The list reads the API serves straight from SQL: an item window with its total
//! (`ItemRepo::window`), every resource of one kind (`ResourceRepo::list_of_type`) and every site
//! page (`SiteRepo::list_all_pages`), on SQLite and, with the `mysql` feature, on MySQL and
//! MariaDB.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;
mod import_db;

use import_db::on_both;
use omeka_db::ids::{ItemId, UserId};
use omeka_db::repos::{
    ItemFilter, ItemRepo, ItemSetRepo, ItemWindow, MediaVisibility, ResourceRepo, SiteRepo,
    UserRepo,
};
use omeka_db::rows::{
    NewItem, NewItemSet, NewResource, NewSite, NewSitePage, NewUser, resource_type,
};

fn ids(page: &omeka_db::repos::ItemPage) -> Vec<ItemId> {
    page.rows.iter().map(|row| row.id()).collect()
}

on_both!(a_window_reads_one_page_and_the_filter_total, |pool| {
    let owner = UserRepo::<Db>::new(pool)
        .create(&NewUser {
            email: "owner@example.org".into(),
            name: "Owner".into(),
            password_hash: None,
            role: "author".into(),
            is_active: true,
            created: None,
        })
        .await
        .unwrap();
    let items = ItemRepo::<Db>::new(pool);
    let mut public = Vec::new();
    for _ in 0..3 {
        public.push(items.create(&NewItem::default()).await.unwrap().id());
    }
    let private_owned = items
        .create(&NewItem {
            resource: NewResource {
                owner_id: Some(owner.id),
                is_public: false,
                ..NewResource::default()
            },
            primary_media_id: None,
        })
        .await
        .unwrap()
        .id();
    let private_other = items
        .create(&NewItem {
            resource: NewResource {
                is_public: false,
                ..NewResource::default()
            },
            primary_media_id: None,
        })
        .await
        .unwrap()
        .id();

    let anyone = ItemWindow {
        visibility: MediaVisibility::Actor(None),
        ..ItemWindow::default()
    };
    let page = items.window(&anyone, 2, 0).await.unwrap();
    assert_eq!(page.total, 3, "an anonymous caller sees the public rows");
    assert_eq!(ids(&page), public[..2]);
    let tail = items.window(&anyone, 2, 2).await.unwrap();
    assert_eq!((tail.total, ids(&tail)), (3, vec![public[2]]));
    let past = items.window(&anyone, 2, 10).await.unwrap();
    assert_eq!(past.total, 3, "a window past the end still counts");
    assert!(past.rows.is_empty());

    let signed_in = ItemWindow {
        visibility: MediaVisibility::Actor(Some(owner.id)),
        ..ItemWindow::default()
    };
    let page = items.window(&signed_in, 10, 0).await.unwrap();
    assert_eq!(page.total, 4, "public rows and the caller's own");
    assert!(ids(&page).contains(&private_owned));

    let page = items.window(&ItemWindow::default(), 10, 0).await.unwrap();
    assert_eq!(page.total, 5, "view-all sees every row");
    assert_eq!(ids(&page).last(), Some(&private_other));

    let owned = ItemWindow {
        filter: ItemFilter {
            owner_id: Some(owner.id),
            is_public: Some(false),
            ..ItemFilter::default()
        },
        ..ItemWindow::default()
    };
    let page = items.window(&owned, 10, 0).await.unwrap();
    assert_eq!((page.total, ids(&page)), (1, vec![private_owned]));

    let nobody = ItemWindow {
        filter: ItemFilter {
            owner_id: Some(UserId(owner.id.0 + 100)),
            ..ItemFilter::default()
        },
        ..ItemWindow::default()
    };
    let page = items.window(&nobody, 10, 0).await.unwrap();
    assert_eq!((page.total, page.rows.len()), (0, 0));
});

on_both!(a_window_renders_the_id_list_and_the_item_sets, |pool| {
    let items = ItemRepo::<Db>::new(pool);
    let mut made = Vec::new();
    for _ in 0..4 {
        made.push(items.create(&NewItem::default()).await.unwrap().id());
    }
    let sets = ItemSetRepo::<Db>::new(pool);
    let set = sets.create(&NewItemSet::default()).await.unwrap();
    let other = sets.create(&NewItemSet::default()).await.unwrap();
    items.add_to_item_set(made[1], set.id()).await.unwrap();
    items.add_to_item_set(made[2], other.id()).await.unwrap();
    items.add_to_item_set(made[3], set.id()).await.unwrap();

    let listed = ItemWindow {
        ids: Some(vec![made[0].0 as i64, made[3].0 as i64]),
        ..ItemWindow::default()
    };
    let page = items.window(&listed, 10, 0).await.unwrap();
    assert_eq!((page.total, ids(&page)), (2, vec![made[0], made[3]]));

    let in_sets = ItemWindow {
        item_set_ids: vec![set.id(), other.id()],
        ..ItemWindow::default()
    };
    let page = items.window(&in_sets, 10, 0).await.unwrap();
    assert_eq!(ids(&page), made[1..]);

    let both = ItemWindow {
        ids: Some(vec![made[1].0 as i64, made[2].0 as i64]),
        item_set_ids: vec![set.id()],
        ..ItemWindow::default()
    };
    let page = items.window(&both, 10, 0).await.unwrap();
    assert_eq!((page.total, ids(&page)), (1, vec![made[1]]));

    let one_set = ItemWindow {
        filter: ItemFilter {
            item_set_id: Some(other.id()),
            ..ItemFilter::default()
        },
        ..ItemWindow::default()
    };
    let page = items.window(&one_set, 10, 0).await.unwrap();
    assert_eq!((page.total, ids(&page)), (1, vec![made[2]]));

    let none_listed = ItemWindow {
        ids: Some(Vec::new()),
        ..ItemWindow::default()
    };
    let page = items.window(&none_listed, 10, 0).await.unwrap();
    assert_eq!(page.total, 0, "an empty id list matches no item");
});

on_both!(a_kind_list_and_the_page_list_read_every_row, |pool| {
    let resources = ResourceRepo::<Db>::new(pool);
    let first = resources
        .create_annotation(&NewResource::default())
        .await
        .unwrap();
    ItemRepo::<Db>::new(pool)
        .create(&NewItem::default())
        .await
        .unwrap();
    let second = resources
        .create_annotation(&NewResource {
            is_public: false,
            ..NewResource::default()
        })
        .await
        .unwrap();
    let annotations: Vec<_> = resources
        .list_of_type(resource_type::VALUE_ANNOTATION)
        .await
        .unwrap()
        .into_iter()
        .map(|row| row.id)
        .collect();
    assert_eq!(annotations, [first.resource.id, second.resource.id]);

    let sites = SiteRepo::<Db>::new(pool);
    let mut pages = Vec::new();
    for slug in ["a", "b"] {
        let site = sites
            .create(&NewSite {
                slug: slug.into(),
                theme: "default".into(),
                title: slug.into(),
                ..NewSite::default()
            })
            .await
            .unwrap();
        pages.push(
            sites
                .add_page(&NewSitePage {
                    site_id: site.id,
                    slug: "welcome".into(),
                    title: "Welcome".into(),
                    ..NewSitePage::default()
                })
                .await
                .unwrap()
                .id,
        );
    }
    let all: Vec<_> = sites
        .list_all_pages()
        .await
        .unwrap()
        .into_iter()
        .map(|row| row.id)
        .collect();
    assert_eq!(all, pages);
});
