use super::*;
use serde_json::json;

use crate::i18n::{EN, ES};

fn site() -> Slug {
    Slug::parse("main").unwrap()
}

fn pages() -> Vec<PageRef> {
    vec![
        PageRef {
            id: PageId::new(5),
            slug: Slug::parse("welcome").unwrap(),
            title: "Welcome".to_string(),
            is_public: true,
        },
        PageRef {
            id: PageId::new(7),
            slug: Slug::parse("about").unwrap(),
            title: "About".to_string(),
            is_public: true,
        },
    ]
}

/// The four registered types plus a fallback parse from the stored shape,
/// and each resolves to the URL `Link/*::toZend` produces.
#[test]
fn navigation_links_resolve_to_upstream_urls() {
    let stored = json!([
        {"type": "page", "data": {"id": "5", "label": ""}, "links": []},
        {"type": "page", "data": {"id": 7}, "links": []},
        {"type": "url", "data": {"label": "Docs", "url": "https://example.org/docs", "target_blank": true}},
        {"type": "browse", "data": {"label": "Items", "query": "year=2000"}, "links": []},
        {"type": "browseItemSets", "data": {"label": "Sets"}},
        {"type": "myModuleLink", "data": {"label": "Custom"}}
    ]);
    let nodes = parse_navigation(&stored).unwrap();
    let items = resolve(&nodes, &site(), &pages(), &EN);

    assert_eq!(items[0].label, "Welcome");
    assert_eq!(items[0].url, "/s/main/page/welcome");
    assert_eq!(items[0].page, Some(PageId::new(5)));
    assert_eq!(items[1].label, "About");
    assert_eq!(items[1].url, "/s/main/page/about");
    assert!(items[1].children.is_empty());
    assert_eq!(items[2].label, "Docs");
    assert_eq!(items[2].url, "https://example.org/docs");
    assert!(items[2].target_blank);
    assert_eq!(items[3].label, "Items");
    assert_eq!(items[3].url, "/s/main/item?year=2000");
    assert_eq!(items[4].url, "/s/main/item-set");
    assert_eq!(items[5].label, "[Missing Page]");
    assert_eq!(items[5].url, "");
}

/// A page link whose page is gone resolves to the missing-label default
/// (`Link/Page.php::getLabel`), and a page link's label is the page title.
#[test]
fn page_link_labels_match_upstream_link_classes() {
    let stored = json!([
        {"type": "page", "data": {"id": 99}},
        {"type": "page", "data": {"id": 5, "label": "Start"}}
    ]);
    let nodes = parse_navigation(&stored).unwrap();
    let items = resolve(&nodes, &site(), &pages(), &EN);
    assert_eq!(items[0].label, "[Missing Page]");
    assert_eq!(items[0].url, "");
    assert_eq!(items[1].label, "Start");
    assert_eq!(items[1].url, "/s/main/page/welcome");
}

/// `Link/Url.php::isValid` rejects a missing label, a missing URL, a C0
/// control character and the `javascript` scheme, naming the JSON path.
#[test]
fn url_link_validation_matches_upstream() {
    let cases = [
        (
            json!({"label": "", "url": "https://example.org"}),
            "must not be empty",
        ),
        (json!({"url": "https://example.org"}), "must not be empty"),
        (
            json!({"label": "x", "url": "https://example.org/\u{1}"}),
            "control character",
        ),
        (
            json!({"label": "x", "url": "JavaScript:alert(1)"}),
            "javascript scheme",
        ),
    ];
    for (data, expected) in cases {
        let stored = json!([{"type": "url", "data": data}]);
        let error = parse_navigation(&stored).unwrap_err().to_string();
        assert!(
            error.contains(expected),
            "{error:?} should name {expected:?}"
        );
        assert!(error.contains("[0].data"), "{error:?} should name the path");
    }
}

/// A missing page id and a malformed `links` value name their paths.
#[test]
fn navigation_errors_name_the_json_path() {
    let error = parse_navigation(&json!([{"type": "page", "data": {}}]))
        .unwrap_err()
        .to_string();
    assert!(error.contains("navigation[0].data.id"), "{error}");
    let error = parse_navigation(&json!([{"type": "page", "data": {"id": 1}, "links": 3}]))
        .unwrap_err()
        .to_string();
    assert!(error.contains("navigation[0].links"), "{error}");
    assert!(matches!(
        parse_navigation(&json!("nope")),
        Err(NavError::NotAList { .. })
    ));
}

/// An empty navigation resolves to the single home link
/// (`Translator::toZend`).
#[test]
fn empty_navigation_resolves_to_home_link() {
    let items = resolve(&[], &site(), &pages(), &EN);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "Home");
    assert_eq!(items[0].url, "/s/main");
}

