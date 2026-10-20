//! The `map` Romeka block: the places of chosen items on a Leaflet map.
//!
//! The items come from the item picker or from a browse query, and each item's
//! places are the `lat, lng` values of the site's location field
//! ([`crate::map`]). The block is stored as an upstream `html` block whose plain
//! rendering lists each place as a link to its item, so upstream shows the same
//! places as text. The editor preview runs no script, so it shows a poster
//! with the same list.

use omeka_core::ids::ItemId;
use omeka_core::resource::ResourceKind;
use serde_json::Value as JsonValue;

use super::romeka::{ItemSelection, PlainRefs, RomkaBlock};
use super::schema::{
    ChoiceOption, LayoutSchema, When, boolean, choice, item_list, item_query, map_view,
    multi_choice, option, property,
};
use crate::html::escape_html;
use crate::map::{
    Height, MAX_PLACES, MapPoint, MapView, Place, StartView, canvas, place_list, places_of, poster,
};
use crate::view::{RenderContext, ResourceView};

/// The `map` block.
#[derive(Debug, Clone, Copy, Default)]
pub struct Map;

const SOURCE: &[ChoiceOption] = &[
    option("items", "romekaChoice.map.items"),
    option("query", "romekaChoice.map.query"),
];

const HEIGHT: &[ChoiceOption] = &[
    option("short", "romekaChoice.map.short"),
    option("medium", "romekaChoice.map.medium"),
    option("tall", "romekaChoice.map.tall"),
];

const POPUP: &[ChoiceOption] = &[
    option("title", "romekaChoice.map.title"),
    option("thumbnail", "romekaChoice.map.thumbnail"),
];

/// The zoom a set centre starts at when no zoom is stored: a region.
const DEFAULT_ZOOM: u8 = 10;

/// Leaflet's deepest zoom on the OpenStreetMap tiles.
const MAX_ZOOM: u8 = 19;

const SCHEMA: LayoutSchema = LayoutSchema {
    fields: &[
        choice(
            "source",
            "romekaField.map.source",
            "fieldHelp.map.source",
            SOURCE,
        )
        .default_text("items"),
        item_list("items", "romekaField.map.items", "fieldHelp.map.items")
            .when(When::equals("source", "items")),
        item_query("query", "romekaField.map.query", "fieldHelp.map.query")
            .when(When::equals("source", "query")),
        map_view(
            "center",
            "zoom",
            "romekaField.map.center",
            "fieldHelp.map.center",
        ),
        choice(
            "height",
            "romekaField.map.height",
            "fieldHelp.map.height",
            HEIGHT,
        )
        .default_text("medium"),
        multi_choice(
            "popup",
            "romekaField.map.popup",
            "fieldHelp.map.popup",
            POPUP,
        )
        .default_list(&["title", "thumbnail"]),
        property(
            "popup_property",
            "romekaField.map.popup_property",
            "fieldHelp.map.popup_property",
        ),
        boolean(
            "cluster",
            "romekaField.map.cluster",
            "fieldHelp.map.cluster",
        ),
    ],
};

impl RomkaBlock for Map {
    fn name(&self) -> &'static str {
        "map"
    }

    fn label(&self) -> &'static str {
        "romeka.map"
    }

    fn schema(&self) -> &'static LayoutSchema {
        &SCHEMA
    }

    fn render(&self, settings: &JsonValue, ctx: &RenderContext<'_>) -> String {
        let settings = MapSettings::parse(settings);
        let points = settings.points(ctx);
        if ctx.is_preview() {
            return format!("<div class=\"romeka-map\">{}</div>", poster(&points, ctx));
        }
        if points.is_empty() {
            return String::new();
        }
        let map = MapView {
            points: &points,
            start: settings.start,
            height: settings.height,
            cluster: settings.cluster,
            show_title: settings.popup_title,
        };
        format!(
            "<div class=\"romeka-map\">{}<details class=\"romeka-map-places\"><summary>{} ({})</summary>{}</details></div>",
            canvas(&map, ctx),
            escape_html(ctx.translator().messages().map_places),
            points.len(),
            place_list(&points)
        )
    }

    fn plain(&self, _settings: &JsonValue, refs: &PlainRefs) -> String {
        let mut rows = String::new();
        for item in refs.items() {
            for place in item.places.iter().filter_map(|text| Place::parse(text)) {
                rows.push_str(&format!(
                    "<li><a href=\"{}\">{}</a> {place}</li>",
                    escape_html(&item.url),
                    escape_html(&item.title)
                ));
            }
        }
        if rows.is_empty() {
            return String::new();
        }
        format!("<ul>{rows}</ul>")
    }

    fn items(&self, settings: &JsonValue) -> Option<ItemSelection> {
        MapSettings::parse(settings).selection
    }
}

