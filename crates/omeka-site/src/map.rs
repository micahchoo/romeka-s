//! Places on a map: the `lat, lng` value a site's location field holds, and the
//! map the map block and the item page's map section draw.
//!
//! Romeka has no geographic data type, so a place is an ordinary literal value,
//! `"51.5072, -0.1276"`, in the property the site setting
//! `romeka_location_property` names. Upstream shows it as text, and a round
//! trip through upstream keeps it.
//!
//! A drawn map is a `div` carrying its settings and places as JSON in
//! [`MARKER`]. [`with_assets`] adds Leaflet and `map.js` to a page only when its
//! content holds such a `div`, so a page without a map loads no map code. The
//! purifier drops every `data-` attribute from stored html, so no stored block
//! can ask for the scripts.

use std::fmt;

use serde_json::{Value as JsonValue, json};

use crate::html::escape_html;
use crate::i18n::sprintf1;
use crate::theme::{AssetKind, ThemeAsset};
use crate::view::{RenderContext, ResourceView};

/// The attribute that marks a map for `map.js`.
pub const MARKER: &str = "data-romeka-map";

/// The script that draws every map on a page.
pub const MAP_SCRIPT: ThemeAsset = ThemeAsset {
    path: "js/map.js",
    kind: AssetKind::Library,
    content: include_str!("map.js"),
};

/// Leaflet 1.9.4, vendored by `vendor/leaflet/update.sh` (BSD-2-Clause).
pub const LEAFLET_SCRIPT: ThemeAsset = ThemeAsset {
    path: "vendor/leaflet/leaflet.js",
    kind: AssetKind::Library,
    content: include_str!("../vendor/leaflet/leaflet.js"),
};

/// Leaflet's own stylesheet, from the same release.
pub const LEAFLET_STYLESHEET: ThemeAsset = ThemeAsset {
    path: "vendor/leaflet/leaflet.css",
    kind: AssetKind::LibraryStylesheet,
    content: include_str!("../vendor/leaflet/leaflet.css"),
};

/// The most places one map draws, so a broad query cannot make a page heavy.
pub const MAX_PLACES: usize = 500;

/// One point on the earth, in degrees.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Place {
    lat: f64,
    lng: f64,
}

impl Place {
    /// A place, `None` when the latitude is outside -90 to 90 or the longitude
    /// outside -180 to 180.
    #[must_use]
    pub fn new(lat: f64, lng: f64) -> Option<Self> {
        ((-90.0..=90.0).contains(&lat) && (-180.0..=180.0).contains(&lng))
            .then_some(Self { lat, lng })
    }

    /// The place a value's text holds: two decimal numbers, latitude first,
    /// separated by a comma. Anything else is not a place.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let (lat, lng) = text.split_once(',')?;
        Self::new(lat.trim().parse().ok()?, lng.trim().parse().ok()?)
    }

    /// The latitude.
    #[must_use]
    pub fn lat(self) -> f64 {
        self.lat
    }

    /// The longitude.
    #[must_use]
    pub fn lng(self) -> f64 {
        self.lng
    }
}

/// The stored form, `"lat, lng"` with at most six decimals (about 10 cm), the
/// form the item form's place control writes.
impl fmt::Display for Place {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}, {}", coordinate(self.lat), coordinate(self.lng))
    }
}

fn coordinate(degrees: f64) -> String {
    let fixed = format!("{degrees:.6}");
    let trimmed = fixed.trim_end_matches('0').trim_end_matches('.');
    match trimmed {
        "-0" => "0".to_string(),
        other => other.to_string(),
    }
}

/// The places a resource's location values hold, in value order. A value that
/// is not a place is skipped.
#[must_use]
pub fn places_of(resource: &ResourceView, term: &str) -> Vec<Place> {
    resource
        .values
        .iter()
        .filter(|value| value.property_term == term)
        .filter_map(|value| Place::parse(&value.value))
        .collect()
}