/// Children resolve recursively, and a page is found for a sub-menu.
#[test]
fn navigation_children_resolve_and_find_by_page() {
    let stored = json!([
        {"type": "page", "data": {"id": 5}, "links": [
            {"type": "page", "data": {"id": 7}}
        ]}
    ]);
    let nodes = parse_navigation(&stored).unwrap();
    let items = resolve(&nodes, &site(), &pages(), &EN);
    assert_eq!(items[0].children.len(), 1);
    assert_eq!(items[0].children[0].url, "/s/main/page/about");
    let found = find_item_by_page(&items, PageId::new(5)).unwrap();
    assert_eq!(found.children[0].label, "About");
    assert_eq!(find_item_by_page(&items, PageId::new(42)), None);
    let path = nav_path(&items, PageId::new(7)).unwrap();
    assert_eq!(path.len(), 2);
    assert_eq!(path[0].label, "Welcome");
    assert_eq!(path[1].label, "About");
    assert_eq!(nav_path(&items, PageId::new(42)), None);
}

/// A menu renders each item as an escaped link; `max_depth` limits the
/// levels of children, which the table of contents relies on.
#[test]
fn render_menu_links_are_escaped_and_depth_limited() {
    let stored = json!([
        {"type": "url", "data": {"label": "Start & Go", "url": "/x"}, "links": [
            {"type": "url", "data": {"label": "Child", "url": "/y"}, "links": [
                {"type": "url", "data": {"label": "Grandchild", "url": "/z"}}
            ]}
        ]}
    ]);
    let items = resolve(&parse_navigation(&stored).unwrap(), &site(), &pages(), &EN);
    let full = render_menu(&items, None, "navigation");
    assert!(full.contains(">Start &amp; Go</a>"), "{full}");
    assert!(full.contains(">Grandchild</a>"), "{full}");
    let shallow = render_menu(&items, Some(1), "navigation");
    assert!(shallow.contains(">Child</a>"), "{shallow}");
    assert!(!shallow.contains(">Grandchild</a>"), "{shallow}");
    let top_only = render_menu(&items, Some(0), "navigation");
    assert!(top_only.contains(">Start &amp; Go</a>"), "{top_only}");
    assert!(!top_only.contains(">Child</a>"), "{top_only}");
    assert_eq!(render_menu(&[], None, "navigation"), "");
}

/// `Link/Url.php::toZend`: a URL link with `target_blank` renders the anchor
/// with `target="_blank"`, and one without it renders no target attribute.
#[test]
fn render_menu_marks_a_target_blank_url_link() {
    let stored = json!([
        {"type": "url", "data": {
            "label": "Elsewhere",
            "url": "https://example.com/x",
            "target_blank": true
        }},
        {"type": "url", "data": {"label": "Here", "url": "/x"}}
    ]);
    let items = resolve(&parse_navigation(&stored).unwrap(), &site(), &pages(), &EN);
    let menu = render_menu(&items, None, "navigation");
    assert!(
        menu.contains("<a href=\"https://example.com/x\" target=\"_blank\">Elsewhere</a>"),
        "{menu}"
    );
    assert!(menu.contains("<a href=\"/x\">Here</a>"), "{menu}");
}

/// A Spanish site prints the link types' names and the home link as `es.po`
/// translates them, while a label the user typed and a page title stay as
/// stored (`Translator::getLinkLabel`).
#[test]
fn fallback_labels_follow_the_site_locale_and_user_labels_do_not() {
    let stored = json!([
        {"type": "page", "data": {"id": 5}},
        {"type": "page", "data": {"id": 99}},
        {"type": "browse", "data": {"label": ""}},
        {"type": "browseItemSets", "data": {}},
        {"type": "browse", "data": {"label": "Items"}},
        {"type": "myModuleLink", "data": {}}
    ]);
    let nodes = parse_navigation(&stored).unwrap();
    let labels = |messages| {
        resolve(&nodes, &site(), &pages(), messages)
            .into_iter()
            .map(|item| item.label)
            .collect::<Vec<_>>()
    };
    assert_eq!(
        labels(&EN),
        [
            "Welcome",
            "[Missing Page]",
            "Browse",
            "Browse item sets",
            "Items",
            "[Missing Page]"
        ]
    );
    assert_eq!(
        labels(&ES),
        [
            "Welcome",
            "[Página perdida]",
            "Explorar",
            "Explorar conjuntos de fichas",
            "Items",
            "[Página perdida]"
        ]
    );
    assert_eq!(resolve(&[], &site(), &pages(), &ES)[0].label, "Inicio");
}
