//! The IIIF presentation media renderer (`Media/Renderer/IiifPresentation.php`).
//!
//! Builds the Mirador viewer query from the media source and the two
//! `iiif_viewer_*` site settings, then emits the viewer iframe.

use crate::data::MediaData;
use crate::render::{MediaView, RenderOptions, Renderer, View};
use crate::text::{escape_html, urlencode};

/// The IIIF presentation renderer.
#[derive(Debug, Clone, Copy, Default)]
pub struct IiifPresentation;

impl Renderer for IiifPresentation {
    fn name(&self) -> &str {
        "iiif_presentation"
    }

    /// `Renderer/IiifPresentation.php:14`, which delegates to
    /// `View/Helper/IiifViewer.php:26`.
    fn render(&self, _view: &View<'_>, media: &MediaView, options: &RenderOptions) -> String {
        let MediaData::IiifPresentation(_) = &media.data else {
            return String::new();
        };
        let theme = match media.env.iiif_viewer_theme.as_str() {
            "dark" => "dark",
            _ => "light",
        };
        // Built by hand so the member order matches PHP's insertion order:
        // `window.sideBarOpen` then `selectedTheme`.
        let mirador_config = format!(
            "{{\"window.sideBarOpen\":{},\"selectedTheme\":\"{}\"}}",
            media.env.iiif_viewer_sidebar, theme
        );
        let source = media.media.source.clone().unwrap_or_default();
        let src = format!(
            "{}?url={}&mirador_config={}",
            media.env.viewer_url,
            urlencode(&source),
            urlencode(&mirador_config)
        );
        let width = options.width.clone().unwrap_or_else(|| "100%".to_string());
        let height = options
            .height
            .clone()
            .unwrap_or_else(|| "700px".to_string());
        let title = options.title.clone().unwrap_or_default();
        format!(
            "<iframe style=\"width: {width}; height: {height};\" aria-label=\"{}\" src=\"{}\"></iframe>",
            escape_html(&title),
            escape_html(&src)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::file_render::FileRendererRegistry;
    use crate::render::RenderEnv;
    use crate::testing::media_view_with_data_and_env;

    fn view(registry: &FileRendererRegistry) -> View<'_> {
        View {
            file_renderers: registry,
        }
    }

    fn presentation_media(env: RenderEnv) -> MediaView {
        let value = serde_json::json!({
            "@context": "http://iiif.io/api/presentation/2/context.json",
            "@type": "sc:Manifest"
        });
        let data = MediaData::parse("iiif_presentation", Some(&value)).expect("data");
        let mut media = media_view_with_data_and_env(data, env);
        media.media.source = Some("https://example.org/manifest".to_string());
        media
    }

    /// `Renderer/IiifPresentation.php:18`: the default theme is light and the
    /// sidebar is closed, and the viewer iframe carries the query.
    #[test]
    fn iiif_presentation_renderer_matches_upstream_defaults() {
        let registry = FileRendererRegistry::with_defaults();
        let html = IiifPresentation.render(
            &view(&registry),
            &presentation_media(RenderEnv::default()),
            &RenderOptions::default(),
        );
        assert_eq!(
            html,
            concat!(
                "<iframe style=\"width: 100%; height: 700px;\" aria-label=\"\" ",
                "src=\"https://example.org/iiif-viewer?url=https%3A%2F%2Fexample.org%2Fmanifest",
                "&amp;mirador_config=%7B%22window.sideBarOpen%22%3Afalse%2C%22selectedTheme%22%3A%22light%22%7D\"></iframe>"
            )
        );
    }

    /// The two site settings change the Mirador configuration
    /// (`Renderer/IiifPresentation.php:19`).
    #[test]
    fn iiif_presentation_renderer_reads_the_viewer_settings() {
        let registry = FileRendererRegistry::with_defaults();
        let env = RenderEnv {
            iiif_viewer_sidebar: true,
            iiif_viewer_theme: "dark".to_string(),
            ..RenderEnv::default()
        };
        let html = IiifPresentation.render(
            &view(&registry),
            &presentation_media(env),
            &RenderOptions::default(),
        );
        assert!(
            html.contains("%22window.sideBarOpen%22%3Atrue%2C%22selectedTheme%22%3A%22dark%22"),
            "{html}"
        );
    }

    /// The iframe size and label come from the options
    /// (`View/Helper/IiifViewer.php:29`).
    #[test]
    fn iiif_presentation_renderer_uses_the_viewer_options() {
        let registry = FileRendererRegistry::with_defaults();
        let options = RenderOptions {
            width: Some("50%".to_string()),
            height: Some("300px".to_string()),
            title: Some("A manifest".to_string()),
            ..RenderOptions::default()
        };
        let html = IiifPresentation.render(
            &view(&registry),
            &presentation_media(RenderEnv::default()),
            &options,
        );
        assert!(
            html.starts_with(
                "<iframe style=\"width: 50%; height: 300px;\" aria-label=\"A manifest\""
            ),
            "{html}"
        );
    }
}
