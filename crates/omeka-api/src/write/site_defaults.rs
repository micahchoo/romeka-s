//! The content a new site is born with: the welcome page
//! `SiteAdapter::hydrate` adds on create (with the port's starter in place of
//! upstream's style sampler), a navigation that links it, and the port's
//! starting theme settings.

use omeka_db::ids::PageId;
use omeka_db::rows::{NewFirstPage, NewFirstPageBlock};
use omeka_site::blocks::templates::welcome_blocks;
use serde_json::{Value, json};

use crate::errors::ApiError;
use crate::read::Api;
use crate::write::site_pages::carry_block;

/// The navigation a site created without `o:navigation` gets: the welcome
/// page, then `SiteAdapter::getDefaultNavigation`'s browse link.
///
/// A port addition: upstream links the browse page alone, so a new site's root
/// shows "This site has no pages." until a curator links one. With the welcome
/// page first, `Site\IndexController::indexAction` sends the root to it. The
/// navigation carries this, not `o:homepage`, so a curator who unlinks or
/// replaces the page moves the root with it.
pub fn default_navigation(welcome: PageId) -> Value {
    json!([
        { "type": "page", "data": { "label": null, "id": welcome.0 }, "links": [] },
        { "type": "browse", "data": { "label": "Browse", "query": "" }, "links": [] },
    ])
}

/// The theme settings a site created without `o:theme_settings` starts with:
/// the new-site preset (`omeka_site::theme::tokens::new_site_settings`).
/// A port addition: upstream's theme has no settings until a curator saves
/// some, and a map without a preset reads as the look sites had before
/// presets, so a new site needs the map written to get the new-site look.
pub fn theme_settings() -> Value {
    omeka_site::theme::tokens::new_site_settings()
}

/// The welcome page `SiteAdapter::hydrate` adds on create.
///
/// Upstream fills it with a sampler of every text style. The port stores the
/// bundled starter instead (`omeka_site::blocks::templates::welcome_blocks`):
/// a banner, an introduction and the latest items. Every block is an upstream
/// layout, and the banner is a Romeka carrier whose plain rendering the write
/// path generates here, so upstream shows the same page without the styling.
pub async fn welcome_page(api: &Api<'_>) -> Result<NewFirstPage, ApiError> {
    let mut blocks = Vec::new();
    for block in welcome_blocks() {
        let layout_data = (!block.layout_data.is_null()).then_some(block.layout_data);
        let (data, layout_data) =
            carry_block(api, None, block.layout, block.data, layout_data).await?;
        blocks.push(NewFirstPageBlock {
            layout: block.layout.to_string(),
            data,
            layout_data,
        });
    }
    Ok(NewFirstPage {
        slug: "welcome".to_string(),
        title: "Welcome".to_string(),
        is_public: true,
        blocks,
    })
}
