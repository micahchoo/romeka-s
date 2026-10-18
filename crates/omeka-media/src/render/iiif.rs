//! The IIIF image media renderer (`Media/Renderer/IIIF.php`).
//!
//! Emits the OpenSeadragon container and its initialisation script, with the
//! stored image information document as the tile source.

use crate::data::MediaData;
use crate::render::{MediaView, RenderOptions, Renderer, View};
use crate::text::escape_html;

/// The renderer's HTML template, copied from the heredoc in
/// `Renderer/IIIF.php:29`, including its indentation.
const TEMPLATE: &str = r#"        <div class="openseadragon" id="%1$s" data-prefix-url="%2$s" data-tile-sources="%3$s" style="height: %4$s;"></div>
        <script>
        {
            const openSeadragonDiv = document.getElementById('%1$s');
            const viewer = OpenSeadragon({
                id: '%1$s',
                prefixUrl: openSeadragonDiv.dataset.prefixUrl,
                tileSources: [JSON.parse(openSeadragonDiv.dataset.tileSources)]
            });
        }
        </script>"#;

/// The IIIF image renderer.
#[derive(Debug, Clone, Copy, Default)]
pub struct Iiif;

impl Renderer for Iiif {
    fn name(&self) -> &str {
        "iiif"
    }

    /// `Renderer/IIIF.php:16`. The height defaults to `400px`; any `height`
    /// option is cast to an integer and suffixed with `px`.
    fn render(&self, _view: &View<'_>, media: &MediaView, options: &RenderOptions) -> String {
        let MediaData::Iiif(data) = &media.data else {
            return String::new();
        };
        let id = format!("iiif-{}", media.media.core.id.get());
        let prefix_url = format!("{}/vendor/openseadragon/images/", media.env.asset_base_path);
        let tile_sources = sorted_json(&data.value);
        let height = options
            .height
            .as_deref()
            .map(|value| format!("{}px", value.parse::<i64>().unwrap_or(0)))
            .unwrap_or_else(|| "400px".to_string());
        TEMPLATE
            .replacen("%1$s", &id, 4)
            .replacen("%2$s", &escape_html(&prefix_url), 1)
            .replacen("%3$s", &escape_html(&tile_sources), 1)
            .replacen("%4$s", &height, 1)
    }
}

/// Serialise a stored document with sorted object keys.
///
/// The workspace enables `serde_json/preserve_order` through `omeka-api`, and
/// Cargo unifies features across a workspace, so a plain `to_string` here emits
/// insertion order on one build and sorted order on another. Sorting keeps the
/// markup stable. Upstream preserves insertion order; the scenario harness
/// compares parsed documents, so a stable order is what matters here.
fn sorted_json(value: &serde_json::Value) -> String {
    fn sort(value: &serde_json::Value) -> serde_json::Value {
        match value {
            serde_json::Value::Object(map) => {
                let sorted: std::collections::BTreeMap<_, _> = map
                    .iter()
                    .map(|(key, entry)| (key.clone(), sort(entry)))
                    .collect();
                serde_json::Value::Object(sorted.into_iter().collect())
            }
            serde_json::Value::Array(items) => {
                serde_json::Value::Array(items.iter().map(sort).collect())
            }
            other => other.clone(),
        }
    }
    serde_json::to_string(&sort(value)).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::file_render::FileRendererRegistry;
    use crate::render::RenderEnv;
    use crate::testing::{media_view_with_data, media_view_with_data_and_env};

    fn view(registry: &FileRendererRegistry) -> View<'_> {
        View {
            file_renderers: registry,
        }
    }

    fn iiif_media() -> MediaView {
        let value = serde_json::json!({
            "@context": "http://iiif.io/api/image/2/context.json",
            "@id": "https://example.org/iiif/1",
            "width": 1000,
            "height": 800
        });
        let data = MediaData::parse("iiif", Some(&value)).expect("iiif data");
        media_view_with_data(data)
    }

    /// `Renderer/IIIF.php:20`: the container, the tile source and the default
    /// height.
    #[test]
    fn iiif_renderer_matches_upstream_markup() {
        let registry = FileRendererRegistry::with_defaults();
        let html = Iiif.render(&view(&registry), &iiif_media(), &RenderOptions::default());
        assert_eq!(
            html,
            concat!(
                "        <div class=\"openseadragon\" id=\"iiif-1\" data-prefix-url=\"/application/asset/vendor/openseadragon/images/\" ",
                "data-tile-sources=\"{&quot;@context&quot;:&quot;http://iiif.io/api/image/2/context.json&quot;,&quot;@id&quot;:&quot;https://example.org/iiif/1&quot;,&quot;height&quot;:800,&quot;width&quot;:1000}\" ",
                "style=\"height: 400px;\"></div>\n",
                "        <script>\n",
                "        {\n",
                "            const openSeadragonDiv = document.getElementById('iiif-1');\n",
                "            const viewer = OpenSeadragon({\n",
                "                id: 'iiif-1',\n",
                "                prefixUrl: openSeadragonDiv.dataset.prefixUrl,\n",
                "                tileSources: [JSON.parse(openSeadragonDiv.dataset.tileSources)]\n",
                "            });\n",
                "        }\n",
                "        </script>"
            )
        );
    }

    /// `Renderer/IIIF.php:26`: a `height` option is cast to an integer and
    /// suffixed with `px`.
    #[test]
    fn iiif_renderer_uses_the_height_option() {
        let registry = FileRendererRegistry::with_defaults();
        let options = RenderOptions {
            height: Some("512".to_string()),
            ..RenderOptions::default()
        };
        let html = Iiif.render(&view(&registry), &iiif_media(), &options);
        assert!(html.contains("style=\"height: 512px;\""), "{html}");
    }

    /// The OpenSeadragon prefix follows the configured asset base.
    #[test]
    fn iiif_renderer_uses_the_asset_base_path() {
        let registry = FileRendererRegistry::with_defaults();
        let env = RenderEnv {
            asset_base_path: "/omeka/application/asset".to_string(),
            ..RenderEnv::default()
        };
        let html = Iiif.render(
            &view(&registry),
            &media_view_with_data_and_env(iiif_media().data, env),
            &RenderOptions::default(),
        );
        assert!(
            html.contains(
                "data-prefix-url=\"/omeka/application/asset/vendor/openseadragon/images/\""
            ),
            "{html}"
        );
    }
}