/// The block's typed settings.
#[derive(Debug, Clone, PartialEq)]
struct MapSettings {
    selection: Option<ItemSelection>,
    start: StartView,
    height: Height,
    popup_title: bool,
    popup_thumbnail: bool,
    popup_property: Option<String>,
    cluster: bool,
}

impl MapSettings {
    fn parse(settings: &JsonValue) -> Self {
        let text_of = |key: &str| settings.get(key).and_then(JsonValue::as_str).unwrap_or("");
        let selection = match text_of("source") {
            "query" => Some(ItemSelection::Query(text_of("query").to_string())),
            _ => {
                let picked: Vec<ItemId> = settings
                    .get("items")
                    .and_then(JsonValue::as_array)
                    .map(|ids| ids.iter().filter_map(item_id_of).collect())
                    .unwrap_or_default();
                (!picked.is_empty()).then_some(ItemSelection::Picked(picked))
            }
        };
        let start = match Place::parse(text_of("center")) {
            Some(centre) => StartView::At {
                centre,
                zoom: settings
                    .get("zoom")
                    .and_then(JsonValue::as_u64)
                    .and_then(|zoom| u8::try_from(zoom).ok())
                    .map_or(DEFAULT_ZOOM, |zoom| zoom.min(MAX_ZOOM)),
            },
            None => StartView::Fit,
        };
        let height = match text_of("height") {
            "short" => Height::Short,
            "tall" => Height::Tall,
            _ => Height::Medium,
        };
        let popup: Option<Vec<&str>> = settings
            .get("popup")
            .and_then(JsonValue::as_array)
            .map(|chosen| chosen.iter().filter_map(JsonValue::as_str).collect());
        let shows = |part: &str| popup.as_ref().is_none_or(|chosen| chosen.contains(&part));
        Self {
            selection,
            start,
            height,
            popup_title: shows("title"),
            popup_thumbnail: shows("thumbnail"),
            popup_property: Some(text_of("popup_property"))
                .filter(|term| !term.is_empty())
                .map(str::to_string),
            cluster: settings
                .get("cluster")
                .and_then(JsonValue::as_bool)
                .unwrap_or(false),
        }
    }

    /// The markers: each selected item's places, in selection order, at most
    /// [`MAX_PLACES`].
    fn points(&self, ctx: &RenderContext<'_>) -> Vec<MapPoint> {
        let Some(term) = ctx.settings.location_property() else {
            return Vec::new();
        };
        let resources: Vec<ResourceView> = match &self.selection {
            None => Vec::new(),
            Some(ItemSelection::Picked(ids)) => ids
                .iter()
                .filter_map(|id| ctx.resources.read(ResourceKind::Item, id.get()))
                .collect(),
            Some(ItemSelection::Query(query)) => {
                let search = ItemSelection::browse_query(query, ctx.site.id, MAX_PLACES);
                ctx.resources.search(ResourceKind::Item, &search).resources
            }
        };
        resources
            .iter()
            .flat_map(|resource| {
                places_of(resource, term)
                    .into_iter()
                    .map(move |place| self.point(resource, place))
            })
            .take(MAX_PLACES)
            .collect()
    }

    fn point(&self, resource: &ResourceView, place: Place) -> MapPoint {
        MapPoint {
            place,
            title: resource.title.clone(),
            url: resource.url.clone(),
            thumbnail: resource
                .thumbnail
                .as_ref()
                .filter(|_| self.popup_thumbnail)
                .map(|thumbnail| thumbnail.url.clone()),
            value: self.popup_property.as_deref().and_then(|term| {
                resource
                    .values
                    .iter()
                    .find(|value| value.property_term == term)
                    .map(|value| value.value.clone())
            }),
        }
    }
}