/// One marker: a place and what its popup shows.
#[derive(Debug, Clone, PartialEq)]
pub struct MapPoint {
    /// Where the marker sits.
    pub place: Place,
    /// The item's title, the marker's accessible name.
    pub title: String,
    /// The item's page.
    pub url: String,
    /// The thumbnail the popup shows, `None` to show none.
    pub thumbnail: Option<String>,
    /// The one extra value the popup shows, `None` to show none.
    pub value: Option<String>,
}

/// Where a map starts.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StartView {
    /// Every place in view.
    Fit,
    /// A set centre and zoom.
    At {
        /// The centre.
        centre: Place,
        /// The Leaflet zoom level, 0 (the world) to 19.
        zoom: u8,
    },
}

/// How tall a map is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Height {
    /// About 300 pixels.
    Short,
    /// About 450 pixels.
    Medium,
    /// About 600 pixels.
    Tall,
}

impl Height {
    /// The stored setting value, which is also the class suffix.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Short => "short",
            Self::Medium => "medium",
            Self::Tall => "tall",
        }
    }
}

/// One map to draw.
#[derive(Debug, Clone, PartialEq)]
pub struct MapView<'a> {
    /// The markers.
    pub points: &'a [MapPoint],
    /// Where the map starts.
    pub start: StartView,
    /// How tall it is.
    pub height: Height,
    /// Whether close markers join into one.
    pub cluster: bool,
    /// Whether a popup shows the item's title.
    pub show_title: bool,
}

/// The map's `div`: the settings and places `map.js` reads, with the basemap
/// from the site settings.
#[must_use]
pub fn canvas(map: &MapView<'_>, ctx: &RenderContext<'_>) -> String {
    let messages = ctx.translator().messages();
    let view = match map.start {
        StartView::Fit => json!({}),
        StartView::At { centre, zoom } => {
            json!({"lat": centre.lat, "lng": centre.lng, "zoom": zoom})
        }
    };
    let points: Vec<JsonValue> = map.points.iter().map(point_json).collect();
    let data = json!({
        "tiles": ctx.settings.map_tiles(),
        "attribution": ctx.settings.map_attribution(),
        "view": view,
        "cluster": map.cluster,
        "showTitle": map.show_title,
        "clusterLabel": messages.map_place_count,
        "points": points,
    });
    format!(
        "<div class=\"romeka-map-canvas romeka-map-{}\" role=\"region\" aria-label=\"{}\" {MARKER}=\"{}\"></div>",
        map.height.as_str(),
        escape_html(messages.map),
        escape_html(&data.to_string())
    )
}

fn point_json(point: &MapPoint) -> JsonValue {
    let mut out = json!({
        "lat": point.place.lat,
        "lng": point.place.lng,
        "title": point.title,
        "url": point.url,
    });
    if let Some(object) = out.as_object_mut() {
        if let Some(thumbnail) = &point.thumbnail {
            object.insert("thumbnail".to_string(), json!(thumbnail));
        }
        if let Some(value) = &point.value {
            object.insert("value".to_string(), json!(value));
        }
    }
    out
}

/// The place list a map carries for readers without the map: each place as a
/// link to its item, then the place itself.
#[must_use]
pub fn place_list(points: &[MapPoint]) -> String {
    let mut out = String::from("<ul class=\"romeka-map-list\">");
    for point in points {
        out.push_str(&format!(
            "<li><a href=\"{}\">{}</a> <span class=\"romeka-map-place\">{}</span></li>",
            escape_html(&point.url),
            escape_html(&point.title),
            point.place
        ));
    }
    out.push_str("</ul>");
    out
}

/// The poster the script-free editor preview shows in place of a map.
#[must_use]
pub fn poster(points: &[MapPoint], ctx: &RenderContext<'_>) -> String {
    let messages = ctx.translator().messages();
    if points.is_empty() {
        return format!(
            "<div class=\"romeka-map-poster\"><p>{}</p></div>",
            escape_html(messages.map_no_places_preview)
        );
    }
    format!(
        "<div class=\"romeka-map-poster\"><p>{}</p>{}</div>",
        escape_html(&sprintf1(messages.map_poster, &points.len().to_string())),
        place_list(points)
    )
}

