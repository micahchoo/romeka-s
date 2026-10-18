//! The fallback media renderer (`Media/Renderer/Fallback.php`).
//!
//! `Manager::get` returns one of these for a name it does not know. It renders
//! nothing, so a media row with a renderer from an unported module still loads.

use crate::render::{MediaView, RenderOptions, Renderer, View};

/// A renderer that emits nothing.
#[derive(Debug, Clone, Copy, Default)]
pub struct Fallback;

impl Renderer for Fallback {
    fn name(&self) -> &str {
        "fallback"
    }

    fn render(&self, _view: &View<'_>, _media: &MediaView, _options: &RenderOptions) -> String {
        String::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::file_render::FileRendererRegistry;
    use crate::testing::media_view;

    /// `Renderer/Fallback.php:12` returns an empty string.
    #[test]
    fn fallback_renders_nothing() {
        let view = View {
            file_renderers: &FileRendererRegistry::with_defaults(),
        };
        assert_eq!(
            Fallback.render(&view, &media_view(), &RenderOptions::default()),
            ""
        );
    }
}