/// An item id from a JSON number or a numeric string.
fn item_id_of(value: &JsonValue) -> Option<ItemId> {
    match value {
        JsonValue::Number(number) => number.as_u64().map(ItemId::new),
        JsonValue::String(text) => text.trim().parse::<u64>().ok().map(ItemId::new),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks::romeka::{CarrierRender, PlainItem, plain_hash, render_carrier};
    use crate::blocks::test_support::{
        TestResources, block_with_layout, context, item, literal_value, site,
    };
    use crate::settings::SiteSettings;
    use crate::view::{RenderContext, Thumbnail, ValueRef};
    use omeka_core::site::SiteBlock;
    use serde_json::json;

    fn spatial(text: &str) -> ValueRef {
        literal_value(7, "Spatial", text)
    }

    fn resources() -> TestResources {
        let mut resources = TestResources::default();
        let mut bridge = item(4, "Tower Bridge", "/s/main/item/4");
        bridge.values = vec![
            spatial("51.5055, -0.0754"),
            literal_value(8, "Date", "1894"),
        ];
        bridge.thumbnail = Some(Thumbnail::new("/files/square/4.jpg", "Tower Bridge"));
        let mut tower = item(5, "Eiffel Tower", "/s/main/item/5");
        tower.values = vec![spatial("48.8584, 2.2945")];
        let mut unplaced = item(6, "A letter", "/s/main/item/6");
        unplaced.values = vec![spatial("somewhere in Kent")];
        resources.items.insert(4, bridge.clone());
        resources.items.insert(5, tower.clone());
        resources.items.insert(6, unplaced);
        resources.browse = vec![tower, bridge];
        resources
    }

    fn located<'a>(
        site: &'a omeka_core::site::Site,
        resources: &'a TestResources,
    ) -> RenderContext<'a> {
        let mut ctx = context(site, resources);
        ctx.settings = SiteSettings {
            romeka_location_property: Some("dcterms:spatial".to_string()),
            ..SiteSettings::default()
        };
        ctx
    }

    fn picked() -> JsonValue {
        json!({"source": "items", "items": [4, "5", 6], "popup_property": "dcterms:date"})
    }

    /// The plain rendering lists every resolved place as a link to its item,
    /// in upstream's vocabulary, and skips a value that is not a place.
    #[test]
    fn plain_lists_each_place_with_a_link() {
        let refs = PlainRefs::new()
            .item(PlainItem {
                title: "Tower Bridge".to_string(),
                url: "/s/main/item/4".to_string(),
                places: vec!["51.5055, -0.0754".to_string()],
            })
            .item(PlainItem {
                title: "A <letter>".to_string(),
                url: "/s/main/item/6".to_string(),
                places: vec!["somewhere".to_string(), "51.28,1.08".to_string()],
            });
        assert_eq!(
            Map.plain(&picked(), &refs),
            "<ul><li><a href=\"/s/main/item/4\">Tower Bridge</a> 51.5055, -0.0754</li>\
             <li><a href=\"/s/main/item/6\">A &lt;letter&gt;</a> 51.28, 1.08</li></ul>"
        );
        assert_eq!(Map.plain(&picked(), &PlainRefs::new()), "");
    }

    /// The rich render draws the picked items' places with the popup's value,
    /// and lists them under the map for readers without it.
    #[test]
    fn rich_draws_the_picked_places() {
        let site = site();
        let resources = resources();
        let ctx = located(&site, &resources);
        let rich = Map.render(&picked(), &ctx);
        assert!(rich.contains("data-romeka-map="), "{rich}");
        assert!(
            rich.contains("&quot;title&quot;:&quot;Tower Bridge&quot;"),
            "{rich}"
        );
        assert!(
            rich.contains("&quot;value&quot;:&quot;1894&quot;"),
            "{rich}"
        );
        assert!(rich.contains("/files/square/4.jpg"), "{rich}");
        assert!(!rich.contains("A letter"), "an item with no place: {rich}");
        assert!(
            rich.contains("<summary>Places on this map (2)</summary>"),
            "{rich}"
        );
        assert!(
            rich.contains("<a href=\"/s/main/item/5\">Eiffel Tower</a> <span class=\"romeka-map-place\">48.8584, 2.2945</span>"),
            "{rich}"
        );
    }

    /// A query selection draws what the search finds, in its order.
    #[test]
    fn rich_draws_the_query_results() {
        let site = site();
        let resources = resources();
        let ctx = located(&site, &resources);
        let rich = Map.render(
            &json!({"source": "query", "query": "fulltext_search=tower"}),
            &ctx,
        );
        let tower = rich.find("Eiffel Tower").unwrap();
        let bridge = rich.find("Tower Bridge").unwrap();
        assert!(tower < bridge, "{rich}");
    }

    /// With no location field, or no places, the public page shows nothing,
    /// and the preview's poster says why.
    #[test]
    fn no_places_render_nothing_publicly_and_a_hint_in_the_preview() {
        let site = site();
        let resources = resources();
        let unlocated = context(&site, &resources);
        assert_eq!(Map.render(&picked(), &unlocated), "");
        let mut preview = context(&site, &resources);
        preview.block_markers = crate::view::BlockMarkers::Preview;
        let poster = Map.render(&picked(), &preview);
        assert!(poster.contains("This map has no places yet"), "{poster}");
        assert!(!poster.contains("data-romeka-map"), "{poster}");
    }

    /// The script-free preview shows a poster with the places, never the map.
    #[test]
    fn preview_shows_a_poster_listing_the_places() {
        let site = site();
        let resources = resources();
        let mut ctx = located(&site, &resources);
        ctx.block_markers = crate::view::BlockMarkers::Preview;
        let poster = Map.render(&picked(), &ctx);
        assert!(!poster.contains("data-romeka-map"), "{poster}");
        assert!(poster.contains("A map of 2 places."), "{poster}");
        assert!(poster.contains("Tower Bridge"), "{poster}");
    }

    /// A stored centre starts the map there at the stored zoom; an empty or
    /// broken centre fits every place.
    #[test]
    fn settings_read_the_start_view() {
        let set = MapSettings::parse(&json!({"center": "48.85, 2.35", "zoom": 12}));
        assert_eq!(
            set.start,
            StartView::At {
                centre: Place::new(48.85, 2.35).unwrap(),
                zoom: 12
            }
        );
        let no_zoom = MapSettings::parse(&json!({"center": "48.85, 2.35"}));
        assert!(matches!(no_zoom.start, StartView::At { zoom: 10, .. }));
        for broken in [json!({}), json!({"center": "Paris", "zoom": 4})] {
            assert_eq!(MapSettings::parse(&broken).start, StartView::Fit);
        }
    }

    /// The selection is the picked ids, or the query, and nothing when no item
    /// is picked.
    #[test]
    fn items_name_the_selection() {
        assert_eq!(
            Map.items(&picked()),
            Some(ItemSelection::Picked(vec![
                ItemId::new(4),
                ItemId::new(5),
                ItemId::new(6)
            ]))
        );
        assert_eq!(
            Map.items(&json!({"source": "query", "query": "item_set_id=2"})),
            Some(ItemSelection::Query("item_set_id=2".to_string()))
        );
        assert_eq!(Map.items(&json!({"items": []})), None);
    }

    /// A map carrier whose plain rendering still matches renders the map, so
    /// the public page loads Leaflet for it.
    #[test]
    fn a_matching_carrier_renders_the_map() {
        let site = site();
        let resources = resources();
        let ctx = located(&site, &resources);
        let plain =
            "<ul><li><a href=\"/s/main/item/4\">Tower Bridge</a> 51.5055, -0.0754</li></ul>";
        let block: SiteBlock = block_with_layout(
            "html",
            json!({ "html": plain }),
            json!({ "romeka": {
                "block": "map",
                "v": 2,
                "settings": picked(),
                "plain_hash": plain_hash(plain),
            }}),
        );
        match render_carrier(&block, &ctx) {
            Some(CarrierRender::Rich(html)) => assert!(html.contains("data-romeka-map"), "{html}"),
            other => panic!("expected the rich map, got {other:?}"),
        }
    }
}
