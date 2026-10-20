//! The IIIF viewer page (`Controller/IiifViewerController.php` and
//! `view/omeka/iiif-viewer/index.phtml`).
//!
//! A bare document, outside every site and theme, that opens one manifest or
//! collection in Mirador. Upstream builds a fixed default configuration, puts
//! the `url` query parameter in its one window, and then reads two keys, and
//! only those, from the `mirador_config` query parameter: `window.sideBarOpen`
//! and `selectedTheme`. An allow-list rather than a merge, so a link cannot
//! hand Mirador a configuration that fetches or runs something else.
//!
//! The port keeps that contract with one tightening: an allowed key applies
//! only with a value of its kind (a boolean, and `light` or `dark`), where
//! upstream copies any value through. The configuration rides in a data
//! attribute and [`INIT_SCRIPT`], a served file rather than an inline script,
//! starts Mirador, so the page's CSP can refuse every inline script.

use serde_json::{Value, json};

use crate::html::{escape_html, query_string};
use crate::settings::SiteSettings;
use crate::theme::library_script;

/// The script that starts Mirador with the page's configuration, served at
/// `js/iiif-viewer.js` under the asset base.
pub const INIT_SCRIPT: &str = include_str!("iiif-viewer.js");

/// `IiifViewerController::indexAction`'s configuration for one request:
/// upstream's defaults, the manifest in the one window, then the allowed keys
/// of `mirador_config`.
#[must_use]
pub fn mirador_config(manifest: Option<&str>, user_config: Option<&str>) -> Value {
    let mut config = json!({
        "id": "viewer",
        "workspaceControlPanel": {"enabled": false},
        "workspace": {"showZoomControls": true},
        "window": {
            "allowClose": false,
            "allowFullscreen": true,
            "allowMaximize": false,
            "sideBarOpen": true,
            "defaultSidebarPanelWidth": 300,
        },
        "windows": [{
            "manifestId": manifest,
            "thumbnailNavigationPosition": "far-bottom",
        }],
    });
    let user: Value = user_config
        .and_then(|raw| serde_json::from_str(raw).ok())
        .unwrap_or(Value::Null);
    if let Some(open) = user.get("window.sideBarOpen").and_then(Value::as_bool) {
        config["window"]["sideBarOpen"] = Value::Bool(open);
    }
    if let Some(theme) = user
        .get("selectedTheme")
        .and_then(Value::as_str)
        .filter(|theme| matches!(*theme, "light" | "dark"))
    {
        config["selectedTheme"] = Value::String(theme.to_string());
    }
    config
}

/// The viewer route's URL for one manifest, with the site's two viewer
/// settings as the `mirador_config` (`block-layout/iiif-presentation.phtml`,
/// `Media/Renderer/IiifPresentation.php`). `viewer_url` is the route,
/// `/iiif-viewer` or absolute.
#[must_use]
pub fn viewer_src(viewer_url: &str, manifest: &str, settings: &SiteSettings) -> String {
    let theme = if settings.iiif_viewer_theme() == "dark" {
        "dark"
    } else {
        "light"
    };
    // Built as a string rather than through a map, so the key order matches
    // `json_encode`'s insertion order whatever serde_json's map type is.
    let config = format!(
        "{{\"window.sideBarOpen\":{},\"selectedTheme\":\"{theme}\"}}",
        settings.iiif_viewer_sidebar()
    );
    let query = query_string(&[
        ("url".to_string(), manifest.to_string()),
        ("mirador_config".to_string(), config),
    ])
    .unwrap_or_default();
    format!("{}?{query}", viewer_url.trim_end_matches('?'))
}

/// `view/omeka/iiif-viewer/index.phtml`: the document for one configuration.
/// `asset_base` is where the theme's files are served, e.g. `/assets`.
#[must_use]
pub fn viewer_page(config: &Value, asset_base: &str) -> String {
    format!(
        "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n\
         <meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <title>Mirador</title>\n\
         <style>html, body {{ margin: 0; height: 100%; }} #viewer {{ position: relative; height: 100%; }}</style>\n\
         {mirador}\n\
         </head>\n<body>\n\
         <div id=\"viewer\" data-mirador-config=\"{config}\"></div>\n\
         {init}\n\
         </body>\n</html>\n",
        mirador = library_script(asset_base, "vendor/mirador/mirador.min.js"),
        init = library_script(asset_base, "js/iiif-viewer.js"),
        config = escape_html(&config.to_string()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `IiifViewerController::indexAction`: the defaults, with the `url` as the
    /// one window's manifest.
    #[test]
    fn the_default_configuration_matches_upstream() {
        let config = mirador_config(Some("https://example.org/m.json"), None);
        assert_eq!(
            config,
            json!({
                "id": "viewer",
                "workspaceControlPanel": {"enabled": false},
                "workspace": {"showZoomControls": true},
                "window": {
                    "allowClose": false,
                    "allowFullscreen": true,
                    "allowMaximize": false,
                    "sideBarOpen": true,
                    "defaultSidebarPanelWidth": 300,
                },
                "windows": [{
                    "manifestId": "https://example.org/m.json",
                    "thumbnailNavigationPosition": "far-bottom",
                }],
            })
        );
    }

    /// The two allowed keys apply; any other key, and an allowed key with a
    /// value of the wrong kind, changes nothing.
    #[test]
    fn mirador_config_applies_only_the_allow_list() {
        let config = mirador_config(
            Some("https://example.org/m.json"),
            Some(
                r#"{"window.sideBarOpen":false,"selectedTheme":"dark","window":{"allowClose":true},"requests":{"preprocessors":[]}}"#,
            ),
        );
        assert_eq!(config["window"]["sideBarOpen"], json!(false));
        assert_eq!(config["selectedTheme"], json!("dark"));
        assert_eq!(config["window"]["allowClose"], json!(false));
        assert!(config.get("requests").is_none());

        let refused = mirador_config(
            None,
            Some(r#"{"window.sideBarOpen":"yes","selectedTheme":"neon"}"#),
        );
        assert_eq!(refused["window"]["sideBarOpen"], json!(true));
        assert!(refused.get("selectedTheme").is_none());
        assert_eq!(refused["windows"][0]["manifestId"], Value::Null);
        assert_eq!(
            mirador_config(None, Some("not json")),
            mirador_config(None, None)
        );
    }

    /// The page loads Mirador and the starter from the asset base, and the
    /// configuration reaches the data attribute escaped, so a manifest URL
    /// carrying markup cannot leave the attribute.
    #[test]
    fn the_page_carries_the_configuration_escaped() {
        let config = mirador_config(Some("https://example.org/\"><script>x</script>"), None);
        let page = viewer_page(&config, "/assets/");
        assert!(
            page.contains("<script src=\"/assets/vendor/mirador/mirador.min.js?v="),
            "{page}"
        );
        assert!(
            page.contains("<script src=\"/assets/js/iiif-viewer.js?v="),
            "{page}"
        );
        assert!(page.contains("data-mirador-config=\"{&quot;"), "{page}");
        assert!(!page.contains("<script>x"), "{page}");
        assert_eq!(
            page.matches("<script").count(),
            2,
            "no inline script: {page}"
        );
    }
}