/// The item page's map section: the item's own places, centred on them. An
/// item with no place, or a site with no location field, shows nothing.
#[must_use]
pub fn item_section(resource: &ResourceView, ctx: &RenderContext<'_>) -> String {
    let Some(term) = ctx.settings.location_property() else {
        return String::new();
    };
    let points: Vec<MapPoint> = places_of(resource, term)
        .into_iter()
        .map(|place| MapPoint {
            place,
            title: resource.title.clone(),
            url: resource.url.clone(),
            thumbnail: None,
            value: None,
        })
        .collect();
    if points.is_empty() {
        return String::new();
    }
    if ctx.is_preview() {
        return format!(
            "<div class=\"romeka-map-section\">{}</div>",
            poster(&points, ctx)
        );
    }
    let map = MapView {
        points: &points,
        start: StartView::Fit,
        height: Height::Short,
        cluster: false,
        show_title: true,
    };
    format!(
        "<div class=\"romeka-map-section\">{}</div>",
        canvas(&map, ctx)
    )
}

/// The page content with Leaflet, its stylesheet and `map.js` appended when the
/// content holds a map, and unchanged otherwise.
#[must_use]
pub fn with_assets(content: String, ctx: &RenderContext<'_>) -> String {
    if !content.contains(MARKER) {
        return content;
    }
    let base = ctx.asset_base.trim_end_matches('/');
    let mut out = content;
    for asset in [LEAFLET_STYLESHEET, LEAFLET_SCRIPT, MAP_SCRIPT] {
        out.push_str(&asset.kind.element(&format!(
            "{base}/{}?v={}",
            escape_html(asset.path),
            asset.version()
        )));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks::test_support::{context, item, site};
    use crate::settings::SiteSettings;
    use crate::view::{NoResources, ValueRef};

    fn place(text: &str) -> Place {
        Place::parse(text).unwrap()
    }

    /// A place is two decimal degrees, latitude first, inside their ranges.
    #[test]
    fn parse_reads_lat_lng_inside_the_ranges() {
        assert_eq!(
            Place::parse("51.5072, -0.1276"),
            Place::new(51.5072, -0.1276)
        );
        assert_eq!(Place::parse(" -33.9,151.2 "), Place::new(-33.9, 151.2));
        assert_eq!(Place::parse("90, 180"), Place::new(90.0, 180.0));
        for text in [
            "",
            "51.5",
            "London",
            "91, 0",
            "0, 180.5",
            "-90.1, 0",
            "NaN, 0",
            "inf, 0",
            "51.5, -0.1, 3",
            "51.5; -0.1",
        ] {
            assert_eq!(Place::parse(text), None, "{text:?}");
        }
    }

    /// The stored form has at most six decimals and no trailing zeros.
    #[test]
    fn display_writes_at_most_six_decimals() {
        assert_eq!(
            place("51.50720000, -0.1276").to_string(),
            "51.5072, -0.1276"
        );
        assert_eq!(
            place("12.3456789, 98.7654321").to_string(),
            "12.345679, 98.765432"
        );
        assert_eq!(place("10, -0.0000001").to_string(), "10, 0");
    }

    fn located(term: &str, values: &[&str]) -> ResourceView {
        let mut resource = item(4, "A bridge", "/s/main/item/4");
        resource.values = values
            .iter()
            .map(|value| ValueRef {
                property_term: term.to_string(),
                value: (*value).to_string(),
                ..crate::blocks::test_support::literal_value(7, "Spatial", value)
            })
            .collect();
        resource
    }

    /// Only the location property's values that are places count.
    #[test]
    fn places_of_reads_the_location_values() {
        let resource = located("dcterms:spatial", &["51.5, -0.12", "London", "48.85, 2.35"]);
        assert_eq!(
            places_of(&resource, "dcterms:spatial"),
            vec![place("51.5, -0.12"), place("48.85, 2.35")]
        );
        assert!(places_of(&resource, "dcterms:coverage").is_empty());
    }

    /// The item section draws the item's own place, and draws nothing when the
    /// site names no location field or the item has no place.
    #[test]
    fn item_section_draws_the_items_place() {
        let site = site();
        let resource = located("dcterms:spatial", &["51.5, -0.12"]);
        let mut ctx = context(&site, &NoResources);
        assert_eq!(item_section(&resource, &ctx), "");
        ctx.settings = SiteSettings {
            romeka_location_property: Some("dcterms:spatial".to_string()),
            ..SiteSettings::default()
        };
        let drawn = item_section(&resource, &ctx);
        assert!(drawn.contains(MARKER), "{drawn}");
        assert!(drawn.contains("&quot;lat&quot;:51.5"), "{drawn}");
        assert!(drawn.contains("aria-label=\"Map\""), "{drawn}");
        let elsewhere = located("dcterms:spatial", &["Somewhere"]);
        assert_eq!(item_section(&elsewhere, &ctx), "");
    }

    /// Leaflet and the map script reach a page only when it holds a map.
    #[test]
    fn with_assets_loads_leaflet_only_when_a_map_is_present() {
        let site = site();
        let ctx = context(&site, &NoResources);
        let plain = "<p>No map here</p>".to_string();
        assert_eq!(with_assets(plain.clone(), &ctx), plain);
        let points = [MapPoint {
            place: place("51.5, -0.12"),
            title: "A bridge".to_string(),
            url: "/s/main/item/4".to_string(),
            thumbnail: None,
            value: None,
        }];
        let map = MapView {
            points: &points,
            start: StartView::Fit,
            height: Height::Medium,
            cluster: false,
            show_title: true,
        };
        let page = with_assets(canvas(&map, &ctx), &ctx);
        assert!(
            page.contains("<link rel=\"stylesheet\" href=\"/assets/vendor/leaflet/leaflet.css?v="),
            "{page}"
        );
        assert!(
            page.contains("<script defer src=\"/assets/vendor/leaflet/leaflet.js?v="),
            "{page}"
        );
        assert!(
            page.contains("<script defer src=\"/assets/js/map.js?v="),
            "{page}"
        );
        assert!(
            page.find("leaflet.js").unwrap() < page.find("js/map.js").unwrap(),
            "Leaflet runs before the map script"
        );
    }

    /// The canvas carries the basemap, the start view and each point as JSON
    /// the script reads, escaped for the attribute.
    #[test]
    fn canvas_carries_the_basemap_and_the_points() {
        let site = site();
        let mut ctx = context(&site, &NoResources);
        ctx.settings.romeka_map_tiles =
            Some("https://tiles.example.org/{z}/{x}/{y}.png".to_string());
        ctx.settings.romeka_map_attribution = Some("<b>Example</b> tiles".to_string());
        let points = [MapPoint {
            place: place("51.5, -0.12"),
            title: "A \"bridge\"".to_string(),
            url: "/s/main/item/4".to_string(),
            thumbnail: Some("/files/square/4.jpg".to_string()),
            value: Some("1894".to_string()),
        }];
        let map = MapView {
            points: &points,
            start: StartView::At {
                centre: place("48, 2"),
                zoom: 6,
            },
            height: Height::Tall,
            cluster: true,
            show_title: true,
        };
        let drawn = canvas(&map, &ctx);
        let encoded = drawn
            .split_once("data-romeka-map=\"")
            .and_then(|(_, rest)| rest.split_once('"'))
            .map(|(value, _)| value)
            .unwrap();
        let data: JsonValue = serde_json::from_str(
            &encoded
                .replace("&quot;", "\"")
                .replace("&lt;", "<")
                .replace("&gt;", ">")
                .replace("&amp;", "&"),
        )
        .unwrap();
        assert_eq!(data["tiles"], "https://tiles.example.org/{z}/{x}/{y}.png");
        assert_eq!(data["attribution"], "<b>Example</b> tiles");
        assert_eq!(data["view"], json!({"lat": 48.0, "lng": 2.0, "zoom": 6}));
        assert_eq!(data["cluster"], true);
        assert_eq!(
            data["points"][0],
            json!({
                "lat": 51.5, "lng": -0.12, "title": "A \"bridge\"", "url": "/s/main/item/4",
                "thumbnail": "/files/square/4.jpg", "value": "1894"
            })
        );
        assert!(drawn.contains("romeka-map-tall"), "{drawn}");
        assert!(!drawn.contains("<b>"), "the attribute is escaped: {drawn}");
    }
}
