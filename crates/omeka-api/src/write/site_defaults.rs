//! The content a new site is born with: `SiteAdapter::getDefaultNavigation`,
//! the welcome page `SiteAdapter::hydrate` adds on create (with the port's
//! starter in place of upstream's style sampler), and the port's starting
//! theme settings.

use omeka_db::ids::SiteId;
use omeka_db::repos::SiteRepo;
use omeka_db::rows::{NewSiteBlock, NewSitePage};
use omeka_site::blocks::templates::welcome_blocks;
use serde_json::{Value, json};

use crate::errors::ApiError;
use crate::read::{Api, db_error};
use crate::write::site_pages::carry_block;

/// `SiteAdapter::getDefaultNavigation`: one link to the browse page.
pub fn default_navigation() -> Value {
    json!([{
        "type": "browse",
        "data": { "label": "Browse", "query": "" },
        "links": [],
    }])
}

/// Store the theme settings a site created without `o:theme_settings` starts
/// with: the new-site preset (`omeka_site::theme::tokens::new_site_settings`).
/// A port addition: upstream's theme has no settings until a curator saves
/// some, and a map without a preset reads as the look sites had before
/// presets, so a new site needs the map written to get the new-site look.
pub async fn theme_settings(api: &Api<'_>, site: SiteId, theme: &str) -> Result<(), ApiError> {
    let settings = omeka_site::theme::tokens::new_site_settings();
    crate::theme_settings::store(api.pool(), site, theme, Some(&settings)).await
}

/// Build the welcome page `SiteAdapter::hydrate` adds on create.
///
/// Upstream fills it with a sampler of every text style. The port stores the
/// bundled starter instead (`omeka_site::blocks::templates::welcome_blocks`):
/// a banner, an introduction and the latest items. Every block is an upstream
/// layout, and the banner is a Romeka carrier whose plain rendering the write
/// path generates here, so upstream shows the same page without the styling.
pub async fn welcome_page(api: &Api<'_>, site: SiteId) -> Result<(), ApiError> {
    let repo = SiteRepo::new(api.pool());
    let page = repo
        .add_page(&NewSitePage {
            site_id: site,
            slug: "welcome".to_string(),
            title: "Welcome".to_string(),
            is_public: true,
            ..NewSitePage::default()
        })
        .await
        .map_err(db_error)?;
    for (position, block) in (1_i64..).zip(welcome_blocks()) {
        let layout_data = (!block.layout_data.is_null()).then_some(block.layout_data);
        let (data, layout_data) =
            carry_block(api, site, block.layout, block.data, layout_data).await?;
        repo.add_block(&NewSiteBlock {
            page_id: page.id,
            layout: block.layout.to_string(),
            data,
            layout_data,
            position,
        })
        .await
        .map_err(db_error)?;
    }
    Ok(())
}
