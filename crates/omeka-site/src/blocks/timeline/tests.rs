use super::*;
use crate::blocks::romeka::{PlainItem, PlainValue};
use crate::blocks::test_support::{TestResources, context, item, literal_value, site};
use crate::view::{BlockMarkers, NoResources, Thumbnail};
use omeka_core::ids::ItemId;
use serde_json::json;

fn picked() -> JsonValue {
    json!({"source": "items", "items": [1, 2, 3, 4]})
}

fn resources() -> TestResources {
    let mut resources = TestResources::default();
    let dated = |id: u64, title: &str, dates: &[&str]| {
        let mut view = item(id, title, &format!("/s/main/item/{id}"));
        view.values = dates
            .iter()
            .map(|text| literal_value(8, "Date", text))
            .collect();
        view
    };
    let mut bridge = dated(1, "Tower Bridge", &["1894-06-30"]);
    bridge.thumbnail = Some(Thumbnail::new("/files/medium/1.jpg", "The bridge"));
    resources.items.insert(1, bridge);
    resources
        .items
        .insert(2, dated(2, "A letter", &["undated", "1886?"]));
    resources
        .items
        .insert(3, dated(3, "A sketch", &["some time in spring"]));
    resources.items.insert(4, dated(4, "A map", &[]));
    resources
}

/// The rich timeline lists the dated items from the earliest, each with its
/// date as written, thumbnail and title, then the undated ones with the
/// value they have.
#[test]
fn rich_places_items_in_date_order() {
    let site = site();
    let resources = resources();
    let ctx = context(&site, &resources);
    let rich = Timeline.render(&picked(), &ctx);
    let letter = rich.find("A letter").unwrap();
    let bridge = rich.find("Tower Bridge").unwrap();
    assert!(letter < bridge, "{rich}");
    assert!(
        rich.contains("<p class=\"romeka-timeline-date\">1886?</p>"),
        "{rich}"
    );
    assert!(
        rich.contains(
            "<a class=\"romeka-timeline-link\" href=\"/s/main/item/1\"><img class=\"romeka-timeline-thumbnail\" \
             src=\"/files/medium/1.jpg\" alt=\"The bridge\" loading=\"lazy\"><span class=\"romeka-timeline-title\">\
             Tower Bridge</span></a>"
        ),
        "{rich}"
    );
    assert!(
        rich.contains(
            "<h3 class=\"romeka-timeline-undated-heading\">Undated</h3><ul class=\"romeka-timeline-undated\">\
             <li><a href=\"/s/main/item/3\">A sketch</a> <span class=\"romeka-timeline-value\">some time in spring</span></li>\
             <li><a href=\"/s/main/item/4\">A map</a></li></ul>"
        ),
        "{rich}"
    );
}

/// The chosen property places the items; dates in another property do not.
#[test]
fn rich_reads_the_chosen_property() {
    let site = site();
    let resources = resources();
    let ctx = context(&site, &resources);
    let rich = Timeline.render(&json!({"items": [1], "property": "dcterms:created"}), &ctx);
    assert!(!rich.contains("romeka-timeline-list"), "{rich}");
    assert!(rich.contains("romeka-timeline-undated"), "{rich}");
}

/// The plain rendering is the same order as a list, date first, then the
/// undated items under the site's word for undated.
#[test]
fn plain_lists_the_items_in_date_order() {
    let value = |text: &str| PlainValue {
        term: "dcterms:date".to_string(),
        label: "Date".to_string(),
        text: text.to_string(),
    };
    let refs = PlainRefs::new()
        .item(PlainItem {
            values: vec![value("1894")],
            ..PlainItem::new(ItemId::new(1), "Tower Bridge", "/s/main/item/1")
        })
        .item(PlainItem {
            values: vec![value("spring")],
            ..PlainItem::new(ItemId::new(3), "A <sketch>", "/s/main/item/3")
        })
        .item(PlainItem {
            values: vec![value("1700-01-02")],
            ..PlainItem::new(ItemId::new(2), "A letter", "/s/main/item/2")
        });
    assert_eq!(
        Timeline.plain(&picked(), &refs),
        "<ol><li>1700-01-02: <a href=\"/s/main/item/2\">A letter</a></li>\
         <li>1894: <a href=\"/s/main/item/1\">Tower Bridge</a></li></ol>\
         <h3>Undated</h3><ul><li><a href=\"/s/main/item/3\">A &lt;sketch&gt;</a> spring</li></ul>"
    );
    assert_eq!(Timeline.plain(&picked(), &PlainRefs::new()), "");
}

/// A query selection lists what the search finds.
#[test]
fn a_query_selection_reads_the_search() {
    let site = site();
    let mut resources = resources();
    resources.browse = vec![resources.items[&1].clone()];
    let ctx = context(&site, &resources);
    let rich = Timeline.render(
        &json!({"source": "query", "query": "fulltext_search=bridge"}),
        &ctx,
    );
    assert!(rich.contains("Tower Bridge"), "{rich}");
    assert!(!rich.contains("A letter"), "{rich}");
    assert_eq!(
        Timeline.items(&json!({"source": "query", "query": "a=b"})),
        Some(ItemSelection::Query("a=b".to_string()))
    );
}

/// No item shows nothing publicly and a hint in the preview.
#[test]
fn an_empty_timeline_renders_nothing_publicly_and_a_hint_in_the_preview() {
    let site = site();
    let ctx = context(&site, &NoResources);
    assert_eq!(Timeline.render(&picked(), &ctx), "");
    let mut preview = context(&site, &NoResources);
    preview.block_markers = BlockMarkers::Preview;
    assert!(
        Timeline
            .render(&picked(), &preview)
            .contains("romeka-timeline-empty")
    );
}
